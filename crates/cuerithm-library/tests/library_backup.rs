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
