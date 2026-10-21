use cuerithm_rekordbox::*;
use std::{collections::BTreeMap, path::PathBuf};

fn row(table: &str, id: &str, values: &[(&str, TargetValue)]) -> TargetRecord {
    let values = values
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    TargetRecord {
        table: table.into(),
        id: id.into(),
        values,
        fingerprint: format!("proof-{table}-{id}"),
    }
}
fn text(s: &str) -> TargetValue {
    TargetValue::Text(s.into())
}
fn collection() -> RekordboxCollection {
    RekordboxCollection {
        identity: DatabaseIdentity {
            database_id: "db".into(),
            device_id: Some("device".into()),
            database_version: Some("7".into()),
            local_update_count: Some(41),
        },
        schema_fingerprint: "schema".into(),
        tracks: vec![],
        playlists: vec![],
        playlist_links: vec![],
        tags: vec![],
        tag_links: vec![],
        issues: vec![],
        records: BTreeMap::from([(
            "agentRegistry".into(),
            vec![row(
                "agentRegistry",
                "localUpdateCount",
                &[("int_1", TargetValue::Integer(41))],
            )],
        )]),
    }
}
fn request() -> NativePlanRequest {
    NativePlanRequest {
        source: PathBuf::from("/owned/master.db"),
        now: "2026-10-21 12:00:00.000 +00:00".into(),
        iso_now: "2026-10-21T12:00:00.000Z".into(),
        id_seed: [7; 32],
        tracks: vec![],
        entities: vec![],
        auxiliary_records: vec![],
        share_root: None,
    }
}
fn draft() -> NativeTrackDraft {
    NativeTrackDraft {
        local_id: "local".into(),
        identity: None,
        expected_raw_hash: None,
        source_path: PathBuf::from("/music/example.flac"),
        fields: BTreeMap::from([
            ("title".into(), text("Example")),
            ("artist".into(), text("Artist")),
        ]),
        cues: None,
        cue_bpm: None,
        cue_colors: None,
        replace_cues: vec![],
        analysis_path: None,
    }
}

#[test]
fn empty_plan_does_not_advance_counter() {
    let result = prepare_native_rows(&collection(), &request()).unwrap();
    assert!(result.plan.rows.is_empty());
    assert_eq!(result.local_usn, None);
}

#[test]
fn new_track_has_foreign_identity_and_one_transaction_usn() {
    let mut request = request();
    request.tracks.push(draft());
    let result = prepare_native_rows(&collection(), &request).unwrap();
    assert_eq!(result.local_usn, Some(42));
    assert_eq!(result.tracks.len(), 1);
    let content = result
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdContent")
        .unwrap();
    let artist = result
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdArtist")
        .unwrap();
    assert_eq!(content.values["ArtistID"], text(&artist.id));
    assert_eq!(content.values["FolderPath"], text("/music/example.flac"));
    assert_eq!(content.values["MasterDBID"], text("db"));
    assert_eq!(content.values["DeviceID"], text("device"));
    assert_eq!(content.values["TrackInfoUpdated"], text("0"));
    assert_eq!(content.values["rb_local_usn"], TargetValue::Integer(42));
    assert_eq!(artist.values["rb_local_usn"], TargetValue::Integer(42));
    assert_eq!(
        result
            .plan
            .rows
            .iter()
            .filter(|r| r.table == "agentRegistry")
            .count(),
        1
    );
}

#[test]
fn explicit_clear_is_distinct_from_an_absent_field() {
    let mut collection = collection();
    let mut request = request();
    request.tracks.push(draft());
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    fold(&mut collection, &prepared.plan);
    bind(&mut request.tracks[0], &prepared, &collection);
    request.tracks[0].fields.clear();
    assert!(
        prepare_native_rows(&collection, &request)
            .unwrap()
            .plan
            .rows
            .is_empty()
    );
    request.tracks[0]
        .fields
        .insert("artist".into(), TargetValue::Null);
    let cleared = prepare_native_rows(&collection, &request).unwrap();
    let content = cleared
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdContent")
        .unwrap();
    assert_eq!(content.values["ArtistID"], TargetValue::Null);
    assert!(!cleared.plan.rows.iter().any(|r| r.table == "djmdArtist"));
}

#[test]
fn native_field_types_are_checked_before_staging_and_integral_reals_are_canonical() {
    let mut request = request();
    let mut draft = draft();
    draft.fields.insert("title".into(), TargetValue::Integer(7));
    request.tracks.push(draft);
    assert!(prepare_native_rows(&collection(), &request).is_err());
    request.tracks[0]
        .fields
        .insert("title".into(), text("Title"));
    request.tracks[0]
        .fields
        .insert("duration_seconds".into(), TargetValue::Real(120.));
    let prepared = prepare_native_rows(&collection(), &request).unwrap();
    let content = prepared
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdContent")
        .unwrap();
    assert_eq!(content.values["Length"], TargetValue::Integer(120));
}

#[test]
fn reviewed_artwork_uses_observed_native_prefix_for_add_clear_and_repeat() {
    let mut collection = collection();
    let mut request = request();
    let mut track = draft();
    track.fields.insert(
        "artwork_path".into(),
        text("/PIONEER/Artwork/CUERITHM/run/cover.jpg"),
    );
    request.tracks.push(track);
    let added = prepare_native_rows(&collection, &request).unwrap();
    let content = added
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdContent")
        .unwrap();
    assert_eq!(
        content.values["ImagePath"],
        text("/PIONEER/Artwork/CUERITHM/run/cover.jpg")
    );
    assert!(!added.plan.rows.iter().any(|r| r.table == "djmdAlbum"));
    fold(&mut collection, &added.plan);
    bind(&mut request.tracks[0], &added, &collection);
    assert!(
        prepare_native_rows(&collection, &request)
            .unwrap()
            .plan
            .rows
            .is_empty()
    );
    request.tracks[0]
        .fields
        .insert("artwork_path".into(), text(""));
    let cleared = prepare_native_rows(&collection, &request).unwrap();
    let content = cleared
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdContent")
        .unwrap();
    assert_eq!(content.values["ImagePath"], text(""));
    assert_eq!(content.values["TrackInfoUpdated"], text("1"));
    fold(&mut collection, &cleared.plan);
    bind(&mut request.tracks[0], &cleared, &collection);
    assert!(
        prepare_native_rows(&collection, &request)
            .unwrap()
            .plan
            .rows
            .is_empty()
    );
    request.tracks[0]
        .fields
        .insert("artwork_path".into(), TargetValue::Null);
    assert!(
        prepare_native_rows(&collection, &request)
            .unwrap()
            .plan
            .rows
            .is_empty()
    );
}

#[test]
fn invalid_artwork_references_are_rejected_before_planning() {
    for path in [
        "/Users/dj/cover.jpg",
        "C:\\Users\\dj\\cover.jpg",
        "file:///tmp/cover.jpg",
        "PIONEER/Artwork/cover.jpg",
        "/Artwork/cover.jpg",
        "//PIONEER/Artwork/cover.jpg",
        "/PIONEER/Artwork/../cover.jpg",
        "/PIONEER/Artwork/./cover.jpg",
        "/PIONEER/Artwork//cover.jpg",
        "/PIONEER/Artwork/cover.jpg\0",
        "/PIONEER/Artwork\\cover.jpg",
        "/PIONEER/Artwork/",
        "/PIONEER/Artwork/cover.jpg?raw=1",
        "/PIONEER/Artwork/cover.jpg#part",
    ] {
        let mut request = request();
        let mut track = draft();
        track.fields.insert("artwork_path".into(), text(path));
        request.tracks.push(track);
        assert!(
            prepare_native_rows(&collection(), &request).is_err(),
            "{path:?}"
        );
    }
}

#[test]
fn colored_memory_cue_uses_null_sql_palette_index() {
    let mut request = request();
    let mut track = draft();
    let mut memory = cue(NativeCueSlot::Memory, false);
    memory.memory_color = 2;
    track.cues = Some(vec![memory]);
    request.tracks.push(track);
    let prepared = prepare_native_rows(&collection(), &request).unwrap();
    let memory = prepared
        .plan
        .rows
        .iter()
        .find(|row| row.table == "djmdCue")
        .unwrap();
    assert_eq!(memory.values["Color"], TargetValue::Integer(1));
    assert_eq!(memory.values["ColorTableIndex"], TargetValue::Null);
}

#[test]
fn native_sql_palette_nullability_matches_the_cue_destination() {
    let mut request = request();
    let mut track = draft();
    track.cues = Some(vec![cue(NativeCueSlot::Memory, false)]);
    track.cue_colors = Some(vec![NativeCueColor {
        color: 1,
        color_table_index: Some(0),
    }]);
    request.tracks.push(track);
    assert!(prepare_native_rows(&collection(), &request).is_err());
    request.tracks[0].cue_colors.as_mut().unwrap()[0].color_table_index = None;
    assert!(prepare_native_rows(&collection(), &request).is_ok());
    request.tracks[0].cues.as_mut().unwrap()[0].slot = NativeCueSlot::Hot(0);
    assert!(prepare_native_rows(&collection(), &request).is_err());
}

fn add_entity(
    collection: &mut RekordboxCollection,
    kind: NativeEntityKind,
    id: &str,
    parent: &str,
) -> NativeEntityDraft {
    let table = match kind {
        NativeEntityKind::Playlist => "djmdPlaylist",
        NativeEntityKind::MyTag => "djmdMyTag",
    };
    let uuid = format!("uuid-{id}");
    let record = row(
        table,
        id,
        &[
            ("ID", text(id)),
            ("UUID", text(&uuid)),
            ("Name", text(id)),
            ("ParentID", text(parent)),
            ("Attribute", TargetValue::Integer(0)),
            ("Seq", TargetValue::Integer(1)),
            ("SmartList", TargetValue::Null),
            ("rb_local_deleted", TargetValue::Integer(0)),
        ],
    );
    let proof = record.fingerprint.clone();
    collection
        .records
        .entry(table.into())
        .or_default()
        .push(record);
    NativeEntityDraft {
        kind,
        local_id: format!("local-{id}"),
        identity: Some(NativeEntityIdentity {
            native_id: id.into(),
            uuid: Some(uuid),
            expected_raw_hash: proof,
            owned: true,
            owned_links: vec![],
        }),
        parent: if parent == "root" {
            None
        } else {
            Some(NativeEntityRef::Native(parent.into()))
        },
        name: id.into(),
        attribute: 0,
        sequence: 1,
        smart_rule: None,
        members: vec![],
        remove: true,
    }
}

fn add_entity_link(
    collection: &mut RekordboxCollection,
    kind: NativeEntityKind,
    id: &str,
    parent: &str,
) -> NativeOwnedRow {
    let (table, parent_field) = match kind {
        NativeEntityKind::Playlist => ("djmdSongPlaylist", "PlaylistID"),
        NativeEntityKind::MyTag => ("djmdSongMyTag", "MyTagID"),
    };
    let record = row(
        table,
        id,
        &[
            ("ID", text(id)),
            (parent_field, text(parent)),
            ("ContentID", text("native-track")),
            ("rb_local_deleted", TargetValue::Integer(0)),
        ],
    );
    let proof = NativeOwnedRow {
        id: id.into(),
        expected_raw_hash: record.fingerprint.clone(),
    };
    collection
        .records
        .entry(table.into())
        .or_default()
        .push(record);
    proof
}

#[test]
fn removal_rejects_live_external_member_links_from_complete_native_records() {
    for kind in [NativeEntityKind::MyTag, NativeEntityKind::Playlist] {
        let mut collection = collection();
        let mut request = request();
        let entity = add_entity(&mut collection, kind, "parent", "root");
        add_entity_link(&mut collection, kind, "external", "parent");
        request.entities.push(entity);
        let before = serde_json::to_vec(&collection).unwrap();
        assert!(
            prepare_native_rows(&collection, &request).is_err(),
            "{kind:?}"
        );
        assert_eq!(serde_json::to_vec(&collection).unwrap(), before);
    }
}

#[test]
fn removal_rejects_an_unbound_native_child_even_when_local_binding_lists_are_empty() {
    for kind in [NativeEntityKind::MyTag, NativeEntityKind::Playlist] {
        let mut collection = collection();
        let mut request = request();
        let entity = add_entity(&mut collection, kind, "parent", "root");
        add_entity(&mut collection, kind, "unbound-child", "parent");
        request.entities.push(entity);
        let before = serde_json::to_vec(&collection).unwrap();
        assert!(
            prepare_native_rows(&collection, &request).is_err(),
            "{kind:?}"
        );
        assert_eq!(serde_json::to_vec(&collection).unwrap(), before);
    }
}

#[test]
fn reviewed_owned_child_and_link_removals_are_leaf_first_and_repeat_without_writes() {
    for kind in [NativeEntityKind::MyTag, NativeEntityKind::Playlist] {
        let mut collection = collection();
        let mut request = request();
        let parent = add_entity(&mut collection, kind, "parent", "root");
        let mut child = add_entity(&mut collection, kind, "child", "parent");
        child
            .identity
            .as_mut()
            .unwrap()
            .owned_links
            .push(add_entity_link(
                &mut collection,
                kind,
                "owned-link",
                "child",
            ));
        request.entities = vec![parent, child];
        let prepared = prepare_native_rows(&collection, &request).unwrap();
        let table = match kind {
            NativeEntityKind::Playlist => "djmdPlaylist",
            NativeEntityKind::MyTag => "djmdMyTag",
        };
        let deleted: Vec<_> = prepared
            .plan
            .rows
            .iter()
            .filter(|row| row.table == table)
            .map(|row| row.id.as_str())
            .collect();
        assert_eq!(deleted, vec!["child", "parent"]);
        assert!(prepared.plan.rows.iter().any(
            |r| r.id == "owned-link" && r.values["rb_local_deleted"] == TargetValue::Integer(1)
        ));
        fold(&mut collection, &prepared.plan);
        for draft in &mut request.entities {
            let identity = draft.identity.as_mut().unwrap();
            identity.expected_raw_hash = collection.records[table]
                .iter()
                .find(|row| row.id == identity.native_id)
                .unwrap()
                .fingerprint
                .clone();
            let link_table = if kind == NativeEntityKind::Playlist {
                "djmdSongPlaylist"
            } else {
                "djmdSongMyTag"
            };
            for proof in &mut identity.owned_links {
                proof.expected_raw_hash = collection.records[link_table]
                    .iter()
                    .find(|row| row.id == proof.id)
                    .unwrap()
                    .fingerprint
                    .clone();
            }
        }
        let repeated = prepare_native_rows(&collection, &request).unwrap();
        assert!(repeated.plan.rows.is_empty());
        assert_eq!(repeated.local_usn, None);
    }
}

#[test]
fn reviewed_relocation_is_explicit_and_album_artist_changes_do_not_mutate_shared_albums() {
    let mut collection = collection();
    let mut request = request();
    let mut draft = draft();
    draft.fields.insert("album".into(), text("Album"));
    draft.fields.insert("album_artist".into(), text("Original"));
    request.tracks.push(draft);
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    fold(&mut collection, &prepared.plan);
    bind(&mut request.tracks[0], &prepared, &collection);
    let old_album = collection.records["djmdAlbum"][0].id.clone();
    request.tracks[0].source_path = PathBuf::from("/music/relocated.flac");
    request.tracks[0].fields.clear();
    assert!(
        prepare_native_rows(&collection, &request)
            .unwrap()
            .plan
            .rows
            .is_empty()
    );
    request.tracks[0]
        .fields
        .insert("source_path".into(), text("/music/relocated.flac"));
    request.tracks[0]
        .fields
        .insert("album_artist".into(), text("Different"));
    let changed = prepare_native_rows(&collection, &request).unwrap();
    let content = changed
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdContent")
        .unwrap();
    assert_eq!(content.values["FolderPath"], text("/music/relocated.flac"));
    assert_eq!(content.values["FileNameL"], text("relocated.flac"));
    assert_ne!(content.values["AlbumID"], text(&old_album));
    assert!(
        !changed
            .plan
            .rows
            .iter()
            .any(|r| r.table == "djmdAlbum" && r.id == old_album)
    );
    let album = changed
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdAlbum")
        .unwrap();
    assert_eq!(album.values["Name"], text("Album"));
    request.tracks[0]
        .fields
        .insert("source_path".into(), text("relative.flac"));
    assert!(prepare_native_rows(&collection, &request).is_err());
}

#[test]
fn all_hot_slots_map_around_the_active_memory_marker() {
    let mut request = request();
    let mut draft = draft();
    draft.cues = Some(
        (0..8)
            .map(|slot| cue(NativeCueSlot::Hot(slot), false))
            .collect(),
    );
    request.tracks.push(draft);
    let prepared = prepare_native_rows(&collection(), &request).unwrap();
    let mut kinds: Vec<_> = prepared
        .plan
        .rows
        .iter()
        .filter(|r| r.table == "djmdCue")
        .map(|r| match r.values["Kind"] {
            TargetValue::Integer(kind) => kind,
            _ => panic!(),
        })
        .collect();
    kinds.sort();
    assert_eq!(kinds, vec![1, 2, 3, 5, 6, 7, 8, 9]);
    request.tracks[0].cues.as_mut().unwrap()[0].color = [255, 0, 0, 255];
    assert!(prepare_native_rows(&collection(), &request).is_err());
    request.tracks[0].cue_colors = Some(
        (0..8)
            .map(|_| NativeCueColor {
                color: -1,
                color_table_index: Some(41),
            })
            .collect(),
    );
    assert!(prepare_native_rows(&collection(), &request).is_ok());
}

#[test]
fn hierarchy_is_topological_and_cycles_are_rejected() {
    let mut request = request();
    request.entities = vec![
        NativeEntityDraft {
            kind: NativeEntityKind::Playlist,
            local_id: "child".into(),
            identity: None,
            parent: Some(NativeEntityRef::Local("parent".into())),
            name: "Child".into(),
            attribute: 0,
            sequence: 1,
            smart_rule: None,
            members: vec![],
            remove: false,
        },
        NativeEntityDraft {
            kind: NativeEntityKind::Playlist,
            local_id: "parent".into(),
            identity: None,
            parent: None,
            name: "Parent".into(),
            attribute: 1,
            sequence: 1,
            smart_rule: None,
            members: vec![],
            remove: false,
        },
    ];
    let prepared = prepare_native_rows(&collection(), &request).unwrap();
    let rows: Vec<_> = prepared
        .plan
        .rows
        .iter()
        .filter(|r| r.table == "djmdPlaylist")
        .collect();
    assert_eq!(rows[0].values["Name"], text("Parent"));
    assert_eq!(rows[1].values["Name"], text("Child"));
    request.entities[1].parent = Some(NativeEntityRef::Local("child".into()));
    assert!(prepare_native_rows(&collection(), &request).is_err());
}

#[test]
fn a_conflicting_unowned_hot_slot_is_rejected_without_replacement() {
    let mut collection = collection();
    let mut request = request();
    request.tracks.push(draft());
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    fold(&mut collection, &prepared.plan);
    bind(&mut request.tracks[0], &prepared, &collection);
    collection.records.insert(
        "djmdCue".into(),
        vec![row(
            "djmdCue",
            "external",
            &[
                ("ContentID", text(&prepared.tracks[0].identity.content_id)),
                ("Kind", TargetValue::Integer(1)),
                ("rb_local_deleted", TargetValue::Integer(0)),
            ],
        )],
    );
    request.tracks[0].cues = Some(vec![cue(NativeCueSlot::Hot(0), false)]);
    assert!(prepare_native_rows(&collection, &request).is_err());
}

fn fold(collection: &mut RekordboxCollection, plan: &NativeWritePlan) {
    for mutation in &plan.rows {
        let rows = collection
            .records
            .entry(mutation.table.clone())
            .or_default();
        if let Some(row) = rows.iter_mut().find(|r| r.id == mutation.id) {
            row.values.extend(mutation.values.clone());
            row.fingerprint.push('x');
        } else {
            rows.push(row(
                &mutation.table,
                &mutation.id,
                &mutation
                    .values
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.clone()))
                    .collect::<Vec<_>>(),
            ));
        }
    }
    if let Some(TargetValue::Integer(count)) =
        collection.records["agentRegistry"][0].values.get("int_1")
    {
        collection.identity.local_update_count = Some(*count as u64);
    }
}
fn bind(
    draft: &mut NativeTrackDraft,
    prepared: &PreparedNativeRows,
    collection: &RekordboxCollection,
) {
    draft.identity = Some(prepared.tracks[0].identity.clone());
    draft.expected_raw_hash = Some(
        collection.records["djmdContent"]
            .iter()
            .find(|r| r.id == prepared.tracks[0].identity.content_id)
            .unwrap()
            .fingerprint
            .clone(),
    );
    draft.replace_cues = prepared.tracks[0]
        .owned_cue_ids
        .iter()
        .map(|id| NativeOwnedRow {
            id: id.clone(),
            expected_raw_hash: collection.records["djmdCue"]
                .iter()
                .find(|r| r.id == *id)
                .unwrap()
                .fingerprint
                .clone(),
        })
        .collect();
}
fn cue(slot: NativeCueSlot, active: bool) -> NativeCue {
    NativeCue {
        slot,
        start: NativeTime::from_seconds(1.2349).unwrap(),
        end: active.then(|| NativeTime::from_seconds(3.2349).unwrap()),
        active_loop: active,
        order_first: 0,
        order_last: 0,
        comment: "Reviewed".into(),
        memory_color: 0,
        color: [0; 4],
        loop_fraction: (0, 0),
    }
}

#[test]
fn repeated_plan_preserves_unknown_columns_and_is_a_true_noop() {
    let mut collection = collection();
    let mut request = request();
    request.tracks.push(draft());
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    fold(&mut collection, &prepared.plan);
    collection.records.get_mut("djmdContent").unwrap()[0]
        .values
        .insert("Opaque".into(), TargetValue::Blob(vec![1, 255, 7]));
    bind(&mut request.tracks[0], &prepared, &collection);
    let repeated = prepare_native_rows(&collection, &request).unwrap();
    assert!(repeated.plan.rows.is_empty());
    assert_eq!(repeated.local_usn, None);
    request.tracks[0]
        .fields
        .insert("title".into(), text("Changed"));
    let changed = prepare_native_rows(&collection, &request).unwrap();
    let content = changed
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdContent")
        .unwrap();
    assert!(!content.values.contains_key("Opaque"));
    assert_eq!(content.values["TrackInfoUpdated"], text("1"));
    assert_eq!(
        content.expected_fingerprint,
        request.tracks[0].expected_raw_hash
    );
}

#[test]
fn reused_native_identity_and_stale_proofs_are_rejected() {
    let mut collection = collection();
    let mut request = request();
    request.tracks.push(draft());
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    fold(&mut collection, &prepared.plan);
    bind(&mut request.tracks[0], &prepared, &collection);
    request.tracks[0].identity.as_mut().unwrap().uuid = Some("reused".into());
    assert!(prepare_native_rows(&collection, &request).is_err());
    bind(&mut request.tracks[0], &prepared, &collection);
    request.tracks[0].expected_raw_hash = Some("stale".into());
    assert!(prepare_native_rows(&collection, &request).is_err());
}

#[test]
fn allocator_uses_independent_columns_and_falls_back_at_u32_limit() {
    let mut collection = collection();
    collection.records.insert(
        "djmdContent".into(),
        vec![row(
            "djmdContent",
            "4294967295",
            &[("MasterSongID", text("31"))],
        )],
    );
    let mut request = request();
    request.tracks.push(draft());
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert_ne!(prepared.tracks[0].identity.content_id, "4294967295");
    assert_eq!(
        prepared.tracks[0].identity.master_song_id.as_deref(),
        Some("32")
    );
    assert!(
        prepared.tracks[0]
            .identity
            .content_id
            .parse::<u32>()
            .is_ok()
    );
}

#[test]
fn exact_foreign_names_reuse_and_revive_deleted_rows() {
    let mut collection = collection();
    collection.records.insert(
        "djmdArtist".into(),
        vec![row(
            "djmdArtist",
            "9",
            &[
                ("Name", text("Artist")),
                ("rb_local_deleted", TargetValue::Integer(1)),
                ("Opaque", text("keep")),
            ],
        )],
    );
    let mut request = request();
    request.tracks.push(draft());
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    let artist = prepared
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdArtist")
        .unwrap();
    assert_eq!(artist.id, "9");
    assert!(!artist.values.contains_key("Opaque"));
    assert_eq!(artist.values["rb_data_status"], TargetValue::Integer(257));
    assert_eq!(artist.values["rb_local_deleted"], TargetValue::Integer(0));
}

#[test]
fn cue_slots_active_marker_cache_and_repeat_noop() {
    let mut collection = collection();
    let mut request = request();
    let mut draft = draft();
    draft.fields.insert("bpm".into(), TargetValue::Real(120.));
    draft.cues = Some(vec![
        cue(NativeCueSlot::Memory, false),
        cue(NativeCueSlot::Hot(3), true),
    ]);
    request.tracks.push(draft);
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    let rows: Vec<_> = prepared
        .plan
        .rows
        .iter()
        .filter(|r| r.table == "djmdCue")
        .collect();
    assert_eq!(rows.len(), 3);
    let hot = rows
        .iter()
        .find(|r| r.values["Kind"] == TargetValue::Integer(5))
        .unwrap();
    assert_eq!(hot.values["InMsec"], TargetValue::Integer(1234));
    assert_eq!(hot.values["OutMsec"], TargetValue::Integer(3234));
    assert_eq!(
        hot.values["BeatLoopSize"],
        TargetValue::Integer(4 * 65536 + 1)
    );
    assert_eq!(hot.values["ActiveLoop"], TargetValue::Integer(1));
    assert_eq!(hot.values["rb_local_usn"], TargetValue::Integer(42));
    assert!(
        rows.iter()
            .any(|r| r.values["Kind"] == TargetValue::Integer(4)
                && r.values["ActiveLoop"] == TargetValue::Integer(0))
    );
    let cache = prepared
        .plan
        .rows
        .iter()
        .find(|r| r.table == "contentCue")
        .unwrap();
    let TargetValue::Text(json) = &cache.values["Cues"] else {
        panic!()
    };
    let cached: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(cached.as_array().unwrap().len(), 3);
    assert_eq!(cached[0]["created_at"], "2026-10-21T12:00:00.000Z");
    assert!(cached[0].get("rb_data_status").is_none());
    fold(&mut collection, &prepared.plan);
    bind(&mut request.tracks[0], &prepared, &collection);
    request.tracks[0].replace_cues.reverse();
    let repeated = prepare_native_rows(&collection, &request).unwrap();
    assert!(repeated.plan.rows.is_empty());
    request.tracks[0].cues = Some(vec![]);
    let removed = prepare_native_rows(&collection, &request).unwrap();
    assert_eq!(
        removed
            .plan
            .rows
            .iter()
            .filter(
                |r| r.table == "djmdCue" && r.values["rb_local_deleted"] == TargetValue::Integer(1)
            )
            .count(),
        3
    );
}

fn loop_draft_with_tempo(bpm: f64) -> NativeTrackDraft {
    let mut track = draft();
    let mut saved_loop = cue(NativeCueSlot::Hot(0), true);
    saved_loop.start = NativeTime::from_seconds(1.875).unwrap();
    saved_loop.end = Some(NativeTime::from_seconds(3.75).unwrap());
    track.cues = Some(vec![saved_loop]);
    // Exercise the optional SDK wire field, including old clients that omit it.
    let mut json = serde_json::to_value(track).unwrap();
    json["cue_bpm"] = serde_json::json!(bpm);
    serde_json::from_value(json).unwrap()
}

fn assert_four_beat_loop_without_bpm_write(prepared: &PreparedNativeRows) {
    let cues: Vec<_> = prepared
        .plan
        .rows
        .iter()
        .filter(|row| row.table == "djmdCue")
        .collect();
    assert_eq!(cues.len(), 2);
    for cue in cues {
        assert_eq!(cue.values["BeatLoopSize"], TargetValue::Integer(262145));
    }
    assert!(
        prepared
            .plan
            .rows
            .iter()
            .filter(|row| row.table == "djmdContent")
            .all(|row| !row.values.contains_key("BPM"))
    );
}

#[test]
fn cue_tempo_preserves_unselected_native_bpm_and_repeat_is_noop() {
    let mut collection = collection();
    let mut request = request();
    let mut initial = draft();
    initial
        .fields
        .insert("bpm".into(), TargetValue::Real(149.72));
    request.tracks.push(initial);
    let created = prepare_native_rows(&collection, &request).unwrap();
    fold(&mut collection, &created.plan);
    request.tracks[0] = loop_draft_with_tempo(128.);
    bind(&mut request.tracks[0], &created, &collection);
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert_four_beat_loop_without_bpm_write(&prepared);
    fold(&mut collection, &prepared.plan);
    assert_eq!(
        collection.records["djmdContent"][0].values["BPM"],
        TargetValue::Integer(14972)
    );
    bind(&mut request.tracks[0], &prepared, &collection);
    let repeated = prepare_native_rows(&collection, &request).unwrap();
    assert!(repeated.plan.rows.is_empty());
    assert_eq!(repeated.local_usn, None);
}

#[test]
fn cue_tempo_sets_loop_size_for_new_track_without_selecting_bpm() {
    let mut request = request();
    request.tracks.push(loop_draft_with_tempo(128.));
    let prepared = prepare_native_rows(&collection(), &request).unwrap();
    assert_four_beat_loop_without_bpm_write(&prepared);
}

#[test]
fn invalid_explicit_cue_tempo_is_rejected_for_loops() {
    for bpm in [0., -128., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut request = request();
        let mut track = loop_draft_with_tempo(128.);
        track.cue_bpm = Some(bpm);
        request.tracks.push(track);
        assert!(prepare_native_rows(&collection(), &request).is_err());
    }
}

#[test]
fn legacy_cue_draft_without_tempo_uses_native_bpm() {
    let mut collection = collection();
    let mut request = request();
    let mut initial = draft();
    initial.fields.insert("bpm".into(), TargetValue::Real(128.));
    request.tracks.push(initial);
    let created = prepare_native_rows(&collection, &request).unwrap();
    fold(&mut collection, &created.plan);
    let mut json = serde_json::to_value(loop_draft_with_tempo(128.)).unwrap();
    json.as_object_mut().unwrap().remove("cue_bpm");
    request.tracks[0] = serde_json::from_value(json).unwrap();
    assert!(request.tracks[0].cue_bpm.is_none());
    bind(&mut request.tracks[0], &created, &collection);
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert_four_beat_loop_without_bpm_write(&prepared);
}

#[test]
fn unowned_cues_survive_and_owned_proof_cannot_cross_tracks() {
    let mut collection = collection();
    let mut request = request();
    request.tracks.push(draft());
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    fold(&mut collection, &prepared.plan);
    bind(&mut request.tracks[0], &prepared, &collection);
    collection.records.insert(
        "djmdCue".into(),
        vec![row(
            "djmdCue",
            "external",
            &[
                ("ContentID", text(&prepared.tracks[0].identity.content_id)),
                ("Kind", TargetValue::Integer(2)),
                ("rb_local_deleted", TargetValue::Integer(0)),
            ],
        )],
    );
    request.tracks[0].cues = Some(vec![cue(NativeCueSlot::Memory, false)]);
    let changed = prepare_native_rows(&collection, &request).unwrap();
    assert!(
        !changed
            .plan
            .rows
            .iter()
            .any(|r| r.table == "djmdCue" && r.id == "external")
    );
    request.tracks[0].replace_cues = vec![NativeOwnedRow {
        id: "external".into(),
        expected_raw_hash: "stale".into(),
    }];
    assert!(prepare_native_rows(&collection, &request).is_err());
}

#[test]
fn analysis_reference_creates_one_auxiliary_file_without_fake_status_columns() {
    let collection = collection();
    let mut request = request();
    let mut draft = draft();
    draft.analysis_path = Some("/PIONEER/USBANLZ/abc/def/ANLZ0000.DAT".into());
    request.tracks.push(draft);
    assert!(prepare_native_rows(&collection, &request).is_err());
    request.share_root = Some(PathBuf::from("/share"));
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    let file = prepared
        .plan
        .rows
        .iter()
        .find(|r| r.table == "contentFile")
        .unwrap();
    assert!(file.id.contains("_%2FPIONEER%2FUSBANLZ%2F"));
    assert_eq!(
        file.values["rb_local_path"],
        text("/share/PIONEER/USBANLZ/abc/def/ANLZ0000.DAT")
    );
    assert!(!file.values.contains_key("rb_data_status"));
    assert_eq!(file.values["rb_local_usn"], TargetValue::Integer(42));
}

#[test]
fn owned_playlist_membership_uses_local_track_references_and_preserves_external_links() {
    let mut collection = collection();
    let mut request = request();
    request.tracks.push(draft());
    request.entities.push(NativeEntityDraft {
        kind: NativeEntityKind::Playlist,
        local_id: "list".into(),
        identity: None,
        parent: None,
        name: "Set".into(),
        attribute: 0,
        sequence: 1,
        smart_rule: None,
        members: vec![NativeEntityRef::Local("local".into())],
        remove: false,
    });
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    let link = prepared
        .plan
        .rows
        .iter()
        .find(|r| r.table == "djmdSongPlaylist")
        .unwrap();
    assert_eq!(
        link.values["ContentID"],
        text(&prepared.tracks[0].identity.content_id)
    );
    assert_eq!(link.values["TrackNo"], TargetValue::Integer(1));
    fold(&mut collection, &prepared.plan);
    bind(&mut request.tracks[0], &prepared, &collection);
    let entity = &prepared.entities[0];
    let native = &collection.records["djmdPlaylist"][0];
    request.entities[0].identity = Some(NativeEntityIdentity {
        native_id: entity.native_id.clone(),
        uuid: entity.uuid.clone(),
        expected_raw_hash: native.fingerprint.clone(),
        owned: true,
        owned_links: entity
            .owned_link_ids
            .iter()
            .map(|id| NativeOwnedRow {
                id: id.clone(),
                expected_raw_hash: collection.records["djmdSongPlaylist"]
                    .iter()
                    .find(|r| r.id == *id)
                    .unwrap()
                    .fingerprint
                    .clone(),
            })
            .collect(),
    });
    assert!(
        prepare_native_rows(&collection, &request)
            .unwrap()
            .plan
            .rows
            .is_empty()
    );
    collection
        .records
        .get_mut("djmdSongPlaylist")
        .unwrap()
        .push(row(
            "djmdSongPlaylist",
            "external",
            &[
                ("PlaylistID", text(&entity.native_id)),
                ("ContentID", text("other")),
                ("TrackNo", TargetValue::Integer(2)),
                ("rb_local_deleted", TargetValue::Integer(0)),
            ],
        ));
    request.entities[0].members.clear();
    let removal = prepare_native_rows(&collection, &request).unwrap();
    assert!(!removal.plan.rows.iter().any(|r| r.id == "external"));
    assert!(
        removal
            .plan
            .rows
            .iter()
            .any(|r| r.table == "djmdSongPlaylist" && r.id == entity.owned_link_ids[0])
    );
    request.entities[0].remove = true;
    request.entities[0].identity.as_mut().unwrap().owned = false;
    assert!(prepare_native_rows(&collection, &request).is_err());
}

// Authored mixed ownership fixture: only A and C links belong to CueRithm.
fn mixed_playlist() -> (RekordboxCollection, NativePlanRequest) {
    let mut collection = collection();
    collection.records.insert(
        "djmdContent".into(),
        ["A", "B", "C"]
            .into_iter()
            .map(|id| {
                row(
                    "djmdContent",
                    id,
                    &[("rb_local_deleted", TargetValue::Integer(0))],
                )
            })
            .collect(),
    );
    let playlist = row(
        "djmdPlaylist",
        "list",
        &[
            ("Name", text("Set")),
            ("ParentID", text("root")),
            ("Attribute", TargetValue::Integer(0)),
            ("Seq", TargetValue::Integer(1)),
            ("SmartList", TargetValue::Null),
            ("rb_local_deleted", TargetValue::Integer(0)),
        ],
    );
    let links: Vec<_> = [
        ("external-B", "B", 1),
        ("owned-A", "A", 2),
        ("owned-C", "C", 3),
    ]
    .into_iter()
    .map(|(id, content, position)| {
        row(
            "djmdSongPlaylist",
            id,
            &[
                ("PlaylistID", text("list")),
                ("ContentID", text(content)),
                ("TrackNo", TargetValue::Integer(position)),
                ("rb_local_deleted", TargetValue::Integer(0)),
            ],
        )
    })
    .collect();
    let owned_links = links
        .iter()
        .filter(|r| r.id.starts_with("owned-"))
        .map(|r| NativeOwnedRow {
            id: r.id.clone(),
            expected_raw_hash: r.fingerprint.clone(),
        })
        .collect();
    let identity = NativeEntityIdentity {
        native_id: "list".into(),
        uuid: None,
        expected_raw_hash: playlist.fingerprint.clone(),
        owned: true,
        owned_links,
    };
    collection
        .records
        .insert("djmdPlaylist".into(), vec![playlist]);
    collection.records.insert("djmdSongPlaylist".into(), links);
    let mut request = request();
    request.entities.push(NativeEntityDraft {
        kind: NativeEntityKind::Playlist,
        local_id: "list".into(),
        identity: Some(identity),
        parent: None,
        name: "Renamed".into(),
        attribute: 0,
        sequence: 1,
        smart_rule: None,
        members: vec![
            NativeEntityRef::Native("A".into()),
            NativeEntityRef::Native("C".into()),
        ],
        remove: false,
    });
    (collection, request)
}
fn playlist_order(collection: &RekordboxCollection) -> Vec<(String, i64)> {
    let mut result: Vec<_> = collection.records["djmdSongPlaylist"]
        .iter()
        .filter(|r| r.values.get("rb_local_deleted") != Some(&TargetValue::Integer(1)))
        .map(|r| {
            let TargetValue::Text(content) = &r.values["ContentID"] else {
                panic!()
            };
            let TargetValue::Integer(position) = r.values["TrackNo"] else {
                panic!()
            };
            (content.clone(), position)
        })
        .collect();
    result.sort_by_key(|(_, position)| *position);
    assert!(
        result.windows(2).all(|w| w[0].1 < w[1].1),
        "positions must be unique"
    );
    result
}
#[test]
fn playlist_rename_preserves_existing_owned_positions_around_external_links() {
    let (mut collection, request) = mixed_playlist();
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert!(
        !prepared
            .plan
            .rows
            .iter()
            .any(|r| r.table == "djmdSongPlaylist")
    );
    fold(&mut collection, &prepared.plan);
    assert_eq!(
        playlist_order(&collection),
        vec![("B".into(), 1), ("A".into(), 2), ("C".into(), 3)]
    );
}
#[test]
fn playlist_reorder_reserves_foreign_positions_without_claiming_links() {
    let (mut collection, mut request) = mixed_playlist();
    request.entities[0].members.reverse();
    let foreign = collection.records["djmdSongPlaylist"][0].clone();
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert!(!prepared.entities[0].owned_link_ids.contains(&foreign.id));
    assert!(!prepared.plan.rows.iter().any(|r| r.id == foreign.id));
    fold(&mut collection, &prepared.plan);
    assert_eq!(
        collection.records["djmdSongPlaylist"]
            .iter()
            .find(|r| r.id == foreign.id)
            .unwrap()
            .values,
        foreign.values
    );
    assert_eq!(
        playlist_order(&collection),
        vec![("B".into(), 1), ("C".into(), 2), ("A".into(), 3)]
    );
}
#[test]
fn desired_member_with_external_link_keeps_its_position_and_ownership() {
    let (mut collection, mut request) = mixed_playlist();
    request.entities[0]
        .members
        .insert(0, NativeEntityRef::Native("B".into()));
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert!(
        !prepared
            .plan
            .rows
            .iter()
            .any(|r| r.table == "djmdSongPlaylist")
    );
    assert_eq!(prepared.entities[0].owned_link_ids.len(), 2);
    fold(&mut collection, &prepared.plan);
    assert_eq!(
        playlist_order(&collection),
        vec![("B".into(), 1), ("A".into(), 2), ("C".into(), 3)]
    );
}
#[test]
fn reviewed_order_that_cannot_fit_before_foreign_anchor_is_rejected() {
    let (collection, mut request) = mixed_playlist();
    request.entities[0].members = vec![
        NativeEntityRef::Native("A".into()),
        NativeEntityRef::Native("B".into()),
    ];
    let error = prepare_native_rows(&collection, &request).unwrap_err();
    assert!(error.to_string().contains("playlist order"), "{error:#}");
}

#[test]
fn retained_owned_member_keeps_position_when_other_owned_members_are_removed() {
    let (mut collection, mut request) = mixed_playlist();
    request.entities[0].members.pop();
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert!(
        !prepared
            .plan
            .rows
            .iter()
            .any(|r| r.id == "owned-A" || r.id == "external-B")
    );
    fold(&mut collection, &prepared.plan);
    assert_eq!(
        playlist_order(&collection),
        vec![("B".into(), 1), ("A".into(), 2)]
    );
}
#[test]
fn external_member_does_not_revive_a_deleted_owned_link_to_the_same_track() {
    let (mut collection, mut request) = mixed_playlist();
    let deleted = collection
        .records
        .get_mut("djmdSongPlaylist")
        .unwrap()
        .iter_mut()
        .find(|r| r.id == "owned-A")
        .unwrap();
    deleted
        .values
        .insert("rb_local_deleted".into(), TargetValue::Integer(1));
    collection
        .records
        .get_mut("djmdSongPlaylist")
        .unwrap()
        .push(row(
            "djmdSongPlaylist",
            "external-A",
            &[
                ("PlaylistID", text("list")),
                ("ContentID", text("A")),
                ("TrackNo", TargetValue::Integer(2)),
                ("rb_local_deleted", TargetValue::Integer(0)),
            ],
        ));
    request.entities[0].members.pop();
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert!(
        !prepared.entities[0]
            .owned_link_ids
            .iter()
            .any(|id| id == "owned-A" || id == "external-A")
    );
    assert!(
        !prepared
            .plan
            .rows
            .iter()
            .any(|r| r.id == "owned-A" || r.id == "external-A")
    );
    fold(&mut collection, &prepared.plan);
    assert_eq!(
        playlist_order(&collection),
        vec![("B".into(), 1), ("A".into(), 2)]
    );
}

#[test]
fn owned_reorder_can_fit_on_both_sides_of_a_desired_external_member() {
    let (mut collection, mut request) = mixed_playlist();
    for row in collection.records.get_mut("djmdSongPlaylist").unwrap() {
        let position = match row.id.as_str() {
            "external-B" => 2,
            "owned-A" => 1,
            _ => 3,
        };
        row.values
            .insert("TrackNo".into(), TargetValue::Integer(position));
    }
    request.entities[0].members = ["C", "B", "A"]
        .into_iter()
        .map(|id| NativeEntityRef::Native(id.into()))
        .collect();
    let foreign = collection.records["djmdSongPlaylist"][0].values.clone();
    let prepared = prepare_native_rows(&collection, &request).unwrap();
    assert!(!prepared.plan.rows.iter().any(|r| r.id == "external-B"));
    assert!(
        !prepared.entities[0]
            .owned_link_ids
            .contains(&"external-B".into())
    );
    fold(&mut collection, &prepared.plan);
    assert_eq!(collection.records["djmdSongPlaylist"][0].values, foreign);
    assert_eq!(
        playlist_order(&collection),
        vec![("C".into(), 1), ("B".into(), 2), ("A".into(), 3)]
    );
}
