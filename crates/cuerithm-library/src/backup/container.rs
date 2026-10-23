use super::{format::*, validate::*};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{BufRead, BufReader, Read, Seek, Write},
    path::Path,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};
pub(crate) fn open_zip(path: &Path, l: &ArchiveLimits) -> Result<ZipArchive<File>> {
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        metadata.file_type().is_file() && metadata.len() <= l.compressed_bytes,
        "Archive must be a regular file within compressed limit"
    );
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000);
    }
    let mut file = options.open(path)?;
    ensure!(
        file.metadata()?.is_file() && file.metadata()?.len() == metadata.len(),
        "Archive changed during opening"
    );
    let mut magic = [0; 4];
    file.read_exact(&mut magic)?;
    ensure!(&magic == b"PK\x03\x04", "Invalid ZIP magic");
    preflight_directory(&mut file, l)?;
    file.rewind()?;
    let mut zip = ZipArchive::new(file)?;
    ensure!(zip.len() <= l.entries, "Too many archive entries");
    let mut names = BTreeSet::new();
    let mut ranges = vec![];
    let mut total = 0u64;
    for i in 0..zip.len() {
        let f = zip.by_index(i)?;
        let name = f.name()?;
        ensure!(
            !name.is_empty()
                && !name.contains(['\\', '\0', ':'])
                && !name.starts_with('/')
                && name.split('/').all(|p| p != ".." && p != "."),
            "Unsafe archive name"
        );
        ensure!(
            names.insert(name.to_lowercase()),
            "Duplicate or case-equivalent archive name"
        );
        ensure!(
            matches!(
                f.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            ),
            "Unsupported compression"
        );
        ensure!(!f.encrypted(), "Encrypted archive entry");
        if let Some(mode) = f.unix_mode() {
            let kind = mode & 0o170000;
            ensure!(
                kind == 0 || kind == 0o100000 || kind == 0o040000,
                "Nonregular archive entry"
            );
        }
        ensure!(f.size() <= l.entry_bytes, "Entry expanded limit");
        total = total
            .checked_add(f.size())
            .context("Expanded length overflow")?;
        ensure!(total <= l.expanded_bytes, "Archive expanded limit");
        let start = f.header_start();
        let data_start = f.data_start().context("Unvalidated local header")?;
        ensure!(start <= data_start, "Invalid local header range");
        let end = data_start
            .checked_add(f.compressed_size())
            .context("Entry offset overflow")?;
        ensure!(end <= metadata.len(), "Invalid entry offsets");
        ranges.push((start, end));
    }
    ranges.sort();
    for w in ranges.windows(2) {
        ensure!(w[0].1 <= w[1].0, "Overlapping archive entries");
    }
    Ok(zip)
}
pub fn read_portable_archive(path: &Path, l: &ArchiveLimits) -> Result<LogicalArchive> {
    read_portable_archive_with_control(path, l, &|_, _| Ok(()))
}
pub fn read_portable_archive_with_control(
    path: &Path,
    l: &ArchiveLimits,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<LogicalArchive> {
    let mut zip = open_zip(path, l)?;
    let manifest = {
        let f = zip.by_name("manifest.json")?;
        ensure!(f.size() <= l.manifest_bytes, "Manifest size limit");
        let mut bytes = vec![];
        f.take(l.manifest_bytes + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= l.manifest_bytes,
            "Manifest size limit"
        );
        serde_json::from_value::<Manifest>(parse_strict_json(&bytes, l.depth)?)?
    };
    let mut a = LogicalArchive {
        manifest,
        records: Default::default(),
        optional_payloads: Default::default(),
    };
    let mut names = BTreeSet::from(["manifest.json".to_string()]);
    let mut kinds = BTreeSet::new();
    let mut memory = 0u64;
    for d in &a.manifest.entries {
        control("read", memory)?;
        ensure!(
            names.insert(d.name.clone()) && kinds.insert(d.kind.clone()),
            "Duplicate manifest payload"
        );
        digest(&d.sha256)?;
        ensure!(d.schema == "cuerithm-logical-v1", "Unknown logical schema");
        let known = KINDS.iter().find(|(k, _)| *k == d.kind);
        if let Some((_, name)) = known {
            ensure!(
                *name == d.name && d.required,
                "Invalid required entry descriptor"
            );
        } else {
            ensure!(!d.required, "Unknown required payload kind");
        }
        let file = zip.by_name(&d.name)?;
        ensure!(
            file.size() == d.bytes && d.bytes <= l.entry_bytes,
            "Entry length mismatch"
        );
        let mut read = BufReader::new(file);
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut count = 0u64;
        let mut records = vec![];
        let mut inert = vec![];
        loop {
            let mut line = vec![];
            let n = read
                .by_ref()
                .take(l.record_bytes as u64 + 1)
                .read_until(b'\n', &mut line)?;
            if n == 0 {
                break;
            }
            ensure!(
                n <= l.record_bytes && line.last() == Some(&b'\n'),
                "Record size limit or missing LF terminator"
            );
            ensure!(line.len() > 1 && line[0] != 0xef, "Blank line or BOM");
            memory = memory
                .checked_add((n as u64).saturating_mul(3))
                .context("Memory budget overflow")?;
            ensure!(
                memory <= l.memory_bytes,
                "Archive exceeds in-memory staging budget; use a smaller library snapshot"
            );
            control("read", bytes)?;
            hash.update(&line);
            bytes += n as u64;
            ensure!(bytes <= d.bytes, "Actual expanded size limit");
            count += 1;
            ensure!(
                count
                    <= if d.kind == "tracks" {
                        l.tracks as u64
                    } else {
                        l.relations as u64
                    },
                "Record limit"
            );
            if known.is_some() {
                let value = parse_strict_json(&line[..n - 1], l.depth)?;
                records.push(serde_json::from_value::<LogicalRecord>(value)?);
            } else {
                inert.extend_from_slice(&line);
            }
        }
        ensure!(
            bytes == d.bytes && count == d.records && format!("{:x}", hash.finalize()) == d.sha256,
            "Payload digest/count mismatch"
        );
        if known.is_some() {
            a.records.insert(d.kind.clone(), records);
        } else {
            a.optional_payloads.insert(d.name.clone(), inert);
        }
    }
    ensure!(zip.len() == names.len(), "Unexpected archive entries");
    for i in 0..zip.len() {
        ensure!(
            names.contains(zip.by_index(i)?.name()?.as_ref()),
            "Unlisted archive entry"
        );
    }
    validate_logical_archive(&a, l)?;
    Ok(a)
}
pub fn write_portable_archive(
    path: &Path,
    a: &LogicalArchive,
    l: &ArchiveLimits,
) -> Result<Manifest> {
    write_portable_archive_with_control(path, a, l, &|_, _| Ok(()))
}
pub fn write_portable_archive_with_control(
    path: &Path,
    a: &LogicalArchive,
    l: &ArchiveLimits,
    control: &dyn Fn(&str, u64) -> Result<()>,
) -> Result<Manifest> {
    validate_logical_archive(a, l)?;
    ensure!(!path.exists(), "Backup destination exists");
    let parent = path.parent().unwrap_or(Path::new("."));
    ensure!(
        fs2::available_space(parent)? >= 256 << 20,
        "Insufficient free space: retain 256 MiB reserve"
    );
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    let mut manifest = a.manifest.clone();
    manifest.archive_id = uuid::Uuid::new_v4().to_string();
    manifest.entries.clear();
    {
        let mut zip = ZipWriter::new(temp.as_file_mut());
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .large_file(true);
        for (kind, name) in KINDS {
            ensure!(
                fs2::available_space(parent)? >= 256 << 20,
                "Insufficient free space during archive export"
            );
            zip.start_file(*name, options)?;
            let mut hash = Sha256::new();
            let mut bytes = 0u64;
            let mut rows = a.records[*kind].iter().collect::<Vec<_>>();
            rows.sort_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));
            for row in &rows {
                control("write", bytes)?;
                let mut line = serde_json::to_vec(row)?;
                line.push(b'\n');
                ensure!(line.len() <= l.record_bytes, "Record byte limit");
                bytes += line.len() as u64;
                ensure!(bytes <= l.entry_bytes, "Entry size limit");
                hash.update(&line);
                zip.write_all(&line)?;
            }
            manifest.entries.push(EntryDescriptor {
                name: name.to_string(),
                kind: kind.to_string(),
                bytes,
                records: rows.len() as u64,
                sha256: format!("{:x}", hash.finalize()),
                schema: "cuerithm-logical-v1".into(),
                required: true,
            });
        }
        for (name, data) in &a.optional_payloads {
            let d = a
                .manifest
                .entries
                .iter()
                .find(|d| &d.name == name && !d.required)
                .context("Optional payload descriptor missing")?;
            zip.start_file(name, options)?;
            zip.write_all(data)?;
            manifest.entries.push(d.clone());
        }
        let bytes = serde_json::to_vec(&manifest)?;
        ensure!(bytes.len() as u64 <= l.manifest_bytes, "Manifest limit");
        zip.start_file("manifest.json", options)?;
        zip.write_all(&bytes)?;
        zip.finish()?;
    }
    temp.as_file().sync_all()?;
    read_portable_archive_with_control(temp.path(), l, control)?;
    control("publish", 0)?;
    temp.persist_noclobber(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(manifest)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveFormat {
    Portable,
    Lexicon,
}
pub fn detect_archive_format(path: &Path, limits: &ArchiveLimits) -> Result<ArchiveFormat> {
    let mut zip = open_zip(path, limits)?;
    let mut portable = false;
    let mut lexicon = false;
    for index in 0..zip.len() {
        match zip.by_index(index)?.name()?.as_ref() {
            "manifest.json" => portable = true,
            "main.db" => lexicon = true,
            _ => {}
        }
    }
    ensure!(
        portable != lexicon,
        "Archive must contain exactly one recognized root format"
    );
    Ok(if portable {
        ArchiveFormat::Portable
    } else {
        ArchiveFormat::Lexicon
    })
}

fn preflight_directory(file: &mut File, l: &ArchiveLimits) -> Result<()> {
    use std::io::SeekFrom;
    let size = file.metadata()?.len();
    let tail_len = size.min(65557) as usize;
    file.seek(SeekFrom::End(-(tail_len as i64)))?;
    let mut tail = vec![0; tail_len];
    file.read_exact(&mut tail)?;
    let offset = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|i| {
            tail[*i..].starts_with(b"PK\x05\x06")
                && *i + 22 + u16::from_le_bytes([tail[*i + 20], tail[*i + 21]]) as usize
                    == tail.len()
        })
        .context("ZIP end directory missing")?;
    let end = &tail[offset..];
    let u16_at = |i| u16::from_le_bytes([end[i], end[i + 1]]);
    ensure!(
        u16_at(4) == 0 && u16_at(6) == 0,
        "Multipart archives unsupported"
    );
    let entries = if u16_at(10) == u16::MAX {
        let absolute = size - tail_len as u64 + offset as u64;
        ensure!(absolute >= 20, "Missing ZIP64 locator");
        file.seek(SeekFrom::Start(absolute - 20))?;
        let mut locator = [0; 20];
        file.read_exact(&mut locator)?;
        ensure!(
            &locator[..4] == b"PK\x06\x07"
                && u32::from_le_bytes(locator[4..8].try_into().unwrap()) == 0
                && u32::from_le_bytes(locator[16..20].try_into().unwrap()) == 1,
            "Invalid/multipart ZIP64 locator"
        );
        let position = u64::from_le_bytes(locator[8..16].try_into().unwrap());
        ensure!(position < absolute - 20, "Invalid ZIP64 end offset");
        file.seek(SeekFrom::Start(position))?;
        let mut record = [0; 56];
        file.read_exact(&mut record)?;
        ensure!(
            &record[..4] == b"PK\x06\x06" && record[16..24] == [0; 8],
            "Invalid/multipart ZIP64 directory"
        );
        let disk = u64::from_le_bytes(record[24..32].try_into().unwrap());
        let total = u64::from_le_bytes(record[32..40].try_into().unwrap());
        ensure!(disk == total, "Multipart ZIP64 entries");
        total
    } else {
        ensure!(u16_at(8) == u16_at(10), "Multipart directory entries");
        u16_at(10) as u64
    };
    ensure!(entries <= l.entries as u64, "Too many archive entries");
    Ok(())
}
