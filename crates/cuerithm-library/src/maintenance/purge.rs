//! What removing a track record purges beyond the records Undo needs.
//!
//! Removed music must not linger in the database or in cache folders. Undo needs the track
//! record and what belongs to it (cues, tags, links, history, playlist positions); it does
//! not need anything that is rebuilt on demand. This module is the one list of those
//! rebuildable parts, for every way a record can leave the library (a duplicate merge that
//! deletes files, a Missing & broken removal). Caches that are keyed by file content are
//! only removed when no remaining track or job has the same content (an identical
//! duplicate shares them).
use super::missing::{PREVIEW_RELINK, PREVIEW_REMOVAL};
use crate::waveform::{CACHED_RESOLUTIONS, waveform_cache_key};
use anyhow::Result;
use rusqlite::{Connection, params_from_iter};
use std::{collections::BTreeSet, path::Path};

/// Per-track tables whose rows are caches or flags that are rebuilt on demand. They are
/// deleted with the track and are not stored for Undo.
pub(super) const DERIVED_TABLES: [&str; 5] = [
    "maintenance_fingerprints",
    "smart_playlist_members",
    "curation_native_stale",
    "preparation_reuse",
    "preparation_match_previews",
];
/// Tables whose rows the caller rewrites before the records go, because their order is
/// owned by other code: playlist positions.
pub(super) const CALLER_TABLES: [&str; 1] = ["playlist_members"];
/// Saved reports that list every library track with its path and fingerprint. A removal
/// makes all of them stale, so they are deleted rather than kept with the removed music
/// in them.
const STALE_REPORTS: [&str; 3] = ["duplicates", "duplicate_review", "duplicate_merge_plan"];
/// Saved reports and previews that only mention the tracks they were made for.
const NAMING_REPORTS: [&str; 3] = ["broken", PREVIEW_REMOVAL, PREVIEW_RELINK];

fn marks(n: usize) -> String {
    vec!["?"; n].join(",")
}
fn column(conn: &Connection, sql: &str, values: &[String]) -> Result<Vec<String>> {
    Ok(conn
        .prepare(sql)?
        .query_map(params_from_iter(values), |r| r.get::<_, Option<String>>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

/// What identifies the caches derived from a set of tracks. Read it before the records are
/// deleted: afterwards the rows that say which caches belong to them are gone.
#[derive(Default)]
pub(super) struct Derived {
    hashes: BTreeSet<String>,
    decoders: BTreeSet<String>,
    generations: BTreeSet<String>,
}
pub(super) fn collect(conn: &Connection, ids: &[String]) -> Result<Derived> {
    let mut derived = Derived::default();
    for chunk in ids.chunks(400) {
        let m = marks(chunk.len());
        derived.hashes.extend(column(
            conn,
            &format!("SELECT source_hash FROM tracks WHERE id IN ({m})"),
            chunk,
        )?);
        derived.hashes.extend(column(
            conn,
            &format!("SELECT source_hash FROM jobs WHERE track_id IN ({m})"),
            chunk,
        )?);
        derived.decoders.extend(column(
            conn,
            &format!(
                "SELECT json_extract(analysis_json,'$.media.decoder') FROM tracks WHERE id IN ({m})"
            ),
            chunk,
        )?);
        derived.generations.extend(column(
            conn,
            &format!(
                "SELECT generation FROM preparation_match_generations WHERE track_id IN ({m})"
            ),
            chunk,
        )?);
    }
    let hashes: Vec<String> = derived.hashes.iter().cloned().collect();
    for chunk in hashes.chunks(400) {
        derived.decoders.extend(column(
            conn,
            &format!(
                "SELECT decoder FROM preparation_analysis WHERE source_hash IN ({})",
                marks(chunk.len())
            ),
            chunk,
        )?);
    }
    Ok(derived)
}

/// Cache files that only the removed tracks used.
pub(super) struct Orphans {
    hashes: BTreeSet<String>,
    decoders: BTreeSet<String>,
}

/// Deletes, after the records are gone, the rows that no foreign key reaches: cached
/// analysis results, review artwork, the bookkeeping row that remembers when a track last
/// changed (Undo creates it again), and the saved reports that list the removed music.
/// Names the source hashes nothing in the library uses any more. A hash another track or job
/// still has (a twin) keeps its caches.
pub(super) fn rows(conn: &Connection, derived: &Derived, ids: &[String]) -> Result<Orphans> {
    let hashes: Vec<String> = derived.hashes.iter().cloned().collect();
    let mut used = BTreeSet::new();
    for chunk in hashes.chunks(400) {
        let m = marks(chunk.len());
        for table in ["tracks", "jobs"] {
            used.extend(column(
                conn,
                &format!("SELECT source_hash FROM {table} WHERE source_hash IN ({m})"),
                chunk,
            )?);
        }
    }
    let orphaned: BTreeSet<String> = derived.hashes.difference(&used).cloned().collect();
    let gone: Vec<String> = orphaned.iter().cloned().collect();
    let generations: Vec<String> = derived.generations.iter().cloned().collect();
    for (sql, values) in [
        (
            "DELETE FROM preparation_analysis WHERE source_hash IN",
            &gone,
        ),
        (
            "DELETE FROM preparation_review_artwork WHERE generation IN",
            &generations,
        ),
        (
            "DELETE FROM maintenance_track_epochs WHERE id IN",
            &ids.to_vec(),
        ),
    ] {
        for chunk in values.chunks(400) {
            conn.execute(
                &format!("{sql} ({})", marks(chunk.len())),
                params_from_iter(chunk),
            )?;
        }
    }
    saved_reports(conn, ids)?;
    Ok(Orphans {
        hashes: orphaned,
        decoders: derived.decoders.clone(),
    })
}

/// Saved reports and previews that list the removed music. Every duplicate scan, review and
/// merge plan goes (all stale after a removal); a file report or a repair preview goes when it
/// names a removed track.
fn saved_reports(conn: &Connection, ids: &[String]) -> Result<usize> {
    let mut removed = 0;
    for kind in STALE_REPORTS {
        removed += conn.execute("DELETE FROM maintenance_scans WHERE kind=?1", [kind])?;
    }
    for id in ids {
        for kind in NAMING_REPORTS {
            removed += conn.execute(
                "DELETE FROM maintenance_scans WHERE kind=?1 AND instr(payload_json,?2)>0",
                [kind, id.as_str()],
            )?;
        }
    }
    Ok(removed)
}

/// Cached cover art and waveforms of the removed content. Both are rebuilt on demand if the
/// content ever returns. `also_decoders` names decoders the caller knows were in use beyond
/// those the library recorded. Failures are ignored: a cache file that cannot be removed does
/// no harm and is found again by the next purge.
pub(super) fn cache_files(data_dir: &Path, orphans: &Orphans, also_decoders: &[String]) -> usize {
    let mut removed = 0;
    let decoders: BTreeSet<&str> = orphans
        .decoders
        .iter()
        .map(String::as_str)
        .chain(also_decoders.iter().map(String::as_str))
        .collect();
    for hash in &orphans.hashes {
        // Cover art is cached under the content hash itself. Only a real hash is ever
        // turned into a path.
        if hash.len() == 64
            && hash.bytes().all(|b| b.is_ascii_hexdigit())
            && std::fs::remove_file(data_dir.join("cache/artwork").join(hash)).is_ok()
        {
            removed += 1;
        }
        for decoder in &decoders {
            for bins in CACHED_RESOLUTIONS {
                let key = waveform_cache_key(hash, decoder, bins);
                if std::fs::remove_file(
                    data_dir.join("cache/waveforms").join(format!("{key}.json")),
                )
                .is_ok()
                {
                    removed += 1;
                }
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
        let orphans = Orphans {
            hashes: BTreeSet::from([gone.clone()]),
            decoders: BTreeSet::new(),
        };
        // The decoder the library never recorded is named by the caller.
        assert_eq!(
            cache_files(dir.path(), &orphans, &["decoder".to_owned()]),
            3
        );
        assert!(!art.join(&gone).exists() && art.join(&kept).exists());
        assert_eq!(std::fs::read_dir(&waves).unwrap().count(), 2);
        // A name that is not a hash is never turned into a path.
        let escape = Orphans {
            hashes: BTreeSet::from(["../escape".to_owned()]),
            decoders: BTreeSet::from(["decoder".to_owned()]),
        };
        assert_eq!(cache_files(dir.path(), &escape, &[]), 0);
    }
}
