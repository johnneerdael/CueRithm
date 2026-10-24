//! Selection controls orchestration and adoption, never the DSP algorithms.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AnalysisSelection {
    pub key: bool,
    pub bpm: bool,
    pub beatgrid: bool,
    pub energy: bool,
    pub cues: bool,
}
impl AnalysisSelection {
    pub const ALL: Self = Self {
        key: true,
        bpm: true,
        beatgrid: true,
        energy: true,
        cues: true,
    };
    pub fn any(self) -> bool {
        self.key || self.bpm || self.beatgrid || self.energy || self.cues
    }
    pub(crate) fn timing(self) -> bool {
        self.bpm || self.beatgrid || self.cues
    }
    pub fn effective_settings(self, original: &crate::EngineSettings) -> crate::EngineSettings {
        let mut settings = original.clone();
        if !self.key {
            settings.key_model = None;
            settings.key_neural_model = None;
            settings.key_openkeyscan_model = None;
        }
        if !self.timing() {
            settings.beat_model = None;
            settings.mel_model = None;
            settings.source_profile_timing = false;
            settings.lexicon_timing = false;
        }
        if !self.energy {
            settings.energy_model = None;
            settings.lexicon_energy = false;
        }
        if !self.cues {
            settings.lexicon_config = None;
            settings.cue_config = None;
        }
        if self != Self::ALL {
            settings.structure_features = false;
        }
        settings
    }
}

#[cfg(test)]
mod tests {
    use crate::{AnalysisSelection, Engine, EngineSettings};
    use cuerithm_media::{MediaRuntime, PreparedAudio};
    fn fixture() -> (tempfile::TempDir, PreparedAudio) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("generated.wav");
        let mut wav = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 44100,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for frame in 0..44100 * 16 {
            let t = frame as f64 / 44100.0;
            let phase = t % 0.5;
            let click = if phase < 0.025 {
                (1.0 - phase / 0.025) * 0.45
            } else {
                0.0
            };
            let chord = [220.0, 261.625565, 329.627557]
                .iter()
                .map(|f| (t * f * std::f64::consts::TAU).sin() * 0.12)
                .sum::<f64>();
            let sample = ((click + chord) * 20000.0) as i16;
            wav.write_sample(sample).unwrap();
            wav.write_sample(sample).unwrap();
        }
        wav.finalize().unwrap();
        let bundle = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tools/media-runtime/bin")
            });
        let runtime = MediaRuntime::from_bundle(&bundle).unwrap();
        let audio = runtime.decode(&path, &dir.path().join("cache")).unwrap();
        (dir, audio)
    }
    fn settings() -> EngineSettings {
        EngineSettings {
            lexicon_energy: true,
            lexicon_config: Some(Default::default()),
            ..Default::default()
        }
    }
    fn value<T: serde::Serialize>(value: T) -> serde_json::Value {
        serde_json::to_value(value).unwrap()
    }
    #[test]
    fn key_only_skips_timing_energy_and_cues() {
        let (_dir, audio) = fixture();
        let reference = Engine::new(&settings())
            .unwrap()
            .analyze_prepared(&audio)
            .unwrap();
        let selection = AnalysisSelection {
            key: true,
            ..Default::default()
        };
        let mut disabled = settings();
        disabled.energy_model = Some("missing-energy-model".into());
        disabled.beat_model = Some("missing-beat-model".into());
        disabled.mel_model = Some("missing-mel-model".into());
        let mut engine = Engine::new_selected(&disabled, selection).unwrap();
        let result = engine.analyze_prepared_selected(&audio, selection).unwrap();
        assert_eq!(value(&result.analysis.key), value(&reference.analysis.key));
        assert!(result.analysis.tempo.is_none());
        assert!(result.analysis.energy.is_none());
        assert!(result.analysis.cues.is_none());
        assert!(result.selected_beatgrid.is_none());
        assert_eq!(engine.selected_stage_counts(), [1, 0, 0, 0]);
        assert_eq!(engine.selected_input_rates(), vec![11025]);
        assert!(engine.analyze_prepared(&audio).is_err());
    }
    #[test]
    fn energy_only_skips_key_and_cues() {
        let (_dir, audio) = fixture();
        let reference = Engine::new(&settings())
            .unwrap()
            .analyze_prepared(&audio)
            .unwrap();
        assert!(reference.analysis.energy.is_some());
        let selection = AnalysisSelection {
            energy: true,
            ..Default::default()
        };
        let mut disabled = settings();
        disabled.key_model = Some("missing-key-model".into());
        let mut engine = Engine::new_selected(&disabled, selection).unwrap();
        let result = engine.analyze_prepared_selected(&audio, selection).unwrap();
        assert_eq!(
            value(&result.analysis.energy),
            value(&reference.analysis.energy)
        );
        assert!(result.analysis.key.is_none());
        assert!(result.analysis.tempo.is_none());
        assert!(result.analysis.cues.is_none());
        assert_eq!(engine.selected_stage_counts(), [0, 0, 1, 0]);
        assert_eq!(engine.selected_input_rates(), vec![22050]);
    }
    #[test]
    fn cue_prerequisites_do_not_publish_unselected_fields() {
        let (_dir, audio) = fixture();
        let reference = Engine::new(&settings())
            .unwrap()
            .analyze_prepared(&audio)
            .unwrap();
        let selection = AnalysisSelection {
            cues: true,
            ..Default::default()
        };
        let mut engine = Engine::new_selected(&settings(), selection).unwrap();
        let result = engine.analyze_prepared_selected(&audio, selection).unwrap();
        assert_eq!(
            value(&result.analysis.lexicon_cues),
            value(&reference.analysis.lexicon_cues)
        );
        assert!(result.analysis.key.is_none());
        assert!(result.analysis.tempo.is_none());
        assert!(result.selected_beatgrid.is_none());
        assert!(result.analysis.energy.is_none());
        assert_eq!(engine.selected_stage_counts(), [0, 1, 0, 1]);
    }
    #[test]
    fn full_selection_matches_existing_engine() {
        let (_dir, audio) = fixture();
        let reference = Engine::new(&settings())
            .unwrap()
            .analyze_prepared(&audio)
            .unwrap();
        let selection = AnalysisSelection::ALL;
        let mut engine = Engine::new_selected(&settings(), selection).unwrap();
        let result = engine.analyze_prepared_selected(&audio, selection).unwrap();
        for (a, b) in [
            (value(&result.analysis.key), value(&reference.analysis.key)),
            (
                value(&result.analysis.tempo),
                value(&reference.analysis.tempo),
            ),
            (
                value(&result.analysis.energy),
                value(&reference.analysis.energy),
            ),
            (
                value(&result.analysis.lexicon_cues),
                value(&reference.analysis.lexicon_cues),
            ),
            (
                value(&result.analysis.models),
                value(&reference.analysis.models),
            ),
        ] {
            assert_eq!(a, b);
        }
        assert_eq!(
            result.analysis.engine_source_sha256,
            reference.analysis.engine_source_sha256
        );
        assert_ne!(
            result.analysis.config_sha256,
            reference.analysis.config_sha256
        );
        assert_eq!(result.selection, Some(selection));
        assert_eq!(engine.selected_stage_counts(), [1, 1, 1, 1]);
    }
    #[test]
    fn bpm_and_grid_selection_are_independent() {
        let (_dir, audio) = fixture();
        let bpm = AnalysisSelection {
            bpm: true,
            ..Default::default()
        };
        let grid = AnalysisSelection {
            beatgrid: true,
            ..Default::default()
        };
        let b = Engine::new_selected(&settings(), bpm)
            .unwrap()
            .analyze_prepared_selected(&audio, bpm)
            .unwrap();
        assert!(b.analysis.tempo.is_some());
        assert!(b.analysis.tempo.as_ref().unwrap().beats_seconds.is_empty());
        assert!(b.selected_beatgrid.is_none());
        let g = Engine::new_selected(&settings(), grid)
            .unwrap()
            .analyze_prepared_selected(&audio, grid)
            .unwrap();
        assert!(g.analysis.tempo.is_none());
        assert!(
            !g.selected_beatgrid
                .as_ref()
                .unwrap()
                .beats_seconds
                .is_empty()
        );
        assert!(g.analysis.key.is_none());
    }
    #[test]
    fn all_off_refuses_work_before_model_loading() {
        let settings = EngineSettings {
            key_model: Some("missing".into()),
            energy_model: Some("missing".into()),
            ..Default::default()
        };
        assert!(Engine::new_selected(&settings, AnalysisSelection::default()).is_err());
    }
}
