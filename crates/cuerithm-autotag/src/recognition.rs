//! Optional recording identification; measured audio analysis remains independent.
use crate::{CatalogAuth, CatalogMatch, CatalogRuntime, CatalogService, Mbid, MusicBrainzBackend};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use cuerithm_media::PreparedAudio;
use rusty_chromaprint::{Configuration, FingerprintCompressor, Fingerprinter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecognitionProvider {
    #[default]
    #[serde(rename = "acoustid")]
    AcoustId,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RecognitionConfig {
    pub enabled: bool,
    pub provider: RecognitionProvider,
    pub fingerprint_consent: bool,
    pub audio_upload_consent: bool,
    pub max_attempts_per_track: u8,
}
impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: RecognitionProvider::AcoustId,
            fingerprint_consent: false,
            audio_upload_consent: false,
            max_attempts_per_track: 1,
        }
    }
}
impl RecognitionConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.max_attempts_per_track == 1,
            "Recognition allows one lookup per preparation attempt"
        );
        ensure!(
            !self.enabled || self.fingerprint_consent,
            "Enable fingerprint transmission consent before identification"
        );
        ensure!(
            !self.audio_upload_consent,
            "No raw-audio recognition adapter is enabled by this configuration"
        );
        Ok(())
    }
}
pub fn acoustid_application_configured() -> bool {
    application_key().is_some()
}
fn application_key() -> Option<&'static str> {
    option_env!("CUERITHM_ACOUSTID_CLIENT_KEY").filter(|s| !s.trim().is_empty())
}

/// Compressed algorithm-2 Chromaprint; caller owns privacy and transmission consent.
pub fn chromaprint_pcm(samples: &[i16], sample_rate: u32, channels: u32) -> Result<String> {
    ensure!(
        (8000..=192000).contains(&sample_rate) && (1..=2).contains(&channels),
        "Unsupported fingerprint PCM layout"
    );
    ensure!(
        samples.len().is_multiple_of(channels as usize)
            && samples.len() <= sample_rate as usize * channels as usize * 120,
        "Fingerprint input exceeds 120 seconds"
    );
    let config = Configuration::preset_test2();
    let mut printer = Fingerprinter::new(&config);
    printer
        .start(sample_rate, channels)
        .map_err(|_| anyhow::anyhow!("Fingerprint initialization failed"))?;
    for chunk in samples.chunks(65536) {
        printer.consume(chunk);
    }
    printer.finish();
    ensure!(
        !printer.fingerprint().is_empty(),
        "Audio is too short for a fingerprint"
    );
    Ok(
        URL_SAFE_NO_PAD
            .encode(FingerprintCompressor::from(&config).compress(printer.fingerprint())),
    )
}
const MIN_FINGERPRINT_SECONDS: u64 = 12;
pub(crate) fn recognition_skip_reason(audio: &PreparedAudio) -> Option<&'static str> {
    (audio.frames < audio.clock.sample_rate as u64 * MIN_FINGERPRINT_SECONDS)
        .then_some("Identification skipped: fingerprint lookup needs at least 12 seconds of audio")
}
fn fingerprint_prepared(audio: &PreparedAudio, cancel: &Arc<AtomicBool>) -> Result<String> {
    let config = Configuration::preset_test2();
    let mut printer = Fingerprinter::new(&config);
    printer
        .start(audio.clock.sample_rate, 2)
        .map_err(|_| anyhow::anyhow!("Fingerprint initialization failed"))?;
    let end = audio.frames.min(audio.clock.sample_rate as u64 * 120);
    let mut position = 0;
    ensure!(
        end >= audio.clock.sample_rate as u64 * MIN_FINGERPRINT_SECONDS,
        "Audio is too short for recording identification"
    );
    while position < end {
        ensure!(!cancel.load(Ordering::Relaxed), "Identification cancelled");
        let block = audio.read_frames(position, ((end - position).min(65536)) as usize)?;
        ensure!(
            block.iter().all(|s| s.is_finite()),
            "Fingerprint PCM contains invalid samples"
        );
        let pcm = block
            .iter()
            .map(|s| (s.clamp(-1., 32767. / 32768.) * 32768.).round() as i16)
            .collect::<Vec<_>>();
        printer.consume(&pcm);
        position += block.len() as u64 / 2;
    }
    printer.finish();
    ensure!(
        !printer.fingerprint().is_empty(),
        "Recording fingerprint is empty"
    );
    Ok(
        URL_SAFE_NO_PAD
            .encode(FingerprintCompressor::from(&config).compress(printer.fingerprint())),
    )
}
fn recording_ids(body: &Value) -> Result<Vec<Mbid>> {
    ensure!(body["status"] == "ok", "AcoustID lookup was unavailable");
    let results = body["results"]
        .as_array()
        .context("AcoustID result collection missing")?;
    ensure!(
        results.len() <= 100,
        "AcoustID response exceeds candidate bound"
    );
    let mut ranked = Vec::new();
    for result in results {
        let score = result["score"]
            .as_f64()
            .filter(|s| s.is_finite() && (0.0..=1.).contains(s))
            .context("AcoustID score invalid")?;
        if score < 0.85 {
            continue;
        }
        let Some(recordings) = result["recordings"].as_array() else {
            continue;
        };
        ensure!(
            recordings.len() <= 100,
            "AcoustID recording collection exceeds bound"
        );
        for r in recordings {
            ranked.push((
                score,
                Mbid::new(
                    r["id"]
                        .as_str()
                        .context("AcoustID recording identifier missing")?,
                )?,
            ));
        }
    }
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut ids = Vec::new();
    for (_, id) in ranked {
        if !ids.contains(&id) {
            ids.push(id);
        }
        if ids.len() == 5 {
            break;
        }
    }
    Ok(ids)
}
pub struct RecognitionCandidates {
    pub matches: Vec<CatalogMatch>,
    pub editions_truncated: bool,
}
fn candidate_budget(groups: Vec<Vec<CatalogMatch>>) -> RecognitionCandidates {
    let total: usize = groups.iter().map(Vec::len).sum();
    let mut groups = groups
        .into_iter()
        .map(std::collections::VecDeque::from)
        .collect::<Vec<_>>();
    let mut matches = Vec::new();
    'collect: loop {
        let before = matches.len();
        for group in &mut groups {
            if let Some(candidate) = group.pop_front() {
                matches.push(candidate);
                if matches.len() == 10 {
                    break 'collect;
                }
            }
        }
        if matches.len() == before {
            break;
        }
    }
    RecognitionCandidates {
        editions_truncated: total > matches.len(),
        matches,
    }
}
/// One network lookup, followed by release-aware MusicBrainz candidates. Never an exact-edition claim.
pub fn recognize_prepared(
    runtime: Arc<CatalogRuntime>,
    audio: &PreparedAudio,
    config: &RecognitionConfig,
    cancel: &Arc<AtomicBool>,
) -> Result<RecognitionCandidates> {
    config.validate()?;
    ensure!(
        config.enabled && config.fingerprint_consent,
        "Identification is disabled"
    );
    let key =
        application_key().context("This build has no registered AcoustID application identity")?;
    ensure!(
        audio.source_is_unchanged()?,
        "Audio source changed before identification"
    );
    let fingerprint = fingerprint_prepared(audio, cancel)?;
    ensure!(
        audio.source_is_unchanged()?,
        "Audio source changed during identification"
    );
    let body = runtime.post_form(
        CatalogService::AcoustId,
        "https://api.acoustid.org/v2/lookup",
        &[
            ("client".into(), key.into()),
            (
                "duration".into(),
                audio.clock.duration_seconds().round().to_string(),
            ),
            ("fingerprint".into(), fingerprint),
            ("meta".into(), "recordings".into()),
            ("format".into(), "json".into()),
        ],
        CatalogAuth::None,
        cancel,
    )?;
    let backend = MusicBrainzBackend::new(runtime);
    let mut groups = Vec::new();
    for id in recording_ids(&body)? {
        ensure!(!cancel.load(Ordering::Relaxed), "Identification cancelled");
        let mut found = backend.recording_candidates(&id, cancel)?;
        for candidate in &mut found {
            candidate.recognition = Some(crate::CatalogRecognition {
                schema_version: 1,
                service: crate::CatalogRecognitionService::AcoustId,
                source_sha256: audio.source_sha256.clone(),
                recording_id: id.clone(),
            });
            candidate.validate()?;
        }
        groups.push(found);
    }
    ensure!(!cancel.load(Ordering::Relaxed), "Identification cancelled");
    ensure!(
        audio.source_is_unchanged()?,
        "Audio source changed during identification"
    );
    Ok(candidate_budget(groups))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn optional_recognition_skips_short_audio_without_overriding_catalog_no_match() {
        use crate::{AutotagConfig, MatchStatus, ProviderBackend, ProviderId, TagInput, Tagger};
        struct EmptyCatalog;
        impl ProviderBackend for EmptyCatalog {
            fn id(&self) -> ProviderId {
                ProviderId::Beatport
            }
            fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
                Ok(vec![])
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = temp.path().join("short.flac");
        std::fs::copy(
            repo.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
            &source,
        )
        .unwrap();
        let media =
            cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin"))
                .unwrap();
        let audio = media.decode(&source, temp.path()).unwrap();
        assert!(audio.clock.duration_seconds() < 12.);
        let stop = Arc::new(AtomicBool::new(false));
        assert!(
            fingerprint_prepared(&audio, &stop)
                .unwrap_err()
                .to_string()
                .contains("too short")
        );
        let input = TagInput {
            path: source.clone(),
            source_sha256: audio.source_sha256.clone(),
            duration_seconds: audio.clock.duration_seconds(),
            media: audio.info.clone(),
        };
        let tagger = Tagger::with_backends(vec![Arc::new(EmptyCatalog)]).unwrap();
        let mut config = AutotagConfig::default();
        config.options.platforms = vec!["beatport".into()];
        config.recognition.enabled = true;
        config.recognition.fingerprint_consent = true;
        let original = tagger.match_track(&input, &config).unwrap();
        assert_eq!(original.status, MatchStatus::NoMatch);
        let outcome=tagger.match_prepared_with_album(&input,&audio,&config,&stop,None).expect("optional short-clip identification must preserve catalog NoMatch before any network setup");
        assert_eq!(outcome.status, MatchStatus::NoMatch);
        assert!(outcome.matches.is_empty());
        assert_eq!(outcome.attempted, original.attempted);
        assert!(
            outcome
                .warnings
                .iter()
                .any(|warning| warning.message.contains("12 seconds"))
        );
        assert_eq!(
            cuerithm_media::file_sha256(&source).unwrap(),
            audio.source_sha256
        );
    }
    #[test]
    fn fingerprint_consent_is_explicit_and_raw_upload_is_not_implicit() {
        assert!(RecognitionConfig::default().validate().is_ok());
        assert!(
            RecognitionConfig {
                enabled: true,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            RecognitionConfig {
                enabled: true,
                fingerprint_consent: true,
                audio_upload_consent: true,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn acoustid_low_confidence_is_absence_and_service_error_is_failure() {
        assert!(
            recording_ids(&json!({"status":"ok","results":[{"score":0.4}]}))
                .unwrap()
                .is_empty()
        );
        assert!(recording_ids(&json!({"status":"error","error":{"code":4}})).is_err());
        assert!(recording_ids(&json!({"status":"ok","results":[{"score":0.99,"recordings":[{"id":"not-an-mbid"}]}]})).is_err());
    }
    #[test]
    fn candidate_budget_retains_alternative_recordings_before_more_editions() {
        let group = |name: &str, count| {
            (0..count)
                .map(|_| {
                    CatalogMatch::new(
                        1.,
                        crate::CatalogTrack {
                            title: name.into(),
                            ..Default::default()
                        },
                    )
                })
                .collect::<Vec<_>>()
        };
        let found = candidate_budget(vec![
            group("first", 11),
            group("second", 3),
            group("third", 1),
        ]);
        assert_eq!(found.matches.len(), 10);
        assert!(found.editions_truncated);
        assert!(found.matches.iter().any(|m| m.track.title == "second"));
        assert!(found.matches.iter().any(|m| m.track.title == "third"));
    }
    #[test]
    fn fingerprint_is_bounded_and_deterministic() {
        let samples = (0..11025 * 16)
            .map(|n| ((n as f64 * 0.3).sin() * 18000.) as i16)
            .collect::<Vec<_>>();
        let a = chromaprint_pcm(&samples, 11025, 1).unwrap();
        assert_eq!(a, chromaprint_pcm(&samples, 11025, 1).unwrap());
        assert!(!a.is_empty());
        assert!(chromaprint_pcm(&samples, 0, 1).is_err());
        assert!(chromaprint_pcm(&samples, 11025, 3).is_err());
    }
}
