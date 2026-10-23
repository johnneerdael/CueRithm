//! Read-only Serato file-owned performance observations; no native-clock inversion.
use super::*;
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SeratoPerformanceObservation {
    pub cues: Presence<Vec<CueObservation>>,
    pub grid: Presence<GridObservation>,
    pub autotags: Presence<Value>,
    pub extensions: BTreeMap<String, Value>,
    pub issues: Vec<ImportIssue>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Markers,
    Grid,
    Autotags,
    Legacy,
}
fn kind(name: &str) -> Option<Kind> {
    match name.to_ascii_lowercase().as_str() {
        "serato markers2" | "serato_markers_v2" | "serato_markers2" | "markersv2" => {
            Some(Kind::Markers)
        }
        "serato beatgrid" | "serato_beatgrid" | "beatgrid" => Some(Kind::Grid),
        "serato autotags" | "serato_autotags" | "autotags" => Some(Kind::Autotags),
        "serato markers_" | "serato_markers_" | "markers" => Some(Kind::Legacy),
        _ => None,
    }
}
fn take<'a>(bytes: &'a [u8], at: &mut usize, n: usize) -> Result<&'a [u8]> {
    let end = at.checked_add(n).context("Serato length overflow")?;
    let out = bytes.get(*at..end).context("Truncated Serato payload")?;
    *at = end;
    Ok(out)
}
fn be32(bytes: &[u8]) -> Result<u32> {
    Ok(u32::from_be_bytes(bytes.try_into()?))
}
fn cstring<'a>(bytes: &'a [u8], at: &mut usize) -> Result<&'a [u8]> {
    let n = bytes
        .get(*at..)
        .context("Truncated Serato string")?
        .iter()
        .position(|b| *b == 0)
        .context("Unterminated Serato string")?;
    let out = take(bytes, at, n)?;
    take(bytes, at, 1)?;
    Ok(out)
}
fn skip_text(bytes: &[u8], at: &mut usize, encoding: u8) -> Result<()> {
    match encoding {
        0 | 3 => {
            cstring(bytes, at)?;
        }
        1 | 2 => {
            let n = bytes
                .get(*at..)
                .context("Truncated UTF16 identity")?
                .as_chunks::<2>()
                .0
                .iter()
                .position(|b| *b == [0, 0])
                .context("Unterminated UTF16 identity")?;
            take(bytes, at, n * 2 + 2)?;
        }
        _ => anyhow::bail!("Unknown GEOB encoding"),
    }
    Ok(())
}
fn b64(bytes: &[u8], limits: &ImportLimits) -> Result<Vec<u8>> {
    ensure!(
        bytes.len() as u64 <= limits.text_bytes,
        "Serato base64 byte limit"
    );
    let bytes: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let out = if bytes.contains(&b'=') {
        STANDARD.decode(bytes)?
    } else {
        STANDARD_NO_PAD.decode(bytes)?
    };
    ensure!(
        out.len() as u64 <= limits.blob_bytes,
        "Serato decoded byte limit"
    );
    Ok(out)
}
fn named(bytes: Vec<u8>, label: &str) -> Result<Vec<u8>> {
    let mut prefix = label.as_bytes().to_vec();
    prefix.push(0);
    let bytes = bytes
        .strip_prefix(b"application/octet-stream\0\0")
        .unwrap_or(&bytes);
    Ok(bytes
        .strip_prefix(prefix.as_slice())
        .context("Wrong or missing Serato named envelope")?
        .to_vec())
}
fn atoms(bytes: &[u8]) -> Result<Vec<(&[u8], &[u8])>> {
    let mut at = 0;
    let mut out = Vec::new();
    while at < bytes.len() {
        let n = be32(take(bytes, &mut at, 4)?)? as usize;
        let id = take(bytes, &mut at, 4)?;
        ensure!(n >= 8, "Invalid Serato MP4 atom length");
        out.push((id, take(bytes, &mut at, n - 8)?));
        ensure!(out.len() <= 65536, "Serato atom count limit");
    }
    Ok(out)
}
fn extract(
    record: &RawTagRecord,
    bytes: &[u8],
    limits: &ImportLimits,
) -> Result<Option<(Kind, Vec<u8>, bool)>> {
    if record.namespace.starts_with("id3v2.") && matches!(record.native_id.as_str(), "GEOB" | "GEO")
    {
        let Some(k) = record.identity.get("description").and_then(|n| kind(n)) else {
            return Ok(None);
        };
        ensure!(record.flags == 0, "Flagged GEOB cannot be interpreted");
        let mut at = 0;
        let encoding = take(bytes, &mut at, 1)?[0];
        cstring(bytes, &mut at)?;
        skip_text(bytes, &mut at, encoding)?;
        skip_text(bytes, &mut at, encoding)?;
        return Ok(Some((k, bytes[at..].to_vec(), false)));
    }
    if record.namespace == "vorbis" {
        let Some(k) = kind(&record.native_id) else {
            return Ok(None);
        };
        let value = bytes
            .splitn(2, |b| *b == b'=')
            .nth(1)
            .context("Missing Serato comment value")?;
        let ogg = record.native_id.eq_ignore_ascii_case("SERATO_MARKERS2");
        if ogg {
            return Ok(Some((k, b64(value, limits)?, true)));
        }
        if k == Kind::Grid && value.contains(&b'(') {
            return Ok(Some((k, value.to_vec(), true)));
        }
        let data = b64(value, limits)?;
        let label = match k {
            Kind::Markers => "Serato Markers2",
            Kind::Grid => "Serato BeatGrid",
            Kind::Autotags => "Serato Autotags",
            Kind::Legacy => "Serato Markers_",
        };
        return Ok(Some((k, named(data, label)?, false)));
    }
    if record.namespace == "mp4.ilst" && record.native_id == "----" {
        let items = atoms(bytes)?;
        let field = |id: &[u8]| -> Option<&[u8]> {
            items
                .iter()
                .find(|(key, _)| *key == id)
                .and_then(|(_, data)| data.get(4..))
        };
        if field(b"mean") != Some(&b"com.serato.dj"[..]) {
            return Ok(None);
        }
        let Some(k) = field(b"name")
            .and_then(|n| std::str::from_utf8(n).ok())
            .and_then(kind)
        else {
            return Ok(None);
        };
        let value = items
            .iter()
            .find(|(key, _)| *key == b"data")
            .and_then(|(_, data)| data.get(8..))
            .context("Missing Serato MP4 value")?;
        let label = match k {
            Kind::Markers => "Serato Markers2",
            Kind::Grid => "Serato BeatGrid",
            Kind::Autotags => "Serato Autotags",
            Kind::Legacy => "Serato Markers_",
        };
        return Ok(Some((k, named(b64(value, limits)?, label)?, false)));
    }
    Ok(None)
}
fn markers(
    bytes: &[u8],
    direct: bool,
    limits: &ImportLimits,
) -> Result<(Vec<CueObservation>, Vec<Value>, Vec<ImportIssue>)> {
    let decoded;
    let body = if direct {
        bytes
    } else {
        ensure!(bytes.starts_with(&[1, 1]), "Unsupported Markers2 version");
        let encoded = bytes[2..].split(|b| *b == 0).next().unwrap_or_default();
        decoded = b64(encoded, limits)?;
        &decoded
    };
    ensure!(
        body.starts_with(&[1, 1]),
        "Unsupported inner Markers2 version"
    );
    let mut at = 2;
    let mut cues = Vec::new();
    let mut entries = Vec::new();
    let mut issues = Vec::new();
    let mut slots = std::collections::BTreeSet::new();
    while at < body.len() {
        let name = std::str::from_utf8(cstring(body, &mut at)?)?;
        if name.is_empty() {
            ensure!(
                body[at..].iter().all(|b| *b == 0),
                "Nonpadding trailing marker bytes"
            );
            break;
        }
        ensure!(entries.len() < 65536, "Serato entry count limit");
        let n = be32(take(body, &mut at, 4)?)? as usize;
        let data = take(body, &mut at, n)?;
        let index = entries.len();
        entries.push(json!({"name":name,"payload_offset":at-n,"payload_bytes":n,"offset_base":"decoded_markers2"}));
        if n == 0 {
            continue;
        }
        if name == "COLOR" {
            ensure!(data.len() == 4, "Invalid Serato color entry");
            entries[index]["value"] = json!(be32(data)? & 0xffffff);
        } else if name == "BPMLOCK" {
            ensure!(data.len() == 1, "Invalid Serato BPM lock entry");
            entries[index]["value"] = json!(data[0] != 0);
        }
        if matches!(name, "CUE" | "LOOP") {
            let fixed = if name == "CUE" { 12 } else { 20 };
            ensure!(data.len() > fixed, "Truncated Serato marker");
            let mut end = fixed;
            let label = std::str::from_utf8(cstring(data, &mut end)?)?;
            ensure!(end == data.len(), "Trailing marker data");
            let slot = i32::from(data[1]);
            let start = f64::from(be32(&data[2..6])?) / 1000.;
            let mut flags = BTreeMap::new();
            let family = if name == "CUE" {
                CueFamily::Hot
            } else {
                CueFamily::Loop
            };
            let finish = if name == "LOOP" {
                let end = f64::from(be32(&data[6..10])?) / 1000.;
                ensure!(end > start, "Invalid Serato loop interval");
                flags.insert("locked".into(), json!(data[19] != 0));
                Some(end)
            } else {
                None
            };
            flags.insert(
                "source_slot_role".into(),
                json!(if name == "CUE" { "hot" } else { "saved_loop" }),
            );
            if !slots.insert((name.to_owned(), slot)) {
                issues.push(ImportIssue {
                    code: "duplicate_serato_slot".into(),
                    field: Some("cues".into()),
                    message: "Duplicate native slot retained; explicit resolution required".into(),
                    ..Default::default()
                });
            }
            cues.push(CueObservation {
                external_id: index.to_string(),
                family,
                source_slot: Some(slot),
                name: Some(label.into()),
                color_rgb: if name == "CUE" {
                    Some(u32::from_be_bytes([0, data[7], data[8], data[9]]))
                } else {
                    None
                },
                original: entries.last().cloned().unwrap_or_default(),
                native_start_seconds: Some(start),
                native_end_seconds: finish,
                flags,
                ..Default::default()
            });
        } else if !matches!(name, "COLOR" | "BPMLOCK") {
            issues.push(ImportIssue {
                code: "unsupported_serato_marker_entry".into(),
                field: Some("cues".into()),
                message: format!("Retained opaque marker entry {name}"),
                ..Default::default()
            });
        }
    }
    Ok((cues, entries, issues))
}
fn grid(bytes: &[u8], ogg: bool) -> Result<GridObservation> {
    let points = if ogg {
        let value = std::str::from_utf8(bytes)?;
        let (prefix, pair) = value.split_once('(').context("Missing Ogg grid tuple")?;
        ensure!(
            prefix == "000000010000000000000001",
            "Unknown Ogg Serato grid prefix"
        );
        let pair = pair
            .strip_suffix(')')
            .context("Unterminated Ogg grid tuple")?;
        let (time, bpm) = pair.split_once(',').context("Invalid Ogg grid tuple")?;
        let time = time.parse::<f64>()?;
        let bpm = bpm.parse::<f64>()?;
        ensure!(
            time.is_finite() && bpm.is_finite() && bpm > 0.,
            "Invalid Ogg grid values"
        );
        vec![GridPoint {
            native_seconds: time,
            bpm: Some(bpm),
            original: json!(value),
            ..Default::default()
        }]
    } else {
        ensure!(
            bytes.starts_with(&[1, 0]) && bytes.len() >= 7,
            "Unsupported BeatGrid envelope"
        );
        let count = be32(&bytes[2..6])? as usize;
        ensure!(count > 0 && count <= 65536, "Invalid BeatGrid count");
        let expected = count
            .checked_mul(8)
            .and_then(|n| n.checked_add(7))
            .context("Grid length overflow")?;
        ensure!(bytes.len() == expected, "BeatGrid count/length mismatch");
        let mut points = Vec::with_capacity(count);
        for index in 0..count {
            let data = &bytes[6 + index * 8..14 + index * 8];
            let time = f64::from(f32::from_be_bytes(data[..4].try_into()?));
            ensure!(time.is_finite(), "Nonfinite Serato grid time");
            let last = index + 1 == count;
            let word = be32(&data[4..])?;
            points.push(GridPoint{native_seconds:time,bpm:if last{Some(f64::from(f32::from_bits(word)))}else{None},original:json!({"position":time,"beats_till_next":if last{None}else{Some(word)},"word":word}),..Default::default()});
        }
        for i in 0..count - 1 {
            let delta = points[i + 1].native_seconds - points[i].native_seconds;
            ensure!(delta > 0., "Nonincreasing Serato grid");
            points[i].bpm = Some(
                60. * points[i].original["beats_till_next"]
                    .as_u64()
                    .context("Missing beat count")? as f64
                    / delta,
            );
        }
        ensure!(
            points
                .iter()
                .all(|p| p.bpm.is_some_and(|b| b.is_finite() && b > 0.)),
            "Invalid Serato grid BPM"
        );
        points
    };
    Ok(GridObservation {
        representation: "segments".into(),
        original_time_base: Some("seconds".into()),
        native_first_beat_seconds: Some(points[0].native_seconds),
        bpm: points[0].bpm,
        points,
        ..Default::default()
    })
}
fn autotags(bytes: &[u8]) -> Result<Value> {
    ensure!(bytes.starts_with(&[1, 1]), "Unsupported Autotags version");
    let mut at = 2;
    let mut values = Vec::new();
    for _ in 0..3 {
        let text = std::str::from_utf8(cstring(bytes, &mut at)?)?;
        let number = text.parse::<f64>()?;
        ensure!(number.is_finite(), "Nonfinite Autotag");
        values.push(number);
    }
    ensure!(
        bytes[at..].iter().all(|b| *b == 0),
        "Trailing Autotags bytes"
    );
    Ok(json!({"bpm":values[0],"auto_gain":values[1],"gain_db":values[2]}))
}
pub fn decode_serato_performance(
    tags: &FileTagObservation,
    limits: &ImportLimits,
) -> Result<SeratoPerformanceObservation> {
    ensure!(tags.records.len() <= 65536, "Tag count limit");
    let mut out = SeratoPerformanceObservation::default();
    let mut seen = BTreeMap::<u8, usize>::new();
    let mut raw = Vec::new();
    for record in &tags.records {
        let bytes = tags
            .blobs
            .get(&record.payload.sha256)
            .context("Missing raw tag blob")?;
        ensure!(
            bytes.len() as u64 <= limits.blob_bytes
                && bytes.len() as u64 == record.payload.bytes
                && digest(bytes) == record.payload.sha256,
            "Invalid raw tag blob"
        );
        let result = (|| -> Result<()> {
            let Some((k, body, ogg)) = extract(record, bytes, limits)? else {
                return Ok(());
            };
            raw.push(json!({"ordinal":record.ordinal,"payload":record.payload}));
            let key = match k {
                Kind::Markers => 0,
                Kind::Grid => 1,
                Kind::Autotags => 2,
                Kind::Legacy => 3,
            };
            *seen.entry(key).or_default() += 1;
            match k {
                Kind::Legacy => out.issues.push(ImportIssue {
                    code: "unsupported_legacy_serato_markers".into(),
                    field: Some("cues".into()),
                    message: "Legacy Markers_ retained without decoded replacement cues".into(),
                    ..Default::default()
                }),
                Kind::Markers => {
                    let (cues, entries, issues) = markers(&body, ogg, limits)?;
                    out.cues = if cues.is_empty() {
                        Presence::Empty(cues)
                    } else {
                        Presence::Value(cues)
                    };
                    out.extensions
                        .insert("marker_entries".into(), json!(entries));
                    out.issues.extend(issues);
                }
                Kind::Grid => out.grid = Presence::Value(grid(&body, ogg)?),
                Kind::Autotags => out.autotags = Presence::Value(autotags(&body)?),
            }
            Ok(())
        })();
        if let Err(error) = result {
            out.issues.push(ImportIssue {
                code: "serato_performance_decode_failed".into(),
                field: Some(record.native_id.clone()),
                message: error.to_string(),
                ..Default::default()
            });
        }
    }
    for (key, count) in seen {
        if count > 1 {
            match key {
                0 => out.cues = Presence::Absent,
                1 => out.grid = Presence::Absent,
                2 => out.autotags = Presence::Absent,
                _ => {}
            }
            out.issues.push(ImportIssue {
                code: "ambiguous_serato_performance_tags".into(),
                message: "Multiple source tags retained; no query-order replacement selected"
                    .into(),
                ..Default::default()
            });
        }
    }
    out.extensions.insert("raw_sources".into(), json!(raw));
    Ok(out)
}

pub(super) fn emit_native_track(
    mut track: TrackObservation,
    options: &ReadOptions,
    sink: &mut dyn ObservationSink,
    cancel: &CancellationToken,
) -> Result<()> {
    cancel.check()?;
    let path = resolve_location(&track.locations, &options.path_mappings)?.resolved;
    if let Some(path) = path.filter(|p| p.is_file()) {
        match read_file_tags(&path, &options.limits, cancel) {
            Ok(mut tags) => {
                let decoded = decode_serato_performance(&tags, &options.limits)?;
                if let Presence::Value(cues) | Presence::Empty(cues) = &decoded.cues {
                    track.cues = cues.clone();
                }
                if let Presence::Value(grid) = &decoded.grid {
                    track.grid = Some(grid.clone());
                }
                if !matches!(decoded.cues, Presence::Absent)
                    && let Some(entries) = decoded
                        .extensions
                        .get("marker_entries")
                        .and_then(Value::as_array)
                {
                    for entry in entries {
                        if entry["name"] == "COLOR" && !track.metadata.contains_key("color") {
                            track.metadata.insert(
                                "color".into(),
                                Presence::Value(json!({"raw":entry["value"],"scale_max":16777215})),
                            );
                        }
                        if entry["name"] == "BPMLOCK" {
                            if let Some(bpm) = &mut track.bpm
                                && bpm.locked.is_none()
                            {
                                bpm.locked = entry["value"].as_bool();
                            }
                            if let Some(grid) = &mut track.grid {
                                grid.extensions
                                    .insert("file_owned_lock".into(), entry["value"].clone());
                            }
                        }
                    }
                }
                if track.bpm.is_none()
                    && let Presence::Value(values) = &decoded.autotags
                    && let Some(bpm) = values["bpm"].as_f64().filter(|b| *b > 0.)
                {
                    track.bpm = Some(ScalarBpm {
                        presence: FieldPresence::Value,
                        value: Some(bpm),
                        provenance: Some("serato_autotags".into()),
                        ..Default::default()
                    });
                }
                track.issues.extend(decoded.issues.clone());
                track
                    .extensions
                    .insert("serato_performance".into(), serde_json::to_value(decoded)?);
                for data in tags.blobs.values() {
                    sink.put_blob(data, None)?;
                }
                tags.blobs.clear();
                track.file_tags =
                    Some(sink.put_blob(&serde_json::to_vec(&tags)?, Some("application/json"))?);
            }
            Err(error) => {
                cancel.check()?;
                issue(&mut track, "serato_performance_unavailable", "cues/grid");
                track
                    .extensions
                    .insert("serato_performance_error".into(), json!(error.to_string()));
            }
        }
    } else {
        issue(&mut track, "serato_performance_unavailable", "cues/grid");
    }
    let options = ReadOptions {
        capture_file_tags: false,
        ..options.clone()
    };
    emit_track(track, &options, sink, cancel)
}
