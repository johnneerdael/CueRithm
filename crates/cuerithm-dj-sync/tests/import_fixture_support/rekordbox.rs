// Independent authored SQL fixture shared from the existing direct-read tests; no vendor database or key.
use std::path::Path;
pub fn fixture(path: &Path) -> rusqlite::Connection {
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
