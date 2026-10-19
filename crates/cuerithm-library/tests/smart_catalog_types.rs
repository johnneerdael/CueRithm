use cuerithm_library::{Library, SmartPreviewScope};
use cuerithm_smartlists::{FieldKind, Operator, RuleValue, SmartDefinition, SmartRule};
use rusqlite::{Connection, params};

const NOW: i64 = 1_792_324_800_000;

struct Fixture {
    _dir: tempfile::TempDir,
    lib: Library,
    conn: Connection,
    unknown: String,
    known: String,
}

fn fixture(unknown_first: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("db");
    let lib = Library::open(&db).unwrap();
    let conn = Connection::open(&db).unwrap();
    let mut ids = Vec::new();
    for (index, bits) in [None, Some(24)].into_iter().enumerate() {
        let path = dir.path().join(format!("authored-{index}.flac"));
        std::fs::write(
            &path,
            format!("Authored catalog input {index}; no decoding"),
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
        let info = cuerithm_media::MediaInfo {
            codec: if bits.is_some() { "flac" } else { "mp3" }.into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: bits,
            tags: Default::default(),
            properties: Default::default(),
        };
        let sort = if (index == 0) == unknown_first {
            "alpha"
        } else {
            "beta"
        };
        conn.execute(
            "UPDATE tracks SET state='ready',sort_title=?1,metadata_json=?2 WHERE id=?3",
            params![sort, serde_json::to_string(&info).unwrap(), id],
        )
        .unwrap();
        ids.push(id);
    }
    Fixture {
        _dir: dir,
        lib,
        conn,
        unknown: ids.remove(0),
        known: ids.remove(0),
    }
}

fn depth_rule() -> SmartDefinition {
    SmartDefinition {
        match_all: true,
        rules: vec![SmartRule {
            id: "depth".into(),
            field: "audio.bits_per_sample".into(),
            operator: Operator::NumberGreaterThan,
            value: RuleValue::Number(16.),
            or: false,
        }],
    }
}

#[test]
fn nullable_audio_metadata_keeps_numeric_kind_in_either_track_order() {
    for unknown_first in [true, false] {
        let f = fixture(unknown_first);
        assert_eq!(
            f.lib.smart_field_catalog().unwrap().catalog.fields["audio.bits_per_sample"].kind,
            FieldKind::Number,
            "A missing bit depth must not determine the field type"
        );
        let preview = f
            .lib
            .preview_smart_list(depth_rule(), SmartPreviewScope::default(), NOW)
            .unwrap();
        assert_eq!(preview.track_ids, vec![f.known]);
    }
}

#[test]
fn changing_sort_order_cannot_break_saved_numeric_metadata_rules() {
    let f = fixture(false);
    let preview = f
        .lib
        .preview_smart_list(depth_rule(), SmartPreviewScope::default(), NOW)
        .unwrap();
    let saved = f
        .lib
        .save_smart_preview(
            &preview.id,
            None,
            "High bit depth".into(),
            None,
            preview.stamp.playlist_revision,
        )
        .unwrap();
    let id = &saved.created_ids[0];
    assert!(f.lib.refresh_smart_lists(NOW).unwrap().errors.is_empty());
    assert_eq!(f.lib.playlist_members(id).unwrap(), vec![f.known.clone()]);
    // A real rename updates title and sort_title together and invalidates inputs.
    f.conn
        .execute(
            "UPDATE tracks SET title='A changed title',sort_title='a' WHERE id=?1",
            [&f.unknown],
        )
        .unwrap();
    let refreshed = f.lib.refresh_smart_lists(NOW + 1).unwrap();
    assert!(
        refreshed.errors.is_empty(),
        "A rename changed catalog typing: {:?}",
        refreshed.errors
    );
    assert_eq!(f.lib.playlist_members(id).unwrap(), vec![f.known]);
}

#[test]
fn malformed_native_metadata_cannot_fall_back_to_stale_album_artist() {
    let f = fixture(false);
    let info = serde_json::json!({"codec":"flac","sample_rate":44100,"channels":2,"bits_per_sample":24,"tags":{"album_artist":"Stale album artist"}});
    f.conn
        .execute(
            "UPDATE tracks SET metadata_json=?1,native_metadata_json='invalid-json' WHERE id=?2",
            params![info.to_string(), f.known],
        )
        .unwrap();
    let mut rules = depth_rule();
    rules.rules[0].field = "albumArtist".into();
    rules.rules[0].operator = Operator::StringContains;
    rules.rules[0].value = RuleValue::Text("Stale".into());
    let error = f
        .lib
        .preview_smart_list(rules, SmartPreviewScope::default(), NOW)
        .unwrap_err();
    assert!(error.to_string().contains("albumArtist"), "{error:#}");
}

#[test]
fn standard_mix_resolves_the_subtitle_tag_written_by_catalog_tagging() {
    let f = fixture(false);
    let info = serde_json::json!({"codec":"flac","sample_rate":44100,"channels":2,"bits_per_sample":24,"tags":{"subtitle":"Extended Mix"}});
    f.conn
        .execute(
            "UPDATE tracks SET metadata_json=?1 WHERE id=?2",
            params![info.to_string(), f.known],
        )
        .unwrap();
    let mut rules = depth_rule();
    rules.rules[0].field = "mix".into();
    rules.rules[0].operator = Operator::StringEquals;
    rules.rules[0].value = RuleValue::Text("Extended Mix".into());
    let preview = f
        .lib
        .preview_smart_list(rules, SmartPreviewScope::default(), NOW)
        .unwrap();
    assert_eq!(preview.track_ids, vec![f.known]);
}

#[test]
fn all_unknown_bit_depths_still_have_the_known_numeric_schema() {
    let f = fixture(true);
    let json: String = f
        .conn
        .query_row(
            "SELECT metadata_json FROM tracks WHERE id=?1",
            [&f.known],
            |r| r.get(0),
        )
        .unwrap();
    let mut info: cuerithm_media::MediaInfo = serde_json::from_str(&json).unwrap();
    info.bits_per_sample = None;
    f.conn
        .execute(
            "UPDATE tracks SET metadata_json=?1 WHERE id=?2",
            params![serde_json::to_string(&info).unwrap(), f.known],
        )
        .unwrap();
    assert_eq!(
        f.lib.smart_field_catalog().unwrap().catalog.fields["audio.bits_per_sample"].kind,
        FieldKind::Number
    );
    assert!(
        f.lib
            .preview_smart_list(depth_rule(), SmartPreviewScope::default(), NOW)
            .unwrap()
            .track_ids
            .is_empty()
    );
}

#[test]
fn mixed_dynamic_property_types_normalize_to_text_independently_of_track_order() {
    for unknown_first in [true, false] {
        let f = fixture(unknown_first);
        for (id, value) in [
            (&f.unknown, serde_json::json!("unknown")),
            (&f.known, serde_json::json!(24)),
        ] {
            let json: String = f
                .conn
                .query_row("SELECT metadata_json FROM tracks WHERE id=?1", [id], |r| {
                    r.get(0)
                })
                .unwrap();
            let mut info: cuerithm_media::MediaInfo = serde_json::from_str(&json).unwrap();
            info.properties.insert("format.fixture_value".into(), value);
            f.conn
                .execute(
                    "UPDATE tracks SET metadata_json=?1 WHERE id=?2",
                    params![serde_json::to_string(&info).unwrap(), id],
                )
                .unwrap();
        }
        assert_eq!(
            f.lib.smart_field_catalog().unwrap().catalog.fields["format.fixture_value"].kind,
            FieldKind::Text
        );
        let mut rules = depth_rule();
        rules.rules[0].field = "format.fixture_value".into();
        rules.rules[0].operator = Operator::StringEquals;
        rules.rules[0].value = RuleValue::Text("24".into());
        assert_eq!(
            f.lib
                .preview_smart_list(rules, SmartPreviewScope::default(), NOW)
                .unwrap()
                .track_ids,
            vec![f.known]
        );
    }
}
