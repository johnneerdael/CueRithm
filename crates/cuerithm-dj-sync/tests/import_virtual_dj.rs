#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use serde_json::json;
use std::fs;
use support::*;
#[test]
fn virtualdj_distinguishes_scan_and_poi_bpm() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("database.xml"),r#"<VirtualDJ_Database Version="8.5"><Song FilePath="D:\Music\A.flac" NativeExtra="keep"><Tags Title="A" Author="Artist" Key="8A" Bpm="119"/><Scan Bpm="0.5" Key="9A"/><Infos SongLength="30.5"/>
<Poi Type="cue" Num="1" Pos="1.25" Name="Hot" Action="play" Point="opaque"/><Poi Type="cue" Pos="1.25" Name="Memory"/>
<Poi Type="loop" Num="3" Pos="2" Size="8" AutoTrigger="yes"/><Poi Type="beatgrid" Pos="0.25" Bpm="128" Num="1"/>
<Poi Type="cue" Num="1" Pos="1.25" Name="Duplicate"/><Poi Type="action" Num="7" Pos="3" Action="native action"/></Song></VirtualDJ_Database>"#).unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::VirtualDj, &["database.xml"]);
    let (_, rows) = read(SourceProvider::VirtualDj, &snapshot);
    let track = rows.tracks()[0];
    assert_eq!(track.bpm.as_ref().unwrap().value, Some(120.));
    assert_eq!(track.grid.as_ref().unwrap().points[0].bpm, Some(128.));
    assert_eq!(track.key.as_ref().unwrap().raw.as_deref(), Some("8A"));
    assert_eq!(track.extensions["duration_seconds"], 30.5);
    assert_eq!(
        track
            .cues
            .iter()
            .filter(|c| c.family == CueFamily::Hot && c.source_slot == Some(0))
            .count(),
        2
    );
    assert!(
        track
            .cues
            .iter()
            .any(|c| c.family == CueFamily::Memory && c.native_start_seconds == Some(1.25))
    );
    assert!(track.cues.iter().any(|c| c.source_slot == Some(2)
        && c.native_end_seconds == Some(6.)
        && c.flags["active_loop"] == true));
    assert!(
        track
            .cues
            .iter()
            .any(|c| c.original["attributes"]["Action"] == "play"
                && c.original["attributes"]["Point"] == "opaque")
    );
    assert!(
        track
            .cues
            .iter()
            .filter(|c| c.original["attributes"]["Type"] == "action")
            .all(|c| c.family != CueFamily::Hot)
    );
    assert!(
        track
            .cues
            .iter()
            .all(|c| c.canonical_start_seconds.is_none())
    );
    assert_eq!(
        track.extensions["native"]["attributes"]["NativeExtra"],
        "keep"
    );
}
#[test]
fn virtualdj_my_lists_legacy_lists_empty_folders_and_two_volumes_survive() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("External")).unwrap();
    fs::create_dir_all(
        root.path()
            .join("MyLists/Folder.subfolders/Empty.subfolders"),
    )
    .unwrap();
    fs::create_dir_all(root.path().join("Playlists")).unwrap();
    fs::write(root.path().join("database.xml"),r#"<VirtualDJ_Database Version="8.5"><Song FilePath="D:/Music/same.flac"><Tags Title="D"/></Song></VirtualDJ_Database>"#).unwrap();
    fs::write(root.path().join("External/database.xml"),r#"<VirtualDJ_Database Version="8.5"><Song FilePath="E:/Music/same.flac"><Tags Title="E"/></Song></VirtualDJ_Database>"#).unwrap();
    fs::write(
        root.path().join("MyLists/Folder.subfolders/order"),
        "Set.vdjfolder\nEmpty.subfolders\n",
    )
    .unwrap();
    fs::write(
        root.path()
            .join("MyLists/Folder.subfolders/Empty.subfolders/order"),
        "",
    )
    .unwrap();
    fs::write(root.path().join("MyLists/Folder.subfolders/Set.vdjfolder"),r#"<FilterFolder filter="unqualified predicate"><song path="D:/Music/same.flac"/><song path="E:/Music/same.flac"/><song path="D:/Music/same.flac"/></FilterFolder>"#).unwrap();
    fs::write(
        root.path().join("Playlists/Legacy.m3u"),
        "D:/Music/same.flac\nE:/Music/same.flac\nD:/Music/same.flac\n",
    )
    .unwrap();
    let snapshot = snapshot(
        root.path(),
        SourceProvider::VirtualDj,
        &[
            "database.xml",
            "External/database.xml",
            "MyLists/Folder.subfolders/order",
            "MyLists/Folder.subfolders/Empty.subfolders/order",
            "MyLists/Folder.subfolders/Set.vdjfolder",
            "Playlists/Legacy.m3u",
        ],
    );
    let (_, rows) = read(SourceProvider::VirtualDj, &snapshot);
    assert_eq!(rows.tracks().len(), 2);
    assert_eq!(rows.members().len(), 6);
    let collections = rows.collections();
    let folder = collections.iter().find(|c| c.name == "Folder").unwrap();
    let list = collections.iter().find(|c| c.name == "Set").unwrap();
    assert_eq!(list.parent_id.as_deref(), Some(folder.external_id.as_str()));
    assert_eq!(list.kind, CollectionKind::Smart);
    assert!(
        list.extensions["issues"]
            .to_string()
            .contains("unsupported_filter")
    );
    assert!(
        collections
            .iter()
            .any(|c| c.name == "Empty" && c.kind == CollectionKind::Folder)
    );
    assert!(
        rows.tracks()
            .iter()
            .any(|t| t.metadata["title"] == Presence::Value(json!("D")))
    );
    assert!(
        rows.tracks()
            .iter()
            .any(|t| t.metadata["title"] == Presence::Value(json!("E")))
    );
}
#[test]
fn virtualdj_missing_reference_and_invalid_loop_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("database.xml"),r#"<VirtualDJ_Database Version="8.5"><Song FilePath="https://example.invalid/audio"><Scan Bpm="0.5"/><Poi Type="loop" Pos="1" Size="-2"/></Song></VirtualDJ_Database>"#).unwrap();
    fs::write(
        root.path().join("List.vdjfolder"),
        r#"<VirtualFolder><song path="D:/Missing.flac"/></VirtualFolder>"#,
    )
    .unwrap();
    let snapshot = snapshot(
        root.path(),
        SourceProvider::VirtualDj,
        &["database.xml", "List.vdjfolder"],
    );
    let (_, rows) = read(SourceProvider::VirtualDj, &snapshot);
    assert_eq!(rows.tracks().len(), 2);
    let tracks = rows.tracks();
    let remote = tracks
        .iter()
        .find(|t| t.locations.availability == "remote")
        .unwrap();
    assert!(remote.cues[0].native_end_seconds.is_none());
    assert!(!remote.issues.is_empty());
    assert!(
        tracks
            .iter()
            .any(|t| t.issues.iter().any(|i| i.code == "missing_track_record"))
    );
}
#[test]
fn virtualdj_rejects_entities_profile_drift_and_order_traversal() {
    for xml in [
        "<Other_Database/>",
        "<!DOCTYPE VirtualDJ_Database [<!ENTITY x SYSTEM \"file:///etc/passwd\">]><VirtualDJ_Database>&x;</VirtualDJ_Database>",
        "<VirtualDJ_Database Version=\"99\"/>",
    ] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("database.xml"), xml).unwrap();
        let snapshot = snapshot(root.path(), SourceProvider::VirtualDj, &["database.xml"]);
        assert!(
            adapter(SourceProvider::VirtualDj)
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
    fs::create_dir(root.path().join("Folder.subfolders")).unwrap();
    fs::write(
        root.path().join("Folder.subfolders/order"),
        "../escape.vdjfolder\n",
    )
    .unwrap();
    let snapshot = snapshot(
        root.path(),
        SourceProvider::VirtualDj,
        &["Folder.subfolders/order"],
    );
    assert!(
        adapter(SourceProvider::VirtualDj)
            .read(
                &snapshot,
                &ReadOptions::default(),
                &mut Records::default(),
                &CancellationToken::default()
            )
            .is_err()
    );
}

#[test]
fn virtualdj_history_is_separate_from_an_ordinary_folder_named_history() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join("Playlists/History")).unwrap();
    fs::create_dir(root.path().join("History")).unwrap();
    fs::write(root.path().join("database.xml"),r#"<VirtualDJ_Database Version="8.4"><Song FilePath="D:/A.flac"><Tags Key="不可判定"/></Song></VirtualDJ_Database>"#).unwrap();
    fs::write(
        root.path().join("Playlists/History/User.m3u8"),
        "D:/A.flac\n",
    )
    .unwrap();
    fs::write(root.path().join("History/Session.m3u8"), "D:/A.flac\n").unwrap();
    let snapshot = snapshot(
        root.path(),
        SourceProvider::VirtualDj,
        &[
            "database.xml",
            "Playlists/History/User.m3u8",
            "History/Session.m3u8",
        ],
    );
    let (_, rows) = read(SourceProvider::VirtualDj, &snapshot);
    assert_eq!(rows.members().len(), 1);
    assert!(
        !rows
            .collections()
            .iter()
            .any(|c| c.kind == CollectionKind::History)
    );
    assert_eq!(
        rows.tracks()[0].key.as_ref().unwrap().raw.as_deref(),
        Some("不可判定")
    );
    assert!(rows.tracks()[0].key.as_ref().unwrap().camelot.is_none());
    let mut rows = Records::default();
    let options = ReadOptions {
        include_history: true,
        ..Default::default()
    };
    adapter(SourceProvider::VirtualDj)
        .read(
            &snapshot,
            &options,
            &mut rows,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(rows.members().len(), 2);
    assert_eq!(
        rows.collections()
            .iter()
            .filter(|c| c.kind == CollectionKind::History)
            .count(),
        1
    );
}
