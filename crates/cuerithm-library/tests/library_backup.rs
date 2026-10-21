use cuerithm_library::{Library, LibraryRestoreStartup, apply_pending_library_restore};
use rusqlite::Connection;
use std::fs;
fn fixture() -> (tempfile::TempDir, Library, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("app/library.sqlite");
    let lib = Library::open(&db).unwrap();
    (d, lib, db)
}
#[test]
fn snapshot_restores_only_after_close_and_preserves_original_database() {
    let (d, lib, db) = fixture();
    let destination = d.path().join("backups");
    let backup = lib.create_library_backup(&destination).unwrap();
    assert!(backup.path.join("manifest.json").is_file());
    assert_eq!(fs::read_dir(&backup.path).unwrap().count(), 2);
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![d.path().join("generated-incoming")];
    lib.save_settings(settings).unwrap();
    lib.stage_library_restore(&backup.path, &backup.database_sha256)
        .unwrap();
    assert!(apply_pending_library_restore(&db).is_err());
    assert_eq!(lib.settings().unwrap().settings.watch_folders.len(), 1);
    drop(lib);
    let LibraryRestoreStartup::Applied {
        pre_restore: preserved,
    } = apply_pending_library_restore(&db).unwrap()
    else {
        panic!("Restore was not applied")
    };
    assert!(preserved.join("library.sqlite").is_file());
    let lib = Library::open(&db).unwrap();
    assert!(lib.settings().unwrap().settings.watch_folders.is_empty());
    assert!(matches!(
        apply_pending_library_restore(&db).unwrap(),
        LibraryRestoreStartup::NoPending
    ));
}
#[test]
fn backup_rejects_unsettled_publication_and_restore_rejects_changed_dependencies() {
    let (d, lib, _) = fixture();
    let resource = lib.data_dir().join("cleanup/retained-original");
    fs::create_dir_all(resource.parent().unwrap()).unwrap();
    fs::write(&resource, b"authored original").unwrap();
    let backup = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    fs::write(&resource, b"changed").unwrap();
    assert!(lib.inspect_library_backup(&backup.path).is_err());
    let conn = Connection::open(lib.data_dir().join("library.sqlite")).unwrap();
    conn.execute("INSERT INTO maintenance_writes(id,kind,state,payload_json,created) VALUES('fixture','apply','conflict','{}',0)",[]).unwrap();
    assert!(
        lib.create_library_backup(&d.path().join("backups"))
            .is_err()
    );
}
#[test]
fn bundle_rejects_tampering_extra_entries_and_wrong_identity() {
    let (d, lib, _) = fixture();
    let backup = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    fs::write(backup.path.join("extra"), b"unexpected").unwrap();
    assert!(lib.inspect_library_backup(&backup.path).is_err());
    fs::remove_file(backup.path.join("extra")).unwrap();
    let conn = Connection::open(backup.path.join("library.sqlite")).unwrap();
    conn.execute_batch("PRAGMA application_id=1;").unwrap();
    drop(conn);
    assert!(
        lib.stage_library_restore(&backup.path, &backup.database_sha256)
            .is_err()
    );
}
#[cfg(unix)]
#[test]
fn bundle_rejects_symlinks() {
    let (d, lib, _) = fixture();
    let b = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    fs::rename(b.path.join("library.sqlite"), d.path().join("moved.sqlite")).unwrap();
    std::os::unix::fs::symlink(d.path().join("moved.sqlite"), b.path.join("library.sqlite"))
        .unwrap();
    assert!(lib.inspect_library_backup(&b.path).is_err());
}

fn insert_source(lib: &Library, path: &std::path::Path, hash: &str) {
    #[cfg(unix)]
    let bytes = {
        use std::os::unix::ffi::OsStrExt;
        [b"unix:".as_slice(), path.as_os_str().as_bytes()].concat()
    };
    #[cfg(windows)]
    let bytes = {
        use std::os::windows::ffi::OsStrExt;
        let mut out = b"windows:".to_vec();
        for unit in path.as_os_str().encode_wide() {
            out.extend(unit.to_le_bytes());
        }
        out
    };
    Connection::open(lib.data_dir().join("library.sqlite")).unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('authored',?1,'Authored fixture',?2,'ready',0,0)",rusqlite::params![bytes,hash]).unwrap();
}
#[test]
fn backup_checks_actual_source_bytes_and_restart_revalidates_after_staging() {
    let (d, lib, db) = fixture();
    let source = d.path().join("authored.wav");
    fs::write(&source, b"generated media fixture").unwrap();
    let hash = cuerithm_media::file_sha256(&source).unwrap();
    insert_source(&lib, &source, &hash);
    let b = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    assert_eq!(b.tracks, 1);
    assert_eq!(b.missing_sources, 0);
    lib.stage_library_restore(&b.path, &b.database_sha256)
        .unwrap();
    fs::write(&source, b"external tag edit after review").unwrap();
    drop(lib);
    assert!(matches!(
        apply_pending_library_restore(&db).unwrap(),
        LibraryRestoreStartup::BlockedBeforeMutation { .. }
    ));
    assert!(
        db.parent()
            .unwrap()
            .join("library-backups/pending.json")
            .is_file()
    );
    assert_eq!(
        fs::read(&source).unwrap(),
        b"external tag edit after review"
    );
    let lib = Library::open(&db).unwrap();
    assert!(
        lib.create_library_backup(&d.path().join("backups"))
            .is_err()
    );
    lib.cancel_library_restore().unwrap();
    assert!(lib.pending_library_restore().unwrap().is_none());
}
#[test]
fn backup_preserves_unavailable_source_status_and_refuses_returned_different_bytes() {
    let (d, lib, _) = fixture();
    let source = d.path().join("missing-authored.wav");
    insert_source(&lib, &source, &"a".repeat(64));
    let b = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    assert_eq!(b.missing_sources, 1);
    assert_eq!(
        lib.inspect_library_backup(&b.path).unwrap().missing_sources,
        1
    );
    fs::write(source, b"a different recording").unwrap();
    assert!(lib.inspect_library_backup(&b.path).is_err());
}
#[test]
fn pending_sync_or_claimed_jobs_do_not_produce_clean_backups() {
    let (d, lib, _) = fixture();
    let sync = lib.data_dir().join("sync");
    fs::create_dir_all(&sync).unwrap();
    let lease = fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(sync.join("service.lock"))
        .unwrap();
    fs2::FileExt::lock_exclusive(&lease).unwrap();
    assert!(
        lib.create_library_backup(&d.path().join("backups"))
            .is_err()
    );
    drop(lease);
    let source = d.path().join("fixture.wav");
    fs::write(&source, b"fixture").unwrap();
    insert_source(
        &lib,
        &source,
        &cuerithm_media::file_sha256(&source).unwrap(),
    );
    let conn = Connection::open(lib.data_dir().join("library.sqlite")).unwrap();
    conn.execute("INSERT INTO jobs(id,track_id,source_hash,settings_revision,state,stage,created,updated) SELECT 'fixture-job','authored','fixture',revision,'running','analysis',0,0 FROM selected_settings",[]).unwrap();
    assert!(
        lib.create_library_backup(&d.path().join("backups"))
            .is_err()
    );
}
#[test]
fn manifest_rejects_platform_installation_schema_and_missing_retained_recovery() {
    let (d, lib, _) = fixture();
    let b = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    let path = b.path.join("manifest.json");
    let original = fs::read(&path).unwrap();
    let mut json: serde_json::Value = serde_json::from_slice(&original).unwrap();
    json["schema"] = serde_json::json!(999);
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(lib.inspect_library_backup(&b.path).is_err());
    json = serde_json::from_slice(&original).unwrap();
    json["platform"] = serde_json::json!("different-platform");
    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(lib.inspect_library_backup(&b.path).is_err());
    fs::write(&path, &original).unwrap();
    let other = Library::open(&d.path().join("another/library.sqlite")).unwrap();
    assert!(other.inspect_library_backup(&b.path).is_err());
}

#[cfg(unix)]
#[test]
fn staging_refuses_a_redirected_managed_directory() {
    let (d, lib, _) = fixture();
    let b = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    let redirected = d.path().join("unrelated");
    fs::create_dir(&redirected).unwrap();
    std::os::unix::fs::symlink(&redirected, lib.data_dir().join("library-backups")).unwrap();
    assert!(
        lib.stage_library_restore(&b.path, &b.database_sha256)
            .is_err()
    );
    assert_eq!(fs::read_dir(redirected).unwrap().count(), 0);
}
#[test]
fn unavailable_original_recovery_copy_blocks_backup_without_pruning_history() {
    let (d, lib, _) = fixture();
    let conn = Connection::open(lib.data_dir().join("library.sqlite")).unwrap();
    conn.execute("INSERT INTO maintenance_writes(id,kind,state,payload_json,created,cleanup_complete) VALUES('retained','apply','committed','{}',0,1)",[]).unwrap();
    conn.execute("INSERT INTO maintenance_write_files(write_id,ordinal,target,staged,backup,before_hash,after_hash,publish_path,displaced_path,workspace) VALUES('retained',0,X'00',X'00',NULL,'before','after',X'00',X'00',X'00')",[]).unwrap();
    assert!(
        lib.create_library_backup(&d.path().join("backups"))
            .is_err()
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM maintenance_writes", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn blocked_restore_keeps_current_library_openable_and_requires_explicit_retry() {
    let (d, lib, db) = fixture();
    let source = d.path().join("generated.wav");
    fs::write(&source, b"original authored bytes").unwrap();
    insert_source(
        &lib,
        &source,
        &cuerithm_media::file_sha256(&source).unwrap(),
    );
    let b = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![d.path().join("current preparation")];
    lib.save_settings(settings).unwrap();
    lib.stage_library_restore(&b.path, &b.database_sha256)
        .unwrap();
    fs::write(&source, b"external edits").unwrap();
    drop(lib);
    assert!(matches!(
        apply_pending_library_restore(&db).unwrap(),
        LibraryRestoreStartup::BlockedBeforeMutation { .. }
    ));
    let lib = Library::open(&db).unwrap();
    assert_eq!(lib.settings().unwrap().settings.watch_folders.len(), 1);
    assert!(
        lib.pending_library_restore()
            .unwrap()
            .unwrap()
            .blocked_reason
            .is_some()
    );
    assert!(lib.retry_library_restore().is_err());
    fs::write(&source, b"original authored bytes").unwrap();
    // Merely fixing the files does not silently retry a blocked request.
    assert!(matches!(
        apply_pending_library_restore(&db).unwrap(),
        LibraryRestoreStartup::BlockedBeforeMutation { .. }
    ));
    assert!(
        lib.retry_library_restore()
            .unwrap()
            .blocked_reason
            .is_none()
    );
    drop(lib);
    assert!(matches!(
        apply_pending_library_restore(&db).unwrap(),
        LibraryRestoreStartup::Applied { .. }
    ));
    let lib = Library::open(&db).unwrap();
    assert!(lib.settings().unwrap().settings.watch_folders.is_empty());
}
#[test]
fn interrupted_database_update_reuses_original_recovery_snapshot_and_never_claims_safe_block() {
    let (d, lib, db) = fixture();
    let b = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![d.path().join("before restore")];
    lib.save_settings(settings).unwrap();
    let original = lib
        .create_library_backup(&lib.data_dir().join("library-backups/recovery-fixture"))
        .unwrap();
    let request = lib
        .stage_library_restore(&b.path, &b.database_sha256)
        .unwrap();
    let pending = lib.data_dir().join("library-backups/pending.json");
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&pending).unwrap()).unwrap();
    json["mutation_started"] = true.into();
    json["pre_restore"] = serde_json::to_value(&original.path).unwrap();
    fs::write(&pending, serde_json::to_vec(&json).unwrap()).unwrap();
    drop(lib);
    let LibraryRestoreStartup::Applied { pre_restore } =
        apply_pending_library_restore(&db).unwrap()
    else {
        panic!("Interrupted restore was not recovered")
    };
    assert_eq!(pre_restore, original.path);
    let conn = Connection::open(pre_restore.join("library.sqlite")).unwrap();
    let saved: String = conn
        .query_row(
            "SELECT s.json FROM settings s JOIN selected_settings a ON a.revision=s.revision",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&saved).unwrap()["settings"]["watch_folders"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Reintroduce the durable marker with a missing recovery snapshot: startup must
    // stop, rather than opening a possibly partially updated library as unchanged.
    json["id"] = request.id.into();
    json["pre_restore"] = serde_json::to_value(d.path().join("absent")).unwrap();
    fs::write(&pending, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(apply_pending_library_restore(&db).is_err());
}

#[test]
fn durable_recovery_pointer_rejects_parent_traversal() {
    let (d, lib, db) = fixture();
    let b = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    lib.stage_library_restore(&b.path, &b.database_sha256)
        .unwrap();
    let directory = lib.data_dir().join("library-backups");
    let pending = directory.join("pending.json");
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&pending).unwrap()).unwrap();
    json["mutation_started"] = true.into();
    json["pre_restore"] = serde_json::to_value(
        directory
            .join("../..")
            .join("backups")
            .join(b.path.file_name().unwrap()),
    )
    .unwrap();
    fs::write(&pending, serde_json::to_vec(&json).unwrap()).unwrap();
    drop(lib);
    assert!(apply_pending_library_restore(&db).is_err());
}

fn organized_fixture(
    mode: cuerithm_library::organization::OrganizationMode,
) -> (
    tempfile::TempDir,
    Library,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    String,
) {
    use cuerithm_library::organization::OrganizationRequest;
    let (d, lib, db) = fixture();
    let root = d.path().canonicalize().unwrap();
    let source_root = root.join("music");
    let destination_root = if mode == cuerithm_library::organization::OrganizationMode::Rename {
        source_root.clone()
    } else {
        root.join("managed")
    };
    fs::create_dir_all(&source_root).unwrap();
    fs::create_dir_all(&destination_root).unwrap();
    let source = source_root.join("authored.wav");
    let sidecar = source_root.join("authored.jpg");
    fs::write(&source, b"authored organization audio").unwrap();
    fs::write(&sidecar, b"authored owned artwork").unwrap();
    insert_source(
        &lib,
        &source,
        &cuerithm_media::file_sha256(&source).unwrap(),
    );
    let conn = Connection::open(&db).unwrap();
    #[cfg(unix)]
    let encoded = {
        use std::os::unix::ffi::OsStrExt;
        [b"unix:".as_slice(), sidecar.as_os_str().as_bytes()].concat()
    };
    #[cfg(windows)]
    let encoded = {
        use std::os::windows::ffi::OsStrExt;
        let mut b = b"windows:".to_vec();
        for n in sidecar.as_os_str().encode_wide() {
            b.extend(n.to_le_bytes());
        }
        b
    };
    conn.execute("INSERT INTO organization_owned_sidecars(track_id,path,source_hash,manifest_id,ordinal) VALUES('authored',?1,?2,'fixture',1)",rusqlite::params![encoded,cuerithm_media::file_sha256(&sidecar).unwrap()]).unwrap();
    drop(conn);
    let destination = destination_root.join("organized.wav");
    let destination_sidecar = destination_root.join("organized.jpg");
    let operation = lib
        .plan_organization(OrganizationRequest {
            track_id: "authored".into(),
            mode,
            source_root,
            destination_root,
            files: vec![
                (source.clone(), destination.clone()),
                (sidecar, destination_sidecar.clone()),
            ],
        })
        .unwrap();
    (d, lib, db, destination, destination_sidecar, operation)
}
fn finish_organization(lib: &Library, id: &str) {
    lib.execute_organization(id).unwrap();
    lib.acknowledge_organization_references(id).unwrap();
    lib.execute_organization(id).unwrap();
}
#[test]
fn settled_organization_with_owned_sidecars_remains_backed_up_and_restorable() {
    use cuerithm_library::organization::OrganizationMode;
    for mode in [
        OrganizationMode::Move,
        OrganizationMode::Copy,
        OrganizationMode::Rename,
    ] {
        let (d, lib, db, destination, sidecar, id) = organized_fixture(mode);
        finish_organization(&lib, &id);
        let backup = lib
            .create_library_backup(&d.path().join("backups"))
            .unwrap();
        assert_eq!(backup.schema, 22);
        assert_eq!(lib.inspect_library_backup(&backup.path).unwrap().tracks, 1);
        lib.stage_library_restore(&backup.path, &backup.database_sha256)
            .unwrap();
        drop(lib);
        assert!(matches!(
            apply_pending_library_restore(&db).unwrap(),
            LibraryRestoreStartup::Applied { .. }
        ));
        let reopened = Library::open(&db).unwrap();
        assert_eq!(reopened.track_path("authored").unwrap(), destination);
        assert_eq!(fs::read(sidecar).unwrap(), b"authored owned artwork");
        assert_eq!(reopened.organization_history().unwrap().len(), 1);
    }
}
#[test]
fn backup_rejects_unsettled_organization_boundaries_but_allows_completed_history() {
    use cuerithm_library::organization::OrganizationMode;
    let (d, lib, _, _, _, id) = organized_fixture(OrganizationMode::Move);
    for step in 0..3 {
        let error = lib
            .create_library_backup(&d.path().join("backups"))
            .unwrap_err();
        assert!(
            error.to_string().to_lowercase().contains("organization"),
            "{error:#}"
        );
        if step == 0 {
            lib.execute_organization(&id).unwrap();
        } else if step == 1 {
            lib.acknowledge_organization_references(&id).unwrap();
        }
    }
    lib.execute_organization(&id).unwrap();
    assert!(lib.create_library_backup(&d.path().join("backups")).is_ok());
}
#[test]
fn backup_manifest_binds_owned_sidecars_and_native_original_objects() {
    use cuerithm_library::organization::OrganizationMode;
    let (d, lib, _, _, sidecar, id) = organized_fixture(OrganizationMode::Move);
    finish_organization(&lib, &id);
    let backup = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    fs::write(&sidecar, b"changed sidecar").unwrap();
    assert!(lib.inspect_library_backup(&backup.path).is_err());
    fs::write(&sidecar, b"authored owned artwork").unwrap();
    let record = lib.organization_history().unwrap().remove(0);
    let file = &record.files[0];
    let quarantine = file
        .source
        .path()
        .unwrap()
        .parent()
        .unwrap()
        .join(&file.quarantine_name);
    let preserved = quarantine.with_extension("preserved-fixture");
    fs::rename(&quarantine, &preserved).unwrap();
    fs::write(&quarantine, fs::read(&preserved).unwrap()).unwrap();
    assert!(lib.inspect_library_backup(&backup.path).is_err());
    assert!(
        lib.create_library_backup(&d.path().join("fresh")).is_err(),
        "Equal bytes from a different native original cannot restore recovery authority"
    );
}
#[test]
fn completed_copy_originals_and_native_reference_resources_are_required() {
    use cuerithm_library::organization::OrganizationMode;
    let (d, lib, _, _, _, id) = organized_fixture(OrganizationMode::Copy);
    finish_organization(&lib, &id);
    let resource = lib
        .data_dir()
        .join("organization-references/authored-retained/backup");
    fs::create_dir_all(resource.parent().unwrap()).unwrap();
    fs::write(&resource, b"authored native recovery bytes").unwrap();
    let backup = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    fs::remove_file(&resource).unwrap();
    assert!(lib.inspect_library_backup(&backup.path).is_err());
    fs::write(&resource, b"authored native recovery bytes").unwrap();
    let original = lib.organization_history().unwrap()[0].files[0]
        .source
        .path()
        .unwrap();
    let preserved = original.with_extension("preserved-fixture");
    fs::rename(&original, &preserved).unwrap();
    fs::write(&original, fs::read(preserved).unwrap()).unwrap();
    assert!(lib.inspect_library_backup(&backup.path).is_err());
    assert!(lib.create_library_backup(&d.path().join("fresh")).is_err());
}
#[test]
fn repeated_moves_and_undo_do_not_require_obsolete_historical_destinations() {
    use cuerithm_library::organization::{OrganizationMode, OrganizationRequest};
    let (d, lib, _, destination, sidecar, first) = organized_fixture(OrganizationMode::Move);
    finish_organization(&lib, &first);
    let second_destination = destination.with_file_name("second.wav");
    let second_sidecar = sidecar.with_file_name("second.jpg");
    let second = lib
        .plan_organization(OrganizationRequest {
            track_id: "authored".into(),
            mode: OrganizationMode::Move,
            source_root: destination.parent().unwrap().into(),
            destination_root: destination.parent().unwrap().into(),
            files: vec![
                (destination.clone(), second_destination),
                (sidecar, second_sidecar),
            ],
        })
        .unwrap();
    finish_organization(&lib, &second);
    assert!(!destination.exists());
    assert!(lib.create_library_backup(&d.path().join("backups")).is_ok());
    let undo = lib.plan_organization_undo(&second).unwrap();
    finish_organization(&lib, &undo);
    assert!(
        lib.create_library_backup(&d.path().join("after-undo"))
            .is_ok()
    );
}

#[test]
fn pre_proof_ignored_copy_history_is_preserved_and_bound_to_observed_native_identity() {
    use cuerithm_library::organization::{AlternateResolution, OrganizationMode};
    let (d, lib, db, _, _, id) = organized_fixture(OrganizationMode::Copy);
    finish_organization(&lib, &id);
    let source = lib.organization_history().unwrap()[0].files[0]
        .source
        .path()
        .unwrap();
    fs::write(&source, b"explicitly ignored authored revision").unwrap();
    let hash = cuerithm_media::file_sha256(&source).unwrap();
    lib.organization_source_state(&source, &hash).unwrap();
    lib.resolve_organization_alternate(&id, AlternateResolution::IgnoreThisRevision)
        .unwrap();
    Connection::open(&db)
        .unwrap()
        .execute(
            "DELETE FROM organization_copy_conflicts WHERE operation_id=?1",
            [&id],
        )
        .unwrap();
    let backup = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    lib.inspect_library_backup(&backup.path).unwrap();
    let preserved = source.with_extension("preserved-fixture");
    fs::rename(&source, &preserved).unwrap();
    fs::write(&source, fs::read(preserved).unwrap()).unwrap();
    assert!(lib.inspect_library_backup(&backup.path).is_err());
}
#[test]
fn preparation_attention_checkpoints_are_unsettled_even_when_jobs_are_not_running() {
    let (d, lib, db) = fixture();
    let source = d.path().join("authored.wav");
    fs::write(&source, b"checkpoint fixture").unwrap();
    let hash = cuerithm_media::file_sha256(&source).unwrap();
    insert_source(&lib, &source, &hash);
    let connection = Connection::open(&db).unwrap();
    let revision = lib.settings().unwrap().revision;
    connection.execute("INSERT INTO jobs(id,track_id,source_hash,settings_revision,state,stage,created,updated) VALUES('checkpoint-job','authored',?1,?2,'attention','organizing',0,0)",rusqlite::params![hash,revision]).unwrap();
    connection.execute("INSERT INTO preparation_checkpoints(job_id,track_id,settings_revision,input_hash,output_hash,attempt,payload_json,state,created,updated) VALUES('checkpoint-job','authored',?1,?2,?2,1,'{}','prepared',0,0)",rusqlite::params![revision,hash]).unwrap();
    for state in ["tagging", "prepared", "organizing"] {
        connection
            .execute("UPDATE preparation_checkpoints SET state=?1", [state])
            .unwrap();
        assert!(
            lib.create_library_backup(&d.path().join("backups"))
                .unwrap_err()
                .to_string()
                .to_lowercase()
                .contains("organization")
        );
    }
}
#[cfg(unix)]
#[test]
fn sparse_oversized_organization_original_is_rejected_before_reading_its_bytes() {
    use cuerithm_library::organization::OrganizationMode;
    let (d, lib, _, _, _, id) = organized_fixture(OrganizationMode::Move);
    finish_organization(&lib, &id);
    let record = lib.organization_history().unwrap().remove(0);
    let file = &record.files[0];
    let quarantine = file
        .source_parent
        .path
        .path()
        .unwrap()
        .join(&file.quarantine_name);
    let file = fs::OpenOptions::new().write(true).open(quarantine).unwrap();
    file.set_len(1024_u64.pow(4) + 1).unwrap();
    let result = lib.create_library_backup(&d.path().join("backups"));
    // Reclaim the authored sparse fixture immediately, even if the assertion fails.
    file.set_len(0).unwrap();
    assert!(result.unwrap_err().to_string().contains("budget"));
}

#[test]
fn cancelling_restore_removes_only_its_owned_staged_copy_and_is_repeatable() {
    let (d, lib, _) = fixture();
    let backup = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    let intent = lib
        .stage_library_restore(&backup.path, &backup.database_sha256)
        .unwrap();
    let staged = lib.data_dir().join("library-backups").join(&intent.id);
    assert!(staged.join("library.sqlite").is_file());
    lib.cancel_library_restore().unwrap();
    assert!(!staged.exists(), "cancelled copies must not accumulate");
    assert!(
        backup.path.join("library.sqlite").is_file(),
        "the selected original backup stays intact"
    );
    assert!(lib.pending_library_restore().unwrap().is_none());
    lib.cancel_library_restore().unwrap();
}
#[test]
fn restore_cancellation_preserves_unowned_entries_and_started_recovery() {
    let (d, lib, _) = fixture();
    let backup = lib
        .create_library_backup(&d.path().join("backups"))
        .unwrap();
    let intent = lib
        .stage_library_restore(&backup.path, &backup.database_sha256)
        .unwrap();
    let staged = lib.data_dir().join("library-backups").join(&intent.id);
    fs::write(staged.join("unowned"), b"preserve this file").unwrap();
    assert!(lib.cancel_library_restore().is_err());
    assert!(staged.join("library.sqlite").exists());
    assert!(lib.pending_library_restore().unwrap().is_some());
    fs::remove_file(staged.join("unowned")).unwrap();
    let pending = lib.data_dir().join("library-backups/pending.json");
    let mut payload: serde_json::Value =
        serde_json::from_slice(&fs::read(&pending).unwrap()).unwrap();
    payload["mutation_started"] = true.into();
    fs::write(&pending, serde_json::to_vec(&payload).unwrap()).unwrap();
    assert!(lib.cancel_library_restore().is_err());
    assert!(staged.join("library.sqlite").exists());
    assert!(pending.exists());
}

#[test]
fn library_backup_refuses_unsettled_native_rekordbox_publication() {
    let (d, lib, db) = fixture();
    let conn = Connection::open(db).unwrap();
    conn.execute("INSERT INTO rekordbox_sync_operations(id,target_id,database_path,status,journal_json,created,updated) VALUES('authored','target','authored.db','publishing','{}',0,0)", []).unwrap();
    for status in ["prepared", "publishing", "native_published", "restoring"] {
        conn.execute("UPDATE rekordbox_sync_operations SET status=?1", [status])
            .unwrap();
        assert!(
            lib.create_library_backup(&d.path().join("backups"))
                .is_err(),
            "{status}"
        );
    }
    for status in ["applied", "restored", "discarded", "rolled_back"] {
        conn.execute("UPDATE rekordbox_sync_operations SET status=?1", [status])
            .unwrap();
        assert!(
            lib.create_library_backup(&d.path().join("backups")).is_ok(),
            "{status}"
        );
    }
}
