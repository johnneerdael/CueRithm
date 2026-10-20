//! Validate framing before handing native records to a deliberately tolerant tag reader.
use anyhow::{Result, ensure};
use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
};
const MAX_TAG: usize = 32 * 1024 * 1024;
fn syncsafe(bytes: &[u8]) -> Result<usize> {
    ensure!(
        bytes.iter().all(|b| b & 128 == 0),
        "Malformed ID3 syncsafe size"
    );
    Ok(bytes.iter().fold(0, |n, b| (n << 7) | usize::from(*b)))
}
pub(crate) fn validate_leading(path: &Path) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    validate_reader(&mut file)
}
fn validate_reader(file: &mut (impl Read + Seek)) -> Result<()> {
    let mut header = [0; 10];
    loop {
        let count = file.read(&mut header)?;
        if count != header.len() {
            ensure!(
                count < 3 || &header[..3] != b"ID3",
                "Truncated existing ID3 header"
            );
            return Ok(());
        }
        if &header[..3] != b"ID3" {
            return Ok(());
        }
        let version = header[3];
        ensure!(
            (2..=4).contains(&version),
            "Unsupported existing ID3 version"
        );
        let len = syncsafe(&header[6..10])?;
        ensure!(
            len <= MAX_TAG,
            "Existing ID3 metadata exceeds safe framing limit"
        );
        let mut body = vec![0; len];
        file.read_exact(&mut body)?;
        let footer = version == 4 && header[5] & 16 != 0;
        if version == 3 && header[5] & 128 != 0 {
            let mut decoded = Vec::with_capacity(body.len());
            let mut i = 0;
            while i < body.len() {
                decoded.push(body[i]);
                if body[i] == 255 && body.get(i + 1) == Some(&0) {
                    i += 1;
                }
                i += 1;
            }
            body = decoded;
        }
        ensure!(
            version != 2 || header[5] & 64 == 0,
            "Compressed existing ID3v2.2 requires manual repair"
        );
        let mut offset = 0;
        if version >= 3 && header[5] & 64 != 0 {
            ensure!(body.len() >= 4, "Malformed ID3 extended header");
            offset = if version == 4 {
                syncsafe(&body[..4])?
            } else {
                4 + u32::from_be_bytes(body[..4].try_into()?) as usize
            };
            ensure!(
                (4..=body.len()).contains(&offset),
                "Malformed ID3 extended header size"
            );
        }
        let id_len = if version == 2 { 3 } else { 4 };
        let header_len = if version == 2 { 6 } else { 10 };
        while offset < body.len() {
            if body[offset] == 0 {
                ensure!(
                    body[offset..].iter().all(|b| *b == 0),
                    "Malformed ID3 padding"
                );
                break;
            }
            ensure!(
                offset + header_len <= body.len(),
                "Truncated existing ID3 frame header"
            );
            ensure!(
                body[offset..offset + id_len]
                    .iter()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()),
                "Malformed existing ID3 frame identifier; refusing metadata reconstruction"
            );
            let size = if version == 4 {
                syncsafe(&body[offset + 4..offset + 8])?
            } else if version == 3 {
                u32::from_be_bytes(body[offset + 4..offset + 8].try_into()?) as usize
            } else {
                u32::from_be_bytes([0, body[offset + 3], body[offset + 4], body[offset + 5]])
                    as usize
            };
            ensure!(
                size > 0 && size <= body.len() - offset - header_len,
                "Malformed existing ID3 frame size"
            );
            offset += header_len + size;
        }
        if footer {
            file.seek(SeekFrom::Current(10))?;
        }
    }
}

pub(crate) fn validate_container(
    path: &Path,
    format: &onetagger_tag::AudioFileFormat,
) -> Result<()> {
    use onetagger_tag::AudioFileFormat;
    if matches!(format, AudioFileFormat::MP3 | AudioFileFormat::AAC) {
        return validate_leading(path);
    }
    if !matches!(format, AudioFileFormat::WAV | AudioFileFormat::AIFF) {
        return Ok(());
    }
    let mut file = std::fs::File::open(path)?;
    let total = file.metadata()?.len();
    let mut header = [0; 12];
    file.read_exact(&mut header)?;
    let little = *format == AudioFileFormat::WAV;
    let mut offset = 12_u64;
    while offset + 8 <= total {
        file.seek(SeekFrom::Start(offset))?;
        let mut chunk = [0; 8];
        file.read_exact(&mut chunk)?;
        let size = if little {
            u32::from_le_bytes(chunk[4..8].try_into()?)
        } else {
            u32::from_be_bytes(chunk[4..8].try_into()?)
        } as u64;
        ensure!(
            size <= total - offset - 8,
            "Malformed native metadata container chunk"
        );
        if chunk[..3].eq_ignore_ascii_case(b"ID3") && matches!(chunk[3], b' ' | 0) {
            ensure!(
                size <= MAX_TAG as u64 + 20,
                "Existing ID3 metadata exceeds safe framing limit"
            );
            let mut bytes = vec![0; size as usize];
            file.read_exact(&mut bytes)?;
            ensure!(bytes.starts_with(b"ID3"), "Malformed native ID3 chunk");
            validate_reader(&mut std::io::Cursor::new(bytes))?;
        }
        offset += 8 + size + (size % 2);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truncated_claimed_id3_header_is_not_untagged_audio() {
        assert!(validate_reader(&mut std::io::Cursor::new(b"ID3".to_vec())).is_err());
    }
}
