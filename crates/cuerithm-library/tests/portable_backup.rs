use cuerithm_library::backup::{
    ArchiveLimits, LogicalArchive, read_portable_archive, write_portable_archive,
};
#[test]
fn portable_reference_roundtrip_and_no_clobber() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("library.cuerithm");
    let archive = LogicalArchive::empty("UTC").unwrap();
    write_portable_archive(&p, &archive, &ArchiveLimits::default()).unwrap();
    let read = read_portable_archive(&p, &ArchiveLimits::default()).unwrap();
    assert_eq!(read.manifest.library_id, archive.manifest.library_id);
    assert_eq!(read.records, archive.records);
    assert!(write_portable_archive(&p, &archive, &ArchiveLimits::default()).is_err());
}
#[test]
fn duplicate_json_keys_rejected() {
    assert!(cuerithm_library::backup::parse_strict_json(br#"{"a":1,"a":2}"#, 32).is_err());
}
#[test]
fn synthetic_lexicon_preserves_signed_timing_and_namespaced_ids() {
    use rusqlite::Connection;
    use serde_json::{Value, json};
    use std::io::Write;
    let d = tempfile::tempdir().unwrap();
    let db = d.path().join("main.db");
    let conn = Connection::open(&db).unwrap();
    let p: Value = serde_json::from_str(include_str!(
        "../../../docs/research/relocation-backup-handover/lexicon-v26-profile.json"
    ))
    .unwrap();
    for (name, t) in p["tables"].as_object().unwrap() {
        let columns = t["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                format!(
                    "\"{}\" {}{}{}",
                    c["name"].as_str().unwrap(),
                    c["declared_type"].as_str().unwrap(),
                    if c["not_null"] == true {
                        " NOT NULL"
                    } else {
                        ""
                    },
                    if c["primary_key_position"] == 1 {
                        " PRIMARY KEY"
                    } else {
                        ""
                    }
                )
            })
            .chain(t["foreign_keys"].as_array().unwrap().iter().map(|f| {
                format!(
                    "FOREIGN KEY(\"{}\") REFERENCES \"{}\"(\"{}\") ON UPDATE {} ON DELETE {}",
                    f["from"].as_str().unwrap(),
                    f["target_table"].as_str().unwrap(),
                    f["to"].as_str().unwrap(),
                    f["on_update"].as_str().unwrap(),
                    f["on_delete"].as_str().unwrap()
                )
            }))
            .collect::<Vec<_>>()
            .join(",");
        conn.execute_batch(&format!("CREATE TABLE \"{name}\" ({columns})"))
            .unwrap();
    }
    conn.execute(
        "INSERT INTO \"Database\" VALUES(26,'synthetic-library')",
        [],
    )
    .unwrap();
    let columns = p["tables"]["Track"]["columns"].as_array().unwrap();
    let names = columns
        .iter()
        .map(|c| format!("\"{}\"", c["name"].as_str().unwrap()))
        .collect::<Vec<_>>()
        .join(",");
    let mut vals = Vec::new();
    for c in columns {
        let k = c["name"].as_str().unwrap();
        let v = match k {
            "id" => json!(9007199254740993i64),
            "location" => json!("D:/Music/Test.flac"),
            "type" => json!("0"),
            "title" => json!("Synthetic track"),
            "duration" => json!(200.25),
            "bpm" => json!(120.5),
            "energy" => json!(7),
            "dateAdded" | "dateModified" => json!("2026-10-23T00:00:00Z"),
            "data" => json!("{}"),
            _ => {
                if c["not_null"] == false {
                    Value::Null
                } else if c["declared_type"] == "TEXT" {
                    json!("")
                } else {
                    json!(0)
                }
            }
        };
        vals.push(match v {
            Value::Null => rusqlite::types::Value::Null,
            Value::String(s) => s.into(),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    i.into()
                } else {
                    n.as_f64().unwrap().into()
                }
            }
            _ => unreachable!(),
        });
    }
    conn.execute(
        &format!(
            "INSERT INTO Track ({names}) VALUES({})",
            vec!["?"; vals.len()].join(",")
        ),
        rusqlite::params_from_iter(vals),
    )
    .unwrap();
    conn.execute(
        "INSERT INTO Cuepoint VALUES(1,9007199254740993,'Lead','1',-0.006,NULL,0,'yellow',0,'{}')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO Tempomarker VALUES(1,9007199254740993,0.5,120.5,'{}')",
        [],
    )
    .unwrap();
    drop(conn);
    let source = d.path().join("source.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&source).unwrap());
    zip.start_file("main.db", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&std::fs::read(db).unwrap()).unwrap();
    zip.finish().unwrap();
    let result =
        cuerithm_library::backup::read_lexicon_archive(&source, "UTC", &ArchiveLimits::default())
            .unwrap();
    assert_eq!(result.archive.records["tracks"].len(), 1);
    let m = &result.archive.records["musical_revisions"][0].value;
    assert_eq!(m["cues"][0]["start_seconds"], json!(-0.006));
    assert_eq!(m["duration_seconds"], json!(200.25));
    assert_eq!(
        result.archive.records["tracks"][0].value["source_record_id"],
        json!("9007199254740993")
    );
    let restored = d.path().join("restored").join("library.sqlite");
    let lib = cuerithm_library::Library::open(&restored).unwrap();
    lib.restore_logical_archive(&result.archive).unwrap();
    assert!(
        lib.local_track_binding(&result.archive.records["tracks"][0].id)
            .unwrap()
            .is_none()
    );
    let again = lib.portable_snapshot().unwrap();
    assert_eq!(
        again.records["musical_revisions"][0].value["cues"][0]["start_seconds"],
        json!(-0.006)
    );
}
#[test]
fn native_snapshot_keeps_cue_history_and_current_selection_without_jobs() {
    use cuerithm_library::{
        AnalysisStatus, EncodedPath, Library, PathFlavor, PortableLocator, PortableTrack,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("source/library.sqlite");
    let lib = Library::open(&path).unwrap();
    lib.save_settings(Default::default()).unwrap();
    lib.import_portable_tracks(&[PortableTrack {
        id: "track".into(),
        locator: PortableLocator {
            id: "locator".into(),
            flavor: PathFlavor::Posix,
            original: EncodedPath::utf8("/Music/House/Test.flac"),
            source_root: None,
            components: None,
        },
        source_sha256: None,
        title: "Title".into(),
        artist: "Artist".into(),
        album: "".into(),
        genre: "".into(),
        metadata: BTreeMap::new(),
        key: Some("8A".into()),
        bpm: Some(120.0),
        energy: Some(7),
        duration: Some(120.0),
        archived: false,
        origin: "cuerithm".into(),
        analysis_status: AnalysisStatus::Imported,
        musical: None,
    }])
    .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    for (id, start, created) in [("earlier", 1.0, 1), ("latest", 2.0, 2)] {
        let cue = json!({"schema":"lexicon-publication-v1","grid":{"bpm":120.0,"startTime":0.5},"cues":[{"name":"Cue","startTime":start,"type":"1","position":0,"color":"yellow"}],"implementation_sha256":"a".repeat(64),"inputs_sha256":"b".repeat(64)});
        conn.execute(
            "INSERT INTO cue_revisions VALUES(?1,'track',NULL,?2,?3)",
            rusqlite::params![id, cue.to_string(), created],
        )
        .unwrap();
    }
    drop(conn);
    let snapshot = lib.portable_snapshot().unwrap();
    assert_eq!(snapshot.records["musical_revisions"].len(), 2);
    assert_eq!(
        snapshot.records["tracks"][0].value["selected_musical_revision_id"],
        json!("latest")
    );
    assert_eq!(
        snapshot.records["musical_revisions"]
            .iter()
            .find(|r| r.id == "latest")
            .unwrap()
            .value["cues"][0]["start_seconds"],
        json!(2.0)
    );
    assert!(!snapshot.records["roots"].is_empty());
    assert_eq!(
        snapshot.records["locators"][0].value["components"],
        json!(["House", "Test.flac"])
    );
    let second = lib.portable_snapshot().unwrap();
    assert_eq!(snapshot.manifest.library_id, second.manifest.library_id);
    let target = Library::open(&d.path().join("restore/library.sqlite")).unwrap();
    target.restore_logical_archive(&snapshot).unwrap();
    assert_eq!(
        target.portable_snapshot().unwrap().records["musical_revisions"].len(),
        2
    );
    assert_eq!(
        rusqlite::Connection::open(d.path().join("restore/library.sqlite"))
            .unwrap()
            .query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn writer_cancellation_and_memory_limit_never_publish() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("out.cuerithm");
    let a = LogicalArchive::empty("UTC").unwrap();
    assert!(
        cuerithm_library::backup::write_portable_archive_with_control(
            &p,
            &a,
            &ArchiveLimits::default(),
            &|_, _| anyhow::bail!("cancelled")
        )
        .is_err()
    );
    assert!(!p.exists());
    let l = ArchiveLimits {
        memory_bytes: 1,
        ..Default::default()
    };
    assert!(write_portable_archive(&p, &a, &l).is_err());
    assert!(!p.exists());
}
#[test]
fn required_feature_unknown_and_unsafe_opaque_component_fail_closed() {
    use cuerithm_library::backup::{LogicalRecord, validate_logical_archive};
    use serde_json::json;
    let mut a = LogicalArchive::empty("UTC").unwrap();
    a.manifest.required_features.push("future-required".into());
    assert!(validate_logical_archive(&a, &ArchiveLimits::default()).is_err());
    a.manifest.required_features.pop();
    a.push("roots",LogicalRecord::new("root",json!({"label":"Root","source_flavor":"posix","source_root":{"encoding":"utf8","data":"/Music"},"case_rule":"unknown"})));
    a.push("locators",LogicalRecord::new("locator",json!({"kind":"local","source_flavor":"posix","original":{"encoding":"utf8","data":"/Music/../secret"},"root_id":"root","components":[{"encoding":"base64-bytes","data":"Li4="}]})));
    assert!(validate_logical_archive(&a, &ArchiveLimits::default()).is_err());
}
#[test]
fn independent_reference_archive_is_accepted() {
    use serde_json::Value;
    use std::io::Write;
    let examples: Value = serde_json::from_str(include_str!(
        "../../../docs/research/relocation-backup-handover/portable-v1-examples.json"
    ))
    .unwrap();
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("reference.cuerithm");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
    for (name, value) in examples["minimal_archive"]["payloads"].as_object().unwrap() {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(value.as_str().unwrap().as_bytes())
            .unwrap();
    }
    writer
        .start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer
        .write_all(
            examples["minimal_archive"]["manifest"]
                .to_string()
                .as_bytes(),
        )
        .unwrap();
    writer.finish().unwrap();
    let archive = read_portable_archive(&p, &ArchiveLimits::default()).unwrap();
    assert_eq!(archive.records["preferences"][0].value["time_zone"], "UTC");
}
#[test]
fn streaming_reference_relocated_to_local_exports_the_new_locator() {
    use cuerithm_library::{
        AnalysisStatus, EncodedPath, Library, PathFlavor, PortableLocator, PortableTrack,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("source/library.sqlite");
    let lib = Library::open(&path).unwrap();
    lib.import_portable_tracks(&[PortableTrack {
        id: "track".into(),
        locator: PortableLocator {
            id: "old".into(),
            flavor: PathFlavor::Posix,
            original: EncodedPath::utf8("/Music/A.mp3"),
            source_root: None,
            components: None,
        },
        source_sha256: None,
        title: "".into(),
        artist: "".into(),
        album: "".into(),
        genre: "".into(),
        metadata: BTreeMap::new(),
        key: None,
        bpm: None,
        energy: None,
        duration: None,
        archived: false,
        origin: "cuerithm".into(),
        analysis_status: AnalysisStatus::Imported,
        musical: None,
    }])
    .unwrap();
    let mut archive = lib.portable_snapshot().unwrap();
    archive.records.get_mut("locators").unwrap()[0].value =
        json!({"kind":"streaming","service":"example","remote_id":"123"});
    let restored_path = d.path().join("restored/library.sqlite");
    let restored = Library::open(&restored_path).unwrap();
    restored.restore_logical_archive(&archive).unwrap();
    let replacement = PortableLocator {
        id: "locator:track".into(),
        flavor: PathFlavor::Posix,
        original: EncodedPath::utf8("/Music/Replacement.wav"),
        source_root: None,
        components: None,
    };
    rusqlite::Connection::open(&restored_path)
        .unwrap()
        .execute(
            "UPDATE tracks SET source_locator_json=?1 WHERE id='track'",
            [serde_json::to_string(&replacement).unwrap()],
        )
        .unwrap();
    let after = restored.portable_snapshot().unwrap();
    assert_eq!(
        after.records["tracks"][0].value["locator_id"],
        json!("locator:track")
    );
    let locator = after.records["locators"]
        .iter()
        .find(|r| r.id == "locator:track")
        .unwrap();
    assert_eq!(locator.value["kind"], json!("local"));
    assert_eq!(
        locator.value["original"]["data"],
        json!("/Music/Replacement.wav")
    );
}
#[test]
fn archive_name_traversal_and_case_alias_rejected_before_payload_parsing() {
    use std::io::Write;
    for names in [
        vec!["../manifest.json"],
        vec!["manifest.json", "MANIFEST.JSON"],
    ] {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("bad.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
        for name in names {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"{}").unwrap();
        }
        zip.finish().unwrap();
        assert!(
            cuerithm_library::backup::detect_archive_format(&p, &ArchiveLimits::default()).is_err()
        );
    }
}
#[test]
fn spooled_roundtrip_projects_live_metadata_attributes_media_and_preferences() {
    use cuerithm_library::backup::{read_portable_spooled, write_portable_spooled};
    use cuerithm_library::{
        AnalysisStatus, EncodedPath, Library, PathFlavor, PortableLocator, PortableTrack,
    };
    use serde_json::{Value, json};
    use std::collections::BTreeMap;
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("source/library.sqlite");
    let lib = Library::open(&path).unwrap();
    lib.save_settings(Default::default()).unwrap();
    let media = json!({"codec":"flac","sample_rate":44100,"channels":2,"bits_per_sample":24,"tags":{"comment":"Before edit","label":"Label","remixer":"Remixer","composer":"Composer","rating":"4","year":"2025","releasedate":"2025-02-01","tracknumber":"2/12","discnumber":"1/2","color":"blue","extra1":"First extra","extra2":"Second extra"},"properties":{"format.size":"123456","audio.bit_rate":"1000000"}});
    let metadata: BTreeMap<String, Value> = serde_json::from_value(media.clone()).unwrap();
    lib.import_portable_tracks(&[PortableTrack {
        id: "full-track".into(),
        locator: PortableLocator {
            id: "full-locator".into(),
            flavor: PathFlavor::Posix,
            original: EncodedPath::utf8("/Music/Test.flac"),
            source_root: None,
            components: None,
        },
        source_sha256: Some("a".repeat(64)),
        title: "Title".into(),
        artist: "Artist".into(),
        album: "Album".into(),
        genre: "Genre".into(),
        metadata,
        key: Some("8A".into()),
        bpm: Some(120.0),
        energy: Some(7),
        duration: Some(240.5),
        archived: false,
        origin: "cuerithm".into(),
        analysis_status: AnalysisStatus::Imported,
        musical: None,
    }])
    .unwrap();
    let native = json!({"fields":{"title":["Title"],"artist":["First Artist","Second Artist"],"album":["Album"],"genre":["Genre"],"comment":["After edit"],"label":["Label"],"remixer":["Remixer"],"composer":["Composer"]},"custom_tags":[]});
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE tracks SET native_metadata_json=?1 WHERE id='full-track'",
        [native.to_string()],
    )
    .unwrap();
    conn.execute(
        "UPDATE library_preferences SET json=?1",
        [json!({"key_mixing_mode":"fuzzy","time_zone":"Europe/Amsterdam"}).to_string()],
    )
    .unwrap();
    drop(conn);
    let source = lib
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    let before = source.record("tracks", "full-track").unwrap().unwrap();
    assert_eq!(before.value["metadata"]["comment"], json!(["After edit"]));
    assert_eq!(
        before.value["metadata"]["artist"],
        json!(["First Artist", "Second Artist"])
    );
    assert_eq!(before.value["attributes"]["rating"], json!(4));
    assert_eq!(before.value["attributes"]["release_year"], json!(2025));
    assert_eq!(before.value["attributes"]["track_number"], json!(2));
    assert_eq!(before.value["content"]["size_bytes"], json!(123456));
    assert_eq!(before.value["media"]["bitrate_kbps"], json!(1000.0));
    assert_eq!(
        source
            .record("preferences", "library")
            .unwrap()
            .unwrap()
            .value["time_zone"],
        json!("Europe/Amsterdam")
    );
    let archive = d.path().join("portable.cuerithm");
    write_portable_spooled(&archive, &source, &ArchiveLimits::default(), &|_, _| Ok(())).unwrap();
    let read = read_portable_spooled(&archive, &ArchiveLimits::default(), &|_, _| Ok(())).unwrap();
    let target = Library::open(&d.path().join("restore/library.sqlite")).unwrap();
    target
        .restore_spooled(&read, &ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    let after = target
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap()
        .record("tracks", "full-track")
        .unwrap()
        .unwrap();
    assert_eq!(before.value["metadata"], after.value["metadata"]);
    assert_eq!(before.value["attributes"], after.value["attributes"]);
    assert_eq!(before.value["content"], after.value["content"]);
    assert_eq!(before.value["media"], after.value["media"]);
}
#[test]
fn native_analysis_restore_keeps_editor_and_sync_without_prepare_and_rowid_ties() {
    use cuerithm_library::{
        AnalysisStatus, DjSyncScope, EncodedPath, Library, PathFlavor, PortableLocator,
        PortableTrack,
    };
    use serde_json::json;
    use sha2::{Digest, Sha256};
    let d = tempfile::tempdir().unwrap();
    let media_path = d.path().join("authored.wav");
    std::fs::write(&media_path, b"synthetic test file; no decoder invoked").unwrap();
    let path = d.path().join("source/library.sqlite");
    let lib = Library::open(&path).unwrap();
    let media = json!({"codec":"pcm_s16le","sample_rate":44100,"channels":2,"bits_per_sample":16,"tags":{},"properties":{}});
    lib.import_portable_tracks(&[PortableTrack {
        id: "native".into(),
        locator: PortableLocator {
            id: "native-locator".into(),
            flavor: PathFlavor::Posix,
            original: EncodedPath::utf8("/Music/Native.wav"),
            source_root: None,
            components: None,
        },
        source_sha256: Some(format!(
            "{:x}",
            Sha256::digest(std::fs::read(&media_path).unwrap())
        )),
        title: "Native".into(),
        artist: "Artist".into(),
        album: "Album".into(),
        genre: "House".into(),
        metadata: serde_json::from_value(media).unwrap(),
        key: Some("8A".into()),
        bpm: Some(120.0),
        energy: Some(7),
        duration: Some(120.0),
        archived: false,
        origin: "cuerithm".into(),
        analysis_status: AnalysisStatus::Complete,
        musical: None,
    }])
    .unwrap();
    let mut publication = json!({"grid":{"bpm":120.0,"startTime":0.5},"cues":[{"name":"Start","startTime":1.0,"type":"1","position":0,"color":"yellow"}],"implementation_sha256":"a".repeat(64),"inputs_sha256":"b".repeat(64)});
    let conn = rusqlite::Connection::open(&path).unwrap();
    for (id, start) in [("z-earlier", 1.0), ("a-current", 2.0)] {
        publication["cues"][0]["startTime"] = json!(start);
        let mut raw = publication.clone();
        raw["schema"] = json!("lexicon-publication-v1");
        conn.execute(
            "INSERT INTO cue_revisions VALUES(?1,'native',NULL,?2,100)",
            rusqlite::params![id, raw.to_string()],
        )
        .unwrap();
    }
    let analysis = json!({"publication":publication,"analysis":{"engine":"synthetic-fixture","engine_source_sha256":"a".repeat(64),"models":{},"config_sha256":"b".repeat(64),"duration_seconds":120.0,"sample_rate":44100,"key":null,"tempo":null,"cues":null,"energy":null},"complete":true,"verified_integrity":null,"source_sample_rate":44100,"elapsed_seconds":0.0});
    conn.execute(
        "UPDATE tracks SET analysis_json=?1 WHERE id='native'",
        [analysis.to_string()],
    )
    .unwrap();
    drop(conn);
    let snapshot = lib
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    assert_eq!(
        snapshot.record("tracks", "native").unwrap().unwrap().value["selected_musical_revision_id"],
        json!("a-current")
    );
    let mut historical = snapshot
        .record("musical_revisions", "z-earlier")
        .unwrap()
        .unwrap();
    historical.revision = 23;
    historical
        .extensions
        .insert("org.example.cue_history".into(), json!({"kept":true}));
    snapshot.upsert("musical_revisions", &historical).unwrap();
    let restored_path = d.path().join("restored/library.sqlite");
    let target = Library::open(&restored_path).unwrap();
    target
        .restore_spooled(&snapshot, &ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    target
        .bind_portable_track("native", &media_path, None)
        .unwrap();
    let native = target.manual_publication_snapshot("native").unwrap();
    assert_eq!(native.publication.cues[0].start_time, 2.0);
    let synced = target.dj_sync_library(DjSyncScope::Full).unwrap();
    assert_eq!(synced.tracks.len(), 1);
    assert_eq!(synced.tracks[0].cues[0].start_seconds, 2.0);
    let after = target
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap()
        .record("musical_revisions", "z-earlier")
        .unwrap()
        .unwrap();
    assert_eq!(after.revision, 23);
    assert_eq!(
        after.extensions["org.example.cue_history"],
        json!({"kept":true})
    );
    assert!(target.jobs().unwrap().is_empty());
}
#[test]
fn spooled_restore_reexports_inactive_external_locators_and_unknown_optional_payloads() {
    use cuerithm_library::{Library, backup::*};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    let examples: Value = serde_json::from_str(include_str!(
        "../../../docs/research/relocation-backup-handover/portable-v1-examples.json"
    ))
    .unwrap();
    let mut manifest = examples["minimal_archive"]["manifest"].clone();
    let payload =
        b"{\"id\":\"future\",\"revision\":1,\"value\":{\"safe\":true},\"extensions\":{}}\n";
    manifest["entries"].as_array_mut().unwrap().push(json!({"name":"data/future.jsonl","kind":"future_optional","bytes":payload.len(),"records":1,"sha256":format!("{:x}",Sha256::digest(payload)),"schema":"cuerithm-logical-v1","required":false}));
    let d = tempfile::tempdir().unwrap();
    let source = d.path().join("source.cuerithm");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&source).unwrap());
    for (name, payload) in examples["minimal_archive"]["payloads"].as_object().unwrap() {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(payload.as_str().unwrap().as_bytes()).unwrap();
    }
    zip.start_file(
        "data/future.jsonl",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(payload).unwrap();
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(manifest.to_string().as_bytes()).unwrap();
    zip.finish().unwrap();
    let limits = ArchiveLimits::default();
    let read = read_portable_spooled(&source, &limits, &|_, _| Ok(())).unwrap();
    read.insert("locators",&LogicalRecord::new("historic-locator",json!({"kind":"local","source_flavor":"posix","original":{"encoding":"utf8","data":"/Former/Remote.mp3"},"root_id":null,"components":null}))).unwrap();
    read.insert("import_sources",&LogicalRecord::new("remote",json!({"vendor":"rekordbox","source_library_id":"synthetic-remote","profile":"rekordbox-observation-v1","archive_sha256":null,"imported_at_ms":0,"source_schema":{}}))).unwrap();
    read.insert("external_links",&LogicalRecord::new("link",json!({"track_id":null,"source_id":"remote","record_kind":"track","record_id":"123","observation":{"metadata":{},"musical":{},"source_locator_id":"historic-locator"},"baseline":{"source_path":{"local":"historic-locator","remote":"historic-locator","owner":"shared","acknowledged":true}},"source_locator_id":"historic-locator","binding_status":"inactive"}))).unwrap();
    let library = Library::open(&d.path().join("restore/library.sqlite")).unwrap();
    library
        .restore_spooled(&read, &limits, &|_, _| Ok(()))
        .unwrap();
    let again = library
        .portable_snapshot_spooled(&limits, &|_, _| Ok(()))
        .unwrap();
    assert!(
        again
            .record("locators", "historic-locator")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        again
            .record("external_links", "link")
            .unwrap()
            .unwrap()
            .value["binding_status"],
        json!("inactive")
    );
    let output = d.path().join("again.cuerithm");
    write_portable_spooled(&output, &again, &limits, &|_, _| Ok(())).unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(output).unwrap()).unwrap();
    let mut preserved = vec![];
    zip.by_name("data/future.jsonl")
        .unwrap()
        .read_to_end(&mut preserved)
        .unwrap();
    assert_eq!(preserved, payload);
}
#[test]
fn spooled_settings_preserve_wire_preferences_envelopes_and_safe_provider_options() {
    use cuerithm_library::{Library, backup::*};
    use serde_json::json;
    let d = tempfile::tempdir().unwrap();
    let mut archive = LogicalArchive::empty("UTC").unwrap();
    let original = archive.records["settings"][0].id.clone();
    let settings = &mut archive.records.get_mut("settings").unwrap()[0];
    settings.revision = 9;
    settings
        .extensions
        .insert("org.example.settings".into(), json!({"kept":true}));
    settings.value["portable_options"] = json!({"mik_style_energy":true,"stability_millis":3333,"analysis":{"sample_rate":96000,"config":{"key_frame_size":4096,"key_hop_size":1024,"rhythm_frame_size":1024,"rhythm_hop_size":128,"min_bpm":70.0,"max_bpm":190.0},"structure_features":false,"source_profile_timing":true,"lexicon_timing":false,"lexicon_energy":true,"lexicon_tempo":{"min_tempo":80.0,"max_tempo":160.0},"lexicon_config":{"genre":null,"drop_at_start":"Never","min_cue_length_beats":64,"add_emergency_loop":false},"cue_template":null},"cleanup":null,"tagging":{"providers":["traxsource","beatport"],"no_match":"attention","analysis_tag_source":"catalog","write_energy":false,"strictness":0.8,"match_duration":true,"max_duration_difference":15,"match_by_id":true,"short_title":true,"merge_genres":true,"album_tagging":true,"album_tagging_ratio":0.75}});
    let preferences = &mut archive.records.get_mut("preferences").unwrap()[0];
    preferences.revision = 7;
    preferences
        .extensions
        .insert("org.example.preferences".into(), json!({"kept":true}));
    let library = Library::open(&d.path().join("restored/library.sqlite")).unwrap();
    library.restore_logical_archive(&archive).unwrap();
    let after = library
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    let historical = after.record("settings", &original).unwrap().unwrap();
    assert_eq!(historical.revision, 9);
    assert_eq!(
        historical.extensions["org.example.settings"],
        json!({"kept":true})
    );
    assert_eq!(
        historical.value["portable_options"]["analysis"]["sample_rate"],
        json!(96000)
    );
    let mut selected = None;
    after
        .for_each("settings", &mut |record| {
            if record.value["selected"] == true {
                selected = Some(record);
            }
            Ok(())
        })
        .unwrap();
    let selected = selected.unwrap();
    assert_eq!(
        selected.value["portable_options"]["mik_style_energy"],
        json!(true)
    );
    assert_eq!(
        selected.value["portable_options"]["analysis"]["lexicon_tempo"],
        json!({"min_tempo":80.0,"max_tempo":160.0})
    );
    assert_eq!(
        selected.value["portable_options"]["tagging"]["providers"],
        json!(["traxsource", "beatport"])
    );
    let preferences = after.record("preferences", "library").unwrap().unwrap();
    assert_eq!(preferences.revision, 7);
    assert_eq!(
        preferences.extensions["org.example.preferences"],
        json!({"kept":true})
    );
    assert_eq!(library.settings().unwrap().settings.stability_millis, 3333);
    assert!(!library.settings().unwrap().settings.autotag_enabled);
    #[cfg(feature = "autotag")]
    {
        let actual = library.settings().unwrap().settings.tagging.unwrap();
        assert_eq!(actual.options.platforms, vec!["traxsource", "beatport"]);
        assert_eq!(actual.no_match, cuerithm_autotag::NoMatchPolicy::Attention);
        assert_eq!(
            actual.analysis_tag_source,
            cuerithm_autotag::AnalysisTagSource::Catalog
        );
        assert!(!actual.write_energy);
        assert_eq!(actual.options.strictness, 0.8);
        assert!(actual.options.match_duration);
        assert_eq!(actual.options.max_duration_difference, 15);
        assert!(actual.options.match_by_id);
        assert!(actual.options.short_title);
        assert!(actual.options.merge_genres);
        assert!(actual.options.album_tagging);
        assert_eq!(actual.options.album_tagging_ratio, 0.75);
    }
}
#[test]
fn malformed_unselected_analysis_and_canonical_observations_fail_preview() {
    use cuerithm_library::{
        AnalysisStatus, EncodedPath, Library, PathFlavor, PortableLocator, PortableTrack, backup::*,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
    lib.import_portable_tracks(&[PortableTrack {
        id: "track".into(),
        locator: PortableLocator {
            id: "locator".into(),
            flavor: PathFlavor::Posix,
            original: EncodedPath::utf8("/Music/A.flac"),
            source_root: None,
            components: None,
        },
        source_sha256: None,
        title: "Title".into(),
        artist: "Artist".into(),
        album: "".into(),
        genre: "".into(),
        metadata: BTreeMap::new(),
        key: None,
        bpm: None,
        energy: None,
        duration: None,
        archived: false,
        origin: "cuerithm".into(),
        analysis_status: AnalysisStatus::NotAnalyzed,
        musical: None,
    }])
    .unwrap();
    let base = lib
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    let valid = json!({"publication":null,"analysis":{"engine":"fixture","engine_source_sha256":"a".repeat(64),"models":{},"config_sha256":"b".repeat(64),"duration_seconds":120.0,"sample_rate":44100,"key":null,"tempo":null,"cues":null},"complete":true,"verified_integrity":null,"source_sample_rate":44100,"elapsed_seconds":0.0});
    let mut unknown = valid.clone();
    unknown["unexpected_native_path"] = json!("/Unapproved/Location");
    let mut nested_unknown = valid.clone();
    nested_unknown["analysis"]["unexpected"] = json!(true);
    for result in [json!({"complete":true}), unknown, nested_unknown] {
        let record = LogicalRecord::new(
            "bad-analysis",
            json!({"track_id":"track","created_at_ms":0,"schema":"cuerithm-file-analysis-v1","engine_identity":{"name":"fixture","version":"1","implementation_sha256":null},"settings_revision":null,"model_identity":[],"result":result,"selected":false}),
        );
        base.upsert("analysis_revisions", &record).unwrap();
        assert!(
            base.validate(&ArchiveLimits::default(), &|_, _| Ok(()))
                .is_err()
        );
    }
    let mut inert = base
        .record("analysis_revisions", "bad-analysis")
        .unwrap()
        .unwrap();
    inert.value["schema"] = json!("future-schema");
    base.upsert("analysis_revisions", &inert).unwrap();
    base.validate(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    base.insert("import_sources",&LogicalRecord::new("remote",json!({"vendor":"rekordbox","source_library_id":"source","profile":"rekordbox-observation-v1","archive_sha256":null,"imported_at_ms":0,"source_schema":{}}))).unwrap();
    for (field, value) in [
        ("title", json!("scalar")),
        ("bpm", json!(0)),
        ("rating", json!(6)),
        ("release_year", json!(0)),
        ("tags", json!([7])),
    ] {
        let link = LogicalRecord::new(
            "bad-link",
            json!({"track_id":null,"source_id":"remote","record_kind":"track","record_id":"1","observation":null,"baseline":{field:{"local":value,"remote":null,"owner":"local","acknowledged":true}},"source_locator_id":null,"binding_status":"inactive"}),
        );
        base.upsert("external_links", &link).unwrap();
        assert!(
            base.validate(&ArchiveLimits::default(), &|_, _| Ok(()))
                .is_err()
        );
    }
    let mut observation = base.record("external_links", "bad-link").unwrap().unwrap();
    observation.value["baseline"] = json!({});
    observation.value["observation"] =
        json!({"metadata":{"title":5},"musical":{},"source_locator_id":null});
    base.upsert("external_links", &observation).unwrap();
    assert!(
        base.validate(&ArchiveLimits::default(), &|_, _| Ok(()))
            .is_err()
    );
    assert_eq!(lib.counts().unwrap().tracks, 1);
}
#[test]
fn restored_large_extensions_are_paged_by_retained_envelope_bytes() {
    use cuerithm_library::{
        AnalysisStatus, EncodedPath, Library, PathFlavor, PortableLocator, PortableTrack,
    };
    use serde_json::json;
    use std::{cell::Cell, collections::BTreeMap};
    let d = tempfile::tempdir().unwrap();
    let source = Library::open(&d.path().join("source/library.sqlite")).unwrap();
    let tracks = (0..16)
        .map(|i| PortableTrack {
            id: format!("track-{i:02}"),
            locator: PortableLocator {
                id: format!("locator-{i:02}"),
                flavor: PathFlavor::Posix,
                original: EncodedPath::utf8(format!("/Music/Track{i:02}.flac")),
                source_root: None,
                components: None,
            },
            source_sha256: None,
            title: format!("Track{i:02}"),
            artist: "".into(),
            album: "".into(),
            genre: "".into(),
            metadata: BTreeMap::new(),
            key: None,
            bpm: None,
            energy: None,
            duration: None,
            archived: false,
            origin: "cuerithm".into(),
            analysis_status: AnalysisStatus::NotAnalyzed,
            musical: None,
        })
        .collect::<Vec<_>>();
    source.import_portable_tracks(&tracks).unwrap();
    let snapshot = source
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    for i in 0..16 {
        let mut record = snapshot
            .record("tracks", &format!("track-{i:02}"))
            .unwrap()
            .unwrap();
        record
            .extensions
            .insert("org.example.large".into(), json!("x".repeat(4 << 20)));
        snapshot.upsert("tracks", &record).unwrap();
    }
    let restored = Library::open(&d.path().join("restored/library.sqlite")).unwrap();
    restored
        .restore_spooled(&snapshot, &ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    drop(snapshot);
    let pages = Cell::new(0usize);
    let small = ArchiveLimits {
        memory_bytes: 16 << 20,
        ..Default::default()
    };
    let after = restored
        .portable_snapshot_spooled(&small, &|phase, _| {
            if phase == "project" {
                pages.set(pages.get() + 1);
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(after.count("tracks").unwrap(), 16);
    assert!(
        pages.get() >= 16,
        "large retained envelopes must not share the128-track row-count page"
    );
    for i in 0..16 {
        assert_eq!(
            after
                .record("tracks", &format!("track-{i:02}"))
                .unwrap()
                .unwrap()
                .extensions["org.example.large"]
                .as_str()
                .unwrap()
                .len(),
            4 << 20
        );
    }
}
#[test]
fn native_observation_export_preserves_publication_and_imported_cue_baselines() {
    use cuerithm_library::Library;
    use serde_json::json;
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("source/library.sqlite");
    let library = Library::open(&path).unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("INSERT INTO native_targets(id,provider,database_id,device_id,identity_json,schema_fingerprint,observation_hash,issues_json,revision,created,updated) VALUES('target','rekordbox','synthetic-db','device','{}',?1,?2,'[]',1,0,0)",rusqlite::params!["a".repeat(64),"b".repeat(64)]).unwrap();
    let baseline = json!({"cues":{"local":[{"name":"Local","startTime":2.0,"type":"1","position":0,"color":"yellow","data":{"rekordbox":{"originalNum":-1}}}],"remote":[{"id":"remote-cue","uuid":null,"content_uuid":null,"kind":1,"slot":2,"memory":false,"start_seconds":3.0,"end_seconds":null,"active_loop":false,"comment":"Remote","color":255,"color_table_index":null,"beat_loop_size":null,"raw_hash":"c".repeat(64)}],"owner":"shared","acknowledged":true},"rating":{"local":"4","remote":5,"owner":"shared","acknowledged":true},"release_year":{"local":"2024","remote":null,"owner":"local","acknowledged":false}});
    conn.execute("INSERT INTO native_track_links(target_id,content_id,local_track_id,identity_json,observation_json,observation_hash,baseline_json,status,updated) VALUES('target','123',NULL,'{}',NULL,NULL,?1,'unbound',0)",[baseline.to_string()]).unwrap();
    drop(conn);
    let archive = library
        .portable_snapshot_spooled(&ArchiveLimits::default(), &|_, _| Ok(()))
        .unwrap();
    let mut links = vec![];
    archive
        .for_each("external_links", &mut |r| {
            links.push(r);
            Ok(())
        })
        .unwrap();
    assert_eq!(links.len(), 1);
    let baseline = &links[0].value["baseline"];
    assert_eq!(baseline["cues"]["local"][0]["start_seconds"], json!(2.0));
    assert_eq!(baseline["cues"]["local"][0]["memory"], json!(true));
    assert_eq!(baseline["cues"]["remote"][0]["start_seconds"], json!(3.0));
    assert_eq!(baseline["rating"]["local"], json!(4));
    assert_eq!(baseline["release_year"]["local"], json!(2024));
    assert_eq!(links[0].value["binding_status"], json!("inactive"));
}
