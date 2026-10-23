#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use serde_json::json;
use std::fs;
use support::*;
fn document(entries: &str, playlists: &str) -> String {
    format!(
        r#"<NML VERSION="19"><HEAD PROGRAM="Traktor" VERSION="4.1.0"/><COLLECTION>{entries}</COLLECTION><PLAYLISTS>{playlists}</PLAYLISTS></NML>"#
    )
}
#[test]
fn traktor_preserves_cue_classes_sparse_slots_and_unknown_xml() {
    let root = tempfile::tempdir().unwrap();
    let entries = r#"<ENTRY TITLE="A" ARTIST="Artist"><LOCATION VOLUME="D:" VOLUMEID="VOL-D" DIR="/:Music/:Été/:" FILE="A.flac"/><INFO KEY="Raw text key" RANKING="153"/><TEMPO BPM="120"/><MUSICAL_KEY VALUE="0"/>
<CUE_V2 NAME="hot" TYPE="0" START="1250" LEN="0" HOTCUE="0"/><CUE_V2 NAME="memory" TYPE="0" START="1250" LEN="0" HOTCUE="-1"/>
<CUE_V2 NAME="loop" TYPE="5" START="2000" LEN="4000" HOTCUE="7"/><CUE_V2 NAME="grid" TYPE="4" START="500" LEN="0" HOTCUE="-1"><GRID BPM="128"/></CUE_V2>
<CUE_V2 NAME="duplicate" TYPE="0" START="1250" LEN="0" HOTCUE="0"/><UNKNOWN_XML Flag="retained"/></ENTRY>
<ENTRY TITLE="B"><LOCATION VOLUME="D:" DIR="/:Music/:" FILE="B.flac"/></ENTRY>"#;
    let playlists = r#"<NODE TYPE="0" NAME="$ROOT"><SUBNODES><NODE TYPE="1" NAME="Set"><PLAYLIST UUID="SET-1"><ENTRY><PRIMARYKEY TYPE="TRACK" KEY="D:/:Music/:Été/:A.flac"/></ENTRY><ENTRY><PRIMARYKEY TYPE="TRACK" KEY="D:/:Music/:B.flac"/></ENTRY><ENTRY><PRIMARYKEY TYPE="TRACK" KEY="D:/:Music/:Été/:A.flac"/></ENTRY></PLAYLIST></NODE><NODE TYPE="1" NAME="Smart"><PLAYLIST UUID="SMART"><SEARCH_EXPRESSION QUERY="native unknown predicate"/></PLAYLIST></NODE></SUBNODES></NODE>"#;
    fs::write(
        root.path().join("collection.nml"),
        document(entries, playlists),
    )
    .unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::Traktor, &["collection.nml"]);
    let (_, rows) = read(SourceProvider::Traktor, &snapshot);
    let tracks = rows.tracks();
    assert_eq!(tracks.len(), 2);
    let a = tracks
        .iter()
        .find(|t| t.metadata.get("title") == Some(&Presence::Value(json!("A"))))
        .unwrap();
    assert_eq!(a.locations.original.as_deref(), Some("D:/Music/Été/A.flac"));
    assert_eq!(a.key.as_ref().unwrap().camelot.as_deref(), Some("8B"));
    assert_eq!(
        a.key.as_ref().unwrap().extensions["info_key"],
        "Raw text key"
    );
    assert_eq!(a.bpm.as_ref().unwrap().value, Some(120.));
    let grid = a.grid.as_ref().unwrap();
    assert_eq!(grid.bpm, Some(128.));
    assert!(grid.sample_rate.is_none());
    assert_eq!(grid.native_first_beat_seconds, Some(0.5));
    assert!(grid.canonical_first_beat_seconds.is_none());
    assert_eq!(a.cues.len(), 4);
    assert_eq!(
        a.cues
            .iter()
            .filter(|c| c.family == CueFamily::Hot && c.source_slot == Some(0))
            .count(),
        2
    );
    assert!(
        a.cues
            .iter()
            .any(|c| c.family == CueFamily::Memory && c.native_start_seconds == Some(1.25))
    );
    assert!(a.cues.iter().any(|c| c.source_slot == Some(7)
        && c.native_start_seconds == Some(2.)
        && c.native_end_seconds == Some(6.)));
    assert!(
        a.cues
            .iter()
            .all(|c| c.clock_status == ClockStatus::UnresolvedNative
                && c.canonical_start_seconds.is_none())
    );
    assert!(
        a.extensions["native"]["children"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["name"] == "UNKNOWN_XML" && n["attributes"]["Flag"] == "retained")
    );
    let members = rows.members();
    assert_eq!(members.len(), 3);
    assert_eq!(
        members[0].1.track_external_id,
        members[2].1.track_external_id
    );
    assert_eq!(members[0].1.track_external_id, a.external_id);
    assert!(rows.collections().iter().any(|c| {
        c.kind == CollectionKind::Smart
            && c.extensions["native"]
                .to_string()
                .contains("native unknown predicate")
    }));
}
#[test]
fn traktor_all_twenty_four_keys_are_source_not_pcm_predictions() {
    let root = tempfile::tempdir().unwrap();
    let entries=(0..24).map(|key|format!(r#"<ENTRY TITLE="Key{key}"><LOCATION VOLUME="Disk One" DIR="/:Music/:" FILE="colon:{key}.flac"/><MUSICAL_KEY VALUE="{key}"/></ENTRY>"#)).collect::<String>();
    fs::write(root.path().join("collection.nml"), document(&entries, "")).unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::Traktor, &["collection.nml"]);
    let (_, rows) = read(SourceProvider::Traktor, &snapshot);
    let expected = [
        "8B", "3B", "10B", "5B", "12B", "7B", "2B", "9B", "4B", "11B", "6B", "1B", "5A", "12A",
        "7A", "2A", "9A", "4A", "11A", "6A", "1A", "8A", "3A", "10A",
    ];
    assert_eq!(rows.tracks().len(), 24);
    for (key, expected) in expected.into_iter().enumerate() {
        let tracks = rows.tracks();
        let track = tracks
            .iter()
            .find(|t| t.metadata.get("title") == Some(&Presence::Value(json!(format!("Key{key}")))))
            .unwrap();
        assert_eq!(
            track.key.as_ref().unwrap().camelot.as_deref(),
            Some(expected)
        );
        assert!(
            track
                .locations
                .original
                .as_ref()
                .unwrap()
                .contains(&format!("colon:{key}.flac"))
        );
    }
}
#[test]
fn traktor_invalid_loop_keeps_source_row_and_streaming_media_is_visible() {
    let root = tempfile::tempdir().unwrap();
    let entries = r#"<ENTRY TITLE="Remote"><LOCATION WEBADDRESS="https://example.invalid/audio"/><CUE_V2 TYPE="5" START="1000" LEN="-10" HOTCUE="-1"/></ENTRY>"#;
    fs::write(root.path().join("collection.nml"), document(entries, "")).unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::Traktor, &["collection.nml"]);
    let (_, rows) = read(SourceProvider::Traktor, &snapshot);
    let track = rows.tracks()[0];
    assert_eq!(track.locations.availability, "remote");
    assert!(track.cues[0].native_end_seconds.is_none());
    assert!(!track.issues.is_empty());
}
#[test]
fn traktor_rejects_other_programs_entities_duplicate_tree_ids_and_cancellation() {
    for xml in [
        "<NML><HEAD PROGRAM=\"OtherApp\"/><COLLECTION/></NML>".into(),
        "<!DOCTYPE NML [<!ENTITY x SYSTEM \"file:///etc/passwd\">]><NML>&x;</NML>".into(),
        document(
            "",
            r#"<NODE TYPE="0" NAME="A" UUID="X"><SUBNODES><NODE TYPE="0" NAME="B" UUID="X"/></SUBNODES></NODE>"#,
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("collection.nml"), xml).unwrap();
        let snapshot = snapshot(root.path(), SourceProvider::Traktor, &["collection.nml"]);
        assert!(
            adapter(SourceProvider::Traktor)
                .read(
                    &snapshot,
                    &ReadOptions::default(),
                    &mut Records::default(),
                    &CancellationToken::default()
                )
                .is_err()
        );
    }
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("collection.nml"), document("", "")).unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::Traktor, &["collection.nml"]);
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(
        adapter(SourceProvider::Traktor)
            .read(
                &snapshot,
                &ReadOptions::default(),
                &mut Records::default(),
                &cancel
            )
            .is_err()
    );
}

#[test]
fn traktor_history_is_opt_in_and_does_not_depend_on_the_selected_root_name() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("History");
    fs::create_dir_all(source.join("History")).unwrap();
    let entry =
        r#"<ENTRY TITLE="Song"><LOCATION VOLUME="D:" DIR="/:Music/:" FILE="A.flac"/></ENTRY>"#;
    fs::write(
        source.join("collection.nml"),
        document(entry, "").replace("4.1.0", "3.11.0"),
    )
    .unwrap();
    fs::write(source.join("History/session.nml"), document(entry, "")).unwrap();
    let snapshot = snapshot(
        &source,
        SourceProvider::Traktor,
        &["collection.nml", "History/session.nml"],
    );
    let (_, rows) = read(SourceProvider::Traktor, &snapshot);
    assert_eq!(rows.tracks().len(), 1);
    assert!(rows.members().is_empty());
    let mut rows = Records::default();
    let options = ReadOptions {
        include_history: true,
        ..Default::default()
    };
    adapter(SourceProvider::Traktor)
        .read(
            &snapshot,
            &options,
            &mut rows,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(rows.tracks().len(), 1);
    assert_eq!(rows.members().len(), 1);
    assert_eq!(rows.collections()[0].kind, CollectionKind::History);
}
