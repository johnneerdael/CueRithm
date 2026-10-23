use super::*;
fn synchsafe(bytes: &[u8]) -> Result<usize> {
    ensure!(
        bytes.len() == 4 && bytes.iter().all(|b| b & 0x80 == 0),
        "Invalid ID3 synchsafe length"
    );
    Ok(bytes.iter().fold(0usize, |n, b| (n << 7) | *b as usize))
}
fn text(bytes: &[u8]) -> Option<String> {
    let (&encoding, bytes) = bytes.split_first()?;
    match encoding {
        0 => Some(bytes.iter().map(|&b| char::from(b)).collect()),
        3 => String::from_utf8(bytes.to_vec()).ok(),
        1 | 2 => {
            let (le, bytes) = if bytes.starts_with(&[0xff, 0xfe]) {
                (true, &bytes[2..])
            } else if bytes.starts_with(&[0xfe, 0xff]) {
                (false, &bytes[2..])
            } else {
                (encoding == 1, bytes)
            };
            if bytes.len() % 2 != 0 {
                return None;
            }
            String::from_utf16(
                &bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| {
                        if le {
                            u16::from_le_bytes([p[0], p[1]])
                        } else {
                            u16::from_be_bytes([p[0], p[1]])
                        }
                    })
                    .collect::<Vec<_>>(),
            )
            .ok()
        }
        _ => None,
    }
}
pub(super) fn read_id3(
    file: &mut File,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut header = [0u8; 10];
    file.read_exact(&mut header)?;
    ensure!(&header[..3] == b"ID3", "Not an ID3 envelope");
    let version = header[3];
    ensure!((2..=4).contains(&version), "Unsupported ID3 version");
    let size = synchsafe(&header[6..])?;
    ensure!(
        size as u64 <= limits.text_bytes,
        "ID3 tag exceeds byte limit"
    );
    let mut body = vec![0; size];
    file.read_exact(&mut body)?;
    cancel.check()?;
    let mut original = header.to_vec();
    original.extend_from_slice(&body);
    if version == 4 && header[5] & 0x10 != 0 {
        let mut footer = [0; 10];
        file.read_exact(&mut footer)?;
        ensure!(
            &footer[..3] == b"3DI" && footer[3..] == header[3..],
            "Invalid ID3 footer"
        );
        original.extend_from_slice(&footer);
    }
    out.record("id3", "envelope", header[5] as u16, &original, limits)?;
    if header[5] & 0x80 != 0 || (version == 2 && header[5] & 0x40 != 0) {
        out.issues.push(ImportIssue {
            code: "id3_unsynchronized_or_compressed".into(),
            field: Some("ID3 frame interpretation".into()),
            message: "Original envelope retained; frame decoding unavailable for these flags"
                .into(),
            ..Default::default()
        });
        return Ok(());
    }
    let mut offset = 0;
    if version >= 3 && header[5] & 0x40 != 0 {
        ensure!(body.len() >= 4, "Truncated ID3 extended header");
        let n = if version == 4 {
            synchsafe(&body[..4])?
        } else {
            u32::from_be_bytes(body[..4].try_into()?) as usize + 4
        };
        ensure!(
            n >= 4 && n <= body.len(),
            "Invalid ID3 extended header size"
        );
        offset = n;
    }
    while offset < body.len() && body[offset] != 0 {
        cancel.check()?;
        let header_size = if version == 2 { 6 } else { 10 };
        let id_size = if version == 2 { 3 } else { 4 };
        ensure!(
            body.len() - offset >= header_size,
            "Truncated ID3 frame header"
        );
        let id = &body[offset..offset + id_size];
        ensure!(
            id.iter()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()),
            "Invalid ID3 frame identity"
        );
        let size = if version == 2 {
            body[offset + 3..offset + 6]
                .iter()
                .fold(0usize, |n, b| (n << 8) | *b as usize)
        } else if version == 4 {
            synchsafe(&body[offset + 4..offset + 8])?
        } else {
            u32::from_be_bytes(body[offset + 4..offset + 8].try_into()?) as usize
        };
        let flags = if version == 2 {
            0
        } else {
            u16::from_be_bytes(body[offset + 8..offset + 10].try_into()?)
        };
        let start = offset + header_size;
        let end = start.checked_add(size).context("ID3 frame size overflow")?;
        ensure!(end <= body.len(), "Truncated ID3 frame payload");
        let id = std::str::from_utf8(id)?;
        let payload = &body[start..end];
        out.record(&format!("id3v2.{version}"), id, flags, payload, limits)?;
        if flags == 0
            && let Err(error) = identity_and_artwork(out, id, payload, limits)
        {
            out.issues.push(ImportIssue {
                code: "id3_structured_field_unresolved".into(),
                field: Some(id.into()),
                message: error.to_string(),
                ..Default::default()
            });
        }
        if flags == 0
            && id.starts_with('T')
            && let Some(value) = text(payload)
        {
            let field = match id {
                "TIT2" | "TT2" => Some("title"),
                "TPE1" | "TP1" => Some("artists"),
                "TALB" | "TAL" => Some("album"),
                "TCON" | "TCO" => Some("genre"),
                "TKEY" | "TKE" => Some("key"),
                "TBPM" | "TBP" => Some("bpm"),
                _ => None,
            };
            if let Some(field) = field {
                out.normalized
                    .entry(field.into())
                    .or_default()
                    .extend(value.split('\0').map(str::to_owned));
            }
        }
        offset = end;
    }
    Ok(())
}

fn terminated(bytes: &[u8], encoding: u8) -> Result<(String, usize)> {
    let (length, terminator) = if matches!(encoding, 1 | 2) {
        (
            bytes
                .as_chunks::<2>()
                .0
                .iter()
                .position(|p| *p == [0, 0])
                .context("Unterminated UTF-16 ID3 identity")?
                * 2,
            2,
        )
    } else {
        (
            bytes
                .iter()
                .position(|b| *b == 0)
                .context("Unterminated ID3 identity")?,
            1,
        )
    };
    let mut encoded = vec![encoding];
    encoded.extend_from_slice(&bytes[..length]);
    Ok((
        text(&encoded).context("Invalid ID3 identity encoding")?,
        length + terminator,
    ))
}
fn identity_and_artwork(
    out: &mut FileTagObservation,
    id: &str,
    bytes: &[u8],
    limits: &ImportLimits,
) -> Result<()> {
    let mut identity = BTreeMap::new();
    match id {
        "COMM" | "COM" | "USLT" | "ULT" => {
            ensure!(bytes.len() >= 4, "Truncated ID3 comment/lyrics");
            identity.insert("language".into(), std::str::from_utf8(&bytes[1..4])?.into());
            let (description, n) = terminated(&bytes[4..], bytes[0])?;
            identity.insert("description".into(), description);
            let mut value = vec![bytes[0]];
            value.extend_from_slice(&bytes[4 + n..]);
            if let Some(value) = text(&value) {
                out.normalized
                    .entry(
                        if matches!(id, "COMM" | "COM") {
                            "comment"
                        } else {
                            "lyrics"
                        }
                        .into(),
                    )
                    .or_default()
                    .push(value);
            }
        }
        "TXXX" | "TXX" => {
            let (&encoding, payload) = bytes.split_first().context("Truncated ID3 user text")?;
            let (description, _) = terminated(payload, encoding)?;
            identity.insert("description".into(), description);
        }
        "UFID" | "UFI" | "PRIV" => {
            let (owner, _) = terminated(bytes, 0)?;
            identity.insert("owner".into(), owner);
        }
        "APIC" => {
            let (&encoding, payload) = bytes.split_first().context("Truncated APIC")?;
            let (mime, n) = terminated(payload, 0)?;
            let picture_type = *payload.get(n).context("Missing APIC picture type")?;
            let (description, len) = terminated(&payload[n + 1..], encoding)?;
            let image = &payload[n + 1 + len..];
            identity.insert("mime_type".into(), mime.clone());
            identity.insert("description".into(), description.clone());
            identity.insert("picture_type".into(), picture_type.to_string());
            out.add_artwork(
                Some(mime),
                Some(picture_type as u32),
                Some(description),
                image,
                limits,
            )?;
        }
        "GEOB" | "GEO" => {
            let (&encoding, payload) = bytes.split_first().context("Truncated GEOB")?;
            let (mime, n) = terminated(payload, 0)?;
            let (filename, m) = terminated(&payload[n..], encoding)?;
            let (description, _) = terminated(&payload[n + m..], encoding)?;
            identity.insert("mime_type".into(), mime);
            identity.insert("filename".into(), filename);
            identity.insert("description".into(), description);
        }
        _ => {}
    }
    out.records
        .last_mut()
        .context("Missing native tag record")?
        .identity
        .extend(identity);
    Ok(())
}
