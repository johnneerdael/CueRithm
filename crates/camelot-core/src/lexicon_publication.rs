//! Publish structural markers through a cue template; raw DSP markers stay separate.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Marker {
    Start,
    Drop,
    Breakdown,
    SecondDrop,
    SecondBreakdown,
    Lastbeat,
    EmergencyLoop,
    Normal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CueType {
    #[serde(rename = "hotcue")]
    HotCue,
    #[serde(rename = "loop_hot")]
    HotLoop,
    #[serde(rename = "loop_active")]
    ActiveLoop,
    #[serde(rename = "memory_cue")]
    MemoryCue,
    #[serde(rename = "memory_cue_loop")]
    MemoryLoop,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartBehavior {
    AtFirstBeat,
    AtExistingCue,
    AtZero,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OffsetPosition {
    Before,
    After,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRow {
    pub checked: bool,
    pub marker_type: Marker,
    pub name: String,
    pub cue_type: CueType,
    pub color: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<Marker>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<OffsetPosition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beats_offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_duration: Option<f64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CueTemplate {
    pub overwrite: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_cue_behavior: Option<StartBehavior>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_first_cue_as_start: Option<bool>,
    pub breakdown_min_beats: u32,
    pub keep_cue_positions: bool,
    pub enable_custom_anchors: bool,
    pub drop_at_start: String,
    pub custom_anchors: Value,
    pub rows: Vec<TemplateRow>,
}
impl Default for CueTemplate {
    fn default() -> Self {
        let rows = [
            (Marker::Start, "Start", "Start", "yellow"),
            (Marker::Drop, "Drop", "Drop", "red"),
            (Marker::Breakdown, "Breakdown", "Breakdown", "blue"),
            (Marker::SecondDrop, "Second Drop", "Drop #2", "red"),
            (
                Marker::SecondBreakdown,
                "Second Breakdown",
                "Breakdown #2",
                "blue",
            ),
            (
                Marker::Lastbeat,
                "End / Fade-out",
                "End / Fade-out",
                "violet",
            ),
        ]
        .into_iter()
        .map(|(marker, label, name, color)| TemplateRow {
            checked: true,
            marker_type: marker,
            name: name.into(),
            label: Some(label.into()),
            cue_type: CueType::HotCue,
            color: color.into(),
            anchor: None,
            position: None,
            beats_offset: None,
            loop_duration: None,
        })
        .collect();
        Self {
            overwrite: true,
            start_cue_behavior: Some(StartBehavior::AtFirstBeat),
            use_first_cue_as_start: None,
            breakdown_min_beats: 64,
            keep_cue_positions: false,
            enable_custom_anchors: false,
            drop_at_start: "never".into(),
            custom_anchors: serde_json::json!({}),
            rows,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuralMarker {
    pub marker_type: Marker,
    pub start_time_seconds: Option<f64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PublishedType {
    #[serde(rename = "1")]
    Cue,
    #[serde(rename = "5")]
    Loop,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedCue {
    pub name: String,
    pub start_time: f64,
    #[serde(rename = "type")]
    pub kind: PublishedType,
    pub position: u32,
    pub color: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_time: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_loop: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicationContext {
    pub bpm: f64,
    pub duration: f64,
    pub cuepoints: Vec<PublishedCue>,
}

/// Match the renderer's two-decimal save boundary, including its 15-digit
/// decimal correction before rounding. Detection and structural times stay raw.
pub fn round_bpm(bpm: f64) -> Result<f64> {
    ensure!(
        bpm.is_finite() && bpm > 0. && bpm <= 1000.,
        "Invalid publication BPM"
    );
    let scaled = bpm * 100.;
    let corrected = format!("{scaled:.14e}").parse::<f64>()?;
    Ok(corrected.round() / 100.)
}
impl CueTemplate {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.rows.len() <= 10_000, "Too many cue template rows");
        for row in &self.rows {
            ensure!(row.name.len() <= 4096, "Cue name is too long");
            ensure!(
                row.color.is_null() || row.color.as_str().is_some_and(|s| s.len() <= 64),
                "Invalid cue color"
            );
            ensure!(
                row.beats_offset
                    .is_none_or(|v| v.is_finite() && (0. ..=1e9).contains(&v)),
                "Invalid cue offset"
            );
            ensure!(
                row.loop_duration
                    .is_none_or(|v| v.is_finite() && (0. ..=100_000.).contains(&v)),
                "Invalid loop duration"
            );
        }
        Ok(())
    }
}
#[derive(Clone)]
struct Point {
    marker: Marker,
    time: Option<f64>,
    row: Option<usize>,
    anchor: Option<Marker>,
}
fn first(points: &[Point], kind: Marker) -> Option<usize> {
    points
        .iter()
        .position(|p| p.marker == kind && p.time.is_some())
}
fn same_slot(a: &TemplateRow, b: &TemplateRow) -> bool {
    a.cue_type == b.cue_type
        && a.marker_type == b.marker_type
        && a.anchor == b.anchor
        && a.position == b.position
        && a.beats_offset == b.beats_offset
}
/// Apply the renderer's template semantics to already measured markers. This
/// does not perform detection or merge old cues; overwrite gating belongs to
/// the caller. `enable_custom_anchors` assumes that its caller supplied those
/// marker positions, just as the renderer's preceding anchor-extraction stage.
pub fn publish(
    markers: &[StructuralMarker],
    context: &PublicationContext,
    template: &CueTemplate,
) -> Result<Vec<PublishedCue>> {
    template.validate()?;
    ensure!(
        context.bpm.is_finite() && context.bpm > 0. && context.bpm <= 1000.,
        "Invalid publication BPM"
    );
    ensure!(
        context.duration.is_finite() && context.duration > 0.,
        "Invalid publication duration"
    );
    ensure!(
        markers.len() <= 10_000 && context.cuepoints.len() <= 10_000,
        "Too many publication markers"
    );
    ensure!(
        markers.iter().all(|p| p
            .start_time_seconds
            .is_none_or(|t| t.is_finite() && t >= 0. && t <= context.duration)),
        "Invalid structural marker time"
    );
    ensure!(
        context.cuepoints.iter().all(|c| c.start_time.is_finite()
            && c.start_time >= 0.
            && c.end_time
                .is_none_or(|v| v.is_finite() && v >= c.start_time)),
        "Invalid retained cue timing"
    );
    if markers.is_empty() && !template.enable_custom_anchors {
        return Ok(context.cuepoints.clone());
    }
    let beat = 60. / context.bpm;
    ensure!(beat.is_finite(), "Invalid publication beat duration");
    let mut points = markers
        .iter()
        .map(|m| Point {
            marker: m.marker_type,
            time: m.start_time_seconds,
            row: None,
            anchor: None,
        })
        .collect::<Vec<_>>();
    let kinds = [
        Marker::Start,
        Marker::Drop,
        Marker::Breakdown,
        Marker::SecondDrop,
        Marker::SecondBreakdown,
        Marker::Lastbeat,
        Marker::EmergencyLoop,
    ];
    let mut anchors = std::collections::BTreeMap::new();
    for kind in kinds {
        let Some(i) = first(&points, kind) else {
            continue;
        };
        let parent = match kind {
            Marker::SecondDrop => Some(Marker::Drop),
            Marker::SecondBreakdown => Some(Marker::Breakdown),
            _ => None,
        };
        if let Some(parent) = parent {
            let Some(parent) = first(&points, parent) else {
                continue;
            };
            if points[i].time == points[parent].time {
                continue;
            }
        }
        anchors.insert(kind, points[i].time.unwrap());
        let checked_only = matches!(kind, Marker::Lastbeat | Marker::EmergencyLoop);
        points[i].row = template
            .rows
            .iter()
            .position(|r| r.marker_type == kind && (!checked_only || r.checked));
    }
    for (row_index, row) in template
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.checked && r.marker_type == Marker::Normal)
    {
        let Some(anchor) = row.anchor else {
            continue;
        };
        let Some(mut time) = anchors.get(&anchor).copied() else {
            continue;
        };
        match row.position {
            Some(OffsetPosition::Before) => {
                time -= beat * row.beats_offset.context("Missing cue offset")?;
            }
            Some(OffsetPosition::After) => {
                time += beat * row.beats_offset.context("Missing cue offset")?;
            }
            None => {}
        }
        if !(0. ..=context.duration).contains(&time) {
            continue;
        }
        let point = Point {
            marker: Marker::Normal,
            time: Some(time),
            row: Some(row_index),
            anchor: Some(anchor),
        };
        if let Some(index) = points.iter().position(|p| p.marker == anchor) {
            let count = points.iter().filter(|p| p.anchor == Some(anchor)).count();
            let index = if row.position == Some(OffsetPosition::After) {
                index + 1 + count
            } else {
                index
            };
            points.insert(index.min(points.len()), point);
        } else {
            points.push(point);
        }
    }
    if let Some(start) = first(&points, Marker::Start) {
        match template.start_cue_behavior.unwrap_or(
            if template.use_first_cue_as_start == Some(true) {
                StartBehavior::AtExistingCue
            } else {
                StartBehavior::AtFirstBeat
            },
        ) {
            StartBehavior::AtZero => points[start].time = Some(0.),
            StartBehavior::AtExistingCue => {
                if let Some(cue) = context.cuepoints.first() {
                    points[start].time = Some(cue.start_time);
                }
            }
            StartBehavior::AtFirstBeat => {}
        }
    }
    let mut out = vec![];
    let mut emergency = None;
    for (index, point) in points.iter().enumerate() {
        let Some(row) = point.row.map(|i| &template.rows[i]).filter(|r| r.checked) else {
            continue;
        };
        let time = point.time.context("Missing cue time")?;
        ensure!(time.is_finite(), "Nonfinite published cue time");
        let position = if template.keep_cue_positions {
            template
                .rows
                .iter()
                .position(|r| same_slot(r, row))
                .context("Missing cue template position")?
        } else {
            index
        };
        let mut cue = PublishedCue {
            name: row.name.clone(),
            start_time: time,
            kind: PublishedType::Cue,
            position: u32::try_from(position)?,
            color: row.color.clone(),
            end_time: None,
            active_loop: None,
            data: None,
        };
        let loop_length = if point.marker == Marker::Lastbeat {
            0.
        } else {
            row.loop_duration.unwrap_or(0.)
        };
        if loop_length != 0.
            && matches!(
                row.cue_type,
                CueType::HotLoop | CueType::ActiveLoop | CueType::MemoryLoop
            )
        {
            cue.kind = PublishedType::Loop;
            let end = time + beat * loop_length;
            ensure!(end.is_finite(), "Nonfinite loop end time");
            cue.end_time = Some(end);
            if row.cue_type == CueType::ActiveLoop {
                cue.active_loop = Some(true);
            }
        }
        if matches!(row.cue_type, CueType::MemoryCue | CueType::MemoryLoop) {
            cue.data = Some(serde_json::json!({"rekordbox":{"originalNum":-1}}));
        }
        if point.marker == Marker::EmergencyLoop {
            emergency = Some(out.len());
        }
        out.push(cue);
    }
    if let Some(index) = emergency.filter(|_| !template.keep_cue_positions) {
        let cue = &out[index];
        let max = out
            .iter()
            .filter(|c| c.kind != cue.kind && c.start_time != cue.start_time)
            .map(|c| c.position)
            .max()
            .context("Emergency loop has no finite publication position")?;
        out[index].position = max.checked_add(1).context("Cue position overflow")?;
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedGrid {
    pub bpm: f64,
    pub start_time: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    pub grid: SavedGrid,
    pub cues: Vec<PublishedCue>,
    pub implementation_sha256: String,
    pub inputs_sha256: String,
}
pub fn implementation_sha256() -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(include_bytes!("lexicon_publication.rs"))
    )
}
/// Full-analysis publication saves rounded BPM before applying beat-relative
/// template offsets. Cue-only publication can instead use `publish` with the
/// track's existing timing, which the original renderer preserves.
pub fn publish_measured(
    markers: &[StructuralMarker],
    measured_bpm: f64,
    first_beat: f64,
    duration: f64,
    template: &CueTemplate,
    existing: &[PublishedCue],
) -> Result<Publication> {
    use sha2::{Digest, Sha256};
    let bpm = round_bpm(measured_bpm)?;
    ensure!(
        first_beat.is_finite() && first_beat >= 0. && first_beat <= duration,
        "Invalid grid anchor"
    );
    let context = PublicationContext {
        bpm,
        duration,
        cuepoints: existing.to_vec(),
    };
    let cues = publish(markers, &context, template)?;
    let inputs_sha256 = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(
            markers,
            measured_bpm,
            first_beat,
            duration,
            template,
            existing
        ))?)
    );
    Ok(Publication {
        grid: SavedGrid {
            bpm,
            start_time: first_beat,
        },
        cues,
        implementation_sha256: implementation_sha256(),
        inputs_sha256,
    })
}
