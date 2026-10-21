use super::*;

fn reorder_round_trip(legacy_receipt: bool) {
    let f = fixture(false);
    cue_fixture(&f);
    let cancel = CancellationToken::default();
    let prior = load(&f.library, &f.id).unwrap();
    f.library.discard_rekordbox_sync(&f.id).unwrap();
    let second_source = f._dir.path().canonicalize().unwrap().join("second.wav");
    fs::copy(f._dir.path().join("authored.wav"), &second_source).unwrap();
    f.library.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,analysis_json,metadata_json,musical_key,bpm,duration,created,updated) SELECT 'second',?1,'Second',source_hash,state,analysis_json,metadata_json,musical_key,bpm,duration,created,updated FROM tracks WHERE id='local'", [crate::model::path_bytes(&second_source)]).unwrap();
    for edit in [
        PlaylistEdit::Create {
            id: Some("set".into()),
            parent_id: None,
            kind: PlaylistKind::Playlist,
            name: "Authored set".into(),
            definition: None,
        },
        PlaylistEdit::AddMembers {
            id: "set".into(),
            track_ids: vec!["local".into(), "second".into()],
        },
    ] {
        f.library
            .apply_playlist_edit(edit, f.library.playlist_state().unwrap().revision)
            .unwrap();
    }
    let request = RekordboxSyncRequest {
        configuration: None,
        target_id: prior.target.id.clone(),
        scope: RekordboxSyncScope::Full,
        options: RekordboxSyncOptions {
            metadata_fields: BTreeSet::from(["title".into()]),
            cues: false,
            beat_grid: false,
            artwork: false,
            playlists: true,
            my_tags: false,
            materialize_smart_playlists: false,
            convert_incompatible_audio: false,
            nearest_color: false,
        },
        decisions: ["local", "second"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    BTreeMap::from([("title".into(), NativeFieldDecision::UseLocal)]),
                )
            })
            .collect(),
    };
    let runtime = runtime();
    let preview = || {
        planning::preview(
            &f.library,
            request.clone(),
            prior.target.clone(),
            &key(),
            Some(&runtime),
            &cancel,
            planning::PreviewPermit::AuthoredFixture,
        )
        .unwrap()
    };
    let first = preview();
    publication::fixture_apply(&f.library, &first.id, &key(), &cancel, &|| Ok(())).unwrap();
    let first_op = load(&f.library, &first.id).unwrap();
    let second_identity = first_op
        .resolved
        .iter()
        .find(|track| track.local_id == "second")
        .unwrap()
        .identity
        .clone();
    assert!(
        first_op
            .resolved
            .iter()
            .any(|track| track.local_id == "second" && track.created)
    );
    let receipt = first_op
        .receipts
        .iter()
        .find(|receipt| receipt.local_id == "set")
        .unwrap();
    assert_eq!(receipt.created_link_ids.len(), 2);
    let links = receipt.created_link_ids.clone();
    let playlist_id = receipt.native_id.clone();
    let native_order = || {
        open(&f.native).prepare("SELECT ContentID FROM djmdSongPlaylist WHERE PlaylistID=?1 AND rb_local_deleted=0 ORDER BY TrackNo").unwrap().query_map([&playlist_id], |row| row.get::<_, String>(0)).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    };
    let original_order = native_order();
    assert_eq!(original_order.len(), 2);
    let before_reorder = native_hash(&f);
    f.library
        .apply_playlist_edit(
            PlaylistEdit::ReorderMembers {
                id: "set".into(),
                track_ids: vec!["second".into(), "local".into()],
            },
            f.library.playlist_state().unwrap().revision,
        )
        .unwrap();
    let reorder = preview();
    assert!(reorder.changed_rows > 0);
    if legacy_receipt {
        // Reproduce the durable version-one journal from before receipt assembly
        // distinguished retained links from newly inserted links.
        let mut op = load(&f.library, &reorder.id).unwrap();
        op.receipts
            .iter_mut()
            .find(|receipt| receipt.local_id == "set")
            .unwrap()
            .created_link_ids = links.clone();
        save(&f.library.conn().unwrap(), &op).unwrap();
        let error = publication::fixture_apply(&f.library, &reorder.id, &key(), &cancel, &|| {
            ensure!(
                status(&f.library, &reorder.id)? != "native_published",
                "Authored interruption after reorder commit"
            );
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("Authored interruption"));
        assert_eq!(
            load(&f.library, &reorder.id).unwrap().status,
            "native_published"
        );
        let connection = f.library.conn().unwrap();
        let (original_remote, original_owned): (String, String) = connection.query_row("SELECT remote_json,owned_links_json FROM native_playlist_bindings WHERE local_id='set'", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        drop(connection);
        let native_after = native_hash(&f);
        let first_link = links.first().unwrap();
        for proof in ["generation", "content", "identity", "ownership"] {
            let mut remote: Value = serde_json::from_str(&original_remote).unwrap();
            let mut owned = links.clone();
            match proof {
                "generation" => remote["link_uuids"][first_link] = json!("recycled-generation"),
                "content" => {
                    remote["links"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|link| link["id"].as_str() == Some(first_link))
                        .unwrap()["content_id"] = json!("unrelated-content")
                }
                "identity" => remote["entity"]["raw_hash"] = json!("unreviewed-before-image"),
                "ownership" => {
                    owned.remove(first_link);
                }
                _ => unreachable!(),
            }
            f.library.conn().unwrap().execute("UPDATE native_playlist_bindings SET remote_json=?1,owned_links_json=?2 WHERE local_id='set'", params![remote.to_string(), serde_json::to_string(&owned).unwrap()]).unwrap();
            let error =
                publication::fixture_recover(&f.library, &reorder.id, &key(), &cancel, &|| Ok(()))
                    .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("Cannot claim an external or missing native member link"),
                "{proof}: {error}"
            );
            assert_eq!(native_hash(&f), native_after);
            assert_eq!(
                load(&f.library, &reorder.id).unwrap().status,
                "native_published"
            );
        }
        f.library.conn().unwrap().execute("UPDATE native_playlist_bindings SET remote_json=?1,owned_links_json=?2 WHERE local_id='set'", params![original_remote, original_owned]).unwrap();
        assert_eq!(
            publication::fixture_recover(&f.library, &reorder.id, &key(), &cancel, &|| Ok(()))
                .unwrap()
                .status,
            "applied"
        );
        assert!(
            publication::fixture_recover(&f.library, &reorder.id, &key(), &cancel, &|| Ok(()))
                .unwrap_err()
                .to_string()
                .contains("does not require recovery")
        );
    } else {
        publication::fixture_apply(&f.library, &reorder.id, &key(), &cancel, &|| Ok(())).unwrap();
        assert!(
            load(&f.library, &reorder.id)
                .unwrap()
                .receipts
                .iter()
                .all(|receipt| receipt.created_link_ids.is_empty())
        );
    }
    assert_eq!(
        native_order(),
        original_order.iter().rev().cloned().collect::<Vec<_>>()
    );
    let binding: String = f
        .library
        .conn()
        .unwrap()
        .query_row(
            "SELECT owned_links_json FROM native_playlist_bindings WHERE local_id='set'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<BTreeSet<String>>(&binding).unwrap(),
        links
    );
    let mapped = f
        .library
        .native_track_link(&first_op.native_target_id, &second_identity.content_id)
        .unwrap();
    assert_eq!(mapped.local_track_id.as_deref(), Some("second"));
    let noop = preview();
    assert_eq!(noop.changed_rows, 0);
    f.library.discard_rekordbox_sync(&noop.id).unwrap();
    assert_eq!(
        publication::fixture_restore(&f.library, &reorder.id, &key(), &cancel, &|| Ok(()))
            .unwrap()
            .status,
        "restored"
    );
    assert_eq!(native_hash(&f), before_reorder);
    assert_eq!(native_order(), original_order);
    assert!(second_source.exists());
}

#[test]
fn coordinator_two_track_reorder_acknowledges_retained_links_and_restores() {
    reorder_round_trip(false);
}

#[test]
fn coordinator_two_track_reorder_recovers_legacy_receipt_after_native_commit() {
    reorder_round_trip(true);
}
