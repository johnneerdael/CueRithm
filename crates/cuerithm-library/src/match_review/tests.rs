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
        Self::reach_attention(&lib, &runtime, &source, &run);
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
    /// Preparation found an ambiguous recording and stopped for review.
    fn reach_attention(
        lib: &Library,
        runtime: &cuerithm_media::MediaRuntime,
        source: &std::path::Path,
        run: &crate::JobRun,
    ) {
        let mut media = runtime.probe(source).unwrap();
        media.tags.insert("title".into(), "Selected title".into());
        media.tags.insert("artist".into(), "Selected artist".into());
        media.tags.insert("version".into(), "Radio Edit".into());
        let input = TagInput {
            path: source.to_path_buf(),
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
            run,
            &input,
            &TagMatchOutcome {
                status: MatchStatus::Ambiguous,
                matches: vec![CatalogMatch::new(1., track)],
                attempted: vec![],
                warnings: vec![],
            },
        )
        .unwrap();
        lib.fail_for_run(run, "matching", "Recording edition requires review")
            .unwrap();
    }
    /// The user saved an edit for the Incoming track, and preparation stopped for review again
    /// before publishing it.
    fn with_saved_edit(
        edit: impl FnOnce(&crate::MetadataEditService) -> crate::MetadataEditorPatch,
    ) -> Self {
        let mut f = Self::new();
        let service = crate::MetadataEditService::new(f.lib.clone(), f.runtime.clone());
        let draft = service
            .create_draft(std::slice::from_ref(&f.run.track_id))
            .unwrap();
        let patch = edit(&service);
        let draft = service
            .update_draft(&draft.id, draft.revision, patch)
            .unwrap();
        let cancel = cuerithm_media::CancellationToken::default();
        let preview = service.preview(&draft.id, draft.revision, &cancel).unwrap();
        let queued = service.apply(&preview.id, &cancel).unwrap();
        assert_eq!(queued.state, "queued", "{queued:?}");
        f.run = f.lib.claim_next().unwrap().unwrap();
        Self::reach_attention(&f.lib, &f.runtime, &f.source, &f.run);
        f
    }
    fn with_saved_title_edit() -> Self {
        use cuerithm_autotag::{MetadataTextField, TextEdit};
        Self::with_saved_edit(|_| {
            let mut patch = crate::MetadataEditorPatch::default();
            patch.fields.text.insert(
                MetadataTextField::Title,
                TextEdit::Set {
                    values: vec!["Saved edit title".into()],
                },
            );
            patch
        })
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

fn previews(lib: &Library) -> i64 {
    lib.conn()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM preparation_match_previews", [], |r| {
            r.get(0)
        })
        .unwrap()
}
#[test]
fn only_the_active_review_preview_is_kept_and_a_consumed_one_is_reclaimed() {
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
    lib.save_settings(crate::PreparationSettings {
        autotag_enabled: true,
        tagging: Some(tagging),
        ..Default::default()
    })
    .unwrap();
    lib.enqueue(
        std::slice::from_ref(&source),
        lib.settings().unwrap().revision,
    )
    .unwrap();
    let run = lib.claim_next().unwrap().unwrap();
    let input = TagInput {
        path: source.clone(),
        source_sha256: run.source_sha256.clone(),
        duration_seconds: 1.,
        media: runtime.probe(&source).unwrap(),
    };
    lib.record_match_outcome(
        &run,
        &input,
        &TagMatchOutcome {
            status: MatchStatus::NoMatch,
            matches: vec![],
            attempted: vec![],
            warnings: vec![],
        },
    )
    .unwrap();
    lib.fail_for_run(&run, "matching", "No match").unwrap();
    let generation = lib
        .track_match_review(&run.track_id)
        .unwrap()
        .unwrap()
        .generation;
    let preview = || {
        lib.preview_match_review_with(
            &cuerithm_autotag::Tagger::with_backends(vec![]).unwrap(),
            &runtime,
            super::MatchReviewRequest {
                track_id: run.track_id.clone(),
                generation: generation.clone(),
                action: super::MatchReviewAction::KeepExistingOnce,
                candidate_id: None,
                fields: vec![],
                acknowledge_conflicts: false,
                allow_id3_upgrade: false,
            },
            &cuerithm_media::CancellationToken::default(),
        )
        .unwrap()
    };
    let first = preview();
    let second = preview();
    assert_eq!(
        previews(&lib),
        1,
        "a newer preview replaces the older one instead of accumulating"
    );
    let cancel = cuerithm_media::CancellationToken::default();
    assert!(
        lib.apply_match_review_with(&runtime, &first.id, &first.revision, &cancel)
            .is_err(),
        "the replaced preview can no longer be applied"
    );
    lib.apply_match_review_with(&runtime, &second.id, &second.revision, &cancel)
        .unwrap();
    assert_eq!(previews(&lib), 0, "an applied preview is not kept");
    assert!(
        lib.apply_match_review_with(&runtime, &second.id, &second.revision, &cancel)
            .unwrap_err()
            .to_string()
            .contains("already consumed"),
        "a consumed preview still explains itself"
    );
}

#[test]
fn a_saved_incoming_edit_is_shown_as_the_value_the_review_will_end_with() {
    let f = ReviewFixture::with_saved_title_edit();
    let cancel = cuerithm_media::CancellationToken::default();
    let title = |preview: &super::MatchReviewPreview| {
        preview
            .fields
            .iter()
            .find(|field| field.field == "title")
            .unwrap()
            .clone()
    };
    let mut request = f.request();
    request.acknowledge_conflicts = true;
    let blocked = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request.clone(), &cancel)
        .unwrap();
    let held = title(&blocked);
    assert_eq!(held.before, vec!["Saved edit title"]);
    assert_eq!(held.after, vec!["Saved edit title"]);
    assert_eq!(
        held.allowed_actions,
        vec![super::MatchReviewFieldAction::Keep]
    );
    assert!(
        !blocked.can_apply,
        "replacing a field the saved edit sets cannot be promised: {:?}",
        blocked.blockers
    );
    request.fields.retain(|decision| decision.field != "title");
    let preview = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request, &cancel)
        .unwrap();
    assert!(preview.can_apply, "{:?}", preview.blockers);
    assert_eq!(title(&preview).after, vec!["Saved edit title"]);
    f.lib
        .apply_match_review_with(&f.runtime, &preview.id, &preview.revision, &cancel)
        .unwrap();
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
        vec!["Saved edit title"],
        "the file ends with what the preview showed"
    );
    assert_eq!(
        after.text[&cuerithm_autotag::MetadataTextField::Label],
        vec!["Expanded label"]
    );
}

#[test]
fn an_accepted_review_is_stored_for_its_job_without_a_write_only_history() {
    let f = ReviewFixture::new();
    let cancel = cuerithm_media::CancellationToken::default();
    let mut request = f.request();
    request.acknowledge_conflicts = true;
    let preview = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request, &cancel)
        .unwrap();
    f.lib
        .apply_match_review_with(&f.runtime, &preview.id, &preview.revision, &cancel)
        .unwrap();
    let conn = f.lib.conn().unwrap();
    let decisions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM preparation_review_decisions WHERE job_id=?1",
            [&f.run.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(decisions, 1);
    let history: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE '%decision_history%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(history, 0, "no write-only copy of every accepted decision");
}

#[test]
fn saved_incoming_artwork_counts_and_cannot_be_replaced_by_a_review() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let covered = repo.join("crates/cuerithm-media/tests/fixtures/covered.flac");
    let f = ReviewFixture::with_saved_edit(|service| {
        let runtime =
            cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin"))
                .unwrap();
        let info = runtime.probe(&covered).unwrap();
        let picture = cuerithm_autotag::read_metadata_snapshot(&covered, &info)
            .unwrap()
            .artwork
            .remove(0);
        let bytes =
            cuerithm_autotag::metadata_artwork_bytes(&covered, &info, &picture.identity).unwrap();
        let asset = service.import_artwork(bytes).unwrap();
        let mut patch = crate::MetadataEditorPatch::default();
        patch.artwork.push(crate::MetadataArtworkChoice {
            target: None,
            asset_id: Some(asset.id),
            kind: Some(3),
            description: None,
        });
        patch
    });
    let cancel = cuerithm_media::CancellationToken::default();
    let mut request = f.request();
    request.acknowledge_conflicts = true;
    let kept = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request.clone(), &cancel)
        .unwrap();
    assert!(
        kept.artwork.existing,
        "the saved cover counts as the file's artwork"
    );
    request.fields.push(super::MatchReviewFieldDecision {
        field: "albumArt".into(),
        action: super::MatchReviewFieldAction::Replace,
    });
    let blocked = f
        .lib
        .preview_match_review_with(&f.tagger, &f.runtime, request, &cancel)
        .unwrap();
    assert!(!blocked.can_apply);
    assert!(
        blocked
            .artwork
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("saved Incoming edit")),
        "{:?}",
        blocked.artwork
    );
}
