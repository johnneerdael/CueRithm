//! Inbound proto2 decoding, deliberately separate from strict writer validation.
use super::*;
#[derive(Debug)]
enum Wire<'a> {
    Varint(u64),
    Double([u8; 8]),
    Bytes(&'a [u8]),
    Other,
}
fn varint(bytes: &[u8], at: &mut usize) -> Result<u64> {
    let mut value = 0u64;
    for index in 0..10 {
        let byte = *bytes.get(*at).context("Truncated protobuf varint")?;
        *at += 1;
        ensure!(index != 9 || byte <= 1, "Protobuf varint overflow");
        value |= u64::from(byte & 127) << (7 * index);
        if byte & 128 == 0 {
            return Ok(value);
        }
    }
    anyhow::bail!("Protobuf varint overflow")
}
fn fields<'a>(bytes: &'a [u8], depth: usize) -> Result<Vec<(u32, Wire<'a>)>> {
    parse_fields(bytes, &mut 0, depth, None)
}
fn parse_fields<'a>(
    bytes: &'a [u8],
    at: &mut usize,
    depth: usize,
    end_group: Option<u32>,
) -> Result<Vec<(u32, Wire<'a>)>> {
    ensure!(depth <= 128, "Protobuf nesting limit");
    let mut result = Vec::new();
    while *at < bytes.len() {
        ensure!(result.len() < 1_000_000, "Protobuf field count limit");
        let tag = varint(bytes, at)?;
        let field = u32::try_from(tag >> 3)?;
        ensure!(
            field > 0 && field < 1 << 29,
            "Invalid protobuf field number"
        );
        if tag & 7 == 4 {
            ensure!(end_group == Some(field), "Mismatched protobuf end group");
            return Ok(result);
        }
        let value = match tag & 7 {
            0 => Wire::Varint(varint(bytes, at)?),
            1 => {
                let end = at.checked_add(8).context("Protobuf length overflow")?;
                let raw = bytes.get(*at..end).context("Truncated protobuf fixed64")?;
                *at = end;
                Wire::Double(raw.try_into()?)
            }
            2 => {
                let length = usize::try_from(varint(bytes, at)?)?;
                let end = at.checked_add(length).context("Protobuf length overflow")?;
                let raw = bytes.get(*at..end).context("Truncated protobuf bytes")?;
                *at = end;
                Wire::Bytes(raw)
            }
            3 => {
                parse_fields(bytes, at, depth + 1, Some(field))?;
                Wire::Other
            }
            5 => {
                *at = at
                    .checked_add(4)
                    .filter(|end| *end <= bytes.len())
                    .context("Truncated protobuf fixed32")?;
                Wire::Other
            }
            _ => anyhow::bail!("Unsupported protobuf wire type"),
        };
        result.push((field, value));
    }
    ensure!(end_group.is_none(), "Unterminated protobuf group");
    Ok(result)
}
fn one<'a, 'b>(fields: &'b [(u32, Wire<'a>)], number: u32) -> Result<Option<&'b Wire<'a>>> {
    let mut matching = fields.iter().filter(|(field, _)| *field == number);
    let value = matching.next().map(|(_, value)| value);
    ensure!(
        matching.next().is_none(),
        "Ambiguous duplicate protobuf scalar"
    );
    Ok(value)
}
fn integer(fields: &[(u32, Wire<'_>)], number: u32) -> Result<Option<u64>> {
    match one(fields, number)? {
        None => Ok(None),
        Some(Wire::Varint(v)) => Ok(Some(*v)),
        _ => anyhow::bail!("Wrong protobuf integer wire type"),
    }
}
fn double(fields: &[(u32, Wire<'_>)], number: u32) -> Result<Option<f64>> {
    match one(fields, number)? {
        None => Ok(None),
        Some(Wire::Double(v)) => {
            let value = f64::from_le_bytes(*v);
            ensure!(value.is_finite(), "Nonfinite protobuf value");
            Ok(Some(value))
        }
        _ => anyhow::bail!("Wrong protobuf double wire type"),
    }
}
fn message<'a>(fields: &[(u32, Wire<'a>)], number: u32) -> Result<Option<&'a [u8]>> {
    match one(fields, number)? {
        None => Ok(None),
        Some(Wire::Bytes(v)) => Ok(Some(v)),
        _ => anyhow::bail!("Wrong protobuf bytes wire type"),
    }
}
fn beat(bytes: &[u8], rate: f64) -> Result<GridPoint> {
    let values = fields(bytes, 1)?;
    let frame = integer(&values, 1)?
        .context("Missing optional beat frame; no zero anchor inferred")? as u32
        as i32;
    let enabled = integer(&values, 2)?.map(|v| v != 0);
    let source = integer(&values, 3)?;
    Ok(GridPoint {
        native_seconds: f64::from(frame) / rate,
        original: json!({"frame_position":frame,"enabled":enabled,"source":source}),
        ..Default::default()
    })
}
pub(super) fn grid(
    bytes: &[u8],
    version: &str,
    rate: f64,
    byte_order: Option<SourceByteOrder>,
) -> Result<GridObservation> {
    ensure!(rate.is_finite() && rate > 0., "Invalid source sample rate");
    let values = match fields(bytes, 0) {
        Ok(values) => values,
        Err(error) => {
            ensure!(
                matches!(version, "BeatGrid-1.0" | "BeatGrid-2.0") && bytes.len() == 16,
                "{error}"
            );
            let byte_order = byte_order.context("Legacy grid source byte order is unqualified")?;
            let decode = |bytes: &[u8]| -> Result<f64> {
                let bytes = bytes.try_into()?;
                Ok(match byte_order {
                    SourceByteOrder::Little => f64::from_le_bytes(bytes),
                    SourceByteOrder::Big => f64::from_be_bytes(bytes),
                })
            };
            let bpm = decode(&bytes[..8])?;
            let frame = decode(&bytes[8..])?;
            ensure!(
                bpm.is_finite() && bpm > 0. && frame.is_finite(),
                "Invalid legacy grid"
            );
            let seconds = frame / rate;
            ensure!(seconds.is_finite(), "Invalid legacy frame time");
            return Ok(GridObservation {
                representation: "constant".into(),
                original_time_base: Some("audio_frames".into()),
                sample_rate: Some(rate),
                native_first_beat_seconds: Some(seconds),
                bpm: Some(bpm),
                points: vec![GridPoint {
                    native_seconds: seconds,
                    bpm: Some(bpm),
                    original: json!({"frame_position":frame,"legacy":true}),
                    ..Default::default()
                }],
                extensions: BTreeMap::from([(
                    "legacy_byte_order".into(),
                    json!(match byte_order {
                        SourceByteOrder::Little => "little",
                        SourceByteOrder::Big => "big",
                    }),
                )]),
                ..Default::default()
            });
        }
    };
    let (representation, mut points, extensions) = match version {
        "BeatGrid-1.0" | "BeatGrid-2.0" => {
            let tempo = fields(
                message(&values, 1)?.context("Missing grid tempo message")?,
                1,
            )?;
            let bpm = double(&tempo, 1)?
                .filter(|v| *v > 0.)
                .context("Missing/invalid grid BPM")?;
            let mut first = beat(
                message(&values, 2)?.context("Missing first beat message")?,
                rate,
            )?;
            first.bpm = Some(bpm);
            (
                "constant",
                vec![first],
                BTreeMap::from([("bpm_source".into(), json!(integer(&tempo, 2)?))]),
            )
        }
        "BeatMap-1.0" => {
            let mut points = Vec::new();
            for (number, value) in &values {
                if *number == 1 {
                    let Wire::Bytes(bytes) = value else {
                        anyhow::bail!("Wrong beat message type")
                    };
                    points.push(beat(bytes, rate)?);
                }
            }
            ensure!(
                points.len() >= 2
                    && points
                        .windows(2)
                        .all(|p| p[1].native_seconds > p[0].native_seconds),
                "Invalid variable beat map"
            );
            for index in 0..points.len() - 1 {
                let bpm = 60. / (points[index + 1].native_seconds - points[index].native_seconds);
                ensure!(bpm.is_finite(), "Invalid beat interval");
                points[index].bpm = Some(bpm);
            }
            ("map", points, BTreeMap::new())
        }
        _ => anyhow::bail!("Unknown Mixxx beat codec version"),
    };
    let first = points.first().context("Empty grid")?;
    let result = GridObservation {
        representation: representation.into(),
        original_time_base: Some("audio_frames".into()),
        sample_rate: Some(rate),
        native_first_beat_seconds: Some(first.native_seconds),
        bpm: first.bpm,
        points: std::mem::take(&mut points),
        extensions,
        ..Default::default()
    };
    Ok(result)
}
pub(super) fn key(bytes: &[u8], version: &str, rate: Option<f64>) -> Result<MusicalKey> {
    ensure!(version == "KeyMap-1.0", "Unknown Mixxx key codec version");
    let values = fields(bytes, 0)?;
    let number = integer(&values, 1)?;
    let text = message(&values, 2)?.map(std::str::from_utf8).transpose()?;
    let normalized = number
        .filter(|v| (1..=24).contains(v))
        .map(|v| {
            let pitch = ((v - 1) % 12) as u8;
            let mode = if v <= 12 {
                camelot_core::Mode::Major
            } else {
                camelot_core::Mode::Minor
            };
            camelot_core::MusicalKey::new(pitch, mode).map(|key| {
                (
                    pitch,
                    if v <= 12 { "major" } else { "minor" },
                    key.camelot(),
                )
            })
        })
        .transpose()?;
    let mut changes = Vec::new();
    for (number, value) in &values {
        if *number == 3 {
            let Wire::Bytes(bytes) = value else {
                anyhow::bail!("Wrong key change message type")
            };
            let change = fields(bytes, 1)?;
            let frame = integer(&change, 1)?.map(|v| v as u32 as i32);
            changes.push(json!({"frame_position":frame,"native_seconds":frame.zip(rate).map(|(frame,rate)|f64::from(frame)/rate),"key":integer(&change,2)?,"tuning_frequency_hz":double(&change,3)?}));
        }
    }
    Ok(MusicalKey {
        presence: FieldPresence::Value,
        raw: text
            .map(str::to_owned)
            .or_else(|| number.map(|n| n.to_string())),
        camelot: normalized.as_ref().map(|n| n.2.clone()),
        pitch_class: normalized.as_ref().map(|n| n.0),
        mode: normalized.as_ref().map(|n| n.1.into()),
        provenance: Some("mixxx_key_map".into()),
        extensions: BTreeMap::from([
            ("global_key".into(), json!(number)),
            ("global_key_text".into(), json!(text)),
            ("source".into(), json!(integer(&values, 4)?)),
            (
                "global_tuning_frequency_hz".into(),
                json!(double(&values, 5)?),
            ),
            ("key_changes".into(), json!(changes)),
        ]),
    })
}
