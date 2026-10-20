use super::scan::{SavedScan, stamp};
use super::*;
use crate::Library;
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Serialize, Deserialize)]
pub(super) struct SavedReview {
    pub(super) review: DuplicateReview,
    pub(super) scan: SavedScan,
}

pub(super) fn read_review(lib: &Library, id: &str) -> Result<(SavedReview, String)> {
    let text: String = lib
        .conn()?
        .query_row(
            "SELECT payload_json FROM maintenance_scans WHERE id=?1 AND kind='duplicate_review'",
            [id],
            |r| r.get(0),
        )
        .optional()?
        .context("Duplicate review not found")?;
    Ok((serde_json::from_str(&text)?, text))
}

pub(super) fn refresh(
    lib: &Library,
    saved: &mut SavedReview,
    cancel: &CancellationToken,
) -> Result<()> {
    lib.refresh_duplicate_staleness_for(&mut saved.scan, cancel, true)?;
    saved.review.stale = saved.scan.report.stale;
    saved.review.stale_reason = saved.scan.report.stale_reason.clone();
    Ok(())
}

fn ignores(conn: &Connection) -> Result<DuplicateIgnores> {
    let mut stmt = conn.prepare("SELECT t.id,t.path,t.title,t.artist FROM maintenance_ignored_tracks i JOIN tracks t ON t.id=i.track_id ORDER BY t.rowid")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let tracks = rows
        .into_iter()
        .map(|(id, path, title, artist)| {
            Ok(IgnoredDuplicateTrack {
                id,
                display_path: crate::model::bytes_path(&path)?
                    .to_string_lossy()
                    .into_owned(),
                title,
                artist,
            })
        })
        .collect::<Result<_>>()?;
    Ok(DuplicateIgnores {
        stamp: stamp(conn)?,
        tracks,
    })
}

impl Library {
    pub fn duplicate_ignores(&self) -> Result<DuplicateIgnores> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let snapshot = ignores(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }
    /// Manage persistent ignores even after the originating review becomes stale.
    pub fn set_duplicate_ignores(
        &self,
        track_ids: Vec<String>,
        ignored: bool,
        expected: MaintenanceStamp,
    ) -> Result<DuplicateIgnores> {
        ensure!(
            !track_ids.is_empty()
                && track_ids.len() <= 5000
                && track_ids.iter().collect::<BTreeSet<_>>().len() == track_ids.len(),
            "Invalid ignored-track selection"
        );
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            stamp(&tx)? == expected,
            "Ignore inputs changed; reload ignores"
        );
        for id in &track_ids {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM tracks WHERE id=?1)",
                [id],
                |r| r.get(0),
            )?;
            ensure!(exists, "Ignored track is not in this library");
        }
        for id in track_ids {
            if ignored {
                tx.execute(
                    "INSERT OR IGNORE INTO maintenance_ignored_tracks(track_id) VALUES(?1)",
                    [id],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM maintenance_ignored_tracks WHERE track_id=?1",
                    [id],
                )?;
            }
        }
        let snapshot = ignores(&tx)?;
        tx.commit()?;
        Ok(snapshot)
    }
    /// Start a durable review without changing track data or source files.
    pub fn begin_duplicate_review(
        &self,
        scan_id: &str,
        cancel: &CancellationToken,
    ) -> Result<DuplicateReview> {
        let scan = self.load_duplicate_scan(scan_id, cancel)?;
        ensure!(!scan.report.stale, "Duplicate scan is stale; scan again");
        let candidates = scan
            .candidates
            .iter()
            .map(|t| (&t.candidate.id, t))
            .collect::<BTreeMap<_, _>>();
        let mut groups = vec![];
        for group in &scan.report.groups {
            cancel.check()?;
            let tracks = group
                .track_ids
                .iter()
                .map(|id| {
                    candidates
                        .get(id)
                        .map(|t| (*t).clone())
                        .context("Duplicate review inputs missing; scan again")
                })
                .collect::<Result<Vec<_>>>()?;
            groups.push(ReviewGroup::new(
                group.id.clone(),
                tracks,
                group.method == DuplicateMatchMethod::Fingerprint,
            )?);
        }
        let review = DuplicateReview {
            id: uuid::Uuid::new_v4().to_string(),
            scan_id: scan_id.into(),
            created: crate::now(),
            revision: 0,
            stamp: scan.report.stamp.clone(),
            groups,
            stale: false,
            stale_reason: None,
        };
        let saved = SavedReview { review, scan };
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            stamp(&tx)? == saved.review.stamp,
            "Library changed; scan again"
        );
        cancel.check()?;
        tx.execute(
            "INSERT INTO maintenance_scans VALUES(?1,'duplicate_review',?2,?3)",
            params![
                saved.review.id,
                serde_json::to_string(&saved)?,
                saved.review.created
            ],
        )?;
        tx.commit()?;
        Ok(saved.review)
    }
    pub fn duplicate_review(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<DuplicateReview> {
        cancel.check()?;
        let (mut saved, _) = read_review(self, id)?;
        refresh(self, &mut saved, cancel)?;
        Ok(saved.review)
    }
    /// Persist review choices with optimistic revision and coherent input checks.
    /// Only explicit ignore edits touch persistent ignore state; no tags or cues are written.
    pub fn edit_duplicate_review(
        &self,
        id: &str,
        expected_revision: u64,
        edit: DuplicateReviewEdit,
        cancel: &CancellationToken,
    ) -> Result<DuplicateReview> {
        cancel.check()?;
        let (mut saved, original) = read_review(self, id)?;
        ensure!(
            saved.review.revision == expected_revision,
            "Duplicate review changed; reload review"
        );
        refresh(self, &mut saved, cancel)?;
        ensure!(!saved.review.stale, "Duplicate review is stale; scan again");
        let mut ignore_change = None;
        match edit {
            DuplicateReviewEdit::Group { group_id, edit } => {
                let group = saved
                    .review
                    .groups
                    .iter_mut()
                    .find(|g| g.id == group_id)
                    .context("Duplicate group is not in this review")?;
                let ignored_id = if let GroupEdit::ToggleIgnore { track_id } = &edit {
                    Some(track_id.clone())
                } else {
                    None
                };
                group.edit(edit)?;
                ignore_change = ignored_id.map(|id| {
                    let ignored = group.ignored.contains(&id);
                    (id, ignored)
                });
            }
            DuplicateReviewEdit::Prefer {
                preference,
                override_locked,
                use_best_cues,
            } => {
                for group in &mut saved.review.groups {
                    cancel.check()?;
                    group.prefer(&preference, override_locked, use_best_cues)?;
                }
            }
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            stamp(&tx)? == saved.review.stamp,
            "Library changed; scan again"
        );
        let current: String = tx.query_row(
            "SELECT payload_json FROM maintenance_scans WHERE id=?1 AND kind='duplicate_review'",
            [id],
            |r| r.get(0),
        )?;
        ensure!(
            current == original,
            "Duplicate review changed; reload review"
        );
        if let Some((id, ignored)) = ignore_change {
            if ignored {
                tx.execute(
                    "INSERT OR IGNORE INTO maintenance_ignored_tracks(track_id) VALUES(?1)",
                    [id],
                )?;
            } else {
                tx.execute(
                    "DELETE FROM maintenance_ignored_tracks WHERE track_id=?1",
                    [id],
                )?;
            }
            // Only this review's explicit ignore change is adopted. Other reviews and scans
            // keep their previous stamp and become stale rather than silently rebasing.
            saved.review.stamp = stamp(&tx)?;
            saved.scan.report.stamp = saved.review.stamp.clone();
        }
        saved.review.revision = saved
            .review
            .revision
            .checked_add(1)
            .context("Duplicate review revision exhausted")?;
        cancel.check()?;
        tx.execute(
            "UPDATE maintenance_scans SET payload_json=?1 WHERE id=?2 AND kind='duplicate_review'",
            params![serde_json::to_string(&saved)?, id],
        )?;
        tx.commit()?;
        Ok(saved.review)
    }
}
