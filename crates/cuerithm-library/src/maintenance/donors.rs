//! One recoverable file/SQLite publication for duplicate donor, archive and reference work.
use super::{
    deletion,
    donor_state::{self, TrackImage},
    files::{current_state, ensure_available},
    merge::{self, SavedPreview},
    purge, track_removal, undo_window, *,
};
use crate::{
    Library,
    artifacts::{Artifact, cleanup_temporaries, current_hash, owned_file, rollback, safe_target},
    model::{bytes_path, path_bytes},
    publish_files,
    record_snapshot::{RecordImage, RestoreReport},
};
use anyhow::{Context, Result, ensure};
use cuerithm_autotag::{
    AnalysisTags, AutotagConfig, MatchStatus, StagedTagWrite, TagInput, TagMatchOutcome,
    TagWritePlan, TransferPatch, TransferSnapshot,
};
use cuerithm_maintenance::{MergeDisposition, MergePlan};
use cuerithm_media::{CancellationToken, MediaRuntime, file_sha256_cancellable};
use rusqlite::{TransactionBehavior, params};
use same_file::Handle;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
};

pub struct DuplicateMergeService {
    lib: Library,
    runtime: MediaRuntime,
    trash: Arc<dyn FileTrash>,
    #[cfg(test)]
    hook: Option<fn(&str)>,
}
/// How a verified duplicate file left its folder.
enum Disposal {
    Trashed,
    Permanent,
}
/// A file this operation intended to move to the Trash, kept in the journal so an
/// interrupted operation can say which files may already be there.
#[derive(Clone, Serialize, Deserialize)]
struct TrashIntent {
    track_id: String,
    path: Vec<u8>,
}
struct StagedImport {
    target: String,
    donor: String,
    record: cuerithm_maintenance::ImportRecord,
    native: TransferSnapshot,
    staged: StagedTagWrite,
}
#[derive(Serialize, Deserialize)]
struct Payload {
    schema: String,
    preview_id: String,
    kind: String,
    undo_of: Option<String>,
    result: DuplicateMergeResult,
    before: BTreeMap<String, TrackImage>,
    after: BTreeMap<String, TrackImage>,
    playlists: Vec<DuplicatePlaylistChange>,
    epochs: BTreeMap<String, u64>,
    undone_by: Option<String>,
    /// Library records removed together with their files; the minimum Undo needs. Records
    /// saved in a format this build does not read are treated as no longer stored.
    #[serde(default, deserialize_with = "saved_records")]
    removed: Option<RecordImage>,
    #[serde(default)]
    trash_intent: Vec<TrashIntent>,
    /// The removed records are no longer kept, so the removal is final.
    #[serde(default)]
    undo_expired: bool,
}
impl Payload {
    /// Drop what only Undo of a removal needs, keeping the summary in `result`.
    fn forget_removed(&mut self) {
        let removed = self.result.removed_ids.clone();
        self.removed = None;
        self.before.retain(|id, _| !removed.contains(id));
    }
}
fn saved_records<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<RecordImage>, D::Error> {
    let value = <Option<serde_json::Value> as Deserialize>::deserialize(deserializer)?;
    Ok(value.and_then(|value| serde_json::from_value(value).ok()))
}
/// A merge that fell out of the shared Undo window (see [`undo_window`]): the stored records
/// are dropped and Undo is no longer offered. The summary stays in `result`.
pub(super) fn drop_saved_records(conn: &rusqlite::Connection, id: &str) -> Result<()> {
    let text: String = conn.query_row(
        "SELECT payload_json FROM maintenance_writes WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    let mut payload: Payload = serde_json::from_str(&text)?;
    payload.forget_removed();
    payload.undo_expired = true;
    conn.execute(
        "UPDATE maintenance_writes SET payload_json=?1 WHERE id=?2",
        params![serde_json::to_string(&payload)?, id],
    )?;
    Ok(())
}
/// An interrupted operation changed no library record, but it may already have moved files to
/// the Trash. Say which, so the user can find them.
fn interruption(write: &Write, base: &str) -> anyhow::Error {
    let gone = write
        .payload
        .trash_intent
        .iter()
        .filter_map(|item| bytes_path(&item.path).ok())
        .filter(|path| !path.exists())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if gone.is_empty() {
        anyhow::anyhow!("{base}")
    } else {
        anyhow::anyhow!(
            "{base}; these files are no longer at their original location and may be in the Trash or deleted, and their library records were kept: {}",
            gone.join(", ")
        )
    }
}
struct Write {
    id: String,
    state: String,
    payload: Payload,
    files: Vec<(Artifact, PathBuf)>,
}
fn load(lib: &Library, id: &str) -> Result<Write> {
    let conn = lib.conn()?;
    let (state, text): (String, String) = conn.query_row(
        "SELECT state,payload_json FROM maintenance_writes WHERE id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let mut q=conn.prepare("SELECT target,staged,backup,before_hash,after_hash,publish_path,displaced_path,workspace FROM maintenance_write_files WHERE write_id=?1 ORDER BY ordinal")?;
    let rows = q
        .query_map([id], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, Option<Vec<u8>>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Vec<u8>>(5)?,
                r.get::<_, Vec<u8>>(6)?,
                r.get::<_, Vec<u8>>(7)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let files: Vec<(Artifact, PathBuf)> = rows
        .into_iter()
        .map(
            |(target, staged, backup, before, after, publish, displaced, workspace)| {
                Ok((
                    Artifact {
                        target: bytes_path(&target)?,
                        staged: bytes_path(&staged)?,
                        backup: backup.map(|p| bytes_path(&p)).transpose()?,
                        before,
                        after,
                        publish: bytes_path(&publish)?,
                        displaced: bytes_path(&displaced)?,
                    },
                    bytes_path(&workspace)?,
                ))
            },
        )
        .collect::<Result<_>>()?;
    let payload: Payload = serde_json::from_str(&text)?;
    let allowed = payload
        .before
        .values()
        .map(|image| image.row.path().map(|p| path_bytes(&p)))
        .collect::<Result<BTreeSet<_>>>()?;
    let root = lib.data_dir().canonicalize()?;
    for (file, workspace) in &files {
        ensure!(
            allowed.contains(&path_bytes(&file.target)),
            "Recovery target does not match its track snapshot"
        );
        let staged = super::files::resolved_prefix(&file.staged)?;
        let backup = file
            .backup
            .as_ref()
            .map(|p| super::files::resolved_prefix(p))
            .transpose()?;
        ensure!(
            workspace.canonicalize()?.starts_with(&root)
                && staged.starts_with(workspace)
                && backup.as_ref().is_some_and(|p| p.starts_with(workspace)),
            "Recovery assets leave application data"
        );
        ensure!(
            file.publish.parent() == file.target.parent()
                && file.displaced.parent() == file.target.parent(),
            "Recovery temporary leaves source folder"
        );
    }

    ensure!(
        payload.schema == "duplicate-file-merge-v2",
        "Unsupported duplicate file journal"
    );
    Ok(Write {
        id: id.into(),
        state,
        payload,
        files,
    })
}
fn reserve(
    lib: &Library,
    id: &str,
    payload: &Payload,
    files: &[(Artifact, PathBuf)],
    expected: &MaintenanceStamp,
    cancel: &CancellationToken,
) -> Result<()> {
    let root = lib.data_dir().canonicalize()?;
    let allowed = payload
        .before
        .values()
        .map(|image| image.row.path().map(|p| path_bytes(&p)))
        .collect::<Result<BTreeSet<_>>>()?;
    for (file, workspace) in files {
        ensure!(
            allowed.contains(&path_bytes(&file.target)),
            "Publication target no longer matches its track snapshot"
        );
        ensure!(
            workspace.canonicalize()?.starts_with(&root),
            "Duplicate staging leaves application data"
        );
        safe_target(&file.target, &file.target)?;
        owned_file(&file.staged, workspace, &file.after)?;
        let backup = file
            .backup
            .as_ref()
            .context("Missing original duplicate backup")?;
        owned_file(
            backup,
            workspace,
            file.before.as_deref().context("Missing source hash")?,
        )?;
        publish_files::durable_assets(&[file.staged.clone(), backup.clone()], workspace, &root)?;
    }
    let mut conn = lib.conn()?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    ensure!(
        super::scan::stamp(&tx)? == *expected,
        "Library changed while staging duplicate work"
    );
    for file in files.iter().map(|(f, _)| f) {
        crate::maintenance::ensure_unused_unreserved(&tx, &[path_bytes(&file.target)])?;
        let busy:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM tag_write_files f JOIN tag_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state='prepared')+(SELECT COUNT(*) FROM cleanup_write_files f JOIN cleanup_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state='prepared')+(SELECT COUNT(*) FROM maintenance_write_files f JOIN maintenance_writes w ON w.id=f.write_id WHERE f.target=?1 AND w.state IN ('prepared','conflict'))",[path_bytes(&file.target)],|r|r.get(0))?;
        ensure!(busy == 0, "Metadata destination is reserved");
        ensure!(
            current_hash(&file.target)? == file.before,
            "Duplicate source changed during staging"
        );
    }
    cancel.check()?;
    tx.execute("INSERT INTO maintenance_writes(id,kind,state,payload_json,created) VALUES(?1,?2,'prepared',?3,?4)",params![id,payload.kind,serde_json::to_string(payload)?,crate::now()])?;
    for (n, (f, workspace)) in files.iter().enumerate() {
        tx.execute(
            "INSERT INTO maintenance_write_files VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                id,
                n as u64,
                path_bytes(&f.target),
                path_bytes(&f.staged),
                f.backup.as_ref().map(|p| path_bytes(p)),
                f.before,
                f.after,
                path_bytes(&f.publish),
                path_bytes(&f.displaced),
                path_bytes(workspace)
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}
fn install(files: &[(Artifact, PathBuf)], cancel: &CancellationToken) -> Result<()> {
    for (file, workspace) in files {
        cancel.check()?;
        safe_target(&file.target, &file.target)?;
        owned_file(&file.staged, workspace, &file.after)?;
        ensure!(
            current_hash(&file.target)? == file.before,
            "Duplicate source changed before publication"
        );
        if file.before.as_ref() == Some(&file.after) {
            continue;
        }
        ensure!(
            !std::fs::metadata(&file.target)?.permissions().readonly(),
            "Duplicate target is read only"
        );
        publish_files::copy_prepared(&file.staged, &file.publish, &file.after)?;
        publish_files::replace(&file.publish, &file.target, &file.displaced)?;
        ensure!(
            current_hash(&file.displaced)? == file.before,
            "Source changed during duplicate exchange"
        );
        ensure!(
            current_hash(&file.target)?.as_deref() == Some(file.after.as_str()),
            "Published duplicate metadata changed"
        );
    }
    Ok(())
}
fn finish(
    lib: &Library,
    id: &str,
    files: &[(Artifact, PathBuf)],
    error: Option<&anyhow::Error>,
) -> Result<()> {
    let artifacts = files.iter().map(|(f, _)| f.clone()).collect::<Vec<_>>();
    if let Some(error) = error {
        let recovered = artifacts
            .iter()
            .try_for_each(|f| safe_target(&f.target, &f.target))
            .and_then(|_| rollback(&artifacts));
        let message = recovered
            .as_ref()
            .err()
            .map(|e| format!("{error:#}; recovery needs attention: {e:#}"))
            .unwrap_or_else(|| format!("{error:#}"));
        lib.conn()?.execute(
            "UPDATE maintenance_writes SET state=?1,error=?2 WHERE id=?3",
            params![
                if recovered.is_ok() {
                    "rolled_back"
                } else {
                    "conflict"
                },
                message,
                id
            ],
        )?;
        if recovered.is_err() {
            return Ok(());
        }
    }
    match cleanup_temporaries(&artifacts, lib.data_dir()) {
        Ok(()) => {
            lib.conn()?.execute(
                "UPDATE maintenance_writes SET cleanup_complete=1 WHERE id=?1",
                [id],
            )?;
        }
        Err(error) => {
            lib.conn()?.execute(
                "UPDATE maintenance_writes SET error=?1 WHERE id=?2",
                params![format!("Recovery cleanup pending: {error:#}"), id],
            )?;
        }
    }
    Ok(())
}
fn result(
    id: &str,
    preview: &DuplicateMergePreview,
    undo_of: Option<String>,
) -> DuplicateMergeResult {
    DuplicateMergeResult {
        id: id.into(),
        preview_id: preview.id.clone(),
        stamp: preview.stamp.clone(),
        archived_ids: vec![],
        restored_ids: vec![],
        affected_playlists: preview.playlists.iter().map(|p| p.id.clone()).collect(),
        undo_of,
        updated_ids: vec![],
        removed_ids: vec![],
        trashed: vec![],
        permanently_deleted: vec![],
        not_deleted: vec![],
        links_not_restored: 0,
    }
}
impl DuplicateMergeService {
    pub fn new(lib: Library, runtime: MediaRuntime) -> Self {
        Self::with_trash(lib, runtime, Arc::new(SystemTrash))
    }
    /// Use another Trash, which lets tests keep real files out of the user's Trash.
    pub fn with_trash(lib: Library, runtime: MediaRuntime, trash: Arc<dyn FileTrash>) -> Self {
        Self {
            lib,
            runtime,
            trash,
            #[cfg(test)]
            hook: None,
        }
    }
    pub fn apply(&self, id: &str, cancel: &CancellationToken) -> Result<DuplicateMergeResult> {
        self.apply_confirmed(id, false, cancel)
    }
    /// Apply a reviewed preview. A preview that moves audio files to the Trash is applied only
    /// with `confirm_deletion`, and only the files it listed (and that still pass every check)
    /// are moved.
    pub fn apply_confirmed(
        &self,
        id: &str,
        confirm_deletion: bool,
        cancel: &CancellationToken,
    ) -> Result<DuplicateMergeResult> {
        cancel.check()?;
        let mut preview: SavedPreview = {
            let conn = self.lib.conn()?;
            merge::load(&conn, id, "duplicate_merge_plan")?
        };
        ensure!(
            preview.consumed_by.is_none(),
            "Duplicate preview already applied"
        );
        let deleting = preview.public.plan.disposition == MergeDisposition::Delete;
        ensure!(
            !deleting || confirm_deletion,
            "Confirm deleting the listed files before applying"
        );
        super::review::refresh(&self.lib, &mut preview.review, cancel)?;
        // Files can vanish between the scan and now; say which before the generic stale refusal.
        ensure_available(&*self.lib.conn()?, &preview.public.plan.participants())?;
        ensure!(
            !preview.review.review.stale,
            "Duplicate preview is stale; scan again"
        );
        // Check again, now, every file the preview listed. What is applied can only be a
        // subset of what the user reviewed.
        let mut plan = preview.public.plan.clone();
        let (mut doomed, mut not_deleted) = if deleting {
            self.reverify(&preview, &mut plan, cancel)?
        } else {
            (vec![], vec![])
        };
        let ids = plan.participants();
        let snapshots = {
            let mut conn = self.lib.conn()?;
            let tx = conn.transaction()?;
            ensure!(
                super::scan::stamp(&tx)? == preview.public.stamp,
                "Library changed; scan again"
            );
            let value = ids
                .iter()
                .map(|id| Ok((id.clone(), TrackImage::capture(&tx, id)?)))
                .collect::<Result<BTreeMap<_, _>>>()?;
            tx.commit()?;
            value
        };
        let cache = self.lib.data_dir().join("maintenance/staging");
        let mut staged = vec![];
        for copy in plan.groups.iter().flat_map(|g| &g.copies) {
            cancel.check()?;
            let source = &snapshots[&copy.from];
            let target = &snapshots[&copy.to];
            ensure!(
                source.row.text("state")? == Some("ready")
                    && target.row.text("state")? == Some("ready"),
                "Prepare incoming tracks before merging"
            );
            let source_path = source.row.path()?;
            let target_path = target.row.path()?;
            for (other_id, other) in &snapshots {
                if other_id != &copy.to {
                    ensure!(
                        other.row.path()?.canonicalize().ok().as_ref()
                            != Some(&target_path.canonicalize()?),
                        "Duplicate records alias the same physical path; file transfer needs review"
                    );
                }
            }
            let source_info = self.runtime.probe_cancellable(&source_path, cancel)?;
            let donor_native =
                cuerithm_autotag::read_transfer_metadata(&source_path, &source_info)?;
            ensure!(
                current_hash(&source_path)?.as_deref() == Some(source.row.hash()?),
                "Donor source changed; analyze again"
            );
            let audio = self
                .runtime
                .decode_cancellable(&target_path, &cache, cancel)?;
            ensure!(
                audio.source_sha256 == target.row.hash()?,
                "Recipient source changed; analyze again"
            );
            let target_native =
                cuerithm_autotag::read_transfer_metadata(&audio.snapshot_path, &audio.info)?;
            let imported = cuerithm_maintenance::import_record(
                &target.record(&target_native)?,
                &source.record(&donor_native)?,
            )?;
            let native =
                donor_state::native_after(&target_native, &donor_native, source, &imported)?;
            let mut cfg = AutotagConfig::default();
            cfg.options.tags.clear();
            cfg.write_energy = false;
            let input = TagInput {
                path: target_path,
                source_sha256: audio.source_sha256.clone(),
                media: audio.info.clone(),
                duration_seconds: audio.clock.duration_seconds(),
            };
            let outcome = TagMatchOutcome {
                status: MatchStatus::NoMatch,
                matches: vec![],
                attempted: vec![],
                warnings: vec![],
            };
            let write = TagWritePlan::build(input, &outcome, &cfg, &AnalysisTags::default())?
                .with_transfer(TransferPatch {
                    before: target_native,
                    after: native.clone(),
                })?
                .stage_prepared_cancellable(&self.runtime, &audio, &cache, cancel)?;
            cancel.check()?;
            std::fs::set_permissions(
                &write.backup_path,
                std::fs::metadata(&audio.source_path)?.permissions(),
            )?;
            staged.push(StagedImport {
                target: copy.to.clone(),
                donor: copy.from.clone(),
                record: imported,
                native,
                staged: write,
            });
        }
        super::review::refresh(&self.lib, &mut preview.review, cancel)?;
        ensure!(
            !preview.review.review.stale,
            "Duplicate inputs changed while staging"
        );
        let _gate = self
            .lib
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let id_op = uuid::Uuid::new_v4().to_string();
        let mut files = vec![];
        for (index, item) in staged.iter().enumerate() {
            let parent = item
                .staged
                .source_path
                .parent()
                .context("Missing source folder")?;
            let publish = parent.join(format!(".cuerithm-duplicate-{id_op}-{index}.tmp"));
            let displaced = if cfg!(windows) {
                publish.with_extension("before.tmp")
            } else {
                publish.clone()
            };
            files.push((
                Artifact {
                    target: item.staged.source_path.clone(),
                    staged: item.staged.path.clone(),
                    backup: Some(item.staged.backup_path.clone()),
                    before: Some(item.staged.source_before_sha256.clone()),
                    after: item.staged.source_after_sha256.clone(),
                    publish,
                    displaced,
                },
                item.staged.workspace().canonicalize()?,
            ));
        }
        let changed = preview
            .archives
            .iter()
            .map(|a| a.id.clone())
            .chain(staged.iter().map(|s| s.target.clone()))
            .chain(doomed.iter().map(|f| f.track_id.clone()))
            .collect::<BTreeSet<_>>();
        let before = changed
            .iter()
            .map(|id| (id.clone(), snapshots[id].clone()))
            .collect();
        let mut payload = Payload {
            schema: "duplicate-file-merge-v2".into(),
            preview_id: id.into(),
            kind: "apply".into(),
            undo_of: None,
            result: result(&id_op, &preview.public, None),
            before,
            after: BTreeMap::new(),
            playlists: preview.public.playlists.clone(),
            epochs: BTreeMap::new(),
            undone_by: None,
            removed: None,
            trash_intent: doomed
                .iter()
                .map(|f| {
                    Ok(TrashIntent {
                        track_id: f.track_id.clone(),
                        path: path_bytes(&snapshots[&f.track_id].row.path()?),
                    })
                })
                .collect::<Result<_>>()?,
            undo_expired: false,
        };
        reserve(
            &self.lib,
            &id_op,
            &payload,
            &files,
            &preview.public.stamp,
            cancel,
        )?;
        for item in &mut staged {
            item.staged.retain_for_journal();
        }
        #[cfg(test)]
        if let Some(hook) = self.hook {
            hook("prepared");
        }
        let mut trashed_log: Vec<String> = vec![];
        let mut permanent: Vec<String> = vec![];
        let mut orphans = None;
        let outcome = (|| -> Result<()> {
            let mut conn = self.lib.conn()?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            ensure!(
                super::scan::stamp(&tx)? == preview.public.stamp,
                "Library changed before duplicate publication"
            );
            merge::check_review(&tx, &preview.public)?;
            ensure_available(&tx, &ids)?;
            let mut current: SavedPreview = merge::load(&tx, id, "duplicate_merge_plan")?;
            ensure!(
                current.consumed_by.is_none(),
                "Duplicate preview already applied"
            );
            ensure!(
                preview.review.scan.scope.time_current(crate::now()),
                "Smart scope expired; scan again"
            );
            for (id, before) in &snapshots {
                ensure!(
                    TrackImage::capture(&tx, id)? == *before,
                    "Duplicate track changed before publication"
                );
            }
            ensure!(
                merge::playlist_changes(&tx, &preview.public.plan)? == preview.public.playlists,
                "Playlist references changed"
            );
            install(&files, cancel)?;
            #[cfg(test)]
            if let Some(hook) = self.hook {
                hook("files");
            }
            let targets = files
                .iter()
                .map(|(f, _)| path_bytes(&f.target))
                .collect::<BTreeSet<_>>();
            let protected = preview
                .review
                .scan
                .files
                .iter()
                .zip(&preview.review.scan.report.tracks)
                .filter(|(f, track)| ids.contains(&track.id) && !targets.contains(&f.path))
                .map(|(file, _)| file.clone())
                .collect::<Vec<_>>();
            ensure!(
                super::files::proofs_current(&protected, cancel)?,
                "Donor or scope source changed during publication"
            );
            // Move the verified duplicates to the Trash. A record is removed below only for a
            // file that really is in the Trash; any other file stays where it is, with its
            // record, and is reported.
            if deleting {
                #[cfg(test)]
                if let Some(hook) = self.hook {
                    hook("trash");
                }
                cancel.check()?;
                let mut moved = vec![];
                for file in std::mem::take(&mut doomed) {
                    match self.dispose_verified(&file, &snapshots, &plan) {
                        Ok(how) => {
                            trashed_log.push(file.display_path.clone());
                            if matches!(how, Disposal::Permanent) {
                                permanent.push(file.track_id.clone());
                            }
                            moved.push(file);
                        }
                        Err((reason, detail)) => not_deleted.push(DuplicateDeletionSkip {
                            track_id: file.track_id.clone(),
                            display_path: file.display_path.clone(),
                            keeper_id: Some(file.keeper_id.clone()),
                            reason,
                            detail: Some(detail),
                        }),
                    }
                }
                doomed = moved;
                for group in &mut plan.groups {
                    group
                        .delete_ids
                        .retain(|id| doomed.iter().any(|f| &f.track_id == id));
                }
            }
            for item in &staged {
                donor_state::write_import(
                    &tx,
                    &snapshots[&item.target],
                    &item.record,
                    &item.native,
                    &item.staged.metadata,
                    &item.staged.source_after_sha256,
                    &item.donor,
                )?;
            }
            for archive in &preview.archives {
                tx.execute(
                    "UPDATE tracks SET archived=?1,updated=?2 WHERE id=?3",
                    params![archive.after, crate::now(), archive.id],
                )?;
                if archive.after {
                    payload.result.archived_ids.push(archive.id.clone());
                } else {
                    payload.result.restored_ids.push(archive.id.clone());
                }
            }
            let playlists = if deleting {
                merge::playlist_changes(&tx, &plan)?
            } else {
                preview.public.playlists.clone()
            };
            merge::write_members(&tx, &playlists, false)?;
            payload.result.affected_playlists = playlists.iter().map(|p| p.id.clone()).collect();
            payload.playlists = playlists;
            let removed_ids = doomed
                .iter()
                .map(|f| f.track_id.clone())
                .collect::<BTreeSet<_>>();
            if !removed_ids.is_empty() {
                let removed =
                    track_removal::remove(&tx, &removed_ids.iter().cloned().collect::<Vec<_>>())?;
                payload.removed = Some(removed.image);
                orphans = Some(removed.orphans);
            }
            for id in changed.iter().filter(|id| !removed_ids.contains(*id)) {
                payload
                    .after
                    .insert(id.clone(), TrackImage::capture(&tx, id)?);
            }
            payload.epochs = donor_state::epochs(
                &tx,
                &payload.after.keys().cloned().collect::<Vec<_>>(),
                &payload.result.affected_playlists,
            )?;
            payload.result.stamp = merge::advance(&tx)?;
            payload.result.updated_ids = staged.iter().map(|s| s.target.clone()).collect();
            payload.result.removed_ids = removed_ids.iter().cloned().collect();
            payload.result.trashed = doomed
                .iter()
                .filter(|f| !permanent.contains(&f.track_id))
                .cloned()
                .collect();
            payload.result.permanently_deleted = doomed
                .iter()
                .filter(|f| permanent.contains(&f.track_id))
                .cloned()
                .collect();
            payload.result.not_deleted = not_deleted.clone();
            current.consumed_by = Some(id_op.clone());
            // Once a file has gone to the Trash the library must record it, so a late
            // cancellation is ignored from here on.
            if trashed_log.is_empty() {
                cancel.check()?;
            }
            tx.execute(
                "UPDATE maintenance_writes SET state='committed',payload_json=?1 WHERE id=?2",
                params![serde_json::to_string(&payload)?, id_op],
            )?;
            tx.execute(
                "UPDATE maintenance_scans SET payload_json=?1 WHERE id=?2",
                params![serde_json::to_string(&current)?, id],
            )?;
            if payload.removed.is_some() {
                undo_window::enforce(&tx)?;
            }
            tx.commit()?;
            Ok(())
        })();
        let reported = outcome.as_ref().err().map(|error| {
            if trashed_log.is_empty() {
                anyhow::anyhow!("{error:#}")
            } else {
                anyhow::anyhow!(
                    "{error:#}; {} file(s) were already moved to the Trash or deleted permanently and their library records were kept: {}",
                    trashed_log.len(),
                    trashed_log.join(", ")
                )
            }
        });
        finish(&self.lib, &id_op, &files, reported.as_ref())?;
        outcome?;
        // Cover art and waveforms of content that is gone; rebuilt on demand if it returns.
        if let Some(orphans) = &orphans {
            purge::cache_files(
                self.lib.data_dir(),
                orphans,
                std::slice::from_ref(&self.runtime.identity),
            );
        }
        Ok(payload.result)
    }
    /// Undo of a removal only goes ahead when the removed tracks' identities and locations are
    /// still free, and nothing else sits where a removed track's file was: a file there must
    /// be the very file the record describes (for example restored from the Trash). Nothing
    /// is changed when this fails.
    fn verify_removed_world(&self, image: &RecordImage, cancel: &CancellationToken) -> Result<()> {
        track_removal::verify_world(&*self.lib.conn()?, image, cancel)
    }
    /// Check the files the preview listed once more and drop any that no longer pass.
    fn reverify(
        &self,
        preview: &SavedPreview,
        plan: &mut MergePlan,
        cancel: &CancellationToken,
    ) -> Result<(Vec<DuplicateDeletionFile>, Vec<DuplicateDeletionSkip>)> {
        let shown = preview
            .public
            .deletion
            .as_ref()
            .context("This preview lists no files to delete")?;
        let now = deletion::plan(&self.lib, &preview.review.scan, plan, &*self.trash, cancel)?;
        let mut files = vec![];
        let mut skipped = shown.skipped.clone();
        skipped.extend(now.skipped);
        for mut file in now.files {
            let listed = shown.files.iter().find(|f| {
                f.track_id == file.track_id
                    && f.sha256 == file.sha256
                    && f.identity == file.identity
                    && f.keeper_id == file.keeper_id
            });
            match listed {
                // What the user confirmed stands. A file confirmed as "permanent" stays so
                // even if the Trash looks usable now (it is still tried first).
                Some(shown) if shown.permanent || !file.permanent => {
                    file.permanent = shown.permanent;
                    files.push(file);
                }
                Some(_) => skipped.push(DuplicateDeletionSkip {
                    track_id: file.track_id.clone(),
                    display_path: file.display_path.clone(),
                    keeper_id: Some(file.keeper_id.clone()),
                    reason: DuplicateSkipReason::TrashFailed,
                    detail: Some(
                        "The Trash can no longer take this file. Preview again to see it listed for permanent deletion."
                            .into(),
                    ),
                }),
                None => skipped.push(DuplicateDeletionSkip {
                    track_id: file.track_id.clone(),
                    display_path: file.display_path.clone(),
                    keeper_id: Some(file.keeper_id.clone()),
                    reason: DuplicateSkipReason::FileChanged,
                    detail: Some("The file is not the one the preview listed.".into()),
                }),
            }
        }
        for group in &mut plan.groups {
            group
                .delete_ids
                .retain(|id| files.iter().any(|f| &f.track_id == id));
        }
        Ok((files, skipped))
    }
    /// The last look before a file leaves: it is still the file that was verified, and it is
    /// not the kept track's file. Then it goes to the Trash. Only a file the user was told
    /// would be deleted permanently is deleted permanently, and only after the Trash has
    /// refused it; any other file the Trash refuses stays exactly where it is.
    fn dispose_verified(
        &self,
        file: &DuplicateDeletionFile,
        snapshots: &BTreeMap<String, TrackImage>,
        plan: &MergePlan,
    ) -> std::result::Result<Disposal, (DuplicateSkipReason, String)> {
        let unavailable = |detail: &str| (DuplicateSkipReason::FileUnavailable, detail.to_owned());
        let path = snapshots
            .get(&file.track_id)
            .and_then(|image| image.row.path().ok())
            .ok_or_else(|| unavailable("The track is no longer part of this merge."))?;
        let keeper = plan
            .groups
            .iter()
            .find(|g| g.delete_ids.contains(&file.track_id))
            .map(|g| g.keeper.clone())
            .filter(|keeper| keeper == &file.keeper_id)
            .and_then(|keeper| snapshots.get(&keeper))
            .and_then(|image| image.row.path().ok())
            .ok_or_else(|| unavailable("The kept track is no longer part of this merge."))?;
        if !current_state(&path).is_available() {
            return Err(unavailable("The file is no longer available."));
        }
        if !current_state(&keeper).is_available() {
            return Err(unavailable("The kept track's file is no longer available."));
        }
        match (Handle::from_path(&path), Handle::from_path(&keeper)) {
            (Ok(a), Ok(b)) if a != b => {}
            _ => {
                return Err((
                    DuplicateSkipReason::SameFileAsKeeper,
                    "This is the kept track's file.".into(),
                ));
            }
        }
        match file_sha256_cancellable(&path, &CancellationToken::default()) {
            Ok(hash) if hash == file.sha256 => {}
            _ => {
                return Err((
                    DuplicateSkipReason::FileChanged,
                    "The file changed after it was verified.".into(),
                ));
            }
        }
        match self.trash.move_to_trash(&path) {
            Ok(()) => Ok(Disposal::Trashed),
            Err(reason) if file.permanent => self
                .trash
                .delete_permanently(&path)
                .map(|()| Disposal::Permanent)
                .map_err(|error| {
                    (
                        DuplicateSkipReason::TrashFailed,
                        format!("{reason}; permanent deletion also failed: {error}"),
                    )
                }),
            Err(reason) => Err((DuplicateSkipReason::TrashFailed, reason)),
        }
    }
    pub fn undo(&self, id: &str, cancel: &CancellationToken) -> Result<DuplicateMergeResult> {
        cancel.check()?;
        let found: bool = self.lib.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM maintenance_writes WHERE id=?1)",
            [id],
            |r| r.get(0),
        )?;
        if !found {
            return self.lib.undo_duplicate_merge(id, cancel);
        }
        let original = load(&self.lib, id)?;
        ensure!(
            original.state == "committed"
                && original.payload.kind == "apply"
                && original.payload.undone_by.is_none(),
            "Duplicate operation is not available for Undo"
        );
        ensure!(
            !original.payload.undo_expired,
            "Undo is no longer available: the removed library records were kept only for the last {REMOVAL_UNDO_LIMIT} removals (duplicate merges that delete files and Missing & broken removals count together)"
        );
        ensure!(
            original.payload.removed.is_some() == !original.payload.result.removed_ids.is_empty(),
            "The records removed by this merge are no longer stored; Undo needs review"
        );
        let removed_ids = original
            .payload
            .result
            .removed_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        // Before anything is staged or reserved, check that the world is as the removal left it.
        if let Some(image) = &original.payload.removed {
            self.verify_removed_world(image, cancel)?;
        }
        let tracks = original.payload.after.keys().cloned().collect::<Vec<_>>();
        let playlists = original.payload.result.affected_playlists.clone();
        let stamp = {
            let conn = self.lib.conn()?;
            ensure!(
                donor_state::epochs(&conn, &tracks, &playlists)? == original.payload.epochs,
                "Later relevant edits prevent Undo"
            );
            for (id, after) in &original.payload.after {
                ensure!(
                    TrackImage::capture(&conn, id)? == *after,
                    "Later track edits prevent Undo"
                );
            }
            super::scan::stamp(&conn)?
        };
        let work = tempfile::Builder::new().prefix("undo-").tempdir_in({
            let p = self.lib.data_dir().join("maintenance/staging");
            std::fs::create_dir_all(&p)?;
            p
        })?;
        let undo_id = uuid::Uuid::new_v4().to_string();
        let mut files = vec![];
        for (i, (file, _)) in original.files.iter().enumerate() {
            ensure!(
                current_hash(&file.target)?.as_deref() == Some(file.after.as_str()),
                "Source changed; Undo needs review"
            );
            if file.before.as_ref() == Some(&file.after) {
                continue;
            }
            cancel.check()?;
            let before = file
                .before
                .as_deref()
                .context("Missing original identity")?;
            let old = file.backup.as_ref().context("Missing original backup")?;
            ensure!(
                current_hash(old)?.as_deref() == Some(before),
                "Original backup changed"
            );
            let stage = work.path().join(format!("restore-{i}"));
            let backup = work.path().join(format!("current-{i}"));
            publish_files::copy_prepared(old, &stage, before)?;
            publish_files::copy_prepared(&file.target, &backup, &file.after)?;
            let publish = file
                .target
                .parent()
                .unwrap()
                .join(format!(".cuerithm-duplicate-{undo_id}-{i}.tmp"));
            let displaced = if cfg!(windows) {
                publish.with_extension("before.tmp")
            } else {
                publish.clone()
            };
            files.push((
                Artifact {
                    target: file.target.clone(),
                    staged: stage,
                    backup: Some(backup),
                    before: Some(file.after.clone()),
                    after: before.into(),
                    publish,
                    displaced,
                },
                work.path().canonicalize()?,
            ));
        }
        let mut result = original.payload.result.clone();
        result.id = undo_id.clone();
        result.undo_of = Some(id.into());
        result.stamp = stamp.clone();
        std::mem::swap(&mut result.archived_ids, &mut result.restored_ids);
        // Removed records come back as restored ones; the files stay wherever the user has them.
        result
            .restored_ids
            .extend(std::mem::take(&mut result.removed_ids));
        result.trashed.clear();
        result.permanently_deleted.clear();
        result.not_deleted.clear();
        let mut payload = Payload {
            schema: "duplicate-file-merge-v2".into(),
            preview_id: original.payload.preview_id.clone(),
            kind: "undo".into(),
            undo_of: Some(id.into()),
            result,
            before: original.payload.after.clone(),
            after: original.payload.before.clone(),
            playlists: original.payload.playlists.clone(),
            epochs: BTreeMap::new(),
            undone_by: None,
            removed: None,
            trash_intent: vec![],
            undo_expired: false,
        };
        let _gate = self
            .lib
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        reserve(&self.lib, &undo_id, &payload, &files, &stamp, cancel)?;
        let _kept = work.keep();
        let outcome = (|| -> Result<()> {
            let mut conn = self.lib.conn()?;
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            ensure!(
                donor_state::epochs(&tx, &tracks, &playlists)? == original.payload.epochs,
                "Later relevant edits prevent Undo"
            );
            let (state, text): (String, String) = tx.query_row(
                "SELECT state,payload_json FROM maintenance_writes WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let mut previous: Payload = serde_json::from_str(&text)?;
            ensure!(
                state == "committed" && previous.undone_by.is_none(),
                "Duplicate operation already undone"
            );
            for (id, after) in &original.payload.after {
                ensure!(
                    TrackImage::capture(&tx, id)? == *after,
                    "Later track edits prevent Undo"
                );
            }
            for p in &payload.playlists {
                let actual: Vec<String> = {
                    let mut q=tx.prepare("SELECT track_id FROM playlist_members WHERE playlist_id=?1 ORDER BY position")?;
                    q.query_map([&p.id], |r| r.get(0))?
                        .collect::<rusqlite::Result<_>>()?
                };
                ensure!(actual == p.after, "Later playlist edits prevent Undo");
            }
            // The removed records come back first: if the library no longer matches what the
            // removal left, this fails before any file is exchanged.
            if let Some(image) = &original.payload.removed {
                let report: RestoreReport = track_removal::restore(&tx, image)?;
                payload.result.links_not_restored = report.left_alone();
            }
            install(&files, cancel)?;
            #[cfg(test)]
            if let Some(hook) = self.hook {
                hook("files");
            }
            for (track_id, image) in &payload.after {
                if !removed_ids.contains(track_id) {
                    donor_state::restore(&tx, image)?;
                }
            }
            merge::write_members(&tx, &payload.playlists, true)?;
            payload.result.stamp = merge::advance(&tx)?;
            payload.epochs = donor_state::epochs(&tx, &tracks, &playlists)?;
            previous.undone_by = Some(undo_id.clone());
            previous.forget_removed();
            cancel.check()?;
            tx.execute(
                "UPDATE maintenance_writes SET state='undone',payload_json=?1 WHERE id=?2",
                params![serde_json::to_string(&previous)?, id],
            )?;
            tx.execute(
                "UPDATE maintenance_writes SET state='committed',payload_json=?1 WHERE id=?2",
                params![serde_json::to_string(&payload)?, undo_id],
            )?;
            tx.commit()?;
            Ok(())
        })();
        finish(&self.lib, &undo_id, &files, outcome.as_ref().err())?;
        outcome?;
        Ok(payload.result)
    }
}
impl Library {
    pub fn duplicate_recovery(&self) -> Result<Vec<DuplicateRecovery>> {
        let conn = self.conn()?;
        let mut q=conn.prepare("SELECT id,kind,state,created,error FROM maintenance_writes WHERE state IN ('prepared','conflict') OR (cleanup_complete=0 AND error IS NOT NULL) ORDER BY created,id")?;
        Ok(q.query_map([], |r| {
            Ok(DuplicateRecovery {
                id: r.get(0)?,
                kind: r.get(1)?,
                state: r.get(2)?,
                created: r.get(3)?,
                error: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
    }
    pub fn retry_duplicate_recovery(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<DuplicateRecovery>> {
        cancel.check()?;
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let write = load(self, id)?;
        if matches!(write.state.as_str(), "prepared" | "conflict") {
            let error = interruption(&write, "Explicit duplicate recovery retry");
            finish(self, id, &write.files, Some(&error))?;
        } else {
            finish(self, id, &write.files, None)?;
        }
        self.duplicate_recovery()
    }
    pub(crate) fn recover_duplicate_writes(&self) -> Result<()> {
        let ids = {
            let conn = self.conn()?;
            let mut q = conn.prepare(
                "SELECT id FROM maintenance_writes WHERE state='prepared' ORDER BY created,id",
            )?;
            q.query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in ids {
            let write = load(self, &id)?;
            let error = interruption(&write, "Interrupted duplicate operation rolled back");
            finish(self, &write.id, &write.files, Some(&error))?;
        }
        let ids = {
            let conn = self.conn()?;
            let mut q=conn.prepare("SELECT id FROM maintenance_writes WHERE state IN ('committed','undone','rolled_back') AND cleanup_complete=0")?;
            q.query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for id in ids {
            let write = load(self, &id)?;
            finish(self, &id, &write.files, None)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DuplicateReviewEdit, DuplicateScanRequest, GroupEdit};
    use std::sync::Arc;
    fn fixture() -> (
        tempfile::TempDir,
        Library,
        MediaRuntime,
        DuplicateMergePreview,
        PathBuf,
        String,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(&dir.path().join("app/library.sqlite")).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
        let mut ids = vec![];
        let mut paths = vec![];
        for index in 0..2 {
            let path = dir.path().join(format!("track-{index}.wav"));
            let mut w = hound::WavWriter::create(
                &path,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 8000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for i in 0..16000 {
                w.write_sample(((i * 37) % 20001 - 10000) as i16).unwrap();
            }
            w.finalize().unwrap();
            let job = lib
                .enqueue(
                    std::slice::from_ref(&path),
                    lib.settings().unwrap().revision,
                )
                .unwrap()[0]
                .clone();
            let id = lib
                .jobs()
                .unwrap()
                .into_iter()
                .find(|j| j.id == job)
                .unwrap()
                .track_id;
            ids.push(id);
            paths.push(path);
        }
        {
            let conn = lib.conn().unwrap();
            conn.execute(
                "UPDATE tracks SET state='ready',duration=20,title='Before',artist='North'",
                [],
            )
            .unwrap();
            conn.execute("UPDATE tracks SET title='Donor' WHERE id=?1", [&ids[1]])
                .unwrap();
        }
        let token = CancellationToken::default();
        let scan = lib
            .scan_duplicates(DuplicateScanRequest::default(), &runtime, &token)
            .unwrap();
        let review = lib.begin_duplicate_review(&scan.id, &token).unwrap();
        let review = lib
            .edit_duplicate_review(
                &review.id,
                0,
                DuplicateReviewEdit::Group {
                    group_id: review.groups[0].id.clone(),
                    edit: GroupEdit::UseTags {
                        track_id: Some(ids[1].clone()),
                    },
                },
                &token,
            )
            .unwrap();
        let preview = lib
            .preview_duplicate_merge(
                &review.id,
                review.revision,
                vec![review.groups[0].id.clone()],
                &token,
            )
            .unwrap();
        (dir, lib, runtime, preview, paths[0].clone(), ids[0].clone())
    }
    fn crash_prepared(phase: &str) {
        if phase == "prepared" {
            panic!("simulated prepared interruption");
        }
    }
    fn crash_files(phase: &str) {
        if phase == "files" {
            panic!("simulated exchanged-file interruption");
        }
    }
    #[test]
    fn recovery_rolls_back_prepared_and_exchanged_apply_without_publishing_library_changes() {
        for hook in [crash_prepared as fn(&str), crash_files as fn(&str)] {
            let (dir, lib, runtime, preview, path, target) = fixture();
            let hash = cuerithm_media::file_sha256(&path).unwrap();
            let mut service = DuplicateMergeService::new(lib.clone(), runtime);
            service.hook = Some(hook);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || service.apply(&preview.id, &CancellationToken::default())
                ))
                .is_err()
            );
            drop(service);
            drop(lib);
            let lib = Library::open(&dir.path().join("app/library.sqlite")).unwrap();
            assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), hash);
            assert_eq!(lib.track(&target).unwrap().title, "Before");
            assert_eq!(
                lib.conn()
                    .unwrap()
                    .query_row(
                        "SELECT state FROM maintenance_writes ORDER BY created DESC LIMIT 1",
                        [],
                        |r| r.get::<_, String>(0)
                    )
                    .unwrap(),
                "rolled_back"
            );
        }
    }
    #[test]
    fn interrupted_undo_recovers_to_the_committed_after_state_and_can_be_retried() {
        let (dir, lib, runtime, preview, path, target) = fixture();
        let original = cuerithm_media::file_sha256(&path).unwrap();
        let mut service = DuplicateMergeService::new(lib.clone(), runtime.clone());
        let applied = service
            .apply(&preview.id, &CancellationToken::default())
            .unwrap();
        let after = cuerithm_media::file_sha256(&path).unwrap();
        service.hook = Some(crash_files);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || service.undo(&applied.id, &CancellationToken::default())
            ))
            .is_err()
        );
        drop(service);
        drop(lib);
        let lib = Library::open(&dir.path().join("app/library.sqlite")).unwrap();
        assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), after);
        assert_eq!(lib.track(&target).unwrap().title, "Donor");
        DuplicateMergeService::new(lib, runtime)
            .undo(&applied.id, &CancellationToken::default())
            .unwrap();
        assert_eq!(cuerithm_media::file_sha256(&path).unwrap(), original);
    }
    struct MustNotRun;
    impl FileTrash for MustNotRun {
        fn move_to_trash(&self, path: &std::path::Path) -> std::result::Result<(), String> {
            panic!("{} must not be moved to the Trash", path.display());
        }
    }
    static CHANGE_BEFORE_TRASH: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);
    fn change_at(phase: &str, wanted: &str) {
        if phase == wanted
            && let Some(path) = CHANGE_BEFORE_TRASH.lock().unwrap().clone()
        {
            let mut bytes = std::fs::read(&path).unwrap();
            bytes.push(0);
            std::fs::write(&path, bytes).unwrap();
        }
    }
    fn change_after_install(phase: &str) {
        change_at(phase, "files");
    }
    fn change_just_before_trash(phase: &str) {
        change_at(phase, "trash");
    }
    #[test]
    fn a_file_that_changes_after_verification_is_kept_with_its_record() {
        // Changed after publication began: the whole merge stops and nothing is moved.
        // Changed just before it would be moved: only that file is kept.
        for (hook, whole_merge_fails) in [
            (change_after_install as fn(&str), true),
            (change_just_before_trash as fn(&str), false),
        ] {
            changed_duplicate_is_kept(hook, whole_merge_fails);
        }
    }
    fn changed_duplicate_is_kept(hook: fn(&str), whole_merge_fails: bool) {
        let (dir, lib, runtime, _donor_preview, paths_zero, _target) = fixture();
        let token = CancellationToken::default();
        let ids = {
            let conn = lib.conn().unwrap();
            let mut q = conn
                .prepare("SELECT id FROM tracks ORDER BY rowid")
                .unwrap();
            q.query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        let duplicate = dir.path().join("track-1.wav");
        // Make the two tracks the same audio so the second can be deleted.
        std::fs::copy(&paths_zero, &duplicate).unwrap();
        let hash = cuerithm_media::file_sha256(&duplicate).unwrap();
        {
            let conn = lib.conn().unwrap();
            conn.execute(
                "UPDATE tracks SET source_hash=?1 WHERE id=?2",
                params![hash, ids[1]],
            )
            .unwrap();
            // A ready track has finished its preparation job.
            conn.execute("UPDATE jobs SET state='completed',stage='completed'", [])
                .unwrap();
        }
        let scan = lib
            .scan_duplicates(DuplicateScanRequest::default(), &runtime, &token)
            .unwrap();
        let review = lib.begin_duplicate_review(&scan.id, &token).unwrap();
        let preview = lib
            .preview_duplicate_merge_choice(
                &review.id,
                review.revision,
                vec![review.groups[0].id.clone()],
                Some(MergeDisposition::Delete),
                &token,
            )
            .unwrap();
        let listed = preview.deletion.as_ref().unwrap().files.clone();
        assert_eq!(listed.len(), 1);
        let removed = listed[0].track_id.clone();
        let removed_path = if removed == ids[1] {
            duplicate.clone()
        } else {
            paths_zero.clone()
        };
        *CHANGE_BEFORE_TRASH.lock().unwrap() = Some(removed_path.clone());
        let mut service =
            DuplicateMergeService::with_trash(lib.clone(), runtime, Arc::new(MustNotRun));
        service.hook = Some(hook);
        let result = service.apply_confirmed(&preview.id, true, &token);
        *CHANGE_BEFORE_TRASH.lock().unwrap() = None;
        if whole_merge_fails {
            assert!(result.is_err());
        } else {
            let result = result.unwrap();
            assert!(result.removed_ids.is_empty());
            assert!(result.trashed.is_empty());
            assert_eq!(result.not_deleted.len(), 1);
            assert_eq!(
                result.not_deleted[0].reason,
                DuplicateSkipReason::FileChanged
            );
        }
        assert!(removed_path.exists(), "the changed file stays where it was");
        assert!(
            lib.track(&removed).is_ok(),
            "its record stays in the library"
        );
        assert!(!lib.track(&removed).unwrap().archived);
    }
    #[test]
    fn recovery_preserves_a_later_user_file_and_retains_its_recovery_assets() {
        let (dir, lib, runtime, preview, path, _) = fixture();
        let mut service = DuplicateMergeService::new(lib.clone(), runtime);
        service.hook = Some(crash_files);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || service.apply(&preview.id, &CancellationToken::default())
            ))
            .is_err()
        );
        drop(service);
        drop(lib);
        std::fs::write(&path, b"later user version").unwrap();
        let lib = Library::open(&dir.path().join("app/library.sqlite")).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"later user version");
        let conn = lib.conn().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT state FROM maintenance_writes ORDER BY created DESC LIMIT 1",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "conflict"
        );
        let backup: Vec<u8> = conn
            .query_row(
                "SELECT backup FROM maintenance_write_files LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(bytes_path(&backup).unwrap().is_file());
    }
}
