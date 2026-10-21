use super::*;
use crate::model::{SyncBeat, SyncCue};
use base64::{Engine, engine::general_purpose::STANDARD};
use lofty::{
    TextEncoding,
    config::WriteOptions,
    file::TaggedFileExt,
    id3::v2::{BinaryFrame, Frame, FrameId, GeneralEncapsulatedObject, Id3v2Tag},
    prelude::{Accessor, TagExt},
    probe::Probe,
    tag::TagType,
};

fn wave(path: &Path) {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(44_u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(44100_u32.to_le_bytes());
    bytes.extend(88200_u32.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(8_u32.to_le_bytes());
    bytes.extend([0; 8]);
    std::fs::write(path, bytes).unwrap();
}

pub(super) fn fixture() -> (tempfile::TempDir, std::path::PathBuf, SyncLibrary) {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("root.sqlite");
    let audio = dir.path().join("track.wav");
    wave(&audio);
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch("PRAGMA user_version=12;
    CREATE TABLE master(revision INTEGER,last_sync_time INTEGER); INSERT INTO master VALUES(10,0);
    CREATE TABLE serato(revision INTEGER,time_last_connected INTEGER); INSERT INTO serato VALUES(10,0);
    CREATE TABLE space(id INTEGER PRIMARY KEY,name TEXT); INSERT INTO space VALUES(1,'Serato Library');
    CREATE TABLE dbv2_status(autosync_disabled INTEGER); INSERT INTO dbv2_status VALUES(0);
    CREATE TABLE container(id INTEGER PRIMARY KEY,revision INTEGER,parent_id INTEGER,name TEXT,type INTEGER,list_order INTEGER,space_id INTEGER,time_added INTEGER,expanded INTEGER,portable_id TEXT,color INTEGER);
    INSERT INTO container VALUES(0,0,NULL,'root',0,0,NULL,0,0,'',NULL);
    CREATE TABLE space_asset(id INTEGER PRIMARY KEY,asset_id INTEGER,space_id INTEGER);
    CREATE TABLE container_asset(id INTEGER PRIMARY KEY,container_id INTEGER,space_asset_id INTEGER,list_order INTEGER,revision INTEGER);
    CREATE TABLE asset(id INTEGER PRIMARY KEY,revision INTEGER,portable_id TEXT,file_name TEXT,file_size INTEGER,file_sample_rate INTEGER,type TEXT,format TEXT,artist TEXT,name TEXT,album TEXT,genre TEXT,bpm REAL,key TEXT,year TEXT,label TEXT,comments TEXT,rating REAL,color INTEGER,length_sec INTEGER,length_ms INTEGER,time_added INTEGER,time_modified INTEGER,is_beatgrid_locked INTEGER,analysis_flags INTEGER,is_corrupt INTEGER,is_missing INTEGER,is_readonly INTEGER,third_party_type INTEGER,is_stale INTEGER,is_whitelabel INTEGER,is_karaoke INTEGER,dj_play_count INTEGER,dj_recently_played INTEGER,architectures INTEGER,stems_analyze_state INTEGER,comment_language TEXT,comment_descriptor TEXT,grouping TEXT,remixer TEXT,composer TEXT,emoji TEXT,part_of_set TEXT,track_number TEXT,video_portable_id TEXT);").unwrap();
    let library = SyncLibrary {
        tracks: vec![SyncTrack {
            id: "one".into(),
            path: audio,
            title: "Example".into(),
            artist: "Artist".into(),
            bpm: 120.,
            rating: 4,
            duration_seconds: 3.,
            sample_rate: 44100,
            cues: vec![SyncCue {
                active_loop: false,
                index: 0,
                name: "Intro".into(),
                start_seconds: 1.234,
                color: Some(0xff0000),
                end_seconds: None,
            }],
            beatgrid: vec![SyncBeat {
                seconds: 0.125,
                bpm: 120.,
                beat: 1,
            }],
            ..Default::default()
        }],
        playlists: vec![
            SyncPlaylist {
                id: "child".into(),
                name: "Crate".into(),
                parent_id: Some("parent".into()),
                track_ids: vec!["one".into()],
                ..Default::default()
            },
            SyncPlaylist {
                id: "parent".into(),
                name: "Folder".into(),
                folder: true,
                ..Default::default()
            },
        ],
    };
    (dir, db, library)
}

#[test]
fn native_markers_match_synthetic_reference_bytes() {
    let track = SyncTrack {
        color: Some(0xff0000),
        cues: vec![
            SyncCue {
                active_loop: false,
                index: 0,
                name: "Intro".into(),
                start_seconds: 1.234,
                color: Some(0xff0000),
                end_seconds: None,
            },
            SyncCue {
                active_loop: false,
                index: 1,
                name: "Loop".into(),
                start_seconds: 2.,
                end_seconds: Some(4.),
                color: None,
            },
        ],
        beatgrid: vec![SyncBeat {
            seconds: 0.125,
            bpm: 120.,
            beat: 1,
        }],
        ..Default::default()
    };
    let encoded = media::markers(&track, &SyncOptions::default()).unwrap();
    assert_eq!(encoded.len(), 470);
    let expected = "AQFDT0xPUgAAAAAEAP8AAENVRQAAAAASAAAAAATSAP8AAAAASW50cm8ATE9PUAAAAAAZAAEAAAfQAAAPoP////8AJ6rhAABMb29wAEJQTUxPQ0sAAAAAAQE=";
    let actual: Vec<u8> = encoded[2..]
        .iter()
        .copied()
        .filter(|v| *v != b'\n' && *v != 0)
        .collect();
    assert_eq!(
        STANDARD.decode(actual).unwrap(),
        STANDARD.decode(expected).unwrap()
    );
    assert_eq!(
        media::beatgrid(&track).unwrap(),
        vec![1, 0, 0, 0, 0, 1, 0x3e, 0, 0, 0, 0x42, 0xf0, 0, 0, 0]
    );
}

#[test]
fn explicit_null_clears_native_smart_metadata_while_absent_preserves_it() {
    use cuerithm_smartlists::FieldValue;
    let (_dir, path, mut library) = fixture();
    let fields = ["composer", "remixer", "grouping", "trackNumber"];
    for field in fields {
        library.tracks[0]
            .attributes
            .insert(field.into(), FieldValue::Text("Fixture".into()));
    }
    write_database(&path, &library, &Default::default()).unwrap();
    library.tracks[0].attributes.clear();
    write_database(&path, &library, &Default::default()).unwrap();
    let values = || {
        Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT composer,remixer,grouping,track_number FROM asset",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .unwrap()
    };
    assert_eq!(
        values(),
        (
            "Fixture".into(),
            "Fixture".into(),
            "Fixture".into(),
            "Fixture".into()
        )
    );
    for field in fields {
        library.tracks[0]
            .attributes
            .insert(field.into(), FieldValue::Null);
    }
    write_database(&path, &library, &Default::default()).unwrap();
    assert_eq!(
        values(),
        (String::new(), String::new(), String::new(), String::new())
    );
    let matching: i64 = Connection::open(&path).unwrap().query_row("SELECT count(*) FROM asset WHERE composer='Fixture' OR remixer='Fixture' OR grouping='Fixture'", [], |row| row.get(0)).unwrap();
    assert_eq!(matching, 0);
}

#[test]
fn binary_grids_keep_same_tempo_bar_reset_anchors() {
    let track = SyncTrack {
        beatgrid: vec![
            SyncBeat {
                seconds: 0.125,
                bpm: 120.,
                beat: 1,
            },
            SyncBeat {
                seconds: 1.625,
                bpm: 120.,
                beat: 1,
            },
        ],
        ..Default::default()
    };
    let bytes = media::beatgrid(&track).unwrap();
    assert_eq!(&bytes[..6], &[1, 0, 0, 0, 0, 2]);
    assert_eq!(u32::from_be_bytes(bytes[10..14].try_into().unwrap()), 3);
    assert_eq!(f32::from_be_bytes(bytes[14..18].try_into().unwrap()), 1.625);
}

#[test]
fn staged_wav_preserves_other_frames_and_audio() {
    let (_dir, _db, library) = fixture();
    let track = &library.tracks[0];
    let mut tag = Id3v2Tag::default();
    tag.set_title("Original title".into());
    tag.insert(Frame::Comment(lofty::id3::v2::CommentFrame::new(
        TextEncoding::UTF16,
        *b"nld",
        "Private notes".into(),
        "Bestaande tekst".into(),
    )));
    let foreign = GeneralEncapsulatedObject::new(
        TextEncoding::Latin1,
        Some("application/octet-stream".into()),
        None,
        Some("Other tool".into()),
        vec![7, 8, 9],
    );
    tag.insert(Frame::Binary(BinaryFrame::new(
        FrameId::new("GEOB").unwrap(),
        foreign.as_bytes(),
    )));
    tag.save_to_path(&track.path, WriteOptions::default())
        .unwrap();
    write_media(&track.path, track, &SyncOptions::default()).unwrap();
    write_media(&track.path, track, &SyncOptions::default()).unwrap();
    {
        use lofty::file::AudioFile;
        let mut input = std::fs::File::open(&track.path).unwrap();
        let native =
            lofty::iff::wav::WavFile::read_from(&mut input, lofty::config::ParseOptions::default())
                .unwrap();
        assert!(native.id3v2().unwrap().into_iter().any(|f|matches!(f,Frame::Comment(c) if c.language==*b"nld"&&c.encoding==TextEncoding::UTF16&&c.description=="Private notes"&&c.content=="Bestaande tekst")));
    }
    let file = Probe::open(&track.path).unwrap().read().unwrap();
    let tag = Id3v2Tag::from(file.tag(TagType::Id3v2).unwrap().clone());
    assert_eq!(tag.title().as_deref(), Some("Original title"));
    let objects: Vec<_> = tag
        .into_iter()
        .filter_map(|frame| {
            if let Frame::Binary(b) = frame {
                GeneralEncapsulatedObject::parse(&b.data, b.flags()).ok()
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        objects
            .iter()
            .filter(|o| o.descriptor.as_deref() == Some("Serato Markers2"))
            .count(),
        1
    );
    assert!(
        objects
            .iter()
            .any(|o| o.descriptor.as_deref() == Some("Other tool") && o.data == [7, 8, 9])
    );
    let bytes = std::fs::read(&track.path).unwrap();
    let data = bytes.windows(4).position(|v| v == b"data").unwrap();
    assert_eq!(&bytes[data + 8..data + 16], &[0; 8]);
}

#[test]
fn upsert_updates_revisions_hierarchy_and_skips_grid_when_requested() {
    let (_dir, db, mut library) = fixture();
    write(&db, &library, &SyncOptions::default()).unwrap();
    let conn = Connection::open(&db).unwrap();
    let (count, bpm, rating): (i64, f64, f64) = conn
        .query_row("SELECT count(*),bpm,rating FROM asset", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!((count, bpm, rating), (1, 120., 0.8));
    let first: i64 = conn
        .query_row("SELECT revision FROM master", [], |r| r.get(0))
        .unwrap();
    assert!(first > 10);
    let links:i64=conn.query_row("SELECT count(*) FROM container_asset a JOIN container c ON c.id=a.container_id JOIN container p ON p.id=c.parent_id WHERE p.name='Folder' AND c.name='Crate'",[],|r|r.get(0)).unwrap();
    assert_eq!(links, 1);
    library.tracks[0].bpm = 140.;
    library.tracks[0].title = "Changed".into();
    write(
        &db,
        &library,
        &SyncOptions {
            skip_beatgrids: true,
            ..Default::default()
        },
    )
    .unwrap();
    let (count, bpm, title): (i64, f64, String) = conn
        .query_row("SELECT count(*),bpm,name FROM asset", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!((count, bpm, title), (1, 120., "Changed".into()));
    let second: i64 = conn
        .query_row("SELECT revision FROM master", [], |r| r.get(0))
        .unwrap();
    assert!(second > first);
    let serato: i64 = conn
        .query_row("SELECT revision FROM serato", [], |r| r.get(0))
        .unwrap();
    assert_eq!(serato, second);
}

#[test]
fn invalid_hierarchy_rolls_back_database_and_never_touches_audio() {
    let (_dir, db, mut library) = fixture();
    let original = std::fs::read(&library.tracks[0].path).unwrap();
    library.playlists[0].parent_id = Some("missing".into());
    assert!(write(&db, &library, &SyncOptions::default()).is_err());
    let conn = Connection::open(db).unwrap();
    let count: i64 = conn
        .query_row("SELECT count(*) FROM asset", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(std::fs::read(&library.tracks[0].path).unwrap(), original);
}

#[test]
fn all_native_containers_preserve_existing_metadata() {
    let fixtures: &[(&str, &[u8])] = &[
        ("mp3", include_bytes!("fixtures/silence.mp3")),
        ("flac", include_bytes!("fixtures/silence.flac")),
        ("m4a", include_bytes!("fixtures/silence.m4a")),
        ("aiff", include_bytes!("fixtures/silence.aiff")),
        ("ogg", include_bytes!("fixtures/silence.ogg")),
    ];
    for (ext, bytes) in fixtures {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("silence.{ext}"));
        std::fs::write(&path, bytes).unwrap();
        let original = Probe::open(&path).unwrap().read().unwrap();
        let original_items: Vec<_> = original.primary_tag().unwrap().items().cloned().collect();
        assert!(
            original_items
                .iter()
                .any(|item| item.value().text() == Some("Foreign metadata")),
            "Fixture lacks its foreign metadata: {ext}"
        );
        if ["flac", "ogg"].contains(ext) {
            use lofty::file::AudioFile;
            let mut input = std::fs::File::open(&path).unwrap();
            let mut old = if *ext == "flac" {
                lofty::flac::FlacFile::read_from(&mut input, lofty::config::ParseOptions::default())
                    .unwrap()
                    .vorbis_comments()
                    .cloned()
                    .unwrap()
            } else {
                lofty::ogg::VorbisFile::read_from(
                    &mut input,
                    lofty::config::ParseOptions::default(),
                )
                .unwrap()
                .vorbis_comments()
                .clone()
            };
            old.insert(
                if *ext == "flac" {
                    "serato_markers_v2"
                } else {
                    "serato_markers2"
                }
                .into(),
                "stale".into(),
            );
            old.save_to_path(&path, WriteOptions::default()).unwrap();
        }
        let track = SyncTrack {
            path: path.clone(),
            beatgrid: vec![SyncBeat {
                seconds: 0.125,
                bpm: 120.,
                beat: 1,
            }],
            ..Default::default()
        };
        validate(
            &SyncLibrary {
                tracks: vec![track.clone()],
                ..Default::default()
            },
            &SyncOptions::default(),
        )
        .unwrap();
        write_media(&path, &track, &SyncOptions::default()).unwrap();
        write_media(&path, &track, &SyncOptions::default()).unwrap();
        let file = Probe::open(&path).unwrap().read().unwrap();
        let tag = file.primary_tag().unwrap();
        assert_eq!(tag.title().as_deref(), Some("Existing title"), "{ext}");
        for item in original_items {
            assert!(
                tag.items().any(|after| after == &item),
                "Original metadata item lost in {ext}: {item:?}"
            );
        }
        if ["flac", "ogg"].contains(ext) {
            let key = if *ext == "flac" {
                "serato_markers_v2"
            } else {
                "serato_markers2"
            };
            let count=tag.items().filter(|i|matches!(i.key(),lofty::tag::ItemKey::Unknown(k) if k.eq_ignore_ascii_case(key))).count();
            assert_eq!(
                count, 1,
                "Serato Vorbis keys must replace case-insensitively"
            );
        }
        match *ext {
            "ogg" => assert!(
                tag.get_string(&lofty::tag::ItemKey::Unknown("SERATO_MARKERS2".into()))
                    .is_some()
            ),
            "flac" => assert!(
                tag.get_string(&lofty::tag::ItemKey::Unknown("SERATO_MARKERS_V2".into()))
                    .is_some()
            ),
            "m4a" => {
                let tag = lofty::mp4::Ilst::from(tag.clone());
                let id = lofty::mp4::AtomIdent::Freeform {
                    mean: std::borrow::Cow::Borrowed("com.serato.dj"),
                    name: std::borrow::Cow::Borrowed("markersv2"),
                };
                assert!(tag.get(&id).is_some());
            }
            _ => {
                let tag = Id3v2Tag::from(tag.clone());
                assert!(tag.into_iter().any(|f| if let Frame::Binary(b) = f {
                    GeneralEncapsulatedObject::parse(&b.data, b.flags())
                        .is_ok_and(|o| o.descriptor.as_deref() == Some("Serato Markers2"))
                } else {
                    false
                }));
            }
        }
    }
}

#[test]
fn location_paths_strip_only_the_selected_volume_prefix() {
    assert_eq!(
        location_portable_id(
            Path::new("/Volumes/DJ Music/Tracks/a.wav"),
            Path::new("/Volumes/DJ Music")
        )
        .unwrap(),
        "Tracks/a.wav"
    );
    assert!(
        location_portable_id(
            Path::new("/Volumes/DJ Music Other/a.wav"),
            Path::new("/Volumes/DJ Music")
        )
        .is_err()
    );
    assert_eq!(
        location_portable_id(Path::new("E:\\Tracks\\a.wav"), Path::new("E:")).unwrap(),
        "Tracks/a.wav"
    );
}

#[test]
fn variable_vorbis_grid_rejects_before_any_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("variable.ogg");
    let original = include_bytes!("fixtures/silence.ogg");
    std::fs::write(&path, original).unwrap();
    let library = SyncLibrary {
        tracks: vec![SyncTrack {
            path: path.clone(),
            beatgrid: vec![
                SyncBeat {
                    seconds: 0.,
                    bpm: 120.,
                    beat: 1,
                },
                SyncBeat {
                    seconds: 1.,
                    bpm: 128.,
                    beat: 3,
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(
        validate(&library, &SyncOptions::default())
            .unwrap_err()
            .to_string()
            .contains("Variable-tempo")
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    validate(
        &library,
        &SyncOptions {
            skip_beatgrids: true,
            ..Default::default()
        },
    )
    .unwrap();
}

#[test]
fn explicit_location_database_uses_portable_paths() {
    let (dir, db, library) = fixture();
    write_location_database(&db, dir.path(), &library, &SyncOptions::default()).unwrap();
    let conn = Connection::open(&db).unwrap();
    let portable: String = conn
        .query_row("SELECT portable_id FROM asset", [], |r| r.get(0))
        .unwrap();
    assert_eq!(portable, "track.wav");
    let (routed, root) = location_for_track(dir.path(), &library.tracks[0].path).unwrap();
    assert_eq!(routed, db);
    assert_eq!(root, None);
}

#[test]
fn aggregate_refresh_links_native_location_ids_without_duplicates() {
    let (_dir, location, library) = fixture();
    write_database(&location, &library, &SyncOptions::default()).unwrap();
    let (_master_dir, master, _) = fixture();
    let conn = Connection::open(&master).unwrap();
    conn.execute_batch("ALTER TABLE asset ADD COLUMN location_id INTEGER;
        ALTER TABLE asset ADD COLUMN external_id INTEGER;
        ALTER TABLE container_asset ADD COLUMN asset_id INTEGER;
        ALTER TABLE container_asset ADD COLUMN location_container_id INTEGER;
        ALTER TABLE container_asset ADD COLUMN external_container_asset_id INTEGER;
        CREATE TABLE connection(location_id INTEGER,database_uri TEXT);
        CREATE TABLE location_container(id INTEGER PRIMARY KEY,container_id INTEGER,location_id INTEGER,external_container_id INTEGER);").unwrap();
    conn.execute(
        "INSERT INTO connection VALUES(42,?)",
        [location.to_str().unwrap()],
    )
    .unwrap();
    refresh_master(&master, &location, &location).unwrap();
    refresh_master(&master, &location, &location).unwrap();
    let (count, location_id): (i64, i64) = conn
        .query_row("SELECT count(*),location_id FROM asset", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((count, location_id), (1, 42));
    let linked:i64=conn.query_row("SELECT count(*) FROM container_asset ca JOIN location_container lc ON ca.location_container_id=lc.id JOIN container c ON c.id=lc.container_id WHERE lc.location_id=42 AND c.name='Crate'",[],|r|r.get(0)).unwrap();
    assert_eq!(linked, 1);
    let (_other_dir, other_location, other_library) = fixture();
    write_database(&other_location, &other_library, &SyncOptions::default()).unwrap();
    conn.execute(
        "INSERT INTO connection VALUES(43,?)",
        [other_location.to_str().unwrap()],
    )
    .unwrap();
    refresh_master(&master, &other_location, &other_location).unwrap();
    refresh_master(&master, &location, &location).unwrap();
    let visible_crates: i64 = conn
        .query_row(
            "SELECT count(*) FROM container WHERE name='Crate'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(visible_crates, 1);
    let links:i64=conn.query_row("SELECT count(*) FROM container_asset ca JOIN location_container lc ON lc.id=ca.location_container_id WHERE lc.location_id IN (42,43)",[],|r|r.get(0)).unwrap();
    assert_eq!(
        links, 2,
        "Refreshing one volume must preserve the other volume's crate members"
    );
}
