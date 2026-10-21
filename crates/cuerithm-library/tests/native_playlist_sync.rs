use cuerithm_library::*;
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::*;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn collection() -> RekordboxCollection {
    let mut collection:RekordboxCollection=serde_json::from_value(json!({
        "identity":{"database_id":"authored","device_id":"device","database_version":"7"},
        "schema_fingerprint":"a".repeat(64), "tracks":[],
        "playlists":[
            {"id":"folder","uuid":"folder-uuid","parent_id":"root","name":"Sets","attribute":1,"sequence":1,"deleted":false,"raw_hash":"f"},
            {"id":"list","uuid":"list-uuid","parent_id":"folder","name":"Set","attribute":0,"sequence":1,"deleted":false,"raw_hash":"l"}
        ], "playlist_links":[],
        "tags":[
            {"id":"category","parent_id":"root","name":"Mood","attribute":1,"sequence":1,"deleted":false,"raw_hash":"c"},
            {"id":"tag","parent_id":"category","name":"Warm","attribute":0,"sequence":1,"deleted":false,"raw_hash":"t"}
        ], "tag_links":[],"records":{},"issues":[]
    })).unwrap();
    collection.records = [
        "djmdProperty",
        "agentRegistry",
        "djmdContent",
        "djmdArtist",
        "djmdAlbum",
        "djmdGenre",
        "djmdLabel",
        "djmdKey",
        "djmdCue",
        "contentCue",
        "djmdPlaylist",
        "djmdSongPlaylist",
        "djmdMyTag",
        "djmdSongMyTag",
    ]
    .into_iter()
    .map(|t| (t.into(), vec![]))
    .collect();
    collection
}
fn observe(lib: &Library, input: &RekordboxCollection) -> NativeImportReport {
    lib.import_rekordbox_collection(
        input,
        NativeImportOptions {
            settings_revision: lib.settings().unwrap().revision,
            content_ids: None,
            initial_owners: BTreeMap::new(),
        },
        &CancellationToken::default(),
    )
    .unwrap()
}
fn import(lib: &Library, input: &RekordboxCollection) -> NativePlaylistImportReport {
    let seen = observe(lib, input);
    lib.import_native_playlists(
        &seen.target_id,
        input,
        NativePlaylistImportOptions {
            native_revision: seen.revision,
            playlist_revision: lib.playlist_state().unwrap().revision,
            materialize_native_ids: BTreeSet::new(),
        },
    )
    .unwrap()
}
fn export(lib: &Library, target: &str, ids: Vec<String>) -> NativePlaylistPlan {
    lib.plan_native_playlists(
        target,
        NativePlaylistExportOptions {
            native_revision: lib.native_target(target).unwrap().revision,
            playlist_ids: ids,
            category_ids: vec![],
            materialize_smart_playlists: BTreeSet::new(),
        },
    )
    .unwrap()
}
fn add_tracks(input: &mut RekordboxCollection, dir: &std::path::Path) {
    for id in ["a", "b"] {
        let path = dir.join(format!("{id}.flac"));
        std::fs::write(&path, b"authored local audio placeholder").unwrap();
        let track:ImportedTrack=serde_json::from_value(json!({
            "identity":{"database_id":"authored","content_id":id,"uuid":format!("uuid-{id}"),"master_song_id":format!("master-{id}")},
            "metadata":{"title":id,"artist":"Author","album":"Album","album_artist":"","genre":"House","label":"","key":"Am","comment":"","remixer":"","composer":"","lyricist":"","mix_name":""},
            "source":{"path":path,"resolved_path":path,"service_id":0},"bpm":128.0,"duration_seconds":10.0,"deleted":false,"cues":[],"tag_ids":[],"raw_hash":id.repeat(64),"metadata_hash":id.repeat(64)
        })).unwrap();
        input
            .records
            .get_mut("djmdContent")
            .unwrap()
            .push(TargetRecord {
                table: "djmdContent".into(),
                id: id.into(),
                values: BTreeMap::from([("ID".into(), TargetValue::Text(id.into()))]),
                fingerprint: track.raw_hash.clone(),
            });
        input.tracks.push(track);
    }
    input.playlist_links = vec![
        PlaylistLink {
            id: "first".into(),
            playlist_id: "list".into(),
            content_id: "b".into(),
            position: 1,
            deleted: Some(false),
            raw_hash: "first".into(),
        },
        PlaylistLink {
            id: "second".into(),
            playlist_id: "list".into(),
            content_id: "a".into(),
            position: 2,
            deleted: Some(false),
            raw_hash: "second".into(),
        },
    ];
    input.tag_links = vec![TagLink {
        id: "tag-link".into(),
        tag_id: "tag".into(),
        content_id: "a".into(),
        deleted: Some(false),
        raw_hash: "tag-link".into(),
    }];
}

#[test]
fn explicit_import_preserves_native_hierarchy_restart_identity_and_noop() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("library");
    let lib = Library::open(&db).unwrap();
    let input = collection();
    let first = import(&lib, &input);
    assert_eq!(first.playlists_created, 2);
    assert_eq!(first.tags_created, 1);
    assert!(lib.jobs().unwrap().is_empty());
    let state = lib.playlist_state().unwrap();
    let folder = state.nodes.iter().find(|n| n.name == "Sets").unwrap();
    let list = state.nodes.iter().find(|n| n.name == "Set").unwrap();
    assert_eq!(list.parent_id.as_deref(), Some(folder.id.as_str()));
    let id = list.id.clone();
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let again = import(&lib, &input);
    assert!(!again.changed);
    assert_eq!(lib.playlist_state().unwrap().revision, state.revision);
    assert!(
        export(&lib, &first.target_id, vec![id])
            .playlists
            .is_empty()
    );
}

#[test]
fn reused_native_id_and_stale_revision_reject_entire_import() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let mut input = collection();
    let first = import(&lib, &input);
    let before = lib.playlist_state().unwrap();
    input.playlists[1].uuid = Some("replacement".into());
    let seen = observe(&lib, &input);
    assert!(
        lib.import_native_playlists(
            &seen.target_id,
            &input,
            NativePlaylistImportOptions {
                native_revision: seen.revision,
                playlist_revision: before.revision,
                materialize_native_ids: BTreeSet::new()
            }
        )
        .is_err()
    );
    assert_eq!(lib.playlist_state().unwrap().nodes, before.nodes);
    assert!(
        lib.import_native_playlists(
            &first.target_id,
            &input,
            NativePlaylistImportOptions {
                native_revision: 0,
                playlist_revision: before.revision,
                materialize_native_ids: BTreeSet::new()
            }
        )
        .is_err()
    );
}

#[test]
fn import_does_not_overwrite_local_edits_and_preserves_unrelated_undo() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let input = collection();
    import(&lib, &input);
    let user = lib
        .apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some("user".into()),
                parent_id: None,
                kind: PlaylistKind::Playlist,
                name: "User".into(),
                definition: None,
            },
            lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    assert!(!import(&lib, &input).changed);
    lib.undo_playlist_edit(
        user.edit_id.as_deref().unwrap(),
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    let id = lib
        .playlist_state()
        .unwrap()
        .nodes
        .iter()
        .find(|n| n.name == "Set")
        .unwrap()
        .id
        .clone();
    lib.apply_playlist_edit(
        PlaylistEdit::Rename {
            id,
            name: "Local".into(),
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    let seen = observe(&lib, &input);
    assert!(
        lib.import_native_playlists(
            &seen.target_id,
            &input,
            NativePlaylistImportOptions {
                native_revision: seen.revision,
                playlist_revision: lib.playlist_state().unwrap().revision,
                materialize_native_ids: BTreeSet::new()
            }
        )
        .is_err()
    );
}

#[test]
fn unknown_smart_rules_require_explicit_whole_list_materialization() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let mut input = collection();
    input.playlists[1].attribute = 4;
    input.playlists[1].smart_rule = Some("<unknown opaque='true'/>".into());
    let seen = observe(&lib, &input);
    let mut opts = NativePlaylistImportOptions {
        native_revision: seen.revision,
        playlist_revision: 0,
        materialize_native_ids: BTreeSet::new(),
    };
    assert!(
        lib.import_native_playlists(&seen.target_id, &input, opts.clone())
            .is_err()
    );
    opts.materialize_native_ids.insert("list".into());
    let result = lib
        .import_native_playlists(&seen.target_id, &input, opts)
        .unwrap();
    assert_eq!(result.materialized_native_ids, vec!["list"]);
    assert!(
        lib.playlist_state()
            .unwrap()
            .nodes
            .iter()
            .all(|n| n.kind != PlaylistKind::Smartlist)
    );
}

#[test]
fn same_named_external_entities_are_never_claimed_or_removed() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let input = collection();
    let imported = import(&lib, &input);
    let state = lib.playlist_state().unwrap();
    let id = state
        .nodes
        .iter()
        .find(|n| n.name == "Set")
        .unwrap()
        .id
        .clone();
    lib.apply_playlist_edit(PlaylistEdit::Remove { id: id.clone() }, state.revision)
        .unwrap();
    assert!(
        export(&lib, &imported.target_id, vec![id])
            .playlists
            .is_empty()
    );
    let created = lib
        .apply_playlist_edit(
            PlaylistEdit::Create {
                id: Some("new".into()),
                parent_id: None,
                kind: PlaylistKind::Playlist,
                name: "Set".into(),
                definition: None,
            },
            lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let plan = export(&lib, &imported.target_id, created.created_ids);
    assert!(matches!(
        &plan.playlists[0],
        NativePlaylistMutation::Upsert { identity: None, .. }
    ));
}

#[test]
fn native_members_and_mytags_import_without_preparing_or_overwriting_ready_tracks() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("db");
    let lib = Library::open(&db).unwrap();
    let mut input = collection();
    add_tracks(&mut input, d.path());
    let seen = observe(&lib, &input);
    let a = lib
        .native_track_link(&seen.target_id, "a")
        .unwrap()
        .local_track_id
        .unwrap();
    let b = lib
        .native_track_link(&seen.target_id, "b")
        .unwrap()
        .local_track_id
        .unwrap();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE tracks SET state='ready',title='Local manual title',native_metadata_json='{}'",
        [],
    )
    .unwrap();
    let jobs = lib.jobs().unwrap().len();
    lib.import_native_playlists(
        &seen.target_id,
        &input,
        NativePlaylistImportOptions {
            native_revision: seen.revision,
            playlist_revision: 0,
            materialize_native_ids: BTreeSet::new(),
        },
    )
    .unwrap();
    let list = lib
        .playlist_state()
        .unwrap()
        .nodes
        .iter()
        .find(|n| n.name == "Set")
        .unwrap()
        .id
        .clone();
    assert_eq!(
        lib.manual_playlist_members(&list).unwrap(),
        vec![b, a.clone()]
    );
    assert_eq!(lib.jobs().unwrap().len(), jobs);
    let linked:(String,String,String)=conn.query_row("SELECT c.name,t.name,x.title FROM track_custom_tags l JOIN custom_tags t ON t.id=l.tag_id JOIN custom_tag_categories c ON c.id=t.category_id JOIN tracks x ON x.id=l.track_id WHERE l.track_id=?1",[&a],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        linked,
        ("Mood".into(), "Warm".into(), "Local manual title".into())
    );
    assert!(
        export(&lib, &seen.target_id, vec![list])
            .playlists
            .is_empty()
    );
}

#[test]
fn external_local_tag_name_collision_rolls_back_playlist_import() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("db");
    let lib = Library::open(&db).unwrap();
    let conn = rusqlite::Connection::open(&db).unwrap();
    conn.execute(
        "INSERT INTO custom_tag_categories VALUES('external','Mood')",
        [],
    )
    .unwrap();
    let input = collection();
    let seen = observe(&lib, &input);
    assert!(
        lib.import_native_playlists(
            &seen.target_id,
            &input,
            NativePlaylistImportOptions {
                native_revision: seen.revision,
                playlist_revision: 0,
                materialize_native_ids: BTreeSet::new()
            }
        )
        .is_err()
    );
    assert!(lib.playlist_state().unwrap().nodes.is_empty());
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM native_playlist_bindings", [], |r| r
            .get::<_, u64>(
            0
        ))
        .unwrap(),
        0
    );
}

#[test]
fn publication_receipts_establish_owned_identity_and_deletion_survives_restart() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("db");
    let lib = Library::open(&db).unwrap();
    let mut input = collection();
    let initial = import(&lib, &input);
    lib.apply_playlist_edit(
        PlaylistEdit::Create {
            id: Some("owned".into()),
            parent_id: None,
            kind: PlaylistKind::Playlist,
            name: "Created".into(),
            definition: None,
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    let plan = export(&lib, &initial.target_id, vec!["owned".into()]);
    input.playlists.push(ImportedPlaylist {
        id: "allocated".into(),
        uuid: Some("allocated-uuid".into()),
        parent_id: Some("root".into()),
        name: "Created".into(),
        attribute: 0,
        sequence: 2,
        smart_rule: None,
        deleted: Some(false),
        raw_hash: "published".into(),
    });
    observe(&lib, &input);
    let receipt = NativePlaylistPublicationReceipt {
        kind: NativePlaylistObjectKind::Playlist,
        local_id: "owned".into(),
        native_id: "allocated".into(),
        created_link_ids: BTreeSet::new(),
    };
    lib.acknowledge_native_playlist_plan(&plan, &input, std::slice::from_ref(&receipt))
        .unwrap();
    lib.acknowledge_native_playlist_plan(&plan, &input, &[receipt])
        .unwrap();
    assert!(
        export(&lib, &initial.target_id, vec!["owned".into()])
            .playlists
            .is_empty()
    );
    drop(lib);
    let lib = Library::open(&db).unwrap();
    lib.apply_playlist_edit(
        PlaylistEdit::Remove { id: "owned".into() },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    let plan = export(&lib, &initial.target_id, vec!["owned".into()]);
    assert!(
        matches!(&plan.playlists[0],NativePlaylistMutation::Remove{identity,..} if identity.owned&&identity.native_id=="allocated"&&identity.uuid.as_deref()==Some("allocated-uuid"))
    );
}

#[test]
fn publication_receipt_cannot_claim_existing_external_object() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let mut input = collection();
    input
        .records
        .get_mut("djmdPlaylist")
        .unwrap()
        .push(TargetRecord {
            table: "djmdPlaylist".into(),
            id: "list".into(),
            values: BTreeMap::new(),
            fingerprint: "l".into(),
        });
    let initial = import(&lib, &input);
    lib.apply_playlist_edit(
        PlaylistEdit::Create {
            id: Some("new".into()),
            parent_id: None,
            kind: PlaylistKind::Playlist,
            name: "Set".into(),
            definition: None,
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    let plan = export(&lib, &initial.target_id, vec!["new".into()]);
    assert!(
        lib.acknowledge_native_playlist_plan(
            &plan,
            &input,
            &[NativePlaylistPublicationReceipt {
                kind: NativePlaylistObjectKind::Playlist,
                local_id: "new".into(),
                native_id: "list".into(),
                created_link_ids: BTreeSet::new()
            }]
        )
        .is_err()
    );
}

#[test]
fn remote_rename_and_move_import_atomically_and_stale_local_plan_rejects() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let mut input = collection();
    let first = import(&lib, &input);
    let list = lib
        .playlist_state()
        .unwrap()
        .nodes
        .iter()
        .find(|n| n.name == "Set")
        .unwrap()
        .id
        .clone();
    let plan = export(&lib, &first.target_id, vec![list.clone()]);
    lib.validate_native_playlist_plan(&plan).unwrap();
    input.playlists[1].name = "Renamed remotely".into();
    input.playlists[1].parent_id = Some("root".into());
    input.playlists[1].sequence = 1;
    input.playlists[0].sequence = 2;
    import(&lib, &input);
    assert!(lib.validate_native_playlist_plan(&plan).is_err());
    let row = lib
        .playlist_state()
        .unwrap()
        .nodes
        .iter()
        .find(|n| n.id == list)
        .unwrap()
        .clone();
    assert_eq!(row.name, "Renamed remotely");
    assert!(row.parent_id.is_none());
    assert_eq!(row.position, 0);
    assert!(
        export(&lib, &first.target_id, vec![list])
            .playlists
            .is_empty()
    );
}

#[test]
fn native_smart_name_edit_keeps_entire_opaque_rule() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let mut input = collection();
    input.playlists[1].attribute = 4;
    input.playlists[1].smart_rule = Some("<entire opaque='payload'/>".into());
    let seen = observe(&lib, &input);
    lib.import_native_playlists(
        &seen.target_id,
        &input,
        NativePlaylistImportOptions {
            native_revision: seen.revision,
            playlist_revision: 0,
            materialize_native_ids: BTreeSet::from(["list".into()]),
        },
    )
    .unwrap();
    let list = lib
        .playlist_state()
        .unwrap()
        .nodes
        .iter()
        .find(|n| n.name == "Set")
        .unwrap()
        .id
        .clone();
    lib.apply_playlist_edit(
        PlaylistEdit::Rename {
            id: list.clone(),
            name: "Renamed".into(),
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    let plan = export(&lib, &seen.target_id, vec![list]);
    assert!(
        matches!(&plan.playlists[0],NativePlaylistMutation::Upsert{representation:NativePlaylistRepresentation::NativeSmart{opaque_rule,..},..} if opaque_rule=="<entire opaque='payload'/>")
    );
}

#[test]
fn local_smart_export_requires_explicit_materialization_and_fresh_complete_membership() {
    use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let input = collection();
    let seen = import(&lib, &input);
    lib.apply_playlist_edit(
        PlaylistEdit::Create {
            id: Some("smart".into()),
            parent_id: None,
            kind: PlaylistKind::Smartlist,
            name: "Missing files".into(),
            definition: Some(SmartDefinition {
                match_all: true,
                rules: vec![SmartRule {
                    id: "r".into(),
                    field: "CustomFileMissing".into(),
                    operator: Operator::BooleanTrue,
                    value: RuleValue::None,
                    or: false,
                }],
            }),
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    let mut opts = NativePlaylistExportOptions {
        native_revision: lib.native_target(&seen.target_id).unwrap().revision,
        playlist_ids: vec!["smart".into()],
        category_ids: vec![],
        materialize_smart_playlists: BTreeSet::new(),
    };
    assert!(
        lib.plan_native_playlists(&seen.target_id, opts.clone())
            .is_err()
    );
    opts.materialize_smart_playlists.insert("smart".into());
    assert!(
        lib.plan_native_playlists(&seen.target_id, opts.clone())
            .is_err()
    );
    lib.refresh_smart_lists(1_792_324_800_000).unwrap();
    let plan = lib.plan_native_playlists(&seen.target_id, opts).unwrap();
    assert!(
        matches!(&plan.playlists[0],NativePlaylistMutation::Upsert{representation:NativePlaylistRepresentation::MaterializedSmart{content_ids,..},..} if content_ids.is_empty())
    );
    assert_eq!(
        lib.playlist_state()
            .unwrap()
            .nodes
            .iter()
            .find(|n| n.id == "smart")
            .unwrap()
            .kind,
        PlaylistKind::Smartlist
    );
}

#[test]
fn mytag_native_uuid_reuse_is_rejected_without_partial_playlist_changes() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let mut input = collection();
    input.tags[1].uuid = Some("original-tag-uuid".into());
    import(&lib, &input);
    let before = lib.playlist_state().unwrap();
    input.tags[1].uuid = Some("reused-tag-uuid".into());
    input.playlists[1].name = "Should roll back".into();
    let seen = observe(&lib, &input);
    assert!(
        lib.import_native_playlists(
            &seen.target_id,
            &input,
            NativePlaylistImportOptions {
                native_revision: seen.revision,
                playlist_revision: before.revision,
                materialize_native_ids: BTreeSet::new()
            }
        )
        .is_err()
    );
    assert_eq!(lib.playlist_state().unwrap().nodes, before.nodes);
}

#[test]
fn owned_link_removal_never_claims_external_members_and_receipts_survive_later_edits() {
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("db");
    let lib = Library::open(&db).unwrap();
    let mut input = collection();
    add_tracks(&mut input, d.path());
    let seen = observe(&lib, &input);
    let a = lib
        .native_track_link(&seen.target_id, "a")
        .unwrap()
        .local_track_id
        .unwrap();
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute("UPDATE tracks SET state='ready'", [])
        .unwrap();
    lib.import_native_playlists(
        &seen.target_id,
        &input,
        NativePlaylistImportOptions {
            native_revision: seen.revision,
            playlist_revision: 0,
            materialize_native_ids: BTreeSet::new(),
        },
    )
    .unwrap();
    lib.apply_playlist_edit(
        PlaylistEdit::Create {
            id: Some("owned".into()),
            parent_id: None,
            kind: PlaylistKind::Playlist,
            name: "Created".into(),
            definition: None,
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    lib.apply_playlist_edit(
        PlaylistEdit::AddMembers {
            id: "owned".into(),
            track_ids: vec![a.clone()],
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    let plan = export(&lib, &seen.target_id, vec!["owned".into()]);
    input.playlists.push(ImportedPlaylist {
        id: "allocated".into(),
        uuid: Some("owned-uuid".into()),
        parent_id: Some("root".into()),
        name: "Created".into(),
        attribute: 0,
        sequence: 2,
        smart_rule: None,
        deleted: Some(false),
        raw_hash: "created".into(),
    });
    input.playlist_links.push(PlaylistLink {
        id: "own-a".into(),
        playlist_id: "allocated".into(),
        content_id: "a".into(),
        position: 1,
        deleted: Some(false),
        raw_hash: "own-a".into(),
    });
    input.playlist_links.push(PlaylistLink {
        id: "external-b".into(),
        playlist_id: "allocated".into(),
        content_id: "b".into(),
        position: 2,
        deleted: Some(false),
        raw_hash: "external-b".into(),
    });
    observe(&lib, &input);
    let receipt = NativePlaylistPublicationReceipt {
        kind: NativePlaylistObjectKind::Playlist,
        local_id: "owned".into(),
        native_id: "allocated".into(),
        created_link_ids: BTreeSet::from(["own-a".into()]),
    };
    lib.acknowledge_native_playlist_plan(&plan, &input, std::slice::from_ref(&receipt))
        .unwrap();
    lib.apply_playlist_edit(
        PlaylistEdit::RemoveMembers {
            id: "owned".into(),
            track_ids: vec![a],
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    // Restart recovery can replay the old acknowledgement after a newer user edit.
    drop(lib);
    let lib = Library::open(&db).unwrap();
    lib.acknowledge_native_playlist_plan(&plan, &input, &[receipt])
        .unwrap();
    let changed = export(&lib, &seen.target_id, vec!["owned".into()]);
    assert!(
        matches!(&changed.playlists[0],NativePlaylistMutation::Upsert{identity:Some(identity),representation:NativePlaylistRepresentation::Ordinary{content_ids},preserve_external_members:true,..} if content_ids.is_empty()&&identity.owned_link_ids==BTreeSet::from(["own-a".into()]))
    );
    assert!(
        lib.acknowledge_native_playlist_plan(
            &changed,
            &input,
            &[NativePlaylistPublicationReceipt {
                kind: NativePlaylistObjectKind::Playlist,
                local_id: "owned".into(),
                native_id: "allocated".into(),
                created_link_ids: BTreeSet::new()
            }]
        )
        .is_err()
    );
    lib.apply_playlist_edit(
        PlaylistEdit::Remove { id: "owned".into() },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
    assert!(
        lib.plan_native_playlists(
            &seen.target_id,
            NativePlaylistExportOptions {
                native_revision: lib.native_target(&seen.target_id).unwrap().revision,
                playlist_ids: vec!["owned".into()],
                category_ids: vec![],
                materialize_smart_playlists: BTreeSet::new()
            }
        )
        .is_err()
    );
}
