use super::*;
use std::path::PathBuf;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct FrozenProcessing {
    pub engine: Option<camelot_core::EngineSettings>,
    #[serde(default)]
    pub cleanup: Option<cuerithm_cleanup::CleanupConfig>,
    #[serde(default)]
    pub optional_error: Option<String>,
    #[serde(default)]
    pub tagging: Option<Value>,
    pub settings_revision: Option<String>,
    pub models: BTreeMap<String, FrozenModel>,
    #[serde(default)]
    pub engine_source_sha256: Option<String>,
    #[serde(default)]
    pub processing_sha256: Option<String>,
    #[serde(default)]
    pub publication_sha256: Option<String>,
    #[serde(default)]
    pub decoder_identity: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FrozenModel {
    #[serde(with = "cuerithm_dj_sync::path_serde")]
    pub path: PathBuf,
    pub sha256: Option<String>,
    pub error: Option<String>,
}
pub(crate) fn selection(policy: &ImportPolicy) -> camelot_core::AnalysisSelection {
    camelot_core::AnalysisSelection {
        key: policy.analysis_fields.contains_key(&AnalysisField::Key)
            && super::super::projection::field_selected(policy, "key"),
        bpm: policy.analysis_fields.contains_key(&AnalysisField::Bpm)
            && super::super::projection::field_selected(policy, "bpm"),
        beatgrid: policy
            .analysis_fields
            .contains_key(&AnalysisField::Beatgrid)
            && super::super::projection::field_selected(policy, "beatgrid"),
        energy: policy.analysis_fields.contains_key(&AnalysisField::Energy)
            && super::super::projection::field_selected(policy, "energy"),
        cues: policy.cue_generation == CueGeneration::Generate
            && super::super::projection::field_selected(policy, "cues.hot"),
    }
}
pub(crate) fn freeze(
    lib: &Library,
    policy: &ImportPolicy,
    cancel: &CancellationToken,
) -> Result<FrozenProcessing> {
    let selected = selection(policy);
    let optional = if policy.cleanup.enabled || policy.metadata_enrichment.enabled {
        Some(lib.settings()?.settings)
    } else {
        None
    };
    let mut cleanup = optional.as_ref().and_then(|s| s.cleanup.clone());
    let mut optional_error = None;
    if policy.cleanup.enabled {
        if let Some(config) = &mut cleanup {
            if !policy.cleanup.rules.is_empty() {
                for id in &policy.cleanup.rules {
                    if id != "genres"
                        && id != "artists"
                        && !config.fixes.iter().any(|f| &f.id == id)
                    {
                        optional_error = Some(format!("Cleanup rule {id} unavailable at preview"));
                    }
                }
                config
                    .fixes
                    .retain(|f| policy.cleanup.rules.contains(&f.id));
                config.genres.enabled &= policy.cleanup.rules.iter().any(|id| id == "genres");
                config.artists.enabled &= policy.cleanup.rules.iter().any(|id| id == "artists");
            }
        } else {
            optional_error = Some("Cleanup configuration unavailable at preview".into());
        }
    }
    let tagging = optional
        .as_ref()
        .and_then(|s| s.tagging.as_ref())
        .map(serde_json::to_value)
        .transpose()?;
    let decoder_identity = if selected.any() || policy.metadata_enrichment.enabled {
        lib.inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
            .decoder_identity
            .clone()
    } else {
        None
    };
    if !selected.any() {
        return Ok(FrozenProcessing {
            cleanup,
            optional_error,
            tagging,
            decoder_identity,
            processing_sha256: policy.requires_processing().then(implementation_sha256),
            ..Default::default()
        });
    }
    let snapshot = lib.settings()?;
    let original = snapshot.settings.engine;
    let mut models = BTreeMap::new();
    for (name, path, needed) in [
        ("key-profile", &original.key_model, selected.key),
        ("skey-key", &original.key_neural_model, selected.key),
        (
            "openkeyscan-key",
            &original.key_openkeyscan_model,
            selected.key,
        ),
        ("track-energy-svc", &original.energy_model, selected.energy),
        (
            "beat",
            &original.beat_model,
            selected.bpm || selected.beatgrid || selected.cues,
        ),
        (
            "mel",
            &original.mel_model,
            selected.bpm || selected.beatgrid || selected.cues,
        ),
    ] {
        if needed && let Some(path) = path {
            cancel.check()?;
            let hash = (|| -> Result<String> {
                ensure!(
                    std::fs::metadata(path)?.len() <= 2 * 1024 * 1024 * 1024,
                    "Selected model exceeds byte limit"
                );
                cuerithm_media::file_sha256_cancellable(path, cancel)
            })();
            let (sha256, error) = match hash {
                Ok(hash) => (Some(hash), None),
                Err(e) => {
                    cancel.check()?;
                    (None, Some(e.to_string()))
                }
            };
            models.insert(
                name.into(),
                FrozenModel {
                    path: path.clone(),
                    sha256,
                    error,
                },
            );
        }
    }
    Ok(FrozenProcessing {
        engine: Some(original),
        cleanup,
        optional_error,
        tagging,
        settings_revision: Some(snapshot.revision),
        models,
        decoder_identity,
        engine_source_sha256: Some(camelot_core::analysis::engine_source_sha256()),
        processing_sha256: Some(implementation_sha256()),
        publication_sha256: selected
            .cues
            .then(camelot_core::lexicon_publication::implementation_sha256),
    })
}
impl FrozenProcessing {
    pub fn effective_identity(&self, policy: &ImportPolicy) -> Result<Value> {
        let selected = selection(policy);
        Ok(
            json!({"engine":self.engine.as_ref().map(|engine|selected.effective_settings(engine)),"cleanup":self.cleanup,"optional_error":self.optional_error,"tagging":self.tagging,"models":self.models,"engine_source_sha256":self.engine_source_sha256,"processing_sha256":self.processing_sha256,"publication_sha256":self.publication_sha256}),
        )
    }
    pub fn validate_models(
        &self,
        selected: camelot_core::AnalysisSelection,
        cancel: &CancellationToken,
    ) -> Result<()> {
        ensure!(
            self.processing_sha256.as_deref() == Some(implementation_sha256().as_str()),
            "Import processing implementation changed; create a new preview"
        );
        if selected.any() {
            ensure!(
                self.engine_source_sha256.as_deref()
                    == Some(camelot_core::analysis::engine_source_sha256().as_str()),
                "Analysis implementation changed; create a new preview"
            );
        }
        if selected.cues {
            ensure!(
                self.publication_sha256.as_deref()
                    == Some(camelot_core::lexicon_publication::implementation_sha256().as_str()),
                "Cue publication implementation changed; create a new preview"
            );
        }
        for (name, model) in &self.models {
            let needed = match name.as_str() {
                "key-profile" | "skey-key" | "openkeyscan-key" => selected.key,
                "track-energy-svc" => selected.energy,
                _ => selected.bpm || selected.beatgrid || selected.cues,
            };
            if needed {
                cancel.check()?;
                ensure!(
                    model.error.is_none(),
                    "Selected model was unavailable at preview: {}",
                    model.error.as_deref().unwrap_or("")
                );
                ensure!(
                    Some(cuerithm_media::file_sha256_cancellable(
                        &model.path,
                        cancel
                    )?) == model.sha256,
                    "Selected model changed after preview"
                );
            }
        }
        Ok(())
    }
}
