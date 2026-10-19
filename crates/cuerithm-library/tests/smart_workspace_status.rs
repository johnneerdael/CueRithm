use cuerithm_library::*;
use cuerithm_smartlists::{Operator, RuleValue, SmartDefinition, SmartRule};
use rusqlite::{Connection, params};

const NOW: i64 = 1_792_324_800_000;

struct Fixture {
    _dir: tempfile::TempDir,
    lib: Library,
    conn: Connection,
    ids: Vec<String>,
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("db");
    let lib = Library::open(&db).unwrap();
    let conn = Connection::open(&db).unwrap();
    let mut ids = vec![];
    for title in ["Alpha", "Beta"] {
        let path = dir.path().join(format!("{title}.flac"));
        std::fs::write(
            &path,
            format!("Authored status fixture {title}; no decoding"),
        )
        .unwrap();
        let job = lib
            .enqueue(&[path], lib.settings().unwrap().revision)
            .unwrap()
            .remove(0);
        let id = lib
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == job)
            .unwrap()
            .track_id;
        conn.execute(
            "UPDATE tracks SET state='ready',title=?1,sort_title=?1 WHERE id=?2",
            params![title, id],
        )
        .unwrap();
        ids.push(id);
    }
    Fixture {
        _dir: dir,
        lib,
        conn,
        ids,
    }
}
fn definition(field: &str, operator: Operator, value: RuleValue) -> SmartDefinition {
    SmartDefinition {
        match_all: true,
        rules: vec![SmartRule {
            id: "rule".into(),
            field: field.into(),
            operator,
            value,
            or: false,
        }],
    }
}
fn title_rule(title: &str) -> SmartDefinition {
    definition(
        "title",
        Operator::StringContains,
        RuleValue::Text(title.into()),
    )
}
fn bad_rule() -> SmartDefinition {
    definition("cloudUpload", Operator::BooleanTrue, RuleValue::None)
}
fn create(
    lib: &Library,
    id: &str,
    parent: Option<&str>,
    kind: PlaylistKind,
    rules: Option<SmartDefinition>,
) {
    lib.apply_playlist_edit(
        PlaylistEdit::Create {
            id: Some(id.into()),
            parent_id: parent.map(str::to_owned),
            kind,
            name: id.into(),
            definition: rules,
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
}
fn hierarchy(lib: &Library) {
    create(lib, "root", None, PlaylistKind::Folder, None);
    create(lib, "nested", Some("root"), PlaylistKind::Folder, None);
}
fn assert_status(lib: &Library, id: &str, at: i64, state: SmartState, has_results: bool) {
    let workspace = lib.playlist_workspace_at(at).unwrap();
    let batch = workspace
        .statuses
        .iter()
        .find(|s| s.playlist_id == id)
        .unwrap();
    let single = lib.playlist_evaluation_status_at(id, at).unwrap();
    for status in [batch, &single] {
        assert_eq!(status.state, state, "{id}");
        assert_eq!(
            serde_json::to_value(status).unwrap()["has_last_good_results"],
            serde_json::json!(has_results),
            "{id}: cached-result availability must survive folder aggregation"
        );
        assert_eq!(
            status.last_good_revision, None,
            "Folders do not have a coherent child input revision"
        );
    }
}
fn visible(lib: &Library, id: &str) -> Vec<String> {
    lib.track_ids(TrackQuery {
        playlist_id: Some(id.into()),
        ..TrackQuery::default()
    })
    .unwrap()
}

#[test]
fn nested_folders_retain_last_good_availability_through_stale_and_failed_children() {
    let f = fixture();
    hierarchy(&f.lib);
    create(
        &f.lib,
        "cached",
        Some("nested"),
        PlaylistKind::Smartlist,
        Some(title_rule("Alpha")),
    );
    create(
        &f.lib,
        "fresh",
        Some("root"),
        PlaylistKind::Smartlist,
        Some(title_rule("Beta")),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    f.conn
        .execute(
            "UPDATE tracks SET title='Alpha changed' WHERE id=?1",
            [&f.ids[0]],
        )
        .unwrap();
    for id in ["root", "nested"] {
        assert_status(&f.lib, id, NOW + 1, SmartState::Stale, true);
    }
    assert!(visible(&f.lib, "root").contains(&f.ids[0]));
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::SetDefinition {
                id: "cached".into(),
                definition: bad_rule(),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    assert_eq!(f.lib.refresh_smart_lists(NOW + 2).unwrap().errors.len(), 1);
    let cached = f
        .lib
        .playlist_evaluation_status_at("cached", NOW + 2)
        .unwrap();
    let fresh = f
        .lib
        .playlist_evaluation_status_at("fresh", NOW + 2)
        .unwrap();
    assert_ne!(cached.last_good_revision, fresh.last_good_revision);
    for id in ["root", "nested"] {
        assert_status(&f.lib, id, NOW + 2, SmartState::Error, true);
    }
    assert!(visible(&f.lib, "root").contains(&f.ids[0]));
    assert!(visible(&f.lib, "root").contains(&f.ids[1]));
}

#[test]
fn nested_folders_without_a_successful_smart_evaluation_have_no_last_good_results() {
    let f = fixture();
    hierarchy(&f.lib);
    create(
        &f.lib,
        "never-good",
        Some("nested"),
        PlaylistKind::Smartlist,
        Some(bad_rule()),
    );
    for id in ["root", "nested"] {
        assert_status(&f.lib, id, NOW, SmartState::Pending, false);
    }
    assert_eq!(f.lib.refresh_smart_lists(NOW + 1).unwrap().errors.len(), 1);
    for id in ["root", "nested"] {
        assert_status(&f.lib, id, NOW + 1, SmartState::Error, false);
        assert!(visible(&f.lib, id).is_empty());
    }
}

#[test]
fn successful_empty_smart_results_still_count_as_last_good_after_a_failure() {
    let f = fixture();
    hierarchy(&f.lib);
    create(
        &f.lib,
        "empty",
        Some("nested"),
        PlaylistKind::Smartlist,
        Some(title_rule("Absent title")),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    for id in ["root", "nested"] {
        assert_status(&f.lib, id, NOW, SmartState::Ready, true);
        assert!(visible(&f.lib, id).is_empty());
    }
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::SetDefinition {
                id: "empty".into(),
                definition: bad_rule(),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.lib.refresh_smart_lists(NOW + 1).unwrap();
    for id in ["root", "nested"] {
        assert_status(&f.lib, id, NOW + 1, SmartState::Error, true);
        assert!(visible(&f.lib, id).is_empty());
    }
}

#[test]
fn manual_only_folders_stay_ready_without_smart_last_good_state() {
    let f = fixture();
    hierarchy(&f.lib);
    create(
        &f.lib,
        "manual",
        Some("nested"),
        PlaylistKind::Playlist,
        None,
    );
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "manual".into(),
                track_ids: vec![f.ids[0].clone()],
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    for id in ["root", "nested"] {
        assert_status(&f.lib, id, NOW, SmartState::Ready, false);
        assert_eq!(visible(&f.lib, id), vec![f.ids[0].clone()]);
    }
}
