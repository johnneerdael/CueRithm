use cuerithm_media::{CancellationToken, file_sha256};
use cuerithm_rekordbox::{CollectionReadOptions, DatabaseKey, ReadOnlyDatabase, TargetValue};
use std::path::Path;
fn key() -> DatabaseKey {
    DatabaseKey::new(b"generated collection key").unwrap()
}
fn fixture(path: &Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(path).unwrap();
    let bytes = b"generated collection key";
    assert_eq!(
        unsafe {
            rusqlite::ffi::sqlite3_key(conn.handle(), bytes.as_ptr().cast(), bytes.len() as i32)
        },
        rusqlite::ffi::SQLITE_OK
    );
    conn.pragma_update(None, "cipher_compatibility", 4).unwrap();
    // Authored minimal format fixtures. These are not redistributed application DDL.
    conn.execute_batch("CREATE TABLE djmdProperty(DBID TEXT,DBVersion TEXT,DeviceID TEXT); INSERT INTO djmdProperty VALUES('db-alpha','version-a','device-a'); CREATE TABLE agentRegistry(registry_id TEXT,int_1 INTEGER); INSERT INTO agentRegistry VALUES('localUpdateCount',20),('unownedCounter',999); CREATE TABLE djmdArtist(ID TEXT PRIMARY KEY,Name TEXT,rb_local_deleted INTEGER); INSERT INTO djmdArtist VALUES('artist','Earth, Wind & Fire',0),('album-artist','Album Artist',0); CREATE TABLE djmdAlbum(ID TEXT PRIMARY KEY,Name TEXT,AlbumArtistID TEXT,rb_local_deleted INTEGER); INSERT INTO djmdAlbum VALUES('album','Authored Album','album-artist',0); CREATE TABLE djmdGenre(ID TEXT PRIMARY KEY,Name TEXT,rb_local_deleted INTEGER); INSERT INTO djmdGenre VALUES('genre','House',0); CREATE TABLE djmdLabel(ID TEXT PRIMARY KEY,Name TEXT,rb_local_deleted INTEGER); INSERT INTO djmdLabel VALUES('label','Authored Label',0); CREATE TABLE djmdKey(ID TEXT PRIMARY KEY,ScaleName TEXT,rb_local_deleted INTEGER); INSERT INTO djmdKey VALUES('key','Am',0);
 CREATE TABLE djmdContent(ID TEXT PRIMARY KEY,UUID TEXT,MasterSongID TEXT,Title TEXT,FolderPath TEXT,OrgFolderPath TEXT,ServiceID INTEGER,ArtistID TEXT,AlbumID TEXT,GenreID TEXT,LabelID TEXT,KeyID TEXT,RemixerID TEXT,ComposerID TEXT,Lyricist TEXT,Subtitle TEXT,Commnt TEXT,BPM INTEGER,Length INTEGER,Rating INTEGER,TrackNo INTEGER,DiscNo INTEGER,ReleaseYear INTEGER,ReleaseDate TEXT,FileType INTEGER,FileSize INTEGER,SampleRate INTEGER,BitRate INTEGER,BitDepth INTEGER,AnalysisDataPath TEXT,ImagePath TEXT,rb_local_deleted INTEGER,rb_local_usn INTEGER,DJPlayCount INTEGER,ReservedUser BLOB);
 INSERT INTO djmdContent VALUES('1','uuid-1','master-1','Signal','/authored/Signal.flac',NULL,0,'artist','album','genre','label','key','artist','artist','Lyricist','Original Mix','Comments',12875,301,4,2,1,2025,'2025-02-03',5,12000,44100,1000,16,'/PIONEER/USBANLZ/a/ANLZ0000.DAT','/PIONEER/ART/a.jpg',0,20,50,x'0102');
 INSERT INTO djmdContent(ID,UUID,MasterSongID,Title,FolderPath,ServiceID,rb_local_deleted) VALUES('2','uuid-2','master-2','Deleted','/authored/Deleted.flac',0,1),('3','uuid-3','master-3','Remote','/contents/remote.flac',2,0),('4','uuid-4',NULL,'Unlinked','/authored/Unlinked.flac',0,0);
 CREATE TABLE djmdCue(ID TEXT PRIMARY KEY,ContentID TEXT,ContentUUID TEXT,UUID TEXT,Kind INTEGER,InMsec INTEGER,OutMsec INTEGER,ActiveLoop INTEGER,Comment TEXT,Color INTEGER,ColorTableIndex INTEGER,BeatLoopSize INTEGER,rb_local_deleted INTEGER);
 INSERT INTO djmdCue VALUES('cue-a','1','uuid-1','cue-uuid-a',1,1250,-1,0,'Drop',65536,3,0,0),('cue-b','1','uuid-1','cue-uuid-b',4,2000,10000,0,'Memory loop',2,0,16,0);
 CREATE TABLE contentCue(ID TEXT PRIMARY KEY,ContentID TEXT,Cues TEXT,rb_cue_count INTEGER,rb_local_deleted INTEGER); INSERT INTO contentCue VALUES('shadow','1','authored opaque cache',2,0);
 CREATE TABLE djmdPlaylist(ID TEXT PRIMARY KEY,UUID TEXT,ParentID TEXT,Name TEXT,Attribute INTEGER,Seq INTEGER,SmartList TEXT,rb_local_deleted INTEGER); INSERT INTO djmdPlaylist VALUES('folder','folder-uuid','root','Folder',1,1,NULL,0),('list','list-uuid','folder','Set',0,1,NULL,0),('smart','smart-uuid','folder','Smart',4,2,'<NODE authored=\"true\"/>',0);
 CREATE TABLE djmdSongPlaylist(ID TEXT PRIMARY KEY,PlaylistID TEXT,ContentID TEXT,TrackNo INTEGER,rb_local_deleted INTEGER); INSERT INTO djmdSongPlaylist VALUES('link-b','list','1',2,0),('link-a','list','1',1,0),('link-c','list','2',3,0),('link-d','list','3',4,0);
 CREATE TABLE djmdMyTag(ID TEXT PRIMARY KEY,ParentID TEXT,Name TEXT,Attribute INTEGER,Seq INTEGER,rb_local_deleted INTEGER); INSERT INTO djmdMyTag VALUES('category','root','Genre',1,1,0),('tag','category','House',0,1,0);
 CREATE TABLE djmdSongMyTag(ID TEXT PRIMARY KEY,MyTagID TEXT,ContentID TEXT,rb_local_deleted INTEGER); INSERT INTO djmdSongMyTag VALUES('tag-link','tag','1',0);").unwrap();
    conn
}
#[test]
fn native_collection_import_resolves_metadata_and_preserves_identity_units_and_opaque_values() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    drop(fixture(&path));
    let before = file_sha256(&path).unwrap();
    let db = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let c = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert_eq!(c.identity.database_id, "db-alpha");
    assert_eq!(c.identity.device_id.as_deref(), Some("device-a"));
    assert_eq!(c.identity.local_update_count, Some(20));
    assert_eq!(
        c.records["agentRegistry"].len(),
        1,
        "Only the protocol counter is part of import, not unrelated registry data"
    );
    assert_eq!(c.tracks.len(), 4);
    assert_eq!(c.active_local_tracks().count(), 1);
    let t = &c
        .tracks
        .iter()
        .find(|t| t.identity.content_id == "1")
        .unwrap();
    assert_eq!(t.identity.uuid.as_deref(), Some("uuid-1"));
    assert_eq!(t.identity.master_song_id.as_deref(), Some("master-1"));
    assert_eq!(t.bpm, Some(128.75));
    assert_eq!(t.duration_seconds, Some(301.));
    assert_eq!(t.metadata.artist, "Earth, Wind & Fire");
    assert_eq!(t.metadata.album_artist, "Album Artist");
    assert_eq!(t.metadata.genre, "House");
    assert_eq!(t.metadata.key, "Am");
    assert_eq!(t.metadata.rating, Some(4));
    assert_eq!(
        t.source.analysis_path.as_deref(),
        Some("/PIONEER/USBANLZ/a/ANLZ0000.DAT")
    );
    assert_eq!(t.tag_ids, vec!["tag"]);
    let raw = c.records["djmdContent"]
        .iter()
        .find(|r| r.id == "1")
        .unwrap();
    assert_eq!(raw.values["ReservedUser"], TargetValue::Blob(vec![1, 2]));
    assert_eq!(raw.values["DJPlayCount"], TargetValue::Integer(50));
    assert!(
        !serde_json::to_value(&t.metadata)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("plays")
    );
    assert_eq!(file_sha256(&path).unwrap(), before);
}
#[test]
fn cue_slots_memory_loops_and_ordered_memberships_keep_native_semantics() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    drop(fixture(&path));
    let db = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let c = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    let t = c.active_local_tracks().next().unwrap();
    let hot = t.cues.iter().find(|c| c.id == "cue-a").unwrap();
    assert_eq!(hot.slot, Some(0));
    assert!(!hot.memory);
    assert_eq!(hot.start_seconds, Some(1.25));
    assert_eq!(hot.end_seconds, None);
    assert_eq!(hot.color_table_index, Some(3));
    let memory = t.cues.iter().find(|c| c.id == "cue-b").unwrap();
    assert!(memory.memory && memory.active_loop);
    assert_eq!(memory.slot, None);
    assert_eq!(memory.end_seconds, Some(10.));
    assert_eq!(c.playlist_local_track_ids("list"), vec!["1"]);
    assert_eq!(c.playlist_links.len(), 4);
    assert!(
        c.playlists.iter().any(
            |p| p.id == "smart" && p.smart_rule.as_deref() == Some("<NODE authored=\"true\"/>")
        )
    );
    assert_eq!(c.tags.len(), 2);
}
#[test]
fn raw_target_changes_are_separate_from_requested_metadata_baselines() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    let writer = fixture(&path);
    let db = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let first = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    writer
        .execute("UPDATE djmdContent SET DJPlayCount=99 WHERE ID='1'", [])
        .unwrap();
    let second = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    let a = first.active_local_tracks().next().unwrap();
    let b = second.active_local_tracks().next().unwrap();
    assert_eq!(a.metadata_hash, b.metadata_hash);
    assert_ne!(a.raw_hash, b.raw_hash);
    writer
        .execute("UPDATE djmdContent SET BPM=12900 WHERE ID='1'", [])
        .unwrap();
    let changed = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert_ne!(
        b.metadata_hash,
        changed.active_local_tracks().next().unwrap().metadata_hash,
        "BPM belongs to the normalized metadata baseline"
    );
}
#[test]
fn exhausted_limits_and_cancellation_never_return_an_apparently_complete_collection() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    drop(fixture(&path));
    let db = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let token = CancellationToken::default();
    assert!(
        db.read_collection(
            CollectionReadOptions {
                max_rows: 2,
                ..Default::default()
            },
            &token
        )
        .is_err()
    );
    let cancelled = CancellationToken::default();
    cancelled.cancel();
    assert!(db.read_collection(Default::default(), &cancelled).is_err());
    let c = db.read_collection(Default::default(), &token).unwrap();
    assert_eq!(c.tracks.len(), 4);
}

#[test]
fn fractional_native_bitrates_are_preserved_instead_of_rejected_or_truncated() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    let writer = fixture(&path);
    writer
        .execute("UPDATE djmdContent SET BitRate=1411.2 WHERE ID='1'", [])
        .unwrap();
    drop(writer);
    let db = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let c = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    let raw = &c.records["djmdContent"]
        .iter()
        .find(|r| r.id == "1")
        .unwrap()
        .values["BitRate"];
    assert_eq!(raw, &TargetValue::Real(1411.2));
    assert_eq!(
        c.active_local_tracks().next().unwrap().source.bitrate,
        Some(1411.2)
    );
}

#[test]
fn database_scope_and_unresolved_native_rows_are_not_silently_collapsed() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("a.db");
    let b = d.path().join("b.db");
    drop(fixture(&a));
    let writer = fixture(&b);
    writer
        .execute("UPDATE djmdProperty SET DBID='db-beta'", [])
        .unwrap();
    writer
        .execute("UPDATE djmdCue SET Kind=18 WHERE ID='cue-a'", [])
        .unwrap();
    writer
        .execute("UPDATE djmdContent SET ArtistID='missing' WHERE ID='1'", [])
        .unwrap();
    drop(writer);
    let ca = ReadOnlyDatabase::open(&a, &key())
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    let cb = ReadOnlyDatabase::open(&b, &key())
        .unwrap()
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert_eq!(
        ca.tracks[0].identity.content_id,
        cb.tracks[0].identity.content_id
    );
    assert_ne!(
        ca.tracks[0].identity.database_id,
        cb.tracks[0].identity.database_id
    );
    assert!(cb.issues.iter().any(|i| i.code == "unsupported_cue_kind"));
    assert!(cb.issues.iter().any(|i| i.code == "missing_foreign_record"));
    assert_eq!(cb.records["djmdCue"].len(), 2);
}
#[test]
fn native_cloud_storage_paths_require_an_explicit_local_resolution() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("master.db");
    let writer = fixture(&path);
    writer.execute("UPDATE djmdContent SET ServiceID=2, FolderPath='/contents_owner/Signal.flac', OrgFolderPath='C:/Original/Signal.flac' WHERE ID='1'",[]).unwrap();
    let db = ReadOnlyDatabase::open(&path, &key()).unwrap();
    let fallback = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert_eq!(
        fallback.tracks[0].source.resolved_path.as_deref(),
        Some("C:/Original/Signal.flac")
    );
    let configured = db
        .read_collection(
            CollectionReadOptions {
                dropbox_root: Some("/Users/Example/Dropbox/".into()),
                ..Default::default()
            },
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(
        configured.tracks[0].source.resolved_path.as_deref(),
        Some("/Users/Example/Dropbox/rekordbox/contents_owner/Signal.flac")
    );
    writer.execute("UPDATE djmdContent SET ServiceID=3, OrgFolderPath='/Original/Signal.flac' WHERE ID='1'",[]).unwrap();
    let google = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert_eq!(
        google.tracks[0].source.resolved_path.as_deref(),
        Some("/Original/Signal.flac")
    );
    writer
        .execute(
            "UPDATE djmdContent SET ServiceID=0, FolderPath='streaming:unsupported' WHERE ID='1'",
            [],
        )
        .unwrap();
    let invalid = db
        .read_collection(Default::default(), &CancellationToken::default())
        .unwrap();
    assert!(!invalid.tracks[0].importable_local());
}
