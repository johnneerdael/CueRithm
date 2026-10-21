//! Decode only the stored absolute path in a bounded CoreFoundation bookmark.
//! This does not resolve security scope, file IDs, aliases or moved files.
use anyhow::{Context, Result, ensure};
use std::{collections::HashSet, path::PathBuf};

const LIMIT: usize = 1024 * 1024;
fn word(bytes: &[u8], at: usize) -> Result<usize> {
    let end = at.checked_add(4).context("bookmark offset overflow")?;
    Ok(u32::from_le_bytes(
        bytes
            .get(at..end)
            .context("truncated bookmark word")?
            .try_into()?,
    ) as usize)
}
fn item(bytes: &[u8], base: usize, offset: usize, kind: usize) -> Result<&[u8]> {
    let at = base.checked_add(offset).context("bookmark item overflow")?;
    let len = word(bytes, at)?;
    ensure!(
        word(bytes, at.checked_add(4).context("bookmark item overflow")?)? & 0xffff_ff00 == kind,
        "unsupported bookmark item type"
    );
    let start = at.checked_add(8).context("bookmark item overflow")?;
    let end = start.checked_add(len).context("bookmark item overflow")?;
    bytes.get(start..end).context("truncated bookmark item")
}
fn read(bytes: &[u8]) -> Result<PathBuf> {
    let len = word(bytes, 4)?;
    let base = word(bytes, 12)?;
    ensure!(
        base >= 16 && base < len && len <= bytes.len(),
        "invalid bookmark header"
    );
    let bytes = &bytes[..len];
    let mut toc = word(bytes, base)?;
    let mut seen = HashSet::new();
    let mut paths = HashSet::new();
    let mut entries = 0usize;
    while toc != 0 {
        ensure!(
            seen.len() < 128 && seen.insert(toc),
            "bookmark TOC cycle or budget exceeded"
        );
        let at = base.checked_add(toc).context("bookmark TOC overflow")?;
        ensure!(
            word(bytes, at.checked_add(4).context("bookmark TOC overflow")?)? == 0xffff_fffe,
            "invalid bookmark TOC magic"
        );
        let count = word(bytes, at.checked_add(16).context("bookmark TOC overflow")?)?;
        entries = entries
            .checked_add(count)
            .context("bookmark entry overflow")?;
        ensure!(entries <= 4096, "bookmark entry budget exceeded");
        let start = at.checked_add(20).context("bookmark TOC overflow")?;
        let end = start
            .checked_add(count.checked_mul(12).context("bookmark TOC overflow")?)
            .context("bookmark TOC overflow")?;
        ensure!(end <= bytes.len(), "truncated bookmark TOC");
        for entry in (start..end).step_by(12) {
            if word(bytes, entry)? != 0x1004 {
                continue;
            }
            let array = item(bytes, base, word(bytes, entry + 4)?, 0x600)?;
            ensure!(
                !array.is_empty() && array.len() % 4 == 0 && array.len() <= 1024,
                "invalid bookmark path array"
            );
            let mut path = PathBuf::from("/");
            for offset in array.as_chunks::<4>().0 {
                let component = item(bytes, base, u32::from_le_bytes(*offset) as usize, 0x100)?;
                let component =
                    std::str::from_utf8(component).context("invalid bookmark path UTF-8")?;
                ensure!(
                    !component.is_empty()
                        && component != "."
                        && component != ".."
                        && !component.contains(['/', '\\', '\0']),
                    "unsafe bookmark path component"
                );
                path.push(component);
            }
            paths.insert(path);
        }
        toc = word(bytes, at + 12)?;
    }
    ensure!(paths.len() == 1, "missing or ambiguous bookmark path");
    paths.into_iter().next().context("missing bookmark path")
}
/// Inspect only binary bookmark properties, including an embedded bookmark.
/// Unknown opaque data without a bookmark signature remains unclaimed.
pub(super) fn stored_path(bytes: &[u8]) -> Result<Option<PathBuf>> {
    ensure!(bytes.len() <= LIMIT, "bookmark byte budget exceeded");
    let mut paths = HashSet::new();
    let mut matches = 0;
    let mut end_of_bookmark = 0;
    for (at, magic) in bytes.windows(4).enumerate() {
        if at < end_of_bookmark || magic != b"book" {
            continue;
        }
        matches += 1;
        ensure!(matches <= 32, "bookmark signature budget exceeded");
        paths.insert(read(&bytes[at..])?);
        end_of_bookmark = at
            .checked_add(word(bytes, at + 4)?)
            .context("bookmark length overflow")?;
    }
    ensure!(paths.len() <= 1, "ambiguous embedded bookmark paths");
    Ok(paths.into_iter().next())
}
