//! Acceptance of the writer on a copy of a real rekordbox 7 library: real schema, indexes and rows.
//!
//! Skipped unless `CUERITHM_REKORDBOX_COPY` names a copied `master.db` (with its WAL/SHM next to it) and a
//! supported rekordbox is installed for the key. The copy is snapshotted into a temporary file first, so
//! neither the original nor the copy is ever opened for writing. Nothing from the library is printed.
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::*;
use std::path::PathBuf;

fn key() -> DatabaseKey {
    let cancel = CancellationToken::default();
    let paths = DiscoveryPaths::current().unwrap();
    let found = discover_targets(&paths, &cancel).unwrap();
    let target = found
        .installations
        .first()
        .expect("a rekordbox installation");
    let source = NativeKeySource::for_installation(target, paths.platform).unwrap();
    resolve_database_key(
        &source,
        &paths.agent_options_file().unwrap(),
        Default::default(),
        &cancel,
    )
    .unwrap()
}
fn collection(
    path: &std::path::Path,
    key: &DatabaseKey,
) -> (RekordboxCollection, DatabaseChecks, String) {
    let db = ReadOnlyDatabase::open(path, key).unwrap();
    let collection = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    (
        collection,
        db.checks().unwrap(),
        db.schema().unwrap().fingerprint,
    )
}
fn usn(collection: &RekordboxCollection) -> i64 {
    collection.identity.local_update_count.unwrap() as i64
}

#[test]
fn writing_to_a_copy_of_a_real_library_passes_rekordbox_style_verification() {
    let Some(copy) = std::env::var_os("CUERITHM_REKORDBOX_COPY").map(PathBuf::from) else {
        eprintln!("skipped: CUERITHM_REKORDBOX_COPY is not set");
        return;
    };
    let key = key();
    let cache = tempfile::tempdir().unwrap();
    let snapshot = ReadOnlyDatabase::open(&copy, &key)
        .unwrap()
        .capture(cache.path(), &key, &CancellationToken::default())
        .unwrap();
    // A second snapshot stands in for the live library that the written copy will be published over.
    let stand_in = ReadOnlyDatabase::open(&copy, &key)
        .unwrap()
        .capture(cache.path(), &key, &CancellationToken::default())
        .unwrap();
    let (before, checks_before, schema) = collection(&snapshot.path, &key);
    assert!(checks_before.integrity_ok);
    assert!(
        WRITE_PROFILES
            .iter()
            .any(|p| p.schema_fingerprint == schema),
        "The copied library is not a verified write profile"
    );
    let track = before
        .tracks
        .iter()
        .find(|t| t.importable_local() && !t.cues.is_empty())
        .expect("a track with cues");
    let others_before: Vec<_> = before
        .tracks
        .iter()
        .filter(|t| t.identity.content_id != track.identity.content_id)
        .map(|t| (t.identity.content_id.clone(), t.raw_hash.clone()))
        .collect();
    let cue = |in_msec, out_msec, kind, comment: &str| CueRow {
        in_msec,
        out_msec,
        kind,
        active_loop: false,
        beat_loop_size: None,
        color: None,
        color_table_index: Some(41),
        comment: Some(comment.into()),
    };
    let fields = ContentFields {
        title: Some(format!("{} (Cuerithm acceptance)", track.metadata.title)),
        genre: Some("Cuerithm Acceptance Genre".into()),
        comment: Some("written by the acceptance test".into()),
        ..Default::default()
    };
    let cues = vec![cue(2000, -1, 1, "Start"), cue(90_000, -1, 2, "Drop")];
    let second = before
        .tracks
        .iter()
        .find(|t| t.importable_local() && t.identity.content_id != track.identity.content_id)
        .expect("a second track");
    let ids = vec![
        second.identity.content_id.clone(),
        track.identity.content_id.clone(),
    ];
    let tag = MyTagRef {
        category: "Cuerithm Acceptance".into(),
        tag: "Written".into(),
    };
    let plan = WritePlan {
        tracks: vec![TrackWrite {
            content_id: track.identity.content_id.clone(),
            expected_fingerprint: track.raw_hash.clone(),
            fields: fields.clone(),
            cues: Some(cues.clone()),
            tags: Some(vec![tag.clone()]),
        }],
        playlists: vec![
            PlaylistWrite {
                path: vec!["Cuerithm Acceptance".into()],
                kind: PlaylistKind::Folder,
                position: 99,
                tracks: vec![],
                expected_members: None,
            },
            PlaylistWrite {
                path: vec!["Cuerithm Acceptance".into(), "Ordered".into()],
                kind: PlaylistKind::Playlist,
                position: 1,
                tracks: ids.clone(),
                expected_members: None,
            },
        ],
        ..Default::default()
    };
    let mut db = WriteDatabase::open(&snapshot.path, &key).unwrap();
    let report = db.apply(&plan, &CancellationToken::default()).unwrap();
    drop(db);
    assert_eq!(report.usn, Some(usn(&before) + 1));
    assert_eq!((report.tracks_updated, report.cues_replaced), (1, 1));
    assert_eq!((report.playlists_created, report.tags_replaced), (2, 1));

    // The result must read back through the independent reader with the schema and integrity unchanged.
    let (after, checks_after, schema_after) = collection(&snapshot.path, &key);
    assert!(checks_after.integrity_ok);
    assert_eq!(
        checks_after.foreign_key_issues,
        checks_before.foreign_key_issues
    );
    assert_eq!(schema_after, schema);
    assert_eq!(usn(&after), usn(&before) + 1);
    assert_eq!(after.tracks.len(), before.tracks.len());
    let written = after
        .tracks
        .iter()
        .find(|t| t.identity.content_id == track.identity.content_id)
        .unwrap();
    assert_eq!(Some(&written.metadata.title), fields.title.as_ref());
    assert_eq!(written.metadata.genre, "Cuerithm Acceptance Genre");
    assert_eq!(written.metadata.comment, "written by the acceptance test");
    assert_eq!(written.metadata.artist, track.metadata.artist);
    assert_eq!(written.bpm, track.bpm);
    // Identity is unchanged except the row's change number, which now carries this write's.
    let mut identity = serde_json::to_value(&written.identity).unwrap();
    let mut original = serde_json::to_value(&track.identity).unwrap();
    assert_eq!(identity["local_usn"], usn(&after));
    identity.as_object_mut().unwrap().remove("local_usn");
    original.as_object_mut().unwrap().remove("local_usn");
    assert_eq!(identity, original);
    assert_eq!(written.cues.len(), 2);
    let mut seen: Vec<_> = written
        .cues
        .iter()
        .map(|c| (c.start_seconds, c.kind, c.comment.clone()))
        .collect();
    seen.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    assert_eq!(
        seen,
        [
            (Some(2.0), 1, "Start".to_string()),
            (Some(90.0), 2, "Drop".to_string())
        ]
    );
    // The folder, the ordered playlist and the MyTag assignment read back through the independent reader.
    let folder = after
        .playlists
        .iter()
        .find(|p| p.name == "Cuerithm Acceptance" && p.attribute == 1)
        .expect("the new folder");
    assert_eq!(folder.parent_id.as_deref(), Some("root"));
    assert_eq!(folder.deleted, Some(false));
    let ordered = after
        .playlists
        .iter()
        .find(|p| p.name == "Ordered" && p.attribute == 0)
        .expect("the new playlist");
    assert_eq!(ordered.parent_id.as_deref(), Some(folder.id.as_str()));
    let mut links: Vec<_> = after
        .playlist_links
        .iter()
        .filter(|l| l.playlist_id == ordered.id && l.deleted == Some(false))
        .collect();
    links.sort_by_key(|l| l.position);
    assert_eq!(
        links
            .iter()
            .map(|l| (l.position, l.content_id.clone()))
            .collect::<Vec<_>>(),
        ids.iter()
            .cloned()
            .enumerate()
            .map(|(i, id)| (i as i64 + 1, id))
            .collect::<Vec<_>>()
    );
    let tag_row = after
        .tags
        .iter()
        .find(|t| t.name == "Written" && t.attribute == 0)
        .expect("the new tag");
    let category = after
        .tags
        .iter()
        .find(|t| t.id == *tag_row.parent_id.as_ref().unwrap())
        .unwrap();
    assert_eq!(category.name, "Cuerithm Acceptance");
    assert!(after.tag_links.iter().any(|l| l.tag_id == tag_row.id
        && l.content_id == track.identity.content_id
        && l.deleted == Some(false)));
    assert_eq!(after.playlists.len(), before.playlists.len() + 2);
    // Every other track is byte-for-byte what it was.
    for (id, hash) in others_before {
        let now = after
            .tracks
            .iter()
            .find(|t| t.identity.content_id == id)
            .unwrap();
        assert_eq!(now.raw_hash, hash, "unrelated track {id} changed");
    }
    // An identical second write is a recorded no-op.
    let again = WritePlan {
        tracks: vec![TrackWrite {
            expected_fingerprint: written.raw_hash.clone(),
            ..plan.tracks[0].clone()
        }],
        playlists: plan.playlists.clone(),
        ..Default::default()
    };
    let mut db = WriteDatabase::open(&snapshot.path, &key).unwrap();
    let repeat = db.apply(&again, &CancellationToken::default()).unwrap();
    drop(db);
    assert_eq!(repeat.usn, None);
    assert_eq!((repeat.tracks_unchanged, repeat.cues_replaced), (1, 0));
    assert_eq!(
        (
            repeat.playlists_unchanged,
            repeat.playlists_created,
            repeat.tags_replaced
        ),
        (2, 0, 0)
    );
    // A write based on the old observation is a conflict, never a silent overwrite.
    let mut db = WriteDatabase::open(&snapshot.path, &key).unwrap();
    let stale = db.apply(&plan, &CancellationToken::default()).unwrap_err();
    assert!(stale.downcast_ref::<ConflictError>().is_some());
    drop(db);

    // Publish the written copy over the stand-in: backup first, then verified, then identical in content.
    let backups = tempfile::tempdir().unwrap();
    let published = publish_verified_copy(
        &PublishRequest {
            live: &stand_in.path,
            work: &snapshot.path,
            key: &key,
            backup_dir: backups.path(),
            base_usn: usn(&before),
        },
        &CancellationToken::default(),
    )
    .unwrap_or_else(|error| panic!("{error:#}"));
    assert_eq!(
        (published.usn_before, published.usn_after),
        (usn(&before), usn(&before) + 1)
    );
    let (live_after, live_checks, live_schema) = collection(&stand_in.path, &key);
    assert!(live_checks.integrity_ok);
    assert_eq!(
        live_checks.foreign_key_issues,
        checks_before.foreign_key_issues
    );
    assert_eq!(live_schema, schema);
    let (work_final, _, _) = collection(&snapshot.path, &key);
    let hashes = |c: &RekordboxCollection| {
        let mut all: Vec<_> = c
            .tracks
            .iter()
            .map(|t| (t.identity.content_id.clone(), t.raw_hash.clone()))
            .collect();
        all.sort();
        all
    };
    assert_eq!(hashes(&live_after), hashes(&work_final));
    assert_eq!(live_after.playlists.len(), work_final.playlists.len());
    assert_eq!(
        live_after.playlist_links.len(),
        work_final.playlist_links.len()
    );
    // The backup is the library as it was before the write.
    let (backed_up, backup_checks, _) = collection(&published.backup, &key);
    assert!(backup_checks.integrity_ok);
    assert_eq!(hashes(&backed_up), hashes(&before));
    let journal = recover_publication(&published.journal, &key).unwrap();
    assert_eq!(journal.state, "committed");
    // Publishing the same copy again is refused: the library has moved past the copy's starting point.
    let again = publish_verified_copy(
        &PublishRequest {
            live: &stand_in.path,
            work: &snapshot.path,
            key: &key,
            backup_dir: backups.path(),
            base_usn: usn(&before),
        },
        &CancellationToken::default(),
    );
    assert!(
        again
            .unwrap_err()
            .to_string()
            .contains("changed the library")
    );
}
