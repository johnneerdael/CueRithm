//! The one place where track records leave the library and come back.
//!
//! A duplicate merge that deletes files and a Missing & broken removal both end here, so
//! they save the same records, purge the same derived data, refuse the same unsafe
//! situations and keep the same bounded Undo history. Only rows are involved: audio files,
//! sidecars and caches on disk are never read, moved or deleted by `remove` and `restore`
//! (the caller decides about files; `purge::cache_files` removes caches after the commit).
use super::purge::{self, CALLER_TABLES, DERIVED_TABLES, Orphans};
use crate::{
    model::bytes_path,
    record_snapshot::{Plan, RecordImage, RestoreReport},
    relink::{is_gone, stamp, work_in_flight},
};
use anyhow::{Result, ensure};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// What a removal took out of the library.
pub(super) struct Removed {
    /// Everything Undo needs, and nothing it does not.
    pub image: RecordImage,
    /// Content nothing in the library uses any more, whose cache files may now go.
    pub orphans: Orphans,
}

/// Removes the track records, every row that belongs to them and everything derived from
/// them. The caller has already rewritten playlist positions (so no playlist entry refers to
/// a removed track), holds the publication gates and commits the transaction; cache files
/// are removed with `purge::cache_files` once it has.
pub(super) fn remove(tx: &Transaction<'_>, ids: &[String]) -> Result<Removed> {
    for chunk in ids.chunks(400) {
        let marks = vec!["?"; chunk.len()].join(",");
        let left: i64 = tx.query_row(
            &format!("SELECT COUNT(*) FROM playlist_members WHERE track_id IN ({marks})"),
            rusqlite::params_from_iter(chunk),
            |r| r.get(0),
        )?;
        ensure!(
            left == 0,
            "Playlist entries of the removed tracks must be rewritten first"
        );
    }
    let derived = purge::collect(tx, ids)?;
    let plan = Plan::new(tx, ids, &CALLER_TABLES, &DERIVED_TABLES)?;
    let image = plan.capture(tx)?;
    ensure!(
        image.removed_tracks()?.len() == ids.iter().collect::<BTreeSet<_>>().len(),
        "A track to remove is no longer in the library"
    );
    plan.apply(tx)?;
    let orphans = purge::rows(tx, &derived, ids)?;
    Ok(Removed { image, orphans })
}

/// How many rows of each table would go if this track were removed, for a preview. Nothing
/// is changed.
pub(super) fn impact(conn: &Connection, track_id: &str) -> Result<BTreeMap<String, usize>> {
    Ok(Plan::new(
        conn,
        std::slice::from_ref(&track_id.to_owned()),
        &CALLER_TABLES,
        &DERIVED_TABLES,
    )?
    .counts())
}

/// Refuse to restore when the library no longer looks like the removal left it: a track
/// with a removed track's identity or file location has appeared. Nothing is changed.
pub(super) fn ensure_free(conn: &Connection, image: &RecordImage) -> Result<()> {
    for track in image.removed_tracks()? {
        let taken: Option<String> = conn
            .query_row(
                "SELECT title FROM tracks WHERE id=?1 OR path=?2",
                rusqlite::params![track.id, track.path],
                |r| r.get(0),
            )
            .optional()?;
        ensure!(
            taken.is_none(),
            "Cannot undo: the identity or file location of \"{}\" is already used by a track in the library (\"{}\"). Remove that track first, or keep the current library",
            track.title,
            taken.unwrap_or_default()
        );
    }
    Ok(())
}

/// What stood at a record's old location when Undo looked, before it took any lock. Undo
/// puts a record back there, so whatever sits there must be the audio the record
/// describes (for example the file restored from the Trash); otherwise the record and its
/// retained analysis would describe the wrong file. Hashing is slow, so it happens here,
/// outside every transaction; the write transaction only re-checks cheaply with
/// [`OldLocation::unchanged`].
pub(super) struct OldLocation {
    path: PathBuf,
    /// Size and modification time of the file that was hashed; `None` if nothing was there.
    seen: Option<(u64, std::time::SystemTime)>,
}
pub(super) fn observe_old_location(
    path: &Path,
    expected_hash: &str,
    cancel: &CancellationToken,
) -> Result<OldLocation> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(OldLocation {
            path: path.to_owned(),
            seen: None,
        }),
        Err(error) => anyhow::bail!(
            "Cannot undo: the old location {} cannot be checked ({error})",
            path.display()
        ),
        Ok(meta) if meta.is_file() => {
            let before = stamp(path);
            ensure!(
                file_sha256_cancellable(path, cancel)? == expected_hash,
                "Cannot undo: a different file is now at {}",
                path.display()
            );
            ensure!(
                before.is_some() && stamp(path) == before,
                "Cannot undo: {} changed while it was being checked; try again",
                path.display()
            );
            Ok(OldLocation {
                path: path.to_owned(),
                seen: before,
            })
        }
        Ok(_) => anyhow::bail!(
            "Cannot undo: something other than the audio file is now at {}",
            path.display()
        ),
    }
}
impl OldLocation {
    /// The cheap check inside the transaction: it is still as it was observed.
    pub(super) fn unchanged(&self) -> Result<()> {
        let same = match self.seen {
            None => is_gone(&self.path),
            Some(seen) => stamp(&self.path) == Some(seen),
        };
        ensure!(
            same,
            "Cannot undo: {} changed while Undo was running; try again",
            self.path.display()
        );
        Ok(())
    }
}
/// Looks at where each removed track's file was, hashing whatever is there. Call it before
/// any lock is taken, and check the result with [`OldLocation::unchanged`] inside the
/// transaction that restores the records.
pub(super) fn observe_world(
    image: &RecordImage,
    cancel: &CancellationToken,
) -> Result<Vec<OldLocation>> {
    image
        .removed_tracks()?
        .iter()
        .map(|track| observe_old_location(&bytes_path(&track.path)?, &track.hash, cancel))
        .collect()
}

/// Puts the removed records back, or fails and leaves the caller to roll back.
pub(super) fn restore(tx: &Transaction<'_>, image: &RecordImage) -> Result<RestoreReport> {
    ensure_free(tx, image)?;
    crate::record_snapshot::restore(tx, image)
}

/// Why a track record must not be removed right now, if it must not: it is still being
/// prepared, or work that changes its files or records has not finished.
pub(super) fn busy_reason(conn: &Connection, track_id: &str) -> Result<Option<&'static str>> {
    let state: Option<String> = conn
        .query_row("SELECT state FROM tracks WHERE id=?1", [track_id], |r| {
            r.get(0)
        })
        .optional()?;
    let Some(state) = state else {
        return Ok(Some("The track is no longer in the library."));
    };
    if state != "ready" {
        return Ok(Some("The track is still being prepared."));
    }
    let unfinished: i64 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM jobs WHERE track_id=?1 AND state IN ('queued','running','attention'))+(SELECT COUNT(*) FROM tag_writes WHERE track_id=?1 AND state='prepared')+(SELECT COUNT(*) FROM cleanup_writes WHERE track_id=?1 AND state='prepared')",
        [track_id],
        |r| r.get(0),
    )?;
    if unfinished > 0 {
        return Ok(Some("The track has unfinished work."));
    }
    work_in_flight(conn, track_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    /// A library in the shape the app's own schema has, small enough to read.
    fn library() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE tracks(id TEXT PRIMARY KEY, path BLOB NOT NULL UNIQUE, title TEXT, source_hash TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'ready');
             CREATE TABLE playlist_members(playlist_id TEXT NOT NULL, track_id TEXT NOT NULL REFERENCES tracks(id), position INTEGER NOT NULL, PRIMARY KEY(playlist_id, track_id));
             INSERT INTO tracks(id,path,title,source_hash) VALUES('gone', X'67', 'Gone', 'h1');
             INSERT INTO playlist_members VALUES('set','gone',0);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn the_free_check_names_the_track_whose_place_is_taken() {
        let conn = library();
        let image = Plan::new(&conn, &["gone".to_owned()], &CALLER_TABLES, &DERIVED_TABLES)
            .unwrap()
            .capture(&conn)
            .unwrap();
        // Still in the library: its own identity is taken.
        let error = ensure_free(&conn, &image).unwrap_err().to_string();
        assert!(
            error.contains("Cannot undo") && error.contains("already used"),
            "{error}"
        );
        conn.execute("DELETE FROM playlist_members", []).unwrap();
        conn.execute("DELETE FROM tracks", []).unwrap();
        ensure_free(&conn, &image).unwrap();
        conn.execute(
            "INSERT INTO tracks(id,path,title,source_hash) VALUES('other', X'67', 'Other', 'h2')",
            [],
        )
        .unwrap();
        let error = ensure_free(&conn, &image).unwrap_err().to_string();
        assert!(error.contains("\"Other\""), "{error}");
    }

    #[test]
    fn a_location_that_changes_after_it_was_observed_is_caught_without_hashing_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.flac");
        let hash = format!("{:x}", Sha256::digest(b"the audio"));
        let cancel = CancellationToken::default();
        // Nothing there: observed as free, and a file turning up is a change.
        let free = observe_old_location(&path, &hash, &cancel).unwrap();
        free.unchanged().unwrap();
        std::fs::write(&path, b"the audio").unwrap();
        assert!(free.unchanged().is_err());
        // The right bytes there: accepted; any later change to the file is caught.
        let same = observe_old_location(&path, &hash, &cancel).unwrap();
        same.unchanged().unwrap();
        std::fs::write(&path, b"something longer than the audio").unwrap();
        assert!(same.unchanged().is_err());
        // Other bytes, or something that is not a file: refused outright.
        assert!(observe_old_location(&path, &hash, &cancel).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(observe_old_location(&path, &hash, &cancel).is_err());
    }
}
