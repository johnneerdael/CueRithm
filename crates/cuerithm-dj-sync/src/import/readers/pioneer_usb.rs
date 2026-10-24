//! Device-root observations only; importing never copies media or repairs references.
use super::*;
use crate::pioneer_usb::{database, pdb};
use cuerithm_rekordbox::anlz::{AnlzFile, CueEntry, ParseLimits};
use rusqlite::{Connection, OptionalExtension, limits::Limit, params, types::ValueRef};
use std::collections::BTreeSet;
pub struct PioneerUsbReader;
fn artifact<'a>(snapshot: &'a ImportSnapshot, name: &str) -> Result<Option<&'a SnapshotArtifact>> {
    let mut found = None;
    for a in &snapshot.artifacts {
        if snapshot
            .original_artifact_path(&a.id)?
            .file_name()
            .is_some_and(|n| n == name)
        {
            ensure!(
                found.is_none(),
                "Choose one coherent device catalog per representation"
            );
            found = Some(a);
        }
    }
    Ok(found)
}
fn schema(db: &Connection) -> Result<Value> {
    let mut tables = serde_json::Map::new();
    for name in [
        "property",
        "content",
        "artist",
        "album",
        "genre",
        "label",
        "key",
        "color",
        "image",
        "cue",
        "playlist",
        "playlist_content",
        "history",
        "history_content",
        "myTag",
        "myTag_content",
    ] {
        let shape: Option<(String, String)> = db
            .query_row(
                "SELECT type,sql FROM sqlite_master WHERE name=?1",
                [name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((kind, sql)) = shape {
            ensure!(
                kind == "table" && !sql.to_ascii_uppercase().contains("VIRTUAL TABLE"),
                "Device catalog requires ordinary music tables"
            );
            let columns=db.prepare(&format!("PRAGMA table_info(\"{name}\")"))?.query_map([],|r|Ok(json!({"name":r.get::<_,String>(1)?,"type":r.get::<_,String>(2)?,"primary":r.get::<_,i64>(5)?})))?.collect::<rusqlite::Result<Vec<_>>>()?;
            tables.insert(name.into(), json!(columns));
        }
    }
    ensure!(
        tables.contains_key("property") && tables.contains_key("content"),
        "Unsupported device content profile"
    );
    Ok(Value::Object(tables))
}
fn inspect(snapshot: &ImportSnapshot, limits: &ImportLimits) -> Result<(SourceProfile, bool)> {
    ensure!(
        snapshot.provider == SourceProvider::PioneerUsb,
        "Mismatched device provider"
    );
    let sql = artifact(snapshot, "exportLibrary.db")?;
    let (mut profile, fingerprint) = if let Some(sql) = sql {
        ensure!(
            matches!(sql.kind, ArtifactKind::Sqlite),
            "Device SQL requires coherent SQLite acquisition"
        );
        let db = database::open_readonly(snapshot.artifact(&sql.id)?)?;
        let shape = schema(&db)?;
        (
            profile(
                SourceProvider::PioneerUsb,
                "pioneer-onelibrary-sqlcipher4-1000",
                &[
                    "descriptive_tags",
                    "key",
                    "bpm",
                    "ratings",
                    "color",
                    "beatgrid",
                    "hot_cues",
                    "memory_cues",
                    "loops",
                    "collections",
                ],
            ),
            digest(&serde_json::to_vec(&shape)?),
        )
    } else {
        let source = artifact(snapshot, "export.pdb")?
            .context("Choose exportLibrary.db or export.pdb under the selected device root")?;
        ensure!(
            matches!(source.kind, ArtifactKind::StableFile),
            "PDB requires a stable file snapshot"
        );
        let header = pdb::inspect_header(snapshot.artifact(&source.id)?, limits.snapshot_bytes)?;
        (
            profile(
                SourceProvider::PioneerUsb,
                "pioneer-pdb-linked-pages-v1",
                &[
                    "descriptive_tags",
                    "key",
                    "bpm",
                    "ratings",
                    "color",
                    "beatgrid",
                    "hot_cues",
                    "memory_cues",
                    "loops",
                    "collections",
                ],
            ),
            digest(&serde_json::to_vec(
                &json!({"page_size":header.page_size,"tables":header.table_types}),
            )?),
        )
    };
    profile.schema_fingerprint = fingerprint;
    profile.origin.insert("catalog_precedence".into(),"Accepted version1000 SQLite before PDB; alternative catalog fingerprint retained, no cross-catalog field merge".into());
    profile.origin.insert("sql_cue_kind_policy".into(),"Existing native writer contract:0 memory, positiveN hot slotN-1; alternative producer/device acceptance unrun".into());
    Ok((profile, sql.is_some()))
}
fn id(value: &Value) -> Option<String> {
    match value {
        Value::String(v) if !v.is_empty() => Some(v.clone()),
        Value::Number(v) => v
            .as_i64()
            .map(|n| n.to_string())
            .or_else(|| v.as_u64().map(|n| n.to_string())),
        _ => None,
    }
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
fn get(db: &Connection, kind: &str, id: &str) -> Result<Option<Value>> {
    db.query_row(
        "SELECT raw FROM records WHERE kind=?1 AND id=?2",
        params![kind, id],
        |r| r.get::<_, String>(0),
    )
    .optional()?
    .map(|s| serde_json::from_str(&s).map_err(Into::into))
    .transpose()
}
fn put(db: &Connection, kind: &str, record_id: &str, raw: &Value) -> Result<()> {
    let parent = match kind {
        "playlist" => id(&raw["playlist_id_parent"]),
        "history" => id(&raw["history_id_parent"]),
        "playlist_content" => id(&raw["playlist_id"]),
        "history_content" => id(&raw["history_id"]),
        "cue" | "myTag_content" => id(&raw["content_id"]),
        _ => None,
    };
    db.execute(
        "INSERT INTO records VALUES(?1,?2,?3,?4,?5)",
        params![
            kind,
            record_id,
            parent,
            raw["sequenceNo"].as_f64(),
            serde_json::to_string(raw)?
        ],
    )?;
    Ok(())
}
struct Pages {
    rows: Vec<Value>,
    bytes: usize,
    refs: Vec<BlobRef>,
}
impl Pages {
    fn new() -> Self {
        Self {
            rows: vec![],
            bytes: 0,
            refs: vec![],
        }
    }
    fn flush(&mut self, sink: &mut dyn ObservationSink) -> Result<()> {
        if !self.rows.is_empty() {
            self.refs
                .push(sink.put_blob(&serde_json::to_vec(&self.rows)?, Some("application/json"))?);
            self.rows.clear();
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
        let bytes = serde_json::to_vec(&value)?.len();
        ensure!(
            bytes as u64 <= options.limits.text_bytes,
            "Device record size limit"
        );
        if self.rows.len() >= options.limits.page_size.max(1)
            || self.bytes + bytes > options.limits.text_bytes as usize
        {
            self.flush(sink)?;
        }
        self.bytes += bytes;
        self.rows.push(value);
        Ok(())
    }
}
fn sql_rows(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
    aux: &Connection,
    pages: &mut Pages,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<()> {
    let source = artifact(snapshot, "exportLibrary.db")?.context("Missing SQL catalog")?;
    let db = database::open_readonly(snapshot.artifact(&source.id)?)?;
    db.set_limit(
        Limit::SQLITE_LIMIT_LENGTH,
        options
            .limits
            .blob_bytes
            .max(options.limits.text_bytes)
            .min(i32::MAX as u64) as i32,
    )?;
    let shape = schema(&db)?;
    let mut total = 0usize;
    for name in shape.as_object().context("Invalid device schema")?.keys() {
        if !options.include_history && matches!(name.as_str(), "history" | "history_content") {
            continue;
        }
        let mut statement =
            db.prepare(&format!("SELECT rowid,* FROM \"{name}\" ORDER BY rowid"))?;
        let names = statement
            .column_names()
            .into_iter()
            .skip(1)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            total += 1;
            ensure!(total <= 2_000_000, "Device row limit");
            let rowid: i64 = row.get(0)?;
            let mut raw = serde_json::Map::new();
            for (i, name) in names.iter().enumerate() {
                let value = match row.get_ref(i + 1)? {
                    ValueRef::Blob(bytes) => {
                        ensure!(
                            bytes.len() as u64 <= options.limits.blob_bytes,
                            "Device blob limit"
                        );
                        json!({"blob":sink.put_blob(bytes,None)?})
                    }
                    value => {
                        if let ValueRef::Text(bytes) = value {
                            ensure!(
                                bytes.len() as u64 <= options.limits.text_bytes,
                                "Device text limit"
                            );
                        }
                        super::mixxx_schema::scalar(value)?
                    }
                };
                raw.insert(name.clone(), value);
            }
            let raw = Value::Object(raw);
            let record_id = if matches!(
                name.as_str(),
                "playlist_content" | "history_content" | "myTag_content" | "property"
            ) {
                format!("sql:{rowid}")
            } else {
                id(&raw[format!("{name}_id")]).context("Device row lacks native identity")?
            };
            put(aux, name, &record_id, &raw)?;
            pages.add(
                json!({"representation":"sql","table":name,"id":record_id,"raw":raw}),
                options,
                sink,
            )?;
        }
    }
    Ok(())
}
fn pdb_rows(
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
    aux: &Connection,
    pages: &mut Pages,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<(BlobRef, u64)> {
    let source = artifact(snapshot, "export.pdb")?.context("Missing PDB catalog")?;
    let mut unrepresented = 0;
    let header = pdb::visit_records(
        snapshot.artifact(&source.id)?,
        options.limits.snapshot_bytes,
        cancel,
        |row| {
            if !options.include_history && matches!(row.kind, 11 | 12) {
                return Ok(());
            }
            let identity = format!("pdb:{}:{}:{}", row.kind, row.page, row.slot);
            let original = sink.put_blob(row.bytes, None)?;
            let decoded = super::pioneer_pdb::decode(row.kind, row.bytes, &identity);
            match decoded {
                Ok(Some(mut parsed)) => {
                    parsed
                        .raw
                        .as_object_mut()
                        .context("Invalid PDB row")?
                        .insert("raw_record".into(), serde_json::to_value(&original)?);
                    put(aux, parsed.table, &parsed.id, &parsed.raw)?;
                    pages.add(json!({"representation":"pdb","physical_id":identity,"table":parsed.table,"id":parsed.id,"raw":parsed.raw}),options,sink)?;
                }
                other => {
                    pages.add(json!({"representation":"pdb","physical_id":identity,"table_type":row.kind,"raw_record":original}),options,sink)?;
                    if other.is_err() || !matches!(row.kind, 16..=19) {
                        unrepresented += 1;
                        sink.push(ObservationRecord::Issue(ImportIssue {
                            code: "unsupported_usb_pdb_record".into(),
                            record_id: Some(identity),
                            field: Some("source".into()),
                            message: "Unsupported device record remains raw provenance".into(),
                        }))?;
                    }
                }
            }
            Ok(())
        },
    )?;
    Ok((sink.put_blob(&header.raw, None)?, unrepresented))
}
fn children(
    aux: &Connection,
    kind: &str,
    parent: &str,
    options: &ReadOptions,
) -> Result<Vec<Value>> {
    let mut statement =
        aux.prepare("SELECT raw FROM records WHERE kind=?1 AND parent=?2 ORDER BY ordering,id")?;
    let mut rows = statement.query(params![kind, parent])?;
    let mut result = Vec::new();
    let mut bytes = 0u64;
    while let Some(row) = rows.next()? {
        let raw: String = row.get(0)?;
        bytes += raw.len() as u64;
        ensure!(
            bytes <= options.limits.blob_bytes && result.len() < 100_000,
            "Device child observation limit"
        );
        result.push(serde_json::from_str(&raw)?);
    }
    Ok(result)
}
fn foreign(
    aux: &Connection,
    raw: &Value,
    column: &str,
    kind: &str,
    track: &mut TrackObservation,
) -> Result<Presence<Value>> {
    let Some(value) = raw.get(column) else {
        return Ok(Presence::Absent);
    };
    if value.is_null() {
        return Ok(Presence::Null);
    }
    let Some(id) = id(value) else {
        issue(track, "invalid_usb_foreign_id", column);
        return Ok(Presence::Absent);
    };
    if let Some(row) = get(aux, kind, &id)? {
        if let Some(name) = row.get("name") {
            return Ok(presence(name));
        }
        issue(track, "unsupported_usb_lookup", column);
    } else if id != "0" {
        issue(track, "missing_usb_lookup", column);
    }
    Ok(Presence::Absent)
}
fn device_location(
    raw: &str,
    snapshot: &ImportSnapshot,
    options: &ReadOptions,
) -> Result<LocationEvidence> {
    ensure!(
        !raw.contains(['\\', '\0']) && !raw.starts_with("//") && !raw.contains("://"),
        "Unsafe device-relative media reference"
    );
    let relative = Path::new(raw.trim_start_matches('/'));
    ensure!(
        !relative.as_os_str().is_empty()
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "Unsafe device-relative media path"
    );
    let native = snapshot.source_root().join(relative);
    if native.exists() {
        ensure!(
            native.canonicalize()?.starts_with(snapshot.source_root()),
            "Device media symlink escapes selected root"
        );
    }
    let mut evidence = LocationEvidence {
        original: Some(raw.into()),
        native_path: Some(native),
        platform: Some(std::env::consts::OS.into()),
        availability: "unresolved".into(),
        ..Default::default()
    };
    let resolved = resolve_location(&evidence, &options.path_mappings)?;
    if let Some(path) = resolved.resolved {
        evidence.resolved = path.to_str().map(str::to_owned);
        evidence.availability = if path.is_file() {
            "available"
        } else {
            "missing"
        }
        .into();
    }
    Ok(evidence)
}
fn cue_entry(entry: &CueEntry, artifact: &str, index: usize) -> CueObservation {
    let slot = entry
        .hot_cue
        .checked_sub(1)
        .and_then(|n| i32::try_from(n).ok());
    let name = entry.extended.as_ref().and_then(|e| e.comment.clone());
    let color = entry
        .extended
        .as_ref()
        .and_then(|e| e.color)
        .map(|c| (u32::from(c[1]) << 16) | (u32::from(c[2]) << 8) | u32::from(c[3]));
    CueObservation {
        external_id: format!("{artifact}:cue:{index}"),
        family: if entry.hot_cue == 0 {
            CueFamily::Memory
        } else if slot.is_some() {
            CueFamily::Hot
        } else {
            CueFamily::Unknown
        },
        source_slot: slot,
        name,
        color_rgb: color,
        original: json!({"hot_cue":entry.hot_cue,"cue_type":entry.cue_type,"time_ms":entry.time_ms,"loop_time_ms":entry.loop_time_ms,"legacy_status":entry.legacy.as_ref().map(|e|e.status),"legacy_order":entry.legacy.as_ref().map(|e|[e.order_first,e.order_last]),"memory_color":entry.extended.as_ref().and_then(|e|e.memory_color),"loop_fraction":entry.extended.as_ref().and_then(|e|e.loop_fraction),"color":entry.extended.as_ref().and_then(|e|e.color)}),
        native_start_seconds: Some(f64::from(entry.time_ms) / 1000.),
        native_end_seconds: (entry.cue_type == 2 && entry.loop_time_ms != u32::MAX)
            .then_some(f64::from(entry.loop_time_ms) / 1000.),
        clock_status: ClockStatus::UnresolvedNative,
        ..Default::default()
    }
}
fn performance(
    snapshot: &ImportSnapshot,
    raw: &Value,
    options: &ReadOptions,
    track: &mut TrackObservation,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<()> {
    let Some(reference) = raw["analysisDataFilePath"]
        .as_str()
        .filter(|s| !s.is_empty())
    else {
        issue(track, "missing_usb_analysis_reference", "beatgrid");
        track
            .extensions
            .insert("hotcue_occupancy".into(), json!("unknown"));
        return Ok(());
    };
    let Some(base) = reference.strip_suffix(".DAT") else {
        issue(track, "unsupported_usb_analysis_reference", "beatgrid");
        track
            .extensions
            .insert("hotcue_occupancy".into(), json!("unknown"));
        return Ok(());
    };
    let mut layers = Vec::new();
    let mut fallback_dat = None;
    let mut fallback_ext = None;
    let mut unknown = false;
    for suffix in ["DAT", "EXT", "2EX"] {
        let expected = format!("{base}.{suffix}");
        let relative = Path::new(expected.trim_start_matches('/'));
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            issue(track, "unsafe_usb_analysis_reference", "beatgrid");
            unknown = true;
            continue;
        }
        let found = snapshot.artifacts.iter().find(|a| {
            matches!(a.kind, ArtifactKind::StableFile)
                && snapshot
                    .original_artifact_path(&a.id)
                    .is_ok_and(|p| p == snapshot.source_root().join(relative))
        });
        let Some(found) = found else {
            if suffix != "2EX" {
                issue(
                    track,
                    &format!("missing_usb_analysis_{}", suffix.to_ascii_lowercase()),
                    "performance",
                );
                unknown = true;
            }
            continue;
        };
        cancel.check()?;
        ensure!(
            found.bytes <= options.limits.blob_bytes,
            "Device ANLZ byte limit"
        );
        let data = fs::read(snapshot.artifact(&found.id)?)?;
        let blob = sink.put_blob(&data, None)?;
        let parsed = AnlzFile::parse(
            &data,
            ParseLimits {
                max_bytes: usize::try_from(options.limits.blob_bytes)?,
                ..Default::default()
            },
        );
        let mut summaries = Vec::new();
        let mut cues = Vec::new();
        let mut has_cues = false;
        match parsed {
            Ok(file) => {
                for tag in file.tags() {
                    let kind = tag.fourcc();
                    summaries.push(
                        json!({"tag":String::from_utf8_lossy(&kind),"bytes":tag.bytes().len()}),
                    );
                    match &kind {
                        b"PQTZ" if track.grid.is_none() => match tag.beats() {
                            Ok(beats) => {
                                track.grid=Some(GridObservation{representation:"pioneer_pqtz_explicit".into(),original_time_base:Some("milliseconds".into()),native_first_beat_seconds:beats.first().map(|b|f64::from(b.time_ms)/1000.),bpm:beats.first().map(|b|f64::from(b.tempo)/100.),downbeat_known:beats.iter().all(|b|(1..=4).contains(&b.number)),clock_status:ClockStatus::UnresolvedNative,raw:Some(blob.clone()),points:beats.into_iter().map(|b|GridPoint{native_seconds:f64::from(b.time_ms)/1000.,bpm:Some(f64::from(b.tempo)/100.),original:json!({"number":b.number,"tempo":b.tempo,"time_ms":b.time_ms}),..Default::default()}).collect(),..Default::default()});
                            }
                            Err(_) => {
                                issue(track, "unsupported_usb_beatgrid", "beatgrid");
                                unknown = true;
                            }
                        },
                        b"PCOB" | b"PCO2" => match tag.cues() {
                            Ok(list) => {
                                has_cues = true;
                                for entry in list.entries {
                                    if !matches!(entry.cue_type, 1 | 2) {
                                        unknown = true;
                                        issue(track, "unknown_usb_cue_type", "cues");
                                    }
                                    let mut value = cue_entry(&entry, &found.id, cues.len());
                                    value.original["list_kind"] = json!(list.kind);
                                    value.extensions.insert(
                                        "raw_record".into(),
                                        serde_json::to_value(sink.put_blob(&entry.raw, None)?)?,
                                    );
                                    cues.push(value);
                                }
                            }
                            Err(_) => {
                                issue(track, "unsupported_usb_cues", "cues");
                                unknown = true;
                            }
                        },
                        _ => {
                            if kind.starts_with(b"PC") {
                                unknown = true;
                                issue(track, "unknown_usb_cue_tag", "cues");
                            }
                        }
                    }
                }
            }
            Err(_) => {
                issue(track, "unsupported_usb_analysis", "performance");
                unknown = true;
            }
        }
        layers
            .push(json!({"artifact":found.id,"raw":blob,"tags":summaries,"cue_count":cues.len()}));
        if has_cues {
            if suffix == "EXT" {
                fallback_ext = Some(cues);
            } else if suffix == "DAT" {
                fallback_dat = Some(cues);
            }
        }
    }
    if track.cues.is_empty() {
        if let Some(cues) = fallback_ext.or(fallback_dat) {
            track.cues = cues;
        } else {
            unknown = true;
        }
    } else if fallback_ext.is_some() || fallback_dat.is_some() {
        issue(track, "usb_sql_cues_precede_anlz", "cues");
    }
    track
        .extensions
        .insert("performance_layers".into(), json!(layers));
    if unknown {
        track
            .extensions
            .insert("hotcue_occupancy".into(), json!("unknown"));
    }
    Ok(())
}
struct TrackContext<'a> {
    snapshot: &'a ImportSnapshot,
    options: &'a ReadOptions,
    aux: &'a Connection,
    cancel: &'a CancellationToken,
}
impl TrackContext<'_> {
    fn project(
        &self,
        id: &str,
        raw: Value,
        sink: &mut dyn ObservationSink,
    ) -> Result<TrackObservation> {
        let mut track = TrackObservation {
            external_id: format!("track:{id}"),
            ..Default::default()
        };
        for (source, target) in [
            ("title", "title"),
            ("subtitle", "version"),
            ("djComment", "comment"),
            ("releaseYear", "year"),
            ("releaseDate", "release_date"),
            ("dateAdded", "date_added"),
            ("isrc", "isrc"),
            ("trackNo", "track_number"),
            ("discNo", "disc_number"),
        ] {
            if let Some(value) = raw.get(source) {
                track.metadata.insert(target.into(), presence(value));
            }
        }
        for (source, table, target) in [
            ("artist_id_artist", "artist", "artist"),
            ("artist_id_remixer", "artist", "remixer"),
            ("artist_id_composer", "artist", "composer"),
            ("artist_id_originalArtist", "artist", "original_artist"),
            ("album_id", "album", "album"),
            ("genre_id", "genre", "genre"),
            ("label_id", "label", "label"),
        ] {
            let value = foreign(self.aux, &raw, source, table, &mut track)?;
            track.metadata.insert(target.into(), value);
        }
        let key = foreign(self.aux, &raw, "key_id", "key", &mut track)?;
        if !matches!(key, Presence::Absent) {
            let text = match &key {
                Presence::Value(v) | Presence::Empty(v) => v.as_str().map(str::to_owned),
                _ => None,
            };
            track.key = Some(MusicalKey {
                presence: if matches!(key, Presence::Null) {
                    FieldPresence::Null
                } else if matches!(key, Presence::Empty(_)) {
                    FieldPresence::Empty
                } else {
                    FieldPresence::Value
                },
                camelot: text
                    .as_deref()
                    .and_then(|s| crate::formatting::parse_key(s).ok())
                    .map(|k| k.camelot()),
                raw: text,
                provenance: Some("device key lookup".into()),
                ..Default::default()
            });
        }
        if let Some(value) = raw.get("bpmx100") {
            track.bpm = Some(ScalarBpm {
                presence: if value.is_null() {
                    FieldPresence::Null
                } else {
                    FieldPresence::Value
                },
                value: value.as_f64().filter(|n| *n > 0.).map(|n| n / 100.),
                provenance: Some("device bpmx100".into()),
                ..Default::default()
            });
        }
        for (source, scale) in [("rating", json!(5)), ("color_id", json!("pioneer_palette"))] {
            if let Some(value) = raw.get(source) {
                track.metadata.insert(
                    if source == "rating" {
                        "rating"
                    } else {
                        "color"
                    }
                    .into(),
                    if value.is_null() {
                        Presence::Null
                    } else {
                        Presence::Value(json!({"raw":value,"scale":scale}))
                    },
                );
            }
        }
        if let Some(path) = raw["path"].as_str().filter(|s| !s.is_empty()) {
            match device_location(path, self.snapshot, self.options) {
                Ok(location) => track.locations = location,
                Err(_) => issue(&mut track, "unsafe_usb_media_reference", "location"),
            }
        } else {
            issue(&mut track, "missing_usb_media_reference", "location");
        }
        if track.locations.availability != "available" {
            issue(&mut track, "unavailable_usb_media", "location");
        }
        for cue in children(self.aux, "cue", id, self.options)? {
            let kind = cue["kind"].as_i64();
            let slot = kind
                .filter(|k| *k > 0)
                .and_then(|k| i32::try_from(k - 1).ok());
            let start = cue["inUsec"].as_f64().map(|n| n / 1e6);
            let end = cue["outUsec"]
                .as_f64()
                .filter(|n| *n != -1.)
                .map(|n| n / 1e6);
            let family = if kind == Some(0) {
                CueFamily::Memory
            } else if slot.is_some() {
                CueFamily::Hot
            } else {
                issue(&mut track, "unknown_usb_sql_cue_kind", "cues");
                track
                    .extensions
                    .insert("hotcue_occupancy".into(), json!("unknown"));
                CueFamily::Unknown
            };
            let mut flags = BTreeMap::new();
            if let Some(active) = cue["isActiveLoop"].as_i64() {
                flags.insert("active_loop".into(), json!(active != 0));
            }
            track.cues.push(CueObservation {
                external_id: format!("sql-cue:{}", cue["cue_id"]),
                family,
                source_slot: slot,
                name: cue["cueComment"].as_str().map(str::to_owned),
                native_start_seconds: start,
                native_end_seconds: end,
                clock_status: ClockStatus::UnresolvedNative,
                flags,
                original: cue,
                ..Default::default()
            });
        }
        if let Some(issues) = raw["parse_issues"].as_array() {
            for code in issues.iter().filter_map(Value::as_str) {
                issue(&mut track, code, "source");
            }
        }
        if raw["missing_source_track"] == true {
            issue(&mut track, "missing_usb_track", "source");
        }
        performance(
            self.snapshot,
            &raw,
            self.options,
            &mut track,
            sink,
            self.cancel,
        )?;
        if let Some(duration) = raw["length"].as_f64().filter(|n| *n >= 0.) {
            track
                .extensions
                .insert("duration_seconds".into(), json!(duration));
        }
        track.extensions.insert("native_identity".into(),json!({"content_id":id,"master_db_id":raw["masterDbId"],"master_content_id":raw["masterContentId"]}));
        track.extensions.insert("source_row".into(), raw);
        Ok(track)
    }
}
fn parent(
    aux: &Connection,
    kind: &str,
    id: &str,
    options: &ReadOptions,
) -> Result<(Option<String>, bool)> {
    let first: Option<String> = aux.query_row(
        "SELECT parent FROM records WHERE kind=?1 AND id=?2",
        params![kind, id],
        |r| r.get(0),
    )?;
    let first = first.filter(|p| p != "0" && !p.is_empty());
    let mut seen = BTreeSet::from([id.to_owned()]);
    let mut next = first.clone();
    while let Some(id) = next {
        if !seen.insert(id.clone()) || seen.len() > options.limits.hierarchy_depth {
            return Ok((None, true));
        }
        next = aux
            .query_row(
                "SELECT parent FROM records WHERE kind=?1 AND id=?2",
                params![kind, id],
                |r| r.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .filter(|p| p != "0" && !p.is_empty());
    }
    Ok((first.map(|p| format!("{kind}:{p}")), false))
}
impl SourceAdapter for PioneerUsbReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::PioneerUsb
    }
    fn capture(
        &self,
        request: &SourceCaptureRequest,
        workspace: &Path,
        limits: &ImportLimits,
        cancel: &CancellationToken,
    ) -> Result<ImportSnapshot> {
        ensure!(
            request.provider == self.provider(),
            "Mismatched device capture provider"
        );
        super::super::snapshot::capture_source_with_sqlite(
            request,
            workspace,
            limits,
            cancel,
            |input, output, limits, cancel| {
                ensure!(
                    input.file_name().is_some_and(|n| n == "exportLibrary.db"),
                    "Unsupported device SQL artifact"
                );
                database::capture_readonly(input, output, limits.snapshot_bytes, cancel)
            },
        )
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        Ok(inspect(snapshot, &ImportLimits::default())?.0)
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
        let (profile, use_sql) = inspect(snapshot, &options.limits)?;
        let temp = tempfile::tempdir()?;
        let aux = Connection::open(temp.path().join("device-import.sqlite"))?;
        aux.execute_batch("PRAGMA cache_size=-2048;CREATE TABLE records(kind TEXT,id TEXT,parent TEXT,ordering REAL,raw TEXT,PRIMARY KEY(kind,id));CREATE INDEX parents ON records(kind,parent,ordering,id);BEGIN;")?;
        let mut pages = Pages::new();
        let (pdb_header, mut unrepresented) = if use_sql {
            sql_rows(snapshot, options, &aux, &mut pages, sink, cancel)?;
            (None, 0)
        } else {
            let (header, errors) = pdb_rows(snapshot, options, &aux, &mut pages, sink, cancel)?;
            (Some(header), errors)
        };
        pages.flush(sink)?;
        aux.execute_batch("INSERT OR IGNORE INTO records SELECT 'content',json_extract(raw,'$.content_id'),NULL,NULL,'{\"missing_source_track\":true}' FROM records WHERE kind IN('playlist_content','history_content','cue') AND json_extract(raw,'$.content_id') IS NOT NULL;INSERT OR IGNORE INTO records SELECT 'playlist',parent,NULL,NULL,'{\"missing_source_collection\":true}' FROM records WHERE kind IN('playlist','playlist_content') AND parent IS NOT NULL AND parent!='0';INSERT OR IGNORE INTO records SELECT 'history',parent,NULL,NULL,'{\"missing_source_collection\":true}' FROM records WHERE kind IN('history','history_content') AND parent IS NOT NULL AND parent!='0';COMMIT;")?;
        let tracks: i64 = aux.query_row(
            "SELECT count(*) FROM records WHERE kind='content'",
            [],
            |r| r.get(0),
        )?;
        let collections: i64 = aux.query_row(
            "SELECT count(*) FROM records WHERE kind IN('playlist','history')",
            [],
            |r| r.get(0),
        )?;
        let members:i64=aux.query_row("SELECT count(*) FROM records WHERE kind IN('playlist_content','history_content') AND parent IS NOT NULL AND json_extract(raw,'$.content_id') IS NOT NULL",[],|r|r.get(0))?;
        let mut header = header(
            snapshot,
            options,
            &profile,
            None,
            tracks as u64,
            collections as u64,
            members as u64,
        );
        header.product_version = if use_sql {
            "Device Library Plus/OneLibrary protocol1000"
        } else {
            "Pioneer paged PDB"
        }
        .into();
        header.extensions.insert(
            "catalog_representation".into(),
            json!(if use_sql { "onelibrary" } else { "pdb" }),
        );
        header.extensions.insert(
            "catalog_artifacts".into(),
            serde_json::to_value(
                snapshot
                    .artifacts
                    .iter()
                    .filter(|a| {
                        snapshot.original_artifact_path(&a.id).is_ok_and(|p| {
                            p.file_name()
                                .is_some_and(|n| n == "export.pdb" || n == "exportLibrary.db")
                        })
                    })
                    .collect::<Vec<_>>(),
            )?,
        );
        header
            .extensions
            .insert("raw_record_pages".into(), serde_json::to_value(pages.refs)?);
        if let Some(blob) = pdb_header {
            header
                .extensions
                .insert("raw_pdb_header".into(), serde_json::to_value(blob)?);
        }
        let context = TrackContext {
            snapshot,
            options,
            aux: &aux,
            cancel,
        };
        let mut statement =
            aux.prepare("SELECT id,raw FROM records WHERE kind='content' ORDER BY id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: String = row.get(0)?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(1)?)?;
            emit_track(context.project(&id, raw, sink)?, options, sink, cancel)?;
        }
        let mut statement=aux.prepare("SELECT kind,id,raw FROM records WHERE kind IN('playlist','history') ORDER BY kind,ordering,id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let kind: String = row.get(0)?;
            let id: String = row.get(1)?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(2)?)?;
            let (parent_id, cycle) = parent(&aux, &kind, &id, options)?;
            if cycle || raw["missing_source_collection"] == true {
                sink.push(ObservationRecord::Issue(ImportIssue {
                    code: if cycle {
                        "usb_collection_cycle"
                    } else {
                        "missing_usb_collection"
                    }
                    .into(),
                    record_id: Some(format!("{kind}:{id}")),
                    message: "Device collection relationship unresolved; source rows retained"
                        .into(),
                    ..Default::default()
                }))?;
            }
            sink.push(ObservationRecord::Collection(CollectionObservation {
                external_id: format!("{kind}:{id}"),
                parent_id,
                kind: if kind == "history" {
                    CollectionKind::History
                } else if raw["attribute"].as_i64() == Some(1) {
                    CollectionKind::Folder
                } else {
                    CollectionKind::Playlist
                },
                name: raw["name"].as_str().unwrap_or("").into(),
                extensions: BTreeMap::from([
                    ("source_row".into(), raw.clone()),
                    ("display_order".into(), raw["sequenceNo"].clone()),
                ]),
                ..Default::default()
            }))?;
        }
        let mut statement=aux.prepare("SELECT kind,id,parent,raw FROM records WHERE kind IN('playlist_content','history_content') AND parent IS NOT NULL AND json_extract(raw,'$.content_id') IS NOT NULL ORDER BY kind,parent,ordering,id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let kind: String = row.get(0)?;
            let raw: Value = serde_json::from_str(&row.get::<_, String>(3)?)?;
            let collection: String = row.get(2)?;
            let content = id(&raw["content_id"]).context("Missing device membership content")?;
            sink.push(ObservationRecord::Membership {
                collection_id: format!(
                    "{}:{collection}",
                    if kind == "history_content" {
                        "history"
                    } else {
                        "playlist"
                    }
                ),
                occurrence: MembershipObservation {
                    external_occurrence_id: row.get(1)?,
                    track_external_id: format!("track:{content}"),
                    position: raw["sequenceNo"].as_i64(),
                    extensions: BTreeMap::from([("source_row".into(), raw)]),
                },
            })?;
        }
        let mut statement=aux.prepare("SELECT kind,id FROM records WHERE kind IN('cue','playlist_content','history_content') AND (parent IS NULL OR json_extract(raw,'$.content_id') IS NULL)")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let kind: String = row.get(0)?;
            let id: String = row.get(1)?;
            sink.push(ObservationRecord::Issue(ImportIssue {
                code: "unlinked_usb_record".into(),
                record_id: Some(format!("{kind}:{id}")),
                message: "Unlinked device row remains in raw record pages".into(),
                ..Default::default()
            }))?;
            unrepresented += 1;
        }
        if unrepresented > 0 {
            header.completeness = Completeness::PartialWithErrors;
            header.accounting.unrepresented_records = unrepresented;
        }
        snapshot.validate_source(cancel)?;
        Ok(header)
    }
}
