use cuerithm_library::{Library, PreparationSettings, TrackQuery, TrackView};

#[test]
fn engine_setup_migration_requeues_unfinished_work_without_reading_sources() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let path = d.path().join("offline.flac");
    std::fs::write(&path, b"source").unwrap();
    let id = lib
        .enqueue(
            std::slice::from_ref(&path),
            lib.settings().unwrap().revision,
        )
        .unwrap()[0]
        .clone();
    lib.claim_next().unwrap().unwrap();
    lib.fail(&id, "setup", "old missing model").unwrap();
    std::fs::remove_file(&path).unwrap();
    let revision = lib
        .save_settings(PreparationSettings {
            stability_millis: 50,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(lib.requeue_unfinished(&revision).unwrap(), 1);
    let job = lib.claim_next().unwrap().unwrap();
    assert_eq!(job.settings_revision, revision);
    assert_ne!(job.id, id);
}

#[test]
fn persists_settings_deduplicates_sources_and_recovers_running_jobs() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("library.sqlite");
    let path = d.path().join("演奏.flac");
    std::fs::write(&path, b"input").unwrap();
    let id;
    let rev;
    {
        let lib = Library::open(&db).unwrap();
        let settings = PreparationSettings {
            watch_folders: vec![d.path().to_owned()],
            ..Default::default()
        };
        rev = lib.save_settings(settings).unwrap();
        id = lib
            .enqueue(&[path.clone(), path.clone()], rev.clone())
            .unwrap()[0]
            .clone();
        assert_eq!(
            lib.enqueue(std::slice::from_ref(&path), rev.clone())
                .unwrap()[0],
            id
        );
        assert_eq!(lib.jobs().unwrap().len(), 1);
        let job = lib.claim_next().unwrap().unwrap();
        assert_eq!(job.id, id);
        assert_eq!(lib.settings().unwrap().settings.watch_folders, [d.path()]);
        assert!(
            Library::open(&db).is_err(),
            "a second process/owner must not reset running work"
        );
    }
    let lib = Library::open(&db).unwrap();
    let job = lib.claim_next().unwrap().unwrap();
    assert_eq!(job.id, id);
    assert_eq!(job.settings_revision, rev);
    let page = lib
        .tracks(TrackQuery {
            view: TrackView::Incoming,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.total, 1);
    assert!(page.rows[0].display_path.contains("演奏"));
}

#[test]
fn rejects_future_schema_without_replacing_it_and_keeps_query_order_stable() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("future.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    drop(conn);
    assert!(Library::open(&path).is_err());
    let conn = rusqlite::Connection::open(path).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        99
    );
    let lib = Library::open(&d.path().join("current.sqlite")).unwrap();
    let rev = lib.settings().unwrap().revision;
    for name in ["Zulu.flac", "Alpha.flac", "Bravo.flac"] {
        let p = d.path().join(name);
        std::fs::write(&p, name).unwrap();
        lib.enqueue(&[p], rev.clone()).unwrap();
    }
    let query = TrackQuery {
        view: TrackView::Incoming,
        limit: 1,
        ..Default::default()
    };
    let one = lib.tracks(query.clone()).unwrap();
    assert_eq!(one.rows[0].title, "Alpha");
    let running = lib.claim_next().unwrap().unwrap();
    lib.set_stage(&running.id, "decoding").unwrap();
    lib.fail(&running.id, "decoding", "test failure").unwrap();
    let two = lib
        .tracks(TrackQuery {
            cursor: one.next_cursor,
            ..query.clone()
        })
        .unwrap();
    assert_eq!(two.rows[0].title, "Bravo");
    let filtered = lib
        .tracks(TrackQuery {
            search: "Zulu".into(),
            ..query
        })
        .unwrap();
    assert_eq!(filtered.total, 1);
    let cursor = two.next_cursor.unwrap();
    lib.save_settings(PreparationSettings {
        stability_millis: 40,
        ..Default::default()
    })
    .unwrap();
    // Settings/job-only updates do not invalidate a track-page cursor.
    assert!(
        lib.tracks(TrackQuery {
            view: TrackView::Incoming,
            limit: 1,
            cursor: Some(cursor.clone()),
            ..Default::default()
        })
        .is_ok()
    );
    let added = d.path().join("Delta.flac");
    std::fs::write(&added, b"delta").unwrap();
    lib.enqueue(&[added], lib.settings().unwrap().revision)
        .unwrap();
    assert!(
        lib.tracks(TrackQuery {
            view: TrackView::Incoming,
            limit: 1,
            cursor: Some(cursor),
            ..Default::default()
        })
        .is_err()
    );
    let accented = d.path().join("ÉCLAIR.flac");
    std::fs::write(&accented, b"source").unwrap();
    lib.enqueue(&[accented], lib.settings().unwrap().revision)
        .unwrap();
    assert_eq!(
        lib.tracks(TrackQuery {
            view: TrackView::Incoming,
            search: "éclair".into(),
            ..Default::default()
        })
        .unwrap()
        .total,
        1
    );
    assert_eq!(
        lib.track_ids(TrackQuery {
            view: TrackView::Incoming,
            limit: 1,
            ..Default::default()
        })
        .unwrap()
        .len(),
        5
    );
}

#[cfg(unix)]
#[test]
fn reopening_waits_for_a_briefly_inherited_child_lease() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("library.sqlite");
    let library = cuerithm_library::Library::open(&db).unwrap();
    // A concurrent decoder spawn can briefly inherit flock's open description.
    // The child performs only async-signal-safe operations and never uses SQLite.
    // It exits when the parent says so, so the lease is held for a fixed 25 ms
    // however long the scheduler takes to start the child.
    let mut pipe = [0; 2];
    assert_eq!(unsafe { libc::pipe(pipe.as_mut_ptr()) }, 0);
    let child = unsafe { libc::fork() };
    assert!(child >= 0);
    if child == 0 {
        let mut byte = 0u8;
        unsafe {
            libc::read(pipe[0], (&mut byte as *mut u8).cast(), 1);
            libc::_exit(0);
        }
    }
    let release = pipe[1];
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(25));
        unsafe {
            libc::write(release, [0u8].as_ptr().cast(), 1);
        }
    });
    drop(library);
    let reopened = cuerithm_library::Library::open(&db);
    releaser.join().unwrap();
    unsafe {
        libc::waitpid(child, std::ptr::null_mut(), 0);
        libc::close(pipe[0]);
        libc::close(pipe[1]);
    }
    assert!(
        reopened.is_ok(),
        "transient inherited lease must settle: {:?}",
        reopened.err()
    );
}

#[test]
fn schema_eighteen_upgrade_keeps_organization_roots_and_preparation_state() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("app/library.sqlite");
    let music = d.path().join("music");
    std::fs::create_dir(&music).unwrap();
    let music = music.canonicalize().unwrap();
    let source = music.join("authored.flac");
    std::fs::write(&source, b"authored migration source").unwrap();
    let library = Library::open(&db).unwrap();
    let root = library.register_organization_root(music).unwrap();
    let settings = library.settings().unwrap().revision;
    let job = library
        .enqueue(std::slice::from_ref(&source), settings.clone())
        .unwrap()[0]
        .clone();
    let track = library.jobs().unwrap()[0].track_id.clone();
    drop(library);
    // Recreate the schema-18 boundary without leaving later feature tables.
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute_batch("DROP TABLE native_reconciliation_plans; DROP TABLE manual_publication_operations; DROP TABLE manual_publication_state; DROP TABLE cue_templates; DROP TABLE native_playlist_bindings; DROP TABLE native_playlist_acknowledgements; DROP TABLE rekordbox_sync_operations; DROP TABLE native_sync_track_ownership; PRAGMA user_version=18;").unwrap();
    drop(conn);
    let library = Library::open(&db).unwrap();
    assert_eq!(library.organization_roots().unwrap()[0].id, root.id);
    assert_eq!(library.track_path(&track).unwrap(), source);
    assert_eq!(library.jobs().unwrap()[0].id, job);
    assert_eq!(library.settings().unwrap().revision, settings);
    library
        .save_cue_template(
            None,
            "Practice".into(),
            Default::default(),
            library.revision().unwrap(),
        )
        .unwrap();
    assert_eq!(library.cue_templates().unwrap().len(), 1);
    let conn = rusqlite::Connection::open(db).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 22);
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM native_reconciliation_plans",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}
