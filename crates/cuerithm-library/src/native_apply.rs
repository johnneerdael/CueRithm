//! The rekordbox synchronization flow: refresh observations from a snapshot, preview what would be written,
//! then write to a copy and publish it with a backup. Every step reads or writes a snapshot or a copy; the
//! live database changes only at the final, verified publication.
use crate::native_new_tracks::NewTrackSummary;
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
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const RUN: &str = "native_sync_run";

/// What a synchronization may do beyond refreshing tracks that are already linked.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct NativeSyncOptions {
    /// Also add CueRithm tracks that rekordbox does not have yet, with their grid, cues and analysis file.
    #[serde(default)]
    pub add_new_tracks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSyncPrepared {
    pub target_id: String,
    pub preview: NativeSyncPreview,
    pub plan: SyncPlanSummary,
    /// What mirroring CueRithm's playlists would do.
    pub playlists: PlaylistPlanSummary,
    /// CueRithm tracks rekordbox does not have: what adding them would do, and the ones that cannot be added.
    #[serde(default)]
    pub new_tracks: NewTrackSummary,
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
    #[serde(default)]
    pub tracks_added: usize,
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
        options: NativeSyncOptions,
        cancel: &CancellationToken,
    ) -> Result<NativeSyncPrepared> {
        let (prepared, ..) = self.prepare_with_snapshot(live, key, work_dir, options, cancel)?;
        Ok(prepared)
    }

    fn prepare_with_snapshot(
        &self,
        live: &Path,
        key: &DatabaseKey,
        work_dir: &Path,
        options: NativeSyncOptions,
        cancel: &CancellationToken,
    ) -> Result<(
        NativeSyncPrepared,
        cuerithm_rekordbox::DatabaseSnapshot,
        cuerithm_rekordbox::RekordboxCollection,
        Vec<cuerithm_rekordbox::NewTrack>,
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
        let (new_tracks, new_summary) = self.plan_native_new_tracks(&target, &collection)?;
        let adding = adding_ids(&new_tracks, options);
        let (_, _, playlists) = self.plan_native_playlists(&target, &collection, &adding)?;
        Ok((
            NativeSyncPrepared {
                target_id: target,
                preview,
                plan,
                playlists,
                new_tracks: new_summary,
                snapshot_usn: usn,
            },
            snapshot,
            collection,
            new_tracks,
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
        options: NativeSyncOptions,
        cancel: &CancellationToken,
    ) -> Result<NativeSyncRun> {
        let (prepared, snapshot, collection, new_tracks) =
            self.prepare_with_snapshot(live, key, work_dir, options, cancel)?;
        let (mut plan, summary) = plan_from_preview(&prepared.preview)?;
        let adding = adding_ids(&new_tracks, options);
        let (playlist_writes, playlist_removals, playlist_summary) =
            self.plan_native_playlists(&prepared.target_id, &collection, &adding)?;
        plan.playlists = playlist_writes;
        plan.remove_playlists = playlist_removals;
        let share = live.parent().map(|p| p.join("share"));
        if options.add_new_tracks && !new_tracks.is_empty() {
            plan.new_tracks = new_tracks;
            plan.share_dir = share.as_ref().map(|p| p.display().to_string());
        }
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
            tracks_added: 0,
            skipped: summary.skipped.len(),
            backup: None,
            journal: None,
        };
        if plan.tracks.is_empty()
            && plan.new_tracks.is_empty()
            && plan.playlists.is_empty()
            && plan.remove_playlists.is_empty()
        {
            self.record_native_run(&run)?;
            return Ok(run);
        }
        let report = {
            let mut writer = WriteDatabase::open(&snapshot.path, key)?;
            writer.apply(&plan, cancel)?
        };
        let published = publish_verified_copy(
            &PublishRequest {
                live,
                work: &snapshot.path,
                key,
                backup_dir,
                assets: &report.assets,
                share_dir: share.as_deref().filter(|_| !report.assets.is_empty()),
                base_usn: prepared.snapshot_usn,
            },
            cancel,
        )?;
        run.state = "committed".into();
        run.usn_before = Some(published.usn_before);
        run.usn_after = Some(published.usn_after);
        run.backup = Some(published.backup);
        run.journal = Some(published.journal);
        run.tracks_added = report.tracks_added;
        // Read the published library back, and only then treat the written fields as agreed. Tracks that were
        // just added are linked first, owned here, so the next synchronization recognises them.
        let mut written: Vec<(String, Vec<String>)> = summary
            .tracks
            .iter()
            .map(|t| (t.content_id.clone(), t.fields.clone()))
            .collect();
        if !report.new_tracks.is_empty() {
            self.link_added_tracks(live, key, work_dir, &report, cancel)?;
            let fields: Vec<String> = native_writable_fields()
                .into_iter()
                .map(str::to_owned)
                .collect();
            written.extend(
                report
                    .new_tracks
                    .iter()
                    .map(|n| (n.content_id.clone(), fields.clone())),
            );
        }
        let (_after, _snapshot, after_collection, _) =
            self.prepare_with_snapshot(live, key, work_dir, options, cancel)?;
        self.native_acknowledge(&prepared.target_id, &written)?;
        self.acknowledge_native_playlists(
            &prepared.target_id,
            &playlist_summary,
            &after_collection,
        )?;
        self.record_native_run(&run)?;
        Ok(run)
    }

    /// Import the tracks a write just added so each links to the CueRithm track it came from. CueRithm owns
    /// every writable field of them, since it supplied all of those values.
    fn link_added_tracks(
        &self,
        live: &Path,
        key: &DatabaseKey,
        work_dir: &Path,
        report: &cuerithm_rekordbox::WriteReport,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let snapshot = ReadOnlyDatabase::open(live, key)?.capture(work_dir, key, cancel)?;
        let collection = ReadOnlyDatabase::open(&snapshot.path, key)?
            .read_collection(CollectionReadOptions::default(), cancel)?;
        self.import_rekordbox_collection(
            &collection,
            NativeImportOptions {
                settings_revision: self.settings()?.revision,
                content_ids: Some(
                    report
                        .new_tracks
                        .iter()
                        .map(|n| n.content_id.clone())
                        .collect(),
                ),
                initial_owners: native_writable_fields()
                    .into_iter()
                    .map(|f| (f.to_owned(), NativeFieldOwner::Local))
                    .collect(),
            },
            cancel,
        )?;
        Ok(())
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

/// Local track ids that the plan adds, when adding was asked for.
fn adding_ids(
    new_tracks: &[cuerithm_rekordbox::NewTrack],
    options: NativeSyncOptions,
) -> BTreeSet<String> {
    if options.add_new_tracks {
        new_tracks.iter().map(|t| t.key.clone()).collect()
    } else {
        BTreeSet::new()
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
    /// Test support: add an analysed, ready track for a file, as the library would hold it after preparation.
    pub fn add_ready_track_for_test(
        &self,
        path: &Path,
        title: &str,
        artist: &str,
        bpm: f64,
        duration: f64,
        first_beat: f64,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let media = cuerithm_media::MediaInfo {
            codec: "pcm_s16le".into(),
            sample_rate: 44_100,
            channels: 2,
            bits_per_sample: Some(16),
            tags: Default::default(),
            properties: Default::default(),
        };
        let analysis = serde_json::json!({"analysis": {"tempo": {"downbeat_time": first_beat}}});
        self.conn()?.execute(
            "INSERT INTO tracks(id,path,title,artist,source_hash,state,bpm,duration,metadata_json,analysis_json,created,updated) VALUES(?1,?2,?3,?4,?5,'ready',?6,?7,?8,?9,?10,?10)",
            params![
                id,
                crate::model::path_bytes(path),
                title,
                artist,
                id,
                bpm,
                duration,
                serde_json::to_string(&media)?,
                analysis.to_string(),
                now()
            ],
        )?;
        Ok(id)
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
