//! Read-only Serato4 aggregate profile. Cues/grids remain file-owned observations.
use super::*;
use rusqlite::{Connection, OpenFlags, limits::Limit};
use std::collections::{BTreeMap, BTreeSet};
pub struct Serato4Reader;
const REQUIRED: &[(&str, &[&str])] = &[
    ("space", &["id", "name"]),
    ("space_asset", &["space_id", "asset_id"]),
    (
        "asset",
        &[
            "id",
            "location_id",
            "portable_id",
            "name",
            "artist",
            "bpm",
            "key",
            "length_ms",
        ],
    ),
    ("connection", &["location_id", "database_uri"]),
    (
        "container",
        &["id", "parent_id", "name", "type", "list_order", "space_id"],
    ),
    ("location_container", &["id", "container_id", "location_id"]),
    (
        "container_asset",
        &["id", "asset_id", "location_container_id", "list_order"],
    ),
];
fn columns(db: &Connection, table: &str) -> Result<Vec<String>> {
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(Vec::new());
    }
    Ok(db
        .prepare(&format!("PRAGMA table_info(\"{table}\")"))?
        .query_map([], |r| r.get(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}
fn open(snapshot: &ImportSnapshot, options: &ReadOptions) -> Result<(Connection, SourceProfile)> {
    ensure!(
        snapshot.provider == SourceProvider::Serato4
            && snapshot.artifacts.len() == 1
            && matches!(snapshot.artifacts[0].kind, ArtifactKind::Sqlite),
        "Select one coherent Serato4 master SQLite snapshot"
    );
    ensure!(
        snapshot
            .original_artifact_path(&snapshot.artifacts[0].id)?
            .file_name()
            .is_some_and(|n| n == "master.sqlite"),
        "Serato4 aggregate reader requires master.sqlite"
    );
    let db = Connection::open_with_flags(
        snapshot.artifact(&snapshot.artifacts[0].id)?,
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
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    ensure!(
        version == 12,
        "Unqualified Serato4 aggregate schema version"
    );
    let mut manifest = BTreeMap::new();
    for (table, required) in REQUIRED {
        let names = columns(&db, table)?;
        ensure!(
            required.iter().all(|name| names.iter().any(|n| n == name)),
            "Unsupported Serato4 {table} projection"
        );
        manifest.insert(*table, names);
    }
    for (table, required) in [
        (
            "container_keep_alive_list_tbl",
            vec!["container_id", "location_id"],
        ),
        (
            "smart_crate_rules",
            vec!["container_id", "version", "rules"],
        ),
    ] {
        let names = columns(&db, table)?;
        ensure!(
            names.is_empty() || required.iter().all(|name| names.iter().any(|n| n == name)),
            "Unsupported optional Serato4 {table} projection"
        );
        manifest.insert(table, names);
    }
    let mut profile = profile(
        SourceProvider::Serato4,
        "serato4-master-schema12",
        &[
            "descriptive_tags",
            "key",
            "bpm",
            "hot_cues",
            "loops",
            "beatgrid",
            "collections",
        ],
    );
    profile.schema_fingerprint = digest(&serde_json::to_vec(
        &json!({"version":version,"columns":manifest}),
    )?);
    for (name, group) in [("rating", "ratings"), ("color", "color")] {
        if manifest["asset"].iter().any(|n| n == name) {
            profile
                .fields
                .extend(super::profile(SourceProvider::Serato4, "optional", &[group]).fields);
        }
    }
    Ok((db, profile))
}
fn row_json(row: &rusqlite::Row<'_>, names: &[String], limits: &ImportLimits) -> Result<Value> {
    let mut out = serde_json::Map::new();
    for (index, name) in names.iter().enumerate() {
        if let rusqlite::types::ValueRef::Text(bytes) = row.get_ref(index)? {
            ensure!(
                bytes.len() as u64 <= limits.text_bytes,
                "Serato text byte limit"
            );
        }
        out.insert(
            name.clone(),
            super::mixxx_schema::scalar(row.get_ref(index)?)?,
        );
    }
    Ok(Value::Object(out))
}
fn native_path(
    portable: &str,
    uri: Option<&str>,
    options: &ReadOptions,
) -> Result<LocationEvidence> {
    if portable.contains("://")
        || portable.as_bytes().get(1) == Some(&b':')
        || portable.starts_with("//")
        || portable.starts_with("\\\\")
    {
        return location(portable, None, options);
    }
    if let Some(uri) = uri {
        let windows = uri.as_bytes().get(1) == Some(&b':')
            || uri
                .strip_prefix("file:///")
                .is_some_and(|v| v.as_bytes().get(1) == Some(&b':'))
            || uri.starts_with("\\\\");
        if let Some(uri) = super::super::paths::normalized(uri, windows)? {
            let normalized = uri.replace('\\', "/");
            let lower = normalized.to_ascii_lowercase();
            let root = if let Some(index) = lower.find("/_serato_/library/") {
                Some(normalized[..index].to_owned())
            } else if lower.ends_with("/root.sqlite") {
                if normalized.as_bytes().get(1) == Some(&b':') {
                    Some(normalized[..2].to_owned())
                } else if normalized.starts_with('/') && !normalized.starts_with("//") {
                    Some(String::new())
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(root) = root {
                return location(
                    &format!(
                        "{}/{}",
                        root,
                        portable.replace('\\', "/").trim_start_matches('/')
                    ),
                    None,
                    options,
                );
            }
        }
    }
    Ok(LocationEvidence {
        original: Some(portable.into()),
        availability: "unresolved".into(),
        extensions: BTreeMap::from([("unresolved_location_root".into(), json!(uri))]),
        ..Default::default()
    })
}
fn convert(raw: Value, options: &ReadOptions) -> Result<TrackObservation> {
    let id = raw["id"]
        .as_i64()
        .context("Invalid Serato asset ID")?
        .to_string();
    let portable = raw["portable_id"].as_str();
    let mut track = TrackObservation {
        external_id: id,
        locations: match portable {
            Some(path) => native_path(path, raw["_database_uri"].as_str(), options)?,
            None => LocationEvidence {
                availability: "unresolved".into(),
                ..Default::default()
            },
        },
        ..Default::default()
    };
    if track.locations.availability == "unresolved" {
        issue(&mut track, "unresolved_serato_location", "location");
    }
    for (source, target) in [
        ("name", "title"),
        ("artist", "artist"),
        ("album", "album"),
        ("genre", "genre"),
        ("comments", "comment"),
        ("label", "label"),
        ("remixer", "remixer"),
        ("composer", "composer"),
        ("grouping", "grouping"),
        ("year", "year"),
        ("track_number", "track_number"),
        ("time_added", "date_added"),
    ] {
        if let Some(value) = raw.get(source) {
            track.metadata.insert(
                target.into(),
                if value.is_null() {
                    Presence::Null
                } else {
                    apple_music::presence(value)
                },
            );
        }
    }
    let key = &raw["key"];
    track.key = Some(MusicalKey {
        presence: if key.is_null() {
            FieldPresence::Null
        } else if key.as_str() == Some("") {
            FieldPresence::Empty
        } else {
            FieldPresence::Value
        },
        raw: key.as_str().map(str::to_owned),
        camelot: key
            .as_str()
            .and_then(|s| crate::formatting::parse_key(s).ok())
            .map(|k| k.camelot()),
        provenance: Some("serato4_database".into()),
        ..Default::default()
    });
    let bpm = &raw["bpm"];
    track.bpm = Some(ScalarBpm {
        presence: if bpm.is_null() {
            FieldPresence::Null
        } else {
            FieldPresence::Value
        },
        value: bpm.as_f64().filter(|b| b.is_finite() && *b > 0.),
        locked: raw["is_beatgrid_locked"].as_i64().map(|v| v != 0),
        provenance: Some("serato4_database".into()),
        ..Default::default()
    });
    for (field, scale) in [("rating", 1), ("color", 16777215)] {
        if let Some(value) = raw.get(field) {
            track.metadata.insert(
                field.into(),
                if value.is_null() {
                    Presence::Null
                } else {
                    Presence::Value(json!({"raw":value,"scale_max":scale}))
                },
            );
        }
    }
    if let Some(value) = raw["length_ms"].as_f64().filter(|n| *n >= 0.) {
        track
            .extensions
            .insert("duration_seconds".into(), json!(value / 1000.));
    }
    if options.include_history
        && let Some(value) = raw.get("dj_play_count")
    {
        track.metadata.insert(
            "play_count".into(),
            if value.is_null() {
                Presence::Null
            } else {
                Presence::Value(value.clone())
            },
        );
    }
    track.extensions.insert("native".into(), raw);
    Ok(track)
}
impl SourceAdapter for Serato4Reader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::Serato4
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        Ok(open(snapshot, &ReadOptions::default())?.1)
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
        let (db, profile) = open(snapshot, options)?;
        let mut spaces = db
            .prepare("SELECT id FROM space WHERE name='Serato Library'")?
            .query_map([], |r| r.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ensure!(
            spaces.len() == 1,
            "Ambiguous or missing Serato Library space"
        );
        let space = spaces.remove(0);
        let mut roots = BTreeMap::<i64, Option<String>>::new();
        let mut connections = db.prepare("SELECT location_id,database_uri FROM connection")?;
        let mut rows = connections.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id = row.get(0)?;
            ensure!(
                !roots.contains_key(&id),
                "Ambiguous Serato location connection"
            );
            roots.insert(id, row.get(1)?);
        }
        drop(rows);
        let native_columns = columns(&db, "asset")?;
        let mut projection = vec![
            "id",
            "location_id",
            "portable_id",
            "name",
            "artist",
            "bpm",
            "key",
            "length_ms",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        for field in [
            "album",
            "genre",
            "year",
            "label",
            "remixer",
            "comments",
            "grouping",
            "composer",
            "rating",
            "emoji",
            "color",
            "time_added",
            "time_modified",
            "dj_play_count",
            "is_beatgrid_locked",
            "file_name",
            "file_size",
            "file_bit_rate",
            "file_sample_rate",
            "format",
            "track_number",
            "is_missing",
            "is_corrupt",
            "is_readonly",
            "third_party_type",
        ] {
            if native_columns.iter().any(|c| c == field) {
                projection.push(field.into());
            }
        }
        let aux_file = tempfile::NamedTempFile::new()?;
        let aux = Connection::open(aux_file.path())?;
        aux.execute_batch("CREATE TABLE emitted(id TEXT PRIMARY KEY);")?;
        let query = format!(
            "SELECT {} FROM asset a JOIN (SELECT DISTINCT asset_id FROM space_asset WHERE space_id=?1) s ON s.asset_id=a.id ORDER BY a.id",
            projection
                .iter()
                .map(|n| format!("a.\"{n}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut statement = db.prepare(&query)?;
        let mut rows = statement.query([space])?;
        let mut tracks = 0;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let mut raw = row_json(row, &projection, &options.limits)?;
            let root = raw["location_id"]
                .as_i64()
                .and_then(|id| roots.get(&id))
                .and_then(|s| s.clone());
            raw.as_object_mut()
                .context("Invalid asset")?
                .insert("_database_uri".into(), json!(root));
            let track = convert(raw, options)?;
            aux.execute("INSERT INTO emitted VALUES(?1)", [&track.external_id])?;
            super::serato_tags::emit_native_track(track, options, sink, cancel)?;
            tracks += 1;
        }
        drop(rows);
        drop(statement);
        let collection_columns =
            ["id", "parent_id", "name", "type", "list_order", "space_id"].map(str::to_owned);
        let mut statement=db.prepare("SELECT id,parent_id,name,type,list_order,space_id FROM container WHERE space_id=?1 OR space_id IS NULL ORDER BY list_order,id")?;
        let mut rows = statement.query([space])?;
        let mut all = BTreeMap::<i64, Value>::new();
        while let Some(row) = rows.next()? {
            cancel.check()?;
            ensure!(all.len() < 1_000_000, "Serato collection count limit");
            let raw = row_json(row, &collection_columns, &options.limits)?;
            let id = raw["id"].as_i64().context("Invalid container ID")?;
            all.insert(id, raw);
        }
        drop(rows);
        drop(statement);
        let mut excluded = BTreeSet::new();
        for (id, raw) in &all {
            if raw["parent_id"].is_null()
                || raw["name"]
                    .as_str()
                    .is_some_and(|n| matches!(n, "Apple Music" | "Streaming Tracks"))
                    && raw["parent_id"]
                        .as_i64()
                        .and_then(|p| all.get(&p))
                        .is_some_and(|p| p["name"] == "Serato Library root")
            {
                excluded.insert(*id);
            }
        }
        for id in all.keys() {
            let mut visited = BTreeSet::new();
            let mut current = Some(*id);
            while let Some(node) = current {
                ensure!(
                    visited.insert(node) && visited.len() <= options.limits.hierarchy_depth,
                    "Cyclic or deep Serato hierarchy"
                );
                current = all.get(&node).and_then(|raw| raw["parent_id"].as_i64());
                if current.is_some_and(|p| !all.contains_key(&p)) {
                    anyhow::bail!("Missing Serato container parent")
                }
            }
        }
        let keepalive = !columns(&db, "container_keep_alive_list_tbl")?.is_empty();
        let has_rules = !columns(&db, "smart_crate_rules")?.is_empty();
        let mut alive = BTreeSet::new();
        for (id, raw) in &all {
            cancel.check()?;
            let has_members:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM location_container l JOIN container_asset c ON c.location_container_id=l.id WHERE l.container_id=?1)",[id],|r|r.get(0))?;
            let keep = if keepalive {
                db.query_row("SELECT EXISTS(SELECT 1 FROM container_keep_alive_list_tbl WHERE container_id=?1)",[id],|r|r.get::<_,bool>(0))?
            } else {
                true
            };
            if has_members || keep {
                let mut current = Some(*id);
                while let Some(node) = current {
                    alive.insert(node);
                    current = all.get(&node).and_then(|r| r["parent_id"].as_i64());
                }
            }
            if raw["space_id"].is_null() {
                excluded.insert(*id);
            }
        }
        let mut collections = 0;
        let mut members = 0;
        for (id, raw) in &all {
            cancel.check()?;
            let mut ancestor = Some(*id);
            let mut skip = excluded.contains(id);
            while let Some(parent) = ancestor
                .and_then(|i| all.get(&i))
                .and_then(|r| r["parent_id"].as_i64())
            {
                if excluded.contains(&parent) && all[&parent]["space_id"].as_i64() == Some(space) {
                    skip = true;
                }
                ancestor = Some(parent);
            }
            if skip || !alive.contains(id) {
                continue;
            }
            let parent = raw["parent_id"].as_i64().filter(|p| !excluded.contains(p));
            let collection_id = format!("container:{id}");
            let mut collection = CollectionObservation {
                external_id: collection_id.clone(),
                parent_id: parent.map(|p| format!("container:{p}")),
                name: raw["name"].as_str().unwrap_or("").into(),
                kind: match raw["type"].as_i64() {
                    Some(0) => CollectionKind::Folder,
                    Some(2) => CollectionKind::Smart,
                    _ => CollectionKind::Crate,
                },
                extensions: BTreeMap::from([("native".into(), raw.clone())]),
                ..Default::default()
            };
            if has_rules {
                let mut statement = db
                    .prepare("SELECT version,rules FROM smart_crate_rules WHERE container_id=?1")?;
                let names = vec!["version".into(), "rules".into()];
                let mut rows = statement.query([id])?;
                let mut rules = Vec::new();
                while let Some(row) = rows.next()? {
                    ensure!(rules.len() < 65536, "Smart rule count limit");
                    rules.push(row_json(row, &names, &options.limits)?);
                }
                if !rules.is_empty() {
                    collection
                        .extensions
                        .insert("smart_rules".into(), json!(rules));
                    collection.extensions.insert(
                        "issues".into(),
                        json!([{"code":"unsupported_serato_smart_rules"}]),
                    );
                }
            }
            sink.push(ObservationRecord::Collection(collection))?;
            collections += 1;
            let mut statement=db.prepare("SELECT ca.id,ca.asset_id,ca.list_order,lc.location_id FROM container_asset ca JOIN location_container lc ON lc.id=ca.location_container_id WHERE lc.container_id=?1 ORDER BY ca.list_order,ca.id")?;
            let mut rows = statement.query([id])?;
            let mut last = None;
            let mut tied = false;
            while let Some(row) = rows.next()? {
                cancel.check()?;
                let occurrence: i64 = row.get(0)?;
                let asset: Option<i64> = row.get(1)?;
                let order: Option<i64> = row.get(2)?;
                let location: Option<i64> = row.get(3)?;
                let external = asset
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| format!("missing:{occurrence}"));
                let inserted =
                    aux.execute("INSERT OR IGNORE INTO emitted VALUES(?1)", [&external])?;
                if inserted > 0 {
                    let mut missing = TrackObservation {
                        external_id: external.clone(),
                        ..Default::default()
                    };
                    issue(&mut missing, "missing_or_unselected_serato_asset", "track");
                    issue(&mut missing, "serato_performance_unavailable", "cues/grid");
                    sink.push(ObservationRecord::Track(Box::new(missing)))?;
                    tracks += 1;
                }
                if order.is_some() && last == order {
                    tied = true;
                }
                last = order;
                sink.push(ObservationRecord::Membership {
                    collection_id: collection_id.clone(),
                    occurrence: MembershipObservation {
                        external_occurrence_id: occurrence.to_string(),
                        track_external_id: external,
                        position: order,
                        extensions: BTreeMap::from([("location_id".into(), json!(location))]),
                    },
                })?;
                members += 1;
            }
            if tied {
                sink.push(ObservationRecord::Issue(ImportIssue {
                    code: "duplicate_serato_membership_position".into(),
                    record_id: Some(collection_id),
                    message: "Equal native positions ordered by row ID; ambiguity retained".into(),
                    ..Default::default()
                }))?;
            }
        }
        snapshot.validate_source(cancel)?;
        cancel.check()?;
        Ok(header(
            snapshot,
            options,
            &profile,
            None,
            tracks,
            collections,
            members,
        ))
    }
}
