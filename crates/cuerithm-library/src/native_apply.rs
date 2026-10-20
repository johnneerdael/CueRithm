//! The rekordbox synchronization flow: refresh observations from a snapshot, preview what would be written,
//! then write to a copy and publish it with a backup. Every step reads or writes a snapshot or a copy; the
//! live database changes only at the final, verified publication.
use crate::native_plan::{PlannedTrack, SyncPlanSummary, plan_from_preview};
use crate::native_playlists::PlaylistPlanSummary;
use crate::native_reconcile::NativeSyncPreview;
use crate::*;
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::{
    CollectionReadOptions, ConflictError, DatabaseKey, PublishRequest, ReadOnlyDatabase,
    WriteDatabase, publish_verified_copy,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const RUN: &str = "native_sync_run";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSyncPrepared {
    pub target_id: String,
    pub preview: NativeSyncPreview,
    pub plan: SyncPlanSummary,
    /// What mirroring CueRithm's playlists would do.
    pub playlists: PlaylistPlanSummary,
    /// The library's change number when the snapshot was taken.
    pub snapshot_usn: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSyncRun {
    pub id: String,
    pub created: i64,
    pub target_id: String,
    /// `committed` after a verified publication, `nothing_to_write` when the plan was empty.
    pub state: String,
    pub usn_before: Option<i64>,
    pub usn_after: Option<i64>,
    pub tracks: Vec<PlannedTrack>,
    pub playlists: PlaylistPlanSummary,
    pub skipped: usize,
    pub backup: Option<PathBuf>,
    pub journal: Option<PathBuf>,
}

/// A conflict found when writing, listed for the screen. Nothing was written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSyncConflict {
    pub subject: String,
    pub reason: String,
}

impl Library {
    /// Snapshot the live library, refresh what is known about already-linked tracks, and say what a sync would
    /// do. Writes only to this library's observation records.
    pub fn native_sync_prepare(
        &self,
        live: &Path,
        key: &DatabaseKey,
        work_dir: &Path,
        cancel: &CancellationToken,
    ) -> Result<NativeSyncPrepared> {
        let (prepared, _snapshot, _collection) =
            self.prepare_with_snapshot(live, key, work_dir, cancel)?;
        Ok(prepared)
    }

    fn prepare_with_snapshot(
        &self,
        live: &Path,
        key: &DatabaseKey,
        work_dir: &Path,
        cancel: &CancellationToken,
    ) -> Result<(
        NativeSyncPrepared,
        cuerithm_rekordbox::DatabaseSnapshot,
        cuerithm_rekordbox::RekordboxCollection,
    )> {
        let snapshot = ReadOnlyDatabase::open(live, key)?.capture(work_dir, key, cancel)?;
        let collection = ReadOnlyDatabase::open(&snapshot.path, key)?
            .read_collection(CollectionReadOptions::default(), cancel)?;
        let usn = collection
            .identity
            .local_update_count
            .context("The library has no local update counter")? as i64;
        let target = self
            .native_targets()?
            .into_iter()
            .find(|t| t.identity.database_id == collection.identity.database_id)
            .map(|t| t.id)
            .context("Import tracks from this rekordbox library before synchronizing")?;
        // Refresh only tracks that are already linked; synchronizing never imports new ones.
        let present: std::collections::BTreeSet<_> = collection
            .tracks
            .iter()
            .map(|t| t.identity.content_id.clone())
            .collect();
        let linked: std::collections::BTreeSet<_> = self
            .native_linked_content_ids(&target)?
            .into_iter()
            .filter(|id| present.contains(id))
            .collect();
        ensure!(!linked.is_empty(), "No linked tracks to synchronize yet");
        self.import_rekordbox_collection(
            &collection,
            NativeImportOptions {
                settings_revision: self.settings()?.revision,
                content_ids: Some(linked),
                initial_owners: Default::default(),
            },
            cancel,
        )?;
        let preview = self.native_sync_preview(&target)?;
        let (_, plan) = plan_from_preview(&preview)?;
        let (_, _, playlists) = self.plan_native_playlists(&target, &collection)?;
        Ok((
            NativeSyncPrepared {
                target_id: target,
                preview,
                plan,
                playlists,
                snapshot_usn: usn,
            },
            snapshot,
            collection,
        ))
    }

    /// Write every field Cuerithm owns and has changed to a copy of the library, then publish the copy with a
    /// backup. A conflict, an unverified result or a library that is in use leaves the live library untouched.
    pub fn native_sync_apply(
        &self,
        live: &Path,
        key: &DatabaseKey,
        work_dir: &Path,
        backup_dir: &Path,
        cancel: &CancellationToken,
    ) -> Result<NativeSyncRun> {
        let (prepared, snapshot, collection) =
            self.prepare_with_snapshot(live, key, work_dir, cancel)?;
        let (mut plan, summary) = plan_from_preview(&prepared.preview)?;
        let (playlist_writes, playlist_removals, playlist_summary) =
            self.plan_native_playlists(&prepared.target_id, &collection)?;
        plan.playlists = playlist_writes;
        plan.remove_playlists = playlist_removals;
        let id = uuid::Uuid::new_v4().to_string();
        let mut run = NativeSyncRun {
            id: id.clone(),
            created: now(),
            target_id: prepared.target_id.clone(),
            state: "nothing_to_write".into(),
            usn_before: None,
            usn_after: None,
            tracks: summary.tracks.clone(),
            playlists: playlist_summary.clone(),
            skipped: summary.skipped.len(),
            backup: None,
            journal: None,
        };
        if plan.tracks.is_empty() && plan.playlists.is_empty() && plan.remove_playlists.is_empty() {
            self.record_native_run(&run)?;
            return Ok(run);
        }
        {
            let mut writer = WriteDatabase::open(&snapshot.path, key)?;
            writer.apply(&plan, cancel)?;
        }
        let published = publish_verified_copy(
            &PublishRequest {
                live,
                work: &snapshot.path,
                key,
                backup_dir,
                base_usn: prepared.snapshot_usn,
            },
            cancel,
        )?;
        run.state = "committed".into();
        run.usn_before = Some(published.usn_before);
        run.usn_after = Some(published.usn_after);
        run.backup = Some(published.backup);
        run.journal = Some(published.journal);
        // Read the published library back, and only then treat the written fields as agreed.
        let (_after, _snapshot, after_collection) =
            self.prepare_with_snapshot(live, key, work_dir, cancel)?;
        let written: Vec<(String, Vec<String>)> = summary
            .tracks
            .iter()
            .map(|t| (t.content_id.clone(), t.fields.clone()))
            .collect();
        self.native_acknowledge(&prepared.target_id, &written)?;
        self.acknowledge_native_playlists(
            &prepared.target_id,
            &playlist_summary,
            &after_collection,
        )?;
        self.record_native_run(&run)?;
        Ok(run)
    }

    fn record_native_run(&self, run: &NativeSyncRun) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO maintenance_scans VALUES(?1,?2,?3,?4)",
            params![run.id, RUN, serde_json::to_string(run)?, run.created],
        )?;
        Ok(())
    }

    /// Past synchronizations, newest first.
    pub fn native_sync_history(&self) -> Result<Vec<NativeSyncRun>> {
        let rows = {
            let conn = self.conn()?;
            let mut q = conn.prepare(
                "SELECT payload_json FROM maintenance_scans WHERE kind=?1 ORDER BY created DESC,rowid DESC",
            )?;
            q.query_map([RUN], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        rows.into_iter()
            .map(|text| Ok(serde_json::from_str(&text)?))
            .collect()
    }
}

/// Conflicts carried by a failed apply, for display.
pub fn native_sync_conflicts(error: &anyhow::Error) -> Option<Vec<NativeSyncConflict>> {
    error.downcast_ref::<ConflictError>().map(|e| {
        e.0.iter()
            .map(|c| NativeSyncConflict {
                subject: c.subject.clone(),
                reason: c.reason.clone(),
            })
            .collect()
    })
}

#[doc(hidden)]
impl Library {
    /// Test support: give a track a custom tag, creating the category and tag if needed.
    pub fn add_track_tag_for_test(&self, track_id: &str, category: &str, tag: &str) -> Result<()> {
        let conn = self.conn()?;
        let category_id = format!("category-{category}");
        let tag_id = format!("tag-{category}-{tag}");
        conn.execute(
            "INSERT OR IGNORE INTO custom_tag_categories(id,name) VALUES(?1,?2)",
            params![category_id, category],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO custom_tags(id,category_id,name) VALUES(?1,?2,?3)",
            params![tag_id, category_id, tag],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO track_custom_tags(track_id,tag_id) VALUES(?1,?2)",
            params![track_id, tag_id],
        )?;
        Ok(())
    }
    /// Test support: set a track's title and genre as a local edit would.
    pub fn set_track_text_for_test(&self, track_id: &str, title: &str, genre: &str) -> Result<()> {
        ensure!(
            self.conn()?.execute(
                "UPDATE tracks SET title=?1,genre=?2 WHERE id=?3",
                params![title, genre, track_id]
            )? == 1,
            "Unknown track"
        );
        Ok(())
    }
}
