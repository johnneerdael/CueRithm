#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use serde_json::json;
use std::fs;
use support::*;

#[test]
fn apple_plist_retains_milliseconds_and_missing_remote_records() {
    let root = tempfile::tempdir().unwrap();
    let xml = r#"<?xml version="1.0"?><!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd"><plist version="1.0"><dict>
<key>Major Version</key><integer>1</integer><key>Minor Version</key><integer>1</integer>
<key>Library Persistent ID</key><string>INVENTED-LIB</string><key>Tracks</key><dict>
<key>1</key><dict><key>Track ID</key><integer>1</integer><key>Name</key><string>Été &amp; Snow</string><key>Location</key><string>file:///missing/A.flac</string><key>Total Time</key><integer>1250</integer><key>Rating</key><integer>80</integer><key>BPM</key><integer>120</integer><key>Unrecognized Safe</key><string>retain me</string><key>Account Password</key><string>INVENTED-DO-NOT-RETAIN</string></dict>
<key>2</key><dict><key>Track ID</key><integer>2</integer><key>Name</key><string></string><key>Track Type</key><string>Remote</string><key>Location</key><string>https://example.invalid/remote</string></dict></dict>
<key>Playlists</key><array><dict><key>Playlist Persistent ID</key><string>PARENT</string><key>Name</key><string>Folder</string><key>Folder</key><true/></dict>
<dict><key>Playlist Persistent ID</key><string>LIST</string><key>Parent Persistent ID</key><string>PARENT</string><key>Name</key><string>Smart</string><key>Smart Info</key><data>AQID</data><key>Smart Criteria</key><data>BAUG</data><key>Playlist Items</key><array><dict><key>Track ID</key><integer>1</integer><key>NativeItemFlag</key><integer>7</integer></dict><dict><key>Track ID</key><integer>2</integer></dict><dict><key>Track ID</key><integer>1</integer></dict><dict><key>Track ID</key><integer>99</integer></dict></array></dict></array></dict></plist>"#;
    fs::write(root.path().join("Music.xml"), xml).unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::AppleMusicXml, &["Music.xml"]);
    let (header, rows) = read(SourceProvider::AppleMusicXml, &snapshot);
    assert_eq!(header.source.library_instance_id, "INVENTED-LIB");
    let tracks = rows.tracks();
    assert_eq!(tracks.len(), 3);
    let a = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert_eq!(a.extensions["duration_seconds"], 1.25);
    assert_eq!(a.metadata["title"], Presence::Value(json!("Été & Snow")));
    assert!(a.extensions["native"].get("Account Password").is_none());
    assert_eq!(
        a.metadata["rating"],
        Presence::Value(json!({"raw":80,"scale_max":100}))
    );
    assert_eq!(a.bpm.as_ref().unwrap().value, Some(120.0));
    assert!(a.cues.is_empty() && a.grid.is_none());
    assert_eq!(a.extensions["native"]["Unrecognized Safe"], "retain me");
    let remote = tracks.iter().find(|t| t.external_id == "2").unwrap();
    assert_eq!(remote.locations.availability, "remote");
    assert_eq!(remote.metadata["title"], Presence::Empty(json!("")));
    assert!(!remote.metadata.contains_key("artist"));
    let missing = tracks.iter().find(|t| t.external_id == "99").unwrap();
    assert_eq!(missing.locations.availability, "unresolved");
    assert!(
        missing
            .issues
            .iter()
            .any(|i| i.code == "missing_track_record")
    );
    let collections = rows.collections();
    let list = collections
        .iter()
        .find(|c| c.external_id == "LIST")
        .unwrap();
    assert_eq!(list.kind, CollectionKind::Smart);
    assert_eq!(list.parent_id.as_deref(), Some("PARENT"));
    assert_eq!(list.extensions["native"]["Smart Criteria"], "BAUG");
    assert_eq!(rows.members().len(), 4);
    assert_eq!(
        rows.members()[0].1.extensions["native"]["NativeItemFlag"],
        7
    );
    assert!(list.extensions["native"].get("Playlist Items").is_none());
    assert_eq!(
        rows.members()[0].1.track_external_id,
        rows.members()[2].1.track_external_id
    );
}

#[test]
fn apple_native_identity_takes_precedence_over_registered_fallback() {
    for (native, expected) in [
        (
            "<key>Library Persistent ID</key><string>APPLE-NATIVE</string>",
            "APPLE-NATIVE",
        ),
        ("", "registered-fallback"),
    ] {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("Music.xml"),
            format!("<plist><dict>{native}<key>Tracks</key><dict/></dict></plist>"),
        )
        .unwrap();
        let snapshot = snapshot(root.path(), SourceProvider::AppleMusicXml, &["Music.xml"]);
        let header = adapter(SourceProvider::AppleMusicXml)
            .read(
                &snapshot,
                &ReadOptions {
                    library_instance_id: Some("registered-fallback".into()),
                    ..Default::default()
                },
                &mut Records::default(),
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(header.source.library_instance_id, expected);
    }
}

#[test]
fn apple_plist_rejects_entities_malformed_profile_and_cycles() {
    for xml in [
        r#"<!DOCTYPE plist [<!ENTITY secret SYSTEM "file:///etc/passwd">]><plist><dict><key>Tracks</key><dict>&secret;</dict></dict></plist>"#,
        "<plist><dict><key>Tracks</key><dict>",
        "<REKORDBOX><dict/></REKORDBOX>",
        "<plist><dict><key>Tracks</key><dict/><key>Library Persistent ID</key><integer>3</integer></dict></plist>",
        "<plist><dict><key>Tracks</key><dict/><key>Library Persistent ID</key><string></string></dict></plist>",
        r#"<plist><dict><key>Tracks</key><dict/><key>Playlists</key><array><dict><key>Playlist Persistent ID</key><string>A</string><key>Parent Persistent ID</key><string>A</string><key>Name</key><string>Cycle</string></dict></array></dict></plist>"#,
    ] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("bad.xml"), xml).unwrap();
        let snapshot = snapshot(root.path(), SourceProvider::AppleMusicXml, &["bad.xml"]);
        assert!(
            adapter(SourceProvider::AppleMusicXml)
                .read(
                    &snapshot,
                    &ReadOptions::default(),
                    &mut Records::default(),
                    &CancellationToken::default()
                )
                .is_err()
        );
    }
}

#[test]
fn apple_empty_export_and_cancelled_or_bounded_reads() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("Music.xml"),
        "<plist><dict><key>Tracks</key><dict/><key>Playlists</key><array/></dict></plist>",
    )
    .unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::AppleMusicXml, &["Music.xml"]);
    let (_, rows) = read(SourceProvider::AppleMusicXml, &snapshot);
    assert!(rows.rows.is_empty());
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(
        adapter(SourceProvider::AppleMusicXml)
            .read(
                &snapshot,
                &ReadOptions::default(),
                &mut Records::default(),
                &cancel
            )
            .is_err()
    );
    let mut options = ReadOptions::default();
    options.limits.text_bytes = 8;
    assert!(
        adapter(SourceProvider::AppleMusicXml)
            .read(
                &snapshot,
                &options,
                &mut Records::default(),
                &CancellationToken::default()
            )
            .is_err()
    );
}
