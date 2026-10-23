//! Follows a file that was moved or renamed inside a watch folder.
//!
//! A track is identified by its stored path. When the file is moved outside
//! CueRithm that path breaks and the watch folder sees the same audio as a new
//! file, which would create a second track beside the missing one. Before a
//! discovered file is admitted as new work, [`Library::relink_moved_tracks`]
//! asks whether it is the content of a track whose file has disappeared from the
//! same watch folder and, only when that answer is unambiguous, points the
//! existing track at the new location.
//!
//! The change itself is the one Organize files makes when it commits a path:
//! `tracks.path` is updated and the `organization_path_revision` and
//! `smart_tracks_update` triggers advance `path_revision` and the smart-playlist
//! input revision. Everything else about the track stays as it is.
use crate::{
    JobId, Library, TrackId,
    model::*,
    relink::{Move, Scope, is_gone, stamp},
};
use anyhow::{Context, Result};
use cuerithm_media::file_sha256;
use rusqlite::OptionalExtension;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};

/// A library track whose stored path now points at the file's new location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelinkedTrack {
    pub track_id: TrackId,
    /// The track's current job, if it has one. Relinking never creates a job.
    pub job_id: Option<JobId>,
    pub from: PathBuf,
    pub to: PathBuf,
}
#[derive(Debug, Default)]
pub struct WatchMoveOutcome {
    pub relinked: Vec<RelinkedTrack>,
    /// Candidates (as the caller passed them) that now belong to a relinked
    /// track and must not be admitted as new files.
    pub consumed: Vec<PathBuf>,
    /// Candidates that are still being written. Ask again once they have settled.
    pub deferred: Vec<PathBuf>,
}

struct Missing {
    id: TrackId,
    path: PathBuf,
    hash: String,
    ready: bool,
}
fn spelling(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}
struct Arrival {
    given: PathBuf,
    path: PathBuf,
    stamp: Option<(u64, std::time::SystemTime)>,
}

impl Library {
    /// Relinks tracks whose files were moved or renamed inside a watch folder.
    ///
    /// `candidates` are files the watch pipeline just discovered. A candidate is
    /// paired with a track only when all of these hold:
    /// - the track is ready, not archived, and its stored path lies inside a
    ///   watch folder that currently exists, where its file is definitely gone;
    /// - the candidate lies inside that same watch folder and its path is not
    ///   owned by any library track or organization operation;
    /// - the candidate's SHA-256 equals the track's stored source hash;
    /// - exactly one such missing track and exactly one such candidate share
    ///   that hash (true duplicates are never paired by guesswork);
    /// - the track has no file work in flight.
    ///
    /// Anything else is left untouched so the caller admits it as it always has.
    /// `settled` is asked, once at least one track is missing from a watch folder,
    /// about every candidate that no track owns, whether the file has stopped
    /// changing; a file still being written is reported in `deferred` instead of
    /// being hashed. There is no way to tell beforehand which of those files could
    /// match: a track keeps no size or other fingerprint of the file as it is on
    /// disk now (the probed size is from analysis and goes stale when tags are
    /// written), so each is hashed and the hash is compared with the missing
    /// tracks'. A library with nothing missing never reaches this point.
    pub fn relink_moved_tracks(
        &self,
        candidates: &[PathBuf],
        folders: &[PathBuf],
        mut settled: impl FnMut(&Path) -> bool,
    ) -> Result<WatchMoveOutcome> {
        let mut outcome = WatchMoveOutcome::default();
        // An unreachable watch folder is an offline disk, not evidence of a move.
        let roots: Vec<PathBuf> = folders
            .iter()
            .filter_map(|folder| folder.canonicalize().ok())
            .filter(|folder| folder.is_dir())
            .collect();
        if candidates.is_empty() || roots.is_empty() {
            return Ok(outcome);
        }
        let mut arrivals = Vec::new();
        for given in candidates {
            let Ok(path) = given.canonicalize() else {
                continue;
            };
            if !path.is_file()
                || path.starts_with(self.data_dir())
                || !roots.iter().any(|root| path.starts_with(root))
                || self.path_is_owned(&path)?
            {
                continue;
            }
            arrivals.push(Arrival {
                given: given.clone(),
                path,
                stamp: None,
            });
        }
        if arrivals.is_empty() {
            return Ok(outcome);
        }
        // A case-only rename on a case-insensitive volume: the stored spelling still
        // resolves to this very file, so it is that track, not a new file. The track
        // keeps its stored spelling, which works.
        let spellings = self.stored_spellings()?;
        arrivals.retain(|arrival| {
            let alias = spellings
                .get(&spelling(&arrival.path))
                .is_some_and(|stored| {
                    stored.iter().any(|known| {
                        known != &arrival.path
                            && same_file::is_same_file(known, &arrival.path).unwrap_or(false)
                    })
                });
            if alias {
                outcome.consumed.push(arrival.given.clone());
            }
            !alias
        });
        if arrivals.is_empty() {
            return Ok(outcome);
        }
        let missing = self.missing_in_roots(&roots)?;
        if missing.is_empty() {
            return Ok(outcome);
        }
        let wanted: HashSet<&str> = missing.iter().map(|track| track.hash.as_str()).collect();
        let mut by_hash: BTreeMap<String, Vec<Arrival>> = BTreeMap::new();
        for mut arrival in arrivals {
            if !settled(&arrival.path) {
                outcome.deferred.push(arrival.given);
                continue;
            }
            arrival.stamp = stamp(&arrival.path);
            let Ok(hash) = file_sha256(&arrival.path) else {
                continue;
            };
            if stamp(&arrival.path) != arrival.stamp {
                outcome.deferred.push(arrival.given);
                continue;
            }
            if wanted.contains(hash.as_str()) {
                by_hash.entry(hash).or_default().push(arrival);
            }
        }
        for (hash, arrivals) in by_hash {
            let tracks: Vec<&Missing> = missing.iter().filter(|track| track.hash == hash).collect();
            let ([track], [arrival]) = (tracks.as_slice(), arrivals.as_slice()) else {
                continue;
            };
            let same_folder = roots
                .iter()
                .any(|root| track.path.starts_with(root) && arrival.path.starts_with(root));
            if !track.ready || !same_folder {
                continue;
            }
            if let Some(relinked) = self.relink_track(track, arrival)? {
                outcome.consumed.push(arrival.given.clone());
                outcome.relinked.push(relinked);
            }
        }
        Ok(outcome)
    }

    /// Every stored path, grouped by its lowercase spelling.
    fn stored_spellings(&self) -> Result<HashMap<String, Vec<PathBuf>>> {
        let rows: Vec<Vec<u8>> = {
            let conn = self.conn()?;
            let mut statement = conn.prepare("SELECT path FROM tracks WHERE path IS NOT NULL")?;
            statement
                .query_map([], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut spellings: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for bytes in rows {
            if let Ok(path) = bytes_path(&bytes) {
                spellings.entry(spelling(&path)).or_default().push(path);
            }
        }
        Ok(spellings)
    }

    fn path_is_owned(&self, path: &Path) -> Result<bool> {
        let encoded = path_bytes(path);
        Ok(self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM tracks WHERE path=?1)",
            [encoded],
            |row| row.get(0),
        )?)
    }
    /// Non-archived tracks stored inside a watch folder whose file is gone.
    /// Tracks in any state count, so an unfinished track with the same content
    /// still makes a pairing ambiguous.
    fn missing_in_roots(&self, roots: &[PathBuf]) -> Result<Vec<Missing>> {
        let rows: Vec<(String, Vec<u8>, String, String)> = {
            let conn = self.conn()?;
            let mut statement =
                conn.prepare("SELECT id,path,source_hash,state FROM tracks WHERE archived=0 AND path IS NOT NULL AND source_hash IS NOT NULL")?;
            statement
                .query_map([], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .collect::<rusqlite::Result<_>>()?
        };
        let mut missing = Vec::new();
        for (id, path, hash, state) in rows {
            let Ok(path) = bytes_path(&path) else {
                continue;
            };
            if roots.iter().any(|root| path.starts_with(root)) && is_gone(&path) {
                missing.push(Missing {
                    id,
                    path,
                    hash,
                    ready: state == "ready",
                });
            }
        }
        Ok(missing)
    }

    /// Points `track` at `arrival` if nothing else has claimed either path.
    /// Takes the same gates as admission so a publication cannot interleave.
    fn relink_track(&self, track: &Missing, arrival: &Arrival) -> Result<Option<RelinkedTrack>> {
        let _organization_gate = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        // Re-check what was observed before waiting for the gates.
        if stamp(&arrival.path) != arrival.stamp {
            return Ok(None);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let job_id: Option<String> = tx
            .query_row(
                "SELECT current_job FROM tracks WHERE id=?1",
                [&track.id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let moved = Move {
            track: &track.id,
            from: &track.path,
            to: &arrival.path,
            hash: &track.hash,
        };
        if Self::relink_in_tx(&tx, &moved, Scope::ActiveReady)?.is_err() {
            return Ok(None);
        }
        tx.commit().context("Relink moved track")?;
        Ok(Some(RelinkedTrack {
            track_id: track.id.clone(),
            job_id,
            from: track.path.clone(),
            to: arrival.path.clone(),
        }))
    }
}
