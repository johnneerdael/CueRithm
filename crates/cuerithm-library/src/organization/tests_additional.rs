use super::journal::OrganizationPhase;
use super::publish::Boundary;
use super::recovery::{AlternateResolution, AlternateSourceState};
struct Fixture {
    _temp: tempfile::TempDir,
    root: std::path::PathBuf,
    library: Library,
    id: String,
    source: std::path::PathBuf,
    target: std::path::PathBuf,
}
impl Fixture {
    fn new(mode: OrganizationMode, sidecar: bool, nested: bool) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let source = root.join("source.wav");
        let target = if nested {
            root.join("Artist/Album/renamed.wav")
        } else {
            root.join("renamed.wav")
        };
        std::fs::write(&source, b"audio plus opaque native DJ records").unwrap();
        let library = Library::open(&root.join("library.sqlite")).unwrap();
        let job = library
            .enqueue(
                std::slice::from_ref(&source),
                library.settings().unwrap().revision,
            )
            .unwrap()
            .remove(0);
        let track_id = library
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == job)
            .unwrap()
            .track_id;
        let mut files = vec![(source.clone(), target.clone())];
        if sidecar {
            let art = root.join("original.jpg");
            std::fs::write(&art, b"owned cover bytes").unwrap();
            files.push((art, target.with_extension("jpg")));
        }
        let id = library
            .plan_organization(OrganizationRequest {
                track_id,
                mode,
                source_root: root.clone(),
                destination_root: root.clone(),
                files,
            })
            .unwrap();
        Self {
            _temp: temp,
            root,
            library,
            id,
            source,
            target,
        }
    }
    fn finish(&self) {
        assert_eq!(
            self.library.execute_organization(&self.id).unwrap().phase,
            OrganizationPhase::AwaitingReferences
        );
        self.library
            .acknowledge_organization_references(&self.id)
            .unwrap();
        assert_eq!(
            self.library.execute_organization(&self.id).unwrap().phase,
            OrganizationPhase::Complete
        );
    }
}
#[test]
fn publication_waits_for_reference_commit_and_retains_the_original_backup() {
    let f = Fixture::new(OrganizationMode::Move, true, false);
    let record = f.library.execute_organization(&f.id).unwrap();
    assert_eq!(record.phase, OrganizationPhase::AwaitingReferences);
    assert!(f.source.exists());
    assert_eq!(
        std::fs::read(&f.target).unwrap(),
        b"audio plus opaque native DJ records"
    );
    assert_eq!(f.library.track_path(&record.track_id).unwrap(), f.source);
    f.library
        .acknowledge_organization_references(&f.id)
        .unwrap();
    let done = f.library.execute_organization(&f.id).unwrap();
    assert_eq!(done.phase, OrganizationPhase::Complete);
    assert!(!f.source.exists());
    assert_eq!(f.library.track_path(&record.track_id).unwrap(), f.target);
    assert_eq!(
        std::fs::read(f.root.join(&done.files[0].quarantine_name)).unwrap(),
        b"audio plus opaque native DJ records"
    );
    assert!(done.retained_bytes() > 0);
    assert_eq!(
        f.library
            .tracks(crate::TrackQuery {
                view: crate::TrackView::Incoming,
                ..Default::default()
            })
            .unwrap()
            .total,
        1,
        "organization must not mark Ready"
    );
}
#[test]
fn restart_recovers_every_staging_and_publication_boundary_without_deleting_sources() {
    for fault in [
        Boundary::StageReserved,
        Boundary::StageDurable,
        Boundary::Published,
    ] {
        let f = Fixture::new(OrganizationMode::Move, false, false);
        assert!(
            f.library
                .execute_organization_with(&f.id, |boundary| {
                    if boundary == fault {
                        anyhow::bail!("simulated process interruption")
                    }
                    Ok(())
                })
                .is_err()
        );
        let path = f.root.join("library.sqlite");
        let id = f.id.clone();
        let source = f.source.clone();
        let target = f.target.clone();
        drop(f.library);
        let reopened = Library::open(&path).unwrap();
        reopened.recover_organization().unwrap();
        let record = reopened.execute_organization(&id).unwrap();
        assert_eq!(
            record.phase,
            OrganizationPhase::AwaitingReferences,
            "{fault:?}"
        );
        assert!(source.exists());
        assert_eq!(
            std::fs::read(target).unwrap(),
            b"audio plus opaque native DJ records"
        );
    }
}
#[test]
fn cleanup_interruption_rolls_forward_and_undo_uses_a_reverse_journal() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.library.execute_organization(&f.id).unwrap();
    f.library
        .acknowledge_organization_references(&f.id)
        .unwrap();
    assert!(
        f.library
            .execute_organization_with(&f.id, |b| {
                if b == Boundary::SourceQuarantined {
                    anyhow::bail!("interrupted cleanup")
                }
                Ok(())
            })
            .is_err()
    );
    assert_eq!(
        f.library.execute_organization(&f.id).unwrap().phase,
        OrganizationPhase::Complete
    );
    let inverse = f.library.plan_organization_undo(&f.id).unwrap();
    f.library.execute_organization(&inverse).unwrap();
    f.library
        .acknowledge_organization_references(&inverse)
        .unwrap();
    f.library.execute_organization(&inverse).unwrap();
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        b"audio plus opaque native DJ records"
    );
    assert!(!f.target.exists());
    assert_eq!(
        f.library.organization_record(&f.id).unwrap().phase,
        OrganizationPhase::Undone
    );
}
#[test]
fn no_replace_publication_preserves_a_racing_destination_and_source() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    assert!(
        f.library
            .execute_organization_with(&f.id, |b| {
                if b == Boundary::StageDurable {
                    std::fs::write(&f.target, b"unrelated racing bytes")?;
                }
                Ok(())
            })
            .is_err()
    );
    assert_eq!(std::fs::read(&f.target).unwrap(), b"unrelated racing bytes");
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        b"audio plus opaque native DJ records"
    );
}
#[test]
fn changed_source_before_cleanup_is_never_deleted_or_marked_complete() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.library.execute_organization(&f.id).unwrap();
    f.library
        .acknowledge_organization_references(&f.id)
        .unwrap();
    std::fs::write(&f.source, b"a later edited original").unwrap();
    assert!(f.library.execute_organization(&f.id).is_err());
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        b"a later edited original"
    );
    assert_eq!(
        std::fs::read(&f.target).unwrap(),
        b"audio plus opaque native DJ records"
    );
}
#[test]
fn copy_alternate_conflict_resolution_is_revision_bound_and_new_audio_gets_a_new_track() {
    let f = Fixture::new(OrganizationMode::Copy, false, false);
    f.finish();
    let old = f.library.organization_record(&f.id).unwrap();
    assert_eq!(
        f.library
            .organization_source_state(&f.source, &old.files[0].original.sha256)
            .unwrap(),
        AlternateSourceState::Owned {
            track_id: old.track_id.clone()
        }
    );
    std::fs::write(&f.source, b"changed original revision one").unwrap();
    let hash = cuerithm_media::file_sha256(&f.source).unwrap();
    assert!(matches!(
        f.library
            .organization_source_state(&f.source, &hash)
            .unwrap(),
        AlternateSourceState::Conflict { .. }
    ));
    f.library
        .resolve_organization_alternate(&f.id, AlternateResolution::IgnoreThisRevision)
        .unwrap();
    assert!(matches!(
        f.library
            .organization_source_state(&f.source, &hash)
            .unwrap(),
        AlternateSourceState::Owned { .. }
    ));
    std::fs::write(&f.source, b"changed original revision two").unwrap();
    let hash = cuerithm_media::file_sha256(&f.source).unwrap();
    assert!(matches!(
        f.library
            .organization_source_state(&f.source, &hash)
            .unwrap(),
        AlternateSourceState::Conflict { .. }
    ));
    let job = f
        .library
        .resolve_organization_alternate(&f.id, AlternateResolution::AddAsNewIncoming)
        .unwrap()
        .unwrap();
    let new = f
        .library
        .jobs()
        .unwrap()
        .into_iter()
        .find(|j| j.id == job)
        .unwrap();
    assert_ne!(new.track_id, old.track_id);
    assert_eq!(f.library.track_path(&old.track_id).unwrap(), f.target);
    assert_eq!(f.library.track_path(&new.track_id).unwrap(), f.source);
}
#[test]
fn copy_undo_requires_unchanged_original_and_never_overwrites_it() {
    let f = Fixture::new(OrganizationMode::Copy, false, false);
    f.finish();
    std::fs::write(&f.source, b"later original edit").unwrap();
    assert!(f.library.plan_organization_undo(&f.id).is_err());
    assert_eq!(std::fs::read(&f.source).unwrap(), b"later original edit");
    assert!(f.target.exists());
}
#[test]
fn copy_undo_rebinds_original_and_quarantines_only_the_verified_managed_copy() {
    let f = Fixture::new(OrganizationMode::Copy, true, false);
    f.finish();
    let inverse = f.library.plan_organization_undo(&f.id).unwrap();
    f.library
        .acknowledge_organization_references(&inverse)
        .unwrap();
    f.library.execute_organization(&inverse).unwrap();
    assert!(f.source.exists());
    assert!(!f.target.exists());
}
#[test]
fn owned_sidecars_share_journaled_nested_directories() {
    let f = Fixture::new(OrganizationMode::Move, true, true);
    f.finish();
    assert_eq!(
        std::fs::read(&f.target).unwrap(),
        b"audio plus opaque native DJ records"
    );
    assert_eq!(
        std::fs::read(f.target.with_extension("jpg")).unwrap(),
        b"owned cover bytes"
    );
}
#[test]
fn unknown_directory_from_interrupted_creation_is_preserved_for_explicit_replanning() {
    let f = Fixture::new(OrganizationMode::Move, false, true);
    assert!(
        f.library
            .execute_organization_with(&f.id, |b| {
                if b == Boundary::DirectoryCreated {
                    anyhow::bail!("interrupted new directory journal")
                }
                Ok(())
            })
            .is_err()
    );
    assert!(f.library.execute_organization(&f.id).is_err());
    assert!(f.root.join("Artist").is_dir());
    assert!(f.source.exists());
    f.library.cancel_unstarted_organization(&f.id).unwrap();
}
#[test]
fn undo_refuses_a_new_file_at_the_original_path() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.finish();
    std::fs::write(&f.source, b"a newly created independent recording").unwrap();
    assert!(f.library.plan_organization_undo(&f.id).is_err());
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        b"a newly created independent recording"
    );
}
#[cfg(unix)]
#[test]
fn parent_replaced_with_symlink_after_planning_never_redirects_publication() {
    let f = Fixture::new(OrganizationMode::Move, false, true);
    std::fs::create_dir(f.root.join("outside")).unwrap();
    std::os::unix::fs::symlink(f.root.join("outside"), f.root.join("Artist")).unwrap();
    assert!(f.library.execute_organization(&f.id).is_err());
    assert!(!f.root.join("outside/Album/renamed.wav").exists());
    assert!(f.source.exists());
}
#[test]
fn changing_destination_parent_after_staging_blocks_publication() {
    let f = Fixture::new(OrganizationMode::Move, false, true);
    assert!(
        f.library
            .execute_organization_with(&f.id, |b| {
                if b == Boundary::StageDurable {
                    anyhow::bail!("hold before publication")
                }
                Ok(())
            })
            .is_err()
    );
    std::fs::rename(f.root.join("Artist"), f.root.join("replaced-artist")).unwrap();
    std::fs::create_dir_all(f.root.join("Artist/Album")).unwrap();
    assert!(f.library.execute_organization(&f.id).is_err());
    assert!(!f.target.exists());
    assert!(f.source.exists());
    assert!(
        std::fs::read_dir(f.root.join("replaced-artist/Album"))
            .unwrap()
            .next()
            .is_some(),
        "staged bytes retained under the captured old directory"
    );
}
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_native_names_round_trip_through_restart_journal() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.library.cancel_unstarted_organization(&f.id).unwrap();
    let target = f
        .root
        .join(std::ffi::OsString::from_vec(b"renamed-\xff.wav".to_vec()));
    let track_id = f.library.organization_record(&f.id).unwrap().track_id;
    let id = f
        .library
        .plan_organization(OrganizationRequest {
            track_id,
            mode: OrganizationMode::Move,
            source_root: f.root.clone(),
            destination_root: f.root.clone(),
            files: vec![(f.source.clone(), target.clone())],
        })
        .unwrap();
    f.library.execute_organization(&id).unwrap();
    f.library.acknowledge_organization_references(&id).unwrap();
    f.library.execute_organization(&id).unwrap();
    assert_eq!(
        std::fs::read(target).unwrap(),
        b"audio plus opaque native DJ records"
    );
}

#[test]
fn both_watch_and_manual_enqueue_suppress_a_published_copy_and_surface_changed_originals() {
    let f = Fixture::new(OrganizationMode::Copy, false, false);
    f.library.execute_organization(&f.id).unwrap();
    let original_job = f.library.jobs().unwrap()[0].id.clone();
    let revision = f.library.settings().unwrap().revision;
    let published = f
        .library
        .enqueue_discovered(std::slice::from_ref(&f.target), revision.clone())
        .unwrap();
    assert_eq!(
        published,
        vec![original_job.clone()],
        "a published output must not gain a second library identity before path commit"
    );
    f.library
        .acknowledge_organization_references(&f.id)
        .unwrap();
    f.library.execute_organization(&f.id).unwrap();
    for discovered in [false, true] {
        let ids = if discovered {
            f.library
                .enqueue_discovered(std::slice::from_ref(&f.source), revision.clone())
        } else {
            f.library
                .enqueue(std::slice::from_ref(&f.source), revision.clone())
        }
        .unwrap();
        assert_eq!(ids, vec![original_job.clone()]);
    }
    std::fs::write(&f.source, b"changed original must be reviewed").unwrap();
    assert!(
        f.library
            .enqueue(std::slice::from_ref(&f.source), revision)
            .is_err()
    );
    assert_eq!(f.library.jobs().unwrap().len(), 1);
}

#[test]
fn same_native_path_is_an_unchanged_intent_and_does_not_rewrite_audio() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.library.cancel_unstarted_organization(&f.id).unwrap();
    let track_id = f.library.organization_record(&f.id).unwrap().track_id;
    let id = f
        .library
        .plan_organization(OrganizationRequest {
            track_id,
            mode: OrganizationMode::Rename,
            source_root: f.root.clone(),
            destination_root: f.root.clone(),
            files: vec![(f.source.clone(), f.source.clone())],
        })
        .unwrap();
    let record = f.library.execute_organization(&id).unwrap();
    assert_eq!(record.phase, OrganizationPhase::Unchanged);
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        b"audio plus opaque native DJ records"
    );
    assert_eq!(
        std::fs::read_dir(&f.root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(".cuerithm-"))
            .count(),
        0
    );
}
#[cfg(unix)]
#[test]
fn native_permission_failure_retains_original_and_can_retry_after_permissions_are_restored() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(OrganizationMode::Move, false, false);
    std::fs::set_permissions(&f.root, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = f.library.execute_organization(&f.id);
    std::fs::set_permissions(&f.root, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(f.source.exists());
    assert!(!f.target.exists());
    f.finish();
}
#[cfg(windows)]
#[test]
fn windows_junction_parent_is_rejected_without_following_it() {
    let f = Fixture::new(OrganizationMode::Move, false, true);
    let outside = f.root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(f.root.join("Artist"))
        .arg(&outside)
        .status()
        .unwrap();
    assert!(
        status.success(),
        "native junction fixture could not be created"
    );
    assert!(f.library.execute_organization(&f.id).is_err());
    assert!(!outside.join("Album/renamed.wav").exists());
    assert!(f.source.exists());
}

#[test]
fn copy_to_a_case_alias_cannot_remove_the_original() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.library.cancel_unstarted_organization(&f.id).unwrap();
    let target = f.root.join("SOURCE.wav");
    if !target.exists() {
        return;
    }
    let track_id = f.library.organization_record(&f.id).unwrap().track_id;
    let planned = f.library.plan_organization(OrganizationRequest {
        track_id,
        mode: OrganizationMode::Copy,
        source_root: f.root.clone(),
        destination_root: f.root.clone(),
        files: vec![(f.source.clone(), target)],
    });
    assert!(
        planned.is_err(),
        "copy mode cannot fulfill retaining the original when source and destination are a case alias"
    );
}
#[test]
fn hard_link_alias_is_not_mistaken_for_a_case_only_rename() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.library.cancel_unstarted_organization(&f.id).unwrap();
    let target = f.root.join("SOURCE.wav");
    if target.exists() {
        return;
    }
    std::fs::hard_link(&f.source, &target).unwrap();
    let track_id = f.library.organization_record(&f.id).unwrap().track_id;
    let planned = f.library.plan_organization(OrganizationRequest {
        track_id,
        mode: OrganizationMode::Rename,
        source_root: f.root.clone(),
        destination_root: f.root.clone(),
        files: vec![(f.source.clone(), target.clone())],
    });
    assert!(
        planned.is_err(),
        "two separate hard-link names cannot authorize quarantining the source before discovering the occupied target"
    );
    assert!(f.source.exists());
    assert!(target.exists());
}

#[test]
fn undo_intent_and_its_history_link_are_persisted_atomically() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.finish();
    f.library.conn().unwrap().execute_batch("CREATE TRIGGER fail_undo_journal BEFORE INSERT ON organization_operations WHEN json_extract(NEW.payload_json,'$.undo_of') IS NOT NULL BEGIN SELECT RAISE(ABORT,'injected inverse journal failure'); END;").unwrap();
    assert!(f.library.plan_organization_undo(&f.id).is_err());
    assert_eq!(
        f.library.organization_history().unwrap().len(),
        1,
        "a failed inverse journal must not leave an unrelated orphan operation"
    );
    assert!(!f.source.exists());
    assert!(f.target.exists());
}

#[test]
fn read_only_audio_keeps_its_permissions_through_publication_and_recovery() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    let mut permissions = std::fs::metadata(&f.source).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&f.source, permissions).unwrap();
    f.finish();
    let record = f.library.organization_record(&f.id).unwrap();
    let backup = f.root.join(&record.files[0].quarantine_name);
    assert!(
        std::fs::metadata(&f.target)
            .unwrap()
            .permissions()
            .readonly()
    );
    assert!(std::fs::metadata(&backup).unwrap().permissions().readonly());
    assert_eq!(
        std::fs::read(&f.target).unwrap(),
        b"audio plus opaque native DJ records"
    );
    f.library.recover_organization().unwrap();
    #[cfg(windows)]
    // Windows read-only files must be made writable so the fixture can be removed.
    #[allow(clippy::permissions_set_readonly_false)]
    for path in [&f.target, &backup] {
        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(path, permissions).unwrap();
    }
}

#[test]
fn cancelling_an_unstarted_undo_keeps_the_original_operation_complete() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.finish();
    let inverse = f.library.plan_organization_undo(&f.id).unwrap();
    f.library.cancel_unstarted_organization(&inverse).unwrap();
    assert_eq!(
        f.library.organization_record(&f.id).unwrap().phase,
        OrganizationPhase::Complete
    );
    assert_eq!(
        f.library.organization_record(&inverse).unwrap().phase,
        OrganizationPhase::Cancelled
    );
    assert!(f.library.plan_organization_undo(&f.id).is_ok());
    assert!(!f.source.exists());
    assert!(f.target.exists());
}

#[test]
fn restart_does_not_resurrect_a_complete_original_from_a_stale_history_snapshot() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    f.finish();
    let inverse = f.library.plan_organization_undo(&f.id).unwrap();
    f.library.execute_organization(&inverse).unwrap();
    f.library
        .acknowledge_organization_references(&inverse)
        .unwrap();
    f.library
        .conn()
        .unwrap()
        .execute(
            "UPDATE organization_operations SET created=CASE WHEN id=?1 THEN 0 ELSE 100 END",
            [&inverse],
        )
        .unwrap();
    let path = f.root.join("library.sqlite");
    let original = f.id.clone();
    drop(f.library);
    let reopened = Library::open(&path).unwrap();
    assert_eq!(
        reopened.organization_record(&inverse).unwrap().phase,
        OrganizationPhase::Complete
    );
    assert_eq!(
        reopened.organization_record(&original).unwrap().phase,
        OrganizationPhase::Undone
    );
    assert!(reopened.plan_organization_undo(&original).is_err());
}

#[test]
fn ownership_hash_validation_does_not_hold_the_library_connection() {
    use std::sync::{Arc, Barrier};
    let f = Fixture::new(OrganizationMode::Copy, false, false);
    f.finish();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let library = f.library.clone();
    let path = f.source.clone();
    let hash = f.library.organization_record(&f.id).unwrap().files[0]
        .original
        .sha256
        .clone();
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let worker = std::thread::spawn(move || {
        library.organization_source_state_with(&path, &hash, |operation, path, hash| {
            worker_entered.wait();
            worker_release.wait();
            super::recovery::owned_revision(operation, path, hash)
        })
    });
    entered.wait();
    let connection_available = f.library.inner.conn.try_lock().is_ok();
    release.wait();
    assert!(matches!(
        worker.join().unwrap().unwrap(),
        AlternateSourceState::Owned { .. }
    ));
    assert!(
        connection_available,
        "the library connection must stay available while filesystem ownership hashes are read"
    );
}

#[test]
fn ownership_revalidation_cannot_resurrect_an_alternate_retired_by_undo() {
    use std::sync::{Arc, Barrier};
    let f = Fixture::new(OrganizationMode::Copy, false, false);
    f.finish();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let library = f.library.clone();
    let path = f.source.clone();
    let worker_entered = entered.clone();
    let worker_release = release.clone();
    let worker = std::thread::spawn(move || {
        let mut first = true;
        library.organization_source_state_with(
            &path,
            "an-obsolete-content-revision",
            |operation, path, hash| {
                if first {
                    first = false;
                    worker_entered.wait();
                    worker_release.wait();
                }
                super::recovery::owned_revision(operation, path, hash)
            },
        )
    });
    entered.wait();
    let inverse = f.library.plan_organization_undo(&f.id).unwrap();
    f.library
        .acknowledge_organization_references(&inverse)
        .unwrap();
    release.wait();
    assert!(
        matches!(worker.join().unwrap().unwrap(),AlternateSourceState::Conflict{operation_id,..} if operation_id==inverse)
    );
    assert_eq!(
        f.library.organization_alternates().unwrap()[0].state,
        "retired"
    );
}

#[test]
fn an_unpublished_intent_can_be_cancelled_after_the_input_changes() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    std::fs::write(&f.source, b"user changed the input before any publication").unwrap();
    assert!(f.library.execute_organization(&f.id).is_err());
    f.library.cancel_unstarted_organization(&f.id).unwrap();
    assert_eq!(
        f.library.organization_record(&f.id).unwrap().phase,
        OrganizationPhase::Cancelled
    );
    assert_eq!(
        std::fs::read(&f.source).unwrap(),
        b"user changed the input before any publication"
    );
    assert!(!f.target.exists());
    let old = f.library.jobs().unwrap()[0].id.clone();
    let new = f
        .library
        .enqueue(
            std::slice::from_ref(&f.source),
            f.library.settings().unwrap().revision,
        )
        .unwrap()[0]
        .clone();
    assert_ne!(new, old, "changed audio must get a fresh preparation job");
    assert_eq!(
        f.library
            .tracks(crate::TrackQuery {
                view: crate::TrackView::Incoming,
                ..Default::default()
            })
            .unwrap()
            .total,
        1
    );
}

#[test]
fn cancellation_detects_publication_that_reached_disk_before_its_journal_update() {
    let f = Fixture::new(OrganizationMode::Move, false, false);
    assert!(
        f.library
            .execute_organization_with(&f.id, |b| {
                if b == Boundary::Published {
                    anyhow::bail!("interrupted before publication journal")
                }
                Ok(())
            })
            .is_err()
    );
    assert!(
        f.library.cancel_unstarted_organization(&f.id).is_err(),
        "publication on disk must remain recoverable even before its journal flag is saved"
    );
    assert!(f.source.exists());
    assert!(f.target.exists());
    f.finish();
}
