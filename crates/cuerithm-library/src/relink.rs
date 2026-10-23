//! The one way a track's stored path is re-pointed at a file that moved.
//!
//! Watch-folder move tracking and the Maintenance "Find moved files" action
//! both end here, so both keep the same invariants: the track row is updated in
//! place (its id, analysis, cues, playlists, links and history stay attached),
//! `path_revision` advances through the existing trigger, nothing is relinked
//! onto a path another record owns, and nothing is relinked while a file
//! operation for the track is unfinished.
use crate::{Library, model::*};
use anyhow::Result;
use cuerithm_media::file_sha256;
use rusqlite::{Connection, Transaction, params};
use std::path::Path;

/// Only a definite "not found" counts: any other error leaves the file's
/// existence unknown, and unknown is never treated as moved or removed.
pub(crate) fn is_gone(path: &Path) -> bool {
    matches!(
        std::fs::symlink_metadata(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
}
pub(crate) fn stamp(path: &Path) -> Option<(u64, std::time::SystemTime)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

/// Why the track's file is not free to be moved or forgotten right now.
pub(crate) fn work_in_flight(conn: &Connection, track: &str) -> Result<Option<&'static str>> {
    const CHECKS: &[(&str, &str)] = &[
        (
            "SELECT EXISTS(SELECT 1 FROM organization_operations WHERE track_id=?1 AND state NOT IN ('complete','unchanged','undone','cancelled'))",
            "A file organization for this track has not finished",
        ),
        (
            "SELECT EXISTS(SELECT 1 FROM preparation_checkpoints WHERE track_id=?1 AND state!='complete')",
            "Preparation of this track has not finished",
        ),
        (
            "SELECT EXISTS(SELECT 1 FROM jobs WHERE track_id=?1 AND state IN ('queued','running'))",
            "This track is queued or being prepared",
        ),
        (
            "SELECT EXISTS(SELECT 1 FROM tag_writes WHERE track_id=?1 AND state='prepared')",
            "A tag write for this track has not finished",
        ),
        (
            "SELECT EXISTS(SELECT 1 FROM cleanup_writes WHERE track_id=?1 AND state='prepared')",
            "A cleanup for this track has not finished",
        ),
        (
            "SELECT EXISTS(SELECT 1 FROM metadata_edit_writes WHERE track_id=?1 AND state IN ('prepared','publishing','recovery_required','conflict'))",
            "A metadata save for this track has not finished",
        ),
        (
            "SELECT EXISTS(SELECT 1 FROM curation_publications WHERE track_id=?1 AND state IN ('saving','waiting_incoming','recovery_required'))",
            "A category save for this track has not finished",
        ),
    ];
    for (sql, reason) in CHECKS {
        if conn.query_row(sql, [track], |row| row.get::<_, bool>(0))? {
            return Ok(Some(reason));
        }
    }
    Ok(None)
}
/// Whether a library track, a reservation or an alternate already owns `path`.
fn path_taken(conn: &Connection, path: &[u8]) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tracks WHERE path=?1)
          OR EXISTS(SELECT 1 FROM organization_reservations WHERE path=?1)
          OR EXISTS(SELECT 1 FROM organization_alternates WHERE path=?1)",
        [path],
        |row| row.get(0),
    )?)
}

/// What a relink may touch.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Watch folders follow ready, active tracks only.
    ActiveReady,
    /// A person chose the track in Maintenance; archived and unfinished tracks included.
    Any,
}
/// What a relink changed besides the track's own path.
pub(crate) struct Relinked {
    /// Owned artwork rows re-pointed with the track, as (old path, new path), so
    /// that undoing the relink can put them back.
    pub sidecars: Vec<(Vec<u8>, Vec<u8>)>,
}
pub(crate) struct Move<'a> {
    pub track: &'a str,
    pub from: &'a Path,
    pub to: &'a Path,
    /// The track's stored source SHA-256, which the file at `to` was verified against.
    pub hash: &'a str,
}

impl Library {
    /// Moves one track's stored path inside `tx`. The caller holds the
    /// organization (and tag) gates and has verified that `to` has the bytes
    /// of `hash`. Returns the reason when the track must be left as it is.
    pub(crate) fn relink_in_tx(
        tx: &Transaction<'_>,
        moved: &Move<'_>,
        scope: Scope,
    ) -> Result<std::result::Result<Relinked, String>> {
        if !is_gone(moved.from) {
            return Ok(Err("The file is back at its old location".into()));
        }
        let (old, new) = (path_bytes(moved.from), path_bytes(moved.to));
        if path_taken(tx, &new)? {
            return Ok(Err(
                "Another library track or file operation already uses the new location".into(),
            ));
        }
        if let Some(reason) = work_in_flight(tx, moved.track)? {
            return Ok(Err(reason.into()));
        }
        let changed = tx.execute(
            "UPDATE tracks SET path=?1,updated=?2 WHERE id=?3 AND path=?4 AND source_hash=?5
               AND (?6 OR (archived=0 AND state='ready'))",
            params![
                new,
                now(),
                moved.track,
                old,
                moved.hash,
                scope == Scope::Any
            ],
        )?;
        if changed != 1 {
            return Ok(Err("The track changed".into()));
        }
        let sidecars = Self::follow_owned_sidecars(tx, moved)?;
        Self::bump(tx)?;
        Ok(Ok(Relinked { sidecars }))
    }

    /// Artwork the library wrote beside the audio moves with a renamed album
    /// folder. A recorded file is re-pointed only if it is gone from its old
    /// place and the identical bytes sit at the same position next to the
    /// track's new location; anything else keeps its recorded location.
    fn follow_owned_sidecars(
        tx: &Transaction<'_>,
        moved: &Move<'_>,
    ) -> Result<Vec<(Vec<u8>, Vec<u8>)>> {
        let mut followed = Vec::new();
        let (Some(old_parent), Some(new_parent)) = (moved.from.parent(), moved.to.parent()) else {
            return Ok(followed);
        };
        let rows: Vec<(Vec<u8>, String)> = {
            let mut statement = tx.prepare(
                "SELECT path,source_hash FROM organization_owned_sidecars WHERE track_id=?1",
            )?;
            statement
                .query_map([moved.track], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        for (recorded, expected) in rows {
            let Ok(sidecar) = bytes_path(&recorded) else {
                continue;
            };
            let Ok(relative) = sidecar.strip_prefix(old_parent) else {
                continue;
            };
            let found = new_parent.join(relative);
            if !is_gone(&sidecar)
                || !found.is_file()
                || file_sha256(&found).ok().as_deref() != Some(expected.as_str())
            {
                continue;
            }
            let new_path = path_bytes(&found);
            let changed = tx.execute(
                "UPDATE OR IGNORE organization_owned_sidecars SET path=?1 WHERE track_id=?2 AND path=?3",
                params![new_path, moved.track, recorded],
            )?;
            if changed == 1 {
                followed.push((recorded, new_path));
            }
        }
        Ok(followed)
    }
}
