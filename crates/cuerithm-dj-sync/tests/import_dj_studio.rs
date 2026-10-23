#[path = "import_fixture_support/mod.rs"]
mod support;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, params_from_iter, types::Value as SqlValue};
use serde_json::{Value, json};
use std::path::Path;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/research/dj-library-import-handover/fixtures/djstudio-synthetic-records.json"
    ))
    .unwrap()
}
fn schema() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/research/dj-library-import-handover/fixtures/djstudio-sqlite-schema.json"
    ))
    .unwrap()
}
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn database(root: &Path) -> Connection {
    std::fs::create_dir_all(root.join("Database")).unwrap();
    let db = Connection::open(root.join("Database/studio.db")).unwrap();
    let manifest = schema();
    for (name, table) in manifest["tables"].as_object().unwrap() {
        let mut columns = Vec::new();
        for col in table["columns"].as_array().unwrap() {
            let mut ddl = format!(
                "{} {}",
                quoted(col["name"].as_str().unwrap()),
                col["declaredType"].as_str().unwrap()
            );
            if col["primaryKey"] == 1 {
                ddl.push_str(" PRIMARY KEY AUTOINCREMENT");
            }
            if col["notNull"] == 1 {
                ddl.push_str(" NOT NULL");
            }
            if let Some(default) = col["default"].as_str() {
                ddl.push_str(" DEFAULT ");
                ddl.push_str(default);
            }
            columns.push(ddl);
        }
        for index in table["indexes"].as_array().unwrap() {
            if index["origin"] == "u" {
                columns.push(format!(
                    "UNIQUE({})",
                    index["columns"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|n| quoted(n.as_str().unwrap()))
                        .collect::<Vec<_>>()
                        .join(",")
                ));
            }
        }
        for fk in table["foreignKeys"].as_array().unwrap().iter().rev() {
            columns.push(format!(
                "FOREIGN KEY({}) REFERENCES {}({})",
                quoted(fk["from"].as_str().unwrap()),
                quoted(fk["table"].as_str().unwrap()),
                quoted(fk["to"].as_str().unwrap())
            ));
        }
        db.execute_batch(&format!(
            "CREATE TABLE {}({});",
            quoted(name),
            columns.join(",")
        ))
        .unwrap();
        for index in table["indexes"].as_array().unwrap() {
            if index["origin"] == "c" {
                db.execute_batch(&format!(
                    "CREATE INDEX {} ON {}({});",
                    quoted(index["name"].as_str().unwrap()),
                    quoted(name),
                    index["columns"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|n| quoted(n.as_str().unwrap()))
                        .collect::<Vec<_>>()
                        .join(",")
                ))
                .unwrap();
            }
        }
    }
    for name in manifest["migrationNames"].as_array().unwrap() {
        db.execute(
            "INSERT INTO migrations(name) VALUES(?1)",
            [name.as_str().unwrap()],
        )
        .unwrap();
    }
    let fixture = fixture();
    for table in ["Tracks", "Playlists", "PlaylistTracks", "Folders"] {
        for row in fixture["sqlite"][table].as_array().unwrap() {
            let values = row.as_object().unwrap();
            let names = values.keys().map(|n| quoted(n)).collect::<Vec<_>>();
            let data = values
                .values()
                .map(|v| match v {
                    Value::Null => SqlValue::Null,
                    Value::String(v) => SqlValue::Text(v.clone()),
                    Value::Number(v) if v.is_i64() => SqlValue::Integer(v.as_i64().unwrap()),
                    Value::Number(v) => SqlValue::Real(v.as_f64().unwrap()),
                    _ => panic!("fixture scalar"),
                })
                .collect::<Vec<_>>();
            db.execute(
                &format!(
                    "INSERT INTO {}({}) VALUES({})",
                    quoted(table),
                    names.join(","),
                    vec!["?"; data.len()].join(",")
                ),
                params_from_iter(data),
            )
            .unwrap();
        }
    }
    db
}
fn write_json(root: &Path, path: &str, value: &Value) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
}
fn records(root: &Path) {
    let f = fixture();
    write_json(root, f["paths"]["audio"].as_str().unwrap(), &f["audio"]);
    write_json(
        root,
        f["paths"]["structure"].as_str().unwrap(),
        &f["structure"],
    );
}
fn capture(root: &Path) -> ImportSnapshot {
    let f = fixture();
    let request = SourceCaptureRequest::from_selected_root(
        SourceProvider::DjStudio,
        root,
        vec![
            ArtifactRequest::sqlite("Database/studio.db"),
            ArtifactRequest::stable(f["paths"]["audio"].as_str().unwrap()),
            ArtifactRequest::stable(f["paths"]["structure"].as_str().unwrap()),
        ],
    )
    .unwrap();
    capture_source(
        &request,
        root,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap()
}
#[test]
fn djstudio_keeps_sql_and_json_divergence_and_unknown_fields() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let f = fixture();
    let mut audio = f["audio"].clone();
    audio["bpm"] = json!(121.5);
    audio["tag"]["title"] = json!("Imported title");
    audio["unowned"] = json!({"keep":[1,null,"unknown"]});
    write_json(root.path(), f["paths"]["audio"].as_str().unwrap(), &audio);
    let (_, rows) = support::read(SourceProvider::DjStudio, &capture(root.path()));
    let tracks = rows.tracks();
    assert_eq!(tracks.len(), 1);
    let one = tracks[0];
    assert_eq!(one.external_id, "sql:101");
    assert!(one.bpm.is_none());
    assert!(!one.metadata.contains_key("title"));
    assert_eq!(
        one.cached_tags["title"],
        Presence::Value(json!("Imported title"))
    );
    assert_eq!(one.extensions["json_record"]["unowned"], audio["unowned"]);
    assert_eq!(one.extensions["sql_row"]["bpm"], 120.25);
    assert_eq!(
        one.extensions["native_identity"]["audio_key"],
        f["audio"]["key"]
    );
    assert_eq!(one.key.as_ref().unwrap().camelot.as_deref(), Some("8A"));
    assert_eq!(
        one.extensions["field_candidates"]["bpm"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(rows.members().len(), 2);
    assert_eq!(
        rows.collections()
            .iter()
            .filter(|c| c.extensions.contains_key("bookmark_path"))
            .count(),
        1
    );
}
#[test]
fn djstudio_shards_utf16_and_preserves_cue_arrays() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let (_, rows) = support::read(SourceProvider::DjStudio, &capture(root.path()));
    let one = rows.tracks()[0];
    assert_eq!(one.bpm.as_ref().unwrap().value, Some(120.25));
    assert_eq!(one.cues.len(), 2);
    assert!(one.cues[0].native_start_seconds.is_none());
    assert_eq!(one.cues[0].flags["placeholder"], true);
    assert_eq!(one.cues[1].native_start_seconds, Some(0.997920997920998));
    assert_eq!(one.cues[1].family, CueFamily::Hot);
    assert!(
        one.cues
            .iter()
            .all(|c| c.color_rgb.is_none() && c.canonical_start_seconds.is_none())
    );
    assert_eq!(one.grid.as_ref().unwrap().points.len(), 7);
    assert_eq!(
        one.extensions["json_record"]["cueData"]["hotCuePoints"][0]["time"],
        -1
    );
    assert!(
        rows.collections().iter().all(
            |c| c.extensions["ordering_qualification"] == "deterministic_sort_id_unverified_ui"
        )
    );
}
#[test]
fn djstudio_rejects_unsafe_keys() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let f = fixture();
    let mut audio = f["audio"].clone();
    audio["structureKey"] = json!("../../outside");
    write_json(root.path(), f["paths"]["audio"].as_str().unwrap(), &audio);
    let (_, rows) = support::read(SourceProvider::DjStudio, &capture(root.path()));
    let one = rows.tracks()[0];
    assert!(one.grid.is_none());
    assert!(
        one.issues
            .iter()
            .any(|i| i.code == "unsafe_djstudio_structure_key")
    );
    assert_eq!(
        one.metadata["title"],
        Presence::Value(json!("Synthetic fixture"))
    );
}

#[test]
fn record_names_use_javascript_utf16_and_reject_path_and_json_ambiguity() {
    use cuerithm_dj_sync::import::readers::dj_studio_records::*;
    assert_eq!(shard("file_é😀/fixed").unwrap(), "64");
    assert_eq!(
        record_path("audio.library", "file_é😀/fixed").unwrap(),
        std::path::PathBuf::from("Database/audio-library-table/64/file_é😀/fixed")
    );
    for key in [
        "",
        "../escape",
        "a/../b",
        "/absolute",
        "C:/escape",
        "a\\b",
        "a//b",
        "a\0b",
    ] {
        assert!(record_path("track-structures", key).is_err());
    }
    assert!(record_path("accounts", "safe").is_err());
    for bytes in [
        b"{\"key\":\"a\",\"key\":\"b\"}".as_slice(),
        b"{\"key\":\"a\"} trailing",
        b"{\"bpm\":NaN}",
    ] {
        assert!(decode_record(bytes, &ImportLimits::default()).is_err());
    }
}
fn capture_extra(root: &Path, extra: &[&str]) -> ImportSnapshot {
    let f = fixture();
    let mut artifacts = vec![
        ArtifactRequest::sqlite("Database/studio.db"),
        ArtifactRequest::stable(f["paths"]["audio"].as_str().unwrap()),
        ArtifactRequest::stable(f["paths"]["structure"].as_str().unwrap()),
    ];
    artifacts.extend(extra.iter().map(|p| ArtifactRequest::stable(p)));
    let request =
        SourceCaptureRequest::from_selected_root(SourceProvider::DjStudio, root, artifacts)
            .unwrap();
    capture_source(
        &request,
        root,
        &ImportLimits::default(),
        &CancellationToken::default(),
    )
    .unwrap()
}
#[test]
fn selected_alternate_grid_resolves_cues_without_regenerating_from_scalar_bpm() {
    use cuerithm_dj_sync::import::readers::dj_studio_records::record_path;
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let f = fixture();
    let mut audio = f["audio"].clone();
    let key = format!("{}/custom", audio["key"].as_str().unwrap());
    let mut structure = f["structure"].clone();
    structure["key"] = json!(key);
    for beat in structure["beatData"].as_array_mut().unwrap() {
        beat["time"] = json!(beat["time"].as_f64().unwrap() * 2.);
    }
    audio["beatDataSource"] = json!("custom");
    audio["beatGrids"]
        .as_array_mut()
        .unwrap()
        .push(json!({"beatDataSource":"custom","trackStructureKey":key,"bpm":60.125}));
    let path = record_path("track-structures", &key).unwrap();
    write_json(root.path(), path.to_str().unwrap(), &structure);
    write_json(root.path(), f["paths"]["audio"].as_str().unwrap(), &audio);
    let (_, rows) = support::read(
        SourceProvider::DjStudio,
        &capture_extra(root.path(), &[path.to_str().unwrap()]),
    );
    let one = rows.tracks()[0];
    assert_eq!(one.bpm.as_ref().unwrap().value, Some(120.25));
    assert_eq!(one.cues[1].native_start_seconds, Some(1.995841995841996));
    assert_eq!(one.extensions["selected_structure_key"], key);
    assert_eq!(
        one.extensions["json_record"]["structureKey"],
        f["audio"]["structureKey"]
    );
}
#[test]
fn cue_holes_removed_types_and_beat_lengths_remain_separate_from_time_seconds() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let f = fixture();
    let mut audio = f["audio"].clone();
    audio["cueData"] = json!({"systemCuePoints":[{"beat":-1,"time":0,"length":0,"type":5,"name":"zero"}],"hotCuePoints":[null,{"beat":2,"time":-1,"length":0,"type":3,"color":[1,2,3]}],"memCuePoints":[{"beat":-2,"time":-1,"length":0,"type":0},{"beat":0.5,"time":-1,"length":0,"type":1},{"beat":2,"time":-1,"length":2,"type":0},{"beat":1,"time":-1,"length":0,"type":999}]});
    write_json(root.path(), f["paths"]["audio"].as_str().unwrap(), &audio);
    let (_, rows) = support::read(SourceProvider::DjStudio, &capture(root.path()));
    let one = rows.tracks()[0];
    assert_eq!(one.cues.len(), 7);
    assert_eq!(one.cues[0].native_start_seconds, Some(0.));
    assert!(one.cues[1].original.is_null());
    assert!(one.cues[3].native_start_seconds.is_none());
    assert!(one.cues[4].native_start_seconds.is_none());
    assert_eq!(one.cues[5].native_end_seconds, Some(1.995841995841996));
    assert_eq!(one.cues[6].flags["removed"], true);
    assert!(one.cues.iter().all(|c| c.color_rgb.is_none()));
    assert_eq!(one.extensions["json_record"]["cueData"], audio["cueData"]);
}
#[test]
fn numeric_key_table_and_unknown_sentinels_are_preserved() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    records(root.path());
    let f = fixture();
    let map: Value = serde_json::from_str(include_str!(
        "../../../docs/research/dj-library-import-handover/fixtures/djstudio-key-map.json"
    ))
    .unwrap();
    for value in (-2..0).chain(0..24) {
        db.execute("UPDATE Tracks SET key=?1", [value]).unwrap();
        let mut audio = f["audio"].clone();
        audio["camelotKey"] = json!(value);
        write_json(root.path(), f["paths"]["audio"].as_str().unwrap(), &audio);
        let (_, rows) = support::read(SourceProvider::DjStudio, &capture(root.path()));
        let key = rows.tracks()[0].key.as_ref().unwrap();
        if value < 0 {
            assert!(key.camelot.is_none());
            assert_eq!(key.raw.as_deref(), Some(value.to_string().as_str()));
        } else {
            assert_eq!(
                key.camelot.as_deref(),
                map["keys"][value as usize]["camelot"].as_str()
            );
        }
    }
}
#[test]
fn malformed_and_trailing_records_are_accounted_without_cache_or_media_writes() {
    for suffix in [" trailing", "{"] {
        let root = tempfile::tempdir().unwrap();
        drop(database(root.path()));
        records(root.path());
        std::fs::create_dir_all(root.path().join("Cache/Database")).unwrap();
        std::fs::write(
            root.path().join("Cache/Database/audio-library-table.json"),
            b"untouched cache",
        )
        .unwrap();
        let f = fixture();
        let path = root.path().join(f["paths"]["audio"].as_str().unwrap());
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.extend(suffix.as_bytes());
        std::fs::write(&path, &bytes).unwrap();
        let (header, rows) = support::read(SourceProvider::DjStudio, &capture(root.path()));
        assert_eq!(header.completeness, Completeness::PartialWithErrors);
        assert_eq!(header.accounting.unrepresented_records, 1);
        assert_eq!(
            rows.tracks()[0].metadata["title"],
            Presence::Value(json!("Synthetic fixture"))
        );
        assert!(rows.tracks()[0].cues.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            std::fs::read(root.path().join("Cache/Database/audio-library-table.json")).unwrap(),
            b"untouched cache"
        );
    }
}
#[test]
fn future_migration_keeps_sql_raw_and_never_promotes_renderer_or_write_acceptance() {
    let root = tempfile::tempdir().unwrap();
    let db = database(root.path());
    db.execute("INSERT INTO migrations(name) VALUES('011_future.js')", [])
        .unwrap();
    drop(db);
    records(root.path());
    let (header, rows) = support::read(SourceProvider::DjStudio, &capture(root.path()));
    assert_eq!(header.completeness, Completeness::PartialWithErrors);
    assert!(header.accounting.unrepresented_records > 0);
    assert!(rows.tracks()[0].external_id.starts_with("audio:"));
    assert!(
        adapter(SourceProvider::DjStudio)
            .inspect(&capture(root.path()))
            .unwrap()
            .fields
            .values()
            .all(|f| !f.write)
    );
    assert!(rows.rows.iter().any(
        |r| matches!(r,ObservationRecord::Issue(i) if i.code=="djstudio_renderer_unverified")
    ));
}
#[test]
fn duplicate_record_paths_do_not_silently_coalesce_distinct_processed_identities() {
    use cuerithm_dj_sync::import::readers::dj_studio_records::record_path;
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let f = fixture();
    let mut audio = f["audio"].clone();
    audio["key"] = json!("file_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    let path = record_path("audio-library", audio["key"].as_str().unwrap()).unwrap();
    write_json(root.path(), path.to_str().unwrap(), &audio);
    let (_, rows) = support::read(
        SourceProvider::DjStudio,
        &capture_extra(root.path(), &[path.to_str().unwrap()]),
    );
    assert_eq!(rows.tracks().len(), 3);
    assert!(rows.tracks().iter().all(|t| {
        t.issues
            .iter()
            .any(|i| i.code == "ambiguous_djstudio_path_join")
    }));
}
#[test]
fn missing_structure_and_changed_source_keep_native_values_unresolved() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let f = fixture();
    let mut audio = f["audio"].clone();
    audio["structureKey"] = json!("file_missing/fixed");
    audio["beatGrids"][0]["trackStructureKey"] = json!("file_missing/fixed");
    write_json(root.path(), f["paths"]["audio"].as_str().unwrap(), &audio);
    let snapshot = capture(root.path());
    let (_, rows) = support::read(SourceProvider::DjStudio, &snapshot);
    assert!(rows.tracks()[0].grid.is_none());
    assert!(rows.tracks()[0].cues[1].native_start_seconds.is_none());
    audio["tag"]["comment"] = json!("later source edit");
    write_json(root.path(), f["paths"]["audio"].as_str().unwrap(), &audio);
    let mut rows = support::Records::default();
    assert!(
        adapter(SourceProvider::DjStudio)
            .read(
                &snapshot,
                &ReadOptions::default(),
                &mut rows,
                &CancellationToken::default()
            )
            .is_err()
    );
    assert!(rows.rows.is_empty());
}

#[test]
fn project_and_invalid_record_provenance_use_bounded_pages() {
    use cuerithm_dj_sync::import::readers::dj_studio_records::record_path;
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let f = fixture();
    std::fs::write(
        root.path().join(f["paths"]["audio"].as_str().unwrap()),
        b"malformed",
    )
    .unwrap();
    let invalid = record_path("audio-library", "file_bad").unwrap();
    let path = root.path().join(&invalid);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"malformed").unwrap();
    let projects = [
        record_path("projects", "project_one").unwrap(),
        record_path("projects-meta", "project_two").unwrap(),
    ];
    for project in &projects {
        write_json(
            root.path(),
            project.to_str().unwrap(),
            &json!({"opaque":"untouched"}),
        );
    }
    let extra = [
        invalid.to_str().unwrap(),
        projects[0].to_str().unwrap(),
        projects[1].to_str().unwrap(),
    ];
    let snapshot = capture_extra(root.path(), &extra);
    let mut rows = support::Records::default();
    let header = adapter(SourceProvider::DjStudio)
        .read(
            &snapshot,
            &ReadOptions {
                limits: ImportLimits {
                    page_size: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            &mut rows,
            &CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(
        header.extensions["project_reference_pages"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        header.extensions["invalid_record_reference_pages"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(header.accounting.unrepresented_records, 2);
    for project in projects {
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(root.path().join(project)).unwrap())
                .unwrap(),
            json!({"opaque":"untouched"})
        );
    }
}

#[cfg(unix)]
#[test]
fn record_symlink_escape_is_refused_before_outside_bytes_are_captured() {
    let root = tempfile::tempdir().unwrap();
    drop(database(root.path()));
    records(root.path());
    let outside = tempfile::tempdir().unwrap();
    let f = fixture();
    let original = root.path().join(f["paths"]["audio"].as_str().unwrap());
    let target = outside.path().join("record");
    std::fs::write(&target, b"outside sentinel").unwrap();
    std::fs::remove_file(&original).unwrap();
    std::os::unix::fs::symlink(&target, &original).unwrap();
    assert!(
        SourceCaptureRequest::from_selected_root(
            SourceProvider::DjStudio,
            root.path(),
            vec![
                ArtifactRequest::sqlite("Database/studio.db"),
                ArtifactRequest::stable(f["paths"]["audio"].as_str().unwrap())
            ]
        )
        .is_err()
    );
    assert_eq!(std::fs::read(target).unwrap(), b"outside sentinel");
}
