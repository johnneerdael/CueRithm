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
    relink::work_in_flight,
};
use anyhow::{Result, ensure};
use cuerithm_media::{CancellationToken, file_sha256_cancellable};
use rusqlite::{Connection, OptionalExtension, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
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

/// Undo puts a record back at its old location. If something sits there now it must be the
/// audio the record describes (for example the file restored from the Trash); otherwise the
/// record and its retained analysis would silently describe the wrong file.
pub(super) fn ensure_old_location(
    path: &Path,
    expected_hash: &str,
    cancel: &CancellationToken,
) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => anyhow::bail!(
            "Cannot undo: the old location {} cannot be checked ({error})",
            path.display()
        ),
        Ok(meta) if meta.is_file() => {
            ensure!(
                file_sha256_cancellable(path, cancel)? == expected_hash,
                "Cannot undo: a different file is now at {}, where the track's file was",
                path.display()
            );
            Ok(())
        }
        Ok(_) => anyhow::bail!(
            "Cannot undo: something other than the audio file is now at {}",
            path.display()
        ),
    }
}
fn ensure_old_locations(image: &RecordImage, cancel: &CancellationToken) -> Result<()> {
    for track in image.removed_tracks()? {
        ensure_old_location(&bytes_path(&track.path)?, &track.hash, cancel)?;
    }
    Ok(())
}

/// Everything Undo checks before it changes anything.
pub(super) fn verify_world(
    conn: &Connection,
    image: &RecordImage,
    cancel: &CancellationToken,
) -> Result<()> {
    ensure_free(conn, image)?;
    ensure_old_locations(image, cancel)
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
}
