//! Validate framing before handing native records to a deliberately tolerant tag reader.
use anyhow::{Context, Result, ensure};
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
        // Each version defines only its leading header flag bits; the rest are reserved
        // and would be dropped when the tolerant reader rewrites the tag.
        let defined = match version {
            2 => 0xc0,
            3 => 0xe0,
            _ => 0xf0,
        };
        ensure!(
            header[5] & !defined == 0,
            "Existing ID3 header uses reserved flags; refusing metadata reconstruction"
        );
        let len = syncsafe(&header[6..10])?;
        ensure!(
            len <= MAX_TAG,
            "Existing ID3 metadata exceeds safe framing limit"
        );
        let mut body = vec![0; len];
        file.read_exact(&mut body)?;
        let footer = version == 4 && header[5] & 16 != 0;
        // v2.2 and v2.3 unsynchronize the whole tag, so frame sizes count decoded
        // bytes. v2.4 sizes count the stored bytes and need no tag-level decoding.
        if matches!(version, 2 | 3) && header[5] & 128 != 0 {
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
            if version >= 3 {
                // The reader keeps only the flags it defines and rejects encrypted or
                // grouped frames, so a rewrite would silently change or drop them.
                let flags = u16::from_be_bytes([body[offset + 8], body[offset + 9]]);
                let (defined, unsupported) = if version == 3 {
                    (0xe0e0, 0x0060)
                } else {
                    (0x704f, 0x0044)
                };
                ensure!(
                    flags & !defined == 0,
                    "Existing ID3 frame uses undefined flags; refusing metadata reconstruction"
                );
                ensure!(
                    flags & unsupported == 0,
                    "Encrypted or grouped existing ID3 frames cannot be rewritten safely"
                );
            }
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
            // The footer repeats the header with "3DI" for the signature.
            let mut tail = [0; 10];
            file.read_exact(&mut tail)
                .context("Truncated existing ID3 footer")?;
            ensure!(
                &tail[..3] == b"3DI" && tail[3..] == header[3..],
                "Malformed existing ID3 footer"
            );
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
    #[test]
    fn id3v22_and_v23_tag_level_unsynchronization_is_reversed_before_frame_sizes() {
        // A frame holding [00 FF FB]; the FF FB pair is stored as FF 00 FB, so the
        // stored body is one byte longer than the size in the frame header.
        let v22 = [
            &b"ID3\x02\x00\x80\x00\x00\x00\x0a"[..],
            b"TT2\x00\x00\x03\x00\xff\x00\xfb",
        ]
        .concat();
        assert!(validate_reader(&mut std::io::Cursor::new(v22)).is_ok());
        let v23 = [
            &b"ID3\x03\x00\x80\x00\x00\x00\x0e"[..],
            b"TIT2\x00\x00\x00\x03\x00\x00\x00\xff\x00\xfb",
        ]
        .concat();
        assert!(validate_reader(&mut std::io::Cursor::new(v23)).is_ok());
    }
    fn flagged_tag(version: u8, flags: [u8; 2]) -> Vec<u8> {
        // One TIT2 frame with a single payload byte and the given frame flags.
        let mut frame = b"TIT2\x00\x00\x00\x01".to_vec();
        frame.extend_from_slice(&flags);
        frame.push(0);
        let mut tag = vec![b'I', b'D', b'3', version, 0, 0, 0, 0, 0, frame.len() as u8];
        tag.extend(frame);
        tag
    }
    #[test]
    fn frame_flags_must_be_defined_and_supported_by_the_rewriting_reader() {
        let accepts = |version, flags| {
            validate_reader(&mut std::io::Cursor::new(flagged_tag(version, flags))).is_ok()
        };
        // v2.3: preservation and read-only status bits, compression format bit.
        assert!(accepts(3, [0x00, 0x00]));
        assert!(accepts(3, [0xe0, 0x00]));
        assert!(accepts(3, [0x00, 0x80]));
        // v2.3: undefined bits, encryption and grouping identity.
        for flags in [[0x00, 0x01], [0x01, 0x00], [0x00, 0x40], [0x00, 0x20]] {
            assert!(!accepts(3, flags), "v2.3 {flags:?}");
        }
        // v2.4: status bits, unsynchronisation and compression format bits.
        assert!(accepts(4, [0x00, 0x00]));
        assert!(accepts(4, [0x70, 0x00]));
        assert!(accepts(4, [0x00, 0x0a]));
        // v2.4: undefined bits, encryption and grouping identity.
        for flags in [
            [0x80, 0x00],
            [0x00, 0x80],
            [0x00, 0x20],
            [0x00, 0x04],
            [0x00, 0x40],
        ] {
            assert!(!accepts(4, flags), "v2.4 {flags:?}");
        }
    }
    #[test]
    fn reserved_tag_header_flags_are_refused_for_every_version() {
        let empty =
            |version: u8, flags: u8| [b'I', b'D', b'3', version, 0, flags, 0, 0, 0, 0].to_vec();
        let accepts = |version, flags| {
            validate_reader(&mut std::io::Cursor::new(empty(version, flags))).is_ok()
        };
        // Defined bits: v2.2 unsynchronisation, v2.3 adds extended header and experimental,
        // v2.4 adds the footer (an empty body with a footer is covered separately).
        assert!(accepts(2, 0x80));
        assert!(accepts(3, 0x80));
        assert!(accepts(4, 0x80));
        assert!(accepts(4, 0x20));
        // Reserved bits.
        for (version, flags) in [
            (2, 0x01),
            (2, 0x20),
            (3, 0x01),
            (3, 0x10),
            (4, 0x01),
            (4, 0x08),
        ] {
            assert!(!accepts(version, flags), "v2.{version} flags {flags:#04x}");
        }
    }
    fn footer_tag(footer: &[u8]) -> Vec<u8> {
        // An empty ID3v2.4 tag whose header sets the footer flag.
        let mut bytes = b"ID3\x04\x00\x10\x00\x00\x00\x00".to_vec();
        bytes.extend_from_slice(footer);
        bytes
    }
    #[test]
    fn id3v24_footer_must_be_present_and_mirror_the_header() {
        let valid = footer_tag(b"3DI\x04\x00\x10\x00\x00\x00\x00");
        assert!(validate_reader(&mut std::io::Cursor::new(valid)).is_ok());
        for (name, footer) in [
            ("missing", &b""[..]),
            ("truncated", b"3DI\x04\x00"),
            ("wrong signature", b"XYZ\x04\x00\x10\x00\x00\x00\x00"),
            ("different version", b"3DI\x03\x00\x10\x00\x00\x00\x00"),
            ("different size", b"3DI\x04\x00\x10\x00\x00\x00\x05"),
        ] {
            assert!(
                validate_reader(&mut std::io::Cursor::new(footer_tag(footer))).is_err(),
                "{name} footer"
            );
        }
    }
}
