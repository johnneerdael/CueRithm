use super::*;
fn bookmark(path: &str) -> Vec<u8> {
    let mut b = vec![0; 20];
    b[..4].copy_from_slice(b"book");
    b[12..16].copy_from_slice(&16_u32.to_le_bytes());
    let mut offsets = Vec::new();
    for component in path.trim_start_matches('/').split('/') {
        offsets.push((b.len() - 16) as u32);
        b.extend((component.len() as u32).to_le_bytes());
        b.extend(0x101_u32.to_le_bytes());
        b.extend(component.as_bytes());
    }
    let array = (b.len() - 16) as u32;
    b.extend((offsets.len() as u32 * 4).to_le_bytes());
    b.extend(0x601_u32.to_le_bytes());
    for offset in offsets {
        b.extend(offset.to_le_bytes());
    }
    let toc = (b.len() - 16) as u32;
    b[16..20].copy_from_slice(&toc.to_le_bytes());
    b.extend(32_u32.to_le_bytes());
    b.extend(0xffff_fffe_u32.to_le_bytes());
    b.extend(1_u32.to_le_bytes());
    b.extend(0_u32.to_le_bytes());
    b.extend(1_u32.to_le_bytes());
    b.extend(0x1004_u32.to_le_bytes());
    b.extend(array.to_le_bytes());
    b.extend(0_u32.to_le_bytes());
    let len = b.len() as u32;
    b[4..8].copy_from_slice(&len.to_le_bytes());
    b
}
#[test]
fn bookmark_parser_preserves_posix_path_spelling_on_every_host() {
    for path in ["/music/café & mix.wav", "/music/C:/track.wav"] {
        let decoded = bookmark::stored_path(&bookmark(path)).unwrap().unwrap();
        assert_eq!(decoded.to_str(), Some(path));
    }
}
#[test]
fn stored_bookmark_path_recovers_existing_native_uuid() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("MediaLibrary.db");
    let mut conn = Connection::open(&db).unwrap();
    conn.execute_batch("CREATE TABLE database2(rowid INTEGER PRIMARY KEY,collection TEXT,key TEXT,data BLOB,metadata BLOB,UNIQUE(collection,key)); CREATE TABLE yap2(extension TEXT,key TEXT,data);").unwrap();
    let tx = conn.transaction().unwrap();
    let mut location = Object::new("ADCMediaItemLocation");
    location.set("uuid", text("native-id"));
    location.set(
        "sourceURIs",
        Value::Array(vec![Value::Uri("file://localhost/old.wav".into())]),
    );
    let raw = bookmark("/music/café & mix.wav");
    location.set("urlBookmarkData", Value::Binary(raw.clone()));
    upsert(&tx, "localMediaItemLocations", "native-id", &location).unwrap();
    tx.commit().unwrap();
    let library = SyncLibrary {
        tracks: vec![SyncTrack {
            native_path: None,
            id: "source".into(),
            path: "/music/café & mix.wav".into(),
            ..Default::default()
        }],
        playlists: vec![],
    };
    write(&db, &library, &SyncOptions::default()).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM database2 WHERE collection='localMediaItemLocations'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    let tx = conn.transaction().unwrap();
    let object = existing(
        &tx,
        "localMediaItemLocations",
        "native-id",
        "ADCMediaItemLocation",
    )
    .unwrap();
    assert_eq!(object.get("urlBookmarkData"), Some(&Value::Binary(raw)));
}
#[test]
fn bookmark_parser_is_bounded_and_rejects_ambiguous_paths() {
    let valid = bookmark("/music/track.wav");
    assert_eq!(
        bookmark::stored_path(&bookmark("/music/book.wav")).unwrap(),
        Some(PathBuf::from("/music/book.wav"))
    );
    assert_eq!(
        bookmark::stored_path(&valid).unwrap(),
        Some(PathBuf::from("/music/track.wav"))
    );
    assert!(bookmark::stored_path(&bookmark("/music/../track.wav")).is_err());
    assert!(bookmark::stored_path(&bookmark("/music/invalid\0.wav")).is_err());
    let mut invalid_utf8 = valid.clone();
    invalid_utf8[28] = 0xff;
    assert!(bookmark::stored_path(&invalid_utf8).is_err());
    assert!(bookmark::stored_path(&vec![0; 1024 * 1024 + 1]).is_err());
    let mut two = valid.clone();
    two.extend(bookmark("/different/track.wav"));
    assert!(bookmark::stored_path(&two).is_err());
    let mut cycle = valid.clone();
    let toc = u32::from_le_bytes(cycle[16..20].try_into().unwrap()) as usize + 16;
    let offset = (toc - 16) as u32;
    cycle[toc + 12..toc + 16].copy_from_slice(&offset.to_le_bytes());
    assert!(bookmark::stored_path(&cycle).is_err());
    for end in 0..valid.len() {
        assert!(
            bookmark::stored_path(&valid[..end]).is_err()
                || bookmark::stored_path(&valid[..end]).unwrap().is_none()
        );
    }
}

#[test]
fn competing_native_bookmark_and_uri_identities_fail_without_writes() {
    for duplicate_bookmark in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("MediaLibrary.db");
        let mut conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE database2(rowid INTEGER PRIMARY KEY,collection TEXT,key TEXT,data BLOB,metadata BLOB,UNIQUE(collection,key)); CREATE TABLE yap2(extension TEXT,key TEXT,data);").unwrap();
        let tx = conn.transaction().unwrap();
        for id in ["first", "second"] {
            let mut location = Object::new("ADCMediaItemLocation");
            location.set("uuid", text(id));
            if duplicate_bookmark || id == "second" {
                location.set(
                    "urlBookmarkData",
                    Value::Binary(bookmark("/music/track.wav")),
                );
            } else {
                location.set(
                    "sourceURIs",
                    Value::Array(vec![Value::Uri(
                        file_uri(Path::new("/music/track.wav")).unwrap(),
                    )]),
                );
            }
            upsert(&tx, "localMediaItemLocations", id, &location).unwrap();
        }
        tx.commit().unwrap();
        let before: Vec<Vec<u8>> = conn
            .prepare("SELECT data FROM database2 ORDER BY rowid")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let library = SyncLibrary {
            tracks: vec![SyncTrack {
                native_path: None,
                id: "source".into(),
                path: "/music/track.wav".into(),
                ..Default::default()
            }],
            playlists: vec![],
        };
        assert!(write(&db, &library, &SyncOptions::default()).is_err());
        let after: Vec<Vec<u8>> = conn
            .prepare("SELECT data FROM database2 ORDER BY rowid")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(before, after);
    }
}
