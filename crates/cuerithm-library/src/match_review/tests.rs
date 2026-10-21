use crate::Library;
use cuerithm_autotag::{
    AutotagConfig, CatalogMatch, CatalogTrack, MatchStatus, TagInput, TagMatchOutcome,
};
use cuerithm_media::MediaInfo;
#[test]
fn bounded_complete_candidates_survive_restart_with_source_and_attempt_identity() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let db = root.join("data/library.sqlite");
    let lib = Library::open(&db).unwrap();
    let source = root.join("Artist - Signal.flac");
    std::fs::write(&source, b"frozen source fixture").unwrap();
    let mut settings = crate::PreparationSettings {
        autotag_enabled: true,
        ..Default::default()
    };
    let mut tagging = AutotagConfig::default();
    tagging.options.platforms = vec!["beatport".into()];
    settings.tagging = Some(tagging);
    lib.save_settings(settings).unwrap();
    let job = lib
        .enqueue(
            std::slice::from_ref(&source),
            lib.settings().unwrap().revision,
        )
        .unwrap()
        .remove(0);
    let run = lib.claim_next().unwrap().unwrap();
    let input = TagInput {
        path: source,
        source_sha256: run.source_sha256.clone(),
        duration_seconds: 300.,
        media: MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("artist".into(), "Artist".into()),
                ("title".into(), "Signal".into()),
            ]
            .into(),
        },
    };
    let matches = (0..12)
        .map(|i| {
            CatalogMatch::new(
                0.95,
                CatalogTrack {
                    platform: "beatport".into(),
                    track_id: Some(format!("candidate{i}")),
                    title: "Signal".into(),
                    artists: vec!["Artist".into()],
                    label: Some(format!("Label{i}")),
                    url: format!("https://www.beatport.com/track/signal/{i}"),
                    ..Default::default()
                },
            )
        })
        .collect();
    let outcome = TagMatchOutcome {
        status: MatchStatus::Ambiguous,
        matches,
        attempted: vec![],
        warnings: vec![],
    };
    lib.record_match_outcome(&run, &input, &outcome).unwrap();
    lib.fail_for_run(&run, "matching", "ambiguous").unwrap();
    let track = run.track_id.clone();
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let state = lib
        .track_match_review(&track)
        .unwrap()
        .expect("retained evidence must become reviewable");
    assert_eq!(state.job_id, Some(job));
    assert_eq!(state.attempt, run.attempt);
    assert_eq!(state.candidates.len(), 10);
    assert!(state.truncated);
    assert!(state.candidates.iter().all(|c| c.selectable));
    assert_eq!(state.source_sha256, input.source_sha256);
    assert!(
        state.candidates[0]
            .source_url
            .as_deref()
            .unwrap()
            .starts_with("https://www.beatport.com/")
    );
}

#[test]
fn keep_existing_once_is_previewed_then_queues_same_job_and_rejects_the_old_attempt() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = root.join("source.flac");
    std::fs::copy(
        repo.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &source,
    )
    .unwrap();
    let runtime =
        cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin")).unwrap();
    let mut tagging = AutotagConfig::default();
    tagging.options.platforms = vec!["beatport".into()];
    tagging.options.parse_filename = false;
    tagging.no_match = cuerithm_autotag::NoMatchPolicy::Attention;
    let mut settings = crate::PreparationSettings {
        autotag_enabled: true,
        tagging: Some(tagging),
        ..Default::default()
    };
    settings.engine.lexicon_energy = true;
    lib.save_settings(settings).unwrap();
    let job = lib
        .enqueue(
            std::slice::from_ref(&source),
            lib.settings().unwrap().revision,
        )
        .unwrap()
        .remove(0);
    let old = lib.claim_next().unwrap().unwrap();
    let input = TagInput {
        path: source.clone(),
        source_sha256: old.source_sha256.clone(),
        duration_seconds: 1.,
        media: runtime.probe(&source).unwrap(),
    };
    lib.record_match_outcome(
        &old,
        &input,
        &TagMatchOutcome {
            status: MatchStatus::NoMatch,
            matches: vec![],
            attempted: vec![],
            warnings: vec![],
        },
    )
    .unwrap();
    lib.fail_for_run(&old, "matching", "No match").unwrap();
    let state = lib.track_match_review(&old.track_id).unwrap().unwrap();
    assert!(state.can_keep_existing_once);
    let before = std::fs::read(&source).unwrap();
    let preview = lib
        .preview_match_review_with(
            &cuerithm_autotag::Tagger::with_backends(vec![]).unwrap(),
            &runtime,
            super::MatchReviewRequest {
                track_id: old.track_id.clone(),
                generation: state.generation,
                action: super::MatchReviewAction::KeepExistingOnce,
                candidate_id: None,
                fields: vec![],
                acknowledge_conflicts: false,
                allow_id3_upgrade: false,
            },
            &cuerithm_media::CancellationToken::default(),
        )
        .expect("Keep existing once must be a real explicit preview");
    assert!(preview.can_apply);
    assert_eq!(std::fs::read(&source).unwrap(), before);
    let queued = lib
        .apply_match_review_with(
            &runtime,
            &preview.id,
            &preview.revision,
            &cuerithm_media::CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(queued.job_id, Some(job.clone()));
    let current = lib.claim_next().unwrap().unwrap();
    assert_eq!(current.id, job);
    assert_eq!(current.attempt, old.attempt + 1);
    assert!(!Library::run_owns(&lib.conn().unwrap(), &old).unwrap());
    assert_eq!(
        lib.settings().unwrap().settings.tagging.unwrap().no_match,
        cuerithm_autotag::NoMatchPolicy::Attention
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);
    lib.fail_for_run(&current, "matching", "synthetic interruption before worker")
        .unwrap();
    lib.retry(&job, None).unwrap();
    let service = crate::PreparationService::start_with_tagger(
        lib.clone(),
        runtime,
        1,
        std::sync::Arc::new(cuerithm_autotag::Tagger::with_backends(vec![]).unwrap()),
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let state = lib
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == job)
            .unwrap();
        if state.state == "completed" {
            break;
        }
        if state.state == "attention" {
            panic!(
                "Protected Keep existing once must bypass another lookup failure: {:?}",
                state.error
            )
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Protected job did not settle"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    service.shutdown();
    let fresh = lib.track_match_review(&old.track_id).unwrap().unwrap();
    assert_ne!(
        fresh.generation, preview.generation,
        "source-bound old query becomes stale after our owned write"
    );
    assert!(!fresh.can_keep_existing_once);
}

#[test]
fn ready_without_a_job_has_an_explicit_current_config_query_and_no_preparation_side_effects() {
    use std::sync::{Arc, Mutex};
    struct Catalog(Mutex<Vec<Option<String>>>);
    impl cuerithm_autotag::ProviderBackend for Catalog {
        fn id(&self) -> cuerithm_autotag::ProviderId {
            cuerithm_autotag::ProviderId::Beatport
        }
        fn candidates(
            &self,
            _: &TagInput,
            config: &AutotagConfig,
        ) -> anyhow::Result<Vec<CatalogMatch>> {
            self.0.lock().unwrap().push(config.solver.endpoint.clone());
            Ok(vec![])
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = root.join("source.flac");
    std::fs::copy(
        repo.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
        &source,
    )
    .unwrap();
    let runtime =
        cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin")).unwrap();
    let mut tagging = AutotagConfig::default();
    tagging.options.platforms = vec!["beatport".into()];
    let mut settings = crate::PreparationSettings {
        autotag_enabled: true,
        tagging: Some(tagging),
        ..Default::default()
    };
    lib.save_settings(settings.clone()).unwrap();
    let job = lib
        .enqueue(
            std::slice::from_ref(&source),
            lib.settings().unwrap().revision,
        )
        .unwrap()
        .remove(0);
    let track: String = lib
        .conn()
        .unwrap()
        .query_row("SELECT track_id FROM jobs WHERE id=?1", [job], |r| r.get(0))
        .unwrap();
    lib.conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET state='ready',current_job=NULL WHERE id=?1",
            [&track],
        )
        .unwrap();
    let before = std::fs::read(&source).unwrap();
    let initial = lib
        .track_match_review(&track)
        .unwrap()
        .expect("Ready imported tracks without a job must expose an explicit query capability");
    assert!(initial.job_id.is_none());
    assert!(!initial.can_keep_existing_once);
    settings.tagging.as_mut().unwrap().solver.endpoint = Some("http://127.0.0.1:8191".into());
    lib.save_settings(settings).unwrap();
    let backend = Arc::new(Catalog(Mutex::new(vec![])));
    let tagger = cuerithm_autotag::Tagger::with_backends(vec![backend.clone()]).unwrap();
    let queried = lib
        .requery_matches_with(
            &tagger,
            &runtime,
            &track,
            &initial.generation,
            &cuerithm_media::CancellationToken::default(),
        )
        .unwrap();
    assert_ne!(queried.generation, initial.generation);
    assert_eq!(queried.status, "no_match");
    assert!(queried.job_id.is_none());
    assert_eq!(
        *backend.0.lock().unwrap(),
        vec![Some("http://127.0.0.1:8191".into())]
    );
    assert_eq!(lib.track(&track).unwrap().state, "ready");
    assert_eq!(lib.cue_revision_count(&track).unwrap(), 0);
    assert_eq!(std::fs::read(source).unwrap(), before);
    assert!(
        lib.requery_matches_with(
            &tagger,
            &runtime,
            &track,
            &initial.generation,
            &cuerithm_media::CancellationToken::default()
        )
        .is_err(),
        "old query tokens must stay invalid"
    );
}

fn wait_for_job(lib: &Library, id: &str, wanted: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let job = lib
            .jobs()
            .unwrap()
            .into_iter()
            .find(|j| j.id == id)
            .unwrap();
        if job.state == wanted {
            return;
        }
        if wanted == "completed" && job.state == "attention" {
            panic!("Preparation failed: {job:?}")
        }
        assert!(
            std::time::Instant::now() < deadline,
            "wanted{wanted}: {job:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
#[test]
fn keep_once_then_title_edit_retains_decision_and_dsp_through_required_naming() {
    use cuerithm_autotag::{MetadataTextField, TextEdit};
    use cuerithm_media::CancellationToken;
    use std::sync::Arc;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = root.join("source.flac");
    std::fs::copy(
        repo.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &source,
    )
    .unwrap();
    let runtime =
        cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin")).unwrap();
    let managed = root.join("managed");
    std::fs::create_dir(&managed).unwrap();
    let grant = lib.register_organization_root(managed.clone()).unwrap();
    let mut tagging = AutotagConfig::default();
    tagging.options.platforms = vec!["beatport".into()];
    tagging.options.parse_filename = false;
    tagging.no_match = cuerithm_autotag::NoMatchPolicy::Attention;
    let mut settings = crate::PreparationSettings {
        autotag_enabled: true,
        tagging: Some(tagging),
        organization: crate::OrganizationSettings {
            mode: crate::OrganizationSettingMode::Move,
            pattern: "{title}".into(),
            destination_root_id: Some(grant.id),
            ..Default::default()
        },
        ..Default::default()
    };
    settings.engine.lexicon_energy = true;
    lib.save_settings(settings).unwrap();
    let job = lib
        .enqueue(
            std::slice::from_ref(&source),
            lib.settings().unwrap().revision,
        )
        .unwrap()
        .remove(0);
    let old = lib.claim_next().unwrap().unwrap();
    let input = TagInput {
        path: source.clone(),
        source_sha256: old.source_sha256.clone(),
        media: runtime.probe(&source).unwrap(),
        duration_seconds: 64.,
    };
    lib.record_match_outcome(
        &old,
        &input,
        &TagMatchOutcome {
            status: MatchStatus::NoMatch,
            matches: vec![],
            attempted: vec![],
            warnings: vec![],
        },
    )
    .unwrap();
    lib.fail_for_run(&old, "matching", "No match").unwrap();
    let state = lib.track_match_review(&old.track_id).unwrap().unwrap();
    let empty = Arc::new(cuerithm_autotag::Tagger::with_backends(vec![]).unwrap());
    let preview = lib
        .preview_match_review_with(
            &empty,
            &runtime,
            super::MatchReviewRequest {
                track_id: old.track_id.clone(),
                generation: state.generation,
                action: super::MatchReviewAction::KeepExistingOnce,
                candidate_id: None,
                fields: vec![],
                acknowledge_conflicts: false,
                allow_id3_upgrade: false,
            },
            &CancellationToken::default(),
        )
        .unwrap();
    lib.apply_match_review_with(
        &runtime,
        &preview.id,
        &preview.revision,
        &CancellationToken::default(),
    )
    .unwrap();
    let service = crate::PreparationService::start_with_tagger(
        lib.clone(),
        runtime.clone(),
        1,
        empty.clone(),
    )
    .unwrap();
    wait_for_job(&lib, &job, "attention");
    service.shutdown();
    drop(service);
    let failed = lib
        .jobs()
        .unwrap()
        .into_iter()
        .find(|j| j.id == job)
        .unwrap();
    assert_eq!(
        failed.stage, "organizing",
        "Keep once must finish measured tags before missing Title blocks naming: {failed:?}"
    );
    assert_ne!(
        cuerithm_media::file_sha256(&source).unwrap(),
        old.source_sha256
    );
    let immutable: String = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT payload_json FROM preparation_review_decisions WHERE job_id=?1",
            [&job],
            |r| r.get(0),
        )
        .unwrap();
    lib.conn().unwrap().execute("UPDATE preparation_analysis SET analysis_json=json_set(analysis_json,'$.elapsed_seconds',12345.25) WHERE source_hash=?1",[&old.source_sha256]).unwrap();
    let editor = crate::MetadataEditService::new(lib.clone(), runtime.clone());
    let draft = editor
        .create_draft(std::slice::from_ref(&old.track_id))
        .unwrap();
    let mut patch = crate::MetadataEditorPatch::default();
    patch.fields.text.insert(
        MetadataTextField::Title,
        TextEdit::Set {
            values: vec!["Accepted user title".into()],
        },
    );
    let draft = editor
        .update_draft(&draft.id, draft.revision, patch)
        .unwrap();
    let edit = editor
        .preview(&draft.id, draft.revision, &CancellationToken::default())
        .unwrap();
    let before = std::fs::read(&source).unwrap();
    let queued = editor
        .apply(&edit.id, &CancellationToken::default())
        .unwrap();
    assert_eq!(queued.state, "queued");
    assert_eq!(std::fs::read(&source).unwrap(), before);
    let current: String = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT current_job FROM tracks WHERE id=?1",
            [&old.track_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(current, job);
    let service =
        crate::PreparationService::start_with_tagger(lib.clone(), runtime, 1, empty).unwrap();
    wait_for_job(&lib, &job, "completed");
    service.shutdown();
    let track = lib.track(&old.track_id).unwrap();
    assert_eq!(track.title, "Accepted user title");
    assert_eq!(track.state, "ready");
    assert_eq!(
        lib.track_path(&old.track_id).unwrap(),
        managed.join("Accepted user title.flac")
    );
    let (payload, decision_state): (String, String) = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT payload_json,state FROM preparation_review_decisions WHERE job_id=?1",
            [&job],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        payload, immutable,
        "own tag-source rebinding cannot rewrite the accepted decision"
    );
    let elapsed: f64 = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT json_extract(analysis_json,'$.elapsed_seconds') FROM tracks WHERE id=?1",
            [&old.track_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        elapsed, 12345.25,
        "valid measured DSP must be reused after our own metadata publication"
    );
    assert_eq!(
        decision_state, "complete",
        "Keep existing once must finish with its logical preparation, not govern later explicit re-preparation"
    );
}

struct ReviewFixture {
    _temp: tempfile::TempDir,
    lib: Library,
    runtime: cuerithm_media::MediaRuntime,
    source: std::path::PathBuf,
    run: crate::JobRun,
    tagger: cuerithm_autotag::Tagger,
}
impl ReviewFixture {
    fn new() -> Self {
        Self::with_solver(None)
    }
    fn with_solver(endpoint: Option<&str>) -> Self {
        struct Expansion;
        impl cuerithm_autotag::ProviderBackend for Expansion {
            fn id(&self) -> cuerithm_autotag::ProviderId {
                cuerithm_autotag::ProviderId::Beatport
            }
            fn candidates(
                &self,
                _: &TagInput,
                _: &AutotagConfig,
            ) -> anyhow::Result<Vec<CatalogMatch>> {
                anyhow::bail!("accepted selection must not perform another lookup")
            }
            fn extend(&self, track: &mut CatalogTrack, _: &AutotagConfig) -> anyhow::Result<()> {
                track.label = Some("Expanded label".into());
                Ok(())
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = root.join("source.flac");
        std::fs::copy(
            repo.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
            &source,
        )
        .unwrap();
        let runtime =
            cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin"))
                .unwrap();
        let mut tagging = AutotagConfig::default();
        tagging.options.platforms = vec!["beatport".into()];
        tagging.options.parse_filename = false;
        tagging.solver.endpoint = endpoint.map(str::to_owned);
        let mut settings = crate::PreparationSettings {
            autotag_enabled: true,
            tagging: Some(tagging),
            ..Default::default()
        };
        settings.organization = crate::OrganizationSettings {
            mode: crate::OrganizationSettingMode::RenameInPlace,
            pattern: "{original_name}".into(),
            ..Default::default()
        };
        settings.engine.lexicon_energy = true;
        lib.save_settings(settings).unwrap();
        lib.enqueue(
            std::slice::from_ref(&source),
            lib.settings().unwrap().revision,
        )
        .unwrap();
        let run = lib.claim_next().unwrap().unwrap();
        let mut media = runtime.probe(&source).unwrap();
        media.tags.insert("title".into(), "Selected title".into());
        media.tags.insert("artist".into(), "Selected artist".into());
        media.tags.insert("version".into(), "Radio Edit".into());
        let input = TagInput {
            path: source.clone(),
            source_sha256: run.source_sha256.clone(),
            media,
            duration_seconds: 64.,
        };
        let track = CatalogTrack {
            platform: "beatport".into(),
            track_id: Some("123".into()),
            title: "Selected title".into(),
            version: Some("Club Mix".into()),
            artists: vec!["Selected artist".into()],
            label: Some("Search label".into()),
            url: "https://www.beatport.com/track/selected/123".into(),
            art: Some("https://geo-media.beatport.com/image/fixture.png".into()),
            ..Default::default()
        };
        lib.record_match_outcome(
            &run,
            &input,
            &TagMatchOutcome {
                status: MatchStatus::Ambiguous,
                matches: vec![CatalogMatch::new(1., track)],
                attempted: vec![],
                warnings: vec![],
            },
        )
        .unwrap();
        lib.fail_for_run(&run, "matching", "Recording edition requires review")
            .unwrap();
        Self {
            _temp: temp,
            lib,
            runtime,
            source,
            run,
            tagger: cuerithm_autotag::Tagger::with_backends(vec![std::sync::Arc::new(Expansion)])
                .unwrap(),
        }
    }
    fn request(&self) -> super::MatchReviewRequest {
        let state = self
            .lib
            .track_match_review(&self.run.track_id)
            .unwrap()
            .unwrap();
        super::MatchReviewRequest {
            track_id: self.run.track_id.clone(),
            generation: state.generation,
            action: super::MatchReviewAction::UseSelected,
            candidate_id: Some(state.candidates[0].id.clone()),
            fields: vec![
                super::MatchReviewFieldDecision {
                    field: "title".into(),
                    action: super::MatchReviewFieldAction::Replace,
                },
                super::MatchReviewFieldDecision {
                    field: "label".into(),
                    action: super::MatchReviewFieldAction::Replace,
                },
            ],
            acknowledge_conflicts: false,
            allow_id3_upgrade: false,
        }
    }
}
#[test]
fn selected_recording_requires_explicit_edition_ack_and_only_writes_chosen_fields() {
    let f = ReviewFixture::new();
    let cancel = cuerithm_media::CancellationToken::default();
    let before =
        cuerithm_autotag::read_metadata_snapshot(&f.source, &f.runtime.probe(&f.source).unwrap())
            .unwrap();
    let request = f.request();
    let blocked = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request.clone(), &cancel)
        .unwrap();
    assert!(!blocked.can_apply);
    assert!(!blocked.candidate.unwrap().identity_conflicts.is_empty());
    let mut request = request;
    request.acknowledge_conflicts = true;
    let preview = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request, &cancel)
        .unwrap();
    assert!(preview.can_apply, "{:?}", preview.blockers);
    assert_eq!(
        preview
            .fields
            .iter()
            .find(|field| field.field == "label")
            .unwrap()
            .proposed,
        vec!["Expanded label"]
    );
    assert_eq!(
        preview
            .fields
            .iter()
            .find(|field| field.field == "artist")
            .unwrap()
            .after,
        before
            .text
            .get(&cuerithm_autotag::MetadataTextField::Artist)
            .cloned()
            .unwrap_or_default()
    );
    f.lib
        .apply_match_review_with(&f.runtime, &preview.id, &preview.revision, &cancel)
        .unwrap();
    assert!(
        f.lib
            .apply_match_review_with(&f.runtime, &preview.id, &preview.revision, &cancel)
            .is_err()
    );
    let service = crate::PreparationService::start_with_tagger(
        f.lib.clone(),
        f.runtime.clone(),
        1,
        std::sync::Arc::new(f.tagger),
    )
    .unwrap();
    wait_for_job(&f.lib, &f.run.id, "completed");
    service.shutdown();
    let after =
        cuerithm_autotag::read_metadata_snapshot(&f.source, &f.runtime.probe(&f.source).unwrap())
            .unwrap();
    assert_eq!(
        after.text[&cuerithm_autotag::MetadataTextField::Title],
        vec!["Selected title (Club Mix)"]
    );
    assert_eq!(
        after.text[&cuerithm_autotag::MetadataTextField::Label],
        vec!["Expanded label"]
    );
    assert_eq!(
        after.text.get(&cuerithm_autotag::MetadataTextField::Artist),
        before
            .text
            .get(&cuerithm_autotag::MetadataTextField::Artist)
    );
}
#[test]
fn artwork_preview_is_a_bounded_immutable_native_asset_bound_to_the_generation() {
    let f = ReviewFixture::new();
    let cancel = cuerithm_media::CancellationToken::default();
    let mut request = f.request();
    request.acknowledge_conflicts = true;
    let preview = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request.clone(), &cancel)
        .unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let covered = repo.join("crates/cuerithm-media/tests/fixtures/covered.flac");
    let info = f.runtime.probe(&covered).unwrap();
    let picture = cuerithm_autotag::read_metadata_snapshot(&covered, &info)
        .unwrap()
        .artwork
        .remove(0);
    let bytes =
        cuerithm_autotag::metadata_artwork_bytes(&covered, &info, &picture.identity).unwrap();
    let asset = f
        .lib
        .review_artwork_asset_with(
            &f.runtime,
            &preview.id,
            &preview.revision,
            &cancel,
            |provider, url, _| {
                assert_eq!(provider, cuerithm_autotag::ProviderId::Beatport);
                assert_eq!(url, "https://geo-media.beatport.com/image/fixture.png");
                Ok(bytes)
            },
        )
        .unwrap();
    let again = f
        .lib
        .review_artwork_asset_with(
            &f.runtime,
            &preview.id,
            &preview.revision,
            &cancel,
            |_, _, _| panic!("a viewed generation must reuse its exact opaque asset"),
        )
        .unwrap();
    assert_eq!(again.id, asset.id);
    assert_eq!(again.sha256, asset.sha256);
    request.fields.push(super::MatchReviewFieldDecision {
        field: "albumArt".into(),
        action: super::MatchReviewFieldAction::Replace,
    });
    let with_art = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request, &cancel)
        .unwrap();
    assert!(with_art.can_apply, "{:?}", with_art.blockers);
    assert_eq!(
        with_art.artwork.action,
        super::MatchReviewFieldAction::Replace
    );
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET metadata_revision=metadata_revision+1 WHERE id=?1",
            [&f.run.track_id],
        )
        .unwrap();
    assert!(
        f.lib
            .review_artwork_asset_with(
                &f.runtime,
                &preview.id,
                &preview.revision,
                &cancel,
                |_, _, _| panic!("stale source must not fetch image bytes")
            )
            .is_err()
    );
}

#[test]
fn repeated_tag_replay_binds_each_attempt_to_its_known_restored_native_input() {
    let f = ReviewFixture::new();
    let (run, mut staged, result) = prepare_review_write(&f);
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    let output = staged.source_after_sha256.clone();
    let encoded: String = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT artifact_authorities_json FROM tag_writes WHERE id=?1",
            [&journal],
            |r| r.get(0),
        )
        .unwrap();
    let mut authorities: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    let mut published = authorities[0]["before"].clone();
    published["sha256"] = serde_json::json!(output);
    published["bytes"] = serde_json::json!(std::fs::metadata(&staged.path).unwrap().len());
    authorities[0]["published"] = published;
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE tag_writes SET artifact_authorities_json=?1 WHERE id=?2",
            rusqlite::params![serde_json::to_string(&authorities).unwrap(), journal],
        )
        .unwrap();
    std::fs::write(&f.source, std::fs::read(&staged.path).unwrap()).unwrap();
    f.lib.recover_tag_writes().unwrap();
    let (_, restored) = crate::organization::capture_regular_source(&f.source).unwrap();
    let current = f.lib.claim_next().unwrap().unwrap();
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE preparation_checkpoints SET attempt=?1 WHERE job_id=?2",
            rusqlite::params![current.attempt, current.id],
        )
        .unwrap();
    let mut next = None;
    assert!(
        f.lib
            .replay_checkpoint_tag_write_with(&current, &journal, |id| {
                next = Some(id.to_owned());
                anyhow::bail!("fault after durable intent before publication")
            })
            .is_err()
    );
    let next = next.unwrap();
    let input: String = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT input_source_proof_json FROM tag_writes WHERE id=?1",
            [&next],
            |r| r.get(0),
        )
        .unwrap();
    let proof: crate::artifacts::ArtifactRestoreProof = serde_json::from_str(&input).unwrap();
    assert_eq!(
        proof.fingerprint.identity, restored.identity,
        "each fresh retry journal must persist its exact proven current input, not the first attempt's retired inode"
    );
    f.lib.recover_tag_writes().unwrap();
    let resumed = f.lib.claim_next().unwrap().unwrap();
    f.lib
        .conn()
        .unwrap()
        .execute(
            "UPDATE preparation_checkpoints SET attempt=?1 WHERE job_id=?2",
            rusqlite::params![resumed.attempt, resumed.id],
        )
        .unwrap();
    f.lib.replay_checkpoint_tag_write(&resumed, &next).unwrap();
    assert_eq!(cuerithm_media::file_sha256(&f.source).unwrap(), output);
}

fn prepare_review_write(
    f: &ReviewFixture,
) -> (
    crate::JobRun,
    cuerithm_autotag::StagedTagWrite,
    camelot_core::FileAnalysis,
) {
    use cuerithm_autotag::{AnalysisTags, TagWritePlan};
    let state = f.lib.track_match_review(&f.run.track_id).unwrap().unwrap();
    let request = super::MatchReviewRequest {
        track_id: f.run.track_id.clone(),
        generation: state.generation,
        action: super::MatchReviewAction::KeepExistingOnce,
        candidate_id: None,
        fields: vec![],
        acknowledge_conflicts: false,
        allow_id3_upgrade: false,
    };
    let cancel = cuerithm_media::CancellationToken::default();
    let preview = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request, &cancel)
        .unwrap();
    f.lib
        .apply_match_review_with(&f.runtime, &preview.id, &preview.revision, &cancel)
        .unwrap();
    let run = f.lib.claim_next().unwrap().unwrap();
    let review = f
        .lib
        .protected_match_review_for_run(&run, &f.runtime)
        .unwrap()
        .unwrap();
    let audio = f.runtime.decode(&f.source, f.lib.data_dir()).unwrap();
    let mut engine = camelot_core::Engine::new(&run.snapshot.settings.engine).unwrap();
    let mut result = engine.analyze_prepared(&audio).unwrap();
    crate::publication::apply(&mut result, &run.snapshot.settings).unwrap();
    let measured = AnalysisTags {
        camelot_key: result.analysis.key.as_ref().map(|k| k.camelot.clone()),
        standard_key: result.analysis.key.as_ref().map(|k| k.key.name()),
        bpm: crate::publication::tag_bpm(&result, &run.snapshot.settings).unwrap(),
        energy: result.analysis.energy.as_ref().map(|e| e.level),
    };
    let input = TagInput {
        path: f.source.clone(),
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    let staged = TagWritePlan::build(input, &review.outcome, &review.config, &measured)
        .unwrap()
        .stage_prepared(&f.runtime, &audio, f.lib.data_dir())
        .unwrap();
    (run, staged, result)
}

#[test]
fn native_tag_publication_never_adopts_equal_byte_replaced_objects() {
    use crate::tagging::TagPublicationBoundary as B;
    for fault in [B::CopiesPrepared, B::BeforeExchange, B::Published] {
        let f = ReviewFixture::new();
        let (run, mut staged, result) = prepare_review_write(&f);
        let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
        let encoded: Vec<u8> = f
            .lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT publish_path FROM tag_write_files WHERE write_id=?1 AND ordinal=0",
                [&journal],
                |r| r.get(0),
            )
            .unwrap();
        let publication = crate::model::bytes_path(&encoded).unwrap();
        let retained = f.source.with_extension("external-retained");
        let mut injected = false;
        let error = f
            .lib
            .publish_preparation_with(&run, &journal, |at| {
                if at == fault && !injected {
                    injected = true;
                    let path = if fault == B::CopiesPrepared {
                        &publication
                    } else {
                        &f.source
                    };
                    let bytes = std::fs::read(path)?;
                    std::fs::rename(path, &retained)?;
                    std::fs::write(path, bytes)?;
                }
                Ok(())
            })
            .unwrap_err();
        assert!(injected, "fault boundary must execute: {error:#}");
        assert!(retained.exists(), "externally retained object must survive");
        assert_eq!(
            cuerithm_media::file_sha256(&staged.backup_path).unwrap(),
            run.source_sha256
        );
        if fault == B::CopiesPrepared {
            assert_eq!(
                cuerithm_media::file_sha256(&f.source).unwrap(),
                run.source_sha256
            );
            assert!(
                publication.exists(),
                "unknown same-byte stage must remain retained"
            );
        } else {
            assert_eq!(
                std::fs::read(&f.source).unwrap(),
                std::fs::read(&retained).unwrap(),
                "same-byte replacement at the source path must never be overwritten by rollback"
            );
        }
        assert_eq!(f.lib.cue_revision_count(&run.track_id).unwrap(), 0);
    }
}

#[test]
fn tag_temporary_rotation_keeps_path_and_native_receipts_atomic_on_database_failure() {
    let f = ReviewFixture::new();
    let (run, mut staged, result) = prepare_review_write(&f);
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    let old: Vec<u8> = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT publish_path FROM tag_write_files WHERE write_id=?1 AND ordinal=0",
            [&journal],
            |r| r.get(0),
        )
        .unwrap();
    std::fs::write(
        crate::model::bytes_path(&old).unwrap(),
        b"unproven interrupted copy",
    )
    .unwrap();
    f.lib.conn().unwrap().execute_batch("CREATE TRIGGER fail_receipt_rotation BEFORE UPDATE OF artifact_authorities_json ON tag_writes BEGIN SELECT RAISE(ABORT,'injected authority write failure');END;").unwrap();
    assert!(f.lib.publish_preparation(&run, &journal).is_err());
    let current: Vec<u8> = f
        .lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT publish_path FROM tag_write_files WHERE write_id=?1 AND ordinal=0",
            [&journal],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        current, old,
        "publication names must not commit without the matching immutable retirement receipts"
    );
    assert_eq!(
        std::fs::read(crate::model::bytes_path(&old).unwrap()).unwrap(),
        b"unproven interrupted copy"
    );
    let db = f.lib.data_dir().join("library.sqlite");
    drop(f.lib);
    let reopened = Library::open(&db).unwrap();
    let recovered: Vec<u8> = reopened
        .conn()
        .unwrap()
        .query_row(
            "SELECT publish_path FROM tag_write_files WHERE write_id=?1 AND ordinal=0",
            [&journal],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(recovered, old);
    assert_eq!(
        std::fs::read(crate::model::bytes_path(&old).unwrap()).unwrap(),
        b"unproven interrupted copy"
    );
}
#[test]
fn explicit_requery_uses_folder_preset_matching_and_current_global_transport_only() {
    use cuerithm_autotag::{ChallengeStore, ProviderBackend, ProviderId};
    use std::sync::{Arc, Mutex};
    struct Catalog(Mutex<Vec<(f64, Option<String>)>>);
    impl ProviderBackend for Catalog {
        fn id(&self) -> ProviderId {
            ProviderId::Traxsource
        }
        fn candidates(
            &self,
            _input: &TagInput,
            config: &AutotagConfig,
        ) -> anyhow::Result<Vec<CatalogMatch>> {
            self.0
                .lock()
                .unwrap()
                .push((config.options.strictness, config.solver.endpoint.clone()));
            Ok(vec![])
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let incoming = root.join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let source = incoming.join("source.flac");
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::copy(
        repo.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
        &source,
    )
    .unwrap();
    let runtime =
        cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin")).unwrap();
    let mut tagging = AutotagConfig::default();
    tagging.options.platforms = vec!["beatport".into()];
    tagging.solver.endpoint = Some("http://127.0.0.1:8191".into());
    let mut settings = crate::PreparationSettings {
        watch_folders: vec![incoming],
        tagging: Some(tagging),
        ..Default::default()
    };
    let base = lib.save_settings(settings.clone()).unwrap();
    let mut preset_draft = settings.clone();
    let config = preset_draft.tagging.as_mut().unwrap();
    config.options.platforms = vec!["traxsource".into()];
    config.options.strictness = 0.33;
    config.solver.endpoint = Some("http://127.0.0.1:9999".into());
    let projection = lib.project_preparation_preset(&preset_draft).unwrap();
    let preset = lib
        .save_preparation_preset(crate::PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Assigned review rules".into(),
            rules: projection.rules,
            connections: projection.connections,
        })
        .unwrap();
    let folder = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    lib.save_settings_with_preparation_assignments(
        settings.clone(),
        &base,
        &[crate::PreparationFolderChange {
            folder_id: folder.folder_id,
            expected_revision: None,
            choice: crate::PreparationFolderChoice::Preset {
                preset_id: preset.id.clone(),
            },
        }],
    )
    .unwrap();
    let job = lib
        .enqueue(std::slice::from_ref(&source), base)
        .unwrap()
        .remove(0);
    let track: String = lib
        .conn()
        .unwrap()
        .query_row("SELECT track_id FROM jobs WHERE id=?1", [&job], |r| {
            r.get(0)
        })
        .unwrap();
    lib.conn()
        .unwrap()
        .execute(
            "UPDATE tracks SET state='ready',current_job=NULL WHERE id=?1",
            [&track],
        )
        .unwrap();
    lib.mark_requires_solver("traxsource").unwrap();
    let before = std::fs::read(&source).unwrap();
    let state = lib.track_match_review(&track).unwrap().unwrap();
    assert_eq!(
        state
            .providers
            .iter()
            .map(|provider| provider.id.as_str())
            .collect::<Vec<_>>(),
        vec!["traxsource"],
        "fresh capabilities must use the source's assigned preset, not the global fallback provider list"
    );
    assert!(state.providers[0].available && state.providers[0].requires_solver);
    let backend = Arc::new(Catalog(Mutex::new(vec![])));
    let tagger = cuerithm_autotag::Tagger::with_backends(vec![backend.clone()]).unwrap();
    let first = lib
        .requery_matches_with(
            &tagger,
            &runtime,
            &track,
            &state.generation,
            &cuerithm_media::CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(
        *backend.0.lock().unwrap(),
        vec![(0.33, Some("http://127.0.0.1:8191".into()))]
    );
    let mut rules = preset.rules;
    rules.tagging.as_mut().unwrap().options.strictness = 0.55;
    lib.save_preparation_preset(crate::PreparationPresetSave {
        id: Some(preset.id),
        expected_revision: Some(preset.revision),
        name: preset.name,
        rules,
        connections: preset.connections,
    })
    .unwrap();
    settings.tagging.as_mut().unwrap().solver.endpoint = Some("http://127.0.0.1:8192".into());
    lib.save_settings(settings).unwrap();
    assert_eq!(
        lib.track_match_review(&track).unwrap().unwrap().generation,
        first.generation,
        "preset/settings edits must not replace retained catalog generations"
    );
    assert_eq!(backend.0.lock().unwrap().len(), 1);
    lib.requery_matches_with(
        &tagger,
        &runtime,
        &track,
        &first.generation,
        &cuerithm_media::CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(
        *backend.0.lock().unwrap(),
        vec![
            (0.33, Some("http://127.0.0.1:8191".into())),
            (0.55, Some("http://127.0.0.1:8192".into()))
        ]
    );
    assert_eq!(std::fs::read(&source).unwrap(), before);
    assert_eq!(lib.track(&track).unwrap().state, "ready");
    assert_eq!(lib.cue_revision_count(&track).unwrap(), 0);
}

#[test]
fn owned_keep_once_retry_survives_changed_or_disconnected_current_preset() {
    use std::sync::Arc;
    for disconnected in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
        let incoming = root.join("incoming");
        std::fs::create_dir(&incoming).unwrap();
        let source = incoming.join("source.flac");
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::copy(
            repo.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
            &source,
        )
        .unwrap();
        let runtime =
            cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin"))
                .unwrap();
        let mut tagging = AutotagConfig::default();
        tagging.options.platforms = vec!["beatport".into()];
        tagging.options.parse_filename = false;
        tagging.no_match = cuerithm_autotag::NoMatchPolicy::Attention;
        let mut settings = crate::PreparationSettings {
            watch_folders: vec![incoming],
            autotag_enabled: true,
            tagging: Some(tagging),
            ..Default::default()
        };
        settings.engine.lexicon_energy = true;
        let base = lib.save_settings(settings.clone()).unwrap();
        let projection = lib.project_preparation_preset(&settings).unwrap();
        let preset = lib
            .save_preparation_preset(crate::PreparationPresetSave {
                id: None,
                expected_revision: None,
                name: "Owned policy".into(),
                rules: projection.rules,
                connections: projection.connections,
            })
            .unwrap();
        let folder = lib
            .preparation_preset_folders_for_settings(&settings)
            .unwrap()
            .remove(0);
        lib.save_settings_with_preparation_assignments(
            settings,
            &base,
            &[crate::PreparationFolderChange {
                folder_id: folder.folder_id,
                expected_revision: None,
                choice: crate::PreparationFolderChoice::Preset {
                    preset_id: preset.id.clone(),
                },
            }],
        )
        .unwrap();
        let job = lib
            .enqueue_with_current_preparation(std::slice::from_ref(&source), false)
            .unwrap()
            .remove(0);
        let original = lib.claim_next().unwrap().unwrap();
        let input = TagInput {
            path: source.clone(),
            source_sha256: original.source_sha256.clone(),
            duration_seconds: 64.,
            media: runtime.probe(&source).unwrap(),
        };
        lib.record_match_outcome(
            &original,
            &input,
            &TagMatchOutcome {
                status: MatchStatus::NoMatch,
                matches: vec![],
                attempted: vec![],
                warnings: vec![],
            },
        )
        .unwrap();
        lib.fail_for_run(&original, "matching", "No match").unwrap();
        let state = lib.track_match_review(&original.track_id).unwrap().unwrap();
        let tagger = Arc::new(cuerithm_autotag::Tagger::with_backends(vec![]).unwrap());
        let preview = lib
            .preview_match_review_with(
                &tagger,
                &runtime,
                super::MatchReviewRequest {
                    track_id: original.track_id.clone(),
                    generation: state.generation,
                    action: super::MatchReviewAction::KeepExistingOnce,
                    candidate_id: None,
                    fields: vec![],
                    acknowledge_conflicts: false,
                    allow_id3_upgrade: false,
                },
                &cuerithm_media::CancellationToken::default(),
            )
            .unwrap();
        lib.apply_match_review_with(
            &runtime,
            &preview.id,
            &preview.revision,
            &cuerithm_media::CancellationToken::default(),
        )
        .unwrap();
        let owned = lib.claim_next().unwrap().unwrap();
        let immutable: String = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT payload_json FROM preparation_review_decisions WHERE job_id=?1",
                [&job],
                |r| r.get(0),
            )
            .unwrap();
        let mut rules = preset.rules;
        rules.tagging.as_mut().unwrap().options.strictness = 0.99;
        let mut connections = preset.connections;
        if disconnected {
            let destination = root.join("other-managed");
            std::fs::create_dir(&destination).unwrap();
            connections.destination_root_id = Some(
                lib.register_organization_root(destination.clone())
                    .unwrap()
                    .id,
            );
            rules.organization.mode = crate::OrganizationSettingMode::Copy;
            lib.save_preparation_preset(crate::PreparationPresetSave {
                id: Some(preset.id.clone()),
                expected_revision: Some(preset.revision.clone()),
                name: preset.name.clone(),
                rules: rules.clone(),
                connections: connections.clone(),
            })
            .unwrap();
            std::fs::rename(&destination, root.join("retained-other-managed")).unwrap();
        } else {
            lib.save_preparation_preset(crate::PreparationPresetSave {
                id: Some(preset.id),
                expected_revision: Some(preset.revision),
                name: preset.name,
                rules,
                connections,
            })
            .unwrap();
        }
        // An owned running claim keeps the worker idle until this explicit retry.
        let service =
            crate::PreparationService::start_with_tagger(lib.clone(), runtime, 1, tagger).unwrap();
        lib.fail_for_run(
            &owned,
            "matching",
            "synthetic interruption before publication",
        )
        .unwrap();
        let retried = service.retry(job.clone(), Some(lib.settings().unwrap().revision));
        if let Err(error) = retried {
            service.shutdown();
            panic!(
                "owned accepted recovery must remain usable after a current preset change (disconnected={disconnected}): {error:#}"
            );
        }
        wait_for_job(&lib, &job, "completed");
        service.shutdown();
        let current = lib
            .jobs()
            .unwrap()
            .into_iter()
            .find(|row| row.id == job)
            .unwrap();
        assert_eq!(current.attempts, owned.attempt + 1);
        let revision: String = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT settings_revision FROM jobs WHERE id=?1",
                [&job],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(revision, original.settings_revision);
        let retained: String = lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT payload_json FROM preparation_review_decisions WHERE job_id=?1",
                [&job],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            retained, immutable,
            "current rules must not rewrite accepted provenance"
        );
        assert_eq!(lib.track_path(&original.track_id).unwrap(), source);
        assert_eq!(lib.cue_revision_count(&original.track_id).unwrap(), 1);
    }
}

#[test]
fn incoming_editor_fresh_lookup_uses_current_global_solver_without_rebinding_accepted_evidence() {
    use cuerithm_autotag::{MetadataTextField, ProviderBackend, ProviderId, TextEdit};
    use cuerithm_media::CancellationToken;
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct Catalog(Mutex<Vec<(Option<String>, u64, f64)>>);
    impl ProviderBackend for Catalog {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(
            &self,
            _: &TagInput,
            config: &AutotagConfig,
        ) -> anyhow::Result<Vec<CatalogMatch>> {
            self.0.lock().unwrap().push((
                config.solver.endpoint.clone(),
                config.solver.timeout_seconds,
                config.options.strictness,
            ));
            Ok(vec![])
        }
    }
    for endpoint in [Some("http://127.0.0.1:8192".to_owned()), None] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = root.join("source.flac");
        std::fs::copy(
            repo.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
            &source,
        )
        .unwrap();
        let runtime =
            cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin"))
                .unwrap();
        let mut config = AutotagConfig::default();
        config.options.platforms = vec!["beatport".into()];
        config.options.parse_filename = false;
        config.options.strictness = 0.37;
        config.no_match = cuerithm_autotag::NoMatchPolicy::KeepExisting;
        config.solver.endpoint = Some("http://127.0.0.1:8191".into());
        let mut settings = crate::PreparationSettings {
            autotag_enabled: true,
            tagging: Some(config),
            ..Default::default()
        };
        settings.engine.lexicon_energy = true;
        lib.save_settings(settings.clone()).unwrap();
        let job = lib
            .enqueue(
                std::slice::from_ref(&source),
                lib.settings().unwrap().revision,
            )
            .unwrap()
            .remove(0);
        let first = lib.claim_next().unwrap().unwrap();
        lib.fail_for_run(&first, "matching", "synthetic provider interruption")
            .unwrap();
        let editor = crate::MetadataEditService::new(lib.clone(), runtime.clone());
        let draft = editor
            .create_draft(std::slice::from_ref(&first.track_id))
            .unwrap();
        let mut patch = crate::MetadataEditorPatch::default();
        patch.fields.text.insert(
            MetadataTextField::Title,
            TextEdit::Set {
                values: vec!["Accepted manual title".into()],
            },
        );
        let draft = editor
            .update_draft(&draft.id, draft.revision, patch)
            .unwrap();
        let preview = editor
            .preview(&draft.id, draft.revision, &CancellationToken::default())
            .unwrap();
        editor
            .apply(&preview.id, &CancellationToken::default())
            .unwrap();
        let owned = lib.claim_next().unwrap().unwrap();
        let evidence = || {
            lib.conn().unwrap().query_row("SELECT payload_json,execution_source_proof_json,settings_revision FROM metadata_edit_overrides WHERE job_id=?1",[&job],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?))).unwrap()
        };
        let before = evidence();
        settings.tagging.as_mut().unwrap().solver.endpoint = endpoint.clone();
        settings.tagging.as_mut().unwrap().solver.timeout_seconds = 42;
        lib.save_settings(settings).unwrap();
        let backend = Arc::new(Catalog::default());
        let tagger =
            Arc::new(cuerithm_autotag::Tagger::with_backends(vec![backend.clone()]).unwrap());
        let worker =
            crate::PreparationService::start_with_tagger(lib.clone(), runtime, 1, tagger).unwrap();
        lib.fail_for_run(&owned, "matching", "synthetic interruption before lookup")
            .unwrap();
        worker
            .retry(job.clone(), Some(lib.settings().unwrap().revision))
            .unwrap();
        wait_for_job(&lib, &job, "completed");
        worker.shutdown();
        assert_eq!(
            *backend.0.lock().unwrap(),
            vec![(endpoint, 42, 0.37)],
            "fresh lookup must consume current global transport, including removal, while keeping captured match policy"
        );
        assert_eq!(
            evidence(),
            before,
            "transport refresh must not rewrite accepted payload, source proof or policy revision"
        );
        assert_eq!(
            lib.track(&first.track_id).unwrap().title,
            "Accepted manual title"
        );
    }
}

#[test]
fn retained_candidate_expansion_uses_live_transport_and_blocks_after_solver_removal() {
    use cuerithm_autotag::{ChallengeStore, ProviderBackend, ProviderId};
    use std::sync::{Arc, Mutex};
    #[derive(Default)]
    struct Catalog(Mutex<Vec<(Option<String>, u64, f64)>>);
    impl ProviderBackend for Catalog {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
            anyhow::bail!("retained selection must not requery")
        }
        fn extend(&self, _: &mut CatalogTrack, config: &AutotagConfig) -> anyhow::Result<()> {
            self.0.lock().unwrap().push((
                config.solver.endpoint.clone(),
                config.solver.timeout_seconds,
                config.options.strictness,
            ));
            Ok(())
        }
    }
    for endpoint in [Some("http://127.0.0.1:8192".to_owned()), None] {
        let f = ReviewFixture::with_solver(Some("http://127.0.0.1:8191"));
        let captured = || {
            f.lib
                .conn()
                .unwrap()
                .query_row(
                    "SELECT payload_json FROM preparation_match_generations WHERE track_id=?1",
                    [&f.run.track_id],
                    |r| r.get::<_, String>(0),
                )
                .unwrap()
        };
        let before = captured();
        let source_before = std::fs::read(&f.source).unwrap();
        let mut request = f.request();
        request.acknowledge_conflicts = true;
        let generation = request.generation.clone();
        let candidate = request.candidate_id.clone();
        let mut settings = f.lib.settings().unwrap().settings;
        let config = settings.tagging.as_mut().unwrap();
        config.solver.endpoint = endpoint.clone();
        config.solver.timeout_seconds = 42;
        config.options.strictness = 0.99;
        f.lib.save_settings(settings).unwrap();
        f.lib.mark_requires_solver("beatport").unwrap();
        let backend = Arc::new(Catalog::default());
        let tagger = cuerithm_autotag::Tagger::with_backends(vec![backend.clone()]).unwrap();
        let preview = f
            .lib
            .preview_match_review_with(
                &tagger,
                &f.runtime,
                request,
                &cuerithm_media::CancellationToken::default(),
            )
            .unwrap();
        if let Some(endpoint) = endpoint {
            assert!(preview.can_apply, "{:?}", preview.blockers);
            assert_eq!(
                *backend.0.lock().unwrap(),
                vec![(
                    Some(endpoint),
                    42,
                    f.run
                        .snapshot
                        .settings
                        .tagging
                        .as_ref()
                        .unwrap()
                        .options
                        .strictness
                )]
            );
        } else {
            assert!(
                !preview.can_apply,
                "a detected provider must be unavailable after global solver removal"
            );
            assert!(
                preview
                    .blockers
                    .iter()
                    .any(|reason| reason.contains("Unavailable"))
            );
            assert!(
                backend.0.lock().unwrap().is_empty(),
                "unavailable providers must block before expansion"
            );
        }
        assert_eq!(preview.generation, generation);
        assert_eq!(
            preview
                .candidate
                .as_ref()
                .map(|candidate| candidate.id.clone()),
            candidate
        );
        assert_eq!(
            captured(),
            before,
            "transport refresh must not rewrite retained candidate/policy/native evidence"
        );
        assert_eq!(std::fs::read(&f.source).unwrap(), source_before);
    }
}

#[test]
fn retained_candidate_access_keeps_captured_provider_policy_when_current_preset_disconnects() {
    let f = ReviewFixture::with_solver(Some("http://127.0.0.1:8191"));
    let mut settings = f.lib.settings().unwrap().settings;
    settings.watch_folders = vec![f.source.parent().unwrap().to_path_buf()];
    settings.tagging.as_mut().unwrap().solver.endpoint = Some("http://127.0.0.1:8192".into());
    let selected = f.lib.save_settings(settings.clone()).unwrap();
    let projection = f.lib.project_preparation_preset(&settings).unwrap();
    let mut rules = projection.rules;
    rules.organization.mode = crate::OrganizationSettingMode::Copy;
    let destination = f.source.parent().unwrap().join("future-managed");
    std::fs::create_dir(&destination).unwrap();
    let grant = f
        .lib
        .register_organization_root(destination.clone())
        .unwrap();
    let preset = f
        .lib
        .save_preparation_preset(crate::PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Unavailable future policy".into(),
            rules,
            connections: crate::PreparationPresetConnections {
                destination_root_id: Some(grant.id),
            },
        })
        .unwrap();
    let folder = f
        .lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    f.lib
        .save_settings_with_preparation_assignments(
            settings,
            &selected,
            &[crate::PreparationFolderChange {
                folder_id: folder.folder_id,
                expected_revision: folder.revision,
                choice: crate::PreparationFolderChoice::Preset {
                    preset_id: preset.id,
                },
            }],
        )
        .unwrap();
    std::fs::rename(
        &destination,
        destination.with_file_name("retained-future-managed"),
    )
    .unwrap();
    let state = f.lib.track_match_review(&f.run.track_id).unwrap().unwrap();
    assert!(
        !state.can_requery,
        "new queries require available current rules"
    );
    assert!(state.can_keep_existing_once);
    assert_eq!(
        state
            .providers
            .iter()
            .map(|provider| (provider.id.as_str(), provider.available))
            .collect::<Vec<_>>(),
        vec![("beatport", true)],
        "retained choices must retain captured provider policy while global transport remains usable"
    );
    let mut request = f.request();
    request.acknowledge_conflicts = true;
    let preview = f
        .lib
        .preview_match_review_with(
            &f.tagger,
            &f.runtime,
            request,
            &cuerithm_media::CancellationToken::default(),
        )
        .unwrap();
    assert!(preview.can_apply, "{:?}", preview.blockers);
    assert_eq!(preview.generation, state.generation);
}
