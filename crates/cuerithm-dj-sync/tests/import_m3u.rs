#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use std::fs;
use support::*;

#[test]
fn m3u_resolves_relative_entries_and_preserves_a_b_a() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sets")).unwrap();
    fs::write(root.path().join("sets/list.m3u8"), "\u{feff}#EXTM3U\r\n#EXTINF:12,Display only\r\n../Música/A.flac\r\n../B.flac\r\n../Música/A.flac\r\n").unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::M3u, &["sets/list.m3u8"]);
    let (header, rows) = read(SourceProvider::M3u, &snapshot);
    assert_eq!(header.completeness, Completeness::SelectedScope);
    let tracks = rows.tracks();
    assert_eq!(tracks.len(), 2);
    assert_eq!(
        tracks[0].locations.native_path.as_deref(),
        Some(
            root.path()
                .canonicalize()
                .unwrap()
                .join("Música/A.flac")
                .as_path()
        )
    );
    assert_eq!(tracks[0].locations.availability, "missing");
    assert!(tracks[0].metadata.is_empty());
    assert!(tracks[0].cues.is_empty() && tracks[0].grid.is_none());
    let members = rows.members();
    assert_eq!(members.len(), 3);
    assert_eq!(
        members[0].1.track_external_id,
        members[2].1.track_external_id
    );
    assert_ne!(
        members[0].1.external_occurrence_id,
        members[2].1.external_occurrence_id
    );
    assert_eq!(members[0].1.extensions["extinf"], "12,Display only");
    let profile = adapter(SourceProvider::M3u).inspect(&snapshot).unwrap();
    assert!(profile.fields["collections"].read && !profile.fields["collections"].write);
}

#[test]
fn m3u_utf16le_paths_uri_and_empty_playlist_are_explicit() {
    let root = tempfile::tempdir().unwrap();
    let text = "#EXTM3U\rC:\\Music\\A.flac\r\\\\server\\share\\B.flac\rfile:///C:/M%C3%BAsica/C.flac\rhttps://example.invalid/remote\r";
    let bytes: Vec<u8> = [0xff, 0xfe]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    fs::write(root.path().join("windows.m3u"), bytes).unwrap();
    fs::write(root.path().join("empty.m3u8"), "#EXTM3U\n").unwrap();
    let snapshot = snapshot(
        root.path(),
        SourceProvider::M3u,
        &["windows.m3u", "empty.m3u8"],
    );
    let (_, rows) = read(SourceProvider::M3u, &snapshot);
    assert_eq!(rows.collections().len(), 2);
    assert_eq!(rows.tracks().len(), 4);
    let tracks = rows.tracks();
    assert_eq!(tracks[0].locations.platform.as_deref(), Some("windows"));
    assert!(
        tracks
            .iter()
            .any(|t| t.locations.original.as_deref() == Some("file:///C:/M%C3%BAsica/C.flac"))
    );
    if !cfg!(windows) {
        assert!(tracks.iter().all(|t| t.locations.native_path.is_none()));
    }
    assert!(tracks.iter().any(|t| t.locations.availability == "remote"));
}

#[test]
fn m3u_rejects_malformed_uri_unknown_encoding_limits_and_cancellation() {
    for bytes in [
        b"file:///bad%Q0.flac\n".to_vec(),
        vec![0xff, 0x80],
        vec![0xff, 0xfe, 1],
    ] {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("bad.m3u8"), bytes).unwrap();
        let snapshot = snapshot(root.path(), SourceProvider::M3u, &["bad.m3u8"]);
        assert!(
            adapter(SourceProvider::M3u)
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
    fs::write(root.path().join("list.m3u8"), "a.flac\n").unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::M3u, &["list.m3u8"]);
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(
        adapter(SourceProvider::M3u)
            .read(
                &snapshot,
                &ReadOptions::default(),
                &mut Records::default(),
                &cancel
            )
            .is_err()
    );
    let mut options = ReadOptions::default();
    options.limits.text_bytes = 2;
    assert!(
        adapter(SourceProvider::M3u)
            .read(
                &snapshot,
                &options,
                &mut Records::default(),
                &CancellationToken::default()
            )
            .is_err()
    );
}

#[test]
fn m3u_identity_normalizes_equivalent_windows_paths() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("list.m3u8"),
        "C:\\Music\\A.flac\nfile:///C:/Music/A.flac\nC:/MUSIC/A.flac\n",
    )
    .unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::M3u, &["list.m3u8"]);
    let (_, rows) = read(SourceProvider::M3u, &snapshot);
    assert_eq!(rows.tracks().len(), 1);
    assert_eq!(rows.members().len(), 3);
}

#[test]
fn m3u_file_tag_capture_is_explicit_read_only_and_separate_from_native_metadata() {
    let root = tempfile::tempdir().unwrap();
    let mut body = b"TIT2".to_vec();
    let title = b"\0File title";
    body.extend((title.len() as u32).to_be_bytes());
    body.extend([0, 0]);
    body.extend(title);
    let size = body.len() as u32;
    let mut media = b"ID3\x03\0\0".to_vec();
    media.extend([
        ((size >> 21) & 127) as u8,
        ((size >> 14) & 127) as u8,
        ((size >> 7) & 127) as u8,
        (size & 127) as u8,
    ]);
    media.extend(body);
    media.extend(include_bytes!("../src/mixxx/fixtures/no_tag.mp3"));
    fs::write(root.path().join("song.mp3"), &media).unwrap();
    fs::write(root.path().join("list.m3u8"), "song.mp3\n").unwrap();
    let snapshot = snapshot(root.path(), SourceProvider::M3u, &["list.m3u8"]);
    let (_, no_tags) = read(SourceProvider::M3u, &snapshot);
    assert!(no_tags.tracks()[0].file_tags.is_none() && no_tags.blobs.is_empty());
    let options = ReadOptions {
        capture_file_tags: true,
        ..Default::default()
    };
    let mut rows = Records::default();
    adapter(SourceProvider::M3u)
        .read(
            &snapshot,
            &options,
            &mut rows,
            &CancellationToken::default(),
        )
        .unwrap();
    let track = rows.tracks()[0];
    let reference = track
        .file_tags
        .as_ref()
        .expect("selected file tags captured");
    let captured: FileTagObservation =
        serde_json::from_slice(&rows.blobs[&reference.sha256]).unwrap();
    assert_eq!(captured.normalized["title"], vec!["File title"]);
    for record in captured.records {
        assert!(rows.blobs.contains_key(&record.payload.sha256));
    }
    assert!(track.metadata.is_empty() && track.key.is_none() && track.cues.is_empty());
    assert_eq!(fs::read(root.path().join("song.mp3")).unwrap(), media);
}
