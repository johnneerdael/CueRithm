//! Which duplicate files a "Delete from disk" merge may move to the Trash.
//!
//! A file is only listed when every rule holds right now: the file is available and still the
//! bytes the scan saw, it is proven identical to the kept track's audio, no other remaining
//! library record uses it, and nothing else is working on it. A file that fails a rule is left
//! exactly where it is and reported with the reason. This module only reads; it never moves or
//! deletes anything.
use super::{
    files::{current_state, state_phrase},
    records, removal,
    scan::SavedScan,
    *,
};
use crate::{
    Library,
    model::{bytes_path, path_bytes},
};
use anyhow::{Context, Result};
use cuerithm_maintenance::MergePlan;
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use rusqlite::{Connection, OptionalExtension};
use same_file::Handle;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

struct Record {
    path: PathBuf,
    artist: String,
    title: String,
}
fn record(conn: &Connection, id: &str) -> Result<Option<Record>> {
    conn.query_row(
        "SELECT path,artist,title FROM tracks WHERE id=?1",
        [id],
        |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )
    .optional()?
    .map(|(path, artist, title)| {
        Ok(Record {
            path: bytes_path(&path)?,
            artist,
            title,
        })
    })
    .transpose()
}
fn skip(
    id: &str,
    path: &Path,
    keeper: &str,
    reason: DuplicateSkipReason,
    detail: impl Into<Option<String>>,
) -> DuplicateDeletionSkip {
    DuplicateDeletionSkip {
        track_id: id.into(),
        display_path: path.to_string_lossy().into_owned(),
        keeper_id: Some(keeper.into()),
        reason,
        detail: detail.into(),
    }
}
/// Another remaining library record that points at the same physical file as one of the
/// candidates, through a link, a hard link or a different spelling of the path.
fn sharing_records(
    lib: &Library,
    candidates: &BTreeMap<String, (Handle, u64)>,
    leaving: &BTreeSet<&str>,
    cancel: &CancellationToken,
) -> Result<BTreeMap<String, String>> {
    let sizes = candidates
        .values()
        .map(|(_, size)| *size)
        .collect::<BTreeSet<_>>();
    let mut shared = BTreeMap::new();
    // The library connection is shared with the rest of the application, so it is held only
    // for the read, not while the files are examined.
    let rows = {
        let conn = lib.conn()?;
        let mut stmt = conn.prepare("SELECT id,path FROM tracks")?;
        stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (id, path) in rows {
        cancel.check()?;
        if leaving.contains(id.as_str()) {
            continue;
        }
        let path = bytes_path(&path)?;
        // Only a file of the same size can be the same file; this keeps the check to one
        // cheap metadata call for almost every record.
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() || !sizes.contains(&meta.len()) {
            continue;
        }
        let Ok(handle) = Handle::from_path(&path) else {
            continue;
        };
        for (candidate, (other, _)) in candidates {
            if *other == handle {
                shared
                    .entry(candidate.clone())
                    .or_insert_with(|| id.clone());
            }
        }
    }
    Ok(shared)
}
fn reserved(conn: &Connection, path: &Path) -> Result<bool> {
    let bytes = path_bytes(path);
    if removal::ensure_unused_unreserved(conn, std::slice::from_ref(&bytes)).is_err() {
        return Ok(true);
    }
    let busy: i64 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM tag_write_files f JOIN tag_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state='prepared')+(SELECT COUNT(*) FROM cleanup_write_files f JOIN cleanup_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state='prepared')+(SELECT COUNT(*) FROM maintenance_write_files f JOIN maintenance_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state IN ('prepared','conflict'))",
        [&bytes],
        |r| r.get(0),
    )?;
    Ok(busy > 0)
}

/// Verify every file the plan would delete. Files that pass stay in the plan and are
/// returned in `files`; the rest are removed from the plan and returned in `skipped`.
/// Nothing is changed on disk or in the library.
pub(super) fn plan(
    lib: &Library,
    scan: &SavedScan,
    merge: &mut MergePlan,
    trash: &dyn FileTrash,
    cancel: &CancellationToken,
) -> Result<DuplicateDeletionPlan> {
    let rows = scan
        .report
        .tracks
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect::<BTreeMap<_, _>>();
    let candidates = merge
        .groups
        .iter()
        .flat_map(|g| g.delete_ids.iter().map(|id| (g.keeper.clone(), id.clone())))
        .collect::<Vec<_>>();
    let mut files = vec![];
    let mut skipped = vec![];
    let mut sound = vec![]; // (keeper, id, record, size, hash, identity)
    let mut handles: BTreeMap<String, (Handle, u64)> = BTreeMap::new();
    let mut keepers: BTreeMap<String, Option<(Record, String, Handle)>> = BTreeMap::new();
    for (keeper, id) in &candidates {
        cancel.check()?;
        let Some(item) = record(&*lib.conn()?, id)? else {
            skipped.push(DuplicateDeletionSkip {
                track_id: id.clone(),
                display_path: String::new(),
                keeper_id: Some(keeper.clone()),
                reason: DuplicateSkipReason::FileUnavailable,
                detail: Some("The track is no longer in the library.".into()),
            });
            continue;
        };
        let scanned = rows
            .get(id.as_str())
            .context("Duplicate scan lacks a track of this merge")?;
        // The kept track, verified once per group.
        if !keepers.contains_key(keeper) {
            let found = record(&*lib.conn()?, keeper)?;
            let state = match found {
                Some(k) if current_state(&k.path).is_available() => {
                    let hash = file_sha256_cancellable(&k.path, cancel);
                    cancel.check()?;
                    match (hash, Handle::from_path(&k.path)) {
                        (Ok(hash), Ok(handle)) => Some((k, hash, handle)),
                        _ => None,
                    }
                }
                _ => None,
            };
            keepers.insert(keeper.clone(), state);
        }
        let Some(Some((kept, kept_hash, kept_handle))) = keepers.get(keeper) else {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::FileUnavailable,
                "The kept track's file is not available.".to_owned(),
            ));
            continue;
        };
        if let Some(reason) = records::busy_reason(&*lib.conn()?, id)? {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::Busy,
                reason.to_owned(),
            ));
            continue;
        }
        let state = current_state(&item.path);
        if !state.is_available() {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::FileUnavailable,
                format!("The file is not available ({}).", state_phrase(state)),
            ));
            continue;
        }
        if reserved(&*lib.conn()?, &item.path)? {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::Busy,
                "Another operation is using this file.".to_owned(),
            ));
            continue;
        }
        let Ok(handle) = Handle::from_path(&item.path) else {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::FileUnavailable,
                "The file could not be opened.".to_owned(),
            ));
            continue;
        };
        if handle == *kept_handle {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::SameFileAsKeeper,
                format!("This is the kept track's file ({}).", kept.path.display()),
            ));
            continue;
        }
        let hash = file_sha256_cancellable(&item.path, cancel);
        cancel.check()?;
        let Ok(hash) = hash else {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::FileUnavailable,
                "The file could not be read.".to_owned(),
            ));
            continue;
        };
        let kept_row = rows.get(keeper.as_str());
        let unchanged = scanned.observed_sha256.as_deref() == Some(hash.as_str())
            && kept_row.is_some_and(|k| k.observed_sha256.as_deref() == Some(kept_hash.as_str()))
            && path_bytes(&item.path) == path_bytes(Path::new(&scanned.display_path));
        if !unchanged {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::FileChanged,
                "The file or the kept track changed since the scan.".to_owned(),
            ));
            continue;
        }
        let identity = if hash == *kept_hash {
            Some(DuplicateIdentity::SameFile)
        } else {
            match (
                scanned.fingerprint.as_ref(),
                kept_row.and_then(|k| k.fingerprint.as_ref()),
            ) {
                (Some(a), Some(b))
                    if a.source_sha256 == hash
                        && b.source_sha256 == *kept_hash
                        && (a.sample_rate, a.channels, &a.digest)
                            == (b.sample_rate, b.channels, &b.digest) =>
                {
                    Some(DuplicateIdentity::SameAudio)
                }
                _ => None,
            }
        };
        let Some(identity) = identity else {
            skipped.push(skip(
                id,
                &item.path,
                keeper,
                DuplicateSkipReason::NotVerifiedIdentical,
                "The audio is not proven identical to the kept track.".to_owned(),
            ));
            continue;
        };
        let size = std::fs::metadata(&item.path)
            .map(|m| m.len())
            .unwrap_or_default();
        handles.insert(id.clone(), (handle, size));
        sound.push((keeper.clone(), id.clone(), item, size, hash, identity));
    }
    // Only records that really leave the library are not "remaining": a candidate that was
    // skipped above stays, and so does anything it points at.
    let leaving = sound
        .iter()
        .map(|(_, id, ..)| id.as_str())
        .collect::<BTreeSet<_>>();
    let shared = sharing_records(lib, &handles, &leaving, cancel)?;
    let mut keep = BTreeSet::new();
    for (keeper, id, item, size, hash, identity) in sound {
        if let Some(other) = shared.get(&id) {
            let other = record(&*lib.conn()?, other)?
                .map(|r| format!("{} - {}", r.artist, r.title))
                .unwrap_or_default();
            skipped.push(skip(
                &id,
                &item.path,
                &keeper,
                DuplicateSkipReason::SharedWithRemainingRecord,
                format!("Another library record ({other}) still uses this file."),
            ));
            continue;
        }
        let kept = keepers
            .get(&keeper)
            .and_then(Option::as_ref)
            .map(|(k, _, _)| k.path.to_string_lossy().into_owned())
            .unwrap_or_default();
        keep.insert(id.clone());
        let permanent = !trash.can_trash(&item.path);
        files.push(DuplicateDeletionFile {
            track_id: id,
            display_path: item.path.to_string_lossy().into_owned(),
            size_bytes: size,
            keeper_id: keeper,
            keeper_display_path: kept,
            identity,
            sha256: hash,
            permanent,
        });
    }
    for group in &mut merge.groups {
        group.delete_ids.retain(|id| keep.contains(id));
    }
    // Keep the order of the plan: groups in order, and each group's files in order.
    let position = candidates
        .iter()
        .enumerate()
        .map(|(i, (_, id))| (id.clone(), i))
        .collect::<BTreeMap<_, _>>();
    files.sort_by_key(|f| position[&f.track_id]);
    skipped.sort_by_key(|s| position.get(&s.track_id).copied().unwrap_or(usize::MAX));
    let mut impact = vec![];
    for file in &files {
        cancel.check()?;
        let captured = records::capture(&*lib.conn()?, std::slice::from_ref(&file.track_id))?;
        let counts = captured.counts();
        let count = |table: &str| counts.get(table).copied().unwrap_or_default();
        let named = ["cue_revisions", "track_custom_tags"]
            .into_iter()
            .chain([
                "sync_links",
                "native_track_links",
                "native_sync_track_ownership",
            ])
            .map(count)
            .sum::<usize>();
        let total = counts.values().sum::<usize>();
        impact.push(DuplicateRemovalImpact {
            track_id: file.track_id.clone(),
            cue_revisions: count("cue_revisions"),
            tags: count("track_custom_tags"),
            sync_links: count("sync_links")
                + count("native_track_links")
                + count("native_sync_track_ownership"),
            // The track record itself and its playlist entries (moved to the keeper) are
            // not "other" records.
            other_records: total
                .saturating_sub(named)
                .saturating_sub(count("tracks"))
                .saturating_sub(count("playlist_members")),
        });
    }
    Ok(DuplicateDeletionPlan {
        files,
        skipped,
        impact,
        undo_limit: DUPLICATE_UNDO_LIMIT,
    })
}
