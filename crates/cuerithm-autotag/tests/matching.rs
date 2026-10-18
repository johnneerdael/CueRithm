use anyhow::Result;
use cuerithm_autotag::{
    AutotagConfig, CatalogMatch, CatalogTrack, MatchStatus, ProviderBackend, ProviderId, TagInput,
    Tagger,
};
use cuerithm_media::MediaInfo;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

fn input() -> TagInput {
    TagInput {
        path: PathBuf::from("Fixture Artist - Signal.flac"),
        source_sha256: "1".repeat(64),
        duration_seconds: 300.,
        media: MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: BTreeMap::new(),
            tags: [
                ("artist".into(), "Fixture Artist".into()),
                ("title".into(), "Signal".into()),
            ]
            .into(),
        },
    }
}
fn track(provider: ProviderId, title: &str) -> CatalogTrack {
    CatalogTrack {
        platform: provider.as_str().into(),
        title: title.into(),
        artists: vec!["Fixture Artist".into()],
        duration: Duration::from_secs(300),
        label: Some("Fixture Label".into()),
        ..Default::default()
    }
}
struct Fake {
    id: ProviderId,
    calls: Arc<Mutex<Vec<ProviderId>>>,
    found: bool,
    fail: bool,
}
impl ProviderBackend for Fake {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
        self.calls.lock().unwrap().push(self.id);
        if self.fail {
            anyhow::bail!("HTTP 403");
        }
        Ok(if self.found {
            vec![CatalogMatch::new(0.95, track(self.id, "Signal"))]
        } else {
            vec![]
        })
    }
}
fn tagger(first: bool, second: bool, fail: bool) -> (Tagger, Arc<Mutex<Vec<ProviderId>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    (
        Tagger::with_backends(vec![
            Arc::new(Fake {
                id: ProviderId::Beatport,
                calls: calls.clone(),
                found: first,
                fail,
            }),
            Arc::new(Fake {
                id: ProviderId::Traxsource,
                calls: calls.clone(),
                found: second,
                fail: false,
            }),
        ])
        .unwrap(),
        calls,
    )
}
#[test]
fn provider_order_first_success_and_multiplatform_are_explicit() {
    let (tagger, calls) = tagger(true, true, false);
    let mut config = AutotagConfig::default();
    let result = tagger.match_track(&input(), &config).unwrap();
    assert_eq!(result.status, MatchStatus::Matched);
    assert_eq!(result.matches.len(), 1);
    assert_eq!(*calls.lock().unwrap(), vec![ProviderId::Beatport]);
    calls.lock().unwrap().clear();
    config.options.multiplatform = true;
    let result = tagger.match_track(&input(), &config).unwrap();
    assert_eq!(result.matches.len(), 2);
    assert_eq!(
        *calls.lock().unwrap(),
        vec![ProviderId::Beatport, ProviderId::Traxsource]
    );
    config.options.platforms.reverse();
    let result = tagger.match_track(&input(), &config).unwrap();
    assert_eq!(result.matches[0].track.platform, "traxsource");
}
#[test]
fn no_match_is_distinct_from_failed_catalogs_and_fallback_records_warnings() {
    let (a, _) = tagger(false, false, false);
    assert_eq!(
        a.match_track(&input(), &AutotagConfig::default())
            .unwrap()
            .status,
        MatchStatus::NoMatch
    );
    let (b, _) = tagger(false, false, true);
    assert!(b.match_track(&input(), &AutotagConfig::default()).is_err());
    let (c, _) = tagger(false, true, true);
    let result = c.match_track(&input(), &AutotagConfig::default()).unwrap();
    assert_eq!(result.matches[0].track.platform, "traxsource");
    assert_eq!(result.warnings.len(), 1);
}
#[test]
fn duration_strictness_custom_fields_and_filename_options_survive() {
    let mut config = AutotagConfig::default();
    config.options.strictness = 1.;
    config.options.match_duration = true;
    config.options.max_duration_difference = 2;
    let mut wrong = track(ProviderId::Beatport, "Signal");
    wrong.duration = Duration::from_secs(320);
    assert!(
        Tagger::match_candidates(&input(), vec![wrong], &config)
            .unwrap()
            .is_empty()
    );
    assert!(
        Tagger::match_candidates(
            &input(),
            vec![track(ProviderId::Beatport, "Other song")],
            &config
        )
        .unwrap()
        .is_empty()
    );
    let mut custom = track(ProviderId::Beatport, "Signal");
    custom.other.push((
        cuerithm_autotag::FrameName::same("CUSTOM_MOOD"),
        vec!["Warm".into()],
    ));
    let matches = Tagger::match_candidates(&input(), vec![custom], &config).unwrap();
    assert_eq!(matches[0].track.other[0].1, vec!["Warm"]);
    config.options.parse_filename = true;
    let mut missing = input();
    missing.media.tags.clear();
    assert_eq!(
        missing.audio_info(&config).unwrap().artists,
        vec!["Fixture Artist"]
    );
    let restored: AutotagConfig =
        serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
    assert_eq!(restored.options.strictness, 1.);
    assert!(restored.options.match_duration);
}
#[test]
fn invalid_or_excluded_configuration_is_rejected() {
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["spotify".into()];
    assert!(config.validate().is_err());
    config = AutotagConfig::default();
    config.options.strictness = f64::NAN;
    assert!(config.validate().is_err());
    config = AutotagConfig::default();
    config.options.post_command = Some("echo unexpected".into());
    assert!(config.validate().is_err());
    config = AutotagConfig::default();
    config.options.threads = 0;
    assert!(config.validate().is_err());
    config = AutotagConfig::default();
    config.options.custom.0.insert(
        "beatport".into(),
        serde_json::json!({"max_pages":0,"art_resolution":500,"ignore_version":false}),
    );
    assert!(config.validate().is_err());
}
#[test]
fn configured_multi_provider_work_does_not_silently_finish_after_one_fails() {
    let (tagger, _) = tagger(false, true, true);
    let mut config = AutotagConfig::default();
    config.options.multiplatform = true;
    assert!(tagger.match_track(&input(), &config).is_err());
}
#[test]
fn actual_media_format_is_used_for_aac_wma_and_wrong_extensions() {
    let config = AutotagConfig::default();
    let mut source = input();
    source.path = PathBuf::from("wrong.bin");
    source.media.codec = "aac".into();
    assert_eq!(
        format!("{:?}", source.audio_info(&config).unwrap().format),
        "AAC"
    );
    source.media.properties.insert(
        "format.format_name".into(),
        "mov,mp4,m4a,3gp,3g2,mj2".into(),
    );
    assert_eq!(
        format!("{:?}", source.audio_info(&config).unwrap().format),
        "MP4"
    );
    source.media.codec = "wmav2".into();
    assert_eq!(
        format!("{:?}", source.audio_info(&config).unwrap().format),
        "WMA"
    );
}
#[test]
fn remix_version_is_not_an_exact_match_when_strictness_requires_it() {
    let mut source = input();
    source
        .media
        .tags
        .insert("title".into(), "Signal (Club Remix)".into());
    let mut candidate = track(ProviderId::Beatport, "Signal");
    candidate.version = Some("Festival Remix".into());
    let mut config = AutotagConfig::default();
    config.options.strictness = 1.;
    assert!(
        Tagger::match_candidates(&source, vec![candidate], &config)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn skip_tagged_recognizes_cuerithm_and_legacy_markers_without_network() {
    let (tagger, calls) = tagger(true, true, false);
    let mut config = AutotagConfig::default();
    config.options.skip_tagged = true;
    for marker in ["cuerithm_taggeddate", "1t_taggeddate"] {
        let mut source = input();
        source
            .media
            .tags
            .insert(marker.into(), "2026-01-01_AT".into());
        let result = tagger.match_track(&source, &config).unwrap();
        assert_eq!(result.status, MatchStatus::AlreadyTagged);
        assert!(result.matches.is_empty());
    }
    assert!(calls.lock().unwrap().is_empty());
}

#[test]
fn invalid_file_tag_limits_are_rejected_before_any_write() {
    let mut config = AutotagConfig::default();
    config.options.track_number_leading_zeroes = usize::MAX;
    assert!(config.validate().is_err());
    config = AutotagConfig::default();
    config.options.id3_comm_lang = Some("invalid".into());
    assert!(config.validate().is_err());
    config.options.id3_comm_lang = Some("eng".into());
    assert!(config.validate().is_ok());
}
