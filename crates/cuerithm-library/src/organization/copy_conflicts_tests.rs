use super::*;
use cuerithm_media::CancellationToken;
use rusqlite::params;
use std::path::PathBuf;
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    library: Library,
    source: PathBuf,
    destination: PathBuf,
    operation: String,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let library = Library::open(&root.join("data/library.sqlite")).unwrap();
        let source = root.join("original.wav");
        let destination = root.join("managed/captured.wav");
        std::fs::write(&source, b"captured managed audio").unwrap();
        std::fs::create_dir(destination.parent().unwrap()).unwrap();
        let hash = cuerithm_media::file_sha256(&source).unwrap();
        library.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('copy-track',?1,'Captured',?2,'ready',1,1)", params![NativePath::new(&source).0, hash]).unwrap();
        let references = library
            .organization_reference_snapshot("copy-track")
            .unwrap();
        let operation = library
            .plan_organization(OrganizationRequest {
                track_id: "copy-track".into(),
                mode: OrganizationMode::Copy,
                source_root: root.clone(),
                destination_root: root.join("managed"),
                files: vec![(source.clone(), destination.clone())],
            })
            .unwrap();
        library
            .bind_organization_references(&operation, &references.digest)
            .unwrap();
        library.execute_organization(&operation).unwrap();
        std::fs::write(&source, b"changed original audio").unwrap();
        assert!(library.repair_organization_references(&operation).is_err());
        Self {
            _temp: temp,
            root,
            library,
            source,
            destination,
            operation,
        }
    }
}
#[test]
fn pending_copy_keep_managed_choice_continues_and_survives_restart() {
    let f = Fixture::new();
    let result = f.library.resolve_organization_alternate_cancellable(
        &f.operation,
        AlternateResolution::IgnoreThisRevision,
        &CancellationToken::default(),
    );
    assert!(
        result.is_ok(),
        "pending copy must offer the approved choice: {result:?}"
    );
    assert_eq!(f.library.track_path("copy-track").unwrap(), f.destination);
    assert_eq!(std::fs::read(&f.source).unwrap(), b"changed original audio");
    assert_eq!(
        std::fs::read(&f.destination).unwrap(),
        b"captured managed audio"
    );
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let hash = cuerithm_media::file_sha256(&f.source).unwrap();
    assert!(matches!(
        reopened
            .organization_source_state(&f.source, &hash)
            .unwrap(),
        AlternateSourceState::Owned { .. }
    ));
    std::fs::write(&f.source, b"another original revision").unwrap();
    let hash = cuerithm_media::file_sha256(&f.source).unwrap();
    assert!(matches!(
        reopened
            .organization_source_state(&f.source, &hash)
            .unwrap(),
        AlternateSourceState::Conflict { .. }
    ));
}
#[test]
fn pending_copy_add_original_atomically_creates_a_distinct_incoming_identity() {
    let f = Fixture::new();
    let result = f.library.resolve_organization_alternate_cancellable(
        &f.operation,
        AlternateResolution::AddAsNewIncoming,
        &CancellationToken::default(),
    );
    assert!(
        result.is_ok(),
        "pending copy must queue its changed original after retarget: {result:?}"
    );
    let job = result
        .unwrap()
        .expect("changed original needs its own Incoming job");
    assert_eq!(f.library.track_path("copy-track").unwrap(), f.destination);
    let incoming = f
        .library
        .jobs()
        .unwrap()
        .into_iter()
        .find(|entry| entry.id == job)
        .unwrap();
    assert_ne!(incoming.track_id, "copy-track");
    assert_eq!(f.library.track_path(&incoming.track_id).unwrap(), f.source);
    assert_eq!(std::fs::read(&f.source).unwrap(), b"changed original audio");
    assert_eq!(
        std::fs::read(&f.destination).unwrap(),
        b"captured managed audio"
    );
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    assert_eq!(
        reopened
            .jobs()
            .unwrap()
            .iter()
            .filter(|entry| entry.id == job)
            .count(),
        1
    );
}

#[test]
fn stale_pending_copy_decision_cannot_acknowledge_a_later_original_revision() {
    let f = Fixture::new();
    f.library
        .choose_pending_copy_resolution(&f.operation, AlternateResolution::IgnoreThisRevision)
        .unwrap();
    let guard = f
        .library
        .repair_organization_references(&f.operation)
        .unwrap();
    std::fs::write(&f.source, b"changed again after repair").unwrap();
    assert!(
        f.library
            .acknowledge_organization_references_checked(&f.operation, &guard)
            .is_err()
    );
    drop(guard);
    assert_eq!(f.library.track_path("copy-track").unwrap(), f.source);
    assert_eq!(
        std::fs::read(&f.destination).unwrap(),
        b"captured managed audio"
    );
    let alternates = f.library.organization_alternates().unwrap();
    assert_eq!(alternates[0].state, "conflict");
    assert_eq!(
        alternates[0].observed_sha256.as_deref(),
        Some(cuerithm_media::file_sha256(&f.source).unwrap().as_str())
    );
    f.library
        .resolve_organization_alternate(&f.operation, AlternateResolution::IgnoreThisRevision)
        .unwrap();
    assert_eq!(f.library.track_path("copy-track").unwrap(), f.destination);
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        b"changed again after repair"
    );
}
#[test]
fn pending_copy_decision_survives_restart_but_a_new_original_identity_invalidates_it() {
    let f = Fixture::new();
    f.library
        .choose_pending_copy_resolution(&f.operation, AlternateResolution::AddAsNewIncoming)
        .unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let replacement = f.root.join("replacement.wav");
    std::fs::write(&replacement, b"changed original audio").unwrap();
    std::fs::rename(&replacement, &f.source).unwrap();
    assert!(
        reopened
            .repair_organization_references(&f.operation)
            .is_err(),
        "equal bytes from a different native object are not the approved revision"
    );
    assert_eq!(reopened.track_path("copy-track").unwrap(), f.source);
    let state: String = reopened
        .conn()
        .unwrap()
        .query_row(
            "SELECT state FROM organization_copy_conflicts WHERE operation_id=?1",
            [&f.operation],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(state, "pending");
    let job = reopened
        .resolve_organization_alternate(&f.operation, AlternateResolution::AddAsNewIncoming)
        .unwrap()
        .unwrap();
    assert_eq!(reopened.track_path("copy-track").unwrap(), f.destination);
    assert_eq!(
        reopened
            .jobs()
            .unwrap()
            .iter()
            .filter(|entry| entry.id == job)
            .count(),
        1
    );
}
#[test]
fn managed_destination_replacement_cannot_be_adopted_by_a_copy_decision() {
    let f = Fixture::new();
    let replacement = f.root.join("replacement.wav");
    std::fs::write(&replacement, b"captured managed audio").unwrap();
    std::fs::rename(&replacement, &f.destination).unwrap();
    assert!(
        f.library
            .resolve_organization_alternate(&f.operation, AlternateResolution::IgnoreThisRevision)
            .is_err()
    );
    assert_eq!(f.library.track_path("copy-track").unwrap(), f.source);
    assert_eq!(std::fs::read(&f.source).unwrap(), b"changed original audio");
    assert!(f.library.jobs().unwrap().is_empty());
}
#[test]
fn new_original_worker_may_change_source_after_ack_before_old_copy_cleanup() {
    let f = Fixture::new();
    f.library
        .choose_pending_copy_resolution(&f.operation, AlternateResolution::AddAsNewIncoming)
        .unwrap();
    let guard = f
        .library
        .repair_organization_references(&f.operation)
        .unwrap();
    f.library
        .acknowledge_organization_references_checked(&f.operation, &guard)
        .unwrap();
    drop(guard);
    assert_eq!(f.library.jobs().unwrap().len(), 1);
    std::fs::write(&f.source, b"new Incoming worker tagged original").unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let guard = reopened
        .verify_acknowledged_organization_references(&f.operation)
        .unwrap();
    reopened
        .execute_organization_checked(&f.operation, &guard)
        .unwrap();
    assert_eq!(
        reopened.organization_record(&f.operation).unwrap().phase,
        OrganizationPhase::Complete
    );
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        b"new Incoming worker tagged original"
    );
    assert_eq!(
        std::fs::read(&f.destination).unwrap(),
        b"captured managed audio"
    );
}
#[test]
fn full_manual_pending_copy_choices_continue_their_batch_after_restart() {
    use cuerithm_cleanup::{Metadata, MetadataField};
    for resolution in [
        AlternateResolution::IgnoreThisRevision,
        AlternateResolution::AddAsNewIncoming,
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let library = Library::open(&root.join("data/library.sqlite")).unwrap();
        let source = root.join("original.wav");
        std::fs::write(&source, b"captured managed audio").unwrap();
        let hash = cuerithm_media::file_sha256(&source).unwrap();
        let mut metadata = Metadata::default();
        metadata
            .fields
            .insert(MetadataField::Artist, vec!["Artist".into()]);
        metadata
            .fields
            .insert(MetadataField::Title, vec!["Captured".into()]);
        library.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,native_metadata_json,state,created,updated) VALUES('copy-track',?1,'Captured',?2,?3,'ready',1,1)", params![NativePath::new(&source).0, hash, serde_json::to_string(&metadata).unwrap()]).unwrap();
        let managed = root.join("managed");
        std::fs::create_dir(&managed).unwrap();
        let destination = managed.join("Artist - Captured.wav");
        let grant = library.register_organization_root(managed).unwrap();
        let preview = library
            .preview_organization(
                OrganizationPreviewRequest {
                    scope: OrganizationScope::Tracks {
                        track_ids: vec!["copy-track".into()],
                    },
                    settings: OrganizationSettings {
                        mode: OrganizationSettingMode::Copy,
                        destination_root_id: Some(grant.id),
                        ..Default::default()
                    },
                },
                &CancellationToken::default(),
            )
            .unwrap();
        let mut changed = false;
        let batch = library
            .apply_organization_preview_with(
                &preview.id,
                &preview.revision,
                &CancellationToken::default(),
                |boundary| {
                    if boundary == publish::Boundary::Published && !changed {
                        changed = true;
                        std::fs::write(&source, b"changed original audio")?;
                    }
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(batch.state, "recovery_required");
        let alternate = library.organization_alternates().unwrap().remove(0);
        assert_eq!(alternate.state, "conflict");
        drop(library);
        let reopened = Library::open(&root.join("data/library.sqlite")).unwrap();
        let job = reopened
            .resolve_organization_alternate_cancellable(
                &alternate.operation_id,
                resolution,
                &CancellationToken::default(),
            )
            .unwrap();
        let completed = reopened
            .organization_batches()
            .unwrap()
            .into_iter()
            .find(|entry| entry.id == batch.id)
            .unwrap();
        assert_eq!(completed.state, "complete");
        assert!(completed.error.is_none());
        assert_eq!(completed.totals.completed, 1);
        assert_eq!(reopened.track_path("copy-track").unwrap(), destination);
        assert_eq!(std::fs::read(&source).unwrap(), b"changed original audio");
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"captured managed audio"
        );
        if resolution == AlternateResolution::AddAsNewIncoming {
            assert!(job.is_some());
            assert_eq!(reopened.jobs().unwrap().len(), 1);
        } else {
            assert!(job.is_none());
        }
    }
}

#[test]
fn restoring_the_exact_captured_original_invalidates_an_old_choice_and_recovers_normally() {
    let f = Fixture::new();
    f.library
        .choose_pending_copy_resolution(&f.operation, AlternateResolution::AddAsNewIncoming)
        .unwrap();
    std::fs::write(&f.source, b"captured managed audio").unwrap();
    let guard = f
        .library
        .repair_organization_references(&f.operation)
        .unwrap();
    assert!(guard.source_proof.decision_digest.is_none());
    f.library
        .acknowledge_organization_references_checked(&f.operation, &guard)
        .unwrap();
    f.library
        .execute_organization_checked(&f.operation, &guard)
        .unwrap();
    assert_eq!(f.library.track_path("copy-track").unwrap(), f.destination);
    assert!(
        f.library.jobs().unwrap().is_empty(),
        "restored captured audio is not the formerly approved changed revision"
    );
    assert_eq!(
        f.library.organization_alternates().unwrap()[0].state,
        "active"
    );
    assert_eq!(std::fs::read(f.source).unwrap(), b"captured managed audio");
}

#[test]
fn pending_copy_add_decision_replays_across_ack_restart_without_duplicate_jobs() {
    let f = Fixture::new();
    f.library
        .choose_pending_copy_resolution(&f.operation, AlternateResolution::AddAsNewIncoming)
        .unwrap();
    drop(f.library);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let guard = reopened
        .repair_organization_references(&f.operation)
        .unwrap();
    reopened
        .acknowledge_organization_references_checked(&f.operation, &guard)
        .unwrap();
    let jobs = reopened.jobs().unwrap();
    assert_eq!(jobs.len(), 1);
    let queued = jobs[0].id.clone();
    assert_ne!(jobs[0].track_id, "copy-track");
    drop(guard);
    drop(reopened);
    let reopened = Library::open(&f.root.join("data/library.sqlite")).unwrap();
    let guard = reopened
        .verify_acknowledged_organization_references(&f.operation)
        .unwrap();
    reopened
        .execute_organization_checked(&f.operation, &guard)
        .unwrap();
    drop(guard);
    assert_eq!(
        reopened
            .resolve_organization_alternate(&f.operation, AlternateResolution::AddAsNewIncoming)
            .unwrap()
            .as_deref(),
        Some(queued.as_str())
    );
    assert_eq!(reopened.jobs().unwrap().len(), 1);
    assert_eq!(reopened.track_path("copy-track").unwrap(), f.destination);
    assert_eq!(std::fs::read(f.source).unwrap(), b"changed original audio");
}

#[test]
fn ignored_copy_revision_does_not_suppress_a_same_bytes_replacement_object() {
    let f = Fixture::new();
    f.library
        .resolve_organization_alternate(&f.operation, AlternateResolution::IgnoreThisRevision)
        .unwrap();
    let replacement = f.root.join("replacement.wav");
    std::fs::write(&replacement, b"changed original audio").unwrap();
    std::fs::rename(&replacement, &f.source).unwrap();
    let hash = cuerithm_media::file_sha256(&f.source).unwrap();
    assert!(matches!(
        f.library
            .organization_source_state(&f.source, &hash)
            .unwrap(),
        AlternateSourceState::Conflict { .. }
    ));
    f.library
        .resolve_organization_alternate(&f.operation, AlternateResolution::IgnoreThisRevision)
        .unwrap();
    assert!(matches!(
        f.library
            .organization_source_state(&f.source, &hash)
            .unwrap(),
        AlternateSourceState::Owned { .. }
    ));
    assert_eq!(
        std::fs::read(&f.destination).unwrap(),
        b"captured managed audio"
    );
}
