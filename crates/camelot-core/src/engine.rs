//! Stateful engine facade shared by the CLI and future desktop host.
use crate::{
    Analysis, AnalysisConfig, analysis::analyze_components, audio, profile::KeyProfileModel,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSettings {
    pub config: AnalysisConfig,
    pub sample_rate: u32,
    pub max_seconds: Option<f64>,
    pub beat_model: Option<PathBuf>,
    pub mel_model: Option<PathBuf>,
    pub key_model: Option<PathBuf>,
    pub key_neural_model: Option<PathBuf>,
    /// Pinned OpenKeyScan3 graph, evaluated on direct 44100 Hz PCM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_openkeyscan_model: Option<PathBuf>,
    /// Include the experimental PCM structure timeline; does not predict cues.
    #[serde(default)]
    pub structure_features: bool,
    /// Use the complete, verified source-profile FLAC timing pipeline.
    #[serde(default)]
    pub source_profile_timing: bool,
    /// Complete local first-channel timing; independent of the key/energy mix.
    #[serde(default, skip_serializing_if = "is_false")]
    pub lexicon_timing: bool,
    /// Read-only migration marker: enabled legacy cue configurations are rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cue_config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexicon_config: Option<crate::lexicon_cues::LexiconConfig>,
    /// Local, explicitly supplied RBF SVC; produces one duration-modal track rating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_model: Option<PathBuf>,
    /// Built-in whole-track acoustic energy, evaluated on 22050 Hz mono PCM.
    #[serde(default)]
    pub lexicon_energy: bool,
}

fn is_false(value: &bool) -> bool {
    !value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_backends_request_their_own_direct_rates_without_duplicates() {
        let mut settings = EngineSettings {
            beat_model: Some("beat.onnx".into()),
            mel_model: Some("mel.onnx".into()),
            key_openkeyscan_model: Some("openkeyscan.onnx".into()),
            ..Default::default()
        };
        assert_eq!(settings.input_rates(), [11025, 22050, 44100]);
        settings.sample_rate = 44100;
        assert_eq!(settings.input_rates(), [44100, 22050]);
        settings.beat_model = None;
        settings.mel_model = None;
        assert_eq!(settings.input_rates(), [44100]);
        settings.key_openkeyscan_model = None;
        settings.key_neural_model = Some("skey.onnx".into());
        settings.sample_rate = 22050;
        assert_eq!(settings.input_rates(), [22050]);
        settings.lexicon_config = Some(Default::default());
        assert_eq!(settings.input_rates(), [22050, 44100]);
    }

    #[test]
    fn source_identity_rejects_same_size_content_changes_with_restored_timestamp() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.flac");
        std::fs::write(&path, b"first").unwrap();
        let before = SourceIdentity::read(&path).unwrap();
        let modified = path.metadata().unwrap().modified().unwrap();
        assert_eq!(before, SourceIdentity::read(&path).unwrap());
        std::fs::write(&path, b"other").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert_ne!(before, SourceIdentity::read(&path).unwrap());
    }

    #[test]
    fn both_decoder_inputs_stay_on_a_during_transient_source_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.flac");
        let parked = temp.path().join("parked.flac");
        std::fs::write(&path, b"encoded A").unwrap();
        let snapshot = SourceSnapshot::capture(&path).unwrap();
        let private_path = snapshot.file.path().to_path_buf();
        let timing_input = std::fs::read(snapshot.file.path()).unwrap();
        std::fs::rename(&path, &parked).unwrap();
        std::fs::write(&path, b"different encoded B").unwrap();
        let key_input = std::fs::read(snapshot.file.path()).unwrap();
        assert_ne!(std::fs::read(&path).unwrap(), key_input);
        std::fs::remove_file(&path).unwrap();
        std::fs::rename(&parked, &path).unwrap();
        // A pathname-only guard accepts the restored A, but both actual decoder
        // opens must still be bound to the captured A while B occupies the path.
        assert_eq!(snapshot.identity, SourceIdentity::read(&path).unwrap());
        assert_eq!(timing_input, b"encoded A");
        assert_eq!(key_input, timing_input);
        drop(snapshot);
        assert!(!private_path.exists());
    }

    #[test]
    fn timing_snapshot_preserves_encoded_identity_and_probe_extension_for_common_files() {
        let temp = tempfile::tempdir().unwrap();
        for extension in ["wav", "aiff", "mp3", "m4a", "ogg", "opus", "flac", "FLAC"] {
            let path = temp.path().join(format!("source.{extension}"));
            std::fs::write(&path, b"encoded A").unwrap();
            let snapshot = SourceSnapshot::capture(&path).unwrap();
            assert_eq!(std::fs::read(snapshot.file.path()).unwrap(), b"encoded A");
            assert_eq!(
                snapshot.file.path().extension().unwrap(),
                extension.to_ascii_lowercase().as_str()
            );
            std::fs::write(&path, b"encoded B").unwrap();
            assert_ne!(snapshot.identity, SourceIdentity::read(&path).unwrap());
            assert_eq!(std::fs::read(snapshot.file.path()).unwrap(), b"encoded A");
        }
    }
}

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            config: AnalysisConfig::default(),
            sample_rate: 11025,
            max_seconds: None,
            beat_model: None,
            mel_model: None,
            key_model: None,
            key_neural_model: None,
            key_openkeyscan_model: None,
            structure_features: false,
            source_profile_timing: false,
            lexicon_timing: false,
            cue_config: None,
            lexicon_config: None,
            energy_model: None,
            lexicon_energy: false,
        }
    }
}

impl EngineSettings {
    fn input_rates(&self) -> Vec<u32> {
        let mut rates = vec![self.sample_rate];
        for rate in [
            self.beat_model.as_ref().map(|_| 22050),
            self.key_neural_model.as_ref().map(|_| 22050),
            self.key_openkeyscan_model.as_ref().map(|_| 44100),
            self.lexicon_config.as_ref().map(|_| 44100),
            self.lexicon_energy.then_some(22050),
        ]
        .into_iter()
        .flatten()
        {
            if !rates.contains(&rate) {
                rates.push(rate);
            }
        }
        rates
    }

    pub fn validate(&self) -> Result<()> {
        self.config.validate()?;
        ensure!(
            !(self.lexicon_timing && self.source_profile_timing),
            "Select one timing method"
        );
        if self.lexicon_timing {
            ensure!(
                self.max_seconds.is_none(),
                "Lexicon timing requires complete input"
            );
            ensure!(
                self.beat_model.is_none() && self.mel_model.is_none(),
                "Lexicon timing cannot be combined with learned beat models"
            );
        }
        ensure!(
            !self.lexicon_energy || self.energy_model.is_none(),
            "Select one track energy method"
        );
        ensure!(
            !self.lexicon_energy || self.max_seconds.is_none(),
            "Track energy requires complete input"
        );
        if self.source_profile_timing {
            ensure!(
                self.max_seconds.is_none(),
                "Source timing requires complete input"
            );
            ensure!(
                self.beat_model.is_none() && self.mel_model.is_none(),
                "Source timing cannot be combined with learned beat models"
            );
            ensure!(
                self.cue_config.is_none(),
                "Source timing cannot be combined with cue suggestions"
            );
        }
        ensure!(
            (8000..=192000).contains(&self.sample_rate),
            "Invalid analysis sample rate"
        );
        ensure!(
            self.max_seconds.is_none_or(|x| x.is_finite() && x > 0.0),
            "Invalid analysis duration"
        );
        ensure!(
            self.beat_model.is_some() == self.mel_model.is_some(),
            "Both mel and beat model files are required"
        );
        ensure!(
            [
                self.key_model.as_ref(),
                self.key_neural_model.as_ref(),
                self.key_openkeyscan_model.as_ref()
            ]
            .into_iter()
            .flatten()
            .count()
                <= 1,
            "Select only one key backend: learned DSP profiles, S-KEY or OpenKeyScan"
        );
        ensure!(
            self.cue_config.is_none(),
            "Legacy cue suggestions are retired; use lexicon_config"
        );
        if let Some(config) = &self.lexicon_config {
            config.validate()?;
        }
        ensure!(
            self.energy_model.is_none() || self.max_seconds.is_none(),
            "Track energy requires complete input"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceTimingMetadata {
    pub profile: String,
    pub full_source_frames: usize,
    pub analyzed_frames: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysis {
    /// App publication is derived after DSP; older results retain their raw data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<crate::lexicon_publication::Publication>,
    pub analysis: Analysis,
    pub complete: bool,
    pub verified_integrity: Option<bool>,
    pub source_sample_rate: u32,
    /// Actual input rate used by the optional learned beat frontend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_sample_rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub neural_key_sample_rate: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openkeyscan_sample_rate: Option<u32>,
    /// Reader profile and prefix boundary used by source timing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_timing: Option<SourceTimingMetadata>,
    pub elapsed_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media: Option<crate::prepared::PreparedMediaIdentity>,
}

pub struct Engine {
    settings: EngineSettings,
    key: Option<KeyProfileModel>,
    energy: Option<crate::energy::EnergyModel>,
    #[cfg(feature = "neural-key")]
    neural_key: Option<crate::neural_key::NeuralKeyModel>,
    #[cfg(feature = "openkeyscan-key")]
    openkeyscan: Option<crate::openkeyscan::OpenKeyScanModel>,
    #[cfg(feature = "neural-beats")]
    beats: Option<crate::neural::NeuralBeatTracker>,
}

impl Engine {
    pub fn new(settings: &EngineSettings) -> Result<Self> {
        settings.validate()?;
        let key = settings
            .key_model
            .as_deref()
            .map(KeyProfileModel::load)
            .transpose()?;
        if let Some(model) = &key {
            model.validate_for(settings.sample_rate, &settings.config)?;
        }
        #[cfg(feature = "neural-key")]
        let neural_key = settings
            .key_neural_model
            .as_deref()
            .map(crate::neural_key::NeuralKeyModel::load)
            .transpose()?;
        #[cfg(not(feature = "neural-key"))]
        ensure!(
            settings.key_neural_model.is_none(),
            "Build with --features neural-key to use S-KEY"
        );
        #[cfg(feature = "openkeyscan-key")]
        let openkeyscan = settings
            .key_openkeyscan_model
            .as_deref()
            .map(crate::openkeyscan::OpenKeyScanModel::load)
            .transpose()?;
        #[cfg(not(feature = "openkeyscan-key"))]
        ensure!(
            settings.key_openkeyscan_model.is_none(),
            "Build with --features openkeyscan-key to use OpenKeyScan"
        );
        #[cfg(feature = "neural-beats")]
        let beats = match (&settings.mel_model, &settings.beat_model) {
            (Some(mel), Some(beat)) => Some(crate::neural::NeuralBeatTracker::new(mel, beat)?),
            _ => None,
        };
        #[cfg(not(feature = "neural-beats"))]
        ensure!(
            settings.beat_model.is_none(),
            "Build with --features neural-beats to use beat models"
        );
        let energy = settings
            .energy_model
            .as_deref()
            .map(crate::energy::EnergyModel::load)
            .transpose()?;
        Ok(Self {
            energy,
            settings: settings.clone(),
            key,
            #[cfg(feature = "neural-key")]
            neural_key,
            #[cfg(feature = "openkeyscan-key")]
            openkeyscan,
            #[cfg(feature = "neural-beats")]
            beats,
        })
    }

    pub fn model_identity(&self) -> BTreeMap<String, String> {
        let mut models = BTreeMap::new();
        if self.settings.lexicon_config.is_some() {
            models.insert(
                "lexicon-cues-dsp".into(),
                crate::lexicon_cue_frontend::implementation_sha256(),
            );
        }
        if self.settings.lexicon_timing {
            models.insert(
                "lexicon-timing-dsp".into(),
                crate::lexicon_timing::implementation_sha256(),
            );
            let mut frontend = Sha256::new();
            for source in [
                include_bytes!("audio.rs").as_slice(),
                include_bytes!("prepared.rs").as_slice(),
                include_bytes!("engine.rs").as_slice(),
            ] {
                frontend.update((source.len() as u64).to_le_bytes());
                frontend.update(source);
            }
            models.insert(
                "lexicon-timing-input".into(),
                format!("{:x}", frontend.finalize()),
            );
        }
        if self.settings.lexicon_energy {
            models.insert(
                "track-energy-dsp".into(),
                format!("{:x}", Sha256::digest(include_bytes!("lexicon_energy.rs"))),
            );
        }
        if let Some(energy) = &self.energy {
            models.insert("track-energy-svc".into(), energy.source_sha256().into());
        }
        if let Some(key) = &self.key {
            models.insert("key-profile".into(), key.source_sha256().to_string());
        }
        #[cfg(feature = "neural-key")]
        if let Some(key) = &self.neural_key {
            models.insert("skey-key".into(), key.source_sha256().to_string());
        }
        #[cfg(feature = "openkeyscan-key")]
        if let Some(key) = &self.openkeyscan {
            models.insert("openkeyscan-key".into(), key.source_sha256().to_string());
            models.insert(
                "openkeyscan-frontend".into(),
                crate::openkeyscan::frontend_sha256(),
            );
            models.insert(
                "openkeyscan-native".into(),
                crate::openkeyscan::NATIVE_SHA256.into(),
            );
        }
        #[cfg(feature = "neural-beats")]
        if let Some(beats) = &self.beats {
            models.extend(beats.model_identity().clone());
        }
        models
    }

    pub fn analyze_file(&mut self, path: &Path) -> Result<FileAnalysis> {
        self.analyze_file_inner(path, None)
    }

    pub fn analyze_prepared(
        &mut self,
        audio: &cuerithm_media::PreparedAudio,
    ) -> Result<FileAnalysis> {
        let configured = self.settings.lexicon_config.clone();
        if let Some(config) = &mut self.settings.lexicon_config
            && config.genre.is_none()
        {
            config.genre = audio
                .info
                .tags
                .get("genre")
                .filter(|value| !value.trim().is_empty())
                .cloned();
        }
        // Keep loaded key/beat models, while restoring per-worker settings on
        // both success and failure before another track uses this engine.
        let result = self.analyze_prepared_inner(audio);
        self.settings.lexicon_config = configured;
        result
    }

    fn analyze_prepared_inner(
        &mut self,
        audio: &cuerithm_media::PreparedAudio,
    ) -> Result<FileAnalysis> {
        ensure!(
            self.settings.max_seconds.is_none(),
            "Preparation requires complete input"
        );
        ensure!(
            audio.source_is_unchanged()?,
            "Source changed before analysis"
        );
        let legacy_profile = audio.info.codec == "flac"
            && audio.info.sample_rate == 44100
            && audio.info.channels == 2
            && audio.info.bits_per_sample == Some(16);
        let mut result = if legacy_profile {
            self.analyze_file(&audio.snapshot_path)?
        } else {
            let wave = crate::prepared::float_wave(audio)?;
            self.analyze_file_inner(wave.path(), Some(audio))?
        };
        ensure!(
            audio.source_is_unchanged()?,
            "Source changed during analysis"
        );
        result.source_sample_rate = audio.info.sample_rate;
        // A DSP-rate buffer may round by a fraction of a source frame. Public
        // duration belongs to the shared playable clock, as do cues and playback.
        result.analysis.duration_seconds = audio.clock.duration_seconds();
        result.media = Some(crate::prepared::PreparedMediaIdentity {
            decoder: audio.decoder_identity.clone(),
            codec: audio.info.codec.clone(),
            source_sha256: audio.source_sha256.clone(),
            pcm_sha256: audio.pcm_sha256()?,
            clock: audio.clock.clone(),
            cue_genre: self
                .settings
                .lexicon_config
                .as_ref()
                .and_then(|c| c.genre.clone()),
        });
        result.analysis.config_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(
                &serde_json::json!({"analysis":result.analysis.config_sha256,"decoder":audio.decoder_identity,
                    "feature_profile":if legacy_profile {"validated-file-profile"} else {"canonical-full-pcm16-v1"}})
            )?)
        );
        result.analysis.engine.push_str("+common-media-v1");
        if self.settings.lexicon_config.is_some() {
            result.analysis.engine.push_str("+cue-genre-input-v1");
        }
        Ok(result)
    }

    /// Reuse verified audio and measured timing when configured catalog/cleanup
    /// changes the effective genre. Key, BPM and scalar energy are preserved.
    pub fn refresh_prepared_cues(
        &self,
        audio: &cuerithm_media::PreparedAudio,
        result: &mut FileAnalysis,
        genre: Option<&str>,
    ) -> Result<bool> {
        let Some(mut config) = self.settings.lexicon_config.clone() else {
            return Ok(false);
        };
        if config.genre.is_none() {
            config.genre = genre.filter(|s| !s.trim().is_empty()).map(str::to_owned);
        }
        let media = result
            .media
            .as_ref()
            .context("Missing prepared cue input identity")?;
        ensure!(
            result.complete
                && media.source_sha256 == audio.source_sha256
                && media.decoder == audio.decoder_identity
                && media.clock.frames == audio.clock.frames
                && media.clock.sample_rate == audio.clock.sample_rate
                && media.pcm_sha256 == audio.pcm_sha256()?
                && audio.source_is_unchanged()?,
            "Prepared cue input changed or belongs to another analysis"
        );
        if media.cue_genre == config.genre {
            return Ok(false);
        }
        let started = Instant::now();
        let cues = if let Some(tempo) = &result.analysis.tempo {
            let mut samples = Vec::with_capacity(audio.frames as usize);
            let mut position = 0;
            while position < audio.frames {
                let chunk = audio.read_frames(position, 65536)?;
                samples.extend(chunk.as_chunks::<2>().0.iter().map(|frame| {
                    if self.settings.lexicon_timing {
                        frame[0]
                    } else {
                        (frame[0] + frame[1]) * 0.5
                    }
                }));
                position += chunk.len() as u64 / 2;
            }
            let first = tempo
                .downbeat_time
                .or_else(|| tempo.beats_seconds.first().copied())
                .unwrap_or(0.);
            let generate = if self.settings.lexicon_timing {
                crate::lexicon_cues::try_generate_renderer_cues
            } else {
                crate::lexicon_cues::try_generate_structural_cues
            };
            generate(&samples, audio.clock.sample_rate, tempo.bpm, first, &config)?
        } else {
            crate::lexicon_cues::StructuralCueResult {
                cues: vec![],
                emergency_loop: None,
            }
        };
        ensure!(
            audio.source_is_unchanged()?,
            "Source changed while refreshing cues"
        );
        result.analysis.config_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(
                &serde_json::json!({"prepared_analysis":result.analysis.config_sha256,"effective_cue_config":config})
            )?)
        );
        result.analysis.cues = Some(
            cues.cues
                .iter()
                .map(|c| crate::Cue {
                    time_seconds: c.time_seconds,
                    name: c.name.clone(),
                })
                .collect(),
        );
        result.analysis.lexicon_cues = Some(cues);
        result.media.as_mut().unwrap().cue_genre = config.genre;
        result.elapsed_seconds += started.elapsed().as_secs_f64();
        Ok(true)
    }

    fn analyze_file_inner(
        &mut self,
        path: &Path,
        prepared: Option<&cuerithm_media::PreparedAudio>,
    ) -> Result<FileAnalysis> {
        let start = Instant::now();
        let source_snapshot = (prepared.is_none()
            && (self.settings.source_profile_timing
                || self.settings.lexicon_timing
                || self.energy.is_some()))
        .then(|| SourceSnapshot::capture(path))
        .transpose()?;
        let decode_path = source_snapshot
            .as_ref()
            .map_or(path, |snapshot| snapshot.file.path());
        let pcm = match prepared {
            Some(audio) if self.settings.source_profile_timing || self.energy.is_some() => {
                Some(audio.stereo_s16()?)
            }
            _ => None,
        };
        let profile = pcm
            .as_ref()
            .map(|samples| camelot_source_analysis::PcmProfile::new(samples, samples.len() / 2))
            .transpose()?;
        let source_timing = self
            .settings
            .source_profile_timing
            .then(|| match &profile {
                Some(p) => camelot_source_analysis::analyze_timing_pcm(p),
                None => camelot_source_analysis::analyze_timing_file(decode_path),
            })
            .transpose()?;
        let beat_sample_rate = self.settings.beat_model.as_ref().map(|_| 22050);
        let neural_key_sample_rate = self.settings.key_neural_model.as_ref().map(|_| 22050);
        let openkeyscan_sample_rate = self.settings.key_openkeyscan_model.as_ref().map(|_| 44100);
        let rates = self.settings.input_rates();
        let (decoded, timing_input) = if self.settings.lexicon_timing {
            let (mixed, first) =
                audio::decode_at_rates_with_first_channel(decode_path, &rates, None)?;
            ensure!(first.complete, "Lexicon timing requires complete input");
            (mixed, Some(first))
        } else {
            (
                audio::decode_at_rates(decode_path, &rates, self.settings.max_seconds)?,
                None,
            )
        };
        if let Some(snapshot) = &source_snapshot {
            ensure!(
                snapshot.identity == SourceIdentity::read(path)?,
                "Source changed between timing and key decodes"
            );
        }
        let audio = decoded
            .iter()
            .find(|a| a.sample_rate == self.settings.sample_rate)
            .context("Missing DSP input rate")?;
        #[cfg(feature = "neural-beats")]
        let compute_tempo = self.beats.is_none();
        #[cfg(not(feature = "neural-beats"))]
        let compute_tempo = true;
        let compute_tempo =
            compute_tempo && !self.settings.source_profile_timing && !self.settings.lexicon_timing;
        let mut analysis = analyze_components(
            &audio.samples,
            audio.sample_rate,
            &self.settings.config,
            compute_tempo,
        )?;
        let source_timing = source_timing.map(|timing| {
            analysis.tempo = Some(crate::TempoEstimate {
                bpm: timing.bpm,
                strength: None,
                candidates: Vec::new(),
                beats_seconds: timing.beats_seconds,
                downbeats_seconds: None,
                downbeat_time: Some(timing.downbeat_time),
                method: if prepared.is_some() {
                    "canonical-pcm-timing-v1"
                } else {
                    "source-profile-timing-v1"
                }
                .into(),
            });
            analysis.engine.push_str("+source-profile-timing-v1");
            SourceTimingMetadata {
                profile: timing.profile,
                full_source_frames: timing.full_source_frames,
                analyzed_frames: timing.analyzed_frames,
            }
        });
        if let Some(input) = &timing_input {
            analysis.duration_seconds = input.duration_seconds();
            analysis.tempo = crate::lexicon_timing::analyze(
                &input.samples,
                input.sample_rate,
                &crate::lexicon_timing::LexiconTempoOptions::default(),
            )?
            .map(|timing| {
                let period = 60. / timing.bpm;
                let duration = input.duration_seconds();
                let beats = (0..)
                    .map(|index| timing.first_beat_seconds + index as f64 * period)
                    .take_while(|beat| *beat < duration)
                    .collect();
                crate::TempoEstimate {
                    bpm: timing.bpm,
                    strength: Some(timing.confidence),
                    candidates: Vec::new(),
                    beats_seconds: beats,
                    downbeats_seconds: None,
                    downbeat_time: Some(timing.first_beat_seconds),
                    method: crate::lexicon_timing::METHOD.into(),
                }
            });
            analysis.engine.push_str("+lexicon-timing-v1");
        }
        if let Some(model) = &self.key {
            if let Some(key) = &mut analysis.key {
                model.apply(key)?;
            }
            analysis.engine.push_str("+key-profile-v1");
        }
        #[cfg(feature = "neural-key")]
        if let Some(model) = &mut self.neural_key {
            let input = decoded
                .iter()
                .find(|a| a.sample_rate == 22050)
                .context("Missing S-KEY input rate")?;
            let features = analysis.key.take().and_then(|k| k.features);
            analysis.key = model.analyze(&input.samples, input.sample_rate)?;
            if let Some(key) = &mut analysis.key {
                key.features = features;
            }
            analysis.engine.push_str("+skey-rten-v1");
        }
        #[cfg(feature = "openkeyscan-key")]
        if let Some(model) = &mut self.openkeyscan {
            let input = decoded
                .iter()
                .find(|a| a.sample_rate == 44100)
                .context("Missing OpenKeyScan input rate")?;
            let features = analysis.key.take().and_then(|k| k.features);
            analysis.key = model.analyze(&input.samples, input.sample_rate)?;
            if let Some(key) = &mut analysis.key {
                key.features = features;
            }
            analysis.engine.push_str("+openkeyscan-cqt-rten-v1");
        }
        #[cfg(feature = "neural-beats")]
        if let Some(tracker) = &mut self.beats {
            let beat_audio = decoded
                .iter()
                .find(|a| a.sample_rate == 22050)
                .context("Missing beat input rate")?;
            analysis.tempo = tracker.analyze(&beat_audio.samples, beat_audio.sample_rate)?;
            analysis.engine.push_str("+beat-this-1.1.0-22050hz");
        }
        analysis.models = self.model_identity();
        // Cover all effective analysis settings, including cue configuration,
        // without making model file locations part of content identity.
        let mut config_identity = serde_json::json!({
            "dsp": self.settings.config, "sample_rate": self.settings.sample_rate,
            "max_seconds": self.settings.max_seconds, "cue_config": self.settings.cue_config,
            "structure_features": self.settings.structure_features,
            "source_profile_timing": self.settings.source_profile_timing,
            "beat_sample_rate": beat_sample_rate,
            "neural_key_sample_rate": neural_key_sample_rate,
            "models": analysis.models,
        });
        if let Some(config) = &self.settings.lexicon_config {
            config_identity["lexicon_config"] = serde_json::to_value(config)?;
            config_identity["lexicon_sample_rate"] = timing_input
                .as_ref()
                .map_or(44100, |input| input.sample_rate)
                .into();
            if timing_input.is_some() {
                config_identity["lexicon_channel"] = "first".into();
                config_identity["lexicon_frontend"] = "webaudio-q1db-v1".into();
            }
        }
        if let Some(input) = &timing_input {
            config_identity["lexicon_timing"] = serde_json::json!({
                "method": crate::lexicon_timing::METHOD,
                "options": crate::lexicon_timing::LexiconTempoOptions::default(),
                "channel": "first", "sample_rate": input.sample_rate,
            });
        }
        if self.energy.is_some() {
            config_identity["energy_method"] = "svc-duration-mode-v1".into();
        } else if self.settings.lexicon_energy {
            config_identity["energy_method"] = crate::lexicon_energy::METHOD.into();
        }
        if let Some(rate) = openkeyscan_sample_rate {
            config_identity["openkeyscan_sample_rate"] = rate.into();
        }
        analysis.config_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&config_identity)?)
        );
        if self.settings.structure_features {
            analysis.structure = Some(crate::structure::extract(
                &audio.samples,
                audio.sample_rate,
            )?);
        }
        if let Some(config) = &self.settings.lexicon_config {
            let result = match &analysis.tempo {
                Some(tempo) => {
                    let input = match &timing_input {
                        Some(first) => first,
                        None => decoded
                            .iter()
                            .find(|a| a.sample_rate == 44100)
                            .context("Missing cue input rate")?,
                    };
                    let first = tempo
                        .downbeat_time
                        .or_else(|| tempo.beats_seconds.first().copied())
                        .unwrap_or(0.0);
                    let generate = if timing_input.is_some() {
                        crate::lexicon_cues::try_generate_renderer_cues
                    } else {
                        crate::lexicon_cues::try_generate_structural_cues
                    };
                    generate(&input.samples, input.sample_rate, tempo.bpm, first, config)?
                }
                None => crate::lexicon_cues::StructuralCueResult {
                    cues: Vec::new(),
                    emergency_loop: None,
                },
            };
            analysis.cues = Some(
                result
                    .cues
                    .iter()
                    .map(|c| crate::Cue {
                        time_seconds: c.time_seconds,
                        name: c.name.clone(),
                    })
                    .collect(),
            );
            analysis.lexicon_cues = Some(result);
            analysis.engine.push_str("+lexicon-cues-v1");
            if timing_input.is_some() {
                analysis.engine.push_str("+lexicon-cue-webaudio-v1");
            }
        }
        if let Some(model) = &self.energy {
            let segments = match &profile {
                Some(p) => camelot_source_analysis::analyze_energy_pcm(p)?,
                None => camelot_source_analysis::analyze_energy_file(decode_path)?,
            };
            analysis.energy = model.estimate(&segments)?;
            analysis.engine.push_str("+track-energy-svc-v1");
        } else if self.settings.lexicon_energy {
            let input = decoded
                .iter()
                .find(|a| a.sample_rate == 22050)
                .context("Missing energy input rate")?;
            analysis.energy = crate::lexicon_energy::analyze(&input.samples, input.sample_rate)?
                .map(|result| crate::energy::TrackEnergy {
                    level: result.level,
                    method: crate::lexicon_energy::METHOD.into(),
                });
            analysis.engine.push_str("+track-energy-dsp-v1");
        }
        if let Some(snapshot) = &source_snapshot {
            ensure!(
                snapshot.identity == SourceIdentity::read(path)?,
                "Source changed during analysis"
            );
        }
        Ok(FileAnalysis {
            publication: None,
            analysis,
            complete: audio.complete,
            verified_integrity: audio.verified_integrity,
            source_sample_rate: audio.source_sample_rate,
            beat_sample_rate,
            neural_key_sample_rate,
            openkeyscan_sample_rate,
            source_timing,
            elapsed_seconds: start.elapsed().as_secs_f64(),
            media: None,
        })
    }
}

/// Guard the separate source-timing and key decodes with content as well as
/// length/mtime, so a same-size rewrite with a restored timestamp is rejected.
#[derive(Debug, PartialEq, Eq)]
struct SourceIdentity {
    len: u64,
    modified: std::time::SystemTime,
    sha256: [u8; 32],
}

/// Both decoder opens use this private encoded copy. The original pathname can
/// change transiently without changing either consumer's input. Drop removes it
/// on success and error; the result never exposes its name.
struct SourceSnapshot {
    file: tempfile::NamedTempFile,
    identity: SourceIdentity,
}

impl SourceSnapshot {
    fn capture(path: &Path) -> Result<Self> {
        let mut source = std::fs::File::open(path).context("Open source snapshot input")?;
        let before = source.metadata()?;
        ensure!(before.is_file(), "Source must be a regular file");
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| value.len() <= 16 && value.bytes().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or("audio")
            .to_ascii_lowercase();
        let mut file = tempfile::Builder::new()
            .prefix("camelot-source-")
            .suffix(&format!(".{extension}"))
            .tempfile()
            .context("Create private source snapshot")?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        let mut len = 0_u64;
        loop {
            let count = source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
            len += count as u64;
        }
        file.flush()?;
        let after = source.metadata()?;
        ensure!(
            before.len() == len && after.len() == len && before.modified()? == after.modified()?,
            "Source changed while copying snapshot"
        );
        let identity = SourceIdentity {
            len,
            modified: before.modified()?,
            sha256: hash.finalize().into(),
        };
        ensure!(
            identity == SourceIdentity::read(path)?,
            "Source changed while copying snapshot"
        );
        Ok(Self { file, identity })
    }
}

impl SourceIdentity {
    fn read(path: &Path) -> Result<Self> {
        let mut file = std::fs::File::open(path).context("Open source identity")?;
        let before = file.metadata()?;
        ensure!(before.is_file(), "Source must be a regular file");
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
        let after = path.metadata()?;
        ensure!(
            before.len() == after.len() && before.modified()? == after.modified()?,
            "Source changed while checking identity"
        );
        Ok(Self {
            len: before.len(),
            modified: before.modified()?,
            sha256: hash.finalize().into(),
        })
    }
}
