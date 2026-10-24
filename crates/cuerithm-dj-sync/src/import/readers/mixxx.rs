use super::*;
use rusqlite::{Connection, Row, params, types::ValueRef};
pub struct MixxxReader;
fn cell(
    value: ValueRef<'_>,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
) -> Result<Value> {
    match value {
        ValueRef::Blob(bytes) => {
            ensure!(
                bytes.len() as u64 <= options.limits.blob_bytes,
                "Native blob byte limit"
            );
            Ok(json!({"blob":sink.put_blob(bytes,None)?}))
        }
        ValueRef::Text(bytes) => {
            ensure!(
                bytes.len() as u64 <= options.limits.text_bytes,
                "Native text byte limit"
            );
            mixxx_schema::scalar(value)
        }
        _ => mixxx_schema::scalar(value),
    }
}
fn raw_row(
    row: &Row<'_>,
    names: &[String],
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
) -> Result<Value> {
    let mut value = serde_json::Map::new();
    for (index, name) in names.iter().enumerate() {
        value.insert(name.clone(), cell(row.get_ref(index)?, options, sink)?);
    }
    Ok(Value::Object(value))
}
fn add_cues(track: &mut TrackObservation, aux: &Connection, rate: Option<f64>) -> Result<()> {
    let mut statement = aux.prepare("SELECT json FROM cue_rows WHERE track_id=?1 ORDER BY id")?;
    let numeric = track
        .external_id
        .parse::<i64>()
        .context("Invalid Mixxx track identity")?;
    let mut rows = statement.query([numeric])?;
    let mut slots = std::collections::BTreeSet::new();
    while let Some(row) = rows.next()? {
        let raw: Value = serde_json::from_str(&row.get::<_, String>(0)?)?;
        let kind = raw["type"].as_i64().context("Invalid native cue type")?;
        let slot = raw["hotcue"]
            .as_i64()
            .filter(|v| *v >= 0)
            .map(i32::try_from)
            .transpose()?;
        let position = raw["position"].as_f64();
        let length = raw["length"].as_f64();
        let start = position
            .filter(|p| *p != -1.)
            .zip(rate)
            .map(|(p, r)| p / (2. * r));
        let end = length
            .filter(|l| *l > 0.)
            .zip(rate)
            .map(|(l, r)| start.unwrap_or(0.) + l / (2. * r))
            .filter(|v| v.is_finite());
        let color = raw["color"]
            .as_u64()
            .filter(|v| *v <= 0xffffff)
            .map(|v| v as u32);
        if kind == 1
            && let Some(slot) = slot
            && !slots.insert(slot)
        {
            issue(track, "duplicate_hotcue_slot", "cues.hot");
        }
        if kind == 1 && position == Some(0.) && raw["color"].as_i64() == Some(0) {
            issue(track, "historical_black_zero_cue", "cues.hot");
        }
        if kind == 4 && (length.is_none_or(|l| l <= 0.) || end.is_none()) {
            issue(track, "invalid_loop_length", "cues.loop");
        }
        let family = match kind {
            1 => CueFamily::Hot,
            2 => CueFamily::Main,
            4 => CueFamily::Loop,
            6 => CueFamily::Intro,
            7 => CueFamily::Outro,
            8 => CueFamily::AudibleRange,
            _ => CueFamily::Marker,
        };
        let end = end.filter(|end| start.is_none_or(|start| *end >= start));
        track.cues.push(CueObservation {
            external_id: raw["id"].as_i64().context("Cue ID missing")?.to_string(),
            family,
            source_slot: slot,
            name: raw["label"].as_str().map(str::to_owned),
            color_rgb: color,
            native_start_seconds: start,
            native_end_seconds: end,
            flags: BTreeMap::from([
                ("native_type".into(), json!(kind)),
                (
                    "source_slot_role".into(),
                    json!(if slot.is_some() { "hot" } else { "unassigned" }),
                ),
            ]),
            original: raw,
            ..Default::default()
        });
    }
    Ok(())
}
fn missing(
    id: &str,
    aux: &Connection,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<bool> {
    if aux.execute("INSERT OR IGNORE INTO emitted VALUES(?1)", [id])? == 0 {
        return Ok(false);
    }
    let mut track = TrackObservation {
        external_id: id.into(),
        locations: LocationEvidence {
            availability: "unresolved".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    issue(&mut track, "missing_track_record", "location");
    if id.parse::<i64>().is_ok() {
        add_cues(&mut track, aux, None)?;
    }
    emit_track(track, options, sink, cancel)?;
    Ok(true)
}
fn convert(
    row: &Row<'_>,
    names: &[String],
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
) -> Result<TrackObservation> {
    let mut native = raw_row(row, names, options, sink)?;
    let value = native.as_object_mut().context("Invalid source row")?;
    let location = value.remove("_location").unwrap_or(Value::Null);
    let fs_deleted = value.remove("_fs_deleted").unwrap_or(Value::Null);
    let id = value["id"]
        .as_i64()
        .context("Mixxx track ID is not an integer")?
        .to_string();
    let mut track = TrackObservation {
        external_id: id,
        locations: match location.as_str() {
            Some(path) => location_fn(path, options)?,
            None => LocationEvidence {
                availability: "unresolved".into(),
                ..Default::default()
            },
        },
        ..Default::default()
    };
    if location.is_null() {
        issue(&mut track, "missing_location_record", "location");
    }
    track
        .locations
        .extensions
        .insert("fs_deleted".into(), fs_deleted);
    track
        .locations
        .extensions
        .insert("library_location".into(), value["location"].clone());
    for (source, target) in [
        ("artist", "artist"),
        ("title", "title"),
        ("album", "album"),
        ("album_artist", "album_artist"),
        ("year", "year"),
        ("genre", "genre"),
        ("composer", "composer"),
        ("grouping", "grouping"),
        ("tracknumber", "track_number"),
        ("tracktotal", "track_total"),
        ("comment", "comment"),
    ] {
        let data = &value[source];
        ensure!(
            !data.is_object(),
            "Unsupported native descriptive cell type"
        );
        track.metadata.insert(
            target.into(),
            if data.is_null() {
                Presence::Null
            } else {
                apple_music::presence(data)
            },
        );
    }
    if !value["rating"].is_null() {
        track.metadata.insert(
            "rating".into(),
            Presence::Value(json!({"raw":value["rating"],"scale_max":5})),
        );
    }
    if !value["color"].is_null() {
        track.metadata.insert(
            "color".into(),
            Presence::Value(json!({"raw":value["color"],"epoch":"rgb24-schema39-40"})),
        );
    }
    if options.include_history {
        for (source, target) in [
            ("timesplayed", "play_count"),
            ("last_played_at", "last_played"),
        ] {
            if !value[source].is_null() {
                track
                    .metadata
                    .insert(target.into(), Presence::Value(value[source].clone()));
            }
        }
    }
    if let Some(duration) = value["duration"].as_f64().filter(|v| *v >= 0.) {
        track
            .extensions
            .insert("duration_seconds".into(), json!(duration));
    }
    let rate = value["samplerate"]
        .as_f64()
        .filter(|v| v.is_finite() && *v > 0.);
    if rate.is_none() {
        issue(&mut track, "invalid_sample_rate", "timing");
    }
    if let Some(bpm) = value["bpm"].as_f64().filter(|v| *v > 0.) {
        track.bpm = Some(ScalarBpm {
            presence: FieldPresence::Value,
            value: Some(bpm),
            locked: value["bpm_lock"]
                .as_bool()
                .or_else(|| value["bpm_lock"].as_i64().map(|v| v != 0)),
            extensions: BTreeMap::from([("native_bpm_lock".into(), value["bpm_lock"].clone())]),
            ..Default::default()
        });
    }
    for (field, version_field) in [("beats", "beats_version"), ("keys", "keys_version")] {
        let index = names
            .iter()
            .position(|name| name == field)
            .context("Missing codec column")?;
        let version = value[version_field].as_str().unwrap_or("");
        let raw = row.get_ref(index)?;
        match (field, raw) {
            ("beats", ValueRef::Blob(bytes)) => match rate
                .context("Invalid source sample rate")
                .and_then(|rate| mixxx_codec::grid(bytes, version, rate, options.source_byte_order))
            {
                Ok(mut grid) => {
                    grid.raw = Some(serde_json::from_value(value[field]["blob"].clone())?);
                    track.grid = Some(grid);
                }
                Err(_) => issue(&mut track, "grid_decode_failed", "grid"),
            },
            ("keys", ValueRef::Blob(bytes)) => match mixxx_codec::key(bytes, version, rate) {
                Ok(key) => {
                    if let Some(primary) = key.camelot.as_deref() {
                        let text = value["key"]
                            .as_str()
                            .and_then(|s| crate::formatting::parse_key(s).ok());
                        let cache = value["key_id"].as_u64().filter(|v| (1..=24).contains(v));
                        let number = key.extensions["global_key"].as_u64();
                        if text.is_some_and(|k| k.camelot() != primary)
                            || cache.is_some_and(|n| Some(n) != number)
                        {
                            issue(&mut track, "conflicting_key_evidence", "key");
                        }
                    }
                    track.key = Some(key);
                }
                Err(_) => issue(&mut track, "key_decode_failed", "key"),
            },
            (_, ValueRef::Null) => {}
            _ => issue(
                &mut track,
                if field == "beats" {
                    "grid_decode_failed"
                } else {
                    "key_decode_failed"
                },
                field,
            ),
        }
        if field == "keys"
            && !matches!(raw, ValueRef::Blob(_))
            && version.is_empty()
            && let Some(text) = value["key"].as_str()
        {
            let key = crate::formatting::parse_key(text).ok();
            track.key = Some(MusicalKey {
                presence: if text.is_empty() {
                    FieldPresence::Empty
                } else {
                    FieldPresence::Value
                },
                raw: Some(text.into()),
                camelot: key.map(|k| k.camelot()),
                provenance: Some("mixxx_legacy_text".into()),
                ..Default::default()
            });
        }
    }
    track.extensions.insert("native".into(), native);
    Ok(track)
}
fn location_fn(raw: &str, options: &ReadOptions) -> Result<LocationEvidence> {
    location(raw, None, options)
}
impl SourceAdapter for MixxxReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::Mixxx
    }
    fn inspect(&self, snapshot: &ImportSnapshot) -> Result<SourceProfile> {
        Ok(mixxx_schema::open(snapshot, &ReadOptions::default())?.1)
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
        ensure!(options.limits.page_size > 0, "Invalid page size");
        let (db, profile, version) = mixxx_schema::open(snapshot, options)?;
        // A private disk index makes cue lookup bounded without modifying a
        // snapshot or rescanning every source cue for every track.
        let auxiliary = tempfile::NamedTempFile::new()?;
        let aux = Connection::open(auxiliary.path())?;
        aux.execute_batch("CREATE TABLE cue_rows(track_id INTEGER,id INTEGER,json TEXT,PRIMARY KEY(track_id,id));CREATE TABLE emitted(id TEXT PRIMARY KEY);")?;
        let cue_names = [
            "id", "track_id", "type", "position", "length", "hotcue", "label", "color",
        ]
        .map(str::to_owned);
        let mut statement = db.prepare(
            "SELECT id,track_id,type,position,length,hotcue,label,color FROM cues ORDER BY id",
        )?;
        let mut rows = statement.query([])?;
        let tx = aux.unchecked_transaction()?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let raw = raw_row(row, &cue_names, options, sink)?;
            let encoded = serde_json::to_string(&raw)?;
            ensure!(
                encoded.len() as u64 <= options.limits.text_bytes,
                "Cue record byte limit"
            );
            tx.execute(
                "INSERT INTO cue_rows VALUES(?1,?2,?3)",
                params![
                    raw["track_id"].as_i64().context("Invalid cue track ID")?,
                    raw["id"].as_i64().context("Invalid cue ID")?,
                    encoded
                ],
            )?;
        }
        tx.commit()?;
        drop(rows);
        drop(statement);
        let mut names = [
            "id",
            "artist",
            "title",
            "album",
            "year",
            "genre",
            "tracknumber",
            "location",
            "comment",
            "url",
            "duration",
            "bitrate",
            "samplerate",
            "cuepoint",
            "bpm",
            "channels",
            "datetime_added",
            "mixxx_deleted",
            "played",
            "header_parsed",
            "filetype",
            "replaygain",
            "timesplayed",
            "rating",
            "key",
            "beats",
            "beats_version",
            "composer",
            "bpm_lock",
            "beats_sub_version",
            "keys",
            "keys_version",
            "keys_sub_version",
            "key_id",
            "grouping",
            "album_artist",
            "coverart_source",
            "coverart_type",
            "coverart_location",
            "coverart_hash",
            "replaygain_peak",
            "tracktotal",
            "color",
            "coverart_color",
            "coverart_digest",
            "last_played_at",
            "source_synchronized_ms",
        ]
        .map(str::to_owned)
        .to_vec();
        if version == 40 {
            names.push("tuning_frequency_hz".into());
        }
        let projection = names
            .iter()
            .map(|n| format!("l.\"{n}\""))
            .collect::<Vec<_>>()
            .join(",");
        names.extend(["_location".into(), "_fs_deleted".into()]);
        let mut statement=db.prepare(&format!("SELECT {projection},t.location AS _location,t.fs_deleted AS _fs_deleted FROM library l LEFT JOIN track_locations t ON t.id=l.location ORDER BY l.id"))?;
        let mut rows = statement.query([])?;
        let mut tracks = 0;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let mut track = convert(row, &names, options, sink)?;
            let rate = track.extensions["native"]["samplerate"]
                .as_f64()
                .filter(|r| *r > 0.);
            add_cues(&mut track, &aux, rate)?;
            aux.execute("INSERT INTO emitted VALUES(?1)", [&track.external_id])?;
            emit_track(track, options, sink, cancel)?;
            tracks += 1;
        }
        drop(rows);
        drop(statement);
        let mut after: Option<i64> = None;
        loop {
            let mut statement=aux.prepare("SELECT DISTINCT c.track_id FROM cue_rows c LEFT JOIN emitted e ON e.id=CAST(c.track_id AS TEXT) WHERE e.id IS NULL AND (?1 IS NULL OR c.track_id>?1) ORDER BY c.track_id LIMIT ?2")?;
            let page = statement
                .query_map(params![after, options.limits.page_size.min(1000)], |r| {
                    r.get::<_, i64>(0)
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(statement);
            if page.is_empty() {
                break;
            }
            for id in page {
                cancel.check()?;
                if missing(&id.to_string(), &aux, options, sink, cancel)? {
                    tracks += 1;
                }
                after = Some(id);
            }
        }
        let mut collections = 0;
        let mut members = 0;
        for (table, namespace) in [("Playlists", "playlist"), ("crates", "crate")] {
            let collection_names = if table == "Playlists" {
                vec![
                    "id",
                    "name",
                    "position",
                    "hidden",
                    "date_created",
                    "date_modified",
                    "locked",
                ]
            } else {
                vec!["id", "name", "count", "show", "locked", "autodj_source"]
            }
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
            let mut statement = db.prepare(&format!(
                "SELECT {} FROM {table} ORDER BY id",
                collection_names
                    .iter()
                    .map(|n| format!("\"{n}\""))
                    .collect::<Vec<_>>()
                    .join(",")
            ))?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                cancel.check()?;
                let raw = raw_row(row, &collection_names, options, sink)?;
                let numeric = raw["id"].as_i64().context("Invalid collection ID")?;
                let hidden = raw["hidden"].as_i64().unwrap_or(0);
                if namespace == "playlist"
                    && (hidden == 1 && !options.include_queue
                        || hidden == 2 && !options.include_history)
                {
                    continue;
                }
                let id = format!("{namespace}:{numeric}");
                let kind = if namespace == "crate" {
                    CollectionKind::Crate
                } else {
                    match hidden {
                        1 => CollectionKind::Queue,
                        2 => CollectionKind::History,
                        _ => CollectionKind::Playlist,
                    }
                };
                let mut collection = CollectionObservation {
                    external_id: id.clone(),
                    name: raw["name"].as_str().unwrap_or("").into(),
                    kind,
                    extensions: BTreeMap::from([("native".into(), raw)]),
                    ..Default::default()
                };
                let query = if namespace == "crate" {
                    "SELECT track_id,NULL,NULL FROM crate_tracks WHERE crate_id=?1 ORDER BY track_id"
                } else {
                    "SELECT track_id,id,position FROM PlaylistTracks WHERE playlist_id=?1 ORDER BY position,id"
                };
                let mut member_statement = db.prepare(query)?;
                let mut member_rows = member_statement.query([numeric])?;
                // Ordered by position/id: adjacent comparison stays constant-space.
                let mut last_position = None;
                let mut duplicate = false;
                while let Some(member) = member_rows.next()? {
                    cancel.check()?;
                    let native_track = member.get::<_, Option<i64>>(0)?;
                    let occurrence = member.get::<_, Option<i64>>(1)?;
                    let position = member.get::<_, Option<i64>>(2)?;
                    let track = native_track
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| format!("missing:{id}:{}", occurrence.unwrap_or(0)));
                    if missing(&track, &aux, options, sink, cancel)? {
                        tracks += 1;
                    }
                    if let Some(position) = position {
                        duplicate |= last_position == Some(position);
                        last_position = Some(position);
                    }
                    sink.push(ObservationRecord::Membership {
                        collection_id: id.clone(),
                        occurrence: MembershipObservation {
                            external_occurrence_id: occurrence
                                .map(|n| n.to_string())
                                .unwrap_or_else(|| track.clone()),
                            track_external_id: track,
                            position,
                            extensions: BTreeMap::from([(
                                "source_order".into(),
                                json!(if namespace == "crate" {
                                    "unordered"
                                } else {
                                    "position"
                                }),
                            )]),
                        },
                    })?;
                    members += 1;
                }
                if duplicate {
                    collection.extensions.insert(
                        "issues".into(),
                        json!([{"code":"duplicate_membership_position"}]),
                    );
                }
                sink.push(ObservationRecord::Collection(collection))?;
                collections += 1;
            }
        }
        let mut result = header(
            snapshot,
            options,
            &profile,
            None,
            tracks,
            collections,
            members,
        );
        let mut statement = db.prepare("SELECT directory FROM directories ORDER BY directory")?;
        let mut rows = statement.query([])?;
        let mut page = Vec::new();
        let mut refs = Vec::new();
        while let Some(row) = rows.next()? {
            cancel.check()?;
            page.push(mixxx_schema::scalar(row.get_ref(0)?)?);
            if page.len() >= options.limits.page_size.min(1000) {
                let data = serde_json::to_vec(&page)?;
                ensure!(
                    data.len() as u64 <= options.limits.text_bytes,
                    "Root reference page byte limit"
                );
                refs.push(sink.put_blob(&data, Some("application/json"))?);
                page.clear();
            }
        }
        if !page.is_empty() {
            let data = serde_json::to_vec(&page)?;
            ensure!(
                data.len() as u64 <= options.limits.text_bytes,
                "Root reference page byte limit"
            );
            refs.push(sink.put_blob(&data, Some("application/json"))?);
        }
        result
            .extensions
            .insert("directory_reference_pages".into(), json!(refs));
        cancel.check()?;
        snapshot.validate_source(cancel)?;
        Ok(result)
    }
}
