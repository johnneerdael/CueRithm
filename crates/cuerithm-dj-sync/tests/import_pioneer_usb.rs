#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use std::{collections::BTreeMap, path::Path};
fn le16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}
fn le32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn be32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}
fn text(s: &str, utf16: bool) -> Vec<u8> {
    if utf16 {
        let payload = s
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let mut b = vec![0x90];
        b.extend(((payload.len() + 4) as u16).to_le_bytes());
        b.push(0);
        b.extend(payload);
        b
    } else {
        let mut b = vec![(s.len() * 2 + 3) as u8];
        b.extend(s.as_bytes());
        b
    }
}
fn named(id: u32, name: &str) -> Vec<u8> {
    let mut b = id.to_le_bytes().to_vec();
    b.extend(text(name, false));
    b
}
fn track_row() -> Vec<u8> {
    let mut b = vec![0; 136];
    le16(&mut b, 0, 36);
    for (at, v) in [
        (8, 48000),
        (16, 100),
        (20, 99),
        (24, 78),
        (32, 1),
        (56, 12875),
        (68, 2),
        (72, 7),
    ] {
        le32(&mut b, at, v);
    }
    le16(&mut b, 84, 90);
    le16(&mut b, 90, 5);
    let strings = BTreeMap::from([
        (10, "2026-10-23"),
        (14, "/PIONEER/USBANLZ/a/ANLZ0000.DAT"),
        (16, "source comment"),
        (17, "Example"),
        (19, "Example.flac"),
        (20, "/Contents/Example.flac"),
    ]);
    for i in 0..21 {
        let at = b.len() as u16;
        le16(&mut b, 94 + i * 2, at);
        b.extend(text(strings.get(&i).unwrap_or(&""), false));
    }
    b
}
fn pdb_tables() -> BTreeMap<u32, Vec<Vec<u8>>> {
    let mut artist = vec![0; 16];
    le16(&mut artist, 0, 100);
    le32(&mut artist, 4, 2);
    le16(&mut artist, 10, 16);
    artist.extend(text("DJ 🎧", true));
    let mut key = vec![0; 8];
    le32(&mut key, 0, 1);
    le32(&mut key, 4, 1);
    key.extend(text("Am", false));
    let playlist = |id, parent, folder, name: &str| {
        let mut b = vec![0; 20];
        le32(&mut b, 0, parent);
        le32(&mut b, 8, 5);
        le32(&mut b, 12, id);
        le32(&mut b, 16, folder);
        b.extend(text(name, false));
        b
    };
    let member = |position| {
        let mut b = vec![0; 12];
        le32(&mut b, 0, position);
        le32(&mut b, 4, 7);
        le32(&mut b, 8, 11);
        b
    };
    BTreeMap::from([
        (0, vec![track_row()]),
        (1, vec![named(1, "House")]),
        (2, vec![artist]),
        (5, vec![key]),
        (
            7,
            vec![playlist(10, 0, 1, "Folder"), playlist(11, 10, 0, "Set")],
        ),
        (8, vec![member(9), member(4)]),
        (11, vec![named(12, "History")]),
        (
            12,
            vec![{
                let mut b = vec![0; 12];
                le32(&mut b, 0, 7);
                le32(&mut b, 4, 12);
                le32(&mut b, 8, 3);
                b
            }],
        ),
    ])
}
// Hand-authored page directory/heap/bitmaps, independent of product encoders.
fn pdb(tables: &BTreeMap<u32, Vec<Vec<u8>>>, size: usize) -> Vec<u8> {
    let mut b = vec![0; size * (tables.len() + 1)];
    le32(&mut b, 4, size as u32);
    le32(&mut b, 8, tables.len() as u32);
    le32(&mut b, 12, (tables.len() + 1) as u32);
    for (i, (kind, rows)) in tables.iter().enumerate() {
        let page = i + 1;
        let dir = 28 + i * 16;
        le32(&mut b, dir, *kind);
        le32(&mut b, dir + 8, page as u32);
        le32(&mut b, dir + 12, page as u32);
        let out = &mut b[page * size..(page + 1) * size];
        le32(out, 4, page as u32);
        le32(out, 8, *kind);
        let packed = rows.len() as u32 | ((rows.len() as u32) << 13);
        out[24..27].copy_from_slice(&packed.to_le_bytes()[..3]);
        out[27] = 0x24;
        let mut used = 0;
        let mut cursor = size;
        for group in rows.chunks(16) {
            cursor -= 4;
            le16(out, cursor, ((1u32 << group.len()) - 1) as u16);
            for row in group {
                cursor -= 2;
                le16(out, cursor, used as u16);
                out[40 + used..40 + used + row.len()].copy_from_slice(row);
                used += row.len();
            }
        }
        assert!(40 + used <= cursor);
        le16(out, 28, (cursor - 40 - used) as u16);
        le16(out, 30, used as u16);
    }
    b
}
fn sql(root: &Path) -> rusqlite::Connection {
    let path = root.join("PIONEER/rekordbox/exportLibrary.db");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = rusqlite::Connection::open(path).unwrap();
    // Existing public device-format interoperability profile, not an account credential.
    db.pragma_update(
        None,
        "key",
        "r8gddnr4k847830ar6cqzbkk0el6qytmb3trbbx805jm74vez64i5o8fnrqryqls",
    )
    .unwrap();
    db.pragma_update(None, "cipher_compatibility", 4).unwrap();
    db.execute_batch("CREATE TABLE property(deviceName TEXT,dbVersion TEXT);INSERT INTO property VALUES('Invented USB','1000');CREATE TABLE content(content_id INTEGER PRIMARY KEY,title TEXT,bpmx100 INTEGER,length INTEGER,path TEXT,analysisDataFilePath TEXT,artist_id_artist INTEGER,key_id INTEGER,masterDbId INTEGER,masterContentId INTEGER,djComment TEXT);INSERT INTO content VALUES(7,'SQL Example',12875,90,'/Contents/Example.flac','/PIONEER/USBANLZ/a/ANLZ0000.DAT',2,1,78,99,'source SQL comment');CREATE TABLE artist(artist_id INTEGER PRIMARY KEY,name TEXT);INSERT INTO artist VALUES(2,'DJ 🎧');CREATE TABLE album(album_id INTEGER PRIMARY KEY,name TEXT);CREATE TABLE genre(genre_id INTEGER PRIMARY KEY,name TEXT);CREATE TABLE label(label_id INTEGER PRIMARY KEY,name TEXT);CREATE TABLE key(key_id INTEGER PRIMARY KEY,name TEXT);INSERT INTO key VALUES(1,'Am');CREATE TABLE playlist(playlist_id INTEGER PRIMARY KEY,sequenceNo INTEGER,name TEXT,attribute INTEGER,playlist_id_parent INTEGER);INSERT INTO playlist VALUES(10,5,'Folder',1,0),(11,5,'Set',0,10);CREATE TABLE playlist_content(playlist_id INTEGER,content_id INTEGER,sequenceNo INTEGER);INSERT INTO playlist_content VALUES(11,7,9),(11,7,4);CREATE TABLE cue(cue_id INTEGER PRIMARY KEY,content_id INTEGER,kind INTEGER,inUsec INTEGER,outUsec INTEGER,cueComment TEXT,isActiveLoop INTEGER);INSERT INTO cue VALUES(1,7,0,1250000,NULL,'Memory',0),(2,7,3,1250000,-1,'Hot',0),(3,7,1,4000000,8000000,'Loop',1);CREATE TABLE history(history_id INTEGER PRIMARY KEY,sequenceNo INTEGER,name TEXT,attribute INTEGER,history_id_parent INTEGER);INSERT INTO history VALUES(12,1,'History',0,0);CREATE TABLE history_content(history_id INTEGER,content_id INTEGER,sequenceNo INTEGER);INSERT INTO history_content VALUES(12,7,3);").unwrap();
    db
}
fn tag(kind: &[u8; 4], header: usize, body: &[u8]) -> Vec<u8> {
    let mut b = vec![0; header];
    b[..4].copy_from_slice(kind);
    be32(&mut b, 4, header as u32);
    be32(&mut b, 8, (header + body.len()) as u32);
    b.extend(body);
    b
}
fn anlz() -> Vec<u8> {
    let mut beats = Vec::new();
    for (number, time) in [(4u16, 250u32), (1, 750), (2, 1250)] {
        beats.extend(number.to_be_bytes());
        beats.extend(12000u16.to_be_bytes());
        beats.extend(time.to_be_bytes());
    }
    let mut grid = tag(b"PQTZ", 24, &beats);
    be32(&mut grid, 20, 3);
    let mut entries = Vec::new();
    for (hot, kind, start, end) in [
        (0u32, 1, 1250, u32::MAX),
        (3, 1, 1250, u32::MAX),
        (0, 2, 4000, 8000),
    ] {
        let mut b = vec![0; 40];
        b[..4].copy_from_slice(b"PCPT");
        be32(&mut b, 4, 28);
        be32(&mut b, 8, 40);
        be32(&mut b, 12, hot);
        b[28] = kind;
        be32(&mut b, 32, start);
        be32(&mut b, 36, end);
        entries.extend(b);
    }
    let mut cues = tag(b"PCOB", 24, &entries);
    cues[18..20].copy_from_slice(&3u16.to_be_bytes());
    let mut all = grid;
    all.extend(cues);
    tag(b"PMAI", 28, &all)
}
fn files(root: &Path) {
    std::fs::create_dir_all(root.join("Contents")).unwrap();
    std::fs::write(
        root.join("Contents/Example.flac"),
        b"invented metadata-only media",
    )
    .unwrap();
    let path = root.join("PIONEER/rekordbox/export.pdb");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, pdb(&pdb_tables(), 4096)).unwrap();
    let path = root.join("PIONEER/USBANLZ/a/ANLZ0000.DAT");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, anlz()).unwrap();
}
fn capture(root: &Path, sql: bool, both: bool) -> ImportSnapshot {
    let mut artifacts = vec![ArtifactRequest::stable("PIONEER/USBANLZ/a/ANLZ0000.DAT")];
    if sql {
        artifacts.push(ArtifactRequest::sqlite(
            "PIONEER/rekordbox/exportLibrary.db",
        ));
    }
    if !sql || both {
        artifacts.push(ArtifactRequest::stable("PIONEER/rekordbox/export.pdb"));
    }
    let request =
        SourceCaptureRequest::from_selected_root(SourceProvider::PioneerUsb, root, artifacts)
            .unwrap();
    adapter(SourceProvider::PioneerUsb)
        .capture(
            &request,
            root,
            &ImportLimits::default(),
            &CancellationToken::default(),
        )
        .unwrap()
}
#[test]
fn usb_reads_pdb_rows_and_onelibrary_without_copying_audio() {
    for use_sql in [false, true] {
        let root = tempfile::tempdir().unwrap();
        files(root.path());
        if use_sql {
            drop(sql(root.path()));
        }
        let snapshot = capture(root.path(), use_sql, use_sql);
        let (_, rows) = support::read(SourceProvider::PioneerUsb, &snapshot);
        let tracks = rows.tracks();
        assert_eq!(tracks.len(), 1);
        let one = tracks[0];
        assert_eq!(one.external_id, "track:7");
        assert_eq!(one.bpm.as_ref().unwrap().value, Some(128.75));
        assert_eq!(one.key.as_ref().unwrap().camelot.as_deref(), Some("8A"));
        assert_eq!(
            one.metadata["artist"],
            Presence::Value(serde_json::json!("DJ 🎧"))
        );
        assert_eq!(
            one.metadata["title"],
            Presence::Value(serde_json::json!(if use_sql {
                "SQL Example"
            } else {
                "Example"
            }))
        );
        assert_eq!(
            one.locations.resolved.as_deref(),
            root.path().join("Contents/Example.flac").to_str()
        );
        assert_eq!(
            std::fs::read(root.path().join("Contents/Example.flac")).unwrap(),
            b"invented metadata-only media"
        );
        assert_eq!(
            std::fs::read_dir(root.path().join("Contents"))
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            rows.members()
                .iter()
                .map(|(_, m)| m.position.unwrap())
                .collect::<Vec<_>>(),
            [4, 9]
        );
    }
}
#[test]
fn usb_preserves_cue_classes_anlz_and_membership_order() {
    for use_sql in [false, true] {
        let root = tempfile::tempdir().unwrap();
        files(root.path());
        if use_sql {
            drop(sql(root.path()));
        }
        let (_, rows) = support::read(
            SourceProvider::PioneerUsb,
            &capture(root.path(), use_sql, false),
        );
        let one = rows.tracks()[0];
        assert_eq!(one.cues.len(), 3);
        assert!(
            one.cues
                .iter()
                .any(|c| c.family == CueFamily::Memory && c.native_start_seconds == Some(1.25))
        );
        assert!(one.cues.iter().any(|c| c.family == CueFamily::Hot
            && c.source_slot == Some(2)
            && c.native_start_seconds == Some(1.25)));
        assert!(one.cues.iter().all(|c| c.canonical_start_seconds.is_none()));
        let loop_cue = one
            .cues
            .iter()
            .find(|c| c.native_start_seconds == Some(4.))
            .unwrap();
        assert_eq!(loop_cue.native_end_seconds, Some(8.));
        if use_sql {
            assert_eq!(loop_cue.flags["active_loop"], true);
        }
        let grid = one.grid.as_ref().unwrap();
        assert_eq!(
            grid.points
                .iter()
                .map(|p| p.native_seconds)
                .collect::<Vec<_>>(),
            [0.25, 0.75, 1.25]
        );
        assert_eq!(grid.points[0].original["number"], 4);
        assert!(
            one.issues
                .iter()
                .any(|i| i.code == "missing_usb_analysis_ext")
        );
    }
}

fn read_options(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
) -> (ObservationHeader, support::Records) {
    let mut rows = support::Records::default();
    let header = adapter(SourceProvider::PioneerUsb)
        .read(snapshot, options, &mut rows, &CancellationToken::default())
        .unwrap();
    validate_observation(&header, &mut rows.rows.clone().into_iter(), &options.limits).unwrap();
    snapshot
        .validate_source(&CancellationToken::default())
        .unwrap();
    (header, rows)
}
#[test]
fn pdb_overflow_heap_overlap_and_link_cycles_fail_without_a_complete_observation() {
    for variant in 0..4 {
        let root = tempfile::tempdir().unwrap();
        files(root.path());
        let mut bytes = pdb(&pdb_tables(), 4096);
        match variant {
            0 => le32(&mut bytes, 28 + 8, u32::MAX),
            1 => le16(&mut bytes, 4096 + 30, 4090),
            2 => {
                le32(&mut bytes, 28 + 12, 2);
                le32(&mut bytes, 4096 + 12, 1);
            }
            _ => le16(&mut bytes, 4096 + 4096 - 6, u16::MAX),
        }
        std::fs::write(root.path().join("PIONEER/rekordbox/export.pdb"), bytes).unwrap();
        let snapshot = capture(root.path(), false, false);
        let mut rows = support::Records::default();
        assert!(
            adapter(SourceProvider::PioneerUsb)
                .read(
                    &snapshot,
                    &ReadOptions::default(),
                    &mut rows,
                    &CancellationToken::default()
                )
                .is_err()
        );
        snapshot
            .validate_source(&CancellationToken::default())
            .unwrap();
    }
}
#[test]
fn unknown_packed_string_is_not_an_empty_title() {
    let root = tempfile::tempdir().unwrap();
    files(root.path());
    let mut tables = pdb_tables();
    let row = &mut tables.get_mut(&0).unwrap()[0];
    let at = u16::from_le_bytes(row[128..130].try_into().unwrap()) as usize;
    row[at] = 0x20;
    std::fs::write(
        root.path().join("PIONEER/rekordbox/export.pdb"),
        pdb(&tables, 4096),
    )
    .unwrap();
    let (_, rows) = support::read(
        SourceProvider::PioneerUsb,
        &capture(root.path(), false, false),
    );
    let one = rows.tracks()[0];
    assert!(!one.metadata.contains_key("title"));
    assert!(one.issues.iter().any(|i| i.code == "unresolved_pdb_title"));
    assert!(one.extensions["source_row"]["raw_record"].is_object());
}
#[test]
fn inactive_rows_still_bound_visible_row_lengths_and_head_pages_are_followed() {
    let root = tempfile::tempdir().unwrap();
    files(root.path());
    let first = track_row();
    let mut second = first.clone();
    le32(&mut second, 72, 8);
    let mut bytes = pdb(&BTreeMap::from([(0, vec![first.clone(), second])]), 4096);
    le16(&mut bytes, 4096 + 4096 - 4, 1);
    std::fs::write(root.path().join("PIONEER/rekordbox/export.pdb"), &bytes).unwrap();
    let (_, rows) = support::read(
        SourceProvider::PioneerUsb,
        &capture(root.path(), false, false),
    );
    assert_eq!(rows.tracks().len(), 1);
    assert_eq!(
        rows.tracks()[0].extensions["source_row"]["raw_record"]["bytes"],
        first.len()
    );
    let mut chain = vec![0; 4096 * 3];
    chain[..4096].copy_from_slice(&bytes[..4096]);
    le32(&mut chain, 28 + 12, 2);
    let head = &mut chain[4096..8192];
    le32(head, 4, 1);
    le32(head, 8, 0);
    le32(head, 12, 2);
    head[27] = 0x64;
    chain[8192..].copy_from_slice(&bytes[4096..]);
    le32(&mut chain, 8192 + 4, 2);
    std::fs::write(root.path().join("PIONEER/rekordbox/export.pdb"), chain).unwrap();
    let (_, rows) = support::read(
        SourceProvider::PioneerUsb,
        &capture(root.path(), false, false),
    );
    assert_eq!(
        rows.tracks()[0].metadata["title"],
        Presence::Value(serde_json::json!("Example"))
    );
}
fn extended() -> Vec<u8> {
    let comment = "Black\0"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    let mut entry = vec![0; 44];
    entry[..4].copy_from_slice(b"PCP2");
    be32(&mut entry, 4, 16);
    be32(&mut entry, 8, (44 + comment.len() + 4) as u32);
    be32(&mut entry, 12, 1);
    entry[16] = 1;
    be32(&mut entry, 20, 1250);
    be32(&mut entry, 24, u32::MAX);
    be32(&mut entry, 40, comment.len() as u32);
    entry.extend(comment);
    entry.extend([5, 0, 0, 0]);
    let mut list = tag(b"PCO2", 20, &entry);
    list[16..18].copy_from_slice(&1u16.to_be_bytes());
    tag(b"PMAI", 28, &list)
}
#[test]
fn extended_cues_precede_legacy_while_sql_rows_keep_their_separate_authority() {
    for use_sql in [false, true] {
        let root = tempfile::tempdir().unwrap();
        files(root.path());
        if use_sql {
            drop(sql(root.path()));
        }
        std::fs::write(
            root.path().join("PIONEER/USBANLZ/a/ANLZ0000.EXT"),
            extended(),
        )
        .unwrap();
        let mut request = SourceCaptureRequest::from_selected_root(
            SourceProvider::PioneerUsb,
            root.path(),
            vec![
                ArtifactRequest::stable("PIONEER/rekordbox/export.pdb"),
                ArtifactRequest::stable("PIONEER/USBANLZ/a/ANLZ0000.DAT"),
                ArtifactRequest::stable("PIONEER/USBANLZ/a/ANLZ0000.EXT"),
            ],
        )
        .unwrap();
        if use_sql {
            request = SourceCaptureRequest::from_selected_root(
                SourceProvider::PioneerUsb,
                root.path(),
                vec![
                    ArtifactRequest::sqlite("PIONEER/rekordbox/exportLibrary.db"),
                    ArtifactRequest::stable("PIONEER/USBANLZ/a/ANLZ0000.DAT"),
                    ArtifactRequest::stable("PIONEER/USBANLZ/a/ANLZ0000.EXT"),
                ],
            )
            .unwrap();
        }
        let snapshot = adapter(SourceProvider::PioneerUsb)
            .capture(
                &request,
                root.path(),
                &ImportLimits::default(),
                &CancellationToken::default(),
            )
            .unwrap();
        let (_, rows) = support::read(SourceProvider::PioneerUsb, &snapshot);
        let one = rows.tracks()[0];
        if use_sql {
            assert_eq!(one.cues.len(), 3);
            assert!(
                one.issues
                    .iter()
                    .any(|i| i.code == "usb_sql_cues_precede_anlz")
            );
        } else {
            assert_eq!(one.cues.len(), 1);
            assert_eq!(one.cues[0].source_slot, Some(0));
            assert_eq!(one.cues[0].name.as_deref(), Some("Black"));
            assert_eq!(one.cues[0].color_rgb, Some(0));
        }
        assert_eq!(
            one.extensions["performance_layers"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }
}
#[test]
fn history_is_opt_in_and_repeated_memberships_keep_physical_occurrences() {
    for use_sql in [false, true] {
        let root = tempfile::tempdir().unwrap();
        files(root.path());
        if use_sql {
            drop(sql(root.path()));
        }
        let snapshot = capture(root.path(), use_sql, false);
        let (_, default) = support::read(SourceProvider::PioneerUsb, &snapshot);
        assert!(
            default
                .collections()
                .iter()
                .all(|c| c.kind != CollectionKind::History)
        );
        let (_, rows) = read_options(
            &snapshot,
            &ReadOptions {
                include_history: true,
                ..Default::default()
            },
        );
        assert!(
            rows.collections()
                .iter()
                .any(|c| c.kind == CollectionKind::History)
        );
        let members = rows.members();
        assert_eq!(members.len(), 3);
        let list = members
            .iter()
            .filter(|(c, _)| *c == "playlist:11")
            .collect::<Vec<_>>();
        assert_eq!(list.len(), 2);
        assert_ne!(
            list[0].1.external_occurrence_id,
            list[1].1.external_occurrence_id
        );
    }
}
#[test]
fn unsupported_sql_version_does_not_fall_back_to_an_older_pdb() {
    let root = tempfile::tempdir().unwrap();
    files(root.path());
    let db = sql(root.path());
    db.execute_batch("UPDATE property SET dbVersion='1001'")
        .unwrap();
    drop(db);
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::PioneerUsb,
        root.path(),
        vec![
            ArtifactRequest::sqlite("PIONEER/rekordbox/exportLibrary.db"),
            ArtifactRequest::stable("PIONEER/rekordbox/export.pdb"),
        ],
    )
    .unwrap();
    assert!(
        adapter(SourceProvider::PioneerUsb)
            .capture(
                &request,
                root.path(),
                &ImportLimits::default(),
                &CancellationToken::default()
            )
            .is_err()
    );
}
#[test]
fn missing_media_and_cancelled_reads_never_drop_catalog_metadata() {
    let root = tempfile::tempdir().unwrap();
    files(root.path());
    std::fs::remove_file(root.path().join("Contents/Example.flac")).unwrap();
    let snapshot = capture(root.path(), false, false);
    let (_, rows) = support::read(SourceProvider::PioneerUsb, &snapshot);
    assert_eq!(rows.tracks()[0].locations.availability, "missing");
    assert_eq!(
        rows.tracks()[0].metadata["title"],
        Presence::Value(serde_json::json!("Example"))
    );
    let cancel = CancellationToken::default();
    cancel.cancel();
    let mut rows = support::Records::default();
    assert!(
        adapter(SourceProvider::PioneerUsb)
            .read(&snapshot, &ReadOptions::default(), &mut rows, &cancel)
            .is_err()
    );
    assert!(rows.rows.is_empty());
}

#[test]
fn alternate_page_size_lookup_subtypes_long_ascii_and_multiple_row_groups_survive() {
    let root = tempfile::tempdir().unwrap();
    files(root.path());
    let mut tables = pdb_tables();
    let mut artist = vec![0; 10];
    le16(&mut artist, 0, 96);
    le32(&mut artist, 4, 2);
    artist[9] = 10;
    artist.extend(text("Short DJ", false));
    tables.insert(2, vec![artist]);
    let name = "Long album ".repeat(14);
    let mut album = vec![0; 24];
    le16(&mut album, 0, 132);
    le32(&mut album, 8, 2);
    le32(&mut album, 12, 4);
    le16(&mut album, 22, 24);
    album.push(0x40);
    album.extend(((name.len() + 4) as u16).to_le_bytes());
    album.push(0);
    album.extend(name.as_bytes());
    tables.insert(3, vec![album]);
    tables.insert(
        1,
        (1..=17)
            .map(|id| named(id, &format!("Genre{id}")))
            .collect(),
    );
    le32(&mut tables.get_mut(&0).unwrap()[0], 60, 3);
    le32(&mut tables.get_mut(&0).unwrap()[0], 64, 4);
    std::fs::write(
        root.path().join("PIONEER/rekordbox/export.pdb"),
        pdb(&tables, 1024),
    )
    .unwrap();
    let (_, rows) = support::read(
        SourceProvider::PioneerUsb,
        &capture(root.path(), false, false),
    );
    let one = rows.tracks()[0];
    assert_eq!(
        one.metadata["artist"],
        Presence::Value(serde_json::json!("Short DJ"))
    );
    assert_eq!(
        one.metadata["album"],
        Presence::Value(serde_json::json!(name))
    );
    assert_eq!(
        one.metadata["genre"],
        Presence::Value(serde_json::json!("Genre3"))
    );
}
