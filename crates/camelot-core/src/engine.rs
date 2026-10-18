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
    /// Read-only migration marker: enabled legacy cue configurations are rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cue_config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lexicon_config: Option<crate::lexicon_cues::LexiconConfig>,
    /// Local, explicitly supplied RBF SVC; produces one duration-modal track rating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy_model: Option<PathBuf>,
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
            cue_config: None,
            lexicon_config: None,
            energy_model: None,
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
            clock: audio.clock.clone(),
        });
        result.analysis.config_sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(
                &serde_json::json!({"analysis":result.analysis.config_sha256,"decoder":audio.decoder_identity,
                    "feature_profile":if legacy_profile {"validated-file-profile"} else {"canonical-full-pcm16-v1"}})
            )?)
        );
        result.analysis.engine.push_str("+common-media-v1");
        Ok(result)
    }

    fn analyze_file_inner(
        &mut self,
        path: &Path,
        prepared: Option<&cuerithm_media::PreparedAudio>,
    ) -> Result<FileAnalysis> {
        let start = Instant::now();
        let source_snapshot = (prepared.is_none()
            && (self.settings.source_profile_timing || self.energy.is_some()))
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
        let decoded = audio::decode_at_rates(decode_path, &rates, self.settings.max_seconds)?;
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
        let compute_tempo = compute_tempo && !self.settings.source_profile_timing;
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
            config_identity["lexicon_sample_rate"] = 44100.into();
        }
        if self.energy.is_some() {
            config_identity["energy_method"] = "svc-duration-mode-v1".into();
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
                    let input = decoded
                        .iter()
                        .find(|a| a.sample_rate == 44100)
                        .context("Missing cue input rate")?;
                    let first = tempo
                        .downbeat_time
                        .or_else(|| tempo.beats_seconds.first().copied())
                        .unwrap_or(0.0);
                    crate::lexicon_cues::try_generate_structural_cues(
                        &input.samples,
                        input.sample_rate,
                        tempo.bpm,
                        first,
                        config,
                    )?
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
        }
        if let Some(model) = &self.energy {
            let segments = match &profile {
                Some(p) => camelot_source_analysis::analyze_energy_pcm(p)?,
                None => camelot_source_analysis::analyze_energy_file(decode_path)?,
            };
            analysis.energy = model.estimate(&segments)?;
            analysis.engine.push_str("+track-energy-svc-v1");
        }
        if let Some(snapshot) = &source_snapshot {
            ensure!(
                snapshot.identity == SourceIdentity::read(path)?,
                "Source changed during analysis"
            );
        }
        Ok(FileAnalysis {
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
        ensure!(
            path.to_string_lossy()
                .to_ascii_lowercase()
                .ends_with("flac"),
            "FLAC reader path required"
        );
        let mut source = std::fs::File::open(path).context("Open source snapshot input")?;
        let before = source.metadata()?;
        ensure!(before.is_file(), "Source must be a regular file");
        let mut file = tempfile::Builder::new()
            .prefix("camelot-source-")
            .suffix(".flac")
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
