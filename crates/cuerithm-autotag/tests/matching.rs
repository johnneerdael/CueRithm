use anyhow::Result;
use cuerithm_autotag::{
    AutotagConfig, CatalogMatch, CatalogTrack, MatchStatus, ProviderBackend, ProviderId, TagInput,
    TagMatchOutcome, Tagger,
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

#[test]
fn cleaned_similarity_never_overrides_explicit_edition_or_recording_conflicts() {
    let mut config = AutotagConfig::default();
    config.options.strictness = 0.5;
    for (wanted, wrong) in [
        ("Club Mix", "Dub Mix"),
        ("Extended Mix", "Radio Edit"),
        ("Instrumental", "Vocal Mix"),
    ] {
        let mut source = input();
        source
            .media
            .tags
            .insert("title".into(), format!("Signal ({wanted})"));
        let mut candidate = track(ProviderId::Beatport, "Signal");
        candidate.version = Some(wrong.into());
        assert!(
            Tagger::match_candidates(&source, vec![candidate], &config)
                .unwrap()
                .is_empty(),
            "{wanted} vs {wrong}"
        );
    }
    let mut source = input();
    source
        .media
        .tags
        .insert("isrc".into(), "GBABC2500001".into());
    let mut candidate = track(ProviderId::Beatport, "Signal");
    candidate.isrc = Some("GBABC2500002".into());
    assert!(
        Tagger::match_candidates(&source, vec![candidate], &config)
            .unwrap()
            .is_empty()
    );
}
#[test]
fn equally_good_distinct_candidates_require_review_in_stable_order() {
    struct Ties;
    impl ProviderBackend for Ties {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
            Ok(["2", "1"]
                .into_iter()
                .map(|id| {
                    let mut t = track(ProviderId::Beatport, "Signal");
                    t.track_id = Some(id.into());
                    CatalogMatch::new(1., t)
                })
                .collect())
        }
    }
    let tagger = Tagger::with_backends(vec![Arc::new(Ties)]).unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["beatport".into()];
    let outcome = tagger.match_track(&input(), &config).unwrap();
    assert_ne!(outcome.status, MatchStatus::Matched);
    assert_eq!(
        outcome
            .matches
            .iter()
            .map(|m| m.track.track_id.as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["1", "2"]
    );
}

#[test]
fn provider_high_similarity_cannot_bypass_identity_checks() {
    struct WrongVersion;
    impl ProviderBackend for WrongVersion {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
            let mut t = track(ProviderId::Beatport, "Signal");
            t.version = Some("Dub Mix".into());
            Ok(vec![CatalogMatch::new(1., t)])
        }
    }
    let tagger = Tagger::with_backends(vec![Arc::new(WrongVersion)]).unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["beatport".into()];
    let mut source = input();
    source
        .media
        .tags
        .insert("title".into(), "Signal (Club Mix)".into());
    assert_ne!(
        tagger.match_track(&source, &config).unwrap().status,
        MatchStatus::Matched
    );
}

#[test]
fn candidate_provenance_is_bounded_and_bound_to_source_and_config() {
    let mut output = TagMatchOutcome {
        status: MatchStatus::Ambiguous,
        matches: (0..40)
            .map(|n| {
                let mut t = track(ProviderId::Beatport, &"Long title".repeat(2000));
                t.track_id = Some(n.to_string());
                CatalogMatch::new(1., t)
            })
            .collect(),
        attempted: vec![],
        warnings: vec![],
    };
    let config = AutotagConfig::default();
    let source = input();
    let evidence = output.provenance(&source, &config).unwrap();
    assert_eq!(evidence.candidates.len(), 10);
    assert!(evidence.truncated);
    assert_eq!(evidence.source_sha256, source.source_sha256);
    assert!(serde_json::to_vec(&evidence).unwrap().len() < 100_000);
    let decoded: AutotagConfig =
        serde_json::from_value(serde_json::to_value(&config).unwrap()).unwrap();
    assert_eq!(
        evidence.config_sha256,
        output.provenance(&source, &decoded).unwrap().config_sha256
    );
    output.matches[0].track.duration = Duration::ZERO;
    assert_eq!(
        output.provenance(&source, &config).unwrap().candidates[0].duration_seconds,
        None
    );
}

#[test]
fn nonfinite_candidate_evidence_is_rejected_instead_of_serialized_as_null() {
    let output = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(
            f64::NAN,
            track(ProviderId::Beatport, "Signal"),
        )],
        attempted: vec![],
        warnings: vec![],
    };
    assert!(
        output
            .provenance(&input(), &AutotagConfig::default())
            .is_err()
    );
}

#[test]
fn provider_other_tags_cannot_bypass_explicit_or_native_record_protection() {
    use cuerithm_autotag::{AnalysisTags, FrameName, SupportedTag, TagWritePlan};
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::OtherTags];
    for key in [
        "GEOB",
        "PRIV",
        "POPM",
        "COMM",
        "COMMENT",
        "ITUNESADVISORY",
        "rtng",
        "TITLE",
        "ARTIST",
        "BEATPORT_TRACK_ID",
    ] {
        let mut candidate = track(ProviderId::Beatport, "Signal");
        candidate.other = vec![(FrameName::same(key), vec!["clobber".into()])];
        let output = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches: vec![CatalogMatch::new(1., candidate)],
            attempted: vec![],
            warnings: vec![],
        };
        assert!(
            TagWritePlan::build(input(), &output, &config, &AnalysisTags::default()).is_err(),
            "{key}"
        );
    }
}

#[test]
fn mp4_provider_other_tags_cannot_alias_protected_fields_through_extra_components() {
    use cuerithm_autotag::{AnalysisTags, FrameName, SupportedTag, TagWritePlan};
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::OtherTags];
    for key in [
        "com.apple.iTunes:LABEL:ignored",
        "iTunes:LABEL",
        "----:com.apple.iTunes:LABEL:ignored",
        "com.apple.iTunes:RATING:ignored",
        "com.apple.iTunes:SERATO_MARKERS:ignored",
        "com.apple.iTunes:BEATPORT_TRACK_ID:ignored",
    ] {
        let mut source = input();
        source.media.codec = "alac".into();
        let mut candidate = track(ProviderId::Beatport, "Signal");
        candidate.other = vec![(FrameName::same(key), vec!["clobber".into()])];
        let output = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches: vec![CatalogMatch::new(1., candidate)],
            attempted: vec![],
            warnings: vec![],
        };
        assert!(
            TagWritePlan::build(source, &output, &config, &AnalysisTags::default()).is_err(),
            "{key}"
        );
    }
}

#[test]
fn mp4_native_comment_lyrics_description_artwork_and_rating_atoms_are_protected() {
    use cuerithm_autotag::{AnalysisTags, FrameName, SupportedTag, TagWritePlan};
    for key in ["©lyr", "©cmt", "desc", "covr", "rtng"] {
        let output = |candidate| TagMatchOutcome {
            status: MatchStatus::Matched,
            matches: vec![CatalogMatch::new(1., candidate)],
            attempted: vec![],
            warnings: vec![],
        };
        let mut source = input();
        source.media.codec = "alac".into();
        let mut other = AutotagConfig::default();
        other.options.tags = vec![SupportedTag::OtherTags];
        let mut candidate = track(ProviderId::Beatport, "Signal");
        candidate.other = vec![(FrameName::same(key), vec!["clobber".into()])];
        assert!(
            TagWritePlan::build(
                source.clone(),
                &output(candidate),
                &other,
                &AnalysisTags::default()
            )
            .is_err(),
            "other tags {key}"
        );
        let mut style = AutotagConfig::default();
        style.options.tags = vec![SupportedTag::Style];
        style.options.styles_options = onetagger_tagger::StylesOptions::CustomTag;
        style.options.styles_custom_tag = Some(FrameName::same(key));
        assert!(
            TagWritePlan::build(
                source,
                &output(track(ProviderId::Beatport, "Signal")),
                &style,
                &AnalysisTags::default()
            )
            .is_err(),
            "style {key}"
        );
    }
}

#[test]
fn mp4_provider_other_tags_keep_unrelated_freeform_names() {
    use cuerithm_autotag::{AnalysisTags, FrameName, SupportedTag, TagWritePlan};
    let mut config = AutotagConfig::default();
    config.options.tags = vec![SupportedTag::OtherTags];
    let mut source = input();
    source.media.codec = "alac".into();
    let mut candidate = track(ProviderId::Beatport, "Signal");
    candidate.other = vec![(
        FrameName::same("com.apple.iTunes:CUERITHM_NOTE"),
        vec!["kept".into()],
    )];
    let output = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![CatalogMatch::new(1., candidate)],
        attempted: vec![],
        warnings: vec![],
    };
    TagWritePlan::build(source, &output, &config, &AnalysisTags::default()).unwrap();
}

#[test]
fn explicit_release_preference_can_choose_reissues_but_never_conflicting_recordings() {
    struct Reissues {
        conflict: bool,
    }
    impl ProviderBackend for Reissues {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
            Ok([(2020, "older"), (2024, "newer")]
                .into_iter()
                .map(|(year, id)| {
                    let mut t = track(ProviderId::Beatport, "Signal");
                    t.track_id = Some(id.into());
                    t.release_id = Some(format!("release-{id}"));
                    t.isrc = Some(
                        if self.conflict && year == 2020 {
                            "GBABC2000002"
                        } else {
                            "GBABC2000001"
                        }
                        .into(),
                    );
                    t.version = Some("Club Mix".into());
                    t.release_date = chrono::NaiveDate::from_ymd_opt(year, 1, 1);
                    CatalogMatch::new(1., t)
                })
                .collect())
        }
    }
    for (preference, id) in [
        (onetagger_tagger::MultipleMatchesSort::Oldest, "older"),
        (onetagger_tagger::MultipleMatchesSort::Newest, "newer"),
    ] {
        let mut config = AutotagConfig::default();
        config.options.platforms = vec!["beatport".into()];
        config.options.multiple_matches = preference;
        let tagger = Tagger::with_backends(vec![Arc::new(Reissues { conflict: false })]).unwrap();
        let output = tagger.match_track(&input(), &config).unwrap();
        assert_eq!(output.status, MatchStatus::Matched);
        assert_eq!(output.matches[0].track.track_id.as_deref(), Some(id));
        let tagger = Tagger::with_backends(vec![Arc::new(Reissues { conflict: true })]).unwrap();
        assert_eq!(
            tagger.match_track(&input(), &config).unwrap().status,
            MatchStatus::Ambiguous
        );
    }
}

#[test]
fn custom_style_binding_cannot_overwrite_unselected_native_fields() {
    use cuerithm_autotag::{AnalysisTags, FrameName, SupportedTag, TagWritePlan};
    for (codec, key) in [
        ("flac", "TITLE"),
        ("mp3", "TIT2"),
        ("flac", "GENRE"),
        ("alac", "©day"),
        ("mp3", "TYER"),
        ("flac", "CUERITHM_TAGGEDDATE"),
        // The MP4 writer keeps only the first two `:` components of a freeform key.
        ("alac", "com.apple.iTunes:LABEL:ignored"),
        ("alac", "iTunes:LABEL"),
        ("alac", "com.apple.iTunes:RATING:ignored"),
    ] {
        let mut config = AutotagConfig::default();
        config.options.tags = vec![SupportedTag::Style];
        config.options.styles_options = onetagger_tagger::StylesOptions::CustomTag;
        config.options.styles_custom_tag = Some(FrameName::same(key));
        let mut source = input();
        source.media.codec = codec.into();
        let output = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches: vec![CatalogMatch::new(1., track(ProviderId::Beatport, "Signal"))],
            attempted: vec![],
            warnings: vec![],
        };
        assert!(
            TagWritePlan::build(source, &output, &config, &AnalysisTags::default()).is_err(),
            "{codec} {key}"
        );
    }
}
#[test]
fn multiple_providers_cannot_merge_conflicting_known_recordings() {
    struct Recording {
        provider: ProviderId,
        version: &'static str,
        isrc: &'static str,
    }
    impl ProviderBackend for Recording {
        fn id(&self) -> ProviderId {
            self.provider
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
            let mut t = track(self.provider, "Signal");
            t.version = Some(self.version.into());
            t.isrc = Some(self.isrc.into());
            Ok(vec![CatalogMatch::new(1., t)])
        }
    }
    let tagger = Tagger::with_backends(vec![
        Arc::new(Recording {
            provider: ProviderId::Beatport,
            version: "Club Mix",
            isrc: "GBABC2000001",
        }),
        Arc::new(Recording {
            provider: ProviderId::Traxsource,
            version: "Radio Edit",
            isrc: "GBABC2000002",
        }),
    ])
    .unwrap();
    let mut config = AutotagConfig::default();
    config.options.multiplatform = true;
    let output = tagger.match_track(&input(), &config).unwrap();
    assert_eq!(output.status, MatchStatus::Ambiguous);
    assert_eq!(output.matches.len(), 2);
}
#[test]
fn featured_artist_annotation_is_not_invented_edition_evidence() {
    let mut source = input();
    source
        .media
        .tags
        .insert("title".into(), "Signal (feat. Guest Artist)".into());
    let mut config = AutotagConfig::default();
    config.options.strictness = 0.5;
    let mut candidate = track(ProviderId::Beatport, "Signal (feat. Guest Artist)");
    candidate.version = Some("Original Mix".into());
    assert_eq!(
        Tagger::match_candidates(&source, vec![candidate], &config)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn actual_ambiguous_pipeline_retains_bounded_overflow_evidence() {
    struct Many;
    impl ProviderBackend for Many {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
            Ok((0..40)
                .map(|id| {
                    let mut t = track(ProviderId::Beatport, "Signal");
                    t.track_id = Some(format!("{id:02}"));
                    CatalogMatch::new(1., t)
                })
                .collect())
        }
    }
    let tagger = Tagger::with_backends(vec![Arc::new(Many)]).unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["beatport".into()];
    let outcome = tagger.match_track(&input(), &config).unwrap();
    let evidence = outcome.provenance(&input(), &config).unwrap();
    assert_eq!(outcome.matches.len(), 11);
    assert_eq!(evidence.candidates.len(), 10);
    assert!(evidence.truncated);
}
