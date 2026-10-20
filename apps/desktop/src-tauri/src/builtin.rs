use anyhow::{Context, Result, ensure};
use cuerithm_library::{Library, PreparationSettings};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

const KEY_SHA: &str = "262ac22bce23d371fe045d478e90d200f71b5be16046792a8b55432ae5c9f952";
const KEY: &[u8] = include_bytes!("../../../../models/builtin/openkeyscan3-logits.onnx");
const ENERGY_SHA: &str = "bf7079a52240d04d8d3d5c6a7db537a47673c19655dcde45bcc10cb210272d96";
/// Window classifier for the opt-in "MIK-style" energy rating, fitted to captured Mixed In Key window levels.
const ENERGY: &[u8] = include_bytes!("../../../../models/builtin/energy-svc-v1.json");

pub struct BuiltinModels {
    pub key: PathBuf,
    pub energy: PathBuf,
}
impl BuiltinModels {
    pub fn verify() -> Result<()> {
        ensure!(
            format!("{:x}", Sha256::digest(KEY)) == KEY_SHA
                && format!("{:x}", Sha256::digest(ENERGY)) == ENERGY_SHA,
            "Built-in analysis component failed verification; reinstall CueRithm"
        );
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
        // The built-in formula rates energy unless the user opted in to the bundled window classifier; either
        // way the engine only ever sees paths and flags that this build installed itself.
        let mik_style = settings.mik_style_energy;
        let e = &mut settings.engine;
        let energy_model = mik_style.then(|| self.energy.clone());
        let changed = e.key_openkeyscan_model.as_ref() != Some(&self.key)
            || e.energy_model != energy_model
            || e.lexicon_energy == mik_style
            || e.key_model.is_some()
            || e.key_neural_model.is_some()
            || e.source_profile_timing
            || !e.lexicon_timing
            || e.beat_model.is_some()
            || e.mel_model.is_some()
            || e.max_seconds.is_some()
            || e.cue_config.is_some();
        e.key_openkeyscan_model = Some(self.key.clone());
        e.energy_model = energy_model;
        e.lexicon_energy = !mik_style;
        e.key_model = None;
        e.key_neural_model = None;
        e.source_profile_timing = false;
        e.lexicon_timing = true;
        e.beat_model = None;
        e.mel_model = None;
        e.max_seconds = None;
        e.cue_config = None;
        changed
    }
    pub fn configure_library(&self, library: &Library) -> Result<bool> {
        let selected = library.settings()?;
        let mut settings = selected.settings;
        let changed = self.apply(&mut settings);
        let expected = camelot_core::Engine::new(&settings.engine)?.model_identity();
        let obsolete: Vec<_> = library
            .unfinished_settings()?
            .into_iter()
            .filter(|snapshot| self.needs_migration(snapshot, &expected))
            .map(|snapshot| snapshot.revision)
            .collect();
        if !changed && selected.models == expected && obsolete.is_empty() {
            return Ok(false);
        }
        library.save_settings_migrating(settings, &obsolete)?;
        Ok(true)
    }
    fn needs_migration(
        &self,
        snapshot: &cuerithm_library::SettingsSnapshot,
        expected: &std::collections::BTreeMap<String, String>,
    ) -> bool {
        let mut settings = snapshot.settings.clone();
        self.apply(&mut settings) || &snapshot.models != expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuerithm_library::{Library, PreparationSettings};
    #[test]
    fn desktop_selects_complete_local_timing_and_clears_obsolete_backends() {
        let d = tempfile::tempdir().unwrap();
        let models = BuiltinModels::install(d.path()).unwrap();
        let mut settings = PreparationSettings::default();
        settings.engine.source_profile_timing = true;
        settings.engine.beat_model = Some(d.path().join("missing-beat.onnx"));
        settings.engine.mel_model = Some(d.path().join("missing-mel.onnx"));
        settings.engine.max_seconds = Some(30.0);
        settings.engine.cue_config = Some(serde_json::json!({"enabled": true}));
        assert!(models.apply(&mut settings));
        assert!(settings.engine.lexicon_timing);
        assert!(!settings.engine.source_profile_timing);
        assert!(settings.engine.beat_model.is_none());
        assert!(settings.engine.mel_model.is_none());
        assert!(settings.engine.max_seconds.is_none());
        assert!(settings.engine.cue_config.is_none());
        settings.ready().unwrap();
        let engine = camelot_core::Engine::new(&settings.engine).unwrap();
        assert!(engine.model_identity().contains_key("lexicon-timing-dsp"));
        assert!(!models.apply(&mut settings));
    }
    #[test]
    fn energy_defaults_to_the_compiled_formula_and_migrates_stray_model_paths() {
        let dir = tempfile::tempdir().unwrap();
        let builtin = BuiltinModels::install(dir.path()).unwrap();
        let files: Vec<_> = std::fs::read_dir(dir.path().join("analysis-components"))
            .unwrap()
            .collect();
        assert_eq!(
            files.len(),
            2,
            "The key graph and the opt-in energy classifier are installed"
        );
        let mut settings = PreparationSettings::default();
        settings.engine.energy_model = Some(dir.path().join("missing-svc.json"));
        assert!(builtin.apply(&mut settings));
        assert!(settings.engine.energy_model.is_none());
        assert!(settings.engine.lexicon_energy);
        settings.ready().unwrap();
        assert!(camelot_core::Engine::new(&settings.engine).is_ok());
    }
    #[test]
    fn mik_style_energy_is_opt_in_and_uses_only_the_installed_classifier() {
        let dir = tempfile::tempdir().unwrap();
        let builtin = BuiltinModels::install(dir.path()).unwrap();
        assert_eq!(
            cuerithm_media::file_sha256(&builtin.energy).unwrap(),
            ENERGY_SHA
        );
        let mut settings = PreparationSettings::default();
        assert!(!settings.mik_style_energy);
        builtin.apply(&mut settings);
        assert!(settings.engine.lexicon_energy && settings.engine.energy_model.is_none());
        // Opting in swaps the formula for the installed classifier and nothing else about the engine.
        settings.mik_style_energy = true;
        assert!(builtin.apply(&mut settings));
        assert!(!settings.engine.lexicon_energy);
        assert_eq!(settings.engine.energy_model.as_ref(), Some(&builtin.energy));
        assert!(settings.engine.lexicon_timing && !settings.engine.source_profile_timing);
        settings.ready().unwrap();
        let identity = camelot_core::Engine::new(&settings.engine)
            .unwrap()
            .model_identity();
        assert!(identity.keys().any(|k| k.contains("energy")));
        assert!(!builtin.apply(&mut settings));
        // A path the user typed into stored settings never survives, opted in or not.
        settings.engine.energy_model = Some(dir.path().join("elsewhere.json"));
        assert!(builtin.apply(&mut settings));
        assert_eq!(settings.engine.energy_model.as_ref(), Some(&builtin.energy));
        // Opting out returns to the formula.
        settings.mik_style_energy = false;
        assert!(builtin.apply(&mut settings));
        assert!(settings.engine.lexicon_energy && settings.engine.energy_model.is_none());
    }
    #[test]
    fn the_opt_in_rates_a_real_file_with_the_window_classifier_and_the_default_uses_the_formula() {
        let dir = tempfile::tempdir().unwrap();
        let builtin = BuiltinModels::install(dir.path()).unwrap();
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
        let rate = |mik_style: bool| {
            let mut settings = PreparationSettings {
                mik_style_energy: mik_style,
                ..Default::default()
            };
            builtin.apply(&mut settings);
            let mut engine = camelot_core::Engine::new(&settings.engine).unwrap();
            engine
                .analyze_file(&fixture)
                .unwrap()
                .analysis
                .energy
                .map(|e| (e.method, e.level))
        };
        let (formula_method, formula_level) = rate(false).unwrap();
        assert_eq!(formula_method, camelot_core::lexicon_energy::METHOD);
        let (classifier_method, classifier_level) = rate(true).unwrap();
        assert!(classifier_method.contains("svc"), "{classifier_method}");
        assert!((1..=10).contains(&formula_level) && (1..=10).contains(&classifier_level));
    }
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
        std::fs::write(&models.key, b"broken").unwrap();
        let repaired = BuiltinModels::install(d.path()).unwrap();
        assert_eq!(models.key, repaired.key);
        assert_eq!(cuerithm_media::file_sha256(&repaired.key).unwrap(), KEY_SHA);
    }

    #[test]
    fn interrupted_old_migration_recovers_after_reopen() {
        let d = tempfile::tempdir().unwrap();
        let db = d.path().join("library.sqlite");
        let models = BuiltinModels::install(d.path()).unwrap();
        {
            let lib = Library::open(&db).unwrap();
            let source = d.path().join("track.flac");
            std::fs::write(&source, b"source").unwrap();
            let id = lib
                .enqueue(&[source], lib.settings().unwrap().revision)
                .unwrap()[0]
                .clone();
            lib.claim_next().unwrap().unwrap();
            lib.fail(&id, "setup", "old missing model").unwrap();
            let mut settings = lib.settings().unwrap().settings;
            models.apply(&mut settings);
            // Simulate the previous implementation dying between transactions.
            lib.save_settings(settings).unwrap();
        }
        let lib = Library::open(&db).unwrap();
        assert!(models.configure_library(&lib).unwrap());
        let job = lib.claim_next().unwrap().unwrap();
        assert!(job.snapshot.settings.engine.lexicon_timing);
        assert!(!job.snapshot.settings.engine.source_profile_timing);
    }
    #[test]
    fn changed_implementation_identity_requires_migration_with_same_paths() {
        let d = tempfile::tempdir().unwrap();
        let models = BuiltinModels::install(d.path()).unwrap();
        let mut settings = PreparationSettings::default();
        models.apply(&mut settings);
        let expected = std::collections::BTreeMap::from([(
            "openkeyscan-frontend".to_string(),
            "current".to_string(),
        )]);
        let snapshot = cuerithm_library::SettingsSnapshot {
            revision: "old-build".into(),
            settings,
            models: Default::default(),
        };
        assert!(models.needs_migration(&snapshot, &expected));
    }
}
