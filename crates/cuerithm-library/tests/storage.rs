use cuerithm_library::{Library, PreparationSettings, TrackQuery, TrackView};

#[cfg(unix)]
#[test]
fn library_resources_remain_bound_when_its_requested_directory_alias_changes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let original = root.join("original");
    let replacement = root.join("replacement");
    std::fs::create_dir(&original).unwrap();
    std::fs::create_dir(&replacement).unwrap();
    let alias = root.join("library-alias");
    std::os::unix::fs::symlink(&original, &alias).unwrap();
    let library = Library::open(&alias.join("library.sqlite")).unwrap();
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&replacement, &alias).unwrap();
    std::fs::write(library.data_dir().join("private-resource"), b"owned").unwrap();
    assert!(original.join("private-resource").is_file());
    assert!(!replacement.join("private-resource").exists());
    assert_eq!(library.data_dir(), original);
}

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

// A build without native tag writers must not silently resume past accepted
// editor/review work, even before a physical write journal has been created.
#[cfg(not(feature = "autotag"))]
#[test]
fn untagged_build_preserves_native_curation_before_child_allocation() {
    use cuerithm_library::*;
    for native_required in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("library.sqlite");
        let source = temp.path().join("source.flac");
        std::fs::write(&source, b"curation guard fixture; never decoded").unwrap();
        let library = Library::open(&db).unwrap();
        library
            .enqueue(
                std::slice::from_ref(&source),
                library.settings().unwrap().revision,
            )
            .unwrap();
        let run = library.claim_next().unwrap().unwrap();
        let service = CurationService::new(library.clone(), None);
        service
            .update(CurationUpdateRequest {
                request_id: format!("pending-{native_required}"),
                taxonomy_revision: service.catalog().unwrap().revision,
                tracks: vec![CurationExpectedRevision {
                    track_id: run.track_id.clone(),
                    revision: 0,
                }],
                patch: CurationPatch {
                    notes: CurationValueEdit::Set {
                        value: "Pending note".into(),
                    },
                    ..Default::default()
                },
            })
            .unwrap();
        let sql = rusqlite::Connection::open(&db).unwrap();
        // Simulate a desired head accepted by a tagging-enabled application,
        // before its native child journal can have been allocated.
        sql.execute(
            "UPDATE curation_tracks SET native_required=?1 WHERE track_id=?2",
            rusqlite::params![native_required, run.track_id],
        )
        .unwrap();
        drop(service);
        drop(library);
        let opened = Library::open(&db);
        if native_required {
            let error = opened
                .err()
                .expect("accepted native curation needs the full writer build");
            assert!(error.to_string().contains("tagging-enabled"));
            let state: String = sql
                .query_row("SELECT state FROM jobs WHERE id=?1", [&run.id], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(state, "running", "refusal must precede ownership recovery");
        } else {
            assert!(
                opened.is_ok(),
                "library-only pending curation remains available"
            );
        }
        assert_eq!(
            std::fs::read(&source).unwrap(),
            b"curation guard fixture; never decoded"
        );
    }
}

#[cfg(not(feature = "autotag"))]
#[test]
fn untagged_build_preserves_pending_native_decisions_before_recovery() {
    for (kind, state, completed) in [
        ("editor", "queued", false),
        ("editor", "claimed", false),
        ("editor", "committed", false),
        ("review", "pending", false),
        ("review", "claimed", false),
        ("review", "applied", false),
        ("editor", "committed", true),
        ("review", "applied", true),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.flac");
        std::fs::write(&path, b"guard fixture; never decoded").unwrap();
        let db = temp.path().join("library.sqlite");
        let library = Library::open(&db).unwrap();
        library
            .enqueue(
                std::slice::from_ref(&path),
                library.settings().unwrap().revision,
            )
            .unwrap();
        let run = library.claim_next().unwrap().unwrap();
        library
            .fail_for_run(&run, "review", "retained native decision")
            .unwrap();
        drop(library);
        let sql = rusqlite::Connection::open(&db).unwrap();
        sql.pragma_update(None, "foreign_keys", true).unwrap();
        if kind == "editor" {
            sql.execute(
                "INSERT INTO metadata_edit_drafts VALUES('draft',1,'draft','{}',0,0)",
                [],
            )
            .unwrap();
            sql.execute("INSERT INTO metadata_edit_batches VALUES('batch','draft',1,'save','complete','{}',NULL,0,0)", []).unwrap();
            sql.execute("INSERT INTO metadata_edit_overrides(job_id,track_id,draft_id,draft_revision,batch_id,source_hash,settings_revision,path_revision,metadata_revision,accepted_attempt,execution_source_proof_json,state,payload_json,final_json,created,updated) VALUES(?1,?2,'draft',1,'batch',?3,?4,0,0,?5,'{}',?6,'{}',?7,0,0)",rusqlite::params![run.id,run.track_id,run.source_sha256,run.settings_revision,run.attempt,state,completed.then_some("{}")]).unwrap();
        } else {
            sql.execute("INSERT INTO preparation_review_decisions(job_id,track_id,source_hash,settings_revision,generation,accepted_attempt,payload_json,binding_json,state,created,updated) VALUES(?1,?2,?3,?4,'generation',?5,'{}','{}',?6,0,0)",rusqlite::params![run.id,run.track_id,run.source_sha256,run.settings_revision,run.attempt,state]).unwrap();
        }
        if completed {
            sql.execute("UPDATE jobs SET state='completed' WHERE id=?1", [&run.id])
                .unwrap();
            sql.execute(
                "UPDATE tracks SET state='ready' WHERE id=?1",
                [&run.track_id],
            )
            .unwrap();
        }
        let before: String = sql
            .query_row(
                "SELECT json_array(state,stage,attempts,settings_revision) FROM jobs WHERE id=?1",
                [&run.id],
                |r| r.get(0),
            )
            .unwrap();
        let opened = Library::open(&db);
        if completed {
            assert!(
                opened.is_ok(),
                "completed {kind} history must remain readable"
            );
        } else {
            let error = opened
                .err()
                .expect("pending native decisions require a tagging-enabled build");
            assert!(
                error.to_string().contains("Pending metadata writes"),
                "{kind}/{state}: {error:#}"
            );
            let after: String = sql.query_row("SELECT json_array(state,stage,attempts,settings_revision) FROM jobs WHERE id=?1", [&run.id], |r| r.get(0)).unwrap();
            assert_eq!(
                after, before,
                "refused reopen must retain pending ownership"
            );
        }
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"guard fixture; never decoded"
        );
    }
}
