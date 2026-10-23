//! Page and row encoders for the removable-device PDB interchange format.
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, HashSet};

pub(super) type Tables = BTreeMap<u32, Vec<Vec<u8>>>;
const PAGE: usize = 4096;
pub(super) fn u16le(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
pub(super) fn u32le(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
pub(crate) fn read16(bytes: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        bytes
            .get(at..at + 2)
            .context("Truncated PDB word")?
            .try_into()?,
    ))
}
pub(crate) fn read32(bytes: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .context("Truncated PDB integer")?
            .try_into()?,
    ))
}
pub(super) fn pad(mut bytes: Vec<u8>) -> Vec<u8> {
    bytes.resize(bytes.len().next_multiple_of(4), 0);
    bytes
}
pub(super) fn string(text: &str) -> Result<Vec<u8>> {
    if text.is_ascii() && text.len() <= 125 {
        let mut bytes = vec![(text.len() * 2 + 3) as u8];
        bytes.extend(text.as_bytes());
        return Ok(bytes);
    }
    let payload = if text.is_ascii() {
        text.as_bytes().to_vec()
    } else {
        text.encode_utf16().flat_map(u16::to_le_bytes).collect()
    };
    let length = u16::try_from(payload.len() + 4).context("PDB string too long")?;
    let mut bytes = vec![if text.is_ascii() { 0x40 } else { 0x90 }];
    bytes.extend(length.to_le_bytes());
    bytes.push(0);
    bytes.extend(payload);
    Ok(bytes)
}
pub(super) fn display(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{200b}'..='\u{200d}' | '\u{2060}' | '\u{feff}')
        })
        .take(255)
        .collect()
}
pub(super) fn named(id: u32, text: &str) -> Result<Vec<u8>> {
    let mut row = id.to_le_bytes().to_vec();
    row.extend(string(&display(text))?);
    Ok(pad(row))
}
pub(super) fn artist(id: u32, text: &str) -> Result<Vec<u8>> {
    let name = string(&display(text))?;
    let start = if name[0] & 1 == 1 { 10 } else { 12 };
    let mut row = vec![0; 16 + name.len().next_multiple_of(4)];
    u16le(&mut row, 0, 96);
    u32le(&mut row, 4, id);
    row[8] = 3;
    row[9] = start as u8;
    row[start..start + name.len()].copy_from_slice(&name);
    Ok(pad(row))
}
pub(super) fn album(id: u32, text: &str) -> Result<Vec<u8>> {
    let name = string(&display(text))?;
    let start = if name[0] & 1 == 1 { 22 } else { 24 };
    let mut row = vec![0; 28 + name.len().next_multiple_of(4)];
    u16le(&mut row, 0, 128);
    u32le(&mut row, 12, id);
    row[20] = 3;
    row[21] = start as u8;
    row[start..start + name.len()].copy_from_slice(&name);
    Ok(pad(row))
}
pub(super) fn key(id: u32, text: &str) -> Result<Vec<u8>> {
    let mut row = vec![0; 8];
    u32le(&mut row, 0, id);
    u32le(&mut row, 4, id);
    row.extend(string(&display(text))?);
    Ok(pad(row))
}
pub(super) fn playlist(
    id: u32,
    parent: u32,
    order: u32,
    folder: bool,
    name: &str,
) -> Result<Vec<u8>> {
    let mut row = vec![0; 20];
    u32le(&mut row, 0, parent);
    u32le(&mut row, 8, order);
    u32le(&mut row, 12, id);
    u32le(&mut row, 16, u32::from(folder));
    row.extend(string(&display(name))?);
    Ok(pad(row))
}
pub(super) fn playlist_entry(list: u32, track: u32, index: u32) -> Vec<u8> {
    let mut row = vec![0; 12];
    u32le(&mut row, 0, index);
    u32le(&mut row, 4, track);
    u32le(&mut row, 8, list);
    row
}

/// Preserve opaque device menu/category/sort/name records; never reinterpret settings.
#[cfg(test)]
pub(super) fn config(bytes: &[u8]) -> Result<Tables> {
    Ok(read_tables(bytes)?
        .into_iter()
        .filter(|(kind, _)| (16..=19).contains(kind))
        .collect())
}
pub(crate) fn decode_string(bytes: &[u8], offset: usize) -> Result<String> {
    let first = *bytes.get(offset).context("PDB string offset outside row")?;
    if first & 1 == 1 {
        let length = (first >> 1).saturating_sub(1) as usize;
        return Ok(std::str::from_utf8(
            bytes
                .get(offset + 1..offset + 1 + length)
                .context("Truncated short PDB string")?,
        )?
        .into());
    }
    let size = read16(bytes, offset + 1)? as usize;
    ensure!(size >= 4, "Invalid long PDB string size");
    let payload = bytes
        .get(offset + 4..offset + size)
        .context("Truncated long PDB string")?;
    match first {
        0x40 => Ok(std::str::from_utf8(payload)?.into()),
        0x90 => {
            ensure!(payload.len().is_multiple_of(2), "Odd UTF-16 PDB string");
            let words = payload
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_le_bytes(*b))
                .collect::<Vec<_>>();
            Ok(String::from_utf16(&words)?)
        }
        _ => anyhow::bail!("Unsupported PDB string encoding"),
    }
}
pub(super) fn read_tables(bytes: &[u8]) -> Result<Tables> {
    let size = read32(bytes, 4)? as usize;
    let count = read32(bytes, 8)? as usize;
    ensure!(
        (128..=65536).contains(&size) && bytes.len().is_multiple_of(size) && count <= 256,
        "Invalid PDB header"
    );
    let mut tables = Tables::new();
    for n in 0..count {
        let pointer = 28 + n * 16;
        let kind = read32(bytes, pointer)?;
        let mut page = read32(bytes, pointer + 8)? as usize;
        let mut visited = HashSet::new();
        let mut rows = Vec::new();
        while page > 0 && page < bytes.len() / size && visited.insert(page) {
            let p = bytes
                .get(page * size..(page + 1) * size)
                .context("PDB page outside file")?;
            let next = read32(p, 12)? as usize;
            if p[27] & 0x40 == 0 {
                let num = (u32::from(p[24]) | (u32::from(p[25]) << 8) | (u32::from(p[26]) << 16))
                    as usize
                    & 8191;
                let used = read16(p, 30)? as usize;
                ensure!(40 + used <= size, "PDB heap exceeds page");
                let mut cursor = size;
                let mut offsets = Vec::new();
                for group in 0..num.div_ceil(16) {
                    ensure!(cursor >= 4, "PDB row group outside page");
                    cursor -= 4;
                    let present = read16(p, cursor)?;
                    for n in 0..(num - group * 16).min(16) {
                        ensure!(cursor >= 2, "PDB row offset outside page");
                        cursor -= 2;
                        let offset = read16(p, cursor)? as usize;
                        ensure!(offset <= used, "PDB row outside heap");
                        offsets.push((offset, present & (1 << n) != 0));
                    }
                }
                let mut sorted: Vec<_> = offsets.iter().map(|(o, _)| *o).collect();
                sorted.sort_unstable();
                sorted.dedup();
                for (offset, present) in offsets {
                    if present {
                        let end = sorted
                            .iter()
                            .find(|o| **o > offset)
                            .copied()
                            .unwrap_or(used);
                        rows.push(p[40 + offset..40 + end].to_vec());
                    }
                }
            }
            page = next;
        }
        tables.insert(kind, rows);
    }
    Ok(tables)
}

struct Page {
    kind: u32,
    head: bool,
    rows: Vec<Vec<u8>>,
}
pub(super) fn build(tables: &Tables) -> Result<Vec<u8>> {
    let mut pages = vec![Page {
        kind: 0,
        head: false,
        rows: Vec::new(),
    }];
    let mut pointers = Vec::new();
    for kind in 0..20 {
        let first = pages.len();
        pages.push(Page {
            kind,
            head: true,
            rows: Vec::new(),
        });
        let mut current = Vec::new();
        let mut used = 0;
        for source in tables.get(&kind).into_iter().flatten() {
            let mut row = pad(source.clone());
            ensure!(row.len() + 6 <= PAGE - 40, "PDB row exceeds page capacity");
            let mut count = current.len() + 1;
            if used + row.len() + count * 2 + count.div_ceil(16) * 4 > PAGE - 40 {
                pages.push(Page {
                    kind,
                    head: false,
                    rows: current,
                });
                current = Vec::new();
                used = 0;
                count = 1;
            }
            if matches!(kind, 0 | 2 | 3 | 19) && row.len() >= 4 {
                u16le(&mut row, 2, ((count - 1) * 32) as u16);
            }
            used += row.len();
            current.push(row);
        }
        if !current.is_empty() {
            pages.push(Page {
                kind,
                head: false,
                rows: current,
            });
        }
        let last = pages.len() - 1;
        let empty = pages.len();
        pages.push(Page {
            kind,
            head: false,
            rows: Vec::new(),
        });
        pointers.push((kind, empty, first, last));
    }
    let mut bytes = vec![0; pages.len() * PAGE];
    u32le(&mut bytes, 4, PAGE as u32);
    u32le(&mut bytes, 8, 20);
    u32le(&mut bytes, 12, pages.len() as u32);
    u32le(&mut bytes, 16, 5);
    for (n, (kind, empty, first, last)) in pointers.iter().enumerate() {
        for (o, v) in [*kind, *empty as u32, *first as u32, *last as u32]
            .iter()
            .enumerate()
        {
            u32le(&mut bytes, 28 + n * 16 + o * 4, *v);
        }
    }
    let mut sequence = 0;
    for (index, page) in pages.iter().enumerate().skip(1) {
        if !page.head && page.rows.is_empty() {
            continue;
        }
        let out = &mut bytes[index * PAGE..(index + 1) * PAGE];
        u32le(out, 4, index as u32);
        u32le(out, 8, page.kind);
        u32le(out, 12, (index + 1) as u32);
        if page.head {
            u32le(out, 16, 1);
            out[27] = 100;
            u16le(out, 32, 8191);
            u16le(out, 34, 8191);
            u16le(out, 36, 1004);
            u32le(out, 40, index as u32);
            u32le(
                out,
                44,
                if pages[index + 1].rows.is_empty() {
                    67108863
                } else {
                    (index + 1) as u32
                },
            );
            u32le(out, 48, 67108863);
            for n in 0..1004 {
                u16le(out, 58 + n * 4, 8191);
                u16le(out, 60 + n * 4, 65528);
            }
            u16le(out, 4074, 8191);
        } else {
            sequence += 1;
            u32le(out, 16, sequence);
            let mut used = 0;
            let mut offsets = Vec::new();
            for row in &page.rows {
                offsets.push(used);
                out[40 + used..40 + used + row.len()].copy_from_slice(row);
                used += row.len();
            }
            let count = offsets.len();
            let packed = count as u32 | ((count as u32) << 13);
            out[24..27].copy_from_slice(&packed.to_le_bytes()[..3]);
            out[27] = 36;
            let batch = matches!(page.kind, 6 | 16 | 17 | 18 | 19);
            let mut cursor = PAGE;
            for (n, group) in offsets.chunks(16).enumerate() {
                let present = ((1_u32 << group.len()) - 1) as u16;
                let active = if batch {
                    present
                } else if n == count.div_ceil(16) - 1 {
                    1 << (group.len() - 1)
                } else {
                    0
                };
                cursor -= 2;
                u16le(out, cursor, active);
                cursor -= 2;
                u16le(out, cursor, present);
                for offset in group {
                    cursor -= 2;
                    u16le(out, cursor, *offset as u16);
                }
            }
            u16le(out, 28, (cursor - 40 - used) as u16);
            u16le(out, 30, used as u16);
            u16le(out, 32, if batch { count as u16 } else { 1 });
            u16le(out, 34, if batch { 0 } else { (count - 1) as u16 });
        }
    }
    u32le(&mut bytes, 20, (sequence + 1).max(6));
    Ok(bytes)
}

/// Inbound cursor: retain physical row identity without collecting a library.
pub(crate) struct ReadHeader {
    pub page_size: usize,
    pub table_types: Vec<u32>,
    pub raw: Vec<u8>,
}
pub(crate) struct ReadRow<'a> {
    pub kind: u32,
    pub page: u32,
    pub slot: usize,
    pub bytes: &'a [u8],
}
pub(crate) fn visit_records(
    path: &std::path::Path,
    max_bytes: u64,
    cancel: &cuerithm_media::CancellationToken,
    mut visit: impl FnMut(ReadRow<'_>) -> Result<()>,
) -> Result<ReadHeader> {
    use std::io::{Read, Seek, SeekFrom};
    cancel.check()?;
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    ensure!(len >= 32 && len <= max_bytes, "PDB byte limit");
    let mut prefix = [0; 32];
    file.read_exact(&mut prefix)?;
    let size = read32(&prefix, 4)? as usize;
    let count = read32(&prefix, 8)? as usize;
    ensure!(
        (128..=65536).contains(&size)
            && len.is_multiple_of(size as u64)
            && count <= 256
            && 28 + count * 16 <= size,
        "Invalid PDB directory"
    );
    let pages = usize::try_from(len / (size as u64))?;
    let mut header = vec![0; size];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut header)?;
    let mut table_types = Vec::new();
    let mut total = 0usize;
    let mut buffer = vec![0; size];
    for index in 0..count {
        cancel.check()?;
        let at = 28 + index * 16;
        let kind = read32(&header, at)?;
        ensure!(
            !table_types.contains(&kind),
            "Duplicate PDB table directory"
        );
        table_types.push(kind);
        let mut page = read32(&header, at + 8)? as usize;
        let last = read32(&header, at + 12)? as usize;
        ensure!(
            (page == 0 && last == 0) || page > 0 && page < pages && last > 0 && last < pages,
            "PDB directory page outside file"
        );
        let mut visited = vec![0u8; pages.div_ceil(8)];
        while page != 0 {
            cancel.check()?;
            ensure!(
                page < pages && visited[page / 8] & (1 << (page % 8)) == 0,
                "PDB page cycle/outside file"
            );
            visited[page / 8] |= 1 << (page % 8);
            file.seek(SeekFrom::Start((page as u64) * (size as u64)))?;
            file.read_exact(&mut buffer)?;
            ensure!(
                read32(&buffer, 4)? as usize == page && read32(&buffer, 8)? == kind,
                "Mismatched PDB page identity/type"
            );
            if buffer[27] & 0x40 == 0 {
                let word = u32::from(buffer[24])
                    | (u32::from(buffer[25]) << 8)
                    | (u32::from(buffer[26]) << 16);
                let slots = (word & 8191) as usize;
                let used = usize::from(read16(&buffer, 30)?);
                let offset_bytes = slots * 2 + slots.div_ceil(16) * 4;
                ensure!(
                    40 + used <= size
                        && offset_bytes <= size - 40
                        && 40 + used <= size - offset_bytes,
                    "PDB heap/row table overlap"
                );
                let mut cursor = size;
                let mut offsets = Vec::with_capacity(slots);
                for group in 0..slots.div_ceil(16) {
                    cursor -= 4;
                    let present = read16(&buffer, cursor)?;
                    for slot in 0..(slots - group * 16).min(16) {
                        cursor -= 2;
                        let offset = usize::from(read16(&buffer, cursor)?);
                        ensure!(offset <= used, "PDB row outside heap");
                        offsets.push((offset, present & (1 << slot) != 0));
                    }
                }
                let mut sorted = offsets
                    .iter()
                    .map(|(offset, _)| *offset)
                    .collect::<Vec<_>>();
                sorted.sort_unstable();
                sorted.dedup();
                let mut visible = HashSet::new();
                for (slot, (offset, present)) in offsets.into_iter().enumerate() {
                    if !present {
                        continue;
                    }
                    ensure!(visible.insert(offset), "Duplicate visible PDB row offset");
                    let end = sorted
                        .iter()
                        .find(|next| **next > offset)
                        .copied()
                        .unwrap_or(used);
                    ensure!(end > offset, "Empty visible PDB row");
                    total += 1;
                    ensure!(total <= 2_000_000, "PDB row count limit");
                    visit(ReadRow {
                        kind,
                        page: page as u32,
                        slot,
                        bytes: &buffer[40 + offset..40 + end],
                    })?;
                }
            }
            if page == last {
                break;
            }
            page = read32(&buffer, 12)? as usize;
            ensure!(page != 0, "PDB page chain ended before declared last page");
        }
    }
    cancel.check()?;
    Ok(ReadHeader {
        page_size: size,
        table_types,
        raw: header,
    })
}

pub(crate) fn inspect_header(path: &std::path::Path, max_bytes: u64) -> Result<ReadHeader> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    ensure!(len >= 32 && len <= max_bytes, "PDB byte limit");
    let mut first = [0; 32];
    file.read_exact(&mut first)?;
    let page_size = read32(&first, 4)? as usize;
    let count = read32(&first, 8)? as usize;
    ensure!(
        (128..=65536).contains(&page_size)
            && len.is_multiple_of(page_size as u64)
            && count <= 256
            && 28 + count * 16 <= page_size,
        "Invalid PDB directory"
    );
    let mut raw = vec![0; page_size];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut raw)?;
    let table_types = (0..count)
        .map(|i| read32(&raw, 28 + i * 16))
        .collect::<Result<Vec<_>>>()?;
    Ok(ReadHeader {
        page_size,
        table_types,
        raw,
    })
}
