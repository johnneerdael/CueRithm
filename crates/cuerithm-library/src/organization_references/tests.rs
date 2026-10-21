use super::*;
use crate::organization::{OrganizationMode, OrganizationRequest};
use std::sync::mpsc;

// Native DJ databases store ordinary paths; keep canonical filesystem paths in
// the library and only project strings when authoring or inspecting native XML.
fn native_reference(path: &std::path::Path) -> String {
    cuerithm_dj_sync::destination_reference(path)
        .to_str()
        .unwrap()
        .to_owned()
}
fn fixture() -> (tempfile::TempDir, Library, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let library = Library::open(&root.join("library.db")).unwrap();
    let source = root.join("old.wav");
    let destination = root.join("managed/new.wav");
    fs::write(&source, b"invented audio fixture").unwrap();
    fs::create_dir(destination.parent().unwrap()).unwrap();
    let hash = file_hash(&source).unwrap().unwrap();
    library.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('track',?1,'Fixture',?2,'incoming',1,1)",params![NativePath::new(&source).0,hash]).unwrap();
    (dir, library, source, destination)
}
#[cfg(unix)]
#[test]
fn receipt_digest_preserves_raw_native_paths() {
    use std::os::unix::ffi::OsStringExt;
    let one = PathBuf::from(std::ffi::OsString::from_vec(b"/native/\xff".to_vec()));
    let two = PathBuf::from(std::ffi::OsString::from_vec(b"/native/\xfe".to_vec()));
    let files = std::collections::BTreeMap::from([(one.clone(), Some("hash".into()))]);
    let directories = std::collections::BTreeMap::from([(two.clone(), Some(Vec::new()))]);
    let captured = receipt_digest(&files, "registration", &directories).unwrap();
    assert_eq!(
        captured,
        receipt_digest(&files, "registration", &directories).unwrap()
    );
    let different = std::collections::BTreeMap::from([(two, Some("hash".into()))]);
    assert_ne!(
        captured,
        receipt_digest(&different, "registration", &directories).unwrap()
    );
}
#[cfg(target_os = "linux")]
#[test]
fn independent_copy_history_keeps_raw_source_paths_losslessly() {
    use std::os::unix::ffi::OsStringExt;
    let (dir, library, source, destination) = fixture();
    let raw = source
        .parent()
        .unwrap()
        .join(std::ffi::OsString::from_vec(b"raw-\xff.wav".to_vec()));
    fs::rename(&source, &raw).unwrap();
    library
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET path=?1 WHERE id='track'",
            [NativePath::new(&raw).0],
        )
        .unwrap();
    let usb = dir.path().canonicalize().unwrap().join("USB");
    fs::create_dir(&usb).unwrap();
    library
        .seed_organization_reference_fixture(
            "track",
            SyncTarget {
                environment: Default::default(),
                software: cuerithm_dj_sync::DjSoftware::PioneerUsb,
                path: usb,
                source_root: None,
            },
            true,
        )
        .unwrap();
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    assert!(snapshot.blockers.is_empty());
    let operation = planned(&library, &raw, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    assert_eq!(library.track_path("track").unwrap(), destination);
    assert!(raw.is_file());
}
fn planned(library: &Library, source: &std::path::Path, destination: &std::path::Path) -> String {
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    assert!(snapshot.blockers.is_empty(), "{:?}", snapshot.blockers);
    let id = library
        .plan_organization(OrganizationRequest {
            track_id: "track".into(),
            mode: OrganizationMode::Move,
            source_root: source.parent().unwrap().into(),
            destination_root: destination.parent().unwrap().into(),
            files: vec![(source.into(), destination.into())],
        })
        .unwrap();
    library
        .bind_organization_references(&id, &snapshot.digest)
        .unwrap();
    library.execute_organization(&id).unwrap();
    id
}
#[test]
fn slow_reference_verifier_does_not_hold_library_sqlite_connection() {
    let (_dir, library, source, destination) = fixture();
    let operation = planned(&library, &source, &destination);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (query_tx, query_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        let lib = &library;
        let op = &operation;
        let worker = scope.spawn(move || {
            lib.repair_organization_references_with(op, move || {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            })
        });
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        let query = scope.spawn(|| {
            query_tx.send(library.revision().is_ok()).unwrap();
        });
        let available = query_rx
            .recv_timeout(std::time::Duration::from_millis(300))
            .unwrap_or(false);
        release_tx.send(()).unwrap();
        let _guard = worker.join().unwrap().unwrap();
        query.join().unwrap();
        assert!(available, "Reference verification held Library::conn");
    });
    assert!(source.is_file());
    assert!(destination.is_file());
}
#[test]
fn unlinked_incoming_track_never_needs_a_ready_projection() {
    let (_dir, library, source, destination) = fixture();
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    assert_eq!(library.track_path("track").unwrap(), source);
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    assert_eq!(library.track_path("track").unwrap(), destination);
    let state: String = library
        .conn()
        .unwrap()
        .query_row("SELECT state FROM tracks WHERE id='track'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(state, "incoming");
    assert!(source.is_file());
}
#[test]
fn linked_reference_repairs_only_path_before_local_ack_and_preserves_remote_edits() {
    let (dir, library, source, destination) = fixture();
    let root = dir.path().canonicalize().unwrap();
    let database = root.join("database.xml");
    let original = format!(
        r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{}"><Tags Author="Remote artist"/><Scan BeatGrid="remote grid"/><Poi Type="cue" Pos="99"/></Song></VirtualDJ_Database>"#,
        native_reference(&source)
    );
    #[cfg(windows)]
    assert!(!original.contains(r"\\?\"));
    fs::write(&database, &original).unwrap();
    let target = SyncTarget {
        environment: Default::default(),
        software: cuerithm_dj_sync::DjSoftware::VirtualDj,
        path: database.clone(),
        source_root: None,
    };
    library
        .seed_organization_reference_fixture("track", target, false)
        .unwrap();
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    assert_eq!(snapshot.references[0].state, "required");
    assert!(snapshot.estimated_bytes > 0);
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    assert!(source.is_file());
    assert_eq!(library.track_path("track").unwrap(), source);
    let native = fs::read_to_string(&database).unwrap();
    assert!(native.contains(&native_reference(&destination)));
    for value in ["Remote artist", "remote grid", "99"] {
        assert!(native.contains(value));
    }
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    assert_eq!(library.track_path("track").unwrap(), destination);
    assert!(source.is_file());
}

#[test]
fn normal_sync_cannot_replace_native_receipts_during_interrupted_cleanup() {
    let (dir, library, source, destination) = fixture();
    let database = dir.path().canonicalize().unwrap().join("database.xml");
    let target = linked_target(&library, &source, &database);
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    assert!(
        library
            .ensure_organization_references_settled(&target)
            .is_err()
    );
    library
        .execute_organization_checked(&operation, &guard)
        .unwrap();
    assert!(
        library
            .ensure_organization_references_settled(&target)
            .is_ok()
    );
}

fn linked_target(
    library: &Library,
    source: &std::path::Path,
    path: &std::path::Path,
) -> SyncTarget {
    let body = format!(
        r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{}"><Tags Author="Remote artist"/><Scan BeatGrid="remote grid"/><Poi Type="cue" Pos="99"/></Song></VirtualDJ_Database>"#,
        native_reference(source)
    );
    fs::write(path, body).unwrap();
    let target = SyncTarget {
        environment: Default::default(),
        software: cuerithm_dj_sync::DjSoftware::VirtualDj,
        path: path.into(),
        source_root: None,
    };
    library
        .seed_organization_reference_fixture("track", target.clone(), false)
        .unwrap();
    target
}
#[test]
fn external_edit_after_preview_blocks_before_audio_publication() {
    let (dir, library, source, destination) = fixture();
    let database = dir.path().canonicalize().unwrap().join("database.xml");
    linked_target(&library, &source, &database);
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    let changed = fs::read_to_string(&database)
        .unwrap()
        .replace("Remote artist", "External changed");
    fs::write(&database, &changed).unwrap();
    let operation = library
        .plan_organization(OrganizationRequest {
            track_id: "track".into(),
            mode: OrganizationMode::Move,
            source_root: source.parent().unwrap().into(),
            destination_root: destination.parent().unwrap().into(),
            files: vec![(source.clone(), destination.clone())],
        })
        .unwrap();
    assert!(
        library
            .bind_organization_references(&operation, &snapshot.digest)
            .is_err()
    );
    assert!(!destination.exists());
    assert!(source.is_file());
    assert_eq!(fs::read_to_string(database).unwrap(), changed);
}

#[test]
fn required_offline_target_is_a_visible_blocker_before_publication() {
    let (dir, library, source, destination) = fixture();
    let database = dir.path().canonicalize().unwrap().join("database.xml");
    linked_target(&library, &source, &database);
    fs::rename(&database, database.with_extension("offline")).unwrap();
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    assert!(!snapshot.blockers.is_empty());
    assert_eq!(snapshot.references[0].state, "blocked");
    assert!(snapshot.references[0].reason.is_some());
    assert!(source.is_file());
    assert!(!destination.exists());
}

#[test]
fn a_byte_identical_replaced_native_target_invalidates_the_frozen_reference() {
    let (dir, library, source, destination) = fixture();
    let database = dir.path().canonicalize().unwrap().join("database.xml");
    linked_target(&library, &source, &database);
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    let replacement = database.with_extension("replacement");
    fs::copy(&database, &replacement).unwrap();
    fs::rename(replacement, &database).unwrap();
    let operation = library
        .plan_organization(OrganizationRequest {
            track_id: "track".into(),
            mode: OrganizationMode::Move,
            source_root: source.parent().unwrap().into(),
            destination_root: destination.parent().unwrap().into(),
            files: vec![(source.clone(), destination.clone())],
        })
        .unwrap();
    assert!(
        library
            .bind_organization_references(&operation, &snapshot.digest)
            .is_err()
    );
    assert!(source.is_file());
    assert!(!destination.exists());
}
#[test]
fn partial_multi_target_repair_retains_original_and_retry_does_not_republish_completed_target() {
    let (dir, library, source, destination) = fixture();
    let root = dir.path().canonicalize().unwrap();
    linked_target(&library, &source, &root.join("one.xml"));
    linked_target(&library, &source, &root.join("two.xml"));
    let capture = library.capture_organization_references("track").unwrap();
    let first = capture.references[0].known.target.path.clone();
    let second = capture.references[1].known.target.path.clone();
    let before_second = fs::read(&second).unwrap();
    let operation = planned(&library, &source, &destination);
    let result = library.repair_organization_references_hooks(
        &operation,
        || {},
        |index| {
            if index == 1 {
                let changed = String::from_utf8(before_second.clone())
                    .unwrap()
                    .replace("Remote artist", "Changed during repair");
                fs::write(&second, changed).unwrap();
            }
        },
    );
    assert!(result.is_err());
    assert!(source.is_file());
    assert!(destination.is_file());
    assert_eq!(library.track_path("track").unwrap(), source);
    assert!(
        fs::read_to_string(&first)
            .unwrap()
            .contains(&native_reference(&destination))
    );
    assert!(
        fs::read_to_string(&second)
            .unwrap()
            .contains("Changed during repair")
    );
    let first_hash = file_hash(&first).unwrap();
    fs::write(&second, &before_second).unwrap();
    let guard = library.repair_organization_references(&operation).unwrap();
    assert_eq!(file_hash(&first).unwrap(), first_hash);
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    assert_eq!(library.track_path("track").unwrap(), destination);
    assert!(source.is_file());
}
#[test]
fn independent_device_copy_does_not_require_a_mounted_or_closed_target() {
    let (dir, library, source, destination) = fixture();
    let root = dir.path().canonicalize().unwrap();
    let usb = root.join("USB");
    fs::create_dir(&usb).unwrap();
    fs::write(usb.join("owned-media.bin"), b"independent device bytes").unwrap();
    let target = SyncTarget {
        environment: Default::default(),
        software: cuerithm_dj_sync::DjSoftware::PioneerUsb,
        path: usb.clone(),
        source_root: None,
    };
    library
        .seed_organization_reference_fixture("track", target, true)
        .unwrap();
    let offline = root.join("unmounted");
    fs::rename(usb, &offline).unwrap();
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    assert!(snapshot.blockers.is_empty());
    assert_eq!(snapshot.references[0].state, "independent_copy");
    assert_eq!(snapshot.estimated_bytes, 0);
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    assert_eq!(
        fs::read(offline.join("owned-media.bin")).unwrap(),
        b"independent device bytes"
    );
}
#[test]
fn imported_rekordbox_and_legacy_links_are_visible_blockers() {
    let (_dir, library, _source, _destination) = fixture();
    let conn = library.conn().unwrap();
    conn.execute(
        "INSERT INTO sync_links VALUES('track','unregistered-native','external-id','{}')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO native_targets VALUES('native','rekordbox','fixture-db','','{}',?1,'observation','[]',1,1,1)",["0".repeat(64)]).unwrap();
    conn.execute("INSERT INTO native_track_links(target_id,content_id,local_track_id,identity_json,baseline_json,status,updated) VALUES('native','content','track','{}','{}','observed',1)",[]).unwrap();
    drop(conn);
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    assert_eq!(snapshot.blockers.len(), 2);
    assert_eq!(snapshot.references.len(), 2);
    assert!(
        snapshot
            .references
            .iter()
            .all(|reference| reference.state == "blocked")
    );
    assert!(
        snapshot
            .blockers
            .iter()
            .any(|b| b.contains("Imported Rekordbox"))
    );
}
#[test]
fn acknowledged_target_edit_blocks_cleanup_verification_without_overwrite() {
    let (dir, library, source, destination) = fixture();
    let database = dir.path().canonicalize().unwrap().join("database.xml");
    linked_target(&library, &source, &database);
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    drop(guard);
    let changed = fs::read_to_string(&database)
        .unwrap()
        .replace(&native_reference(&destination), &native_reference(&source));
    fs::write(&database, &changed).unwrap();
    let library_path = dir.path().canonicalize().unwrap().join("library.db");
    drop(library);
    let library = Library::open(&library_path).unwrap();
    assert!(
        library
            .verify_acknowledged_organization_references(&operation)
            .is_err()
    );
    assert!(source.is_file());
    assert!(destination.is_file());
    assert_eq!(fs::read_to_string(database).unwrap(), changed);
}
#[test]
fn a_retained_reference_guard_cannot_acknowledge_a_later_external_native_edit() {
    let (dir, library, source, destination) = fixture();
    let database = dir.path().canonicalize().unwrap().join("database.xml");
    linked_target(&library, &source, &database);
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    let changed = fs::read_to_string(&database)
        .unwrap()
        .replace(&native_reference(&destination), &native_reference(&source));
    fs::write(&database, &changed).unwrap();
    assert!(
        library
            .acknowledge_organization_references_checked(&operation, &guard)
            .is_err()
    );
    assert_eq!(library.track_path("track").unwrap(), source);
    assert!(source.is_file());
    assert!(destination.is_file());
    assert_eq!(fs::read_to_string(database).unwrap(), changed);
}
#[test]
fn a_retained_reference_guard_cannot_retire_source_after_a_later_external_native_edit() {
    let (dir, library, source, destination) = fixture();
    let database = dir.path().canonicalize().unwrap().join("database.xml");
    linked_target(&library, &source, &database);
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    let changed = fs::read_to_string(&database)
        .unwrap()
        .replace(&native_reference(&destination), &native_reference(&source));
    fs::write(&database, &changed).unwrap();
    assert!(
        library
            .execute_organization_checked(&operation, &guard)
            .is_err()
    );
    assert_eq!(library.track_path("track").unwrap(), destination);
    assert!(source.is_file());
    assert!(destination.is_file());
    assert_eq!(fs::read_to_string(database).unwrap(), changed);
}
#[test]
fn a_verified_own_receipt_can_rebase_another_frozen_track_snapshot() {
    let (dir, library, source, destination) = fixture();
    let root = dir.path().canonicalize().unwrap();
    let second = root.join("second.wav");
    let second_destination = root.join("managed/second.wav");
    fs::write(&second, b"second invented audio").unwrap();
    let hash = file_hash(&second).unwrap().unwrap();
    library.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('second',?1,'Second',?2,'incoming',1,1)",params![NativePath::new(&second).0,hash]).unwrap();
    let database = root.join("database.xml");
    fs::write(&database,format!(r#"<VirtualDJ_Database Version="8.5"><Song FilePath="{}"/><Song FilePath="{}"/></VirtualDJ_Database>"#,native_reference(&source),native_reference(&second))).unwrap();
    let target = SyncTarget {
        environment: Default::default(),
        software: cuerithm_dj_sync::DjSoftware::VirtualDj,
        path: database.clone(),
        source_root: None,
    };
    library
        .seed_organization_reference_fixture("track", target.clone(), false)
        .unwrap();
    library
        .seed_organization_reference_fixture("second", target, false)
        .unwrap();
    let frozen = library.organization_reference_snapshot("second").unwrap();
    let first = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&first).unwrap();
    library
        .acknowledge_organization_references_checked(&first, &guard)
        .unwrap();
    drop(guard);
    let operation = library
        .plan_organization(OrganizationRequest {
            track_id: "second".into(),
            mode: OrganizationMode::Move,
            source_root: second.parent().unwrap().into(),
            destination_root: second_destination.parent().unwrap().into(),
            files: vec![(second.clone(), second_destination.clone())],
        })
        .unwrap();
    library
        .bind_organization_references(&operation, &frozen.digest)
        .unwrap();
    library.execute_organization(&operation).unwrap();
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    let native = fs::read_to_string(database).unwrap();
    assert!(native.contains(&native_reference(&destination)));
    assert!(native.contains(&native_reference(&second_destination)));
    assert!(source.is_file());
    assert!(second.is_file());
    drop(guard);
    let cleanup = library
        .verify_acknowledged_organization_references(&first)
        .unwrap();
    library
        .execute_organization_checked(&first, &cleanup)
        .unwrap();
    assert!(!source.exists());
    assert!(second.exists());
}

#[test]
fn copy_inverse_rebinds_to_owned_original_and_preserves_later_native_edits() {
    let (dir, library, source, destination) = fixture();
    let database = dir.path().canonicalize().unwrap().join("database.xml");
    linked_target(&library, &source, &database);
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    let operation = library
        .plan_organization(OrganizationRequest {
            track_id: "track".into(),
            mode: OrganizationMode::Copy,
            source_root: source.parent().unwrap().into(),
            destination_root: destination.parent().unwrap().into(),
            files: vec![(source.clone(), destination.clone())],
        })
        .unwrap();
    library
        .bind_organization_references(&operation, &snapshot.digest)
        .unwrap();
    library.execute_organization(&operation).unwrap();
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    library
        .execute_organization_checked(&operation, &guard)
        .unwrap();
    drop(guard);
    let changed = fs::read_to_string(&database)
        .unwrap()
        .replace("Remote artist", "Edited after organization");
    fs::write(&database, &changed).unwrap();
    let inverse_snapshot = library.organization_reference_snapshot("track").unwrap();
    let inverse = library.plan_organization_undo(&operation).unwrap();
    assert_eq!(
        library.organization_record(&inverse).unwrap().phase,
        OrganizationPhase::AwaitingReferences
    );
    library
        .bind_organization_references(&inverse, &inverse_snapshot.digest)
        .unwrap();
    let guard = library.repair_organization_references(&inverse).unwrap();
    library
        .acknowledge_organization_references_checked(&inverse, &guard)
        .unwrap();
    library
        .execute_organization_checked(&inverse, &guard)
        .unwrap();
    let native = fs::read_to_string(database).unwrap();
    assert!(native.contains(&native_reference(&source)));
    assert!(native.contains("Edited after organization"));
    assert!(native.contains("remote grid"));
    assert_eq!(library.track_path("track").unwrap(), source);
    assert!(!destination.exists());
    assert_eq!(
        library.organization_record(&operation).unwrap().phase,
        OrganizationPhase::Undone
    );
}
#[test]
fn a_published_move_inverse_cannot_acquire_a_new_reference_binding_late() {
    let (_dir, library, source, destination) = fixture();
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    library
        .execute_organization_checked(&operation, &guard)
        .unwrap();
    drop(guard);
    let snapshot = library.organization_reference_snapshot("track").unwrap();
    let inverse = library.plan_organization_undo(&operation).unwrap();
    library.execute_organization(&inverse).unwrap();
    assert!(
        library
            .bind_organization_references(&inverse, &snapshot.digest)
            .is_err()
    );
}

#[test]
fn pending_copy_source_edit_requires_an_explicit_choice_and_preserves_native_metadata() {
    for resolution in [
        crate::organization::AlternateResolution::IgnoreThisRevision,
        crate::organization::AlternateResolution::AddAsNewIncoming,
    ] {
        let (dir, library, source, destination) = fixture();
        let database = dir.path().canonicalize().unwrap().join("database.xml");
        linked_target(&library, &source, &database);
        let native_before = fs::read(&database).unwrap();
        let original = fs::read(&source).unwrap();
        let snapshot = library.organization_reference_snapshot("track").unwrap();
        let operation = library
            .plan_organization(OrganizationRequest {
                track_id: "track".into(),
                mode: OrganizationMode::Copy,
                source_root: source.parent().unwrap().into(),
                destination_root: destination.parent().unwrap().into(),
                files: vec![(source.clone(), destination.clone())],
            })
            .unwrap();
        library
            .bind_organization_references(&operation, &snapshot.digest)
            .unwrap();
        library.execute_organization(&operation).unwrap();
        let changed = b"externally replaced original audio";
        fs::write(&source, changed).unwrap();
        let changed_hash = file_hash(&source).unwrap().unwrap();
        assert!(matches!(
            library.organization_source_state(&source, &changed_hash).unwrap(),
            crate::organization::AlternateSourceState::Conflict { operation_id, .. } if operation_id == operation
        ));
        for _ in 0..2 {
            assert!(library.repair_organization_references(&operation).is_err());
            assert_eq!(fs::read(&source).unwrap(), changed);
            assert_eq!(fs::read(&destination).unwrap(), original);
            assert_eq!(fs::read(&database).unwrap(), native_before);
        }
        library
            .resolve_organization_alternate(&operation, resolution)
            .unwrap();
        assert_eq!(library.track_path("track").unwrap(), destination);
        assert_eq!(fs::read(&source).unwrap(), changed);
        assert_eq!(fs::read(&destination).unwrap(), original);
        let native = fs::read_to_string(&database).unwrap();
        assert!(native.contains(&native_reference(&destination)));
        for value in ["Remote artist", "remote grid", "99"] {
            assert!(native.contains(value));
        }
        let new_originals: i64 = library
            .conn()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM tracks WHERE path=?1 AND id<>'track'",
                [NativePath::new(&source).0],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            new_originals,
            i64::from(resolution == crate::organization::AlternateResolution::AddAsNewIncoming)
        );
    }
}

#[test]
fn windows_configuration_organization_repairs_native_refs_and_legacy_membership() {
    let (dir, library, source, destination) = fixture();
    let root = dir.path().canonicalize().unwrap();
    let home = root.join("VirtualDJ");
    fs::create_dir_all(home.join("Playlists/CueRithm")).unwrap();
    let database = home.join("database.xml");
    fs::write(&database,br#"<VirtualDJ_Database Version="8.5"><Song FilePath="C:\old.wav" Custom="native"/></VirtualDJ_Database>"#).unwrap();
    let playlist = home.join("Playlists/CueRithm/Owned.m3u");
    fs::write(&playlist, b"#EXTVDJ:\nC:\\old.wav\n").unwrap();
    let target:SyncTarget=serde_json::from_value(serde_json::json!({"software":"virtual_dj","path":database,"source_root":null,
      "environment":{"kind":"windows_compatibility","prefix":root,"installation":null,"mappings":[{"windows_root":"C:\\","host_root":root}]}})).unwrap();
    library
        .seed_organization_reference_fixture("track", target, false)
        .unwrap();
    let operation = planned(&library, &source, &destination);
    let guard = library.repair_organization_references(&operation).unwrap();
    library
        .acknowledge_organization_references_checked(&operation, &guard)
        .unwrap();
    assert_eq!(library.track_path("track").unwrap(), destination);
    assert!(
        fs::read_to_string(&database)
            .unwrap()
            .contains(r"C:\managed\new.wav")
    );
    assert_eq!(
        fs::read_to_string(&playlist).unwrap(),
        "#EXTVDJ:\nC:\\managed\\new.wav\n"
    );
    assert!(
        library
            .organization_reference_snapshot("track")
            .unwrap()
            .blockers
            .is_empty()
    );
    assert!(source.is_file());
}
