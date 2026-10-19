use cuerithm_library::*;
use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
use rusqlite::params;

struct Fixture {
    dir: tempfile::TempDir,
    lib: Library,
    ids: Vec<String>,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("db")).unwrap();
    let mut ids = vec![];
    for name in ["Alpha", "Beta", "Gamma", "Delta"] {
        let path = dir.path().join(format!("{name}.flac"));
        std::fs::write(&path, b"Authored playlist fixture; no decoding").unwrap();
        lib.enqueue(&[path], lib.settings().unwrap().revision)
            .unwrap();
        let id = lib
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| !ids.contains(&j.track_id))
            .unwrap()
            .track_id;
        let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
        conn.execute(
            "UPDATE tracks SET state='ready',title=?1,sort_title=lower(?1) WHERE id=?2",
            params![name, id],
        )
        .unwrap();
        ids.push(id);
    }
    Fixture { dir, lib, ids }
}
fn create(
    lib: &Library,
    id: &str,
    parent: Option<&str>,
    kind: PlaylistKind,
    name: &str,
    rules: Option<SmartDefinition>,
) {
    lib.apply_playlist_edit(
        PlaylistEdit::Create {
            id: Some(id.into()),
            parent_id: parent.map(str::to_owned),
            kind,
            name: name.into(),
            definition: rules,
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
}
fn members(lib: &Library, id: &str, ids: &[String]) {
    lib.apply_playlist_edit(
        PlaylistEdit::AddMembers {
            id: id.into(),
            track_ids: ids.to_vec(),
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
}
fn apply(lib: &Library, request: PlaylistToolRequest) -> PlaylistChange {
    let preview = lib.preview_playlist_tool(request).unwrap();
    lib.apply_playlist_tool(&preview.id, preview.stamp.playlist_revision)
        .unwrap()
}

#[test]
fn ordered_union_intersection_and_library_complement_preserve_sources_and_support_undo() {
    let f = fixture();
    create(&f.lib, "a", None, PlaylistKind::Playlist, "A", None);
    create(&f.lib, "b", None, PlaylistKind::Playlist, "B", None);
    members(&f.lib, "a", &[f.ids[1].clone(), f.ids[0].clone()]);
    members(&f.lib, "b", &[f.ids[0].clone(), f.ids[2].clone()]);
    let before = f.lib.playlist_state().unwrap();
    for (mode, expected) in [
        (
            PlaylistCombineMode::Union,
            vec![f.ids[1].clone(), f.ids[0].clone(), f.ids[2].clone()],
        ),
        (PlaylistCombineMode::Intersection, vec![f.ids[0].clone()]),
        (PlaylistCombineMode::AbsentFromAll, vec![f.ids[3].clone()]),
    ] {
        let change = apply(
            &f.lib,
            PlaylistToolRequest::Combine {
                source_ids: vec!["a".into(), "b".into()],
                parent_id: None,
                name: "Result".into(),
                mode,
            },
        );
        assert_eq!(change.created_ids.len(), 1);
        assert_eq!(
            f.lib
                .manual_playlist_members(&change.created_ids[0])
                .unwrap(),
            expected
        );
        assert_eq!(
            f.lib.manual_playlist_members("a").unwrap(),
            vec![f.ids[1].clone(), f.ids[0].clone()]
        );
        f.lib
            .undo_playlist_edit(change.edit_id.as_ref().unwrap(), change.revision)
            .unwrap();
    }
    assert_eq!(f.lib.playlist_state().unwrap().nodes, before.nodes);
    assert_eq!(
        std::fs::read(f.dir.path().join("Alpha.flac")).unwrap(),
        b"Authored playlist fixture; no decoding"
    );
    assert_eq!(f.lib.cue_revision_count(&f.ids[0]).unwrap(), 0);
}

#[test]
fn sibling_sort_and_visible_member_reorder_keep_hidden_slots() {
    let f = fixture();
    for (id, name) in [
        ("a", "Zulu"),
        ("b", "Hidden"),
        ("c", "Alpha"),
        ("d", "Last"),
    ] {
        create(&f.lib, id, None, PlaylistKind::Playlist, name, None);
    }
    apply(
        &f.lib,
        PlaylistToolRequest::SortSiblings {
            parent_id: None,
            ids: Some(vec!["a".into(), "c".into()]),
            order: PlaylistSortOrder::NameAscending,
        },
    );
    let ids = f
        .lib
        .playlist_state()
        .unwrap()
        .nodes
        .into_iter()
        .filter(|n| !n.deleted)
        .map(|n| n.id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["c", "b", "a", "d"]);
    members(&f.lib, "a", &f.ids);
    f.lib.set_archived(&f.ids[1], true).unwrap();
    apply(
        &f.lib,
        PlaylistToolRequest::ReorderMembers {
            id: "a".into(),
            track_ids: vec![f.ids[2].clone(), f.ids[0].clone()],
        },
    );
    assert_eq!(
        f.lib.manual_playlist_members("a").unwrap(),
        vec![
            f.ids[2].clone(),
            f.ids[1].clone(),
            f.ids[0].clone(),
            f.ids[3].clone()
        ]
    );
}

#[test]
fn pinned_previews_reject_tree_races_and_failed_sql_is_atomic_and_retryable() {
    let f = fixture();
    create(&f.lib, "a", None, PlaylistKind::Playlist, "A", None);
    let request = || PlaylistToolRequest::Combine {
        source_ids: vec!["a".into()],
        parent_id: None,
        name: "Result".into(),
        mode: PlaylistCombineMode::Union,
    };
    let old = f.lib.preview_playlist_tool(request()).unwrap();
    create(&f.lib, "other", None, PlaylistKind::Playlist, "Other", None);
    assert!(
        f.lib
            .apply_playlist_tool(&old.id, old.stamp.playlist_revision)
            .is_err()
    );
    let preview = f.lib.preview_playlist_tool(request()).unwrap();
    let conn = rusqlite::Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_tool BEFORE INSERT ON playlists WHEN NEW.name='Result' BEGIN SELECT RAISE(FAIL,'injected'); END;").unwrap();
    let before = f.lib.playlist_state().unwrap();
    assert!(
        f.lib
            .apply_playlist_tool(&preview.id, preview.stamp.playlist_revision)
            .is_err()
    );
    assert_eq!(f.lib.playlist_state().unwrap(), before);
    conn.execute_batch("DROP TRIGGER reject_tool").unwrap();
    f.lib
        .apply_playlist_tool(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    assert!(
        f.lib
            .apply_playlist_tool(&preview.id, preview.stamp.playlist_revision)
            .is_err()
    );
}

#[test]
fn recursive_snapshot_keeps_original_smart_definitions_and_reports_complete_memberships() {
    let f = fixture();
    create(&f.lib, "folder", None, PlaylistKind::Folder, "Folder", None);
    let definition = SmartDefinition {
        match_all: true,
        rules: vec![SmartRule {
            id: "title".into(),
            field: "title".into(),
            operator: Operator::StringEquals,
            value: RuleValue::Text("Alpha".into()),
            or: false,
        }],
    };
    create(
        &f.lib,
        "smart",
        Some("folder"),
        PlaylistKind::Smartlist,
        "Smart",
        Some(definition.clone()),
    );
    let change = apply(
        &f.lib,
        PlaylistToolRequest::Snapshot {
            source_id: "folder".into(),
            parent_id: None,
            name: "Snapshot".into(),
        },
    );
    let state = f.lib.playlist_state().unwrap();
    assert_eq!(
        state
            .nodes
            .iter()
            .find(|n| n.id == "smart")
            .unwrap()
            .definition,
        Some(definition)
    );
    let copied = state
        .nodes
        .iter()
        .find(|n| change.created_ids.contains(&n.id) && n.kind == PlaylistKind::Playlist)
        .unwrap();
    assert_eq!(
        f.lib.manual_playlist_members(&copied.id).unwrap(),
        vec![f.ids[0].clone()]
    );
}

#[test]
fn prefixes_follow_tree_order_and_pure_name_edits_ignore_unrelated_track_publication() {
    let f = fixture();
    for (id, name) in [("a", "03 - First"), ("b", "Hidden"), ("c", "09 - Last")] {
        create(&f.lib, id, None, PlaylistKind::Playlist, name, None);
    }
    let preview = f
        .lib
        .preview_playlist_tool(PlaylistToolRequest::Prefix {
            ids: vec!["c".into(), "a".into()],
            prefix: PlaylistPrefix::Number {
                start: 1,
                leading_zero: true,
                replace_existing: true,
            },
        })
        .unwrap();
    let conn = rusqlite::Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute(
        "UPDATE tracks SET title='Later metadata' WHERE id=?1",
        [&f.ids[0]],
    )
    .unwrap();
    let applied = f
        .lib
        .apply_playlist_tool(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    let state = f.lib.playlist_state().unwrap();
    assert_eq!(
        state
            .nodes
            .iter()
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>(),
        vec!["01 First", "Hidden", "02 Last"]
    );
    f.lib
        .undo_playlist_edit(applied.edit_id.as_ref().unwrap(), applied.revision)
        .unwrap();
    let applied = apply(
        &f.lib,
        PlaylistToolRequest::Uppercase {
            ids: vec!["a".into()],
        },
    );
    assert_eq!(
        f.lib
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .find(|n| n.id == "a")
            .unwrap()
            .name,
        "03 - FIRST"
    );
    f.lib
        .undo_playlist_edit(applied.edit_id.as_ref().unwrap(), applied.revision)
        .unwrap();
}

#[test]
fn smart_tools_pin_metadata_and_unrelated_failed_lists_do_not_block_selected_sources() {
    let f = fixture();
    let rules = |field: &str, operator, value| SmartDefinition {
        match_all: true,
        rules: vec![SmartRule {
            id: "r".into(),
            field: field.into(),
            operator,
            value,
            or: false,
        }],
    };
    create(
        &f.lib,
        "smart",
        None,
        PlaylistKind::Smartlist,
        "Smart",
        Some(rules(
            "title",
            Operator::StringEquals,
            RuleValue::Text("Alpha".into()),
        )),
    );
    create(
        &f.lib,
        "unavailable",
        None,
        PlaylistKind::Smartlist,
        "Unavailable",
        Some(rules("cloudUpload", Operator::BooleanTrue, RuleValue::None)),
    );
    let preview = f
        .lib
        .preview_playlist_tool(PlaylistToolRequest::Snapshot {
            source_id: "smart".into(),
            parent_id: None,
            name: "Captured".into(),
        })
        .unwrap();
    assert_eq!(preview.changes[0].track_count, 1);
    let conn = rusqlite::Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute(
        "UPDATE tracks SET title='Later metadata' WHERE id=?1",
        [&f.ids[0]],
    )
    .unwrap();
    assert!(
        f.lib
            .apply_playlist_tool(&preview.id, preview.stamp.playlist_revision)
            .is_err()
    );
    assert!(
        !f.lib
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .any(|n| n.name == "Captured")
    );
}

#[test]
fn lexical_sort_uses_javascript_code_units_and_count_sort_uses_direct_membership() {
    let f = fixture();
    for (id, name, kind) in [
        ("a", "Set 2", PlaylistKind::Playlist),
        ("b", "Set 10", PlaylistKind::Playlist),
        ("c", "\u{e000}", PlaylistKind::Playlist),
        ("d", "😀", PlaylistKind::Folder),
    ] {
        create(&f.lib, id, None, kind, name, None);
    }
    members(&f.lib, "a", &f.ids[..2]);
    members(&f.lib, "b", &f.ids[..1]);
    apply(
        &f.lib,
        PlaylistToolRequest::SortSiblings {
            parent_id: None,
            ids: None,
            order: PlaylistSortOrder::NameAscending,
        },
    );
    assert_eq!(
        f.lib
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b", "a", "d", "c"]
    );
    apply(
        &f.lib,
        PlaylistToolRequest::SortSiblings {
            parent_id: None,
            ids: None,
            order: PlaylistSortOrder::TrackCountDescending,
        },
    );
    assert_eq!(
        f.lib
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b", "c", "d"]
    );
}

#[test]
fn over_capacity_snapshots_and_invalid_prefix_names_leave_no_partial_plan() {
    let f = fixture();
    create(&f.lib, "folder", None, PlaylistKind::Folder, "Folder", None);
    let mut conn = rusqlite::Connection::open(f.dir.path().join("db")).unwrap();
    let tx = conn.transaction().unwrap();
    for i in 0..5000 {
        tx.execute("INSERT INTO playlists(id,parent_id,kind,name,position,last_edit_id) VALUES(?1,'folder','playlist',?2,?3,'fixture')",params![format!("child-{i}"),format!("Child {i}"),i]).unwrap();
    }
    tx.commit().unwrap();
    let before = f.lib.playlist_state().unwrap();
    assert!(
        f.lib
            .preview_playlist_tool(PlaylistToolRequest::Snapshot {
                source_id: "folder".into(),
                parent_id: None,
                name: "Too large".into()
            })
            .is_err()
    );
    assert_eq!(f.lib.playlist_state().unwrap(), before);
    create(
        &f.lib,
        "long",
        None,
        PlaylistKind::Playlist,
        &"a".repeat(255),
        None,
    );
    assert!(
        f.lib
            .preview_playlist_tool(PlaylistToolRequest::Prefix {
                ids: vec!["long".into()],
                prefix: PlaylistPrefix::Text { text: "X".into() }
            })
            .is_err()
    );
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM playlist_previews", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
}
