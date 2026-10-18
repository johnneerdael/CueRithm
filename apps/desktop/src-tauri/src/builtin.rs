use anyhow::{Context, Result, ensure};
use cuerithm_library::{Library, PreparationSettings};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

const KEY_SHA: &str = "262ac22bce23d371fe045d478e90d200f71b5be16046792a8b55432ae5c9f952";
const ENERGY_SHA: &str = "bf7079a52240d04d8d3d5c6a7db537a47673c19655dcde45bcc10cb210272d96";
const KEY: &[u8] = include_bytes!("../../../../models/builtin/openkeyscan3-logits.onnx");
const ENERGY: &[u8] = include_bytes!("../../../../models/builtin/energy-svc-v1.json");

pub struct BuiltinModels {
    pub key: PathBuf,
    pub energy: PathBuf,
}
impl BuiltinModels {
    pub fn verify() -> Result<()> {
        for (bytes, expected) in [(KEY, KEY_SHA), (ENERGY, ENERGY_SHA)] {
            ensure!(
                format!("{:x}", Sha256::digest(bytes)) == expected,
                "Built-in analysis component failed verification; reinstall Cuerithm"
            );
        }
        Ok(())
    }
    pub fn install(root: &Path) -> Result<Self> {
        Self::verify()?;
        let folder = root.join("analysis-components");
        std::fs::create_dir_all(&folder)?;
        fn install(folder: &Path, bytes: &[u8], hash: &str, suffix: &str) -> Result<PathBuf> {
            let path = folder.join(format!("{hash}.{suffix}"));
            if cuerithm_media::file_sha256(&path).ok().as_deref() != Some(hash) {
                let mut temp = tempfile::NamedTempFile::new_in(folder)?;
                temp.write_all(bytes)?;
                temp.as_file().sync_all()?;
                temp.persist(&path)
                    .map_err(|e| e.error)
                    .context("Install built-in analysis component")?;
            }
            Ok(path)
        }
        Ok(Self {
            key: install(&folder, KEY, KEY_SHA, "onnx")?,
            energy: install(&folder, ENERGY, ENERGY_SHA, "json")?,
        })
    }
    pub fn apply(&self, settings: &mut PreparationSettings) -> bool {
        let e = &mut settings.engine;
        let changed = e.key_openkeyscan_model.as_ref() != Some(&self.key)
            || e.energy_model.as_ref() != Some(&self.energy)
            || e.key_model.is_some()
            || e.key_neural_model.is_some()
            || !e.source_profile_timing;
        e.key_openkeyscan_model = Some(self.key.clone());
        e.energy_model = Some(self.energy.clone());
        e.key_model = None;
        e.key_neural_model = None;
        e.source_profile_timing = true;
        changed
    }
    pub fn configure_library(&self, library: &Library) -> Result<bool> {
        let mut settings = library.settings()?.settings;
        if !self.apply(&mut settings) {
            return Ok(false);
        }
        let revision = library.save_settings(settings)?;
        library.requeue_unfinished(&revision)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuerithm_library::{Library, PreparationSettings};
    #[test]
    fn first_run_is_ready_and_migration_preserves_user_preferences() {
        let d = tempfile::tempdir().unwrap();
        let lib = Library::open(&d.path().join("library.sqlite")).unwrap();
        let mut settings = PreparationSettings {
            watch_folders: vec![d.path().join("Music")],
            ..Default::default()
        };
        settings
            .engine
            .lexicon_config
            .as_mut()
            .unwrap()
            .min_cue_length_beats = 128;
        lib.save_settings(settings).unwrap();
        let models = BuiltinModels::install(d.path()).unwrap();
        assert!(models.configure_library(&lib).unwrap());
        let snapshot = lib.settings().unwrap();
        snapshot.settings.ready().unwrap();
        assert_eq!(
            snapshot.settings.watch_folders,
            vec![d.path().join("Music")]
        );
        assert_eq!(
            snapshot
                .settings
                .engine
                .lexicon_config
                .unwrap()
                .min_cue_length_beats,
            128
        );
        assert!(!models.configure_library(&lib).unwrap());
    }
    #[test]
    fn cached_parameters_are_repaired_from_embedded_bytes() {
        let d = tempfile::tempdir().unwrap();
        let models = BuiltinModels::install(d.path()).unwrap();
        std::fs::write(&models.energy, b"broken").unwrap();
        let repaired = BuiltinModels::install(d.path()).unwrap();
        assert_eq!(models.energy, repaired.energy);
        assert_eq!(
            cuerithm_media::file_sha256(&repaired.energy).unwrap(),
            ENERGY_SHA
        );
    }
}
