//! djay Yap music records read from a private snapshot; derived indexes stay unused.
use super::*;
use rusqlite::{Connection, OpenFlags, OptionalExtension, limits::Limit, params, types::ValueRef};
use std::collections::{BTreeMap, BTreeSet};
pub struct DjayProReader;
const COLLECTIONS: &[&str] = &[
    "mediaItems",
    "localMediaItemLocations",
    "globalMediaItemLocations",
    "mediaItemAnalyzedData",
    "mediaItemUserData",
    "mediaItemPlaylists",
    "mediaItemPlaylistItems",
    "mediaArtists",
    "mediaAlbums",
    "mediaLabels",
    "mediaGenres",
    "mediaItemTitleIDs",
];
const LAYERS: &[&str] = &[
    "mediaItems",
    "mediaItemUserData",
    "mediaItemAnalyzedData",
    "localMediaItemLocations",
    "globalMediaItemLocations",
    "mediaItemTitleIDs",
];
fn open(snapshot: &ImportSnapshot, options: &ReadOptions) -> Result<(Connection, SourceProfile)> {
    ensure!(
        snapshot.provider == SourceProvider::DjayPro
            && snapshot.artifacts.len() == 1
            && matches!(snapshot.artifacts[0].kind, ArtifactKind::Sqlite),
        "Select one coherent djay MediaLibrary.db snapshot"
    );
    ensure!(
        snapshot
            .original_artifact_path(&snapshot.artifacts[0].id)?
            .file_name()
            .is_some_and(|n| n == "MediaLibrary.db"),
        "Choose djay MediaLibrary.db"
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
    let (kind, sql): (String, String) = db.query_row(
        "SELECT type,sql FROM sqlite_master WHERE name='database2'",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(
        kind == "table"
            && !sql.to_ascii_uppercase().contains("VIRTUAL TABLE")
            && !sql.to_ascii_uppercase().contains("WITHOUT ROWID"),
        "Unknown djay music-table profile"
    );
    let names = db
        .prepare("PRAGMA table_info(database2)")?
        .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        ["collection", "key", "data", "metadata"]
            .iter()
            .all(|name| names.iter().any(|(n, _)| n == name)),
        "Unknown djay database2 projection"
    );
    let mut profile = profile(
        SourceProvider::DjayPro,
        "djay-yap-tsaf3-keyed-archive",
        &[
            "descriptive_tags",
            "key",
            "bpm",
            "beatgrid",
            "hot_cues",
            "loops",
            "ratings",
            "color",
            "collections",
        ],
    );
    profile.schema_fingerprint = digest(&serde_json::to_vec(&names)?);
    profile.origin.insert(
        "storage_codec".into(),
        "TSAF3.0/3.0 or bplist00 keyed archives; app acceptance unrun".into(),
    );
    Ok((db, profile))
}
#[derive(Clone)]
struct Record {
    value: Option<Value>,
    raw: Value,
    error: Option<String>,
}
fn get(db: &Connection, collection: &str, key: &str) -> Result<Option<Record>> {
    let row: Option<(Option<String>, String, Option<String>)> = db
        .query_row(
            "SELECT json,raw,error FROM records WHERE collection=?1 AND key=?2",
            params![collection, key],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    row.map(|(value, raw, error)| {
        Ok(Record {
            value: value.map(|v| serde_json::from_str(&v)).transpose()?,
            raw: serde_json::from_str(&raw)?,
            error,
        })
    })
    .transpose()
}
fn number(value: &Value) -> Option<f64> {
    value.as_f64().filter(|v| v.is_finite())
}
fn ids(value: &Value) -> Vec<String> {
    if let Some(values) = value.as_array() {
        values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    } else {
        value.as_str().map(|s| vec![s.into()]).unwrap_or_default()
    }
}
fn key(value: &Value) -> Option<String> {
    let index = number(value).filter(|n| n.fract() == 0. && *n >= 0. && *n < 24.)? as usize;
    Some(
        [
            "8B", "8A", "3B", "3A", "10B", "10A", "5B", "5A", "12B", "12A", "7B", "7A", "2B", "2A",
            "9B", "9A", "4B", "4A", "11B", "11A", "6B", "6A", "1B", "1A",
        ][index]
            .into(),
    )
}
fn exponent(value: &Value) -> Result<i32> {
    let value = number(value).unwrap_or(0.);
    ensure!(
        value.fract() == 0. && (-128.0..=255.).contains(&value),
        "Invalid djay exponent"
    );
    let value = value as i32;
    Ok(if value > 100 { value - 256 } else { value })
}
fn uri_values(value: &Value, out: &mut Vec<String>) {
    if let Some(values) = value.as_array() {
        for value in values {
            uri_values(value, out)
        }
    } else if let Some(value) = value.as_str() {
        out.push(value.into())
    } else if let Some(value) = value.get("$uri").and_then(Value::as_str) {
        out.push(value.into())
    } else if value.get("NS.base").is_none_or(Value::is_null)
        && let Some(value) = value.get("NS.relative").and_then(Value::as_str)
    {
        out.push(value.into())
    }
}
fn grid(
    analysis: &Value,
    edits: &Value,
    base: Option<f64>,
    first: i32,
    last: i32,
) -> Result<Option<GridObservation>> {
    let Some(base) = base else { return Ok(None) };
    let first_bpm = base * 2f64.powi(first);
    let last_bpm = base * 2f64.powi(last);
    ensure!(
        first_bpm.is_finite() && first_bpm > 0. && last_bpm.is_finite() && last_bpm > 0.,
        "Invalid djay adjusted tempo"
    );
    let mut points = Vec::new();
    if let Some(markers) = edits["downbeatMarkers"]
        .as_array()
        .filter(|m| !m.is_empty())
    {
        ensure!(markers.len() <= 65536, "djay marker count limit");
        for (index, marker) in markers.iter().enumerate() {
            let position = number(&marker["position"]).context("Missing djay downbeat position")?;
            let bpm = if let Some(next) = markers.get(index + 1) {
                let next = number(&next["position"]).context("Missing next downbeat")?;
                ensure!(next > position, "Nonincreasing djay downbeats");
                let bars = number(&marker["numberOfBars"]);
                if let Some(bars) = bars.filter(|b| *b != -1. && *b != 255.) {
                    ensure!(bars > 0., "Invalid bar span");
                    60. * 4. * bars / (next - position)
                } else {
                    first_bpm
                }
            } else {
                last_bpm
            };
            ensure!(bpm.is_finite() && bpm > 0., "Invalid djay segment tempo");
            points.push(GridPoint {
                native_seconds: position,
                bpm: Some(bpm),
                original: marker.clone(),
                ..Default::default()
            });
        }
    } else {
        let position = number(&edits["firstDownbeatPosition"])
            .or_else(|| number(&edits["startBarPosition"]))
            .or_else(|| number(&analysis["firstDownbeatPosition"]))
            .or_else(|| number(&analysis["startBarPosition"]))
            .or_else(|| {
                number(&analysis["startPoint"])
                    .map(|p| p + 0.003 * number(&edits["nrOfBeatShift"]).unwrap_or(0.))
            });
        let Some(position) = position else {
            return Ok(None);
        };
        points.push(GridPoint{native_seconds:position,bpm:Some(first_bpm),original:json!({"edits":edits,"analysis_anchor":analysis.get("firstDownbeatPosition")}),..Default::default()});
    }
    Ok(Some(GridObservation {
        representation: if points.len() == 1 {
            "constant"
        } else {
            "segments"
        }
        .into(),
        original_time_base: Some("seconds".into()),
        native_first_beat_seconds: Some(points[0].native_seconds),
        bpm: points[0].bpm,
        points,
        extensions: BTreeMap::from([
            ("first_exponent".into(), json!(first)),
            ("last_exponent".into(), json!(last)),
            ("native_edits".into(), edits.clone()),
        ]),
        ..Default::default()
    }))
}
fn convert(db: &Connection, id: &str, options: &ReadOptions) -> Result<TrackObservation> {
    let mut track = TrackObservation {
        external_id: id.into(),
        locations: LocationEvidence {
            availability: "unresolved".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut native = serde_json::Map::new();
    let mut raw = serde_json::Map::new();
    for collection in LAYERS {
        if let Some(record) = get(db, collection, id)? {
            raw.insert((*collection).into(), record.raw);
            if let Some(error) = record.error {
                issue(&mut track, "djay_record_decode_failed", collection);
                track
                    .extensions
                    .entry("record_errors".into())
                    .or_insert(json!({}))[collection] = json!(error);
            }
            if let Some(value) = record.value {
                native.insert((*collection).into(), value);
            }
        }
    }
    let base = native.get("mediaItems").cloned().unwrap_or(Value::Null);
    let user = native
        .get("mediaItemUserData")
        .cloned()
        .unwrap_or(Value::Null);
    let analysis = native
        .get("mediaItemAnalyzedData")
        .cloned()
        .unwrap_or(Value::Null);
    let title_ids = native
        .get("mediaItemTitleIDs")
        .cloned()
        .unwrap_or(Value::Null);
    if base.is_null() {
        issue(&mut track, "missing_djay_media_item", "track");
    }
    for (source, target) in [
        ("title", "title"),
        ("artist", "artist"),
        ("album", "album"),
        ("genre", "genre"),
        ("label", "label"),
        ("composer", "composer"),
        ("grouping", "grouping"),
        ("year", "year"),
        ("comments", "comment"),
        ("albumTrackNumber", "track_number"),
    ] {
        if let Some(value) = base
            .get(source)
            .or_else(|| base["titleID"].get(source))
            .or_else(|| title_ids.get(source))
        {
            if !(value.is_null()
                || value.is_string()
                || matches!(target, "year" | "track_number") && value.is_number())
            {
                issue(&mut track, "unresolved_djay_metadata_field", target);
                continue;
            }
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
    for (collection, field, target) in [
        ("mediaArtists", "artistUUIDs", "artist"),
        ("mediaAlbums", "albumUUID", "album"),
        ("mediaGenres", "genreUUIDs", "genre"),
        ("mediaLabels", "labelUUID", "label"),
    ] {
        let references = ids(&base[field]);
        if references.is_empty() {
            continue;
        }
        let mut names = Vec::new();
        let mut missing = false;
        for reference in references {
            if let Some(record) = get(db, collection, &reference)?
                && let Some(name) = record.value.as_ref().and_then(|v| v["name"].as_str())
            {
                names.push(name.to_string());
            } else {
                missing = true;
            }
        }
        if missing {
            issue(&mut track, &format!("unresolved_djay_{target}"), target);
        } else {
            track
                .metadata
                .insert(target.into(), Presence::Value(json!(names.join(", "))));
        }
    }
    for layer in [&base, &user] {
        if let Some(date) = layer.get("addedDate") {
            let utc = if let Some(utc) = date.get("utc").and_then(Value::as_str) {
                Some(utc.to_owned())
            } else if let Some(seconds) = number(date) {
                super::keyed_archive::date_value(seconds)
                    .ok()
                    .and_then(|v| v["utc"].as_str().map(str::to_owned))
            } else {
                None
            };
            if let Some(utc) = utc {
                track
                    .metadata
                    .insert("date_added".into(), Presence::Value(json!(utc)));
            }
        }
    }
    if let Some(duration) = number(&base["duration"])
        .or_else(|| number(&base["titleID"]["duration"]))
        .filter(|n| *n >= 0.)
    {
        track
            .extensions
            .insert("duration_seconds".into(), json!(duration));
    }
    for (field, scale) in [("rating", json!(5)), ("colorIndex", json!("djay_palette"))] {
        if let Some(value) = user.get(field) {
            track.metadata.insert(
                if field == "rating" { "rating" } else { "color" }.into(),
                if value.is_null() {
                    Presence::Null
                } else {
                    Presence::Value(json!({"raw":value,"scale":scale}))
                },
            );
        }
    }
    if let Some(value) = analysis.get("keySignatureIndex") {
        track.cached_tags.insert(
            "djay_analyzed_key".into(),
            if value.is_null() {
                Presence::Null
            } else {
                Presence::Value(json!({"raw":value,"camelot":key(value)}))
            },
        );
    }
    let manual = user
        .get("manualKeySignatureIndex2")
        .filter(|v| !v.is_null());
    let selected = manual.or_else(|| analysis.get("keySignatureIndex"));
    if let Some(value) = selected {
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
            camelot: key(value),
            provenance: Some(
                if manual.is_some() {
                    "djay_manual_key"
                } else {
                    "djay_analyzed_key"
                }
                .into(),
            ),
            ..Default::default()
        });
    }
    if let Some(value) = analysis.get("bpm") {
        track.cached_tags.insert(
            "djay_analyzed_bpm".into(),
            if value.is_null() {
                Presence::Null
            } else {
                Presence::Value(value.clone())
            },
        );
    }
    if let Some(value) = base.get("bpm") {
        track.cached_tags.insert(
            "djay_media_item_bpm".into(),
            if value.is_null() {
                Presence::Null
            } else {
                Presence::Value(value.clone())
            },
        );
    }
    let edits = &user["beatGridEdits"];
    let first = exponent(&edits["firstGridSegmentTempoExponent"]);
    let last = exponent(&edits["lastGridSegmentTempoExponent"]);
    let native_bpm = number(&user["manualBPM"])
        .filter(|b| *b > 0.)
        .or_else(|| number(&analysis["bpm"]).filter(|b| *b > 0.))
        .or_else(|| number(&base["bpm"]).filter(|b| *b > 0.));
    if let (Ok(first), Ok(last)) = (first, last) {
        if let Some(base) = native_bpm {
            let bpm = base * 2f64.powi(first);
            if bpm.is_finite() && bpm > 0. {
                track.bpm = Some(ScalarBpm {
                    presence: FieldPresence::Value,
                    value: Some(bpm),
                    provenance: Some("djay_native_tempo_and_exponent".into()),
                    ..Default::default()
                });
            }
        }
        let straight = edits
            .get("useStraightBeats")
            .or_else(|| analysis.get("isStraightGrid"));
        let alternate = straight.is_some_and(|v| v == &Value::Bool(false) || number(v) == Some(0.));
        match if alternate {
            Err(anyhow::anyhow!("Unsupported alternate grid"))
        } else {
            grid(&analysis, edits, native_bpm, first, last)
        } {
            Ok(grid) => track.grid = grid,
            Err(_) => issue(
                &mut track,
                if alternate {
                    "unsupported_djay_alternate_grid"
                } else {
                    "djay_grid_decode_failed"
                },
                "grid",
            ),
        }
    } else {
        issue(&mut track, "unsupported_djay_tempo_exponent", "bpm/grid");
    }
    let mut slots = BTreeSet::new();
    for (field, saved) in [("cuePoints", false), ("loopRegions", true)] {
        if let Some(cues) = user[field].as_array() {
            ensure!(cues.len() <= 65536, "djay cue count limit");
            for (index, cue) in cues.iter().enumerate() {
                let start = number(&cue["time"]).or_else(|| number(&cue["startTime"]));
                let end = number(&cue["endTime"]).filter(|v| *v != -1.);
                let slot = number(&cue["number"])
                    .filter(|n| n.fract() == 0. && *n >= 1. && *n <= i32::MAX as f64)
                    .map(|n| n as i32 - 1);
                if start.is_none() {
                    issue(&mut track, "unresolved_djay_cue_time", field);
                }
                if let (Some(first), Some(last)) = (start, end)
                    && last <= first
                {
                    issue(&mut track, "invalid_djay_loop_range", field);
                }
                if let Some(slot) = slot
                    && !slots.insert((saved, slot))
                {
                    issue(&mut track, "duplicate_djay_cue_slot", field);
                }
                let color = cue["color"]
                    .as_str()
                    .and_then(|s| u32::from_str_radix(s.trim_start_matches('#'), 16).ok())
                    .filter(|c| *c <= 0xffffff);
                let family = if saved {
                    CueFamily::Loop
                } else if slot.is_some() {
                    CueFamily::Hot
                } else {
                    CueFamily::Memory
                };
                track.cues.push(CueObservation {
                    external_id: format!("{field}:{index}"),
                    family,
                    source_slot: slot,
                    name: cue["comment"]
                        .as_str()
                        .or_else(|| cue["name"].as_str())
                        .map(str::to_owned),
                    color_rgb: color,
                    original: cue.clone(),
                    native_start_seconds: start,
                    native_end_seconds: end,
                    flags: BTreeMap::from([
                        (
                            "source_slot_role".into(),
                            json!(if saved {
                                "saved_loop"
                            } else if slot.is_some() {
                                "hot"
                            } else {
                                "unassigned"
                            }),
                        ),
                        ("loop".into(), json!(saved || end.is_some())),
                    ]),
                    ..Default::default()
                });
            }
        }
    }
    let mut candidates = Vec::new();
    for collection in ["localMediaItemLocations", "globalMediaItemLocations"] {
        if let Some(value) = native.get(collection) {
            uri_values(&value["sourceURIs"], &mut candidates);
            uri_values(&value["uri"], &mut candidates);
        }
    }
    let mut locations = Vec::new();
    for candidate in &candidates {
        let mut evidence = location(candidate, None, options)?;
        evidence
            .extensions
            .insert("source_candidate".into(), json!(candidate));
        locations.push(evidence);
    }
    let selected = locations
        .iter()
        .position(|v| v.availability == "available")
        .unwrap_or(0);
    if let Some(location) = locations.get(selected) {
        track.locations = location.clone();
    } else {
        issue(&mut track, "missing_djay_location", "location");
    }
    track
        .locations
        .extensions
        .insert("alternate_candidates".into(), json!(locations));
    track
        .extensions
        .insert("native_records".into(), Value::Object(native));
    track
        .extensions
        .insert("raw_records".into(), Value::Object(raw));
    Ok(track)
}
impl SourceAdapter for DjayProReader {
    fn provider(&self) -> SourceProvider {
        SourceProvider::DjayPro
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
        let file = tempfile::NamedTempFile::new()?;
        let aux = Connection::open(file.path())?;
        aux.execute_batch("CREATE TABLE records(collection TEXT,key TEXT,row_id INTEGER,json TEXT,raw TEXT,error TEXT,PRIMARY KEY(collection,key));CREATE TABLE emitted(id TEXT PRIMARY KEY);")?;
        let quoted = COLLECTIONS
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(",");
        let mut statement=db.prepare(&format!("SELECT rowid,collection,key,data,metadata FROM database2 WHERE collection IN({quoted}) ORDER BY rowid"))?;
        let mut rows = statement.query([])?;
        let tx = aux.unchecked_transaction()?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let rowid: i64 = row.get(0)?;
            let collection: String = row.get(1)?;
            let key: String = row.get(2)?;
            ensure!(
                !key.is_empty() && key.len() as u64 <= options.limits.text_bytes,
                "Invalid djay music key"
            );
            let mut raw = json!({"rowid":rowid,"collection":collection,"key":key});
            let mut value = None;
            let mut error = None;
            if let ValueRef::Blob(bytes) = row.get_ref(3)? {
                ensure!(
                    bytes.len() as u64 <= options.limits.blob_bytes,
                    "djay record byte limit"
                );
                raw["data"] = json!(sink.put_blob(bytes, None)?);
                let decoded = if bytes.starts_with(b"bplist") {
                    super::keyed_archive::decode_keyed_archive(bytes, &options.limits)
                } else {
                    super::djay_codec::decode_djay_stream(bytes, &options.limits)
                }
                .and_then(|value| value.to_json());
                match decoded {
                    Ok(decoded) => {
                        let encoded = serde_json::to_string(&decoded)?;
                        if encoded.len() as u64 <= options.limits.text_bytes {
                            value = Some(encoded)
                        } else {
                            error = Some("Decoded record exceeds text limit".into())
                        }
                    }
                    Err(e) => error = Some(e.to_string()),
                }
            } else {
                raw["data"] = Value::Null;
                error = Some("Music record has no native data blob".into());
            }
            if let ValueRef::Blob(bytes) = row.get_ref(4)? {
                ensure!(
                    bytes.len() as u64 <= options.limits.blob_bytes,
                    "Metadata blob byte limit"
                );
                raw["metadata"] = json!(sink.put_blob(bytes, None)?);
            } else {
                raw["metadata"] = Value::Null;
            }
            tx.execute(
                "INSERT INTO records VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    collection,
                    key,
                    rowid,
                    value,
                    serde_json::to_string(&raw)?,
                    error
                ],
            )?;
        }
        tx.commit()?;
        drop(rows);
        drop(statement);
        let mut tracks = 0;
        let mut statement=aux.prepare("SELECT DISTINCT key FROM records WHERE collection IN('mediaItems','mediaItemUserData','mediaItemAnalyzedData','localMediaItemLocations','globalMediaItemLocations') ORDER BY key")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let id: String = row.get(0)?;
            let track = convert(&aux, &id, options)?;
            aux.execute("INSERT INTO emitted VALUES(?1)", [&id])?;
            emit_track(track, options, sink, cancel)?;
            tracks += 1;
        }
        drop(rows);
        drop(statement);
        let mut playlists = BTreeMap::<String, Record>::new();
        let mut statement = aux.prepare(
            "SELECT key FROM records WHERE collection='mediaItemPlaylists' ORDER BY row_id",
        )?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            ensure!(playlists.len() < 1_000_000, "djay playlist count limit");
            let id: String = row.get(0)?;
            playlists.insert(
                id.clone(),
                get(&aux, "mediaItemPlaylists", &id)?.context("Lost staged playlist")?,
            );
        }
        drop(rows);
        drop(statement);
        let root = "mediaItemPlaylist-root";
        let mut parent_map = BTreeMap::<String, (Option<String>, Option<usize>)>::new();
        for (id, record) in &playlists {
            if let Some(value) = &record.value {
                for (position, child) in ids(&value["childUUIDs"]).into_iter().enumerate() {
                    let parent = (id != root).then(|| id.clone());
                    ensure!(
                        parent_map
                            .insert(child.clone(), (parent, Some(position)))
                            .is_none(),
                        "Duplicate/conflicting djay child hierarchy reference"
                    );
                }
            }
        }
        for (id, record) in &playlists {
            if id == root {
                continue;
            }
            if !parent_map.contains_key(id) {
                let parent = record
                    .value
                    .as_ref()
                    .and_then(|v| v["parentUUID"].as_str())
                    .filter(|p| !p.is_empty() && *p != root)
                    .map(str::to_owned);
                parent_map.insert(id.clone(), (parent, None));
            }
        }
        for id in parent_map.keys() {
            let mut at = Some(id);
            let mut seen = BTreeSet::new();
            while let Some(node) = at {
                ensure!(
                    seen.insert(node) && seen.len() <= options.limits.hierarchy_depth,
                    "Cyclic/deep djay playlist hierarchy"
                );
                at = parent_map.get(node).and_then(|(p, _)| p.as_ref());
            }
        }
        let missing = parent_map
            .values()
            .filter_map(|(p, _)| p.as_ref())
            .chain(parent_map.keys())
            .filter(|p| !playlists.contains_key(*p))
            .cloned()
            .collect::<BTreeSet<_>>();
        for id in missing {
            playlists.insert(
                id.clone(),
                Record {
                    value: Some(json!({"name":"","type":1})),
                    raw: json!({"missing_reference":id}),
                    error: Some("Referenced playlist unavailable".into()),
                },
            );
            parent_map.entry(id).or_insert((None, None));
        }
        let mut collections = 0;
        let mut members = 0;
        for (id, record) in &playlists {
            cancel.check()?;
            if id == root {
                continue;
            }
            let empty = Value::Null;
            let value = record.value.as_ref().unwrap_or(&empty);
            let mut collection = CollectionObservation {
                external_id: id.clone(),
                parent_id: parent_map.get(id).and_then(|(p, _)| p.clone()),
                name: value["name"].as_str().unwrap_or("").into(),
                kind: match value["type"].as_u64() {
                    Some(1) => CollectionKind::Folder,
                    Some(2) => CollectionKind::Smart,
                    _ => CollectionKind::Playlist,
                },
                extensions: BTreeMap::from([
                    ("native".into(), value.clone()),
                    ("raw_record".into(), record.raw.clone()),
                    (
                        "display_order".into(),
                        json!(parent_map.get(id).and_then(|(_, o)| *o)),
                    ),
                ]),
                ..Default::default()
            };
            let mut issues = Vec::new();
            if let Some(error) = &record.error {
                issues.push(json!({"code":"djay_record_decode_failed","message":error}));
            }
            if let Some(filter) = value.get("filterSet") {
                collection
                    .extensions
                    .insert("smart_rules".into(), filter.clone());
                issues.push(json!({"code":"unsupported_djay_smart_filter"}));
            }
            if !issues.is_empty() {
                collection.extensions.insert("issues".into(), json!(issues));
            }
            for (position, link) in ids(&value["itemUUIDs"]).into_iter().enumerate() {
                cancel.check()?;
                let record = get(&aux, "mediaItemPlaylistItems", &link)?;
                let native = record.as_ref().and_then(|r| r.value.as_ref());
                let media = native
                    .and_then(|v| v["mediaItemUUID"].as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("missing:{link}"));
                if aux.execute("INSERT OR IGNORE INTO emitted VALUES(?1)", [&media])? > 0 {
                    let mut track = TrackObservation {
                        external_id: media.clone(),
                        locations: LocationEvidence {
                            availability: "unresolved".into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    issue(&mut track, "missing_djay_media_item", "track");
                    sink.push(ObservationRecord::Track(Box::new(track)))?;
                    tracks += 1;
                }
                sink.push(ObservationRecord::Membership {
                    collection_id: id.clone(),
                    occurrence: MembershipObservation {
                        external_occurrence_id: format!("{link}:{position}"),
                        track_external_id: media,
                        position: Some(position as i64),
                        extensions: BTreeMap::from([
                            ("native_link_key".into(), json!(link)),
                            ("native".into(), native.cloned().unwrap_or(Value::Null)),
                            (
                                "raw_record".into(),
                                record.map(|r| r.raw).unwrap_or(Value::Null),
                            ),
                        ]),
                    },
                })?;
                members += 1;
            }
            sink.push(ObservationRecord::Collection(collection))?;
            collections += 1;
        }
        let mut auxiliary_pages = Vec::new();
        let mut page = Vec::new();
        let mut page_bytes = 0usize;
        let mut statement=aux.prepare("SELECT collection,key,json,raw,error FROM records WHERE collection IN('mediaArtists','mediaAlbums','mediaGenres','mediaLabels','mediaItemTitleIDs','mediaItemPlaylistItems') ORDER BY collection,key")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let value = json!({"collection":row.get::<_,String>(0)?,"key":row.get::<_,String>(1)?,"decoded":row.get::<_,Option<String>>(2)?.map(|v|serde_json::from_str::<Value>(&v)).transpose()?,"raw":serde_json::from_str::<Value>(&row.get::<_,String>(3)?)?,"error":row.get::<_,Option<String>>(4)?});
            let length = serde_json::to_vec(&value)?.len();
            if !page.is_empty()
                && (page_bytes + length > 4 * 1024 * 1024
                    || page.len() >= options.limits.page_size.clamp(1, 1000))
            {
                auxiliary_pages
                    .push(sink.put_blob(&serde_json::to_vec(&page)?, Some("application/json"))?);
                page.clear();
                page_bytes = 0;
            }
            page_bytes += length;
            page.push(value);
        }
        if !page.is_empty() {
            auxiliary_pages
                .push(sink.put_blob(&serde_json::to_vec(&page)?, Some("application/json"))?);
        }
        snapshot.validate_source(cancel)?;
        cancel.check()?;
        let mut header = header(
            snapshot,
            options,
            &profile,
            None,
            tracks,
            collections,
            members,
        );
        header
            .extensions
            .insert("auxiliary_record_pages".into(), json!(auxiliary_pages));
        if let Some(record) = playlists.get(root) {
            header.extensions.insert(
                "playlist_root_record".into(),
                json!({"decoded":record.value,"raw":record.raw,"error":record.error}),
            );
        }
        Ok(header)
    }
}
