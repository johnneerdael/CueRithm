//! Historical embedded paths only. No alias resolution, mounting or security access.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookmarkPathEvidence {
    pub embedded_path: Option<String>,
    pub components: Vec<String>,
    pub volume_path: Option<String>,
    pub sha256: String,
    /// Only offsets and byte facts, never arbitrary machine/security strings.
    pub unknown_records: Vec<Value>,
    pub issues: Vec<String>,
}
fn word(bytes: &[u8], offset: usize) -> Result<usize> {
    let end = offset.checked_add(4).context("Bookmark offset overflow")?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .context("Truncated bookmark")?
            .try_into()?,
    ) as usize)
}
fn item(bytes: &[u8], base: usize, offset: usize) -> Result<(usize, &[u8])> {
    let at = base.checked_add(offset).context("Bookmark item overflow")?;
    let len = word(bytes, at)?;
    let kind = word(bytes, at.checked_add(4).context("Bookmark item overflow")?)?;
    let start = at.checked_add(8).context("Bookmark item overflow")?;
    let end = start.checked_add(len).context("Bookmark item overflow")?;
    Ok((
        kind,
        bytes.get(start..end).context("Truncated bookmark item")?,
    ))
}
pub fn decode_embedded_bookmark_path(
    bytes: &[u8],
    limits: &ImportLimits,
) -> Result<BookmarkPathEvidence> {
    ensure!(
        bytes.len() as u64 <= limits.blob_bytes.min(1024 * 1024) && bytes.starts_with(b"book"),
        "Invalid bookmark header/size"
    );
    let len = word(bytes, 4)?;
    let base = word(bytes, 12)?;
    ensure!(
        len == bytes.len() && base >= 16 && base < len,
        "Invalid bookmark length/base"
    );
    let mut toc = word(bytes, base)?;
    let mut seen = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut volumes = BTreeSet::new();
    let mut unknown_records = Vec::new();
    let mut entries = 0usize;
    while toc != 0 {
        ensure!(
            seen.len() < limits.hierarchy_depth && seen.insert(toc),
            "Cyclic/deep bookmark TOC"
        );
        let at = base.checked_add(toc).context("Bookmark TOC overflow")?;
        ensure!(
            word(bytes, at.checked_add(4).context("Bookmark TOC overflow")?)? == 0xffff_fffe,
            "Unknown bookmark TOC magic"
        );
        let count = word(bytes, at.checked_add(16).context("Bookmark TOC overflow")?)?;
        entries = entries
            .checked_add(count)
            .context("Bookmark entry count overflow")?;
        ensure!(entries <= 4096, "Bookmark entry budget");
        let start = at.checked_add(20).context("Bookmark TOC overflow")?;
        let payload_len = count.checked_mul(12).context("Bookmark TOC overflow")?;
        let end = start
            .checked_add(payload_len)
            .context("Bookmark TOC overflow")?;
        // The independent fixture includes four extra bytes in this length word.
        // Bounds follow the actual entry count; neither convention grants access.
        let declared = word(bytes, at)?;
        ensure!(
            end <= bytes.len() && (declared == payload_len + 20 || declared == payload_len + 24),
            "Invalid bookmark TOC size"
        );
        for entry in (start..end).step_by(12) {
            let key = word(bytes, entry)?;
            let offset = word(bytes, entry + 4)?;
            let (kind, data) = item(bytes, base, offset)?;
            match (key, kind) {
                (0x1004, 0x0601) => {
                    ensure!(!data.is_empty() && data.len().is_multiple_of(4) && data.len() <= 1024, "Invalid bookmark path array");
                    let mut components = Vec::new();
                    for offset in data.as_chunks::<4>().0 {
                        let (kind, text) = item(bytes, base, u32::from_le_bytes(*offset) as usize)?;
                        ensure!(kind == 0x0101, "Unsupported bookmark component");
                        let text = std::str::from_utf8(text)?;
                        ensure!(!text.is_empty() && text != "." && text != ".." && !text.contains(['/', '\\', '\0']), "Unsafe bookmark component");
                        components.push(text.to_owned());
                    }
                    paths.insert(components);
                }
                (0x2002, 0x0101) => { volumes.insert(std::str::from_utf8(data)?.to_owned()); }
                (0x1004 | 0x2002, _) => anyhow::bail!("Unsupported bookmark path/volume variant"),
                _ => unknown_records.push(json!({"key":key,"offset":offset,"type":kind,"bytes":data.len(),"sha256":digest(data)})),
            }
        }
        toc = word(bytes, at + 12)?;
    }
    ensure!(
        paths.len() == 1 && volumes.len() <= 1,
        "Missing/ambiguous bookmark path"
    );
    let components = paths
        .into_iter()
        .next()
        .context("Missing path components")?;
    let volume_path = volumes.into_iter().next();
    let root_volume = volume_path.as_deref() == Some("/");
    Ok(BookmarkPathEvidence {
        embedded_path: root_volume.then(|| format!("/{}", components.join("/"))),
        components,
        volume_path,
        sha256: digest(bytes),
        unknown_records,
        issues: if root_volume {
            vec![]
        } else {
            vec!["unqualified_bookmark_volume".into()]
        },
    })
}
