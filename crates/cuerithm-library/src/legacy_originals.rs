//! One-time removal of the original audio that earlier versions kept in the data folder.
//!
//! Earlier versions kept the original bytes of every file they tagged, for ever, so that a write
//! could be rolled back. Once a write has settled (committed or rolled back, and its cleanup is
//! done) nothing reads that copy. This module deletes the ones that are left, a bounded number
//! per start, and only files the application itself created: the path must be in the private
//! folder of a journal row, inside the data folder, a regular file and not a link. Nothing is
//! ever touched in the user's music folders.
use crate::Library;
use anyhow::Result;
use rusqlite::params;
use std::path::Path;

/// Journals handled per start. A library with more simply continues at the next start.
pub const SWEEP_JOURNALS_PER_START: usize = 500;

/// What one sweep removed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct OriginalsSweep {
    pub files_removed: u64,
    pub bytes_freed: u64,
    /// Settled writes whose original is still waiting for a later sweep.
    pub more_to_do: bool,
}

/// The data folder's private areas the tag-write family keeps its workspaces in.
const AREAS: [&str; 1] = ["tagging"];

fn owned_regular_file(data: &Path, path: &Path) -> Option<u64> {
    let area_ok = AREAS.iter().any(|area| {
        data.join(area)
            .canonicalize()
            .ok()
            .zip(path.parent().and_then(|p| p.canonicalize().ok()))
            .is_some_and(|(area, parent)| parent.starts_with(area))
    });
    let meta = std::fs::symlink_metadata(path).ok()?;
    (area_ok && meta.is_file() && !meta.file_type().is_symlink()).then_some(meta.len())
}

impl Library {
    /// Removes the retained originals of tag writes that have settled. See the module notes.
    pub(crate) fn sweep_legacy_originals(&self) -> Result<OriginalsSweep> {
        self.sweep_legacy_originals_with(SWEEP_JOURNALS_PER_START)
    }
    fn sweep_legacy_originals_with(&self, limit: usize) -> Result<OriginalsSweep> {
        let data = self.data_dir().canonicalize()?;
        let rows: Vec<(String, Vec<u8>, i64, Vec<u8>)> = {
            let conn = self.conn()?;
            let mut stmt = conn.prepare(
                "SELECT w.id,w.workspace,f.ordinal,f.backup FROM tag_write_files f JOIN tag_writes w ON w.id=f.write_id
                  WHERE w.state IN ('committed','rolled_back') AND w.cleanup_complete=1 AND f.backup IS NOT NULL
                  ORDER BY w.created,w.id,f.ordinal LIMIT ?1",
            )?;
            stmt.query_map([limit as i64 + 1], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        let mut sweep = OriginalsSweep {
            more_to_do: rows.len() > limit,
            ..Default::default()
        };
        for (id, workspace, ordinal, backup) in rows.into_iter().take(limit) {
            let (Ok(backup), Ok(workspace)) = (
                crate::model::bytes_path(&backup),
                crate::model::bytes_path(&workspace),
            ) else {
                continue;
            };
            let removed = match owned_regular_file(&data, &backup) {
                Some(bytes) => match std::fs::remove_file(&backup) {
                    Ok(()) => Some(bytes),
                    Err(_) => continue,
                },
                // Already gone, or not a file this application owns: only the stale path goes.
                None if !backup.exists() => None,
                None => continue,
            };
            if let Some(bytes) = removed {
                sweep.files_removed += 1;
                sweep.bytes_freed += bytes;
            }
            self.conn()?.execute(
                "UPDATE tag_write_files SET backup=NULL WHERE write_id=?1 AND ordinal=?2",
                params![id, ordinal],
            )?;
            // The private folders go once nothing is left in them.
            let _ = std::fs::remove_dir(&workspace);
            if let Some(folder) = backup.parent()
                && AREAS
                    .iter()
                    .all(|area| data.join(area).canonicalize().ok() != folder.canonicalize().ok())
            {
                let _ = std::fs::remove_dir(folder);
            }
        }
        if sweep.files_removed > 0 {
            let conn = self.conn()?;
            for (key, add) in [
                ("lean_originals_removed_files", sweep.files_removed),
                ("lean_originals_removed_bytes", sweep.bytes_freed),
            ] {
                conn.execute(
                    "INSERT INTO meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=value+excluded.value",
                    params![key, add as i64],
                )?;
            }
        }
        Ok(sweep)
    }

    /// Running totals of what the start-up sweeps removed: (files, bytes).
    pub fn legacy_originals_removed(&self) -> Result<(u64, u64)> {
        let conn = self.conn()?;
        let read = |key: &str| -> Result<u64> {
            Ok(conn
                .query_row("SELECT value FROM meta WHERE key=?1", [key], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap_or(0) as u64)
        };
        Ok((
            read("lean_originals_removed_files")?,
            read("lean_originals_removed_bytes")?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A settled tag write whose original is still on disk, as an earlier version left it.
    fn legacy_write(lib: &Library, n: usize, size: usize) -> std::path::PathBuf {
        let source = lib.data_dir().join(format!("source{n}.flac"));
        std::fs::write(&source, format!("source {n}")).unwrap();
        lib.enqueue(
            std::slice::from_ref(&source),
            lib.settings().unwrap().revision,
        )
        .unwrap();
        let (job, track, revision): (String, String, String) = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT id,track_id,settings_revision FROM jobs ORDER BY rowid DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        let folder = lib.data_dir().join(format!("tagging/cuerithm-tags-{n}"));
        std::fs::create_dir_all(&folder).unwrap();
        let backup = folder.join("original.flac");
        std::fs::write(&backup, vec![1u8; size]).unwrap();
        let conn = lib.conn().unwrap();
        conn.execute(
            "INSERT INTO tag_writes(id,job_id,track_id,attempt,settings_revision,before_hash,after_hash,pcm_hash,frames,decoder,analysis_json,metadata_json,provenance_json,workspace,state,created,cleanup_complete)
             VALUES(?1,?2,?3,1,?4,'a','b','c',0,'d','{}','{}','{}',?5,'committed',?6,1)",
            params![
                format!("write{n}"),
                job,
                track,
                revision,
                crate::model::path_bytes(&folder),
                n as i64
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tag_write_files(write_id,ordinal,target,staged,backup,before_hash,after_hash,publish_path,displaced_path)
             VALUES(?1,0,X'00',X'00',?2,'a','b',X'00',X'00')",
            params![format!("write{n}"), crate::model::path_bytes(&backup)],
        )
        .unwrap();
        backup
    }

    #[test]
    fn one_start_only_handles_so_many_writes_and_the_next_continues() {
        let d = tempfile::tempdir().unwrap();
        let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
        let backups: Vec<_> = (0..5).map(|n| legacy_write(&lib, n, 1000 + n)).collect();
        let first = lib.sweep_legacy_originals_with(2).unwrap();
        assert_eq!((first.files_removed, first.more_to_do), (2, true));
        assert_eq!(backups.iter().filter(|b| b.exists()).count(), 3);
        let second = lib.sweep_legacy_originals_with(2).unwrap();
        assert_eq!((second.files_removed, second.more_to_do), (2, true));
        let third = lib.sweep_legacy_originals_with(2).unwrap();
        assert_eq!((third.files_removed, third.more_to_do), (1, false));
        assert!(backups.iter().all(|b| !b.exists()));
        assert!(backups.iter().all(|b| !b.parent().unwrap().exists()));
        assert_eq!(
            first.bytes_freed + second.bytes_freed + third.bytes_freed,
            (0..5).map(|n| 1000 + n as u64).sum::<u64>()
        );
        assert_eq!(lib.legacy_originals_removed().unwrap(), (5, 5010));
        assert_eq!(
            lib.sweep_legacy_originals().unwrap(),
            OriginalsSweep::default()
        );
    }

    #[test]
    fn a_write_that_is_not_settled_keeps_its_original() {
        let d = tempfile::tempdir().unwrap();
        let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
        let backup = legacy_write(&lib, 0, 100);
        for (state, complete) in [("prepared", 0), ("conflict", 0), ("committed", 0)] {
            lib.conn()
                .unwrap()
                .execute(
                    "UPDATE tag_writes SET state=?1,cleanup_complete=?2",
                    params![state, complete],
                )
                .unwrap();
            assert_eq!(
                lib.sweep_legacy_originals().unwrap(),
                OriginalsSweep::default()
            );
            assert!(backup.exists(), "{state} with cleanup_complete={complete}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_the_private_folder_is_never_followed() {
        let d = tempfile::tempdir().unwrap();
        let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
        let backup = legacy_write(&lib, 0, 100);
        let music = d.path().join("users-music.flac");
        std::fs::write(&music, b"the user's music").unwrap();
        std::fs::remove_file(&backup).unwrap();
        std::os::unix::fs::symlink(&music, &backup).unwrap();
        assert_eq!(
            lib.sweep_legacy_originals().unwrap(),
            OriginalsSweep::default()
        );
        assert!(music.exists());
    }
}
