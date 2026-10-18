use cuerithm_library::{Library, PreparationSettings, TrackQuery, TrackView};

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
}
