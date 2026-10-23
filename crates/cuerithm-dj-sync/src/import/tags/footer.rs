use super::*;

pub(super) fn read(
    file: &mut File,
    out: &mut FileTagObservation,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<()> {
    let total = file.metadata()?.len();
    let mut end = total;
    if total >= 128 {
        file.seek(SeekFrom::End(-128))?;
        let mut tag = [0; 128];
        file.read_exact(&mut tag)?;
        if tag.starts_with(b"TAG") {
            out.record("id3v1", "TAG", 0, &tag, limits)?;
            end -= 128;
            for (field, range) in [
                ("title", 3..33),
                ("artists", 33..63),
                ("album", 63..93),
                ("year", 93..97),
                ("comment", 97..127),
            ] {
                let text: String = tag[range].iter().map(|b| char::from(*b)).collect();
                let text = text.trim_end_matches(['\0', ' ']).to_owned();
                if !text.is_empty() {
                    out.normalized.entry(field.into()).or_default().push(text);
                }
            }
        }
    }
    if end < 32 {
        return Ok(());
    }
    file.seek(SeekFrom::Start(end - 32))?;
    let mut footer = [0; 32];
    file.read_exact(&mut footer)?;
    if !footer.starts_with(b"APETAGEX") {
        return Ok(());
    }
    let version = u32::from_le_bytes(footer[8..12].try_into()?);
    ensure!(
        matches!(version, 1000 | 2000),
        "Unsupported APE tag version"
    );
    let size = u32::from_le_bytes(footer[12..16].try_into()?) as u64;
    let count = u32::from_le_bytes(footer[16..20].try_into()?) as usize;
    ensure!(
        size >= 32 && size <= end && size <= limits.text_bytes && count <= 65536,
        "Invalid APE tag bounds"
    );
    file.seek(SeekFrom::Start(end - size))?;
    let mut bytes = vec![0; size as usize];
    file.read_exact(&mut bytes)?;
    out.record("ape", "envelope", 0, &bytes, limits)?;
    let mut p = 0usize;
    let data = &bytes[..bytes.len() - 32];
    for _ in 0..count {
        cancel.check()?;
        ensure!(
            p.checked_add(8).is_some_and(|n| n <= data.len()),
            "Truncated APE item header"
        );
        let len = u32::from_le_bytes(data[p..p + 4].try_into()?) as usize;
        let flags = u32::from_le_bytes(data[p + 4..p + 8].try_into()?);
        p += 8;
        let key_end = data[p..]
            .iter()
            .position(|b| *b == 0)
            .context("Unterminated APE key")?
            + p;
        let key = std::str::from_utf8(&data[p..key_end])?;
        p = key_end + 1;
        let value_end = p.checked_add(len).context("APE item length overflow")?;
        let value = data.get(p..value_end).context("Truncated APE item")?;
        out.record("ape.item", key, (flags & 0xffff) as u16, value, limits)?;
        out.records
            .last_mut()
            .unwrap()
            .identity
            .insert("flags_u32".into(), flags.to_string());
        if flags & 6 == 0 {
            let field = match key.to_ascii_lowercase().as_str() {
                "title" => Some("title"),
                "artist" => Some("artists"),
                "album" => Some("album"),
                "initialkey" => Some("key"),
                _ => None,
            };
            if let Some(field) = field {
                out.normalized
                    .entry(field.into())
                    .or_default()
                    .extend(std::str::from_utf8(value)?.split('\0').map(str::to_owned));
            }
        }
        p = value_end;
    }
    ensure!(p == data.len(), "Unaccounted APE item bytes");
    Ok(())
}
