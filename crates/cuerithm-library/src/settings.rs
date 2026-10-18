#[cfg(feature = "autotag")]
use anyhow::Context;
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
    #[cfg(feature = "autotag")]
    pub tagging: Option<cuerithm_autotag::AutotagConfig>,
    #[cfg(not(feature = "autotag"))]
    pub tagging: Option<serde_json::Value>,
    pub cleanup_enabled: bool,
    pub cleanup: Option<cuerithm_cleanup::CleanupConfig>,
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
            tagging: None,
            cleanup_enabled: false,
            cleanup: None,
        }
    }
}
impl PreparationSettings {
    pub fn include_subfolders(&self) -> bool {
        #[cfg(feature = "autotag")]
        {
            self.tagging
                .as_ref()
                .is_none_or(|config| config.options.include_subfolders)
        }
        #[cfg(not(feature = "autotag"))]
        {
            self.tagging
                .as_ref()
                .and_then(|config| config.get("options")?.get("includeSubfolders")?.as_bool())
                .unwrap_or(true)
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.engine.validate()?;
        if let Some(config) = &self.cleanup {
            config.validate()?;
        }
        #[cfg(feature = "autotag")]
        if let Some(config) = &self.tagging {
            config.validate()?;
        }
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
        if self.autotag_enabled {
            #[cfg(not(feature = "autotag"))]
            anyhow::bail!("Auto tagging is not available in this build");
            #[cfg(feature = "autotag")]
            self.tagging
                .as_ref()
                .context("Configure automatic tagging in Settings")?
                .validate()?;
        }
        if self.cleanup_enabled {
            #[cfg(not(feature = "autotag"))]
            anyhow::bail!("Automatic cleanup is not available in this build");
            #[cfg(feature = "autotag")]
            self.cleanup
                .as_ref()
                .context("Configure automatic cleanup in Settings")?
                .validate()?;
        }
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
