//! Bounded metadata-container inventories. These routines never decode audio.
use super::*;
use std::io::Write;

fn u16le(b: &[u8]) -> Result<u16> {
    Ok(u16::from_le_bytes(b.try_into()?))
}
fn u32le(b: &[u8]) -> Result<u32> {
    Ok(u32::from_le_bytes(b.try_into()?))
}
fn u64le(b: &[u8]) -> Result<u64> {
    Ok(u64::from_le_bytes(b.try_into()?))
}
fn take<'a>(bytes: &'a [u8], position: &mut usize, n: usize) -> Result<&'a [u8]> {
    let end = position
        .checked_add(n)
        .context("Metadata offset overflow")?;
    let out = bytes
        .get(*position..end)
        .context("Truncated metadata record")?;
    *position = end;
    Ok(out)
}
fn utf16(bytes: &[u8]) -> Result<String> {
    ensure!(
        bytes.len().is_multiple_of(2),
        "Invalid UTF-16 attribute length"
    );
    Ok(String::from_utf16(
        &bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect::<Vec<_>>(),
    )?
    .trim_end_matches('\0')
    .to_owned())
}
fn normalized_field(key: &str) -> Option<&'static str> {
    match key.to_ascii_uppercase().as_str() {
        "TITLE" | "INAM" | "NAME" => Some("title"),
        "ARTIST" | "AUTHOR" | "WM/ARTIST" | "IART" | "AUTH" => Some("artists"),
        "ALBUM" | "WM/ALBUMTITLE" | "IPRD" => Some("album"),
        "GENRE" | "WM/GENRE" | "IGNR" => Some("genre"),
        "COMMENT" | "DESCRIPTION" | "ICMT" | "ANNO" => Some("comment"),
        "INITIALKEY" | "KEY" | "WM/INITIALKEY" => Some("key"),
        "BPM" | "WM/BEATSPERMINUTE" => Some("bpm"),
        _ => None,
    }
}
fn normalize(out: &mut FileTagObservation, key: &str, value: String) {
    if let Some(field) = normalized_field(key) {
        out.normalized.entry(field.into()).or_default().push(value);
    }
}
fn read_bytes(file: &mut File, n: u64, limits: &ImportLimits) -> Result<Vec<u8>> {
    ensure!(
        n <= limits.text_bytes && n <= limits.blob_bytes,
        "Metadata block exceeds byte limit"
    );
    let n = usize::try_from(n).context("Metadata size overflow")?;
    let mut data = vec![0; n];
    file.read_exact(&mut data)?;
    Ok(data)
}
fn comments(
    data: &[u8],
    namespace: &str,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut p = 0;
    let n = u32le(take(data, &mut p, 4)?)? as usize;
    let vendor = take(data, &mut p, n)?;
    out.record(namespace, "vendor", 0, vendor, limits)?;
    let count = u32le(take(data, &mut p, 4)?)? as usize;
    ensure!(count <= data.len() / 4, "Impossible comment count");
    for _ in 0..count {
        cancel.check()?;
        let n = u32le(take(data, &mut p, 4)?)? as usize;
        let raw = take(data, &mut p, n)?;
        let value = std::str::from_utf8(raw).context("Invalid UTF-8 Vorbis comment")?;
        let Some((key, value)) = value.split_once('=') else {
            out.record(namespace, "invalid-comment", 0, raw, limits)?;
            out.issues.push(ImportIssue {
                code: "malformed_vorbis_comment".into(),
                ..Default::default()
            });
            continue;
        };
        ensure!(!key.is_empty(), "Empty Vorbis comment key");
        out.record(namespace, key, 0, raw, limits)?;
        normalize(out, key, value.to_owned());
        if key.eq_ignore_ascii_case("METADATA_BLOCK_PICTURE") {
            use base64::Engine;
            let result = base64::engine::general_purpose::STANDARD
                .decode(value.trim())
                .map_err(anyhow::Error::from)
                .and_then(|picture| flac_picture(&picture, out, limits));
            if let Err(error) = result {
                picture_issue(out, "vorbis.METADATA_BLOCK_PICTURE", error);
            }
        }
    }
    Ok(())
}
fn picture_issue(out: &mut FileTagObservation, field: &str, error: anyhow::Error) {
    out.issues.push(ImportIssue {
        code: "artwork_interpretation_unavailable".into(),
        field: Some(field.into()),
        message: error.to_string(),
        ..Default::default()
    });
}
fn flac_picture(data: &[u8], out: &mut FileTagObservation, limits: &ImportLimits) -> Result<()> {
    let mut p = 0;
    let kind = u32::from_be_bytes(take(data, &mut p, 4)?.try_into()?);
    let n = u32::from_be_bytes(take(data, &mut p, 4)?.try_into()?) as usize;
    let mime = std::str::from_utf8(take(data, &mut p, n)?)?.to_owned();
    let n = u32::from_be_bytes(take(data, &mut p, 4)?.try_into()?) as usize;
    let description = std::str::from_utf8(take(data, &mut p, n)?)?.to_owned();
    take(data, &mut p, 16)?;
    let n = u32::from_be_bytes(take(data, &mut p, 4)?.try_into()?) as usize;
    let image = take(data, &mut p, n)?;
    ensure!(p == data.len(), "Unaccounted FLAC picture bytes");
    out.add_artwork(Some(mime), Some(kind), Some(description), image, limits)
}
fn asf_picture(data: &[u8], out: &mut FileTagObservation, limits: &ImportLimits) -> Result<()> {
    let mut p = 0;
    let kind = take(data, &mut p, 1)?[0] as u32;
    let n = u32le(take(data, &mut p, 4)?)? as usize;
    let mut strings = Vec::new();
    for _ in 0..2 {
        let begin = p;
        loop {
            if take(data, &mut p, 2)? == [0, 0] {
                break;
            }
        }
        strings.push(utf16(&data[begin..p - 2])?);
    }
    let image = take(data, &mut p, n)?;
    ensure!(p == data.len(), "Unaccounted ASF picture bytes");
    out.add_artwork(
        Some(strings[0].clone()),
        Some(kind),
        Some(strings[1].clone()),
        image,
        limits,
    )
}
fn flac(
    file: &mut File,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    file.seek(SeekFrom::Start(4))?;
    let mut count = 0;
    loop {
        cancel.check()?;
        count += 1;
        ensure!(count <= 65536, "Too many FLAC metadata blocks");
        let mut h = [0; 4];
        file.read_exact(&mut h)?;
        let n = ((h[1] as u64) << 16) | ((h[2] as u64) << 8) | h[3] as u64;
        let bytes = read_bytes(file, n, limits)?;
        let id = match h[0] & 0x7f {
            0 => "STREAMINFO",
            1 => "PADDING",
            2 => "APPLICATION",
            3 => "SEEKTABLE",
            4 => "VORBIS_COMMENT",
            5 => "CUESHEET",
            6 => "PICTURE",
            _ => "UNKNOWN_BLOCK",
        };
        out.record("flac", id, h[0] as u16, &bytes, limits)?;
        if h[0] & 0x7f == 4 {
            comments(&bytes, "vorbis", out, limits, cancel)?;
        }
        if h[0] & 0x7f == 6
            && let Err(error) = flac_picture(&bytes, out, limits)
        {
            picture_issue(out, "flac.PICTURE", error);
        }
        if h[0] & 0x80 != 0 {
            break;
        }
    }
    Ok(())
}
fn ogg(
    file: &mut File,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut packet = Vec::new();
    let mut pages = 0;
    let mut cumulative = 0u64;
    loop {
        cancel.check()?;
        pages += 1;
        ensure!(pages <= 65536, "Ogg metadata page limit exceeded");
        let mut h = [0; 27];
        file.read_exact(&mut h)?;
        ensure!(
            &h[..4] == b"OggS" && h[4] == 0,
            "Unsupported Ogg page header"
        );
        let count = h[26] as usize;
        let mut lengths = vec![0; count];
        file.read_exact(&mut lengths)?;
        let n = lengths.iter().map(|n| *n as u64).sum::<u64>();
        cumulative = cumulative
            .checked_add(n + 27 + count as u64)
            .context("Ogg metadata size overflow")?;
        ensure!(
            cumulative <= limits.text_bytes,
            "Ogg metadata exceeds byte limit"
        );
        let payload = read_bytes(file, n, limits)?;
        let mut page = h.to_vec();
        page.extend(&lengths);
        page.extend(&payload);
        out.record("ogg", "metadata-page", h[5] as u16, &page, limits)?;
        let mut p = 0;
        for n in lengths {
            let part = take(&payload, &mut p, n as usize)?;
            ensure!(
                packet
                    .len()
                    .checked_add(part.len())
                    .is_some_and(|n| n as u64 <= limits.text_bytes),
                "Ogg packet size limit exceeded"
            );
            packet.extend(part);
            if n < 255 {
                if packet.starts_with(b"OpusTags") {
                    out.record("opus", "OpusTags", 0, &packet, limits)?;
                    comments(&packet[8..], "opus", out, limits, cancel)?;
                    return Ok(());
                }
                if packet.starts_with(b"\x03vorbis") {
                    out.record("vorbis", "comment-packet", 0, &packet, limits)?;
                    comments(&packet[7..], "vorbis", out, limits, cancel)?;
                    return Ok(());
                }
                ensure!(
                    packet.starts_with(b"OpusHead") || packet.starts_with(b"\x01vorbis"),
                    "Ogg stream has no supported comment packet"
                );
                packet.clear();
            }
        }
    }
}
fn fourcc(id: &[u8]) -> String {
    id.iter().map(|b| char::from(*b)).collect()
}
fn mp4_items(
    bytes: &[u8],
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut p = 0;
    while p < bytes.len() {
        cancel.check()?;
        let start = p;
        let size = u32::from_be_bytes(take(bytes, &mut p, 4)?.try_into()?) as usize;
        let id = take(bytes, &mut p, 4)?;
        ensure!(size >= 8, "Invalid MP4 metadata item size");
        let value = take(bytes, &mut p, size - 8)?;
        let name = fourcc(id);
        out.record("mp4.ilst", &name, 0, value, limits)?;
        if name == "covr" {
            let result = (|| -> Result<()> {
                let mut q = 0;
                while q < value.len() {
                    let length = u32::from_be_bytes(take(value, &mut q, 4)?.try_into()?) as usize;
                    let id = take(value, &mut q, 4)?;
                    ensure!(length >= 8, "Invalid MP4 cover atom");
                    let payload = take(value, &mut q, length - 8)?;
                    if id == b"data" {
                        ensure!(payload.len() >= 8, "Truncated MP4 cover data");
                        let kind = u32::from_be_bytes(payload[..4].try_into()?);
                        let mime = match kind {
                            13 => Some("image/jpeg"),
                            14 => Some("image/png"),
                            _ => None,
                        };
                        out.add_artwork(
                            mime.map(str::to_owned),
                            None,
                            None,
                            &payload[8..],
                            limits,
                        )?;
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                picture_issue(out, "mp4.covr", error);
            }
        }
        if name == "©nam" || name == "©ART" || name == "©alb" {
            let mut q = 0;
            while q < value.len() {
                let length = u32::from_be_bytes(take(value, &mut q, 4)?.try_into()?) as usize;
                let atom_id = take(value, &mut q, 4)?;
                ensure!(length >= 8, "Invalid MP4 data atom");
                let payload = take(value, &mut q, length - 8)?;
                if atom_id == b"data"
                    && payload.len() >= 8
                    && u32::from_be_bytes(payload[..4].try_into()?) == 1
                {
                    normalize(
                        out,
                        match name.as_str() {
                            "©nam" => "title",
                            "©ART" => "artist",
                            _ => "album",
                        },
                        String::from_utf8(payload[8..].to_vec())?,
                    );
                }
            }
        }
        ensure!(p > start, "MP4 item did not advance");
    }
    Ok(())
}
fn mp4_children(
    bytes: &[u8],
    depth: usize,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    ensure!(depth <= 16, "MP4 metadata hierarchy depth exceeded");
    let mut p = 0;
    while p < bytes.len() {
        cancel.check()?;
        let size = u32::from_be_bytes(take(bytes, &mut p, 4)?.try_into()?) as usize;
        let id = take(bytes, &mut p, 4)?;
        ensure!(size >= 8, "Invalid MP4 child atom size");
        let data = take(bytes, &mut p, size - 8)?;
        match id {
            b"udta" | b"moov" => mp4_children(data, depth + 1, out, limits, cancel)?,
            b"meta" => {
                ensure!(data.len() >= 4, "Truncated MP4 metadata header");
                mp4_children(&data[4..], depth + 1, out, limits, cancel)?;
            }
            b"ilst" => mp4_items(data, out, limits, cancel)?,
            _ => {}
        }
    }
    Ok(())
}
fn mp4(
    file: &mut File,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let total = file.metadata()?.len();
    let mut p = 0u64;
    let mut count = 0;
    while p < total {
        cancel.check()?;
        count += 1;
        ensure!(count <= 65536, "MP4 atom count limit exceeded");
        file.seek(SeekFrom::Start(p))?;
        let mut h = [0; 8];
        file.read_exact(&mut h)?;
        let mut size = u32::from_be_bytes(h[..4].try_into()?) as u64;
        let mut header = 8;
        if size == 1 {
            let mut large = [0; 8];
            file.read_exact(&mut large)?;
            size = u64::from_be_bytes(large);
            header = 16;
        } else if size == 0 {
            size = total - p;
        }
        ensure!(
            size >= header && p.checked_add(size).is_some_and(|end| end <= total),
            "Invalid MP4 atom bounds"
        );
        if &h[4..] == b"moov" || &h[4..] == b"meta" || &h[4..] == b"udta" {
            let data = read_bytes(file, size - header, limits)?;
            out.record("mp4", &fourcc(&h[4..]), 0, &data, limits)?;
            if &h[4..] == b"meta" {
                ensure!(data.len() >= 4, "Truncated MP4 metadata header");
                mp4_children(&data[4..], 0, out, limits, cancel)?;
            } else {
                mp4_children(&data, 0, out, limits, cancel)?;
            }
        }
        p += size;
    }
    Ok(())
}
fn chunks(
    file: &mut File,
    be: bool,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut header = [0; 12];
    file.read_exact(&mut header)?;
    let declared = if be {
        u32::from_be_bytes(header[4..8].try_into()?)
    } else {
        u32::from_le_bytes(header[4..8].try_into()?)
    } as u64;
    let total = file.metadata()?.len();
    let end = declared.checked_add(8).context("Chunk length overflow")?;
    ensure!(end <= total && end >= 12, "Invalid audio container size");
    let mut p = 12u64;
    let mut count = 0;
    while p < end {
        cancel.check()?;
        count += 1;
        ensure!(count <= 65536, "Metadata chunk count limit exceeded");
        file.seek(SeekFrom::Start(p))?;
        let mut h = [0; 8];
        file.read_exact(&mut h)?;
        let n = if be {
            u32::from_be_bytes(h[4..].try_into()?)
        } else {
            u32::from_le_bytes(h[4..].try_into()?)
        } as u64;
        let next = p
            .checked_add(8)
            .and_then(|p| p.checked_add(n))
            .context("Chunk length overflow")?;
        ensure!(next <= end, "Truncated audio container chunk");
        if !matches!(&h[..4], b"data" | b"SSND" | b"COMM" | b"fmt ") {
            let data = read_bytes(file, n, limits)?;
            let id = fourcc(&h[..4]);
            out.record(if be { "aiff" } else { "riff" }, &id, 0, &data, limits)?;
            if matches!(&h[..4], b"ID3 " | b"id3 ") {
                let mut temp = tempfile::tempfile()?;
                temp.write_all(&data)?;
                temp.seek(SeekFrom::Start(0))?;
                super::read_id3(&mut temp, out, limits, cancel)?;
            } else if &h[..4] == b"LIST" && data.starts_with(b"INFO") {
                let mut q = 4;
                while q < data.len() {
                    let id = fourcc(take(&data, &mut q, 4)?);
                    let len = u32le(take(&data, &mut q, 4)?)? as usize;
                    let value = take(&data, &mut q, len)?;
                    out.record("riff.info", &id, 0, value, limits)?;
                    normalize(
                        out,
                        &id,
                        String::from_utf8(value.to_vec())?
                            .trim_end_matches('\0')
                            .to_owned(),
                    );
                    if !len.is_multiple_of(2) {
                        take(&data, &mut q, 1)?;
                    }
                }
            } else if be && matches!(&h[..4], b"NAME" | b"AUTH" | b"ANNO") {
                normalize(
                    out,
                    &id,
                    String::from_utf8(data)?.trim_end_matches('\0').to_owned(),
                );
            }
        }
        p = next + (n % 2);
    }
    Ok(())
}
const ASF_HEADER: [u8; 16] = [
    0x30, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11, 0xa6, 0xd9, 0, 0xaa, 0, 0x62, 0xce, 0x6c,
];
const ASF_EXTENDED: [u8; 16] = [
    0x40, 0xa4, 0xd0, 0xd2, 0x07, 0xe3, 0xd2, 0x11, 0x97, 0xf0, 0, 0xa0, 0xc9, 0x5e, 0xa8, 0x50,
];
const ASF_CONTENT: [u8; 16] = [
    0x33, 0x26, 0xb2, 0x75, 0x8e, 0x66, 0xcf, 0x11, 0xa6, 0xd9, 0, 0xaa, 0, 0x62, 0xce, 0x6c,
];
fn asf(
    file: &mut File,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut h = [0; 30];
    file.read_exact(&mut h)?;
    let size = u64le(&h[16..24])?;
    ensure!(
        size >= 30 && size <= file.metadata()?.len() && size <= limits.text_bytes,
        "Invalid ASF header size"
    );
    let count = u32le(&h[24..28])?;
    ensure!(
        count as u64 <= (size - 30) / 24,
        "Impossible ASF header object count"
    );
    let mut consumed = 30;
    for _ in 0..count {
        cancel.check()?;
        let mut h = [0; 24];
        file.read_exact(&mut h)?;
        let n = u64le(&h[16..])?;
        ensure!(n >= 24 && n <= size - consumed, "Invalid ASF object size");
        let data = read_bytes(file, n - 24, limits)?;
        consumed += n;
        let id = h[..16]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        out.record("asf.object", &id, 0, &data, limits)?;
        if h[..16] == ASF_EXTENDED {
            let mut p = 0;
            let count = u16le(take(&data, &mut p, 2)?)?;
            for _ in 0..count {
                cancel.check()?;
                let n = u16le(take(&data, &mut p, 2)?)? as usize;
                let name = utf16(take(&data, &mut p, n)?)?;
                let kind = u16le(take(&data, &mut p, 2)?)?;
                let n = u16le(take(&data, &mut p, 2)?)? as usize;
                let value = take(&data, &mut p, n)?;
                out.record("asf.attribute", &name, kind, value, limits)?;
                if kind == 0 {
                    normalize(out, &name, utf16(value)?);
                }
                if kind == 1
                    && name.eq_ignore_ascii_case("WM/Picture")
                    && let Err(error) = asf_picture(value, out, limits)
                {
                    picture_issue(out, "asf.WM/Picture", error);
                }
            }
        } else if h[..16] == ASF_CONTENT {
            ensure!(data.len() >= 10, "Truncated ASF content description");
            let mut p = 10;
            for (i, name) in ["Title", "Author", "Copyright", "Description", "Rating"]
                .iter()
                .enumerate()
            {
                let n = u16le(&data[i * 2..i * 2 + 2])? as usize;
                let value = take(&data, &mut p, n)?;
                out.record("asf.attribute", name, 0, value, limits)?;
                normalize(out, name, utf16(value)?);
            }
        } else {
            out.issues.push(ImportIssue {
                code: "asf_object_retained_opaque".into(),
                field: Some(id),
                message:
                    "Object retained byte-for-byte; attributes in this object are not interpreted"
                        .into(),
                ..Default::default()
            });
        }
    }
    Ok(())
}
pub(super) fn read(
    file: &mut File,
    magic: &[u8],
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<bool> {
    if magic.starts_with(b"fLaC") {
        flac(file, out, limits, cancel)?;
    } else if magic.starts_with(b"OggS") {
        ogg(file, out, limits, cancel)?;
    } else if magic.len() >= 8 && &magic[4..8] == b"ftyp" {
        mp4(file, out, limits, cancel)?;
    } else if magic.starts_with(b"RIFF") && magic.get(8..12) == Some(b"WAVE") {
        chunks(file, false, out, limits, cancel)?;
    } else if magic.starts_with(b"FORM") && matches!(magic.get(8..12), Some(b"AIFF" | b"AIFC")) {
        chunks(file, true, out, limits, cancel)?;
    } else if magic.len() >= 12 && magic[..12] == ASF_HEADER[..12] {
        asf(file, out, limits, cancel)?;
    } else {
        return Ok(false);
    }
    Ok(true)
}
