//! Read-only Explorer and imported-record surfaces. No IPC, migrations or cache edits.
use super::dj_studio_records as records;
use super::*;
use rusqlite::{Connection, OpenFlags, OptionalExtension, limits::Limit, params, types::ValueRef};
pub struct DjStudioReader;
const MANIFEST: &str = include_str!(
    "../../../../../docs/research/dj-library-import-handover/fixtures/djstudio-sqlite-schema.json"
);
const TABLES: &[&str] = &[
    "Tracks",
    "Playlists",
    "PlaylistTracks",
    "Folders",
    "Libraries",
    "LibraryLocations",
    "TrackLibraryMap",
    "migrations",
];
fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
struct SqlProfile {
    db: Option<Connection>,
    qualified: bool,
    facts: Value,
    fingerprint: String,
}
fn sql_profile(snapshot: &ImportSnapshot, options: &ReadOptions) -> Result<SqlProfile> {
    let selected = snapshot.artifacts.iter().find(|a| {
        snapshot
            .original_artifact_path(&a.id)
            .is_ok_and(|p| p == snapshot.source_root().join("Database/studio.db"))
    });
    let Some(selected) = selected else {
        return Ok(SqlProfile {
            db: None,
            qualified: false,
            facts: json!({"state":"absent"}),
            fingerprint: digest(b"djstudio-json-records-v1"),
        });
    };
    ensure!(
        matches!(selected.kind, ArtifactKind::Sqlite),
        "DJ.Studio SQL requires coherent SQLite capture"
    );
    let db = Connection::open_with_flags(
        snapshot.artifact(&selected.id)?,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    db.pragma_update(None, "query_only", true)?;
    db.set_limit(
        Limit::SQLITE_LIMIT_LENGTH,
        options
            .limits
            .blob_bytes
            .max(options.limits.text_bytes)
            .min(i32::MAX as u64) as i32,
    )?;
    let mut tables = serde_json::Map::new();
    let mut ordinary = Vec::new();
    for table in TABLES {
        let object: Option<(String, String)> = db
            .query_row(
                "SELECT type,sql FROM sqlite_master WHERE name=?1",
                [table],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((kind, sql)) = object else {
            continue;
        };
        if kind != "table" || sql.to_ascii_uppercase().contains("VIRTUAL TABLE") {
            tables.insert((*table).into(), json!({"object_type":kind}));
            continue;
        }
        ordinary.push(*table);
        let columns=db.prepare(&format!("PRAGMA table_info({})",quote(table)))?.query_map([],|r|Ok(json!({"cid":r.get::<_,i64>(0)?,"name":r.get::<_,String>(1)?,"declaredType":r.get::<_,String>(2)?,"notNull":r.get::<_,i64>(3)?,"default":r.get::<_,Option<String>>(4)?,"primaryKey":r.get::<_,i64>(5)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let foreign=db.prepare(&format!("PRAGMA foreign_key_list({})",quote(table)))?.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"seq":r.get::<_,i64>(1)?,"table":r.get::<_,String>(2)?,"from":r.get::<_,String>(3)?,"to":r.get::<_,String>(4)?,"onUpdate":r.get::<_,String>(5)?,"onDelete":r.get::<_,String>(6)?,"match":r.get::<_,String>(7)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let mut indexes=db.prepare(&format!("PRAGMA index_list({})",quote(table)))?.query_map([],|r|Ok(json!({"name":r.get::<_,String>(1)?,"unique":r.get::<_,bool>(2)?,"origin":r.get::<_,String>(3)?,"partial":r.get::<_,bool>(4)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
        for index in &mut indexes {
            let name = index["name"].as_str().context("Invalid native index")?;
            let columns = db
                .prepare(&format!("PRAGMA index_info({})", quote(name)))?
                .query_map([], |r| r.get::<_, Option<String>>(2))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            index["columns"] = json!(columns);
        }
        indexes.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        tables.insert(
            (*table).into(),
            json!({"columns":columns,"foreignKeys":foreign,"indexes":indexes}),
        );
    }
    let migrations = if ordinary.contains(&"migrations") {
        db.prepare("SELECT name FROM migrations ORDER BY id LIMIT 1001")?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    } else {
        vec![]
    };
    let user: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let app: i64 = db.pragma_query_value(None, "application_id", |r| r.get(0))?;
    let mut expected: Value = serde_json::from_str(MANIFEST)?;
    for value in expected["tables"]
        .as_object_mut()
        .context("Invalid schema manifest")?
        .values_mut()
    {
        value["indexes"]
            .as_array_mut()
            .context("Invalid manifest indexes")?
            .sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    }
    let known:Vec<String>=db.prepare("SELECT name FROM sqlite_master WHERE type IN('table','view') AND name NOT GLOB 'sqlite_*' ORDER BY name")?.query_map([],|r|r.get(0))?.collect::<rusqlite::Result<_>>()?;
    let mut expected_names = TABLES.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    expected_names.sort();
    let qualified = Value::Object(tables.clone()) == expected["tables"]
        && json!(migrations) == expected["migrationNames"]
        && user == 0
        && app == 0
        && known == expected_names;
    let facts = json!({"tables":tables,"migrationNames":migrations,"user_version":user,"application_id":app,"ordinary":ordinary,"qualified":qualified});
    let fingerprint = digest(&serde_json::to_vec(&facts)?);
    Ok(SqlProfile {
        db: Some(db),
        qualified,
        facts,
        fingerprint,
    })
}
fn profile_for(sql: &SqlProfile) -> SourceProfile {
    let mut p = profile(
        SourceProvider::DjStudio,
        "djstudio-explorer-imported-json-v1",
        &[
            "descriptive_tags",
            "cached_tags",
            "key",
            "bpm",
            "beatgrid",
            "hot_cues",
            "memory_cues",
            "markers",
            "loops",
            "ratings",
            "collections",
        ],
    );
    p.schema_fingerprint = sql.fingerprint.clone();
    p.origin.insert("native_renderer".into(),"Native/renderer last-writer identities unverified; static record shape only; all writes and canonical timing unqualified".into());
    p.issues.push(ImportIssue{code:"djstudio_renderer_unverified".into(),field:Some("profile".into()),message:"DJ.Studio native/renderer acceptance is unverified; imported values remain source observations".into(),..Default::default()});
    p
}
fn scalar(value: &Value) -> Presence<Value> {
    if value.is_null() {
        Presence::Null
    } else if value.as_str() == Some("") {
        Presence::Empty(value.clone())
    } else {
        Presence::Value(value.clone())
    }
}
fn normalized_path(raw: &str, options: &ReadOptions) -> Result<Option<String>> {
    let windows = raw.as_bytes().get(1) == Some(&b':')
        || raw.starts_with("\\\\")
        || options
            .origin
            .as_ref()
            .is_some_and(|o| o.origin_os == "windows");
    let value = super::super::paths::normalized(raw, windows)?;
    Ok(value.map(|v| if windows { v.to_ascii_lowercase() } else { v }))
}
fn get(aux: &Connection, kind: &str, id: &str) -> Result<Option<Value>> {
    aux.query_row(
        "SELECT json FROM rows WHERE kind=?1 AND id=?2",
        params![kind, id],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|s| serde_json::from_str(&s).map_err(Into::into))
    .transpose()
}
fn put(
    aux: &Connection,
    kind: &str,
    id: &str,
    path: Option<&str>,
    raw: &Value,
    blob: Option<&BlobRef>,
) -> Result<()> {
    aux.execute(
        "INSERT INTO rows VALUES(?1,?2,?3,?4,?5)",
        params![
            kind,
            id,
            path,
            serde_json::to_string(raw)?,
            blob.map(serde_json::to_string).transpose()?
        ],
    )?;
    Ok(())
}
struct RefPages {
    values: Vec<Value>,
    refs: Vec<BlobRef>,
    bytes: usize,
}
impl RefPages {
    fn new() -> Self {
        Self {
            values: vec![],
            refs: vec![],
            bytes: 0,
        }
    }
    fn flush(&mut self, sink: &mut dyn ObservationSink) -> Result<()> {
        if !self.values.is_empty() {
            self.refs
                .push(sink.put_blob(&serde_json::to_vec(&self.values)?, Some("application/json"))?);
            self.values.clear();
            self.bytes = 0;
        }
        Ok(())
    }
    fn add(
        &mut self,
        value: Value,
        options: &ReadOptions,
        sink: &mut dyn ObservationSink,
    ) -> Result<()> {
        let size = serde_json::to_vec(&value)?.len();
        ensure!(
            size as u64 <= options.limits.text_bytes,
            "DJ.Studio reference size limit"
        );
        if self.values.len() >= options.limits.page_size.max(1)
            || self.bytes + size > options.limits.text_bytes as usize
        {
            self.flush(sink)?;
        }
        self.bytes += size;
        self.values.push(value);
        Ok(())
    }
}
fn sql_rows(
    sql: &SqlProfile,
    aux: &Connection,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<(Vec<BlobRef>, u64)> {
    let Some(db) = &sql.db else {
        return Ok((vec![], 0));
    };
    let mut pages = Vec::new();
    let mut page = Vec::new();
    let mut page_bytes = 0;
    let mut count = 0;
    let mut unrepresented = 0;
    for table in TABLES {
        if !sql.facts["ordinary"]
            .as_array()
            .is_some_and(|a| a.iter().any(|s| s.as_str() == Some(*table)))
        {
            continue;
        }
        let mut statement = db.prepare(&format!(
            "SELECT rowid,* FROM {} ORDER BY rowid",
            quote(table)
        ))?;
        let names = statement
            .column_names()
            .into_iter()
            .skip(1)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            count += 1;
            ensure!(count <= 2_000_000, "DJ.Studio SQL row limit");
            let rowid: i64 = row.get(0)?;
            let mut raw = serde_json::Map::new();
            for (index, name) in names.iter().enumerate() {
                let value = match row.get_ref(index + 1)? {
                    ValueRef::Blob(bytes) => {
                        ensure!(
                            bytes.len() as u64 <= options.limits.blob_bytes,
                            "DJ.Studio blob limit"
                        );
                        json!({"blob":sink.put_blob(bytes,None)?})
                    }
                    value => {
                        if let ValueRef::Text(bytes) = value {
                            ensure!(
                                bytes.len() as u64 <= options.limits.text_bytes,
                                "DJ.Studio text limit"
                            );
                        }
                        super::mixxx_schema::scalar(value)?
                    }
                };
                raw.insert(name.clone(), value);
            }
            let raw = Value::Object(raw);
            let id = raw["id"]
                .as_i64()
                .map(|n| n.to_string())
                .unwrap_or_else(|| format!("row:{rowid}"));
            let size = serde_json::to_vec(&raw)?.len();
            ensure!(
                size as u64 <= options.limits.text_bytes,
                "DJ.Studio row text limit"
            );
            if !page.is_empty()
                && (page.len() >= options.limits.page_size.max(1)
                    || page_bytes + size > options.limits.text_bytes as usize)
            {
                pages.push(sink.put_blob(&serde_json::to_vec(&page)?, Some("application/json"))?);
                page.clear();
                page_bytes = 0;
            }
            page_bytes += size;
            page.push(json!({"table":table,"id":id,"raw":raw}));
            if sql.qualified {
                let path = if *table == "Tracks" {
                    raw["filePath"]
                        .as_str()
                        .map(|p| normalized_path(p, options))
                        .transpose()?
                        .flatten()
                } else {
                    None
                };
                put(aux, table, &id, path.as_deref(), &raw, None)?;
            } else {
                unrepresented += 1;
                sink.push(ObservationRecord::Issue(ImportIssue{code:"unsupported_djstudio_sql_profile".into(),record_id:Some(format!("sql:{table}:{id}")),field:Some("source".into()),message:"Unqualified SQL row retained as raw provenance; no migrations or normalization applied".into()}))?;
            }
        }
    }
    if !page.is_empty() {
        pages.push(sink.put_blob(&serde_json::to_vec(&page)?, Some("application/json"))?);
    }
    Ok((pages, unrepresented))
}

fn audio_paths(value: &Value) -> Vec<&str> {
    let mut paths = Vec::new();
    for name in ["fileLocationPath", "lastKnownFilePath"] {
        if let Some(path) = value["externalRec"][name]
            .as_str()
            .filter(|s| !s.is_empty())
            && !paths.contains(&path)
        {
            paths.push(path);
        }
    }
    paths
}
fn json_rows(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
    aux: &Connection,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<(Vec<BlobRef>, u64)> {
    let mut projects = RefPages::new();
    let mut errors = 0;
    for artifact in &snapshot.artifacts {
        cancel.check()?;
        let original = snapshot.original_artifact_path(&artifact.id)?;
        let relative = original.strip_prefix(snapshot.source_root())?;
        let mut parts = relative.components();
        if parts.next().map(|c| c.as_os_str()) != Some(std::ffi::OsStr::new("Database")) {
            continue;
        }
        let Some(store) = parts.next().and_then(|c| c.as_os_str().to_str()) else {
            continue;
        };
        let binding = match store {
            "audio-library-table" => "audio-library",
            "track-structures-table" => "track-structures",
            "projects-table"
            | "projects-meta-table"
            | "mix-data-table"
            | "automation-data-table" => {
                projects.add(
                    json!({"artifact":artifact.id,"sha256":artifact.sha256,"bytes":artifact.bytes}),
                    options,
                    sink,
                )?;
                continue;
            }
            _ => continue,
        };
        ensure!(
            matches!(artifact.kind, ArtifactKind::StableFile),
            "DJ.Studio JSON requires stable file capture"
        );
        ensure!(
            artifact.bytes <= options.limits.text_bytes,
            "DJ.Studio JSON byte limit"
        );
        let bytes = fs::read(snapshot.artifact(&artifact.id)?)?;
        let blob = sink.put_blob(&bytes, Some("application/json"))?;
        let parsed = records::decode_record(&bytes, &options.limits).and_then(|value| {
            let key = value["key"]
                .as_str()
                .context("Missing DJ.Studio record key")?;
            let expected = records::record_path(binding, key)?;
            ensure!(
                relative == expected,
                "DJ.Studio record key/path/shard mismatch"
            );
            Ok(value)
        });
        match parsed {
            Ok(value) => {
                let key = value["key"].as_str().context("Missing parsed key")?;
                let kind = if binding == "audio-library" {
                    "audio"
                } else {
                    "structure"
                };
                put(aux, kind, key, None, &value, Some(&blob))?;
                if kind == "audio" {
                    for path in audio_paths(&value) {
                        if let Some(path) = normalized_path(path, options)? {
                            aux.execute(
                                "INSERT OR IGNORE INTO audio_paths VALUES(?1,?2)",
                                params![key, path],
                            )?;
                        }
                    }
                }
            }
            Err(_) => {
                errors += 1;
                sink.push(ObservationRecord::Issue(ImportIssue{code:"unsupported_djstudio_record".into(),record_id:Some(format!("record:{}",artifact.id)),field:Some("source".into()),message:"Malformed, unsafe or unrecognized record retained unchanged as raw JSON provenance".into()}))?;
                aux.execute(
                    "INSERT INTO invalid_records VALUES(?1,?2)",
                    params![artifact.id, serde_json::to_string(&blob)?],
                )?;
            }
        }
    }
    projects.flush(sink)?;
    Ok((projects.refs, errors))
}
fn joined_audio(aux: &Connection, path: Option<&str>) -> Result<Option<String>> {
    let Some(path) = path else { return Ok(None) };
    let sql_count: i64 = aux.query_row(
        "SELECT count(*) FROM rows WHERE kind='Tracks' AND path=?1",
        [path],
        |r| r.get(0),
    )?;
    if sql_count != 1 {
        return Ok(None);
    }
    let candidates = aux
        .prepare("SELECT DISTINCT id FROM audio_paths WHERE path=?1 ORDER BY id LIMIT 2")?
        .query_map([path], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if candidates.len() != 1 {
        return Ok(None);
    }
    let key = &candidates[0];
    let matches:i64=aux.query_row("SELECT count(DISTINCT s.id) FROM rows s JOIN audio_paths p ON p.path=s.path WHERE s.kind='Tracks' AND p.id=?1",[key],|r|r.get(0))?;
    Ok((matches == 1).then(|| key.clone()))
}
fn candidate(
    track: &mut TrackObservation,
    field: &str,
    sql: Option<&Value>,
    json: Option<&Value>,
) -> Option<Value> {
    match (sql, json) {
        (Some(a), Some(b)) if a != b => {
            issue(track, "djstudio_surface_disagreement", field);
            let value = track
                .extensions
                .entry("field_candidates".into())
                .or_insert_with(|| json!({}));
            value[field] = json!([
                candidate_record(field, "sql", a),
                candidate_record(field, "json", b)
            ]);
            None
        }
        (Some(a), _) => Some(a.clone()),
        (_, Some(b)) => Some(b.clone()),
        _ => None,
    }
}
fn candidate_record(field: &str, surface: &str, value: &Value) -> Value {
    let normalized = match field {
        "key" if value.is_null() || value.is_number() || value.is_string() => Some((
            "key".to_owned(),
            json!(MusicalKey {
                presence: if value.is_null() {
                    FieldPresence::Null
                } else {
                    FieldPresence::Value
                },
                raw: if value.is_null() {
                    None
                } else {
                    Some(value.to_string())
                },
                camelot: records::numeric_key(value),
                provenance: Some("DJ.Studio numeric key table".into()),
                ..Default::default()
            }),
        )),
        "bpm" if value.is_null() || value.as_f64().is_some_and(|n| n > 0.) => Some((
            "bpm".into(),
            json!(ScalarBpm {
                presence: if value.is_null() {
                    FieldPresence::Null
                } else {
                    FieldPresence::Value
                },
                value: value.as_f64(),
                provenance: Some("DJ.Studio native source".into()),
                ..Default::default()
            }),
        )),
        "rating" => Some((
            "metadata.rating".into(),
            if value.is_null() {
                json!(Presence::<Value>::Null)
            } else {
                json!(Presence::Value(
                    json!({"raw":value,"scale":"unqualified_djstudio"})
                ))
            },
        )),
        "duration" if value.as_f64().is_some_and(|v| v >= 0.) => {
            Some(("metadata.duration".into(), json!(Presence::Value(value))))
        }
        "title" | "artist" | "album" | "genre" | "label" | "isrc" | "release_date"
        | "date_added"
            if value.is_string() || value.is_null() =>
        {
            Some((format!("metadata.{field}"), json!(scalar(value))))
        }
        _ => None,
    };
    let mut record = json!({"id":format!("{surface}:{field}"),"surface":surface,"value":value});
    if let Some((field, value)) = normalized {
        record["normalized_field"] = json!(field);
        record["normalized_value"] = value;
    }
    record
}
fn selected_structure(
    aux: &Connection,
    audio: &Value,
    track: &mut TrackObservation,
) -> Result<Option<(String, Value)>> {
    let mut refs = Vec::new();
    if let Some(key) = audio["structureKey"].as_str() {
        refs.push(key.to_string());
    }
    if let Some(grids) = audio["beatGrids"].as_array() {
        for grid in grids {
            if let Some(key) = grid["trackStructureKey"].as_str() {
                refs.push(key.to_string());
            }
        }
    }
    for key in &refs {
        if records::validate_key(key).is_err() {
            issue(track, "unsafe_djstudio_structure_key", "beatgrid");
            return Ok(None);
        }
    }
    let mut retained = Vec::new();
    for key in &refs {
        let blob: Option<String> = aux
            .query_row(
                "SELECT blob FROM rows WHERE kind='structure' AND id=?1",
                [key],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        retained.push(
            json!({"key":key,"raw":blob.map(|s|serde_json::from_str::<Value>(&s)).transpose()?}),
        );
    }
    track
        .extensions
        .insert("structure_references".into(), json!(retained));
    let selected = audio["beatDataSource"].as_str();
    let mut matching = Vec::new();
    if let Some(selected) = selected
        && let Some(grids) = audio["beatGrids"].as_array()
    {
        for grid in grids {
            if grid["beatDataSource"].as_str() == Some(selected)
                && let Some(key) = grid["trackStructureKey"].as_str()
            {
                matching.push(key.to_owned());
            }
        }
    }
    matching.sort();
    matching.dedup();
    let key = if matching.len() == 1 {
        matching.pop()
    } else if matching.len() > 1 {
        issue(track, "ambiguous_djstudio_active_structure", "beatgrid");
        None
    } else if selected.is_some() && audio["beatGrids"].as_array().is_some_and(|g| !g.is_empty()) {
        issue(track, "missing_djstudio_active_structure", "beatgrid");
        None
    } else {
        audio["structureKey"].as_str().map(str::to_owned)
    };
    let Some(key) = key else { return Ok(None) };
    let Some(value) = get(aux, "structure", &key)? else {
        issue(track, "missing_djstudio_structure", "beatgrid");
        return Ok(None);
    };
    track
        .extensions
        .insert("selected_structure_key".into(), json!(key));
    Ok(Some((key, value)))
}
fn beat_time(structure: Option<&Value>, beat: f64) -> Option<f64> {
    if !beat.is_finite() || beat < 0. || beat.fract() != 0. || beat > usize::MAX as f64 {
        return None;
    }
    structure?["beatData"].as_array()?.get(beat as usize)?["time"]
        .as_f64()
        .filter(|v| v.is_finite())
}
fn performance(aux: &Connection, audio: &Value, track: &mut TrackObservation) -> Result<()> {
    let selected = selected_structure(aux, audio, track)?;
    let structure = selected.as_ref().map(|(_, v)| v);
    if let Some((key, value)) = &selected
        && let Some(beats) = value["beatData"].as_array()
    {
        let mut points = Vec::with_capacity(beats.len());
        let mut valid = true;
        for beat in beats {
            if let Some(time) = beat["time"].as_f64().filter(|v| v.is_finite()) {
                points.push(GridPoint {
                    native_seconds: time,
                    beat_index: beat["nr"].as_i64(),
                    original: beat.clone(),
                    ..Default::default()
                });
            } else {
                valid = false;
                break;
            }
        }
        if valid {
            track.grid = Some(GridObservation {
                representation: "djstudio_explicit_structure".into(),
                original_time_base: Some("seconds".into()),
                native_first_beat_seconds: points.first().map(|p| p.native_seconds),
                clock_status: ClockStatus::UnresolvedNative,
                points,
                extensions: BTreeMap::from([
                    ("structure_key".into(), json!(key)),
                    ("bpm_line".into(), value["bpmLine"].clone()),
                    ("alternate_grids".into(), audio["beatGrids"].clone()),
                ]),
                ..Default::default()
            });
        } else {
            issue(track, "unsupported_djstudio_beats", "beatgrid");
        }
    }
    let Some(groups) = audio["cueData"].as_object() else {
        track
            .extensions
            .insert("hotcue_occupancy".into(), json!("unknown"));
        return Ok(());
    };
    for (group, family) in [
        ("systemCuePoints", CueFamily::Marker),
        ("hotCuePoints", CueFamily::Hot),
        ("memCuePoints", CueFamily::Memory),
    ] {
        let Some(cues) = groups.get(group).and_then(Value::as_array) else {
            if group == "hotCuePoints" {
                track
                    .extensions
                    .insert("hotcue_occupancy".into(), json!("unknown"));
            }
            continue;
        };
        for (index, cue) in cues.iter().enumerate() {
            let beat = cue["beat"].as_f64().filter(|v| *v != -1.);
            let time = cue["time"].as_f64().filter(|v| *v != -1.);
            let mapped = beat.and_then(|b| beat_time(structure, b));
            let start = match (beat, time, mapped) {
                (None, Some(time), _) => Some(time),
                (Some(_), None, Some(mapped)) => Some(mapped),
                (Some(_), Some(time), Some(mapped)) if time == mapped => Some(mapped),
                _ => None,
            };
            let placeholder = cue.is_null()
                || beat.is_none()
                    && time.is_none()
                    && cue["cueKey"].as_str().is_none_or(str::is_empty)
                    && cue["name"].as_str().is_none_or(str::is_empty);
            if !placeholder && start.is_none() {
                issue(track, "unresolved_djstudio_cue_position", "cues");
            }
            let length = cue["length"].as_f64().filter(|n| *n > 0.);
            let end = beat
                .zip(length)
                .and_then(|(beat, length)| beat_time(structure, beat + length))
                .filter(|end| start.is_some_and(|start| *end > start));
            if length.is_some() && end.is_none() {
                issue(track, "unresolved_djstudio_loop_end", "loops");
            }
            track.cues.push(CueObservation {
                external_id: format!(
                    "{}:{group}:{index}",
                    audio["key"].as_str().unwrap_or("unknown")
                ),
                family,
                source_slot: if family == CueFamily::Hot {
                    Some(i32::try_from(index)?)
                } else {
                    None
                },
                name: cue["name"].as_str().map(str::to_owned),
                native_start_seconds: start,
                native_end_seconds: end,
                clock_status: ClockStatus::UnresolvedNative,
                original: cue.clone(),
                flags: BTreeMap::from([
                    ("source_group".into(), json!(group)),
                    ("array_index".into(), json!(index)),
                    ("placeholder".into(), json!(placeholder)),
                    ("removed".into(), json!(cue["type"].as_i64() == Some(999))),
                    ("source_type".into(), cue["type"].clone()),
                    ("slot_mapping_qualified".into(), json!(false)),
                ]),
                ..Default::default()
            });
        }
    }
    if groups
        .get("hotCuePoints")
        .and_then(Value::as_array)
        .is_some_and(|c| !c.is_empty())
    {
        issue(track, "djstudio_pad_mapping_unverified", "hot_cues");
        track
            .extensions
            .insert("hotcue_occupancy".into(), json!("unknown"));
    }
    Ok(())
}

struct Projection<'a> {
    aux: &'a Connection,
    options: &'a ReadOptions,
}
impl Projection<'_> {
    fn track(
        &self,
        external_id: &str,
        sql: Option<Value>,
        audio: Option<Value>,
    ) -> Result<TrackObservation> {
        let mut track = TrackObservation {
            external_id: external_id.into(),
            ..Default::default()
        };
        let empty = json!({});
        let sql = sql.as_ref().unwrap_or(&empty);
        let audio = audio.as_ref().unwrap_or(&empty);
        let tag = audio.get("tag").and_then(Value::as_object);
        if let Some(tag) = tag {
            for (key, value) in tag {
                track.cached_tags.insert(key.clone(), scalar(value));
            }
        }
        for (sql_name, json_name, target) in [
            ("title", "title", "title"),
            ("artist", "artist", "artist"),
            ("album", "album", "album"),
            ("genre", "genre", "genre"),
            ("label", "label", "label"),
            ("isrc", "isrc", "isrc"),
            ("releaseDate", "releaseDate", "release_date"),
            ("addedDate", "addedDate", "date_added"),
        ] {
            if let Some(value) = candidate(
                &mut track,
                target,
                sql.get(sql_name),
                tag.and_then(|t| t.get(json_name)),
            ) {
                if value.is_string() || value.is_null() {
                    track.metadata.insert(target.into(), scalar(&value));
                } else {
                    issue(&mut track, "invalid_djstudio_metadata_type", target);
                }
            }
        }
        for (source, target) in [
            ("comment", "comment"),
            ("track", "track_number"),
            ("year", "year"),
        ] {
            if let Some(value) = tag.and_then(|t| t.get(source)) {
                if value.is_string() || value.is_null() {
                    track.metadata.insert(target.into(), scalar(value));
                } else {
                    issue(&mut track, "invalid_djstudio_tag_type", target);
                }
            }
        }
        if let Some(value) = candidate(&mut track, "bpm", sql.get("bpm"), audio.get("bpm")) {
            if value.is_null() || value.is_number() {
                track.bpm = Some(ScalarBpm {
                    presence: if value.is_null() {
                        FieldPresence::Null
                    } else {
                        FieldPresence::Value
                    },
                    value: value.as_f64().filter(|v| *v > 0.),
                    provenance: Some("DJ.Studio native source".into()),
                    ..Default::default()
                });
            } else {
                issue(&mut track, "invalid_djstudio_bpm", "bpm");
            }
        }
        if let Some(value) = candidate(&mut track, "key", sql.get("key"), audio.get("camelotKey")) {
            track.key = Some(MusicalKey {
                presence: if value.is_null() {
                    FieldPresence::Null
                } else {
                    FieldPresence::Value
                },
                raw: if value.is_number() || value.is_string() {
                    Some(value.to_string())
                } else {
                    None
                },
                camelot: records::numeric_key(&value),
                provenance: Some("DJ.Studio numeric key table".into()),
                ..Default::default()
            });
        }
        if let Some(value) = candidate(&mut track, "rating", sql.get("rating"), audio.get("rating"))
        {
            track.metadata.insert(
                "rating".into(),
                if value.is_null() {
                    Presence::Null
                } else {
                    Presence::Value(json!({"raw":value,"scale":"unqualified_djstudio"}))
                },
            );
        }
        for name in ["originalCamelotKey", "noteKey", "mikKey", "bpmMultiplier"] {
            if let Some(value) = audio.get(name) {
                track
                    .cached_tags
                    .insert(format!("djstudio_{name}"), scalar(value));
            }
        }
        let paths = audio_paths(audio);
        let preferred = sql["filePath"].as_str().or_else(|| paths.first().copied());
        if let Some(path) = preferred {
            match location(path, None, self.options) {
                Ok(evidence) => track.locations = evidence,
                Err(_) => {
                    track.locations.original = Some(path.into());
                    track.locations.availability = "unresolved".into();
                    issue(&mut track, "unresolved_djstudio_location", "location");
                }
            }
        } else {
            track.locations.availability = "unresolved".into();
            issue(&mut track, "missing_djstudio_location", "location");
        }
        track
            .locations
            .extensions
            .insert("recorded_path_candidates".into(), json!(paths));
        if track.locations.availability != "available" {
            issue(&mut track, "unavailable_djstudio_media", "location");
        }
        let duration = candidate(
            &mut track,
            "duration",
            sql.get("duration"),
            audio.get("duration"),
        );
        if let Some(value) = duration.and_then(|v| v.as_f64()).filter(|v| *v >= 0.) {
            track
                .extensions
                .insert("duration_seconds".into(), json!(value));
        }
        if audio.get("key").is_some() {
            performance(self.aux, audio, &mut track)?;
        } else {
            track
                .extensions
                .insert("hotcue_occupancy".into(), json!("unknown"));
            issue(&mut track, "missing_djstudio_imported_record", "cues");
        }
        let mut maps = Vec::new();
        let mut map_bytes = 0u64;
        if let Some(id) = sql["id"].as_i64() {
            let mut statement=self.aux.prepare("SELECT json FROM rows WHERE kind='TrackLibraryMap' AND json_extract(json,'$.trackId')=?1 ORDER BY id")?;
            let mut rows = statement.query([id])?;
            while let Some(row) = rows.next()? {
                let raw: String = row.get(0)?;
                map_bytes += raw.len() as u64;
                ensure!(
                    maps.len() < 100_000 && map_bytes <= self.options.limits.blob_bytes,
                    "DJ.Studio per-track mapping budget"
                );
                maps.push(serde_json::from_str::<Value>(&raw)?);
            }
        };
        track
            .extensions
            .insert("external_library_maps".into(), json!(maps));
        track.extensions.insert("native_identity".into(),json!({"sql_id":sql.get("id"),"audio_key":audio.get("key"),"structure_key":audio.get("structureKey")}));
        track.extensions.insert("processed_file_identity".into(),json!({"key":audio.get("key"),"fileHash":audio.get("fileHash"),"kind":audio.get("kind"),"externalRec":audio.get("externalRec")}));
        track.extensions.insert("sql_row".into(), sql.clone());
        track.extensions.insert("json_record".into(), audio.clone());
        Ok(track)
    }
}
impl SourceAdapter for DjStudioReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::DjStudio
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        ensure!(
            snapshot.provider == self.provider(),
            "Mismatched DJ.Studio provider"
        );
        Ok(profile_for(&sql_profile(
            snapshot,
            &ReadOptions::default(),
        )?))
    }
    fn read(
        &self,
        snapshot: &ImportSnapshot,
        options: &ReadOptions,
        sink: &mut dyn ObservationSink,
        cancel: &CancellationToken,
    ) -> Result<ObservationHeader> {
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        let sql = sql_profile(snapshot, options)?;
        let profile = profile_for(&sql);
        let temp = tempfile::tempdir()?;
        let aux = Connection::open(temp.path().join("studio-import.sqlite"))?;
        aux.execute_batch("PRAGMA cache_size=-2048;CREATE TABLE rows(kind TEXT,id TEXT,path TEXT,json TEXT,blob TEXT,PRIMARY KEY(kind,id));CREATE INDEX source_paths ON rows(kind,path);CREATE TABLE audio_paths(id TEXT,path TEXT,PRIMARY KEY(id,path));CREATE INDEX audio_paths_path ON audio_paths(path);CREATE TABLE joined_audio(id TEXT PRIMARY KEY);CREATE TABLE invalid_records(id TEXT PRIMARY KEY,blob TEXT);BEGIN;")?;
        let (sql_pages, mut unrepresented) = sql_rows(&sql, &aux, options, sink, cancel)?;
        let (projects, json_errors) = json_rows(snapshot, options, &aux, sink, cancel)?;
        unrepresented += json_errors;
        aux.execute_batch("COMMIT")?;
        let projection = Projection { aux: &aux, options };
        let mut tracks = 0u64;
        let mut statement =
            aux.prepare("SELECT id,path,json FROM rows WHERE kind='Tracks' ORDER BY id")?;
        let mut rows = statement.query([])?;
        for issue in &profile.issues {
            sink.push(ObservationRecord::Issue(issue.clone()))?;
        }
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: String = row.get(0)?;
            let path: Option<String> = row.get(1)?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(2)?)?;
            let joined = joined_audio(&aux, path.as_deref())?;
            let audio = joined
                .as_deref()
                .map(|key| get(&aux, "audio", key))
                .transpose()?
                .flatten();
            if let Some(key) = joined {
                aux.execute("INSERT INTO joined_audio VALUES(?1)", [key])?;
            }
            let mut track = projection.track(&format!("sql:{id}"), Some(raw), audio)?;
            if let Some(path) = path {
                let json_count: i64 = aux.query_row(
                    "SELECT count(*) FROM audio_paths WHERE path=?1",
                    [path],
                    |r| r.get(0),
                )?;
                if json_count > 0 && track.extensions["native_identity"]["audio_key"].is_null() {
                    issue(&mut track, "ambiguous_djstudio_path_join", "identity");
                }
            }
            emit_track(track, options, sink, cancel)?;
            tracks += 1;
        }
        let mut statement=aux.prepare("SELECT id,json FROM rows WHERE kind='audio' AND id NOT IN(SELECT id FROM joined_audio) ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: String = row.get(0)?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(1)?)?;
            let mut track = projection.track(&format!("audio:{id}"), None, Some(raw))?;
            let count:i64=aux.query_row("SELECT count(*) FROM audio_paths a JOIN audio_paths b ON a.path=b.path AND a.id!=b.id WHERE a.id=?1",[&id],|r|r.get(0))?;
            if count > 0 {
                issue(&mut track, "ambiguous_djstudio_path_join", "identity");
            }
            emit_track(track, options, sink, cancel)?;
            tracks += 1;
        }
        // Native dangling links become visible placeholders without reading new files.
        let mut missing=aux.prepare("SELECT DISTINCT json_extract(json,'$.trackId') FROM rows WHERE kind='PlaylistTracks' AND CAST(json_extract(json,'$.trackId') AS TEXT) NOT IN(SELECT id FROM rows WHERE kind='Tracks')")?;
        let mut rows = missing.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: i64 = row.get(0)?;
            let mut track = TrackObservation {
                external_id: format!("sql:{id}"),
                ..Default::default()
            };
            issue(&mut track, "missing_djstudio_sql_track", "source");
            emit_track(track, options, sink, cancel)?;
            tracks += 1;
        }
        let mut collections = 0u64;
        for kind in ["Playlists", "Folders"] {
            let mut statement=aux.prepare("SELECT id,json FROM rows WHERE kind=?1 ORDER BY CAST(json_extract(json,'$.sort') AS REAL),CAST(id AS INTEGER)")?;
            let mut rows = statement.query([kind])?;
            while let Some(row) = rows.next()? {
                cancel.check()?;
                let id: String = row.get(0)?;
                let raw: Value = serde_json::from_str(&row.get::<_, String>(1)?)?;
                let bookmark = kind == "Folders";
                let mut extensions = BTreeMap::from([
                    ("source_row".into(), raw.clone()),
                    ("display_order".into(), raw["sort"].clone()),
                    (
                        "ordering_qualification".into(),
                        json!("deterministic_sort_id_unverified_ui"),
                    ),
                ]);
                if bookmark {
                    extensions.insert("bookmark_path".into(), raw["path"].clone());
                }
                sink.push(ObservationRecord::Collection(CollectionObservation {
                    external_id: format!("{}:{id}", if bookmark { "bookmark" } else { "playlist" }),
                    kind: if bookmark {
                        CollectionKind::Folder
                    } else {
                        CollectionKind::Playlist
                    },
                    name: raw[if bookmark { "name" } else { "title" }]
                        .as_str()
                        .unwrap_or("")
                        .into(),
                    extensions,
                    ..Default::default()
                }))?;
                collections += 1;
            }
        }
        let mut missing=aux.prepare("SELECT DISTINCT json_extract(json,'$.playlistId') FROM rows WHERE kind='PlaylistTracks' AND CAST(json_extract(json,'$.playlistId') AS TEXT) NOT IN(SELECT id FROM rows WHERE kind='Playlists')")?;
        let mut rows = missing.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: i64 = row.get(0)?;
            sink.push(ObservationRecord::Collection(CollectionObservation {
                external_id: format!("playlist:{id}"),
                extensions: BTreeMap::from([
                    ("missing_source_collection".into(), json!(true)),
                    (
                        "ordering_qualification".into(),
                        json!("deterministic_sort_id_unverified_ui"),
                    ),
                ]),
                ..Default::default()
            }))?;
            sink.push(ObservationRecord::Issue(ImportIssue {
                code: "missing_djstudio_playlist".into(),
                record_id: Some(format!("playlist:{id}")),
                message:
                    "Native membership references a missing playlist; source identity retained"
                        .into(),
                ..Default::default()
            }))?;
            collections += 1;
        }
        let mut memberships = 0u64;
        let mut statement=aux.prepare("SELECT id,json FROM rows WHERE kind='PlaylistTracks' ORDER BY json_extract(json,'$.playlistId'),json_extract(json,'$.sort'),CAST(id AS INTEGER)")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: String = row.get(0)?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(1)?)?;
            sink.push(ObservationRecord::Membership {
                collection_id: format!("playlist:{}", raw["playlistId"]),
                occurrence: MembershipObservation {
                    external_occurrence_id: id,
                    track_external_id: format!("sql:{}", raw["trackId"]),
                    position: raw["sort"].as_i64(),
                    extensions: BTreeMap::from([
                        ("source_row".into(), raw),
                        (
                            "ordering_qualification".into(),
                            json!("deterministic_sort_id_unverified_ui"),
                        ),
                    ]),
                },
            })?;
            memberships += 1;
        }
        let mut header = header(
            snapshot,
            options,
            &profile,
            None,
            tracks,
            collections,
            memberships,
        );
        header.product_version =
            "DJ.Studio migration/JSON structural profile; native/renderer unverified".into();
        header.extensions.insert("sql_profile".into(), sql.facts);
        header
            .extensions
            .insert("sql_raw_pages".into(), serde_json::to_value(sql_pages)?);
        header
            .extensions
            .insert("project_reference_pages".into(), json!(projects));
        header.extensions.insert(
            "native_renderer_origin".into(),
            serde_json::to_value(&options.origin)?,
        );
        let mut references = Vec::new();
        let mut statement = aux.prepare(
            "SELECT kind,id,blob FROM rows WHERE kind IN('audio','structure') ORDER BY kind,id",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let kind: String = row.get(0)?;
            let key: String = row.get(1)?;
            let blob: Option<String> = row.get(2)?;
            references.push(json!({"kind":kind,"key":key,"raw":blob.map(|b|serde_json::from_str::<Value>(&b)).transpose()?}));
            if references.len() >= options.limits.page_size.max(1) {
                let blob =
                    sink.put_blob(&serde_json::to_vec(&references)?, Some("application/json"))?;
                header
                    .extensions
                    .entry("record_reference_pages".into())
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .context("Reference page list")?
                    .push(serde_json::to_value(blob)?);
                references.clear();
            }
        }
        if !references.is_empty() {
            let blob =
                sink.put_blob(&serde_json::to_vec(&references)?, Some("application/json"))?;
            header
                .extensions
                .entry("record_reference_pages".into())
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .context("Reference page list")?
                .push(serde_json::to_value(blob)?);
        }
        let mut invalid = RefPages::new();
        let mut statement = aux.prepare("SELECT id,blob FROM invalid_records ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            invalid.add(json!({"artifact":row.get::<_,String>(0)?,"raw":serde_json::from_str::<Value>(&row.get::<_,String>(1)?)?}),options,sink)?;
        }
        invalid.flush(sink)?;
        header.extensions.insert(
            "invalid_record_reference_pages".into(),
            serde_json::to_value(invalid.refs)?,
        );
        if unrepresented > 0 || sql.db.is_some() && !sql.qualified {
            header.completeness = Completeness::PartialWithErrors;
            header.accounting.unrepresented_records = unrepresented;
        }
        snapshot.validate_source(cancel)?;
        Ok(header)
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    #[test]
    fn source_json_change_during_sql_backup_rejects_the_whole_capture() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("Database");
        fs::create_dir_all(&database).unwrap();
        let input = Connection::open(database.join("studio.db")).unwrap();
        input
            .execute_batch("CREATE TABLE marker(id INTEGER)")
            .unwrap();
        drop(input);
        let record = database.join("audio-record");
        fs::write(&record, b"before").unwrap();
        let request = SourceCaptureRequest::from_selected_root(
            SourceProvider::DjStudio,
            root.path(),
            vec![
                ArtifactRequest::stable("Database/audio-record"),
                ArtifactRequest::sqlite("Database/studio.db"),
            ],
        )
        .unwrap();
        let result = super::super::super::snapshot::capture_source_with_sqlite(
            &request,
            root.path(),
            &ImportLimits::default(),
            &CancellationToken::default(),
            |input, output, _, _| {
                let input = Connection::open_with_flags(input, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
                let mut output = Connection::open(output)?;
                rusqlite::backup::Backup::new(&input, &mut output)?.run_to_completion(
                    64,
                    std::time::Duration::from_millis(1),
                    None,
                )?;
                fs::write(&record, b"after")?;
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&record).unwrap(), b"after");
    }
}
