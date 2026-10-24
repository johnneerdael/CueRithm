//! Engine Database2 snapshots. No source normalization, migration or writer call.
use super::*;
use rusqlite::{Connection, OpenFlags, OptionalExtension, limits::Limit, params, types::ValueRef};
use std::collections::{BTreeMap, BTreeSet};
pub struct EngineDjReader;
fn columns(db: &Connection, table: &str) -> Result<Vec<String>> {
    Ok(db
        .prepare(&format!("PRAGMA table_info(\"{table}\")"))?
        .query_map([], |r| r.get(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}
fn uuid(value: &str) -> Result<String> {
    Ok(uuid::Uuid::parse_str(value)
        .context("Invalid Engine source database UUID")?
        .to_string())
}
fn open(
    snapshot: &ImportSnapshot,
    id: &str,
    options: &ReadOptions,
) -> Result<(Connection, SourceProfile, String, bool)> {
    let artifact = snapshot
        .artifacts
        .iter()
        .find(|a| a.id == id)
        .context("Unknown artifact")?;
    ensure!(
        matches!(artifact.kind, ArtifactKind::Sqlite),
        "Engine requires coherent SQLite artifacts"
    );
    let original = snapshot.original_artifact_path(id)?;
    ensure!(
        original
            .file_name()
            .is_some_and(|n| n == "m.db" || n == "stm.db" || n == "hm.db")
            && original
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|n| n == "Database2"),
        "Choose Engine Library/Database2 native artifacts"
    );
    let db = Connection::open_with_flags(snapshot.artifact(id)?, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.pragma_update(None, "query_only", true)?;
    db.set_limit(
        Limit::SQLITE_LIMIT_LENGTH,
        options
            .limits
            .blob_bytes
            .max(options.limits.text_bytes)
            .min(i32::MAX as u64) as i32,
    )?;
    for table in [
        "Information",
        "Track",
        "Playlist",
        "PlaylistEntity",
        "Smartlist",
        "Historylist",
        "HistorylistEntity",
    ] {
        let object: Option<(String, String)> = db
            .query_row(
                "SELECT type,sql FROM sqlite_master WHERE name=?1",
                [table],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((kind, sql)) = object {
            ensure!(
                kind == "table" && !sql.to_ascii_uppercase().contains("CREATE VIRTUAL TABLE"),
                "Engine music projections require ordinary native tables"
            );
        }
    }
    let information = columns(&db, "Information")?;
    ensure!(
        [
            "uuid",
            "schemaVersionMajor",
            "schemaVersionMinor",
            "schemaVersionPatch"
        ]
        .iter()
        .all(|name| information.iter().any(|n| n == name)),
        "Unknown Engine Information profile"
    );
    let values = db
        .prepare(
            "SELECT uuid,schemaVersionMajor,schemaVersionMinor,schemaVersionPatch FROM Information LIMIT 2",
        )?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u32>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, u32>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        values.len() == 1,
        "Missing or ambiguous Engine library identity"
    );
    let (identity, major, minor, patch) = &values[0];
    let identity = uuid(identity)?;
    ensure!(
        matches!(
            (*major, *minor, *patch),
            (2, 20, 2) | (2, 20, 3) | (2, 21, 0) | (3, 0, 0)
        ),
        "Unqualified Engine relational schema version"
    );
    let tracks = columns(&db, "Track")?;
    ensure!(
        [
            "id",
            "originDatabaseUuid",
            "originTrackId",
            "path",
            "artist",
            "title",
            "bpmAnalyzed",
            "key",
            "rating",
            "length",
            "isBeatGridLocked"
        ]
        .iter()
        .all(|name| tracks.iter().any(|n| n == name)),
        "Unknown Engine Track projection"
    );
    let performance_kind: Option<String> = db
        .query_row(
            "SELECT type FROM sqlite_master WHERE name='PerformanceData'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    // A legacy view is not executed: project its known native Track fields directly.
    let table = performance_kind.as_deref() == Some("table");
    ensure!(
        performance_kind
            .as_deref()
            .is_none_or(|k| k == "table" || k == "view"),
        "Unsupported Engine performance object"
    );
    let perf = columns(&db, if table { "PerformanceData" } else { "Track" })?;
    ensure!(
        ["beatData", "quickCues", "loops"]
            .iter()
            .all(|name| perf.iter().any(|n| n == name))
            && (!table || perf.iter().any(|n| n == "trackId")),
        "Unknown Engine performance projection"
    );
    let mut manifest = BTreeMap::from([
        ("Information", information),
        ("Track", tracks),
        ("performance", perf),
    ]);
    for name in [
        "Playlist",
        "PlaylistEntity",
        "Smartlist",
        "Historylist",
        "HistorylistEntity",
    ] {
        manifest.insert(name, columns(&db, name)?);
    }
    let mut profile = profile(
        SourceProvider::EngineDj,
        "engine-database2-relational",
        &[
            "descriptive_tags",
            "key",
            "bpm",
            "beatgrid",
            "hot_cues",
            "loops",
            "ratings",
            "collections",
        ],
    );
    profile.schema_fingerprint = digest(&serde_json::to_vec(
        &json!({"version":[major,minor,patch],"columns":manifest,"performance_kind":performance_kind}),
    )?);
    profile
        .origin
        .insert("schema_version".into(), format!("{major}.{minor}.{patch}"));
    Ok((db, profile, identity, table))
}
fn raw_row(
    row: &rusqlite::Row<'_>,
    names: &[String],
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
) -> Result<Value> {
    let mut out = serde_json::Map::new();
    for (index, name) in names.iter().enumerate() {
        let value = match row.get_ref(index)? {
            ValueRef::Blob(bytes) => {
                ensure!(
                    bytes.len() as u64 <= options.limits.blob_bytes,
                    "Engine blob byte limit"
                );
                json!({"blob":sink.put_blob(bytes,None)?})
            }
            ValueRef::Text(bytes) => {
                ensure!(
                    bytes.len() as u64 <= options.limits.text_bytes,
                    "Engine text byte limit"
                );
                json!(std::str::from_utf8(bytes)?)
            }
            value => super::mixxx_schema::scalar(value)?,
        };
        out.insert(name.clone(), value);
    }
    Ok(Value::Object(out))
}
fn external_id(origin: &str, id: i64) -> String {
    format!("{origin}:{id}")
}
fn stage(
    db: &Connection,
    mut track: TrackObservation,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
) -> Result<()> {
    let old: Option<String> = db
        .query_row(
            "SELECT json FROM tracks WHERE id=?1",
            [&track.external_id],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(old) = old {
        let previous: TrackObservation = serde_json::from_str(&old)?;
        ensure!(
            previous.metadata == track.metadata
                && previous.key == track.key
                && previous.bpm == track.bpm
                && previous.cues == track.cues
                && previous.grid == track.grid
                && previous.locations == track.locations,
            "Conflicting Engine origin replicas require a narrower source selection"
        );
        let replica = sink.put_blob(&serde_json::to_vec(&track)?, Some("application/json"))?;
        track = previous.clone();
        track
            .extensions
            .entry("replicas".into())
            .or_insert(json!([]))
            .as_array_mut()
            .context("Invalid replica list")?
            .push(json!(replica));
        issue(&mut track, "duplicate_engine_origin_identity", "identity");
    }
    let encoded = serde_json::to_string(&track)?;
    ensure!(
        encoded.len() as u64 <= options.limits.text_bytes,
        "Engine record byte limit"
    );
    db.execute(
        "INSERT INTO tracks VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        params![track.external_id, encoded],
    )?;
    Ok(())
}
fn convert(
    row: &rusqlite::Row<'_>,
    names: &[String],
    source: &str,
    base: &Path,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<TrackObservation> {
    let raw = raw_row(row, names, options, sink)?;
    let local = raw["id"]
        .as_i64()
        .context("Invalid Engine local track ID")?;
    let origin = raw["originDatabaseUuid"].as_str().map(uuid).transpose()?;
    let native = raw["originTrackId"].as_i64().filter(|n| *n > 0);
    let stable = origin
        .zip(native)
        .map(|(origin, id)| external_id(&origin, id));
    let location_raw = raw["uri"]
        .as_str()
        .filter(|s| !s.is_empty() && !raw["streamingSource"].is_null())
        .or_else(|| raw["path"].as_str());
    let mut track = TrackObservation {
        external_id: stable
            .clone()
            .unwrap_or_else(|| format!("local:{source}:{local}")),
        locations: match location_raw {
            Some(raw) => location(raw, Some(base), options)?,
            None => LocationEvidence {
                availability: "unresolved".into(),
                ..Default::default()
            },
        },
        ..Default::default()
    };
    if stable.is_none() {
        issue(&mut track, "unresolved_engine_origin_identity", "identity");
    }
    for (source, target) in [
        ("artist", "artist"),
        ("title", "title"),
        ("album", "album"),
        ("genre", "genre"),
        ("comment", "comment"),
        ("label", "label"),
        ("composer", "composer"),
        ("remixer", "remixer"),
        ("year", "year"),
        ("dateAdded", "date_added"),
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
    if let Some(value) = raw.get("rating") {
        track.metadata.insert(
            "rating".into(),
            if value.is_null() {
                Presence::Null
            } else {
                Presence::Value(json!({"raw":value,"scale_max":100}))
            },
        );
    }
    let key = &raw["key"];
    let number = key.as_u64().filter(|n| *n < 24);
    let normalized = number.and_then(|n| {
        crate::formatting::parse_key(&format!(
            "{}{}",
            n / 2 + 1,
            if n % 2 == 0 { 'd' } else { 'm' }
        ))
        .ok()
    });
    track.key = Some(MusicalKey {
        presence: if key.is_null() {
            FieldPresence::Null
        } else {
            FieldPresence::Value
        },
        raw: number.map(|n| n.to_string()),
        camelot: normalized.map(|k| k.camelot()),
        provenance: Some("engine_open_key_enum".into()),
        extensions: BTreeMap::from([("raw_enum".into(), key.clone())]),
        ..Default::default()
    });
    let bpm = &raw["bpmAnalyzed"];
    track.bpm = Some(ScalarBpm {
        presence: if bpm.is_null() {
            FieldPresence::Null
        } else {
            FieldPresence::Value
        },
        value: bpm.as_f64().filter(|b| b.is_finite() && *b > 0.),
        locked: raw["isBeatGridLocked"].as_i64().map(|v| v != 0),
        provenance: Some("engine_bpm_analyzed".into()),
        ..Default::default()
    });
    if let Some(value) = raw["length"].as_f64().filter(|n| *n >= 0.) {
        track
            .extensions
            .insert("duration_seconds".into(), json!(value));
    }
    if options.include_history {
        for (source, target) in [
            ("timeLastPlayed", "last_played"),
            ("playOrder", "play_order"),
            ("playedIndicator", "played"),
        ] {
            if let Some(value) = raw.get(source) {
                track.metadata.insert(
                    target.into(),
                    if value.is_null() {
                        Presence::Null
                    } else {
                        Presence::Value(value.clone())
                    },
                );
            }
        }
    }
    let mut rate = None;
    if let Some(index) = names.iter().position(|n| n == "beatData")
        && let ValueRef::Blob(bytes) = row.get_ref(index)?
    {
        match super::engine_codec::beats(bytes, &options.limits, cancel) {
            Ok(mut beats) => {
                rate = beats.rate;
                if let Some(grid) = &mut beats.grid {
                    grid.raw = Some(serde_json::from_value(raw["beatData"]["blob"].clone())?);
                } else {
                    issue(&mut track, "unresolved_engine_sample_rate", "grid");
                }
                track.grid = beats.grid;
                track
                    .extensions
                    .insert("native_beat_data".into(), beats.native);
            }
            Err(_) => issue(&mut track, "engine_grid_decode_failed", "grid"),
        }
    }
    for field in ["quickCues", "loops"] {
        if let Some(index) = names.iter().position(|n| n == field)
            && let ValueRef::Blob(bytes) = row.get_ref(index)?
        {
            let result = if field == "quickCues" {
                super::engine_codec::quick(bytes, rate, &options.limits, cancel)
            } else {
                super::engine_codec::loops(
                    bytes,
                    rate,
                    raw["activeOnLoadLoops"].as_u64().unwrap_or(0),
                    &options.limits,
                    cancel,
                )
            };
            match result {
                Ok(cues) => {
                    if rate.is_none() && !cues.is_empty() {
                        issue(&mut track, "unresolved_engine_sample_rate", field);
                    }
                    track.cues.extend(cues);
                }
                Err(_) => issue(&mut track, "engine_cue_decode_failed", field),
            }
        }
    }
    track.extensions.insert("native".into(), raw);
    Ok(track)
}
fn ordered(nodes: &BTreeMap<i64, i64>) -> Result<(Vec<i64>, bool)> {
    let referenced = nodes
        .values()
        .copied()
        .filter(|id| *id != 0)
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut chains = Vec::<Vec<i64>>::new();
    let mut warning = false;
    for head in nodes.keys().filter(|id| !referenced.contains(id)) {
        let mut chain = Vec::new();
        let mut at = *head;
        let mut active = BTreeSet::new();
        while at != 0 {
            ensure!(active.insert(at), "Cyclic Engine next link");
            let Some(next) = nodes.get(&at) else {
                warning = true;
                break;
            };
            if !seen.insert(at) {
                warning = true;
                break;
            }
            chain.push(at);
            at = *next;
        }
        chains.push(chain);
    }
    ensure!(
        seen.len() == nodes.len(),
        "Cyclic or unreachable Engine next links"
    );
    warning |= chains.len() > 1;
    chains.sort_by(|a, b| {
        b.len()
            .cmp(&a.len())
            .then_with(|| a.first().cmp(&b.first()))
    });
    Ok((chains.into_iter().flatten().collect(), warning))
}
fn retain_unsupported(
    db: &Connection,
    table: &str,
    scope: &str,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<u64> {
    let names = columns(db, table)?;
    if names.is_empty() {
        return Ok(0);
    }
    let count: u64 = db.query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |r| {
        r.get(0)
    })?;
    for index in 0..count {
        cancel.check()?;
        sink.push(ObservationRecord::Issue(ImportIssue {
            code: format!("unsupported_engine_{}", table.to_ascii_lowercase()),
            record_id: Some(format!("unrepresented:{scope}:{table}:{index}")),
            field: Some(table.into()),
            message: "Native row counted but this rule/history projection is unqualified".into(),
        }))?;
    }
    Ok(count)
}
fn smart_lists(
    db: &Connection,
    all: &BTreeMap<i64, Value>,
    source: &str,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<Option<u64>> {
    let native_columns = columns(db, "Smartlist")?;
    if native_columns.is_empty() {
        return Ok(Some(0));
    }
    if !["listUuid", "title", "rules"]
        .iter()
        .all(|n| native_columns.iter().any(|c| c == n))
    {
        return Ok(None);
    }
    let mut paths = BTreeMap::<String, Vec<i64>>::new();
    for id in all.keys() {
        let mut names = Vec::new();
        let mut at = Some(*id);
        while let Some(node) = at {
            let raw = &all[&node];
            names.push(raw["title"].as_str().unwrap_or("").to_owned());
            at = raw["parentListId"].as_i64().filter(|n| *n != 0);
        }
        names.reverse();
        paths
            .entry(format!("{};", names.join(";")))
            .or_default()
            .push(*id);
    }
    let names = [
        "listUuid",
        "title",
        "parentPlaylistPath",
        "nextListUuid",
        "nextPlaylistPath",
        "rules",
        "lastEditTime",
    ]
    .into_iter()
    .filter(|n| native_columns.iter().any(|c| c == n))
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let sql = format!(
        "SELECT {} FROM Smartlist ORDER BY listUuid",
        names
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut statement = db.prepare(&sql)?;
    let mut rows = statement.query([])?;
    let mut count = 0;
    while let Some(row) = rows.next()? {
        cancel.check()?;
        let raw = raw_row(row, &names, options, sink)?;
        let id = raw["listUuid"]
            .as_str()
            .filter(|s| !s.is_empty())
            .context("Missing Engine smartlist identity")?;
        let parent_path = raw["parentPlaylistPath"].as_str().unwrap_or("");
        let candidates = paths.get(parent_path);
        let parent = candidates.filter(|ids| ids.len() == 1).map(|ids| ids[0]);
        let mut issues = vec![
            json!({"code":"unsupported_engine_smart_predicate","message":"Native rule retained; no equivalent live predicate or materialized membership claimed"}),
        ];
        if !parent_path.is_empty() && parent.is_none() {
            issues.push(json!({"code":"unresolved_engine_smart_parent"}));
        }
        sink.push(ObservationRecord::Collection(CollectionObservation {
            external_id: format!("{source}:smart:{id}"),
            parent_id: parent.map(|id| format!("{source}:playlist:{id}")),
            name: raw["title"].as_str().unwrap_or("").into(),
            kind: CollectionKind::Smart,
            extensions: BTreeMap::from([
                ("native".into(), raw.clone()),
                ("smart_rules".into(), raw["rules"].clone()),
                ("issues".into(), json!(issues)),
                ("native_order_unresolved".into(), json!(true)),
            ]),
            ..Default::default()
        }))?;
        count += 1;
    }
    Ok(Some(count))
}
impl SourceAdapter for EngineDjReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::EngineDj
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        ensure!(
            snapshot.provider == self.provider(),
            "Engine provider mismatch"
        );
        let mut profiles = Vec::new();
        for a in &snapshot.artifacts {
            if snapshot
                .original_artifact_path(&a.id)?
                .file_name()
                .is_some_and(|n| n == "hm.db")
            {
                continue;
            }
            profiles.push(open(snapshot, &a.id, &ReadOptions::default())?.1);
        }
        ensure!(
            !profiles.is_empty(),
            "Choose Engine master/streaming library"
        );
        let mut profile = profiles[0].clone();
        profile.schema_fingerprint = digest(&serde_json::to_vec(
            &profiles
                .iter()
                .map(|p| &p.schema_fingerprint)
                .collect::<Vec<_>>(),
        )?);
        Ok(profile)
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
        let profile = self.inspect(snapshot)?;
        let aux_file = tempfile::NamedTempFile::new()?;
        let aux = Connection::open(aux_file.path())?;
        aux.execute_batch("CREATE TABLE tracks(id TEXT PRIMARY KEY,json TEXT);")?;
        let mut collections = 0;
        let mut members = 0;
        let mut native_instance = None;
        let mut unrepresented = 0;
        for artifact in &snapshot.artifacts {
            cancel.check()?;
            let original = snapshot.original_artifact_path(&artifact.id)?;
            if original.file_name().is_some_and(|n| n == "hm.db") && !options.include_history {
                continue;
            }
            let (db, _, source, table) = open(snapshot, &artifact.id, options)?;
            if native_instance.is_none() || original.file_name().is_some_and(|n| n == "m.db") {
                native_instance = Some(source.clone());
            }
            let base = original
                .parent()
                .and_then(Path::parent)
                .context("Missing Engine library root")?;
            let native_columns = columns(&db, "Track")?;
            let perf_columns = columns(&db, if table { "PerformanceData" } else { "Track" })?;
            let allowed = [
                "id",
                "originDatabaseUuid",
                "originTrackId",
                "path",
                "artist",
                "title",
                "album",
                "genre",
                "comment",
                "label",
                "composer",
                "remixer",
                "year",
                "fileBytes",
                "length",
                "bitrate",
                "bpmAnalyzed",
                "key",
                "rating",
                "dateAdded",
                "timeLastPlayed",
                "playOrder",
                "playedIndicator",
                "isBeatGridLocked",
                "streamingSource",
                "uri",
                "isAvailable",
                "isAnalyzed",
                "isMetadataImported",
                "fileType",
                "filename",
            ];
            let mut names = allowed
                .into_iter()
                .filter(|n| native_columns.iter().any(|c| c == n))
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let mut projection = names
                .iter()
                .map(|n| format!("t.\"{n}\""))
                .collect::<Vec<_>>();
            for field in [
                "beatData",
                "quickCues",
                "loops",
                "thirdPartySourceId",
                "activeOnLoadLoops",
            ] {
                if perf_columns.iter().any(|c| c == field) {
                    names.push(field.into());
                    projection.push(format!("{}.\"{field}\"", if table { "p" } else { "t" }));
                }
            }
            let query = format!(
                "SELECT {} FROM Track t {} ORDER BY t.id",
                projection.join(","),
                if table {
                    "LEFT JOIN PerformanceData p ON p.trackId=t.id"
                } else {
                    ""
                }
            );
            let mut statement = db.prepare(&query)?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                cancel.check()?;
                stage(
                    &aux,
                    convert(row, &names, &source, base, options, sink, cancel)?,
                    options,
                    sink,
                )?;
            }
            drop(rows);
            drop(statement);
            let collection_names = [
                "id",
                "title",
                "parentListId",
                "nextListId",
                "isPersisted",
                "isExplicitlyExported",
            ]
            .map(str::to_owned);
            let mut statement=db.prepare("SELECT id,title,parentListId,nextListId,isPersisted,isExplicitlyExported FROM Playlist ORDER BY id")?;
            let mut rows = statement.query([])?;
            let mut all = BTreeMap::<i64, Value>::new();
            while let Some(row) = rows.next()? {
                cancel.check()?;
                ensure!(all.len() < 1_000_000, "Engine collection limit");
                let raw = raw_row(row, &collection_names, options, sink)?;
                let id = raw["id"].as_i64().context("Invalid Engine playlist ID")?;
                all.insert(id, raw);
            }
            drop(rows);
            drop(statement);
            let mut groups = BTreeMap::<i64, BTreeMap<i64, i64>>::new();
            for (id, raw) in &all {
                let parent = raw["parentListId"].as_i64().unwrap_or(0);
                let mut at = parent;
                let mut seen = BTreeSet::new();
                while at != 0 {
                    ensure!(
                        seen.insert(at) && seen.len() <= options.limits.hierarchy_depth,
                        "Cyclic/deep Engine playlist hierarchy"
                    );
                    at = all.get(&at).context("Missing Engine parent playlist")?["parentListId"]
                        .as_i64()
                        .unwrap_or(0);
                }
                groups
                    .entry(parent)
                    .or_default()
                    .insert(*id, raw["nextListId"].as_i64().unwrap_or(0));
            }
            let mut display = BTreeMap::new();
            let mut ordering_warnings = BTreeSet::new();
            for (group, nodes) in groups {
                let (order, warning) = ordered(&nodes)?;
                if warning {
                    ordering_warnings.insert(group);
                }
                for (position, id) in order.into_iter().enumerate() {
                    display.insert(id, position);
                }
            }
            for (id, raw) in &all {
                cancel.check()?;
                let collection_id = format!("{source}:playlist:{id}");
                let parent = raw["parentListId"].as_i64().filter(|p| *p != 0);
                let mut collection = CollectionObservation {
                    external_id: collection_id.clone(),
                    parent_id: parent.map(|p| format!("{source}:playlist:{p}")),
                    name: raw["title"].as_str().unwrap_or("").into(),
                    kind: if all
                        .values()
                        .any(|r| r["parentListId"].as_i64() == Some(*id))
                    {
                        CollectionKind::Folder
                    } else {
                        CollectionKind::Playlist
                    },
                    extensions: BTreeMap::from([
                        ("native".into(), raw.clone()),
                        ("display_order".into(), json!(display[id])),
                    ]),
                    ..Default::default()
                };
                if ordering_warnings.contains(&parent.unwrap_or(0)) {
                    collection.extensions.insert(
                        "ordering_warning".into(),
                        json!("disconnected_or_invalid_next_links"),
                    );
                }
                let member_names = [
                    "id",
                    "trackId",
                    "databaseUuid",
                    "nextEntityId",
                    "membershipReference",
                ]
                .map(str::to_owned);
                let mut statement=db.prepare("SELECT id,trackId,databaseUuid,nextEntityId,membershipReference FROM PlaylistEntity WHERE listId=?1 ORDER BY id")?;
                let mut rows = statement.query([id])?;
                let mut direct = BTreeMap::<i64, Value>::new();
                let mut inherited = Vec::new();
                let mut inherited_pages = Vec::new();
                while let Some(row) = rows.next()? {
                    cancel.check()?;
                    let raw = raw_row(row, &member_names, options, sink)?;
                    if raw["membershipReference"].as_i64().unwrap_or(0) != 0 {
                        inherited.push(raw);
                        if inherited.len() >= options.limits.page_size.clamp(1, 1000) {
                            inherited_pages.push(sink.put_blob(
                                &serde_json::to_vec(&inherited)?,
                                Some("application/json"),
                            )?);
                            inherited.clear();
                        }
                    } else {
                        ensure!(direct.len() < 1_000_000, "Engine membership limit");
                        direct.insert(
                            raw["id"].as_i64().context("Invalid native occurrence ID")?,
                            raw,
                        );
                    }
                }
                if !inherited.is_empty() {
                    inherited_pages.push(
                        sink.put_blob(&serde_json::to_vec(&inherited)?, Some("application/json"))?,
                    );
                }
                drop(rows);
                drop(statement);
                if !inherited_pages.is_empty() {
                    collection
                        .extensions
                        .insert("inherited_membership_pages".into(), json!(inherited_pages));
                }
                let nodes = direct
                    .iter()
                    .map(|(id, raw)| (*id, raw["nextEntityId"].as_i64().unwrap_or(0)))
                    .collect();
                let (order, warning) = ordered(&nodes)?;
                if warning {
                    collection.extensions.insert(
                        "membership_ordering_warning".into(),
                        json!("disconnected_or_invalid_next_links"),
                    );
                }
                for (position, occurrence) in order.into_iter().enumerate() {
                    cancel.check()?;
                    let raw = &direct[&occurrence];
                    let origin = raw["databaseUuid"].as_str().map(uuid).transpose()?;
                    let native = raw["trackId"].as_i64();
                    let external = origin
                        .zip(native)
                        .map(|(uuid, id)| external_id(&uuid, id))
                        .unwrap_or_else(|| format!("missing:{collection_id}:{occurrence}"));
                    let exists: bool = aux.query_row(
                        "SELECT EXISTS(SELECT 1 FROM tracks WHERE id=?1)",
                        [&external],
                        |r| r.get(0),
                    )?;
                    if !exists {
                        let mut track = TrackObservation {
                            external_id: external.clone(),
                            locations: LocationEvidence {
                                availability: "unresolved".into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        };
                        issue(&mut track, "missing_engine_origin_track", "track");
                        stage(&aux, track, options, sink)?;
                    }
                    sink.push(ObservationRecord::Membership {
                        collection_id: collection_id.clone(),
                        occurrence: MembershipObservation {
                            external_occurrence_id: occurrence.to_string(),
                            track_external_id: external,
                            position: Some(position as i64),
                            extensions: BTreeMap::from([("native".into(), raw.clone())]),
                        },
                    })?;
                    members += 1;
                }
                sink.push(ObservationRecord::Collection(collection))?;
                collections += 1;
            }
            if let Some(count) = smart_lists(&db, &all, &source, options, sink, cancel)? {
                collections += count;
            } else {
                unrepresented += retain_unsupported(&db, "Smartlist", &artifact.id, sink, cancel)?;
            }
            if options.include_history {
                unrepresented +=
                    retain_unsupported(&db, "Historylist", &artifact.id, sink, cancel)?;
                unrepresented +=
                    retain_unsupported(&db, "HistorylistEntity", &artifact.id, sink, cancel)?;
            }
        }
        let mut statement = aux.prepare("SELECT json FROM tracks ORDER BY id")?;
        let mut rows = statement.query([])?;
        let mut tracks = 0;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let track = serde_json::from_str(&row.get::<_, String>(0)?)?;
            emit_track(track, options, sink, cancel)?;
            tracks += 1;
        }
        snapshot.validate_source(cancel)?;
        cancel.check()?;
        let mut header = header(
            snapshot,
            options,
            &profile,
            native_instance.as_deref(),
            tracks,
            collections,
            members,
        );
        if unrepresented > 0 {
            header.completeness = Completeness::PartialWithErrors;
            header.accounting.unrepresented_records = unrepresented;
        }
        Ok(header)
    }
}
