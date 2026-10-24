//! DataModel 11 SQLite projection, without Core Data, cache execution or source writes.
use super::*;
use keyed_archive::{ArchiveNode, ArchivedValue, decode_keyed_archive};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Row, limits::Limit, params, types::ValueRef,
};
use std::collections::BTreeSet;

pub struct MixedInKey11Reader;
const SCHEMA: &str = include_str!(
    "../../../../../docs/research/dj-library-import-handover/fixtures/mik-coredata-11-schema.json"
);
const ENTITIES: [&str; 6] = [
    "Collection",
    "CuePoint",
    "EnergySegment",
    "KeySegment",
    "Song",
    "Waveform",
];
fn field<'a>(graph: &'a ArchivedValue, name: &str) -> Result<&'a ArchiveNode> {
    graph
        .field(&graph.root, name)?
        .context("Missing MIK archive field")
}
fn open(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
) -> Result<(Connection, SourceProfile, String)> {
    ensure!(
        snapshot.provider == SourceProvider::MixedInKey11
            && snapshot.artifacts.len() == 1
            && matches!(snapshot.artifacts[0].kind, ArtifactKind::Sqlite),
        "Select one macOS-origin Collection11.mikdb SQLite snapshot; native Windows MIK storage is unsupported"
    );
    ensure!(
        snapshot
            .original_artifact_path(&snapshot.artifacts[0].id)?
            .file_name()
            .is_some_and(|n| n == "Collection11.mikdb"),
        "Unsupported MIK store filename/profile"
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
    let manifest: Value = serde_json::from_str(SCHEMA)?;
    for (table, shape) in manifest["tables"]
        .as_object()
        .context("Invalid pinned schema")?
    {
        let (kind, sql): (String, String) = db.query_row(
            "SELECT type,sql FROM sqlite_master WHERE name=?1",
            [table],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(
            kind == "table" && !sql.to_ascii_uppercase().contains("VIRTUAL TABLE"),
            "MIK requires ordinary source tables"
        );
        let actual = db
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))?
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let expected = shape["columns"]
            .as_array()
            .context("Missing schema columns")?;
        ensure!(
            actual.len() == expected.len(),
            "Unknown MIK column inventory"
        );
        for column in expected {
            let native = actual
                .iter()
                .find(|c| Some(c.0.as_str()) == column["name"].as_str())
                .context("Missing MIK column")?;
            ensure!(
                Some(native.1.as_str()) == column["sqliteDeclaredType"].as_str()
                    && native.2 == 0
                    && native.3.is_none()
                    && Some(native.4) == column["primaryKeyOrdinal"].as_i64(),
                "Unknown MIK column type/constraints"
            );
        }
    }
    let entities = db
        .prepare("SELECT Z_ENT,Z_NAME,Z_SUPER FROM Z_PRIMARYKEY ORDER BY Z_ENT")?
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        entities.len() == ENTITIES.len()
            && entities
                .iter()
                .enumerate()
                .all(|(i, (id, name, parent))| *id == i as i64 + 1
                    && name == ENTITIES[i]
                    && *parent == 0),
        "Unknown MIK Core Data entity mapping"
    );
    let metadata_count: i64 = db.query_row("SELECT count(*) FROM Z_METADATA", [], |r| r.get(0))?;
    ensure!(metadata_count == 1, "Ambiguous MIK store metadata");
    let (version, uuid, bytes): (i64, String, Vec<u8>) =
        db.query_row("SELECT Z_VERSION,Z_UUID,Z_PLIST FROM Z_METADATA", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })?;
    ensure!(
        version == 1 && !uuid.is_empty(),
        "Unknown MIK store version/identity"
    );
    let graph = decode_keyed_archive(&bytes, &options.limits)?;
    ensure!(
        graph.text(field(&graph, "NSStoreType")?)? == Some("SQLite")
            && graph.text(field(&graph, "NSStoreUUID")?)? == Some(&uuid),
        "Invalid MIK store metadata"
    );
    let ids = graph
        .array(field(&graph, "NSStoreModelVersionIdentifiers")?)?
        .context("Missing MIK model identifiers")?;
    ensure!(
        ids.len() == 1 && graph.text(&ids[0])? == Some("DataModel 11"),
        "Unsupported MIK model version"
    );
    let hashes = field(&graph, "NSStoreModelVersionHashes")?;
    let json_hashes = graph.to_json()?["NSStoreModelVersionHashes"].clone();
    ensure!(
        json_hashes
            .as_object()
            .is_some_and(|o| o.len() == ENTITIES.len()),
        "Unknown MIK model entity hash set"
    );
    for name in ENTITIES {
        let hash = graph
            .field(hashes, name)?
            .context("Missing MIK model entity hash")?;
        ensure!(
            graph.data(hash)?.is_some_and(|b| b.len() == 32),
            "Invalid MIK model entity hash width"
        );
    }
    let mut profile = profile(
        SourceProvider::MixedInKey11,
        "mik-macos-datamodel11-structural",
        &[
            "descriptive_tags",
            "key",
            "bpm",
            "energy",
            "markers",
            "beatgrid",
            "collections",
        ],
    );
    profile.schema_fingerprint = digest(&serde_json::to_vec(
        &json!({"schema":manifest,"entities":entities,"model":json_hashes}),
    )?);
    profile.origin.insert("origin_os".into(), "macos".into());
    profile.origin.insert("model_qualification".into(), "DataModel 11 structural fixture; native entity hash allowlist and app acceptance unrun; native Windows storage unsupported".into());
    profile.issues.push(ImportIssue { code: "mik_structural_profile_only".into(), message: "MIK DataModel 11 structure is checked; native model hash acceptance remains unqualified".into(), ..Default::default() });
    Ok((db, profile, uuid))
}
fn presence(value: &Value) -> Presence<Value> {
    if value.is_null() {
        Presence::Null
    } else if value.as_str() == Some("") {
        Presence::Empty(value.clone())
    } else {
        Presence::Value(value.clone())
    }
}
fn raw_row(
    row: &Row<'_>,
    names: &[String],
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
) -> Result<Value> {
    let mut raw = serde_json::Map::new();
    for (i, name) in names.iter().enumerate() {
        let value = match row.get_ref(i)? {
            ValueRef::Blob(b) => {
                ensure!(
                    b.len() as u64 <= options.limits.blob_bytes,
                    "MIK blob size limit"
                );
                json!({"blob":sink.put_blob(b, None)?})
            }
            value => {
                if let ValueRef::Text(b) = value {
                    ensure!(
                        b.len() as u64 <= options.limits.text_bytes,
                        "MIK text limit"
                    );
                }
                super::mixxx_schema::scalar(value)?
            }
        };
        raw.insert(name.clone(), value);
    }
    let value = Value::Object(raw);
    ensure!(
        serde_json::to_vec(&value)?.len() as u64 <= options.limits.text_bytes,
        "MIK record size limit"
    );
    Ok(value)
}
fn stage(
    db: &Connection,
    aux: &Connection,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<()> {
    aux.execute_batch("CREATE TABLE records(kind TEXT,id INTEGER,parent INTEGER,ordering REAL,raw TEXT,PRIMARY KEY(kind,id));CREATE INDEX records_parent ON records(kind,parent,ordering,id);CREATE TABLE members(collection INTEGER,song INTEGER,position INTEGER,raw TEXT,PRIMARY KEY(collection,song));")?;
    for (table, entity, parent, order) in [
        ("ZSONG", 5, None, None),
        ("ZCOLLECTION", 1, Some("ZPARENTCOLLECTION"), Some("ZINDEX")),
        ("ZCUEPOINT", 2, Some("ZSONG"), Some("ZTIME")),
        ("ZENERGYSEGMENT", 3, Some("ZSONG"), Some("ZSTARTTIME")),
        ("ZKEYSEGMENT", 4, Some("ZSONG"), Some("ZSTARTTIME")),
    ] {
        let mut statement = db.prepare(&format!("SELECT * FROM {table} ORDER BY Z_PK"))?;
        let names = statement
            .column_names()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let raw = raw_row(row, &names, options, sink)?;
            ensure!(
                raw["Z_ENT"].as_i64() == Some(entity),
                "Unexpected MIK row entity"
            );
            let id = raw["Z_PK"].as_i64().context("Missing MIK row identity")?;
            aux.execute(
                "INSERT INTO records VALUES(?1,?2,?3,?4,?5)",
                params![
                    table,
                    id,
                    parent.and_then(|p| raw[p].as_i64()),
                    order.and_then(|o| raw[o].as_f64()),
                    serde_json::to_string(&raw)?
                ],
            )?;
        }
    }
    let mut statement =
        db.prepare("SELECT * FROM Z_1SONGS ORDER BY Z_1COLLECTIONS,Z_FOK_5SONGS,Z_5SONGS")?;
    let names = statement
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        cancel.check()?;
        let raw = raw_row(row, &names, options, sink)?;
        aux.execute(
            "INSERT INTO members VALUES(?1,?2,?3,?4)",
            params![
                raw["Z_1COLLECTIONS"]
                    .as_i64()
                    .context("Missing collection reference")?,
                raw["Z_5SONGS"].as_i64().context("Missing song reference")?,
                raw["Z_FOK_5SONGS"].as_i64(),
                serde_json::to_string(&raw)?
            ],
        )?;
    }
    // Preserve dangling relationships as visible placeholders, including orphan cue/segment rows.
    aux.execute_batch("INSERT OR IGNORE INTO records SELECT 'ZSONG',song,NULL,NULL,'{\"missing_source_song\":true}' FROM members;INSERT OR IGNORE INTO records SELECT 'ZSONG',parent,NULL,NULL,'{\"missing_source_song\":true}' FROM records WHERE kind IN('ZCUEPOINT','ZKEYSEGMENT','ZENERGYSEGMENT') AND parent IS NOT NULL;INSERT OR IGNORE INTO records SELECT 'ZCOLLECTION',collection,NULL,NULL,'{\"missing_source_collection\":true}' FROM members;INSERT OR IGNORE INTO records SELECT 'ZCOLLECTION',parent,NULL,NULL,'{\"missing_source_collection\":true}' FROM records WHERE kind='ZCOLLECTION' AND parent IS NOT NULL;")?;
    Ok(())
}
fn children(aux: &Connection, kind: &str, id: i64, options: &ReadOptions) -> Result<Vec<Value>> {
    let mut statement =
        aux.prepare("SELECT raw FROM records WHERE kind=?1 AND parent=?2 ORDER BY ordering,id")?;
    let mut rows = statement.query(params![kind, id])?;
    let mut result = Vec::new();
    let mut bytes = 0u64;
    while let Some(row) = rows.next()? {
        let raw: String = row.get(0)?;
        bytes += raw.len() as u64;
        ensure!(
            bytes <= options.limits.blob_bytes && result.len() < 100_000,
            "MIK per-track child budget"
        );
        result.push(serde_json::from_str(&raw)?);
    }
    Ok(result)
}
fn grid(bytes: &[u8], options: &ReadOptions) -> Result<GridObservation> {
    let graph = decode_keyed_archive(bytes, &options.limits)?;
    let class = field(&graph, "$class")?;
    let name = graph
        .field(class, "$classname")?
        .context("Missing MIK beats class")?;
    ensure!(
        graph.text(name)? == Some("MIKBeats"),
        "Unknown MIK beats class"
    );
    let times = field(&graph, "MIKBeats_beatTimes")?;
    let array = if let Some(a) = graph.array(times)? {
        a
    } else {
        let class = graph
            .field(times, "$class")?
            .context("Missing MIK beat array class")?;
        let name = graph
            .field(class, "$classname")?
            .context("Missing array class name")?;
        ensure!(
            graph.text(name)? == Some("NSArray"),
            "Unknown MIK beat array class"
        );
        graph
            .array(
                graph
                    .field(times, "NS.objects")?
                    .context("Missing MIK beat array")?,
            )?
            .context("Invalid MIK beat array")?
    };
    let tempo = graph
        .number(field(&graph, "MIKBeats_tempo")?)?
        .filter(|n| n.is_finite() && *n > 0.);
    let mut points = Vec::with_capacity(array.len());
    for value in array {
        let seconds = graph
            .number(value)?
            .filter(|n| n.is_finite())
            .context("Invalid MIK beat value")?;
        ensure!(
            points
                .last()
                .is_none_or(|p: &GridPoint| p.native_seconds < seconds),
            "Nonincreasing MIK beats"
        );
        points.push(GridPoint {
            native_seconds: seconds,
            original: json!(seconds),
            ..Default::default()
        });
    }
    Ok(GridObservation {
        representation: "explicit_beat_times".into(),
        original_time_base: Some("MIK persisted seconds; subdivision unknown".into()),
        native_first_beat_seconds: points.first().map(|p| p.native_seconds),
        bpm: tempo,
        points,
        clock_status: ClockStatus::UnresolvedNative,
        ..Default::default()
    })
}
fn source_blob(db: &Connection, id: i64, column: &str) -> Result<Option<Vec<u8>>> {
    db.query_row(
        &format!("SELECT {column} FROM ZSONG WHERE Z_PK=?1"),
        [id],
        |r| r.get(0),
    )
    .optional()
    .map(|v| v.flatten())
    .map_err(Into::into)
}
fn track(
    db: &Connection,
    aux: &Connection,
    id: i64,
    raw: Value,
    options: &ReadOptions,
) -> Result<TrackObservation> {
    let mut track = TrackObservation {
        external_id: format!("song:{id}"),
        ..Default::default()
    };
    if raw["missing_source_song"] == true {
        issue(&mut track, "missing_mik_song", "source");
    }
    for (column, target) in [
        ("ZNAME", "title"),
        ("ZARTIST", "artist"),
        ("ZALBUM", "album"),
        ("ZCOMPOSER", "composer"),
        ("ZGENRE", "genre"),
        ("ZGROUPING", "grouping"),
        ("ZCOMMENT", "comment"),
        ("ZLABEL", "label"),
        ("ZREMIXER", "remixer"),
        ("ZYEAR", "year"),
    ] {
        if let Some(value) = raw.get(column) {
            track.metadata.insert(target.into(), presence(value));
        }
    }
    if let Some(value) = raw.get("ZRATING") {
        track.metadata.insert(
            "rating".into(),
            if value.is_null() {
                Presence::Null
            } else {
                Presence::Value(json!({"raw":value,"scale":"unqualified_mik"}))
            },
        );
    }
    for (column, target) in [
        ("ZTAGKEY", "key"),
        ("ZTAGTEMPO", "bpm"),
        ("ZTAGENERGY", "energy"),
    ] {
        if let Some(value) = raw.get(column) {
            track.cached_tags.insert(target.into(), presence(value));
        }
    }
    if let Some(value) = raw.get("ZKEY") {
        track.key = Some(MusicalKey {
            presence: if value.is_null() {
                FieldPresence::Null
            } else {
                FieldPresence::Value
            },
            raw: value.as_str().map(str::to_owned),
            camelot: value.as_str().and_then(camelot),
            provenance: Some("MIK ZKEY".into()),
            ..Default::default()
        });
        if value.as_str() == Some("0") {
            issue(&mut track, "mik_default_key_unanalyzed", "key");
        }
    }
    if let Some(value) = raw.get("ZTEMPO") {
        track.bpm = Some(ScalarBpm {
            presence: if value.is_null() {
                FieldPresence::Null
            } else {
                FieldPresence::Value
            },
            value: value.as_f64().filter(|v| *v > 0.),
            provenance: Some("MIK ZTEMPO".into()),
            ..Default::default()
        });
        if value.as_f64() == Some(0.) {
            issue(&mut track, "mik_default_tempo_unanalyzed", "bpm");
        }
    }
    if let Some(value) = raw.get("ZENERGY") {
        track.energy = if value.is_null() {
            Presence::Null
        } else if value.as_f64().is_some_and(|v| v > 0.) {
            Presence::Value(value.clone())
        } else {
            Presence::Absent
        };
        if value.as_f64() == Some(0.) {
            issue(&mut track, "mik_default_energy_unanalyzed", "energy");
        }
    }
    for column in [
        "ZANALYSISDATE",
        "ZMODIFICATIONDATE",
        "ZSYNCDATE",
        "ZTAGUPDATEDATE",
    ] {
        if let Some(value) = raw[column].as_f64() {
            match keyed_archive::date_value(value) {
                Ok(date) => {
                    track.extensions.insert(column.to_owned(), date);
                }
                Err(_) => issue(&mut track, "invalid_mik_date", column),
            }
        }
    }
    if let Some(bytes) = source_blob(db, id, "ZBOOKMARKDATA")? {
        match bookmark::decode_embedded_bookmark_path(&bytes, &options.limits) {
            Ok(evidence) => {
                track.locations = LocationEvidence {
                    original: evidence.embedded_path.clone(),
                    platform: Some("macos".into()),
                    availability: "unresolved".into(),
                    ..Default::default()
                };
                let resolved = resolve_location(&track.locations, &options.path_mappings)?;
                track.issues.extend(resolved.issues);
                if let Some(path) = resolved.resolved {
                    track.locations.resolved = path.to_str().map(str::to_owned);
                    track.locations.availability = if path.is_file() {
                        "available"
                    } else {
                        "missing"
                    }
                    .into();
                }
                for code in &evidence.issues {
                    issue(&mut track, code, "location");
                }
                track
                    .locations
                    .extensions
                    .insert("bookmark_evidence".into(), serde_json::to_value(evidence)?);
            }
            Err(_) => issue(&mut track, "unresolved_mik_bookmark", "location"),
        }
    } else {
        issue(&mut track, "missing_mik_bookmark", "location");
    }
    if let Some(bytes) = source_blob(db, id, "ZBEATS")? {
        match grid(&bytes, options) {
            Ok(mut value) => {
                value.raw = serde_json::from_value(raw["ZBEATS"]["blob"].clone()).ok();
                track.grid = Some(value);
            }
            Err(_) => issue(&mut track, "unsupported_mik_beats_archive", "beatgrid"),
        }
    }
    if let Some(bytes) = source_blob(db, id, "ZTAGCOMMENTS")? {
        match decode_keyed_archive(&bytes, &options.limits).and_then(|graph| graph.to_json()) {
            Ok(comments) => {
                track
                    .cached_tags
                    .insert("comments".into(), presence(&comments));
            }
            Err(_) => issue(
                &mut track,
                "unsupported_mik_tag_comments_archive",
                "cached_tags",
            ),
        }
    }
    let mut last = None;
    for cue in children(aux, "ZCUEPOINT", id, options)? {
        let start = cue["ZTIME"].as_f64();
        if start.is_none() {
            issue(&mut track, "missing_mik_cue_time", "markers");
        }
        if start.is_some() && last == start {
            issue(&mut track, "mik_duplicate_cue_time", "markers");
        }
        last = start;
        track.cues.push(CueObservation {
            external_id: format!("cue:{}", cue["Z_PK"]),
            family: CueFamily::Marker,
            name: cue["ZNAME"].as_str().map(str::to_owned),
            native_start_seconds: start,
            clock_status: ClockStatus::UnresolvedNative,
            original: cue,
            ..Default::default()
        });
    }
    for (kind, target) in [
        ("ZENERGYSEGMENT", "source_energy_segments"),
        ("ZKEYSEGMENT", "source_key_segments"),
    ] {
        track.extensions.insert(
            target.into(),
            json!({"units":"inferred_seconds","rows":children(aux,kind,id,options)?}),
        );
    }
    track.extensions.insert("source_row".into(), raw);
    Ok(track)
}
fn collection_parent(
    aux: &Connection,
    id: i64,
    options: &ReadOptions,
) -> Result<(Option<String>, Option<&'static str>)> {
    let parent: Option<i64> = aux.query_row(
        "SELECT parent FROM records WHERE kind='ZCOLLECTION' AND id=?1",
        [id],
        |r| r.get(0),
    )?;
    let mut next = parent;
    let mut seen = BTreeSet::from([id]);
    while let Some(current) = next {
        if !seen.insert(current) {
            return Ok((None, Some("mik_collection_cycle")));
        }
        if seen.len() > options.limits.hierarchy_depth {
            return Ok((None, Some("mik_collection_depth_limit")));
        }
        next = aux
            .query_row(
                "SELECT parent FROM records WHERE kind='ZCOLLECTION' AND id=?1",
                [current],
                |r| r.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten();
    }
    Ok((parent.map(|v| format!("collection:{v}")), None))
}
impl SourceAdapter for MixedInKey11Reader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::MixedInKey11
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
        let (db, profile, uuid) = open(snapshot, options)?;
        let temp = tempfile::tempdir()?;
        let aux = Connection::open(temp.path().join("mik-import.sqlite"))?;
        aux.pragma_update(None, "cache_size", -2048)?;
        let transaction = aux.unchecked_transaction()?;
        stage(&db, &transaction, options, sink, cancel)?;
        transaction.commit()?;
        let tracks: i64 =
            aux.query_row("SELECT count(*) FROM records WHERE kind='ZSONG'", [], |r| {
                r.get(0)
            })?;
        let collections: i64 = aux.query_row(
            "SELECT count(*) FROM records WHERE kind='ZCOLLECTION'",
            [],
            |r| r.get(0),
        )?;
        let members: i64 = aux.query_row("SELECT count(*) FROM members", [], |r| r.get(0))?;
        let mut header = header(
            snapshot,
            options,
            &profile,
            Some(&uuid),
            tracks as u64,
            collections as u64,
            members as u64,
        );
        header.product_version = "MIK 11.2.6.7446 macOS-origin structural profile".into();
        header
            .extensions
            .insert("native_store_uuid".into(), json!(uuid));
        header.extensions.insert(
            "model_qualification".into(),
            json!(profile.origin["model_qualification"]),
        );
        sink.push(ObservationRecord::Issue(profile.issues[0].clone()))?;
        for kind in ["ZSONG", "ZCOLLECTION"] {
            let mut statement =
                aux.prepare("SELECT id,raw FROM records WHERE kind=?1 ORDER BY ordering,id")?;
            let mut rows = statement.query([kind])?;
            while let Some(row) = rows.next()? {
                cancel.check()?;
                let id: i64 = row.get(0)?;
                let raw: Value = serde_json::from_str(&row.get::<_, String>(1)?)?;
                if kind == "ZSONG" {
                    emit_track(track(&db, &aux, id, raw, options)?, options, sink, cancel)?;
                } else {
                    let external_id = format!("collection:{id}");
                    let (parent_id, problem) = collection_parent(&aux, id, options)?;
                    for code in problem.into_iter().chain(
                        (raw["missing_source_collection"] == true)
                            .then_some("missing_mik_collection"),
                    ) {
                        sink.push(ObservationRecord::Issue(ImportIssue {
                            code: code.into(),
                            record_id: Some(external_id.clone()),
                            field: Some("parent".into()),
                            message: code.replace('_', " "),
                        }))?;
                    }
                    let library = raw["ZISLIBRARY"].as_i64() == Some(1);
                    let folder = raw["ZISFOLDER"].as_i64() == Some(1);
                    let mut extensions = BTreeMap::from([
                        (
                            "source_kind".into(),
                            json!(if library {
                                "library"
                            } else if folder {
                                "folder"
                            } else {
                                "playlist"
                            }),
                        ),
                        ("source_row".into(), raw.clone()),
                        ("display_order".into(), raw["ZINDEX"].clone()),
                    ]);
                    if raw["missing_source_collection"] == true {
                        extensions.insert("placeholder".into(), json!(true));
                    }
                    sink.push(ObservationRecord::Collection(CollectionObservation {
                        external_id,
                        parent_id,
                        kind: if library || folder {
                            CollectionKind::Folder
                        } else {
                            CollectionKind::Playlist
                        },
                        name: raw["ZNAME"].as_str().unwrap_or("").into(),
                        extensions,
                        ..Default::default()
                    }))?;
                }
            }
        }
        let mut statement = aux.prepare(
            "SELECT collection,song,position,raw FROM members ORDER BY collection,position,song",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let collection: i64 = row.get(0)?;
            let song: i64 = row.get(1)?;
            sink.push(ObservationRecord::Membership {
                collection_id: format!("collection:{collection}"),
                occurrence: MembershipObservation {
                    external_occurrence_id: format!("membership:{collection}:{song}"),
                    track_external_id: format!("song:{song}"),
                    position: row.get(2)?,
                    extensions: BTreeMap::from([(
                        "source_row".into(),
                        serde_json::from_str(&row.get::<_, String>(3)?)?,
                    )]),
                },
            })?;
        }
        // Nullable unlinked children still have their source evidence and an explicit accounting issue.
        let mut orphan = aux.prepare("SELECT kind,id,raw FROM records WHERE kind IN('ZCUEPOINT','ZKEYSEGMENT','ZENERGYSEGMENT') AND parent IS NULL ORDER BY kind,id")?;
        let mut rows = orphan.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let kind: String = row.get(0)?;
            let id: i64 = row.get(1)?;
            sink.put_blob(
                row.get::<_, String>(2)?.as_bytes(),
                Some("application/json"),
            )?;
            sink.push(ObservationRecord::Issue(ImportIssue {
                code: "unlinked_mik_child".into(),
                record_id: Some(format!("{kind}:{id}")),
                field: Some("source".into()),
                message: "Source child row has no song relationship; raw observation retained"
                    .into(),
            }))?;
            header.accounting.unrepresented_records += 1;
        }
        if header.accounting.unrepresented_records > 0 {
            header.completeness = Completeness::PartialWithErrors;
        }
        snapshot.validate_source(cancel)?;
        Ok(header)
    }
}
