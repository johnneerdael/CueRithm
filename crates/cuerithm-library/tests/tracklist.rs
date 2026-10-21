use cuerithm_library::{
    Library, TrackSummary, TracklistFormat, TracklistMode, TracklistOptions, TracklistSaveRequest,
    match_tracklist, parse_tracklist,
};
fn track(id: &str, artist: &str, title: &str) -> TrackSummary {
    serde_json::from_value(serde_json::json!({"id":id,"display_path":"synthetic","source_sha256":"","metadata":{},"title":title,"artist":artist,"album":"","genre":"","state":"ready","archived":false,"key":null,"bpm":null,"energy":null,"duration":null})).unwrap()
}
#[test]
fn parses_unicode_text_and_quoted_csv_with_source_positions() {
    let rows = parse_tracklist(
        "\n宇多田ヒカル - 光 (Remix)\nOnly title",
        &TracklistOptions::default(),
    )
    .unwrap();
    assert_eq!(rows[0].row, 2);
    assert_eq!(rows[0].artist, "宇多田ヒカル");
    assert_eq!(rows[1].title, "Only title");
    let options = TracklistOptions {
        format: TracklistFormat::Csv,
        ..Default::default()
    };
    let rows = parse_tracklist(
        "ARTIST,title\n\"Artist, One\",\"A \"\"quoted\"\"\nTitle\"",
        &options,
    )
    .unwrap();
    assert_eq!(rows[0].artist, "Artist, One");
    assert_eq!(rows[0].title, "A \"quoted\"\nTitle");
    assert!(parse_tracklist("artist,title\na,b,c", &options).is_err());
    assert!(parse_tracklist("artist,title\na,\"broken", &options).is_err());
    assert!(
        parse_tracklist("artist,title", &options)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn rejects_limits_empty_titles_and_excessive_comparisons() {
    let options = TracklistOptions::default();
    for text in [
        "a - ".to_string(),
        "a".repeat(4097),
        "x\n".repeat(1001),
        "x".repeat(1_048_577),
    ] {
        assert!(parse_tracklist(&text, &options).is_err());
    }
    let entries = parse_tracklist(&"x\n".repeat(1000), &options).unwrap();
    assert!(
        match_tracklist(
            &entries,
            &vec![track("x", "", "x"); 10001],
            TracklistMode::Exact
        )
        .is_err()
    );
}
#[test]
fn preserves_versions_unicode_and_duplicates_and_caps_candidates_deterministically() {
    let options = TracklistOptions::default();
    let entries = parse_tracklist(
        "BEYONCÉ - Halo (Original Mix)\n宇多田ヒカル - 光\nunknown",
        &options,
    )
    .unwrap();
    let mut tracks = vec![
        track("b", "Beyoncé", "Halo (Original Mix)"),
        track("a", "Beyoncé", "Halo (Original Mix)"),
        track("remix", "Beyoncé", "Halo (Remix)"),
        track("jp", "宇多田ヒカル", "光"),
    ];
    let rows = match_tracklist(&entries, &tracks, TracklistMode::Exact).unwrap();
    assert_eq!(
        rows[0]
            .candidates
            .iter()
            .map(|c| c.track.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(rows[1].candidates[0].track.id, "jp");
    assert!(rows[2].candidates.is_empty());
    tracks
        .extend((0..20).map(|i| track(&format!("extra-{i:02}"), "Beyoncé", "Halo (Original Mix)")));
    let rows = match_tracklist(&entries, &tracks, TracklistMode::Fuzzy).unwrap();
    assert_eq!(rows[0].candidates.len(), 10);
    assert!(rows[0].candidate_count > 10);
}
#[test]
fn saves_ready_tracks_atomically_in_order_with_revision_guards_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("db");
    let lib = Library::open(&db).unwrap();
    let mut ids = vec![];
    for name in ["one", "two"] {
        let path = dir.path().join(format!("{name}.flac"));
        std::fs::write(&path, name).unwrap();
        let job = lib
            .enqueue(&[path], lib.settings().unwrap().revision)
            .unwrap()[0]
            .clone();
        ids.push(
            lib.jobs()
                .unwrap()
                .into_iter()
                .find(|j| j.id == job)
                .unwrap()
                .track_id,
        );
    }
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE tracks SET state='ready',title='Tune',artist='Artist'",
        [],
    )
    .unwrap();
    let result = lib
        .match_tracklist("Artist - Tune".into(), TracklistOptions::default())
        .unwrap();
    assert_eq!(result.rows[0].candidates.len(), 2);
    let request = TracklistSaveRequest {
        name: "Reference set".into(),
        track_ids: vec![ids[1].clone(), ids[0].clone(), ids[1].clone()],
        expected_library_revision: result.library_revision,
        expected_playlist_revision: result.playlist_revision,
    };
    let mut bad = request.clone();
    bad.track_ids.push("missing".into());
    assert!(lib.save_tracklist_playlist(bad).is_err());
    assert!(lib.playlist_state().unwrap().nodes.is_empty());
    let change = lib.save_tracklist_playlist(request.clone()).unwrap();
    assert_eq!(
        lib.manual_playlist_members(&change.created_ids[0]).unwrap(),
        [ids[1].clone(), ids[0].clone()]
    );
    assert!(lib.save_tracklist_playlist(request).is_err());
    assert_eq!(lib.playlist_state().unwrap().nodes.len(), 1);
    lib.undo_playlist_edit(change.edit_id.as_deref().unwrap(), change.revision)
        .unwrap();
    assert!(lib.playlist_state().unwrap().nodes.is_empty());
    let result = lib
        .match_tracklist("Artist - Tune".into(), TracklistOptions::default())
        .unwrap();
    lib.set_archived(&ids[0], true).unwrap();
    assert!(
        lib.save_tracklist_playlist(TracklistSaveRequest {
            name: "stale".into(),
            track_ids: vec![ids[0].clone()],
            expected_library_revision: result.library_revision,
            expected_playlist_revision: result.playlist_revision
        })
        .is_err()
    );
    assert!(lib.playlist_state().unwrap().nodes.is_empty());
    assert_eq!(std::fs::read(dir.path().join("one.flac")).unwrap(), b"one");
}

#[test]
fn fuzzy_thresholds_keep_versions_and_unknowns_visible_and_skip_unprepared_tracks() {
    let entries = parse_tracklist(
        "Northline - Night Window (Original Mix)\nरवि - प्रकाश\nUnrelated",
        &TracklistOptions::default(),
    )
    .unwrap();
    let mut unavailable = track("incoming", "Northline", "Night Window (Original Mix)");
    unavailable.state = "incoming".into();
    let mut archived = track("archive", "Northline", "Night Window (Original Mix)");
    archived.archived = true;
    let tracks = vec![
        track("similar", "Northline", "Night Windows (Original Mix)"),
        track("different-version", "Northline", "Night Window (Dub Remix)"),
        track("name", "रवि", "प्रकाश"),
        track("other-name", "रव", "प्रकाश"),
        unavailable,
        archived,
    ];
    let exact = match_tracklist(&entries, &tracks, TracklistMode::Exact).unwrap();
    assert!(exact[0].candidates.is_empty());
    assert_eq!(exact[1].candidates.len(), 1);
    let fuzzy = match_tracklist(&entries, &tracks, TracklistMode::Fuzzy).unwrap();
    assert_eq!(fuzzy[0].candidates[0].track.id, "similar");
    assert!(matches!(
        fuzzy[0].candidates[0].reason,
        cuerithm_library::TracklistReason::SimilarTags
    ));
    assert!(fuzzy[0].candidates.iter().all(|c| c.score >= 0.8));
    assert!(fuzzy[2].candidates.is_empty());
}
#[test]
fn swapping_columns_keeps_title_only_text_and_no_header_csv_in_source_order() {
    let options = TracklistOptions {
        swap_artist_title: true,
        ..Default::default()
    };
    let rows = parse_tracklist("Title - Artist\nJust a title", &options).unwrap();
    assert_eq!(rows[0].artist, "Artist");
    assert_eq!(rows[0].title, "Title");
    assert_eq!(rows[1].title, "Just a title");
    let options = TracklistOptions {
        format: TracklistFormat::Csv,
        headers: false,
        ..Default::default()
    };
    let rows = parse_tracklist("Artist,Title\nSecond,Other", &options).unwrap();
    assert_eq!(rows[0].row, 1);
    assert_eq!(rows[1].artist, "Second");
    assert!(parse_tracklist("a,\"closed\"garbage", &options).is_err());
}
