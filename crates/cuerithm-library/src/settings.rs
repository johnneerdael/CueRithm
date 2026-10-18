use anyhow::{Result, ensure};
use camelot_core::{
    EngineSettings,
    lexicon_cues::{DropAtStart, LexiconConfig},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PreparationSettings {
    pub watch_folders: Vec<PathBuf>,
    pub stability_millis: u64,
    pub engine: EngineSettings,
    pub autotag_enabled: bool,
    pub cleanup_enabled: bool,
}
impl Default for PreparationSettings {
    fn default() -> Self {
        Self {
            watch_folders: Vec::new(),
            stability_millis: 2000,
            engine: EngineSettings {
                source_profile_timing: true,
                lexicon_config: Some(LexiconConfig {
                    drop_at_start: DropAtStart::Never,
                    add_emergency_loop: false,
                    ..Default::default()
                }),
                ..Default::default()
            },
            autotag_enabled: false,
            cleanup_enabled: false,
        }
    }
}
impl PreparationSettings {
    pub fn validate(&self) -> Result<()> {
        self.engine.validate()?;
        ensure!(
            self.engine.max_seconds.is_none(),
            "Preparation requires full-track analysis"
        );
        ensure!(
            self.engine.lexicon_config.is_some(),
            "Preparation requires replacement cues"
        );
        ensure!(
            (10..=60000).contains(&self.stability_millis),
            "Invalid stability interval"
        );
        Ok(())
    }
    pub fn ready(&self) -> Result<()> {
        ensure!(
            !self.autotag_enabled,
            "Auto tagging is not available in this build"
        );
        ensure!(
            !self.cleanup_enabled,
            "Automatic cleanup is not available in this build"
        );
        ensure!(
            self.engine.energy_model.is_some(),
            "Choose an energy model in Settings"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsSnapshot {
    pub revision: crate::SettingsRevision,
    pub settings: PreparationSettings,
    pub models: BTreeMap<String, String>,
}
