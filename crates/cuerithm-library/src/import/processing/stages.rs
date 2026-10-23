use super::*;
use camelot_core::{AnalysisSelection, Engine};
#[derive(Clone)]
pub(super) struct Stage {
    pub name: &'static str,
    pub field: &'static str,
    pub selection: AnalysisSelection,
    pub blocker: Option<String>,
}
fn missing(value: Option<&Value>) -> bool {
    value.is_none_or(|v| v.get("presence").and_then(Value::as_str) == Some("absent"))
}
pub(super) fn plan(policy: &ImportPolicy, fields: &BTreeMap<String, Value>) -> Vec<Stage> {
    let mut stages = Vec::new();
    for (field, mode) in &policy.analysis_fields {
        let (name, column, selection) = match field {
            AnalysisField::Key => (
                "analysis.key",
                "key",
                AnalysisSelection {
                    key: true,
                    ..Default::default()
                },
            ),
            AnalysisField::Bpm => (
                "analysis.bpm",
                "bpm",
                AnalysisSelection {
                    bpm: true,
                    ..Default::default()
                },
            ),
            AnalysisField::Beatgrid => (
                "analysis.beatgrid",
                "beatgrid",
                AnalysisSelection {
                    beatgrid: true,
                    ..Default::default()
                },
            ),
            AnalysisField::Energy => (
                "analysis.energy",
                "energy",
                AnalysisSelection {
                    energy: true,
                    ..Default::default()
                },
            ),
        };
        if !super::super::projection::field_selected(policy, column) {
            continue;
        }
        if *mode == AnalysisAdoption::FillMissing && !missing(fields.get(column)) {
            continue;
        }
        stages.push(Stage {
            name,
            field: column,
            selection,
            blocker: None,
        });
    }
    if policy.cue_generation == CueGeneration::Generate
        && super::super::projection::field_selected(policy, "cues.hot")
    {
        let occupied = super::super::performance::hotcue_occupied(fields);
        let unknown = fields.get("_hotcue_occupancy").and_then(Value::as_str) == Some("unknown");
        stages.push(Stage {
            name: "generation.cues",
            field: "cues.hot",
            selection: AnalysisSelection {
                cues: true,
                ..Default::default()
            },
            blocker: if unknown && !policy.replace_hotcues {
                Some(
                    "Unknown native hotcue occupancy requires separate replacement permission"
                        .into(),
                )
            } else if occupied && !policy.replace_hotcues {
                Some("Occupied hotcues require separate replacement permission".into())
            } else {
                None
            },
        });
    }
    let mut optional = Vec::new();
    for (enabled, name) in [
        (policy.metadata_enrichment.enabled, "metadata.enrichment"),
        (policy.cleanup.enabled, "metadata.cleanup"),
    ] {
        if enabled {
            optional.push(Stage {
                name,
                field: "metadata",
                selection: AnalysisSelection::default(),
                blocker: None,
            });
        }
    }
    optional.extend(stages);
    for (enabled, name) in [
        (!policy.write_audio_tags.is_empty(), "file.tags"),
        (
            policy.relocation != RelocationPolicy::Off,
            "file.relocation",
        ),
    ] {
        if enabled {
            optional.push(Stage {
                name,
                field: "metadata",
                selection: AnalysisSelection::default(),
                blocker: None,
            });
        }
    }
    optional
}
pub(super) fn analyze(
    stage: &Stage,
    frozen: &FrozenProcessing,
    audio: &cuerithm_media::PreparedAudio,
    fields: &BTreeMap<String, Value>,
    cancel: &CancellationToken,
) -> Result<(Value, Value)> {
    cancel.check()?;
    frozen.validate_models(stage.selection, cancel)?;
    let mut settings = settings_for_audio(stage, frozen, fields, audio)?;
    let mut engine = Engine::new_selected(&settings, stage.selection)
        .map_err(|e| super::service::BlockedStage(e.to_string()))?;
    let loaded = engine.model_identity();
    for (name, model) in &frozen.models {
        if relevant_model(name, stage.selection) {
            ensure!(
                model
                    .sha256
                    .as_ref()
                    .is_some_and(|sha| loaded.values().any(|value| value == sha)),
                "Loaded model identity differs from the frozen configuration"
            );
        }
    }
    let mut result = engine.analyze_prepared_selected(audio, stage.selection)?;
    cancel.check()?;
    let value = match stage.field {
        "key" => {
            let key = result.analysis.key.as_ref().ok_or_else(|| {
                super::service::BlockedStage("Key analyser returned no qualified value".into())
            })?;
            serde_json::to_value(MusicalKey {
                presence: FieldPresence::Value,
                raw: Some(key.key.name()),
                camelot: Some(key.camelot.clone()),
                pitch_class: Some(key.key.tonic()),
                mode: Some(
                    if key.key.mode() == camelot_core::Mode::Minor {
                        "minor"
                    } else {
                        "major"
                    }
                    .into(),
                ),
                provenance: Some("measured".into()),
                ..Default::default()
            })?
        }
        "bpm" => {
            let tempo = result.analysis.tempo.as_ref().ok_or_else(|| {
                super::service::BlockedStage("BPM analyser returned no qualified value".into())
            })?;
            serde_json::to_value(ScalarBpm {
                presence: FieldPresence::Value,
                value: Some(tempo.bpm),
                provenance: Some("measured".into()),
                ..Default::default()
            })?
        }
        "beatgrid" => {
            let grid = result.selected_beatgrid.as_ref().ok_or_else(|| {
                super::service::BlockedStage("Grid analyser returned no qualified value".into())
            })?;
            json!({"measured_timing":grid,"clock":audio.clock,"pcm_sha256":audio.pcm_sha256()?})
        }
        "energy" => {
            let energy = result.analysis.energy.as_ref().ok_or_else(|| {
                super::service::BlockedStage("Energy analyser returned no qualified value".into())
            })?;
            json!({"presence":"value","value":energy.level,"method":energy.method,"provenance":"measured"})
        }
        "cues.hot" => {
            let timing = engine.take_generation_timing();
            let mut publication = result.clone();
            publication.analysis.tempo = timing;
            // The existing publisher uses detector configuration and measured timing,
            // never imported keys/BPM/cues. Its grid stays a private prerequisite.
            crate::publication::apply(
                &mut publication,
                &crate::PreparationSettings {
                    engine: settings.clone(),
                    ..Default::default()
                },
            )?;
            json!({"structural":result.analysis.lexicon_cues,"published":publication.publication.as_ref().map(|p|&p.cues),"publication_identity":publication.publication.as_ref().map(|p|&p.implementation_sha256),"clock":audio.clock})
        }
        _ => anyhow::bail!("Unknown selected analysis stage"),
    };
    // Record effective cue genre rather than the unselected native musical fields.
    if stage.selection.cues
        && let Some(config) = &mut settings.lexicon_config
    {
        config.genre = result.media.as_ref().and_then(|m| m.cue_genre.clone());
    }
    let identity = json!({"engine_settings":settings,"selection":stage.selection,"engine_source_sha256":result.analysis.engine_source_sha256,"processing_sha256":implementation_sha256(),"models":result.analysis.models,"config_sha256":result.analysis.config_sha256,"pcm_sha256":audio.pcm_sha256()?,"decoder":audio.decoder_identity,"source_sha256":audio.source_sha256,"clock":audio.clock,"media":result.media});
    result.publication = None;
    Ok((
        json!({"field":stage.field,"value":value,"raw_analysis":result}),
        identity,
    ))
}

pub(super) fn relevant_model(name: &str, selection: AnalysisSelection) -> bool {
    match name {
        "key-profile" | "skey-key" | "openkeyscan-key" => selection.key,
        "track-energy-svc" => selection.energy,
        _ => selection.bpm || selection.beatgrid || selection.cues,
    }
}
fn effective_settings(
    stage: &Stage,
    frozen: &FrozenProcessing,
    fields: &BTreeMap<String, Value>,
) -> Result<camelot_core::EngineSettings> {
    let original = frozen
        .engine
        .as_ref()
        .context("No frozen analysis configuration")?;
    let mut settings = stage.selection.effective_settings(original);
    if stage.selection.cues
        && let Some(config) = &mut settings.lexicon_config
        && config.genre.is_none()
    {
        let genre = fields
            .get("metadata.genre")
            .or_else(|| fields.get("metadata.genres"));
        if genre.is_some() {
            config.genre = Some(super::super::native_shadow::text(genre));
        }
    }
    Ok(settings)
}
pub(super) fn signature(
    stage: &Stage,
    frozen: &FrozenProcessing,
    audio: &cuerithm_media::PreparedAudio,
    fields: &BTreeMap<String, Value>,
) -> Result<String> {
    use sha2::{Digest, Sha256};
    let settings = settings_for_audio(stage, frozen, fields, audio)?;
    let settings = settings_identity(&settings)?;
    let models: BTreeMap<_, _> = frozen
        .models
        .iter()
        .filter(|(name, _)| relevant_model(name, stage.selection))
        .map(|(name, model)| (name, json!({"sha256":model.sha256,"error":model.error})))
        .collect();
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(
            &json!({"settings":settings,"models":models,"selection":stage.selection,"source_sha256":audio.source_sha256,"pcm_sha256":audio.pcm_sha256()?,"decoder":audio.decoder_identity,"clock":audio.clock,"export_timing":audio.export_timing,"engine_source_sha256":camelot_core::analysis::engine_source_sha256(),"processing_sha256":implementation_sha256(),"publication":if stage.selection.cues{Some(camelot_core::lexicon_publication::implementation_sha256())}else{None}})
        )?)
    ))
}

pub(super) fn settings_for_audio(
    stage: &Stage,
    frozen: &FrozenProcessing,
    fields: &BTreeMap<String, Value>,
    audio: &cuerithm_media::PreparedAudio,
) -> Result<camelot_core::EngineSettings> {
    let mut settings = effective_settings(stage, frozen, fields)?;
    if stage.selection.cues
        && let Some(config) = &mut settings.lexicon_config
        && config.genre.is_none()
    {
        config.genre = audio
            .info
            .tags
            .get("genre")
            .filter(|v| !v.trim().is_empty())
            .cloned();
    }
    Ok(settings)
}
pub(super) fn settings_identity(settings: &camelot_core::EngineSettings) -> Result<Value> {
    let mut settings = serde_json::to_value(settings)?;
    if let Some(settings) = settings.as_object_mut() {
        for name in [
            "key_model",
            "key_neural_model",
            "key_openkeyscan_model",
            "energy_model",
            "beat_model",
            "mel_model",
        ] {
            settings.remove(name);
        }
    }
    Ok(settings)
}
