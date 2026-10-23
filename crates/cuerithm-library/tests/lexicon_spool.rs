use cuerithm_library::backup::{
    ArchiveLimits, LogicalRecord, read_lexicon_archive, read_lexicon_spooled,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

fn source(directory: &Path, count: usize, alter: impl FnOnce(&Connection)) -> std::path::PathBuf {
    let database = directory.join("source.sqlite");
    let conn = Connection::open(&database).unwrap();
    let profile: Value = serde_json::from_str(include_str!(
        "../../../docs/research/relocation-backup-handover/lexicon-v26-profile.json"
    ))
    .unwrap();
    for (name, table) in profile["tables"].as_object().unwrap() {
        let columns = table["columns"]
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
            .chain(table["foreign_keys"].as_array().unwrap().iter().map(|f| {
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
        conn.execute_batch(&format!("CREATE TABLE \"{name}\"({columns})"))
            .unwrap();
    }
    conn.execute("INSERT INTO \"Database\" VALUES(26,'synthetic-spool')", [])
        .unwrap();
    let columns = profile["tables"]["Track"]["columns"].as_array().unwrap();
    let names = columns
        .iter()
        .map(|c| format!("\"{}\"", c["name"].as_str().unwrap()))
        .collect::<Vec<_>>()
        .join(",");
    let expressions = columns
        .iter()
        .map(|c| match c["name"].as_str().unwrap() {
            "id" => "n".to_string(),
            "location" | "locationUnique" => "'D:/Music/Track-'||n||'.flac'".into(),
            "type" => "'0'".into(),
            "title" => "'Synthetic '||n".into(),
            "duration" => "200.25".into(),
            "bpm" => "120.5".into(),
            "energy" => "7".into(),
            "key" => "'8A'".into(),
            "dateAdded" | "dateModified" => "'2026-10-23T00:00:00Z'".into(),
            "data" => "'{}'".into(),
            _ => {
                if c["not_null"] == false {
                    "NULL".into()
                } else if c["declared_type"] == "TEXT" {
                    "''".into()
                } else {
                    "0".into()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    conn.execute_batch(&format!("WITH RECURSIVE seq(n) AS (SELECT 1 WHERE {count}>0 UNION ALL SELECT n+1 FROM seq WHERE n<{count}) INSERT INTO Track({names}) SELECT {expressions} FROM seq;")).unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    alter(&conn);
    drop(conn);
    let zip_path = directory.join("source.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
    zip.start_file(
        "main.db",
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )
    .unwrap();
    std::io::copy(&mut std::fs::File::open(database).unwrap(), &mut zip).unwrap();
    zip.finish().unwrap();
    zip_path
}

fn records(
    archive: &cuerithm_library::backup::SpoolArchive,
    kind: &str,
) -> BTreeMap<String, LogicalRecord> {
    let mut result = BTreeMap::new();
    archive
        .for_each(kind, &mut |record| {
            result.insert(record.id.clone(), record);
            Ok(())
        })
        .unwrap();
    result
}

#[test]
fn streamed_adapter_matches_small_reader_and_preserves_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = source(directory.path(), 3, |conn| {
        conn.execute_batch("INSERT INTO Cuepoint VALUES(1,1,'Before','1',-0.006,NULL,0,'yellow',0,'{}');INSERT INTO Cuepoint VALUES(2,1,'Loop','5',2.0,4.0,1,'blue',1,'{}');INSERT INTO Tempomarker VALUES(1,1,0.5,120.5,'{}');INSERT INTO Tempomarker VALUES(2,1,32.0,124.0,'{}');INSERT INTO Playlist VALUES(10,'Folder','','','1',NULL,NULL,NULL,0,'{}');INSERT INTO Playlist VALUES(11,'Ordered','','','2',NULL,NULL,10,2,'{}');INSERT INTO LinkTrackPlaylist VALUES(1,11,2,5);INSERT INTO LinkTrackPlaylist VALUES(2,11,1,4);INSERT INTO LinkTrackPlaylist VALUES(3,11,1,8);").unwrap();
    });
    let before = std::fs::read(&path).unwrap();
    let limits = ArchiveLimits::default();
    let small = read_lexicon_archive(&path, "UTC", &limits).unwrap();
    let streamed = read_lexicon_spooled(&path, "UTC", &limits, &|_, _| Ok(())).unwrap();
    for kind in [
        "roots",
        "locators",
        "tracks",
        "musical_revisions",
        "playlists",
        "playlist_members",
        "tag_categories",
        "tags",
        "track_tags",
        "preferences",
    ] {
        let expected = small.archive.records[kind]
            .iter()
            .cloned()
            .map(|r| (r.id.clone(), r))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(records(&streamed.archive, kind), expected, "domain {kind}");
    }
    assert_eq!(
        serde_json::to_value(&streamed.report).unwrap(),
        serde_json::to_value(&small.report).unwrap()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn streaming_crosses_many_rows_and_cancels_without_changing_source() {
    let directory = tempfile::tempdir().unwrap();
    let path = source(directory.path(), 1500, |_| {});
    let before = std::fs::read(&path).unwrap();
    let result = read_lexicon_spooled(&path, "UTC", &ArchiveLimits::default(), &|phase, count| {
        anyhow::ensure!(phase != "read_tables" || count < 513, "test cancellation");
        Ok(())
    });
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("test cancellation")
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    // A total source size above this memory hint still succeeds through a bounded spool.
    let limits = ArchiveLimits {
        memory_bytes: 1 << 20,
        ..Default::default()
    };
    let result = read_lexicon_spooled(&path, "UTC", &limits, &|_, _| Ok(())).unwrap();
    assert_eq!(result.archive.count("tracks").unwrap(), 1500);
    assert_eq!(result.archive.count("musical_revisions").unwrap(), 1500);
}

#[test]
fn profile_and_foreign_reference_corruption_fail_before_publication() {
    for attack in [
        "CREATE VIEW evil AS SELECT 1;",
        "INSERT INTO Cuepoint VALUES(1,999,'Orphan','1',0,NULL,0,'yellow',0,'{}');",
        "UPDATE \"Database\" SET version=27;",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = source(directory.path(), 1, |conn| {
            conn.execute_batch(attack).unwrap();
        });
        assert!(
            read_lexicon_spooled(&path, "UTC", &ArchiveLimits::default(), &|_, _| Ok(())).is_err(),
            "attack accepted: {attack}"
        );
    }
}

#[test]
fn empty_library_and_unknown_key_remain_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let path = source(directory.path(), 0, |_| {});
    let preview = read_lexicon_spooled(
        &path,
        "Europe/Amsterdam",
        &ArchiveLimits::default(),
        &|_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(preview.archive.count("tracks").unwrap(), 0);
    assert_eq!(
        preview
            .archive
            .record("preferences", "library")
            .unwrap()
            .unwrap()
            .value["time_zone"],
        "Europe/Amsterdam"
    );
    let directory = tempfile::tempdir().unwrap();
    let path = source(directory.path(), 1, |conn| {
        conn.execute("UPDATE Track SET key='unrecognized authored key'", [])
            .unwrap();
    });
    let preview =
        read_lexicon_spooled(&path, "UTC", &ArchiveLimits::default(), &|_, _| Ok(())).unwrap();
    let musical = records(&preview.archive, "musical_revisions");
    let key = &musical.values().next().unwrap().value["key"];
    assert_eq!(key["original"], "unrecognized authored key");
    assert!(key["pitch_class"].is_null());
    assert!(
        preview
            .report
            .warnings
            .iter()
            .any(|w| w.contains("Unrecognized source key"))
    );
}

#[test]
fn nested_source_json_is_bounded_and_settings_secrets_never_project() {
    let directory = tempfile::tempdir().unwrap();
    let path = source(directory.path(), 1, |conn| {
        conn.execute(
            "INSERT INTO Setting VALUES(1,'provider','token',?1)",
            ["secret-never-copy"],
        )
        .unwrap();
    });
    let preview =
        read_lexicon_spooled(&path, "UTC", &ArchiveLimits::default(), &|_, _| Ok(())).unwrap();
    for (kind, _) in cuerithm_library::backup::KINDS {
        preview
            .archive
            .for_each(kind, &mut |r| {
                assert!(
                    !serde_json::to_string(&r)
                        .unwrap()
                        .contains("secret-never-copy")
                );
                Ok(())
            })
            .unwrap();
    }
    assert!(
        !serde_json::to_string(&preview.report)
            .unwrap()
            .contains("secret-never-copy")
    );
    let directory = tempfile::tempdir().unwrap();
    let path = source(directory.path(), 1, |conn| {
        let mut value = json!(0);
        for _ in 0..40 {
            value = json!({"nested":value});
        }
        conn.execute("UPDATE Track SET data=?1", [value.to_string()])
            .unwrap();
    });
    assert!(read_lexicon_spooled(&path, "UTC", &ArchiveLimits::default(), &|_, _| Ok(())).is_err());
}

#[test]
#[ignore = "explicit 100,000-track streaming scale acceptance"]
fn lexicon_spool_handles_one_hundred_thousand_tracks() {
    use sha2::Digest;
    let directory = tempfile::tempdir().unwrap();
    let path = source(directory.path(), 100_000, |_| {});
    let digest = format!("{:x}", sha2::Sha256::digest(std::fs::read(&path).unwrap()));
    let started = std::time::Instant::now();
    let last = std::cell::Cell::new(started);
    let gap = std::cell::Cell::new(std::time::Duration::ZERO);
    let preview = read_lexicon_spooled(&path, "UTC", &ArchiveLimits::default(), &|_, _| {
        let now = std::time::Instant::now();
        gap.set(gap.get().max(now.duration_since(last.replace(now))));
        Ok(())
    })
    .unwrap();
    assert_eq!(preview.archive.count("tracks").unwrap(), 100_000);
    assert_eq!(preview.archive.count("musical_revisions").unwrap(), 100_000);
    eprintln!(
        "100,000-track Lexicon spool completed in {:?}; compressed source {} bytes; SHA-256 {}; largest progress callback gap {:?}",
        started.elapsed(),
        std::fs::metadata(path).unwrap().len(),
        digest,
        gap.get()
    );
}
