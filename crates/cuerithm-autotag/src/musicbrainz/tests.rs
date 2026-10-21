use super::wire;
use crate::*;
use serde_json::{Value, json};
const RECORDING: &str = "11111111-1111-4111-8111-111111111111";
const RELEASE: &str = "22222222-2222-4222-8222-222222222222";
const TRACK: &str = "33333333-3333-4333-8333-333333333333";
const ARTIST: &str = "44444444-4444-4444-8444-444444444444";
fn credit() -> Value {
    json!({"name":"Credit Name","joinphrase":" feat. ","artist":{"id":ARTIST,"name":"Canonical Name"}})
}
fn search() -> Value {
    json!({"recordings":[{"id":RECORDING,"title":"Signal (Club Mix)","length":300000,"artist-credit":[credit()],"first-release-date":"1999","isrcs":["USAAA2100001","USAAA2100002"],"releases":[{"id":RELEASE,"title":"Chosen edition","date":"2021-07","status":"Official","country":"BE","release-group":{"id":"55555555-5555-4555-8555-555555555555","first-release-date":"1999-03"},"media":[{"position":2,"track-count":4,"track-offset":0,"format":"Vinyl","track":[{"id":TRACK,"title":"Signal (Club Mix)","number":"A1","length":300000}]}]}]}]})
}
#[test]
fn mapping_preserves_release_recording_credits_partial_dates_and_raw_positions() {
    let matches = wire::recordings(search()).unwrap();
    assert_eq!(matches.len(), 1);
    let m = &matches[0];
    m.validate().unwrap();
    let f = m.musicbrainz().unwrap();
    assert_eq!(f.scope, CatalogIdentityScope::ReleaseTrack);
    assert_eq!(f.recording_id.as_str(), RECORDING);
    assert_eq!(f.release_track_id.as_ref().unwrap().as_str(), TRACK);
    assert_eq!(
        f.selected_release_date,
        Some(TransferDate {
            year: 2021,
            month: Some(7),
            day: None
        })
    );
    assert_eq!(f.recording_first_release_date.as_ref().unwrap().year, 1999);
    assert_eq!(f.position.medium_position, Some(2));
    assert_eq!(f.position.track_position, Some(1));
    assert_eq!(f.position.printed_number.as_deref(), Some("A1"));
    assert_eq!(f.recording_credit[0].credited_name, "Credit Name");
    assert_eq!(f.recording_credit[0].name, "Canonical Name");
    assert_eq!(f.recording_credit[0].join_phrase, " feat. ");
    assert_eq!(f.isrcs.len(), 2);
    assert!(m.track.isrc.is_none());
    assert!(m.track.release_date.is_none());
    assert!(m.track.release_year.is_none());
    assert_eq!(f.cover.as_ref().unwrap().release_id.as_str(), RELEASE);
    assert!(m.track.other.is_empty());
    assert!(m.track.key.is_none() && m.track.bpm.is_none());
}
#[test]
fn recording_without_track_proof_never_claims_release_fields_or_artwork() {
    let mut value = search();
    value["recordings"][0]["releases"][0]["media"] = json!([]);
    let matches = wire::recordings(value).unwrap();
    let m = &matches[0];
    let f = m.musicbrainz().unwrap();
    assert_eq!(f.scope, CatalogIdentityScope::RecordingOnly);
    assert!(
        m.track.album.is_none()
            && m.track.release_id.is_none()
            && f.cover.is_none()
            && f.selected_release_date.is_none()
    );
}
#[test]
fn malformed_identifiers_or_dates_are_errors_instead_of_empty_success() {
    let mut value = search();
    value["recordings"][0]["id"] = json!("not-an-mbid");
    assert!(wire::recordings(value).is_err());
    let mut value = search();
    value["recordings"][0]["releases"][0]["date"] = json!("2021-02-31");
    assert!(wire::recordings(value).is_err());
    assert!(wire::recordings(json!({"unexpected":[]})).is_err());
    assert!(
        wire::recordings(json!({"recordings":[]}))
            .unwrap()
            .is_empty()
    );
}
#[test]
fn selected_release_expansion_refuses_another_recording_or_track() {
    let selected = wire::recordings(search()).unwrap().remove(0);
    let value = json!({"id":RELEASE,"title":"Chosen edition","media":[{"position":2,"tracks":[{"id":TRACK,"position":1,"number":"A1","recording":{"id":"99999999-9999-4999-8999-999999999999","title":"Another recording"}}]}]});
    assert!(wire::release(value, &selected).is_err());
}
#[test]
fn legacy_match_json_remains_compatible_without_a_facts_slot() {
    let legacy = onetagger_tagger::TrackMatch::new(
        0.8,
        CatalogTrack {
            platform: "beatport".into(),
            title: "Legacy".into(),
            ..Default::default()
        },
    );
    let owned = CatalogMatch::from(legacy.clone());
    assert_eq!(
        serde_json::to_value(&legacy).unwrap(),
        serde_json::to_value(&owned).unwrap()
    );
    assert_eq!(
        serde_json::from_value::<CatalogMatch>(serde_json::to_value(&legacy).unwrap()).unwrap(),
        owned
    );
}

fn media_fixture() -> (
    tempfile::TempDir,
    cuerithm_media::MediaRuntime,
    cuerithm_media::PreparedAudio,
    TagInput,
) {
    let d = tempfile::tempdir().unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = d.path().join("source.flac");
    std::fs::copy(
        repo.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
        &source,
    )
    .unwrap();
    let rt =
        cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin")).unwrap();
    let audio = rt.decode(&source, d.path()).unwrap();
    let input = TagInput {
        path: source,
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    (d, rt, audio, input)
}
#[test]
fn catalog_candidate_partial_month_date_is_written_without_a_fake_day() {
    let (d, rt, audio, input) = media_fixture();
    let candidate = wire::recordings(search()).unwrap().remove(0);
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["musicbrainz".into()];
    config.options.tags = vec![SupportedTag::ReleaseDate];
    config.options.overwrite = true;
    config.options.id3v24 = true;
    config.write_energy = false;
    let outcome = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![candidate],
        attempted: vec![ProviderId::Musicbrainz],
        warnings: vec![],
    };
    let staged = TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default())
        .unwrap()
        .stage_prepared(&rt, &audio, d.path())
        .unwrap();
    let native = read_metadata_snapshot(&staged.path, &staged.metadata).unwrap();
    assert_eq!(
        native.release_date,
        Some(TransferDate {
            year: 2021,
            month: Some(7),
            day: None
        })
    );
    assert_eq!(
        cuerithm_media::file_sha256(&audio.source_path).unwrap(),
        audio.source_sha256
    );
}
#[test]
fn catalog_candidate_musicbrainz_ids_use_typed_standard_native_bindings() {
    let (d, rt, audio, input) = media_fixture();
    let candidate = wire::recordings(search()).unwrap().remove(0);
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["musicbrainz".into()];
    config.options.tags = vec![SupportedTag::TrackId, SupportedTag::ReleaseId];
    config.options.overwrite = true;
    config.write_energy = false;
    let outcome = TagMatchOutcome {
        status: MatchStatus::Matched,
        matches: vec![candidate],
        attempted: vec![ProviderId::Musicbrainz],
        warnings: vec![],
    };
    let staged = TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default())
        .unwrap()
        .stage_prepared(&rt, &audio, d.path())
        .unwrap();
    let tag = onetagger_tag::Tag::load_file(&staged.path, true).unwrap();
    assert_eq!(
        tag.tag().get_raw("MUSICBRAINZ_TRACKID"),
        Some(vec![RECORDING.into()])
    );
    assert_eq!(
        tag.tag().get_raw("MUSICBRAINZ_ALBUMID"),
        Some(vec![RELEASE.into()])
    );
    assert_eq!(
        tag.tag().get_raw("MUSICBRAINZ_RELEASETRACKID"),
        Some(vec![TRACK.into()])
    );
    assert!(tag.tag().get_raw("MUSICBRAINZ_TRACK_ID").is_none());
}
#[test]
fn catalog_candidate_source_terms_block_sonovault_key_bpm_but_keep_measured_analysis_independent() {
    for measured in [false, true] {
        let (d, rt, audio, input) = media_fixture();
        let before = read_metadata_snapshot(&input.path, &input.media).unwrap();
        let candidate = CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "sonovault".into(),
                title: "Permitted title".into(),
                bpm: Some(313),
                key: Some("F#m".into()),
                ..Default::default()
            },
        );
        let mut config = AutotagConfig::default();
        config.options.platforms = vec!["sonovault".into()];
        config.options.tags = vec![SupportedTag::Title, SupportedTag::BPM, SupportedTag::Key];
        config.options.overwrite = true;
        config.write_energy = false;
        config.analysis_tag_source = if measured {
            AnalysisTagSource::Measured
        } else {
            AnalysisTagSource::Catalog
        };
        let outcome = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches: vec![candidate],
            attempted: vec![ProviderId::Sonovault],
            warnings: vec![],
        };
        let staged = TagWritePlan::build(
            input,
            &outcome,
            &config,
            &AnalysisTags {
                bpm: Some(123.45),
                standard_key: Some("Am".into()),
                ..Default::default()
            },
        )
        .unwrap()
        .stage_prepared(&rt, &audio, d.path())
        .unwrap();
        let after = read_metadata_snapshot(&staged.path, &staged.metadata).unwrap();
        assert_eq!(
            after.text.get(&MetadataTextField::Title),
            Some(&vec!["Permitted title".into()])
        );
        if measured {
            assert_eq!(after.bpm, Some(123.45));
            assert_eq!(
                after.text.get(&MetadataTextField::Key),
                Some(&vec!["Am".into()])
            );
        } else {
            assert_eq!(
                after.bpm, before.bpm,
                "restricted provider BPM must not be written"
            );
            assert_eq!(
                after.text.get(&MetadataTextField::Key),
                before.text.get(&MetadataTextField::Key)
            );
        }
    }
}

#[test]
fn catalog_candidate_typed_date_and_ids_preserve_pcm_in_every_supported_container() {
    let d = tempfile::tempdir().unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let archive = std::fs::File::open(repo.join("tools/fixtures/media.tar.gz")).unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(d.path())
        .unwrap();
    let cases: Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let rt =
        cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin")).unwrap();
    for row in cases.as_array().unwrap() {
        let source = d.path().join(row["file"].as_str().unwrap());
        let audio = rt.decode(&source, d.path()).unwrap();
        let input = TagInput {
            path: source.clone(),
            source_sha256: audio.source_sha256.clone(),
            media: audio.info.clone(),
            duration_seconds: audio.clock.duration_seconds(),
        };
        let mut config = AutotagConfig::default();
        config.options.platforms = vec!["musicbrainz".into()];
        config.options.tags = vec![
            SupportedTag::ReleaseDate,
            SupportedTag::PublishDate,
            SupportedTag::TrackId,
            SupportedTag::ReleaseId,
        ];
        config.options.overwrite = true;
        config.options.id3v24 = true;
        config.required_tags = config.options.tags.clone();
        config.write_energy = false;
        let outcome = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches: wire::recordings(search()).unwrap(),
            attempted: vec![ProviderId::Musicbrainz],
            warnings: vec![],
        };
        let staged = TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default())
            .unwrap()
            .stage_prepared(&rt, &audio, d.path())
            .unwrap_or_else(|e| panic!("{}: {e:#}", source.display()));
        let fields: Vec<crate::FieldWriteOutcome> =
            serde_json::from_value(staged.provenance["fields"].clone()).unwrap();
        for date_field in [SupportedTag::ReleaseDate, SupportedTag::PublishDate] {
            assert!(
                fields.iter().any(|field| {
                    field.field == date_field
                        && field.required
                        && field.status == crate::FieldWriteStatus::Written
                }),
                "{}: {date_field:?} is a written required field",
                source.display()
            );
        }
        let after = rt.decode(&staged.path, d.path()).unwrap();
        assert_eq!(audio.frames, after.frames, "{}", source.display());
        assert_eq!(
            audio.pcm_sha256().unwrap(),
            after.pcm_sha256().unwrap(),
            "{}",
            source.display()
        );
        assert_eq!(
            cuerithm_media::file_sha256(&source).unwrap(),
            audio.source_sha256
        );
        let metadata = read_metadata_snapshot(&staged.path, &staged.metadata).unwrap();
        assert_eq!(
            metadata.release_date,
            Some(TransferDate {
                year: 2021,
                month: Some(7),
                day: None
            }),
            "{}",
            source.display()
        );
        assert_eq!(
            metadata.original_date,
            Some(TransferDate {
                year: 1999,
                month: None,
                day: None,
            }),
            "{}",
            source.display()
        );
    }
}

#[test]
fn backend_and_selected_cover_share_identified_paced_runtime_without_credentials() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, Ordering},
    };
    #[derive(Default)]
    struct Clock(AtomicI64);
    impl CatalogClock for Clock {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::Relaxed)
        }
        fn wait_until(
            &self,
            until: i64,
            cancel: &Arc<AtomicBool>,
            _: Option<&cuerithm_credentials::Revocation>,
        ) -> anyhow::Result<()> {
            anyhow::ensure!(!cancel.load(Ordering::Relaxed), "cancelled");
            self.0.fetch_max(until, Ordering::Relaxed);
            Ok(())
        }
    }
    type RecordedStart = (CatalogService, String, i64, Vec<(String, String)>);
    struct Transport {
        clock: Arc<Clock>,
        starts: Mutex<Vec<RecordedStart>>,
        png: Vec<u8>,
    }
    impl CatalogTransport for Transport {
        fn execute(
            &self,
            request: CatalogHttpRequest,
            _: &Arc<AtomicBool>,
            _: Option<&cuerithm_credentials::Revocation>,
        ) -> anyhow::Result<CatalogHttpResponse> {
            self.starts.lock().unwrap().push((
                request.service,
                request.url.clone(),
                self.clock.now_ms(),
                request.headers,
            ));
            let mut recording = search()["recordings"][0].clone();
            recording.as_object_mut().unwrap().remove("releases");
            let mut release = search()["recordings"][0]["releases"][0].clone();
            let mut track = release["media"][0]["track"][0].clone();
            track["position"] = json!(1);
            track["recording"] = recording.clone();
            release["media"][0].as_object_mut().unwrap().remove("track");
            release["media"][0]["tracks"] = json!([track]);
            let body = if request.service == CatalogService::CoverArtArchive {
                if request.url.ends_with("/123-500") {
                    return Ok(CatalogHttpResponse {
                        status: 200,
                        body: self.png.clone(),
                        headers: vec![],
                    });
                }
                json!({"release":format!("https://musicbrainz.org/release/{RELEASE}"),"images":[{"id":123,"approved":true,"front":true,"types":["Front"],"image":"https://untrusted.invalid/must-not-be-fetched.png"}]})
            } else if request.url.contains("/recording/") {
                recording
            } else if request.url.contains(&format!("/release/{RELEASE}")) {
                release
            } else {
                json!({"releases":[release]})
            };
            Ok(CatalogHttpResponse {
                status: 200,
                body: serde_json::to_vec(&body)?,
                headers: vec![],
            })
        }
    }
    let clock = Arc::new(Clock::default());
    let mut png = std::io::Cursor::new(Vec::new());
    ::image::DynamicImage::new_rgb8(1, 1)
        .write_to(&mut png, ::image::ImageFormat::Png)
        .unwrap();
    let transport = Arc::new(Transport {
        clock: clock.clone(),
        starts: Mutex::new(vec![]),
        png: png.into_inner(),
    });
    let runtime = Arc::new(
        CatalogRuntime::development()
            .unwrap()
            .with_clock(clock)
            .with_transport(transport.clone()),
    );
    let backend = MusicBrainzBackend::new(runtime.clone());
    let cancel = Arc::new(AtomicBool::new(false));
    let mut candidate = backend
        .recording_candidates(&Mbid::new(RECORDING).unwrap(), &cancel)
        .unwrap()
        .remove(0);
    assert_eq!(
        candidate.musicbrainz().unwrap().scope,
        CatalogIdentityScope::ReleaseTrack
    );
    backend
        .extend_candidate_cancellable(&mut candidate, &AutotagConfig::default(), &cancel)
        .unwrap();
    let cover = fetch_musicbrainz_artwork(
        &runtime,
        candidate.musicbrainz().unwrap().cover.as_ref().unwrap(),
        &cuerithm_media::CancellationToken::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(cover.reference.image_id.as_deref(), Some("123"));
    assert_eq!(cover.provenance.types, vec!["Front"]);
    let starts = transport.starts.lock().unwrap();
    assert_eq!(starts.len(), 5);
    let mb: Vec<_> = starts
        .iter()
        .filter(|row| row.0 == CatalogService::MusicBrainz)
        .collect();
    assert!(mb.windows(2).all(|rows| rows[1].2 - rows[0].2 >= 1000));
    for (_, url, _, headers) in starts.iter() {
        assert!(!url.contains("untrusted.invalid"));
        assert!(headers.iter().any(|(key,value)|key.eq_ignore_ascii_case("user-agent")&&value=="CueRithm/development (https://github.com/johnneerdael/Cuerithm/issues)"));
        assert!(
            !headers
                .iter()
                .any(|(key, _)| key.eq_ignore_ascii_case("authorization")
                    || key.eq_ignore_ascii_case("x-api-key"))
        );
    }
}

#[test]
fn overlapping_partial_release_dates_do_not_claim_an_oldest_exact_edition() {
    use std::sync::Arc;
    struct Catalog(Vec<CatalogMatch>);
    impl ProviderBackend for Catalog {
        fn id(&self) -> ProviderId {
            ProviderId::Musicbrainz
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
            Ok(self.0.clone())
        }
    }
    let mut first = wire::recordings(search()).unwrap().remove(0);
    first.musicbrainz().unwrap();
    if let Some(CatalogFacts::MusicBrainz(f)) = &mut first.facts {
        f.selected_release_date = Some(TransferDate {
            year: 2021,
            month: None,
            day: None,
        });
    }
    first.track.release_year = Some(2021);
    let mut second = wire::recordings(search()).unwrap().remove(0);
    let id = Mbid::new("66666666-6666-4666-8666-666666666666").unwrap();
    second.track.release_id = Some(id.to_string());
    if let Some(CatalogFacts::MusicBrainz(f)) = &mut second.facts {
        f.release_id = Some(id.clone());
        f.cover.as_mut().unwrap().release_id = id.clone();
        f.release_track_id = Some(Mbid::new("77777777-7777-4777-8777-777777777777").unwrap());
    }
    second.track.art = Some(format!(
        "https://coverartarchive.org/release/{id}/front-500"
    ));
    let input = TagInput {
        path: "fixture.flac".into(),
        source_sha256: "0".repeat(64),
        duration_seconds: 300.,
        media: cuerithm_media::MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("title".into(), "Signal (Club Mix)".into()),
                ("artist".into(), "Credit Name".into()),
            ]
            .into(),
        },
    };
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["musicbrainz".into()];
    config.options.parse_filename = false;
    config.options.multiple_matches = onetagger_tagger::MultipleMatchesSort::Oldest;
    let tagger = Tagger::with_backends(vec![Arc::new(Catalog(vec![first, second]))]).unwrap();
    let outcome = tagger.match_track(&input, &config).unwrap();
    assert_eq!(
        outcome.status,
        MatchStatus::Ambiguous,
        "a year-only date cannot prove it precedes a July edition in that same year"
    );
    assert_eq!(outcome.matches.len(), 2);
}

#[test]
fn native_musicbrainz_identifier_aliases_preserve_lookup_and_conflict_guards() {
    let (_, _, _, mut input) = media_fixture();
    let candidate = wire::recordings(search()).unwrap().remove(0);
    for key in [
        "MusicBrainz Track Id",
        "MusicBrainz/Track Id",
        "MUSICBRAINZ_TRACKID",
    ] {
        input.media.tags.clear();
        input.media.tags.insert(key.into(), RECORDING.into());
        assert_eq!(
            super::source_id(&input, &["musicbrainz_trackid"])
                .unwrap()
                .map(|id| id.to_string()),
            Some(RECORDING.into()),
            "{key}"
        );
        input
            .media
            .tags
            .insert(key.into(), "99999999-9999-4999-8999-999999999999".into());
        assert!(
            crate::evidence::match_identity_conflicts(
                &input,
                &candidate,
                &AutotagConfig::default()
            )
            .contains(&"recording_id".into()),
            "{key}"
        );
    }
}

#[test]
fn catalog_date_precedence_follows_provider_order_across_typed_and_legacy_values() {
    let typed = wire::recordings(search()).unwrap().remove(0);
    let mut discogs = CatalogMatch::new(
        1.,
        CatalogTrack {
            platform: "discogs".into(),
            title: "Signal".into(),
            release_id: Some("release2".into()),
            ..Default::default()
        },
    );
    discogs.facts = Some(CatalogFacts::Source(SourceCatalogFacts {
        schema_version: 1,
        provider: "discogs".into(),
        release_date: Some(TransferDate {
            year: 2022,
            month: Some(8),
            day: None,
        }),
        release_id: discogs.release_id.clone(),
        raw_position: None,
        identities: vec![],
        links: vec![],
    }));
    let legacy = CatalogMatch::new(
        1.,
        CatalogTrack {
            platform: "beatport".into(),
            title: "Signal".into(),
            release_date: chrono::NaiveDate::from_ymd_opt(2020, 3, 4),
            ..Default::default()
        },
    );
    for (matches, overwrite, only_year, year, month, day) in [
        (
            vec![typed.clone(), discogs.clone()],
            false,
            false,
            2021,
            Some(7),
            None,
        ),
        (
            vec![typed.clone(), legacy.clone()],
            false,
            false,
            2021,
            Some(7),
            None,
        ),
        (
            vec![legacy.clone(), typed.clone()],
            false,
            false,
            2020,
            Some(3),
            Some(4),
        ),
        (
            vec![typed.clone(), legacy.clone()],
            true,
            false,
            2020,
            Some(3),
            Some(4),
        ),
        (
            vec![typed.clone(), discogs.clone()],
            false,
            true,
            2021,
            None,
            None,
        ),
        (
            vec![legacy.clone(), typed.clone()],
            true,
            true,
            2021,
            None,
            None,
        ),
    ] {
        let (d, rt, audio, input) = media_fixture();
        let mut config = AutotagConfig::default();
        config.options.tags = vec![SupportedTag::ReleaseDate];
        config.options.overwrite = overwrite;
        config.options.only_year = only_year;
        config.write_energy = false;
        let outcome = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches,
            attempted: vec![],
            warnings: vec![],
        };
        let staged = TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default())
            .unwrap()
            .stage_prepared(&rt, &audio, d.path())
            .unwrap();
        let native = read_metadata_snapshot(&staged.path, &staged.metadata).unwrap();
        assert_eq!(
            native.release_date,
            Some(TransferDate { year, month, day }),
            "overwrite={overwrite},only_year={only_year},providers={:?}",
            outcome
                .matches
                .iter()
                .map(|m| &m.platform)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn equal_full_release_dates_do_not_choose_an_edition_by_identifier_order() {
    let mut first = wire::recordings(search()).unwrap().remove(0);
    let mut second = first.clone();
    let id = Mbid::new("66666666-6666-4666-8666-666666666666").unwrap();
    second.track.release_id = Some(id.to_string());
    if let Some(CatalogFacts::MusicBrainz(f)) = &mut second.facts {
        f.release_id = Some(id.clone());
        f.cover.as_mut().unwrap().release_id = id;
        f.release_track_id = Some(Mbid::new("77777777-7777-4777-8777-777777777777").unwrap());
    }
    for m in [&mut first, &mut second] {
        if let Some(CatalogFacts::MusicBrainz(f)) = &mut m.facts {
            f.selected_release_date = Some(TransferDate {
                year: 2021,
                month: Some(7),
                day: Some(12),
            });
        }
        m.track.release_date = chrono::NaiveDate::from_ymd_opt(2021, 7, 12);
    }
    let input = TagInput {
        path: "fixture.flac".into(),
        source_sha256: "0".repeat(64),
        duration_seconds: 300.,
        media: cuerithm_media::MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: Default::default(),
        },
    };
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["musicbrainz".into()];
    for sort in [
        onetagger_tagger::MultipleMatchesSort::Oldest,
        onetagger_tagger::MultipleMatchesSort::Newest,
    ] {
        config.options.multiple_matches = sort;
        assert!(
            crate::evidence::candidate_conflict(&first, &second, &config),
            "same full date cannot establish edition precedence"
        );
        let _ = crate::evidence::match_identity_conflicts(&input, &first, &config);
    }
    if let Some(CatalogFacts::MusicBrainz(f)) = &mut second.facts {
        f.selected_release_date.as_mut().unwrap().day = Some(13);
    }
    assert!(
        !crate::evidence::candidate_conflict(&first, &second, &config),
        "disjoint full dates retain existing ordering"
    );
}
