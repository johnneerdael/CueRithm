use crate::PreparationSettings;
use anyhow::{Context, Result, bail};
use camelot_core::{
    FileAnalysis,
    lexicon_cues::DropAtStart,
    lexicon_publication::{self, CueTemplate, CueType, Marker, StructuralMarker, TemplateRow},
};
/// Publish freshly on both cache hits and misses; the analysis cache keeps raw
/// detector output. Final genre refresh happens before this method is called.
pub(crate) fn apply(result: &mut FileAnalysis, settings: &PreparationSettings) -> Result<()> {
    result.publication = None;
    if !settings.engine.lexicon_timing {
        return Ok(());
    }
    let tempo = result
        .analysis
        .tempo
        .as_ref()
        .context("Missing publication tempo")?;
    let raw = result
        .analysis
        .lexicon_cues
        .as_ref()
        .context("Missing structural markers")?;
    let mut markers = Vec::new();
    for cue in &raw.cues {
        let marker = match cue.name.as_str() {
            "Start" => Marker::Start,
            "Drop" => Marker::Drop,
            "Breakdown" => Marker::Breakdown,
            "Second Drop" => Marker::SecondDrop,
            "Second Breakdown" => Marker::SecondBreakdown,
            "Last beat" => Marker::Lastbeat,
            name => bail!("Unknown structural cue role: {name}"),
        };
        markers.push(StructuralMarker {
            marker_type: marker,
            start_time_seconds: Some(cue.time_seconds),
        });
    }
    let mut template = CueTemplate::default();
    let config = settings
        .engine
        .lexicon_config
        .as_ref()
        .context("Missing cue detector configuration")?;
    template.breakdown_min_beats = config.min_cue_length_beats;
    template.drop_at_start = match config.drop_at_start {
        DropAtStart::Never => "never",
        DropAtStart::Auto => "highEnergyOnly",
    }
    .into();
    if config.add_emergency_loop {
        template.rows.push(TemplateRow {
            checked: true,
            marker_type: Marker::EmergencyLoop,
            name: "S.O.S.".into(),
            label: Some("Emergency Loop".into()),
            cue_type: CueType::ActiveLoop,
            color: "magenta".into(),
            anchor: None,
            position: None,
            beats_offset: None,
            loop_duration: Some(16.),
        });
        if let Some(point) = &raw.emergency_loop {
            markers.push(StructuralMarker {
                marker_type: Marker::EmergencyLoop,
                start_time_seconds: Some(point.time_seconds),
            });
        }
    }
    markers.sort_by(|a, b| {
        a.start_time_seconds
            .unwrap()
            .total_cmp(&b.start_time_seconds.unwrap())
    });
    let first = tempo
        .downbeat_time
        .or_else(|| tempo.beats_seconds.first().copied())
        .context("Missing grid anchor")?;
    result.publication = Some(lexicon_publication::publish_measured(
        &markers,
        tempo.bpm,
        first,
        result.analysis.duration_seconds,
        &template,
        &[],
    )?);
    Ok(())
}
#[cfg(feature = "autotag")]
pub(crate) fn tag_bpm(
    result: &FileAnalysis,
    settings: &PreparationSettings,
) -> Result<Option<f64>> {
    result
        .analysis
        .tempo
        .as_ref()
        .map(|t| {
            if settings.engine.lexicon_timing {
                lexicon_publication::round_bpm(t.bpm)
            } else {
                Ok(t.bpm)
            }
        })
        .transpose()
}
