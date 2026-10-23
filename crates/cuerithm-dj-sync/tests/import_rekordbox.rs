#[path = "import_fixture_support/rekordbox.rs"]
mod fixture;
#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::readers::rekordbox::RekordboxReader;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::{DatabaseKey, ReadOnlyDatabase, validate_write_profile};
use support::*;
fn reader() -> RekordboxReader {
    RekordboxReader::with_provisioned_key(
        DatabaseKey::new(b"generated collection key").unwrap(),
        Default::default(),
    )
}
fn capture(root: &std::path::Path, reader: &RekordboxReader, analysis: bool) -> ImportSnapshot {
    let mut files = vec![ArtifactRequest::sqlite("master.db")];
    if analysis {
        files.push(ArtifactRequest::stable("PIONEER/USBANLZ/a/ANLZ0000.DAT"));
    }
    let request =
        SourceCaptureRequest::from_selected_root(SourceProvider::Rekordbox, root, files).unwrap();
    reader
        .capture(
            &request,
            root,
            &ImportLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap()
}
fn read(snapshot: &ImportSnapshot, reader: &RekordboxReader) -> (ObservationHeader, Records) {
    let mut rows = Records::default();
    let header = reader
        .read(
            snapshot,
            &ReadOptions::default(),
            &mut rows,
            &CancellationToken::default(),
        )
        .unwrap();
    validate_observation(
        &header,
        &mut rows.rows.clone().into_iter(),
        &ImportLimits::default(),
    )
    .unwrap();
    snapshot
        .validate_source(&CancellationToken::default())
        .unwrap();
    (header, rows)
}
fn grid() -> Vec<u8> {
    let mut data = vec![0u8; 28 + 24 + 24];
    data[0..4].copy_from_slice(b"PMAI");
    data[4..8].copy_from_slice(&28u32.to_be_bytes());
    let len = data.len() as u32;
    data[8..12].copy_from_slice(&len.to_be_bytes());
    data[28..32].copy_from_slice(b"PQTZ");
    data[32..36].copy_from_slice(&24u32.to_be_bytes());
    data[36..40].copy_from_slice(&48u32.to_be_bytes());
    data[48..52].copy_from_slice(&3u32.to_be_bytes());
    for (i, (beat, tempo, ms)) in [(4u16, 12000u16, 250u32), (1, 12100, 760), (2, 12200, 1260)]
        .into_iter()
        .enumerate()
    {
        let at = 52 + 8 * i;
        data[at..at + 2].copy_from_slice(&beat.to_be_bytes());
        data[at + 2..at + 4].copy_from_slice(&tempo.to_be_bytes());
        data[at + 4..at + 8].copy_from_slice(&ms.to_be_bytes());
    }
    data
}
#[test]
fn rekordbox_preserves_memory_hot_same_time_and_sixteen_slots() {
    let root = tempfile::tempdir().unwrap();
    let db = fixture::fixture(&root.path().join("master.db"));
    db.execute_batch("DELETE FROM djmdCue;INSERT INTO djmdCue(ID,ContentID,Kind,InMsec,OutMsec,ActiveLoop,rb_local_deleted) VALUES('memory','1',0,1250,-1,0,0),('memory-loop','1',4,1250,3000,0,0),('unknown','1',99,-5,-1,0,0);").unwrap();
    for kind in (1..=3).chain(5..=17) {
        db.execute("INSERT INTO djmdCue(ID,ContentID,Kind,InMsec,OutMsec,ActiveLoop,rb_local_deleted) VALUES(?1,'1',?2,1250,-1,0,0)",rusqlite::params![format!("hot-{kind}"),kind]).unwrap();
    }
    drop(db);
    let path = root.path().join("PIONEER/USBANLZ/a/ANLZ0000.DAT");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, grid()).unwrap();
    let reader = reader();
    let (header, rows) = read(&capture(root.path(), &reader, true), &reader);
    assert_eq!(header.source.library_instance_id, "db-alpha");
    let tracks = rows.tracks();
    let one = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert_eq!(
        one.extensions["native_identity"]["master_song_id"],
        "master-1"
    );
    let mut slots = one
        .cues
        .iter()
        .filter(|c| c.family == CueFamily::Hot)
        .map(|c| c.source_slot.unwrap())
        .collect::<Vec<_>>();
    slots.sort();
    assert_eq!(slots, (0..16).collect::<Vec<_>>());
    assert_eq!(
        one.cues
            .iter()
            .filter(|c| c.family == CueFamily::Memory)
            .count(),
        2
    );
    let memory = one.cues.iter().find(|c| c.external_id == "memory").unwrap();
    assert_eq!(memory.native_start_seconds, Some(1.25));
    assert!(memory.native_end_seconds.is_none());
    let loop_cue = one
        .cues
        .iter()
        .find(|c| c.external_id == "memory-loop")
        .unwrap();
    assert_eq!(loop_cue.native_end_seconds, Some(3.));
    assert_eq!(loop_cue.flags["active_loop"], true);
    let unknown = one
        .cues
        .iter()
        .find(|c| c.external_id == "unknown")
        .unwrap();
    assert_eq!(unknown.family, CueFamily::Unknown);
    assert_eq!(unknown.native_start_seconds, Some(-0.005));
    let grid = one.grid.as_ref().unwrap();
    assert!(
        grid.raw.is_some(),
        "retain original ANLZ including unknown sections"
    );
    assert_eq!(
        grid.points
            .iter()
            .map(|p| p.native_seconds)
            .collect::<Vec<_>>(),
        [0.25, 0.76, 1.26]
    );
    assert!(grid.points.iter().all(|p| p.canonical_seconds.is_none()));
    assert_eq!(grid.points[0].original["number"], 4);
    let members = rows
        .members()
        .into_iter()
        .filter(|(c, _)| *c == "list")
        .collect::<Vec<_>>();
    assert_eq!(
        members
            .iter()
            .map(|(_, m)| m.position.unwrap())
            .collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
}
#[test]
fn rekordbox_profile_acceptance_is_not_widened() {
    let root = tempfile::tempdir().unwrap();
    drop(fixture::fixture(&root.path().join("master.db")));
    let reader = reader();
    let snapshot = capture(root.path(), &reader, false);
    let key = DatabaseKey::new(b"generated collection key").unwrap();
    let db = ReadOnlyDatabase::open(snapshot.artifact("master.db").unwrap(), &key).unwrap();
    assert!(
        !validate_write_profile(&db.schema().unwrap())
            .unwrap()
            .can_write()
    );
    assert!(
        reader
            .inspect(&snapshot)
            .unwrap()
            .fields
            .values()
            .all(|f| !f.write)
    );
    let mut rows = Records::default();
    assert!(
        adapter(SourceProvider::Rekordbox)
            .read(
                &snapshot,
                &ReadOptions::default(),
                &mut rows,
                &CancellationToken::default()
            )
            .is_err()
    );
    assert!(rows.rows.is_empty());
    drop(db);
    let future = tempfile::tempdir().unwrap();
    let original = fixture::fixture(&future.path().join("master.db"));
    original.execute_batch("PRAGMA user_version=999").unwrap();
    drop(original);
    assert!(
        reader
            .inspect(&capture(future.path(), &reader, false))
            .is_err()
    );
}
#[test]
fn missing_analysis_and_media_keep_metadata_my_tags_and_smart_rules() {
    let root = tempfile::tempdir().unwrap();
    drop(fixture::fixture(&root.path().join("master.db")));
    let reader = reader();
    let snapshot = capture(root.path(), &reader, false);
    let (_, rows) = read(&snapshot, &reader);
    let tracks = rows.tracks();
    let one = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert_eq!(
        one.metadata["title"],
        Presence::Value(serde_json::json!("Signal"))
    );
    assert!(one.grid.is_none());
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "missing_rekordbox_analysis")
    );
    assert_eq!(one.extensions["my_tags"][0]["name"], "House");
    assert!(one.cached_tags.contains_key("rekordbox_content_cue"));
    assert!(one.extensions["source_row"]["ReservedUser"].is_object());
    assert!(rows.collections().iter().any(|c| {
        c.kind == CollectionKind::Smart
            && c.extensions["smart_rule"]
                .as_str()
                .unwrap()
                .contains("authored")
    }));
    let mut again = Records::default();
    reader
        .read(
            &snapshot,
            &ReadOptions::default(),
            &mut again,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(rows.rows, again.rows);
}

#[test]
fn direct_snapshot_includes_committed_wal_and_does_not_change_source_bytes() {
    let root = tempfile::tempdir().unwrap();
    let db = fixture::fixture(&root.path().join("master.db"));
    db.pragma_update(None, "journal_mode", "WAL").unwrap();
    db.execute_batch("UPDATE djmdContent SET Title='Committed WAL' WHERE ID='1'")
        .unwrap();
    let before = cuerithm_media::file_sha256(&root.path().join("master.db")).unwrap();
    let wal = cuerithm_media::file_sha256(&root.path().join("master.db-wal")).unwrap();
    let reader = reader();
    let snapshot = capture(root.path(), &reader, false);
    let (_, rows) = read(&snapshot, &reader);
    assert_eq!(
        rows.tracks()
            .iter()
            .find(|t| t.external_id == "1")
            .unwrap()
            .metadata["title"],
        Presence::Value(serde_json::json!("Committed WAL"))
    );
    assert_eq!(
        cuerithm_media::file_sha256(&root.path().join("master.db")).unwrap(),
        before
    );
    assert_eq!(
        cuerithm_media::file_sha256(&root.path().join("master.db-wal")).unwrap(),
        wal
    );
    drop(db);
}
#[test]
fn direct_cloud_original_paths_and_left_joins_do_not_drop_metadata() {
    let root = tempfile::tempdir().unwrap();
    let db = fixture::fixture(&root.path().join("master.db"));
    db.execute_batch("UPDATE djmdContent SET ServiceID=3,OrgFolderPath='/Original/Relocated.flac',ArtistID='missing' WHERE ID='1'").unwrap();
    drop(db);
    let reader = reader();
    let (_, rows) = read(&capture(root.path(), &reader, false), &reader);
    let tracks = rows.tracks();
    let one = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert_eq!(
        one.locations.original.as_deref(),
        Some("/Original/Relocated.flac")
    );
    assert_eq!(
        one.extensions["source_row"]["FolderPath"],
        "/authored/Signal.flac"
    );
    assert_eq!(one.metadata["artist"], Presence::Absent);
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "missing_rekordbox_foreign_record")
    );
    assert_eq!(tracks.len(), 4);
}
#[test]
fn corrupt_analysis_retains_track_and_captured_asset() {
    let root = tempfile::tempdir().unwrap();
    drop(fixture::fixture(&root.path().join("master.db")));
    let path = root.path().join("PIONEER/USBANLZ/a/ANLZ0000.DAT");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut data = grid();
    data[36..40].copy_from_slice(&u32::MAX.to_be_bytes());
    std::fs::write(&path, &data).unwrap();
    let reader = reader();
    let snapshot = capture(root.path(), &reader, true);
    let (_, rows) = read(&snapshot, &reader);
    let tracks = rows.tracks();
    let one = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert!(one.grid.is_none());
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "unsupported_rekordbox_analysis")
    );
    assert_eq!(std::fs::read(&path).unwrap(), data);
}
#[test]
fn unlinked_native_child_is_retained_and_accounted() {
    let root = tempfile::tempdir().unwrap();
    let db = fixture::fixture(&root.path().join("master.db"));
    db.execute_batch("INSERT INTO djmdCue(ID,ContentID,Kind,InMsec,rb_local_deleted) VALUES('unlinked',NULL,1,1000,0)").unwrap();
    drop(db);
    let reader = reader();
    let (header, rows) = read(&capture(root.path(), &reader, false), &reader);
    assert_eq!(header.completeness, Completeness::PartialWithErrors);
    assert_eq!(header.accounting.unrepresented_records, 1);
    assert!(rows.rows.iter().any(|r|matches!(r,ObservationRecord::Issue(i) if i.record_id.as_deref()==Some("djmdCue:unlinked"))));
}
#[test]
fn native_record_cursor_bounds_rows_and_cancellation_without_exposing_accounts() {
    let root = tempfile::tempdir().unwrap();
    drop(fixture::fixture(&root.path().join("master.db")));
    let key = DatabaseKey::new(b"generated collection key").unwrap();
    let db = ReadOnlyDatabase::open(&root.path().join("master.db"), &key).unwrap();
    let mut count = 0;
    assert!(
        db.visit_collection_records(
            cuerithm_rekordbox::CollectionRecordLimits {
                max_rows: 3,
                ..Default::default()
            },
            &CancellationToken::default(),
            |_| {
                count += 1;
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(count, 3);
    let cancel = CancellationToken::default();
    let mut visited = Vec::new();
    assert!(
        db.visit_collection_records(Default::default(), &cancel, |row| {
            visited.push(row.id);
            cancel.cancel();
            Ok(())
        })
        .is_err()
    );
    assert_eq!(visited.len(), 1);
    let mut registry = Vec::new();
    db.visit_collection_records(Default::default(), &CancellationToken::default(), |row| {
        if row.table == "agentRegistry" {
            registry.push(row.id)
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(registry, ["localUpdateCount"]);
}
#[test]
fn numeric_sql_inputs_keep_native_text_id_affinity_and_relationships() {
    let root = tempfile::tempdir().unwrap();
    let db = fixture::fixture(&root.path().join("master.db"));
    db.execute_batch("UPDATE djmdCue SET ContentID=1;UPDATE djmdSongPlaylist SET ContentID=1 WHERE ContentID='1';UPDATE djmdSongMyTag SET ContentID=1;").unwrap();
    drop(db);
    let reader = reader();
    let (_, rows) = read(&capture(root.path(), &reader, false), &reader);
    let tracks = rows.tracks();
    let one = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert_eq!(one.cues.len(), 2);
    assert_eq!(one.extensions["my_tags"][0]["name"], "House");
}

#[test]
fn mislabeled_analysis_artifact_does_not_claim_a_different_native_reference() {
    let root = tempfile::tempdir().unwrap();
    drop(fixture::fixture(&root.path().join("master.db")));
    std::fs::write(root.path().join("unrelated.DAT"), grid()).unwrap();
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::Rekordbox,
        root.path(),
        vec![
            ArtifactRequest::sqlite("master.db"),
            ArtifactRequest {
                id: "PIONEER/USBANLZ/a/ANLZ0000.DAT".into(),
                relative_path: "unrelated.DAT".into(),
                kind: ArtifactKind::StableFile,
            },
        ],
    )
    .unwrap();
    let reader = reader();
    let snapshot = reader
        .capture(
            &request,
            root.path(),
            &ImportLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap();
    let (_, rows) = read(&snapshot, &reader);
    let tracks = rows.tracks();
    let one = tracks.iter().find(|t| t.external_id == "1").unwrap();
    assert!(one.grid.is_none());
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "mismatched_rekordbox_analysis_reference")
    );
}
