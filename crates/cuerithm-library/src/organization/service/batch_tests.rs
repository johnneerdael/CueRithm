use super::*;
use cuerithm_cleanup::{Metadata, MetadataField};
use cuerithm_media::CancellationToken;
use rusqlite::params;
use std::path::PathBuf;

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    library: Library,
    tracks: Vec<String>,
    sources: Vec<PathBuf>,
    destination: OrganizationRoot,
}
impl Fixture {
    fn new(count: usize) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let library = Library::open(&root.join("data/library.sqlite")).unwrap();
        let mut tracks = vec![];
        let mut sources = vec![];
        for index in 0..count {
            let source = root.join(format!("source-{index}.flac"));
            std::fs::write(&source, format!("opaque audio and records {index}")).unwrap();
            let job = library
                .enqueue(
                    std::slice::from_ref(&source),
                    library.settings().unwrap().revision,
                )
                .unwrap()
                .remove(0);
            let track = library
                .jobs()
                .unwrap()
                .into_iter()
                .find(|j| j.id == job)
                .unwrap()
                .track_id;
            let mut metadata = Metadata::default();
            metadata
                .fields
                .insert(MetadataField::Title, vec![format!("Title {index}")]);
            metadata
                .fields
                .insert(MetadataField::Artist, vec!["Artist".into()]);
            library
                .conn()
                .unwrap()
                .execute(
                    "UPDATE tracks SET state='ready',native_metadata_json=?1 WHERE id=?2",
                    params![serde_json::to_string(&metadata).unwrap(), track],
                )
                .unwrap();
            tracks.push(track);
            sources.push(source);
        }
        let managed = root.join("managed");
        std::fs::create_dir(&managed).unwrap();
        let destination = library.register_organization_root(managed).unwrap();
        Self {
            _temp: temp,
            root,
            library,
            tracks,
            sources,
            destination,
        }
    }
    fn preview(&self) -> OrganizationPreview {
        self.library
            .preview_organization(
                OrganizationPreviewRequest {
                    scope: OrganizationScope::Tracks {
                        track_ids: self.tracks.clone(),
                    },
                    settings: OrganizationSettings {
                        mode: OrganizationSettingMode::Move,
                        destination_root_id: Some(self.destination.id.clone()),
                        ..Default::default()
                    },
                },
                &CancellationToken::default(),
            )
            .unwrap()
    }
}
impl Fixture {
    fn reserved_batch(&self) -> (String, String) {
        let preview = self.preview();
        let batch = OrganizationBatch {
            id: uuid::Uuid::new_v4().to_string(),
            preview_id: Some(preview.id.clone()),
            kind: "apply".into(),
            state: "applying".into(),
            created: crate::model::now(),
            totals: Default::default(),
            can_undo: false,
            can_recover: false,
            resume_job_id: None,
            error: None,
        };
        let rows = self.library.organization_plan_rows(&preview.id).unwrap();
        {
            let mut conn = self.library.conn().unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute("INSERT INTO organization_batches(id,preview_id,kind,state,payload_json,created,updated) VALUES(?1,?2,'apply','applying',?3,?4,?4)", params![batch.id, preview.id, serde_json::to_string(&batch).unwrap(), batch.created]).unwrap();
            for (ordinal, row) in rows.iter().enumerate() {
                tx.execute("INSERT INTO organization_batch_rows(id,batch_id,track_id,ordinal,outcome,payload_json) VALUES(?1,?2,?3,?4,'ready',?5)", params![row.public.id, batch.id, row.public.track_id, ordinal, serde_json::to_string(row).unwrap()]).unwrap();
            }
            tx.commit().unwrap();
        }
        let row = &rows[0];
        let frozen = row.captured.as_ref().unwrap();
        let operation = self
            .library
            .plan_organization_for_row(
                crate::organization::OrganizationRequest {
                    track_id: self.tracks[0].clone(),
                    mode: crate::organization::OrganizationMode::Move,
                    source_root: self.root.clone(),
                    destination_root: self.root.join("managed"),
                    files: frozen
                        .files
                        .iter()
                        .map(|file| (file.source.path.0.clone(), file.destination.0.clone()))
                        .collect(),
                },
                &row.public.id,
            )
            .unwrap();
        self.library
            .bind_organization_references(&operation, &frozen.reference_digest)
            .unwrap();
        (batch.id, operation)
    }
}

#[test]
fn batch_applies_entire_frozen_scope_and_history_survives_reopen() {
    let f = Fixture::new(3);
    let preview = f.preview();
    assert_eq!(
        f.library
            .organization_preview_page(&preview.id, 0, 1)
            .unwrap()
            .rows
            .len(),
        1
    );
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(batch.state, "complete");
    assert_eq!(batch.totals.completed, 3);
    assert!(batch.can_undo);
    let page = f.library.organization_batch_page(&batch.id, 1, 1).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.rows.len(), 1);
    for (track, source) in f.tracks.iter().zip(&f.sources) {
        assert!(!source.exists());
        let current = f.library.track_path(track).unwrap();
        assert!(current.starts_with(f.root.join("managed")));
        assert!(current.exists());
    }
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    assert_eq!(
        reopened.organization_batches().unwrap()[0].totals.completed,
        3
    );
    assert!(
        reopened
            .apply_organization_preview(
                &preview.id,
                &preview.revision,
                &CancellationToken::default()
            )
            .is_err()
    );
}
#[test]
fn batch_rejects_wrong_revision_and_any_stale_source_before_publication() {
    let f = Fixture::new(2);
    let preview = f.preview();
    assert!(
        f.library
            .apply_organization_preview(&preview.id, "wrong", &CancellationToken::default())
            .is_err()
    );
    std::fs::write(&f.sources[1], b"externally replaced").unwrap();
    assert!(
        f.library
            .apply_organization_preview(
                &preview.id,
                &preview.revision,
                &CancellationToken::default()
            )
            .is_err()
    );
    assert!(f.sources[0].exists());
    assert_eq!(
        std::fs::read_dir(f.root.join("managed")).unwrap().count(),
        0
    );
}
#[test]
fn batch_keeps_blocked_rows_and_undoes_only_completed_owned_paths() {
    let f = Fixture::new(2);
    let occupied = f.root.join("managed/Artist - Title 1.flac");
    std::fs::write(&occupied, b"unrelated file").unwrap();
    let preview = f.preview();
    assert_eq!(preview.totals.blocked, 1);
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(batch.state, "partial");
    assert_eq!(batch.totals.completed, 1);
    assert_eq!(batch.totals.blocked, 1);
    let undone = f
        .library
        .undo_organization_batch(&batch.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(undone.state, "undone");
    assert!(f.sources.iter().all(|path| path.exists()));
    assert_eq!(std::fs::read(occupied).unwrap(), b"unrelated file");
}
#[test]
fn batch_undo_refuses_external_edits_without_overwriting_them() {
    let f = Fixture::new(1);
    let preview = f.preview();
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    std::fs::write(&f.sources[0], b"later external file").unwrap();
    let undone = f
        .library
        .undo_organization_batch(&batch.id, &CancellationToken::default())
        .unwrap();
    assert_ne!(undone.state, "undone");
    assert_eq!(
        std::fs::read(&f.sources[0]).unwrap(),
        b"later external file"
    );
    assert!(f.library.track_path(&f.tracks[0]).unwrap().exists());
}

#[test]
fn batch_undo_blocks_changed_or_missing_managed_files_without_aborting_the_rest() {
    let f = Fixture::new(3);
    let preview = f.preview();
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(batch.totals.completed, 3);
    let managed = |index: usize| f.root.join(format!("managed/Artist - Title {index}.flac"));
    std::fs::write(managed(0), b"edited after organization").unwrap();
    std::fs::remove_file(managed(1)).unwrap();
    let undone = f
        .library
        .undo_organization_batch(&batch.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(undone.state, "partial");
    assert_eq!(undone.totals.blocked, 2);
    assert_eq!(undone.totals.completed, 1);
    assert!(
        f.sources[2].exists(),
        "the unaffected track returns to its original path"
    );
    assert!(!f.sources[0].exists() && !f.sources[1].exists());
    assert_eq!(
        std::fs::read(managed(0)).unwrap(),
        b"edited after organization"
    );
    assert_eq!(f.library.track_path(&f.tracks[0]).unwrap(), managed(0));
    let blocked = f
        .library
        .organization_batch_page(&undone.id, 0, 10)
        .unwrap()
        .rows
        .into_iter()
        .filter(|row| row.outcome == "blocked")
        .map(|row| row.reason.unwrap())
        .collect::<Vec<_>>();
    assert_eq!(blocked.len(), 2);
    assert!(
        blocked
            .iter()
            .all(|reason| reason.starts_with("Undo blocked because the track changed"))
    );
}

#[test]
fn batch_reserves_full_scope_before_first_publication() {
    let f = Fixture::new(2);
    let preview = f.preview();
    let conflicting = f
        .library
        .plan_organization(crate::organization::OrganizationRequest {
            track_id: f.tracks[1].clone(),
            mode: crate::organization::OrganizationMode::Move,
            source_root: f.root.clone(),
            destination_root: f.root.join("managed"),
            files: vec![(
                f.sources[1].clone(),
                f.root.join("managed/independent.flac"),
            )],
        })
        .unwrap();
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(batch.totals.completed, 0);
    assert_eq!(batch.totals.failed, 2);
    assert!(f.sources.iter().all(|path| path.exists()));
    assert_eq!(
        std::fs::read_dir(f.root.join("managed")).unwrap().count(),
        0
    );
    f.library
        .cancel_unstarted_organization(&conflicting)
        .unwrap();
}
#[test]
fn batch_stale_metadata_invalidates_whole_scope_before_publication() {
    let f = Fixture::new(2);
    let preview = f.preview();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET title='later edit' WHERE id=?1",
            [&f.tracks[1]],
        )
        .unwrap();
    assert!(
        f.library
            .apply_organization_preview(
                &preview.id,
                &preview.revision,
                &CancellationToken::default()
            )
            .is_err()
    );
    assert!(f.sources.iter().all(|path| path.exists()));
    assert_eq!(
        std::fs::read_dir(f.root.join("managed")).unwrap().count(),
        0
    );
}
#[test]
fn batch_cancellation_during_staging_is_durable_and_recovers_after_reopen() {
    let f = Fixture::new(1);
    let source = &f.sources[0];
    let byte_len = 192 * 1024 + 17;
    std::fs::File::create(source)
        .unwrap()
        .set_len(byte_len)
        .unwrap();
    let hash = cuerithm_media::file_sha256(source).unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET source_hash=?1 WHERE id=?2",
            params![hash, f.tracks[0]],
        )
        .unwrap();
    let preview = f.preview();
    let token = CancellationToken::default();
    let mut observed = false;
    let batch = f
        .library
        .apply_organization_preview_with(&preview.id, &preview.revision, &token, |boundary| {
            if boundary == crate::organization::publish::Boundary::StageProgress {
                observed = true;
                token.cancel();
            }
            Ok(())
        })
        .unwrap();
    assert!(
        observed,
        "must interrupt the actual first partial-stage boundary"
    );
    assert_eq!(batch.state, "recovery_required");
    assert_eq!(batch.totals.recovery_required, 1);
    assert!(batch.can_recover);
    assert!(source.exists());
    let row = f
        .library
        .organization_batch_page(&batch.id, 0, 1)
        .unwrap()
        .rows
        .remove(0);
    let record = f
        .library
        .organization_history()
        .unwrap()
        .into_iter()
        .find(|r| Some(&r.id) == row.operation_id.as_ref())
        .unwrap();
    let partial = f.root.join("managed").join(&record.files[0].stage_name);
    let partial_bytes = std::fs::metadata(&partial).unwrap().len();
    assert!(partial_bytes > 0 && partial_bytes < byte_len);
    assert!(!std::path::Path::new(row.after_path.as_ref().unwrap()).exists());
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let recovered = reopened
        .recover_organization_batch(&batch.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "complete");
    assert!(
        recovered.error.is_none(),
        "completed recovery must clear the previous interruption error"
    );
    assert_eq!(recovered.totals.completed, 1);
    assert_eq!(
        cuerithm_media::file_sha256(&reopened.track_path(&f.tracks[0]).unwrap()).unwrap(),
        hash
    );
    assert_eq!(
        std::fs::metadata(partial).unwrap().len(),
        partial_bytes,
        "preserve interrupted recovery bytes"
    );
    drop(reopened);
    let reopened_again = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let history = reopened_again.organization_batches().unwrap();
    assert!(
        history
            .iter()
            .find(|item| item.id == batch.id)
            .unwrap()
            .error
            .is_none()
    );
}

#[test]
fn orphan_applying_batch_is_discoverable_and_recoverable_after_restart() {
    let f = Fixture::new(1);
    let (batch, operation) = f.reserved_batch();
    let fault = f.library.execute_organization_with(&operation, |boundary| {
        if boundary == crate::organization::publish::Boundary::Published {
            anyhow::bail!("simulated process stop after publication");
        }
        Ok(())
    });
    assert!(fault.is_err());
    assert!(f.sources[0].exists());
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let orphan = reopened.organization_batches().unwrap().remove(0);
    assert_eq!(orphan.state, "recovery_required");
    assert_eq!(orphan.totals.recovery_required, 1);
    assert!(orphan.can_recover);
    let recovered = reopened
        .recover_organization_batch(&batch, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "complete");
    assert!(!f.sources[0].exists());
    assert!(reopened.track_path(&f.tracks[0]).unwrap().exists());
}
#[test]
fn cleanup_recovery_requires_a_verified_acknowledgement_before_source_cleanup() {
    let f = Fixture::new(1);
    let (batch, operation) = f.reserved_batch();
    f.library.execute_organization(&operation).unwrap();
    let guard = f
        .library
        .repair_organization_references(&operation)
        .unwrap();
    f.library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    drop(guard);
    // Simulate lost/corrupted receipt evidence after the local path transition.
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE organization_reference_intents SET state='bound' WHERE operation_id=?1",
            [&operation],
        )
        .unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let recovered = reopened
        .recover_organization_batch(&batch, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "recovery_required");
    assert!(recovered.error.unwrap().contains("acknowledgement"));
    assert!(
        f.sources[0].exists(),
        "failed guard cannot authorize source quarantine"
    );
    assert!(reopened.track_path(&f.tracks[0]).unwrap().exists());
}
#[test]
fn batch_copy_undo_rebinds_the_original_without_overwriting_it() {
    let f = Fixture::new(1);
    let preview = f
        .library
        .preview_organization(
            OrganizationPreviewRequest {
                scope: OrganizationScope::Tracks {
                    track_ids: f.tracks.clone(),
                },
                settings: OrganizationSettings {
                    mode: OrganizationSettingMode::Copy,
                    destination_root_id: Some(f.destination.id.clone()),
                    ..Default::default()
                },
            },
            &CancellationToken::default(),
        )
        .unwrap();
    let original_bytes = std::fs::read(&f.sources[0]).unwrap();
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(batch.state, "complete");
    assert!(f.sources[0].exists());
    let managed = f.library.track_path(&f.tracks[0]).unwrap();
    assert_ne!(managed, f.sources[0]);
    let undo = f
        .library
        .undo_organization_batch(&batch.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(undo.state, "undone");
    assert_eq!(f.library.track_path(&f.tracks[0]).unwrap(), f.sources[0]);
    assert_eq!(std::fs::read(&f.sources[0]).unwrap(), original_bytes);
    assert!(!managed.exists());
    assert!(
        !f.library
            .organization_batches()
            .unwrap()
            .iter()
            .find(|b| b.id == batch.id)
            .unwrap()
            .can_undo
    );
}
#[test]
fn durable_cancel_flag_interrupts_apply_and_survives_restart() {
    let f = Fixture::new(1);
    std::fs::File::create(&f.sources[0])
        .unwrap()
        .set_len(192 * 1024 + 17)
        .unwrap();
    let hash = cuerithm_media::file_sha256(&f.sources[0]).unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET source_hash=?1 WHERE id=?2",
            params![hash, f.tracks[0]],
        )
        .unwrap();
    let preview = f.preview();
    let mut observed = false;
    let batch = f
        .library
        .apply_organization_preview_with(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
            |boundary| {
                if boundary == crate::organization::publish::Boundary::StageProgress && !observed {
                    observed = true;
                    let batch = f
                        .library
                        .organization_batches()
                        .unwrap()
                        .into_iter()
                        .find(|b| b.preview_id.as_ref() == Some(&preview.id))
                        .unwrap();
                    f.library.cancel_organization_batch(&batch.id)?;
                }
                Ok(())
            },
        )
        .unwrap();
    assert!(
        observed,
        "cancel through the durable API at a real staging boundary"
    );
    assert_eq!(batch.state, "recovery_required");
    assert!(batch.can_recover);
    assert!(f.sources[0].exists());
    let requested: bool = f
        .library
        .conn()
        .unwrap()
        .query_row(
            "SELECT cancel_requested FROM organization_batches WHERE id=?1",
            [&batch.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(requested);
    let row = f
        .library
        .organization_batch_page(&batch.id, 0, 1)
        .unwrap()
        .rows
        .remove(0);
    assert!(!std::path::Path::new(row.after_path.as_ref().unwrap()).exists());
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let requested: bool = reopened
        .conn()
        .unwrap()
        .query_row(
            "SELECT cancel_requested FROM organization_batches WHERE id=?1",
            [&batch.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        requested,
        "opening must not silently resume a cancelled apply"
    );
    let recovered = reopened
        .recover_organization_batch(&batch.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "complete");
}

#[test]
fn paged_results_and_history_never_decode_off_page_frozen_payloads() {
    let f = Fixture::new(2);
    let preview = f.preview();
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    f.library.conn().unwrap().execute("UPDATE organization_batch_rows SET payload_json='malformed off-page payload' WHERE batch_id=?1 AND ordinal=1", [&batch.id]).unwrap();
    let first = f.library.organization_batch_page(&batch.id, 0, 1).unwrap();
    assert_eq!(first.total, 2);
    assert_eq!(first.rows.len(), 1);
    assert!(f.library.organization_batch_page(&batch.id, 1, 1).is_err());
    let history = f.library.organization_batches().unwrap();
    assert_eq!(history[0].totals.completed, 2);
    assert!(history[0].can_undo);
}
#[test]
fn orphan_after_child_completion_can_reconcile_without_reapplying_files() {
    let f = Fixture::new(1);
    let preview = f.preview();
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    let managed = f.library.track_path(&f.tracks[0]).unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE organization_batches SET state='applying' WHERE id=?1",
            [&batch.id],
        )
        .unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE organization_batch_rows SET outcome='ready' WHERE batch_id=?1",
            [&batch.id],
        )
        .unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let orphan = reopened.organization_batches().unwrap().remove(0);
    assert_eq!(orphan.state, "recovery_required");
    assert!(orphan.can_recover);
    let recovered = reopened
        .recover_organization_batch(&batch.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "complete");
    assert_eq!(recovered.totals.completed, 1);
    assert_eq!(reopened.track_path(&f.tracks[0]).unwrap(), managed);
    assert!(!f.sources[0].exists());
}
#[test]
fn batch_undo_keeps_later_metadata_changes_and_records_a_blocker() {
    let f = Fixture::new(1);
    let preview = f.preview();
    let batch = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    let managed = f.library.track_path(&f.tracks[0]).unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET title='later user title' WHERE id=?1",
            [&f.tracks[0]],
        )
        .unwrap();
    let undo = f
        .library
        .undo_organization_batch(&batch.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(undo.state, "partial");
    assert_eq!(undo.totals.blocked, 1);
    assert_eq!(f.library.track_path(&f.tracks[0]).unwrap(), managed);
    let title: String = f
        .library
        .conn()
        .unwrap()
        .query_row(
            "SELECT title FROM tracks WHERE id=?1",
            [&f.tracks[0]],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(title, "later user title");
}

#[test]
fn interrupted_scope_reservation_cannot_publish_the_reserved_subset_on_recovery() {
    let f = Fixture::new(2);
    // Simulate a crash between reserving the first and second ready row.
    let (batch, _) = f.reserved_batch();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let recovered = reopened
        .recover_organization_batch(&batch, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "partial");
    assert_eq!(recovered.totals.failed, 2);
    assert_eq!(recovered.totals.completed, 0);
    assert!(f.sources.iter().all(|path| path.exists()));
    assert_eq!(
        std::fs::read_dir(f.root.join("managed")).unwrap().count(),
        0
    );
}

#[test]
fn metadata_changed_after_reservation_blocks_recovery_before_publication() {
    let f = Fixture::new(1);
    let (batch, _) = f.reserved_batch();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET title='edited after reservation' WHERE id=?1",
            [&f.tracks[0]],
        )
        .unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let recovered = reopened
        .recover_organization_batch(&batch, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "partial");
    assert_eq!(recovered.totals.failed, 1);
    assert!(f.sources[0].exists());
    assert!(
        !f.root.join("managed/Artist - Title 0.flac").exists(),
        "stale naming evidence must be rejected before filesystem publication"
    );
}

#[test]
fn cancelling_restored_planned_rows_retires_pending_operations_and_reservations() {
    let f = Fixture::new(1);
    let (batch, _) = f.reserved_batch();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    reopened.cancel_organization_batch(&batch).unwrap();
    let result = reopened.organization_batches().unwrap().remove(0);
    assert_eq!(result.state, "cancelled");
    assert_eq!(result.totals.cancelled, 1);
    assert!(!result.can_recover);
    let count: usize = reopened
        .conn()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM organization_reservations", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
    assert!(f.sources[0].exists());
}
#[test]
fn source_changed_before_publication_fails_without_leaking_reservations() {
    let f = Fixture::new(1);
    let (batch, _) = f.reserved_batch();
    std::fs::write(&f.sources[0], b"later user audio bytes").unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let result = reopened
        .recover_organization_batch(&batch, &CancellationToken::default())
        .unwrap();
    assert_eq!(result.state, "partial");
    assert_eq!(result.totals.failed, 1);
    assert!(!result.can_recover);
    assert_eq!(
        std::fs::read(&f.sources[0]).unwrap(),
        b"later user audio bytes"
    );
    let count: usize = reopened
        .conn()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM organization_reservations", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        std::fs::read_dir(f.root.join("managed")).unwrap().count(),
        0
    );
}

#[test]
fn cancelling_restored_published_rows_preserves_reference_recovery() {
    let f = Fixture::new(1);
    let (batch, operation) = f.reserved_batch();
    f.library.execute_organization(&operation).unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    reopened.cancel_organization_batch(&batch).unwrap();
    let result = reopened.organization_batches().unwrap().remove(0);
    assert_eq!(result.state, "recovery_required");
    assert_eq!(result.totals.recovery_required, 1);
    assert!(result.can_recover);
    assert!(f.sources[0].exists());
    assert!(f.root.join("managed/Artist - Title 0.flac").exists());
    let recovered = reopened
        .recover_organization_batch(&batch, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "complete");
    assert!(!f.sources[0].exists());
}

#[test]
fn copy_undo_recovers_a_crash_before_reference_binding() {
    let f = Fixture::new(1);
    let preview = f
        .library
        .preview_organization(
            OrganizationPreviewRequest {
                scope: OrganizationScope::Tracks {
                    track_ids: f.tracks.clone(),
                },
                settings: OrganizationSettings {
                    mode: OrganizationSettingMode::Copy,
                    destination_root_id: Some(f.destination.id.clone()),
                    ..Default::default()
                },
            },
            &CancellationToken::default(),
        )
        .unwrap();
    let original = f
        .library
        .apply_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    let operation = f
        .library
        .organization_batch_page(&original.id, 0, 1)
        .unwrap()
        .rows
        .remove(0)
        .operation_id
        .unwrap();
    let (mut row, _) = f
        .library
        .capture_organization_row(&f.tracks[0], false, false, &CancellationToken::default())
        .unwrap();
    let reference = f
        .library
        .organization_reference_snapshot(&f.tracks[0])
        .unwrap();
    row.captured.as_mut().unwrap().reference_digest = reference.digest;
    let inverse = OrganizationBatch {
        id: uuid::Uuid::new_v4().to_string(),
        preview_id: None,
        kind: "undo".into(),
        state: "applying".into(),
        created: crate::model::now(),
        totals: Default::default(),
        can_undo: false,
        can_recover: false,
        resume_job_id: None,
        error: None,
    };
    f.library.conn().unwrap().execute("INSERT INTO organization_batches(id,undo_of,kind,state,payload_json,created,updated) VALUES(?1,?2,'undo','applying',?3,?4,?4)", params![inverse.id, original.id, serde_json::to_string(&inverse).unwrap(), inverse.created]).unwrap();
    f.library.conn().unwrap().execute("INSERT INTO organization_batch_rows(id,batch_id,track_id,ordinal,outcome,payload_json) VALUES(?1,?2,?3,0,'ready',?4)", params![row.public.id, inverse.id, row.public.track_id, serde_json::to_string(&row).unwrap()]).unwrap();
    f.library
        .plan_organization_undo_for_row(&operation, &row.public.id)
        .unwrap();
    // Simulate interruption before the inverse reference intent is bound.
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let recovered = reopened
        .recover_organization_batch(&inverse.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(recovered.state, "undone");
    assert_eq!(reopened.track_path(&f.tracks[0]).unwrap(), f.sources[0]);
    assert!(f.sources[0].exists());
}

#[test]
fn explicit_stopped_cancellation_retires_partial_stage_without_deleting_recovery_bytes() {
    let f = Fixture::new(1);
    std::fs::File::create(&f.sources[0])
        .unwrap()
        .set_len(192 * 1024 + 17)
        .unwrap();
    let hash = cuerithm_media::file_sha256(&f.sources[0]).unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET source_hash=?1 WHERE id=?2",
            params![hash, f.tracks[0]],
        )
        .unwrap();
    let preview = f.preview();
    let token = CancellationToken::default();
    let batch = f
        .library
        .apply_organization_preview_with(&preview.id, &preview.revision, &token, |boundary| {
            if boundary == crate::organization::publish::Boundary::StageProgress {
                token.cancel();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(batch.state, "recovery_required");
    let operation = f
        .library
        .organization_batch_page(&batch.id, 0, 1)
        .unwrap()
        .rows
        .remove(0)
        .operation_id
        .unwrap();
    let record = f.library.organization_record(&operation).unwrap();
    let stage = f.root.join("managed").join(&record.files[0].stage_name);
    let stage_bytes = std::fs::read(&stage).unwrap();
    assert!(!stage_bytes.is_empty());
    f.library.cancel_organization_batch(&batch.id).unwrap();
    let result = f
        .library
        .organization_batches()
        .unwrap()
        .into_iter()
        .find(|item| item.id == batch.id)
        .unwrap();
    assert_eq!(result.state, "cancelled");
    assert_eq!(result.totals.cancelled, 1);
    assert!(!result.can_recover);
    let reservations: usize = f
        .library
        .conn()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM organization_reservations WHERE operation_id=?1",
            [&operation],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reservations, 0);
    assert_eq!(cuerithm_media::file_sha256(&f.sources[0]).unwrap(), hash);
    assert!(!f.root.join("managed/Artist - Title 0.flac").exists());
    assert_eq!(std::fs::read(stage).unwrap(), stage_bytes);
}

impl Fixture {
    fn preparation_preview(&self, settings: OrganizationSettings) -> (OrganizationPreview, String) {
        self.library
            .conn()
            .unwrap()
            .execute(
                "UPDATE tracks SET state='incoming' WHERE id=?1",
                [&self.tracks[0]],
            )
            .unwrap();
        let preview = self
            .library
            .preview_organization_ids(
                OrganizationPreviewRequest {
                    scope: OrganizationScope::Tracks {
                        track_ids: self.tracks.clone(),
                    },
                    settings,
                },
                Some(self.tracks.clone()),
                &CancellationToken::default(),
            )
            .unwrap();
        let job = self
            .library
            .jobs()
            .unwrap()
            .into_iter()
            .find(|job| job.track_id == self.tracks[0])
            .unwrap();
        let mut plan = self.library.organization_plan(&preview.id).unwrap();
        plan.preparation_job_id = Some(job.id.clone());
        let source_hash = cuerithm_media::file_sha256(&self.sources[0]).unwrap();
        let revision = self.library.settings().unwrap().revision;
        self.library
            .conn()
            .unwrap()
            .execute(
                "UPDATE organization_plans SET payload_json=?1 WHERE id=?2",
                params![serde_json::to_string(&plan).unwrap(), preview.id],
            )
            .unwrap();
        self.library.conn().unwrap().execute("INSERT INTO preparation_checkpoints(job_id,track_id,settings_revision,input_hash,output_hash,attempt,payload_json,state,created,updated) VALUES(?1,?2,?3,?4,?4,?5,'{}','prepared',?6,?6)", params![job.id, self.tracks[0], revision, source_hash, job.attempts, crate::model::now()]).unwrap();
        (preview, job.id)
    }
}
#[test]
fn preparation_no_op_persists_a_linked_batch_and_orphan_bookkeeping_can_finish() {
    let f = Fixture::new(1);
    let (preview, job) = f.preparation_preview(OrganizationSettings::default());
    assert!(!preview.can_apply);
    assert!(
        f.library
            .apply_organization_preview(
                &preview.id,
                &preview.revision,
                &CancellationToken::default()
            )
            .is_err()
    );
    let batch = f
        .library
        .apply_preparation_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(batch.state, "complete");
    assert_eq!(batch.totals.unchanged, 1);
    let linked: String = f
        .library
        .conn()
        .unwrap()
        .query_row(
            "SELECT batch_id FROM preparation_checkpoints WHERE job_id=?1",
            [&job],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(linked, batch.id);
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE organization_batches SET state='applying' WHERE id=?1",
            [&batch.id],
        )
        .unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let orphan = reopened.organization_batches().unwrap().remove(0);
    assert!(!orphan.can_recover, "queued Incoming job owns recovery");
    assert!(
        reopened
            .recover_organization_batch(&batch.id, &CancellationToken::default())
            .unwrap_err()
            .to_string()
            .contains("Retry in Incoming")
    );
    let recovered = reopened
        .recover_preparation_organization_batch_with(
            &batch.id,
            &CancellationToken::default(),
            |_| Ok(()),
        )
        .unwrap();
    assert_eq!(recovered.state, "complete");
    assert_eq!(recovered.totals.unchanged, 1);
    assert!(f.sources[0].exists());
}
#[test]
fn preparation_checkpoint_binding_failure_rolls_back_before_publication() {
    let f = Fixture::new(1);
    let (preview, job) = f.preparation_preview(OrganizationSettings {
        mode: OrganizationSettingMode::Move,
        destination_root_id: Some(f.destination.id.clone()),
        ..Default::default()
    });
    f.library
        .conn()
        .unwrap()
        .execute(
            "DELETE FROM preparation_checkpoints WHERE job_id=?1",
            [&job],
        )
        .unwrap();
    let result = f.library.apply_preparation_organization_preview(
        &preview.id,
        &preview.revision,
        &CancellationToken::default(),
    );
    assert!(
        result.is_err(),
        "automatic publication must require atomic checkpoint ownership"
    );
    assert!(f.sources[0].exists());
    assert_eq!(
        std::fs::read_dir(f.root.join("managed")).unwrap().count(),
        0
    );
    assert!(f.library.organization_batches().unwrap().is_empty());
    let state: String = f
        .library
        .conn()
        .unwrap()
        .query_row(
            "SELECT state FROM organization_plans WHERE id=?1",
            [&preview.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "preview");
}

#[test]
fn preparation_no_op_revalidates_native_source_before_checkpoint_acceptance() {
    let f = Fixture::new(1);
    let (preview, job) = f.preparation_preview(OrganizationSettings::default());
    std::fs::write(&f.sources[0], b"changed after preparation checkpoint").unwrap();
    assert!(
        f.library
            .apply_preparation_organization_preview(
                &preview.id,
                &preview.revision,
                &CancellationToken::default()
            )
            .is_err()
    );
    let linked: Option<String> = f
        .library
        .conn()
        .unwrap()
        .query_row(
            "SELECT batch_id FROM preparation_checkpoints WHERE job_id=?1",
            [&job],
            |row| row.get(0),
        )
        .unwrap();
    assert!(linked.is_none());
    assert!(f.library.organization_batches().unwrap().is_empty());
}

#[test]
fn preparation_worker_callback_guards_reference_repair_acknowledgement_and_cleanup() {
    for stop_at in [2, 3, 4] {
        let f = Fixture::new(1);
        let (preview, _) = f.preparation_preview(OrganizationSettings {
            mode: OrganizationSettingMode::Move,
            destination_root_id: Some(f.destination.id.clone()),
            ..Default::default()
        });
        let mut published_boundaries = 0;
        let stopped = f
            .library
            .apply_preparation_organization_preview_with(
                &preview.id,
                &preview.revision,
                &CancellationToken::default(),
                |boundary| {
                    if boundary == crate::organization::publish::Boundary::Published {
                        published_boundaries += 1;
                        if published_boundaries == stop_at {
                            anyhow::bail!("expired preparation attempt");
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(stopped.state, "recovery_required");
        assert_eq!(published_boundaries, stop_at);
        assert!(
            f.sources[0].exists(),
            "expired worker must leave the original available"
        );
        let current = f.library.track_path(&f.tracks[0]).unwrap();
        if stop_at <= 3 {
            assert_eq!(
                current, f.sources[0],
                "expired worker cannot commit its path after repair"
            );
        }
        let recovered = f
            .library
            .recover_preparation_organization_batch_with(
                &stopped.id,
                &CancellationToken::default(),
                |_| Ok(()),
            )
            .unwrap();
        assert_eq!(recovered.state, "complete");
        assert!(recovered.error.is_none());
        assert!(!f.sources[0].exists());
        let state: String = f
            .library
            .conn()
            .unwrap()
            .query_row(
                "SELECT state FROM tracks WHERE id=?1",
                [&f.tracks[0]],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            state, "incoming",
            "batch coordinator never promotes an Incoming job itself"
        );
    }
}

#[test]
fn active_preparation_history_routes_attention_to_incoming_and_disables_manual_undo() {
    let f = Fixture::new(1);
    let (preview, job) = f.preparation_preview(OrganizationSettings {
        mode: OrganizationSettingMode::Move,
        destination_root_id: Some(f.destination.id.clone()),
        ..Default::default()
    });
    let batch = f
        .library
        .apply_preparation_organization_preview(
            &preview.id,
            &preview.revision,
            &CancellationToken::default(),
        )
        .unwrap();
    assert!(
        !batch.can_undo,
        "organization completion is not Incoming Ready promotion"
    );
    assert!(
        !batch.can_recover,
        "queued owner already controls preparation"
    );
    assert!(batch.resume_job_id.is_none());
    f.library
        .conn()
        .unwrap()
        .execute("UPDATE jobs SET state='attention' WHERE id=?1", [&job])
        .unwrap();
    let attention = f.library.organization_batches().unwrap().remove(0);
    assert_eq!(attention.resume_job_id.as_deref(), Some(job.as_str()));
    assert!(attention.can_recover);
    assert!(!attention.can_undo);
    assert!(
        f.library
            .recover_organization_batch(&batch.id, &CancellationToken::default())
            .unwrap_err()
            .to_string()
            .contains("Retry in Incoming")
    );
    assert!(
        f.library
            .undo_organization_batch(&batch.id, &CancellationToken::default())
            .is_err()
    );
    for state in ["queued", "running"] {
        f.library
            .conn()
            .unwrap()
            .execute("UPDATE jobs SET state=?1 WHERE id=?2", params![state, job])
            .unwrap();
        let owned = f.library.organization_batches().unwrap().remove(0);
        assert!(owned.resume_job_id.is_none());
        assert!(!owned.can_recover);
        assert!(!owned.can_undo);
    }
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE preparation_checkpoints SET state='complete' WHERE job_id=?1",
            [&job],
        )
        .unwrap();
    let completed = f.library.organization_batches().unwrap().remove(0);
    assert!(completed.resume_job_id.is_none());
    assert!(!completed.can_recover);
}
