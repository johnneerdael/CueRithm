//! What removing a track record purges beyond the records Undo needs.
//!
//! Removed music must not linger in the database or in cache folders. Undo needs the track
//! record and what belongs to it (cues, tags, links, history, playlist positions); it does
//! not need anything that is rebuilt on demand. This module lists the rebuildable parts and
//! removes them when the records go. Caches that are keyed by file content are only removed
//! when no remaining track has the same content (an identical duplicate shares them).
// Purging happens when the coordinated merge service removes records, which needs the
// catalog-tagging feature.
#![cfg_attr(not(feature = "autotag"), allow(dead_code))]
use crate::waveform::{CACHED_RESOLUTIONS, waveform_cache_key};
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

/// Per-track tables whose rows are caches or flags that are rebuilt on demand. They are
/// deleted with the track and are not stored for Undo.
pub(super) const DERIVED_TABLES: [&str; 3] = [
    "maintenance_fingerprints",
    "smart_playlist_members",
    "curation_native_stale",
];

/// Source hashes of the removed tracks that no remaining track shares.
pub(super) fn orphaned_hashes(conn: &Connection, hashes: &[String]) -> Result<Vec<String>> {
    let mut orphaned = vec![];
    for hash in hashes {
        let shared: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tracks WHERE source_hash=?1)",
            [hash],
            |r| r.get(0),
        )?;
        if !shared && !orphaned.contains(hash) {
            orphaned.push(hash.clone());
        }
    }
    Ok(orphaned)
}
/// Cached analysis results are keyed by file content, not by track.
pub(super) fn analysis_cache(conn: &Connection, orphaned: &[String]) -> Result<usize> {
    let mut removed = 0;
    for hash in orphaned {
        removed += conn.execute(
            "DELETE FROM preparation_analysis WHERE source_hash=?1",
            [hash],
        )?;
    }
    Ok(removed)
}
/// Saved duplicate scans, reviews and previews list every library track with its path and
/// fingerprint. A merge makes them all stale, so they are removed rather than kept with the
/// removed music in them.
pub(super) fn saved_duplicate_reports(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "DELETE FROM maintenance_scans WHERE kind IN ('duplicates','duplicate_review','duplicate_merge_plan')",
        [],
    )?)
}
/// The bookkeeping row that remembers when a track last changed; a removed track has no use
/// for it, and Undo creates it again.
pub(super) fn epoch_rows(conn: &Connection, removed: &[String]) -> Result<usize> {
    let mut count = 0;
    for id in removed {
        count += conn.execute("DELETE FROM maintenance_track_epochs WHERE id=?1", [id])?;
    }
    Ok(count)
}
/// Cached cover art and waveforms of the removed content. Both are rebuilt on demand if the
/// content ever returns. Failures are ignored: a cache file that cannot be removed does no
/// harm and is found again by the next purge.
pub(super) fn cache_files(data_dir: &Path, decoder_identity: &str, orphaned: &[String]) -> usize {
    let mut removed = 0;
    for hash in orphaned {
        // Cover art is cached under the content hash itself.
        if hash.bytes().all(|b| b.is_ascii_hexdigit())
            && std::fs::remove_file(data_dir.join("cache/artwork").join(hash)).is_ok()
        {
            removed += 1;
        }
        for bins in CACHED_RESOLUTIONS {
            let key = waveform_cache_key(hash, decoder_identity, bins);
            if std::fs::remove_file(data_dir.join("cache/waveforms").join(format!("{key}.json")))
                .is_ok()
            {
                removed += 1;
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_files_of_removed_content_go_and_other_files_stay() {
        let dir = tempfile::tempdir().unwrap();
        let art = dir.path().join("cache/artwork");
        let waves = dir.path().join("cache/waveforms");
        std::fs::create_dir_all(&art).unwrap();
        std::fs::create_dir_all(&waves).unwrap();
        let (gone, kept) = ("ab".repeat(32), "cd".repeat(32));
        for hash in [&gone, &kept] {
            std::fs::write(art.join(hash), b"cover").unwrap();
            for bins in CACHED_RESOLUTIONS {
                std::fs::write(
                    waves.join(format!(
                        "{}.json",
                        waveform_cache_key(hash, "decoder", bins)
                    )),
                    b"{}",
                )
                .unwrap();
            }
        }
        assert_eq!(
            cache_files(dir.path(), "decoder", std::slice::from_ref(&gone)),
            3
        );
        assert!(!art.join(&gone).exists() && art.join(&kept).exists());
        assert_eq!(std::fs::read_dir(&waves).unwrap().count(), 2);
        // A name that is not a hash is never turned into a path.
        assert_eq!(cache_files(dir.path(), "decoder", &["../escape".into()]), 0);
    }
}
