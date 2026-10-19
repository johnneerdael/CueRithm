use crate::{Library, PreparationSettings};
use anyhow::{Context, Result, bail};
use camelot_core::{
    FileAnalysis,
    lexicon_cues::DropAtStart,
    lexicon_publication::{
        self, CueTemplate, CueType, Marker, PublishedCue, StartBehavior, StructuralMarker,
        TemplateRow,
    },
};
/// Structural roles a user can anchor to a named existing cue, in the reference extraction order.
const ANCHOR_ROLES: [(Marker, &str); 7] = [
    (Marker::Start, "start"),
    (Marker::Drop, "drop"),
    (Marker::Breakdown, "breakdown"),
    (Marker::SecondDrop, "second_drop"),
    (Marker::SecondBreakdown, "second_breakdown"),
    (Marker::Lastbeat, "lastbeat"),
    (Marker::EmergencyLoop, "emergency_loop"),
];
/// Whether publication reads the track's current cues: for custom anchors, or a start cue that
/// follows the first existing cue. Other configurations never touch stored cues.
pub(crate) fn needs_existing_cues(settings: &PreparationSettings) -> bool {
    settings.engine.cue_template.as_ref().is_some_and(|t| {
        t.enable_custom_anchors
            || match t.start_cue_behavior {
                Some(behavior) => behavior == StartBehavior::AtExistingCue,
                None => t.use_first_cue_as_start == Some(true),
            }
    })
}
impl Library {
    /// Cues of the latest saved publication; any other revision shape contributes none.
    pub(crate) fn published_cues(&self, track_id: &str) -> Result<Vec<PublishedCue>> {
        let json: Option<String> = self.conn()?.query_row(
            "SELECT (SELECT json FROM cue_revisions WHERE track_id=?1 ORDER BY created DESC,rowid DESC LIMIT 1)",
            [track_id],
            |row| row.get(0),
        )?;
        let Some(value) = json.and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
        else {
            return Ok(Vec::new());
        };
        if value.get("schema").and_then(serde_json::Value::as_str) != Some("lexicon-publication-v1")
        {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_value::<lexicon_publication::Publication>(value)?.cues)
    }
}
/// Publish for one track, reading its stored cues only when the template asks for them.
pub(crate) fn apply_for_track(
    library: &Library,
    track_id: &str,
    result: &mut FileAnalysis,
    settings: &PreparationSettings,
) -> Result<()> {
    let existing = if needs_existing_cues(settings) {
        library.published_cues(track_id)?
    } else {
        Vec::new()
    };
    apply(result, settings, &existing)
}
/// Markers for the roles whose configured anchor name matches an existing cue; unmatched roles are absent.
fn anchored_markers(
    template: &CueTemplate,
    existing: &[PublishedCue],
    duration: f64,
) -> Vec<StructuralMarker> {
    ANCHOR_ROLES
        .iter()
        .filter_map(|(marker, key)| {
            let name = template.custom_anchors.get(*key)?.get("name")?.as_str()?;
            let cue = existing.iter().find(|cue| cue.name == name)?;
            (cue.start_time.is_finite() && (0. ..=duration).contains(&cue.start_time)).then_some(
                StructuralMarker {
                    marker_type: *marker,
                    start_time_seconds: Some(cue.start_time),
                },
            )
        })
        .collect()
}
/// Publish freshly on both cache hits and misses; the analysis cache keeps raw
/// detector output. Final genre refresh happens before this method is called.
pub(crate) fn apply(
    result: &mut FileAnalysis,
    settings: &PreparationSettings,
    existing: &[PublishedCue],
) -> Result<()> {
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
    let mut template = settings.engine.cue_template.clone().unwrap_or_default();
    // Every preparation replaces the previous cues; the detector settings below own these two fields.
    template.overwrite = true;
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
    template
        .rows
        .retain(|row| config.add_emergency_loop || row.marker_type != Marker::EmergencyLoop);
    if config.add_emergency_loop {
        if !template
            .rows
            .iter()
            .any(|row| row.marker_type == Marker::EmergencyLoop)
        {
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
        }
        if let Some(point) = &raw.emergency_loop {
            markers.push(StructuralMarker {
                marker_type: Marker::EmergencyLoop,
                start_time_seconds: Some(point.time_seconds),
            });
        }
    }
    // Custom anchors take marker positions from the track's own named cues. A track with none of
    // the named cues (a first preparation, for instance) keeps the detector's markers instead of
    // publishing nothing, which differs from the reference tool's all-or-nothing behavior.
    if template.enable_custom_anchors {
        let anchored = anchored_markers(&template, existing, result.analysis.duration_seconds);
        if !anchored.is_empty() {
            markers = anchored;
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
        existing,
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
#[cfg(test)]
mod tests {
    use super::*;
    use camelot_core::{
        AnalysisConfig, TempoEstimate,
        lexicon_cues::{EmergencyLoop, StructuralCue, StructuralCueResult},
        lexicon_publication::{OffsetPosition, PublishedType},
    };
    use serde_json::json;

    fn result() -> FileAnalysis {
        let mut analysis =
            camelot_core::analyze_pcm(&vec![0.; 11025], 11025, &AnalysisConfig::default()).unwrap();
        analysis.duration_seconds = 300.;
        analysis.tempo = Some(TempoEstimate {
            bpm: 120.,
            strength: None,
            candidates: vec![],
            beats_seconds: vec![0.],
            downbeats_seconds: None,
            downbeat_time: Some(0.),
            method: "test".into(),
        });
        let cue = |time_seconds, name: &str| StructuralCue {
            time_seconds,
            name: name.into(),
            color: "x".into(),
        };
        analysis.lexicon_cues = Some(StructuralCueResult {
            cues: vec![
                cue(8., "Start"),
                cue(64., "Drop"),
                cue(128., "Breakdown"),
                cue(280., "Last beat"),
            ],
            emergency_loop: None,
        });
        FileAnalysis {
            publication: None,
            analysis,
            complete: true,
            verified_integrity: None,
            source_sample_rate: 44100,
            beat_sample_rate: None,
            neural_key_sample_rate: None,
            openkeyscan_sample_rate: None,
            source_timing: None,
            elapsed_seconds: 0.,
            media: None,
        }
    }
    fn settings(template: Option<CueTemplate>) -> PreparationSettings {
        let mut settings = PreparationSettings::default();
        settings.engine.lexicon_timing = true;
        settings.engine.cue_template = template;
        settings
    }
    fn published(result: &FileAnalysis) -> Vec<(String, f64)> {
        result
            .publication
            .as_ref()
            .unwrap()
            .cues
            .iter()
            .map(|cue| (cue.name.clone(), cue.start_time))
            .collect()
    }
    fn saved(name: &str, start_time: f64) -> PublishedCue {
        PublishedCue {
            name: name.into(),
            start_time,
            kind: PublishedType::Cue,
            position: 0,
            color: json!("red"),
            end_time: None,
            active_loop: None,
            data: None,
        }
    }
    fn offset_row(anchor: Marker, position: OffsetPosition, beats: f64) -> TemplateRow {
        TemplateRow {
            checked: true,
            marker_type: Marker::Normal,
            name: "Pre-drop".into(),
            cue_type: CueType::HotCue,
            color: json!("green"),
            label: None,
            anchor: Some(anchor),
            position: Some(position),
            beats_offset: Some(beats),
            loop_duration: None,
        }
    }

    #[test]
    fn without_a_user_template_the_factory_rows_publish_exactly_as_before() {
        let mut plain = result();
        apply(&mut plain, &settings(None), &[]).unwrap();
        let mut factory = result();
        apply(&mut factory, &settings(Some(CueTemplate::default())), &[]).unwrap();
        assert_eq!(published(&plain), published(&factory));
        assert_eq!(
            published(&plain),
            [
                ("Start".to_string(), 8.),
                ("Drop".to_string(), 64.),
                ("Breakdown".to_string(), 128.),
                ("End / Fade-out".to_string(), 280.),
            ]
        );
        assert_eq!(
            plain.publication.unwrap().inputs_sha256,
            factory.publication.unwrap().inputs_sha256
        );
    }

    #[test]
    fn user_template_rows_add_beat_offset_cues_from_the_saved_tempo() {
        let mut template = CueTemplate::default();
        template
            .rows
            .push(offset_row(Marker::Drop, OffsetPosition::Before, 32.));
        let mut analysed = result();
        apply(&mut analysed, &settings(Some(template)), &[]).unwrap();
        // 32 beats at 120 BPM is 16 seconds.
        assert!(published(&analysed).contains(&("Pre-drop".to_string(), 48.)));
        assert_eq!(analysed.publication.unwrap().cues.len(), 5);
    }

    #[test]
    fn detector_settings_own_the_emergency_loop_row_but_keep_its_edits() {
        let mut template = CueTemplate::default();
        template.rows.push(TemplateRow {
            marker_type: Marker::EmergencyLoop,
            name: "Escape".into(),
            cue_type: CueType::ActiveLoop,
            color: json!("aqua"),
            loop_duration: Some(8.),
            ..offset_row(Marker::Drop, OffsetPosition::Before, 0.)
        });
        let template_row = template.rows.last_mut().unwrap();
        template_row.anchor = None;
        template_row.position = None;
        template_row.beats_offset = None;
        let mut disabled = settings(Some(template.clone()));
        disabled
            .engine
            .lexicon_config
            .as_mut()
            .unwrap()
            .add_emergency_loop = false;
        let mut analysed = result();
        apply(&mut analysed, &disabled, &[]).unwrap();
        assert!(
            published(&analysed)
                .iter()
                .all(|(name, _)| name != "Escape")
        );
        let mut enabled = settings(Some(template));
        enabled
            .engine
            .lexicon_config
            .as_mut()
            .unwrap()
            .add_emergency_loop = true;
        let mut with_loop = result();
        with_loop
            .analysis
            .lexicon_cues
            .as_mut()
            .unwrap()
            .emergency_loop = Some(EmergencyLoop {
            time_seconds: 200.,
            loop_beats: 16,
        });
        apply(&mut with_loop, &enabled, &[]).unwrap();
        let cues = &with_loop.publication.unwrap().cues;
        let escape = cues.iter().find(|cue| cue.name == "Escape").unwrap();
        assert_eq!(
            (escape.start_time, escape.kind),
            (200., PublishedType::Loop)
        );
        assert_eq!(escape.end_time, Some(204.));
    }

    #[test]
    fn custom_anchors_take_marker_positions_from_matching_existing_cues() {
        let template = CueTemplate {
            enable_custom_anchors: true,
            custom_anchors: json!({"drop": {"name": "My Drop"}}),
            ..CueTemplate::default()
        };
        let existing = [saved("Intro", 2.), saved("My Drop", 70.)];
        let mut analysed = result();
        apply(&mut analysed, &settings(Some(template)), &existing).unwrap();
        // Only the named role is anchored; unmatched roles are not borrowed from the detector.
        assert_eq!(published(&analysed), [("Drop".to_string(), 70.)]);
    }

    #[test]
    fn custom_anchors_fall_back_to_detector_markers_when_no_named_cue_exists() {
        let template = CueTemplate {
            enable_custom_anchors: true,
            custom_anchors: json!({"drop": {"name": "My Drop"}}),
            ..CueTemplate::default()
        };
        let mut first_preparation = result();
        apply(
            &mut first_preparation,
            &settings(Some(template.clone())),
            &[],
        )
        .unwrap();
        assert_eq!(published(&first_preparation).len(), 4);
        let mut unrelated = result();
        apply(
            &mut unrelated,
            &settings(Some(template)),
            &[saved("Something else", 30.)],
        )
        .unwrap();
        assert_eq!(published(&unrelated).len(), 4);
    }

    #[test]
    fn anchors_outside_the_track_are_ignored() {
        let template = CueTemplate {
            enable_custom_anchors: true,
            custom_anchors: json!({"drop": {"name": "My Drop"}}),
            ..CueTemplate::default()
        };
        let mut analysed = result();
        apply(
            &mut analysed,
            &settings(Some(template)),
            &[saved("My Drop", 9999.)],
        )
        .unwrap();
        assert_eq!(published(&analysed).len(), 4);
    }

    #[test]
    fn start_can_follow_the_first_existing_cue_and_only_then_reads_stored_cues() {
        let at_existing = CueTemplate {
            start_cue_behavior: Some(StartBehavior::AtExistingCue),
            ..CueTemplate::default()
        };
        assert!(needs_existing_cues(&settings(Some(at_existing.clone()))));
        assert!(!needs_existing_cues(&settings(None)));
        assert!(!needs_existing_cues(&settings(
            Some(CueTemplate::default())
        )));
        let mut analysed = result();
        apply(
            &mut analysed,
            &settings(Some(at_existing)),
            &[saved("Mine", 3.5)],
        )
        .unwrap();
        assert_eq!(published(&analysed)[0], ("Start".to_string(), 3.5));
        // Offsets stay tied to the structural anchor even when Start moves.
        let mut zero = result();
        apply(
            &mut zero,
            &settings(Some(CueTemplate {
                start_cue_behavior: Some(StartBehavior::AtZero),
                ..CueTemplate::default()
            })),
            &[],
        )
        .unwrap();
        assert_eq!(published(&zero)[0], ("Start".to_string(), 0.));
    }

    #[test]
    fn stored_cues_come_from_the_latest_publication_only() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(&dir.path().join("db")).unwrap();
        assert!(library.published_cues("unknown-track").unwrap().is_empty());
    }
}
