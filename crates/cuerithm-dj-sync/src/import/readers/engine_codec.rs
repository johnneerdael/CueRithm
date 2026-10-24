//! Bounded mixed-endian Engine performance codecs, independent of the writer.
use super::*;
use flate2::read::ZlibDecoder;
use std::io::Read;
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|n| *n <= self.bytes.len())
            .context("Engine field exceeds payload")?;
        let value = &self.bytes[self.at..end];
        self.at = end;
        Ok(value)
    }
    fn be64(&mut self) -> Result<f64> {
        let value = f64::from_be_bytes(self.take(8)?.try_into()?);
        ensure!(value.is_finite(), "Nonfinite Engine native value");
        Ok(value)
    }
    fn le64(&mut self) -> Result<f64> {
        let value = f64::from_le_bytes(self.take(8)?.try_into()?);
        ensure!(value.is_finite(), "Nonfinite Engine native value");
        Ok(value)
    }
    fn count(&mut self, be: bool, minimum: usize) -> Result<usize> {
        let bytes = self.take(8)?.try_into()?;
        let n = usize::try_from(if be {
            u64::from_be_bytes(bytes)
        } else {
            u64::from_le_bytes(bytes)
        })?;
        ensure!(
            n <= 65536 && n <= (self.bytes.len() - self.at) / minimum,
            "Engine record count exceeds payload/limit"
        );
        Ok(n)
    }
    fn name(&mut self) -> Result<String> {
        let n = self.take(1)?[0] as usize;
        Ok(std::str::from_utf8(self.take(n)?)?.into())
    }
}
fn inflate(bytes: &[u8], limits: &ImportLimits) -> Result<Vec<u8>> {
    ensure!(
        bytes.len() >= 4 && bytes.len() as u64 <= limits.blob_bytes,
        "Engine compressed byte limit"
    );
    let declared = u32::from_be_bytes(bytes[..4].try_into()?) as u64;
    ensure!(
        declared <= limits.blob_bytes,
        "Engine inflated size exceeds cap"
    );
    let mut decoder = ZlibDecoder::new(&bytes[4..]);
    let mut out = Vec::new();
    decoder.by_ref().take(declared + 1).read_to_end(&mut out)?;
    ensure!(
        out.len() as u64 == declared && decoder.total_in() as usize == bytes.len() - 4,
        "Engine declared/inflated size or stream mismatch"
    );
    Ok(out)
}
pub(super) struct Beats {
    pub rate: Option<f64>,
    pub grid: Option<GridObservation>,
    pub native: Value,
}
pub(super) fn beats(
    bytes: &[u8],
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<Beats> {
    let body = inflate(bytes, limits)?;
    let mut cursor = Cursor {
        bytes: &body,
        at: 0,
    };
    let source_rate = cursor.be64()?;
    let rate = (source_rate > 0.).then_some(source_rate);
    let sample_length = cursor.take(8)?.to_vec();
    let flag = cursor.take(1)?[0];
    let mut lists = Vec::new();
    for _ in 0..2 {
        let count = cursor.count(true, 24)?;
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            cancel.check()?;
            let position = cursor.le64()?;
            let beat = i64::from_le_bytes(cursor.take(8)?.try_into()?);
            let span = u32::from_le_bytes(cursor.take(4)?.try_into()?);
            let reserved = cursor.take(4)?;
            ensure!(
                (-100000..=100000).contains(&beat) && span <= 100000,
                "Engine beat index/span outside profile"
            );
            rows.push(json!({"position_samples":position,"beat_index":beat,"beats_till_next":span,"reserved":reserved}));
        }
        lists.push(rows);
    }
    let footer = cursor.take(1)?[0];
    ensure!(cursor.at == body.len(), "Trailing Engine grid bytes");
    let native = json!({"sample_rate":source_rate,"sample_length_bytes":sample_length,"native_flag":flag,"native_footer":footer,"first_list":lists[0],"second_list":lists[1]});
    let grid = if let Some(rate) = rate {
        let mut points = Vec::<GridPoint>::new();
        for (index, row) in lists[1].iter().enumerate() {
            cancel.check()?;
            let position = row["position_samples"]
                .as_f64()
                .context("Invalid beat position")?
                / rate;
            ensure!(position.is_finite(), "Invalid Engine sample time");
            let bpm = if index + 1 < lists[1].len() {
                let delta = (lists[1][index + 1]["position_samples"]
                    .as_f64()
                    .context("Invalid next beat")?
                    - row["position_samples"].as_f64().context("Invalid beat")?)
                    / rate;
                ensure!(delta > 0., "Nonincreasing Engine grid");
                let span = row["beats_till_next"].as_u64().context("Invalid span")?;
                if span > 0 {
                    let bpm = 60. * span as f64 / delta;
                    ensure!(bpm.is_finite() && bpm > 0., "Invalid Engine BPM");
                    Some(bpm)
                } else {
                    None
                }
            } else {
                None
            };
            points.push(GridPoint {
                native_seconds: position,
                beat_index: row["beat_index"].as_i64(),
                bpm,
                original: row.clone(),
                ..Default::default()
            });
        }
        if points.is_empty() {
            None
        } else {
            Some(GridObservation {
                representation: "segments".into(),
                original_time_base: Some("audio_samples".into()),
                sample_rate: Some(rate),
                native_first_beat_seconds: Some(points[0].native_seconds),
                bpm: points[0].bpm,
                points,
                extensions: BTreeMap::from([
                    ("first_list".into(), json!(lists[0])),
                    ("native_flag".into(), json!(flag)),
                    ("native_footer".into(), json!(footer)),
                    ("sample_length_bytes".into(), json!(sample_length)),
                ]),
                ..Default::default()
            })
        }
    } else {
        None
    };
    Ok(Beats { rate, grid, native })
}
pub(super) fn quick(
    bytes: &[u8],
    rate: Option<f64>,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<Vec<CueObservation>> {
    let body = inflate(bytes, limits)?;
    let mut cursor = Cursor {
        bytes: &body,
        at: 0,
    };
    let count = cursor.count(true, 13)?;
    let mut out = Vec::new();
    for slot in 0..count {
        cancel.check()?;
        let name = cursor.name()?;
        let samples = cursor.be64()?;
        let flags = cursor.take(4)?;
        if samples == -1. {
            continue;
        }
        let seconds = rate.map(|r| samples / r);
        ensure!(
            seconds.is_none_or(f64::is_finite),
            "Invalid cue sample time"
        );
        out.push(CueObservation {
            external_id: format!("hot:{slot}"),
            family: CueFamily::Hot,
            source_slot: Some(i32::try_from(slot)?),
            name: Some(name),
            color_rgb: Some(u32::from_be_bytes([0, flags[1], flags[2], flags[3]])),
            native_start_seconds: seconds,
            original: json!({"position_samples":samples,"alpha_flag":flags[0],"slot":slot}),
            flags: BTreeMap::from([("source_slot_role".into(), json!("hot"))]),
            ..Default::default()
        });
    }
    Ok(out)
}
pub(super) fn loops(
    bytes: &[u8],
    rate: Option<f64>,
    active: u64,
    limits: &ImportLimits,
    cancel: &CancellationToken,
) -> Result<Vec<CueObservation>> {
    ensure!(bytes.len() as u64 <= limits.blob_bytes, "Loop byte limit");
    let mut cursor = Cursor { bytes, at: 0 };
    let count = cursor.count(false, 23)?;
    let mut out = Vec::new();
    for slot in 0..count {
        cancel.check()?;
        let name = cursor.name()?;
        let start = cursor.le64()?;
        let end = cursor.le64()?;
        let flags = cursor.take(6)?;
        if start == -1. && end == -1. {
            continue;
        }
        ensure!(
            start == -1. || end == -1. || end > start,
            "Invalid Engine loop range"
        );
        let first = (start != -1.).then(|| rate.map(|r| start / r)).flatten();
        let last = (end != -1.).then(|| rate.map(|r| end / r)).flatten();
        ensure!(
            first.is_none_or(f64::is_finite) && last.is_none_or(f64::is_finite),
            "Invalid loop sample time"
        );
        out.push(CueObservation {
            external_id: format!("loop:{slot}"),
            family: CueFamily::Loop,
            source_slot: Some(i32::try_from(slot)?),
            name: Some(name),
            color_rgb: Some(u32::from_be_bytes([0, flags[3], flags[4], flags[5]])),
            native_start_seconds: first,
            native_end_seconds: last,
            original: json!({"start_samples":start,"end_samples":end,"flags":flags,"slot":slot}),
            flags: BTreeMap::from([
                ("source_slot_role".into(), json!("saved_loop")),
                (
                    "active_on_load".into(),
                    json!(
                        1u64.checked_shl(slot as u32)
                            .is_some_and(|bit| active & bit != 0)
                    ),
                ),
            ]),
            ..Default::default()
        });
    }
    ensure!(cursor.at == bytes.len(), "Trailing Engine loop bytes");
    Ok(out)
}
