use super::reconcile::{Fixture, track};
use crate::*;
use cuerithm_dj_sync::import::*;
use cuerithm_media::{CancellationToken, MediaRuntime};
fn runtime() -> MediaRuntime {
    let bundle = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    MediaRuntime::from_bundle(&bundle).unwrap()
}
fn setup(policy: ImportPolicy, hot: bool) -> (Fixture, String, String) {
    setup_config(policy, hot, |_, _| {})
}
fn setup_config(
    policy: ImportPolicy,
    hot: bool,
    configure: impl FnOnce(&Fixture, &mut camelot_core::EngineSettings),
) -> (Fixture, String, String) {
    setup_custom(policy, hot, configure, |_| {})
}
fn setup_custom(
    policy: ImportPolicy,
    hot: bool,
    configure: impl FnOnce(&Fixture, &mut camelot_core::EngineSettings),
    transform: impl FnOnce(&std::path::Path),
) -> (Fixture, String, String) {
    let f = Fixture::new();
    let mut settings = PreparationSettings::default();
    settings.engine.source_profile_timing = false;
    settings.engine.lexicon_energy = true;
    configure(&f, &mut settings.engine);
    f.lib.save_settings(settings).unwrap();
    let path = f.root.path().join("generated.wav");
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
    for n in 0..44100 * 16 {
        let t = n as f64 / 44100.0;
        let sample = (([220.0, 261.625565, 329.627557]
            .iter()
            .map(|hz| (t * hz * std::f64::consts::TAU).sin() * 0.14)
            .sum::<f64>()
            + if t % 0.5 < 0.02 { 0.4 } else { 0.0 })
            * 20000.0) as i16;
        wav.write_sample(sample).unwrap();
        wav.write_sample(sample).unwrap();
    }
    wav.finalize().unwrap();
    transform(&path);
    let mut t = track("Imported");
    t.locations.native_path = Some(path);
    t.locations.platform = Some(std::env::consts::OS.into());
    t.key = Some(MusicalKey {
        presence: FieldPresence::Value,
        raw: Some("native unknown".into()),
        ..Default::default()
    });
    if hot {
        t.cues.push(CueObservation {
            external_id: "native-hot".into(),
            family: CueFamily::Hot,
            name: Some("Original".into()),
            source_slot: Some(5),
            ..Default::default()
        });
    }
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.policy = policy;
    let op = f.apply(&f.preview(r));
    let id = f.lib.tracks(TrackQuery::default()).unwrap().rows[0]
        .id
        .clone();
    (f, op.operation_id, id)
}
fn policy(field: AnalysisField, adoption: AnalysisAdoption) -> ImportPolicy {
    ImportPolicy {
        analysis_fields: [(field, adoption)].into(),
        ..Default::default()
    }
}
#[test]
fn explicit_retry_retains_completed_proposals_and_never_replays_publication() {
    let mut p = policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected);
    p.cue_generation = CueGeneration::Generate;
    let (f, op, _) = setup(p, true);
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let first = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!((first.processed, first.blocked), (1, 1));
    let count = || {
        f.lib.conn().unwrap().query_row("SELECT COUNT(*) FROM import_processing_receipts WHERE operation_id=?1 AND state='proposed'",[&op],|r|r.get::<_,u64>(0)).unwrap()
    };
    assert_eq!(count(), 1);
    let retry = service
        .retry_pending(&op, &CancellationToken::default())
        .unwrap();
    assert_eq!((retry.processed, retry.blocked), (0, 1));
    assert_eq!(count(), 1);
    let history = f
        .lib
        .import_operation_page(ImportOperationQuery::default())
        .unwrap();
    assert_eq!(history.records[0].stages.get("blocked"), Some(&1));
    assert!(history.records[0].can_retry && history.records[0].can_review);
    let preview = f
        .lib
        .preview_import_effects(&op, &CancellationToken::default())
        .unwrap();
    let second_preview = f
        .lib
        .preview_import_effects(&op, &CancellationToken::default())
        .unwrap();
    f.lib
        .apply_import_effects(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    assert!(
        service
            .retry_pending(&op, &CancellationToken::default())
            .is_err()
    );
    f.lib
        .undo_import_operation(&op, &CancellationToken::default())
        .unwrap();
    assert!(
        service
            .retry_pending(&op, &CancellationToken::default())
            .is_err()
    );
    assert!(
        f.lib
            .preview_import_effects(&op, &CancellationToken::default())
            .is_err()
    );
    assert!(
        f.lib
            .apply_import_effects(&second_preview.preview_id, &CancellationToken::default())
            .is_err()
    );
}

#[test]
fn global_incoming_settings_cannot_change_import_policy() {
    let (f, op, id) = setup(
        policy(AnalysisField::Key, AnalysisAdoption::ReplaceSelected),
        true,
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    let mut global = f.lib.settings().unwrap().settings;
    global.autotag_enabled = true;
    global.cleanup_enabled = true;
    global.engine.config.key_frame_size = 1024;
    f.lib.save_settings(global).unwrap();
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let result = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(result.processed, 1);
    let stages = f.lib.import_processing_stages(&op, None, 100).unwrap();
    assert_eq!(stages.records.len(), 1);
    assert_eq!(stages.records[0].stage, "analysis.key");
    assert_eq!(
        stages.records[0].identity["engine_settings"]["config"]["key_frame_size"],
        4096
    );
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    assert!(f.lib.jobs().unwrap().is_empty());
    service.shutdown();
}
#[test]
fn key_and_energy_choices_are_independent() {
    for field in [AnalysisField::Key, AnalysisField::Energy] {
        let (f, op, id) = setup(policy(field, AnalysisAdoption::ReplaceSelected), true);
        let before = f.lib.imported_item(&id).unwrap().active_fields;
        let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
        let result = service.run_pending(&CancellationToken::default()).unwrap();
        assert_eq!(result.processed, 1);
        let stages = f.lib.import_processing_stages(&op, None, 100).unwrap();
        assert_eq!(stages.records.len(), 1);
        assert_eq!(
            stages.records[0].stage,
            if field == AnalysisField::Key {
                "analysis.key"
            } else {
                "analysis.energy"
            }
        );
        assert!(stages.records[0].proposal.is_some());
        assert!(
            stages.records[0].identity["pcm_sha256"]
                .as_str()
                .is_some_and(|s| s.len() == 64)
        );
        assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
        service.shutdown();
    }
}
#[test]
fn fill_missing_is_not_replace_unknown() {
    let (f, op, id) = setup(
        policy(AnalysisField::Key, AnalysisAdoption::FillMissing),
        true,
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    assert_eq!(
        f.lib
            .conn()
            .unwrap()
            .query_row(
                "SELECT state FROM import_operations WHERE id=?1",
                [&op],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        "imported"
    );
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let result = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(result.processed, 0);
    assert!(
        f.lib
            .import_processing_stages(&op, None, 100)
            .unwrap()
            .records
            .is_empty()
    );
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    service.shutdown();
}
#[test]
fn generation_respects_occupied_hotcue_family() {
    let p = ImportPolicy {
        cue_generation: CueGeneration::Generate,
        ..Default::default()
    };
    let (f, op, id) = setup(p, true);
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let result = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(result.blocked, 1);
    let stages = f.lib.import_processing_stages(&op, None, 100).unwrap();
    assert_eq!(stages.records[0].stage, "generation.cues");
    assert_eq!(stages.records[0].state, "blocked");
    assert!(stages.records[0].proposal.is_none());
    assert_eq!(
        f.lib.imported_item(&id).unwrap().active_fields["cues.hot"][0]["name"],
        "Original"
    );
    service.shutdown();
}
#[test]
fn source_change_blocks_publication() {
    let (f, op, id) = setup(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        true,
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    std::fs::write(f.lib.track_path(&id).unwrap(), b"changed source").unwrap();
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let result = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(result.blocked, 1);
    let stages = f.lib.import_processing_stages(&op, None, 100).unwrap();
    assert!(stages.records.iter().all(|s| s.proposal.is_none()));
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    service.shutdown();
}
#[test]
fn restart_does_not_replay_completed_stage() {
    let (f, op, _id) = setup(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        false,
    );
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        1
    );
    let first =
        serde_json::to_value(f.lib.import_processing_stages(&op, None, 100).unwrap()).unwrap();
    service.shutdown();
    drop(service);
    let restarted = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    assert_eq!(
        restarted
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        0
    );
    assert_eq!(
        serde_json::to_value(f.lib.import_processing_stages(&op, None, 100).unwrap()).unwrap(),
        first
    );
    assert!(f.lib.jobs().unwrap().is_empty());
    restarted.shutdown();
}

#[test]
fn identical_refresh_does_not_schedule_or_repeat_processing() {
    let (f, op, id) = setup(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        false,
    );
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        1
    );
    let source = f.lib.imported_item(&id).unwrap().observations[0]
        .track
        .clone();
    let i = f.inspect(vec![source], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.policy = policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected);
    let p = f.preview(r);
    assert!(!p.processing_requested);
    let next = f.apply(&p);
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        0
    );
    assert!(
        f.lib
            .import_processing_stages(&next.operation_id, None, 100)
            .unwrap()
            .records
            .is_empty()
    );
    assert_eq!(
        f.lib
            .import_processing_stages(&op, None, 100)
            .unwrap()
            .records
            .len(),
        1
    );
    service.shutdown();
}
#[test]
fn overwrite_permission_alone_and_all_off_create_no_processing_receipts() {
    for replace in [false, true] {
        let (f, op, _) = setup(
            ImportPolicy {
                replace_hotcues: replace,
                ..Default::default()
            },
            true,
        );
        let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
        let s = service.run_pending(&CancellationToken::default()).unwrap();
        assert_eq!(s.processed, 0);
        assert_eq!(s.blocked, 0);
        assert!(
            f.lib
                .import_processing_stages(&op, None, 100)
                .unwrap()
                .records
                .is_empty()
        );
        assert!(!f.lib.inner.root.join("import-media-cache").exists());
        service.shutdown();
    }
}
#[test]
fn null_and_empty_are_not_fill_missing() {
    for presence in [FieldPresence::Null, FieldPresence::Empty] {
        let (f, op, id) = setup(
            policy(AnalysisField::Key, AnalysisAdoption::FillMissing),
            false,
        );
        let conn = f.lib.conn().unwrap();
        conn.execute("UPDATE import_active_fields SET value_json=?1,revision=revision+1 WHERE item_id=?2 AND field='key'",rusqlite::params![serde_json::to_string(&MusicalKey{presence,..Default::default()}).unwrap(),id]).unwrap();
        drop(conn);
        let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
        assert_eq!(
            service
                .run_pending(&CancellationToken::default())
                .unwrap()
                .processed,
            0
        );
        assert!(
            f.lib
                .import_processing_stages(&op, None, 100)
                .unwrap()
                .records
                .is_empty()
        );
        service.shutdown();
    }
}
#[test]
fn allowed_generation_is_a_proposal_with_private_prerequisites() {
    let (f, op, id) = setup(
        ImportPolicy {
            cue_generation: CueGeneration::Generate,
            replace_hotcues: true,
            ..Default::default()
        },
        true,
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.processed, 1);
    let stage = f
        .lib
        .import_processing_stages(&op, None, 10)
        .unwrap()
        .records
        .remove(0);
    let proposal = stage.proposal.unwrap();
    assert!(proposal["raw_analysis"]["analysis"]["key"].is_null());
    assert!(proposal["raw_analysis"]["analysis"]["tempo"].is_null());
    assert!(proposal["raw_analysis"]["analysis"]["energy"].is_null());
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    service.shutdown();
}

#[test]
fn changed_metadata_reuses_only_an_exact_validated_stage_result() {
    let p = policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected);
    let (f, original, id) = setup(p.clone(), false);
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        1
    );
    let mut t = f.lib.imported_item(&id).unwrap().observations[0]
        .track
        .clone();
    t.metadata.insert(
        "title".into(),
        Presence::Value(serde_json::json!("Changed source title")),
    );
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.policy = p;
    let next = f.apply(&f.preview(r));
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.processed, 0);
    assert_eq!(s.reused, 1);
    let stage = f
        .lib
        .import_processing_stages(&next.operation_id, None, 10)
        .unwrap()
        .records
        .remove(0);
    assert_eq!(stage.identity["cache_reuse"]["operation_id"], original);
    service.shutdown();
}
#[test]
fn changed_media_never_reuses_previous_pcm_or_leaves_an_old_source_hash() {
    use std::io::{Seek, Write};
    let p = policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected);
    let (f, _original, id) = setup(p.clone(), false);
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        1
    );
    let path = f.lib.track_path(&id).unwrap();
    let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.seek(std::io::SeekFrom::End(-2)).unwrap();
    file.write_all(&[1, 0]).unwrap();
    drop(file);
    let t = f.lib.imported_item(&id).unwrap().observations[0]
        .track
        .clone();
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.policy = p;
    let preview = f.preview(r);
    assert!(preview.processing_requested);
    f.apply(&preview);
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.processed, 1);
    assert_eq!(s.reused, 0);
    assert_eq!(
        f.lib.source_hash(&id).unwrap(),
        cuerithm_media::file_sha256(&path).unwrap()
    );
    service.shutdown();
}
#[test]
fn changed_selected_model_blocks_only_its_requested_stage() {
    let mut p = policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected);
    p.analysis_fields
        .insert(AnalysisField::Key, AnalysisAdoption::ReplaceSelected);
    let (f, op, _id) = setup_config(p, false, |f, settings| {
        let path = f.root.path().join("energy.json");
        std::fs::write(&path,serde_json::to_vec(&serde_json::json!({"gamma":0.000001,"classes":[3,8],"n_support":[1,1],"support_vectors":[vec![0.;183],vec![1.;183]],"dual_coef":[[1.,-1.]],"intercept":[0.1]})).unwrap()).unwrap();
        settings.energy_model = Some(path);
        settings.lexicon_energy = false;
    });
    std::fs::write(f.root.path().join("energy.json"), b"changed model").unwrap();
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.processed, 1);
    assert_eq!(s.blocked, 1);
    let stages = f.lib.import_processing_stages(&op, None, 10).unwrap();
    assert_eq!(
        stages
            .records
            .iter()
            .find(|s| s.stage == "analysis.energy")
            .unwrap()
            .state,
        "blocked"
    );
    assert_eq!(
        stages
            .records
            .iter()
            .find(|s| s.stage == "analysis.key")
            .unwrap()
            .state,
        "proposed"
    );
    service.shutdown();
}

#[test]
fn cancellation_after_claim_leaves_a_cancelled_receipt_and_no_adoption() {
    use std::sync::{Arc, Barrier};
    let (f, op, id) = setup(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        true,
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    let service = Arc::new(ImportProcessingService::start(f.lib.clone(), runtime()).unwrap());
    let claimed = Arc::new(Barrier::new(2));
    let proceed = Arc::new(Barrier::new(2));
    let a = claimed.clone();
    let b = proceed.clone();
    service.set_claim_hook(Box::new(move || {
        a.wait();
        b.wait();
    }));
    let token = CancellationToken::default();
    let worker_token = token.clone();
    let worker = service.clone();
    let join = std::thread::spawn(move || worker.run_pending(&worker_token));
    claimed.wait();
    token.cancel();
    proceed.wait();
    let s = join.join().unwrap().unwrap();
    assert_eq!(s.cancelled, 1);
    assert_eq!(s.processed, 0);
    let rows = f.lib.import_processing_stages(&op, None, 10).unwrap();
    assert_eq!(rows.records.len(), 1);
    assert_eq!(rows.records[0].state, "cancelled");
    assert!(rows.records[0].proposal.is_none());
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    service.shutdown();
    let restarted = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    assert_eq!(
        restarted
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        0
    );
    restarted.shutdown();
}
#[test]
fn interrupted_proposal_attempt_is_abandoned_and_retried_once() {
    let (f, op, id) = setup(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        false,
    );
    f.lib.conn().unwrap().execute("INSERT INTO import_processing_receipts VALUES(?1,?2,'analysis.energy','interrupted','running','{}','{}',1)",rusqlite::params![op,id]).unwrap();
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        1
    );
    let records = f
        .lib
        .import_processing_stages(&op, None, 10)
        .unwrap()
        .records;
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].state, "abandoned");
    assert_eq!(records[1].state, "proposed");
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        0
    );
    service.shutdown();
}
#[test]
fn concurrent_processors_do_not_claim_the_same_stage_twice() {
    let (f, op, _) = setup(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        false,
    );
    let a = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let b = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let results = std::thread::scope(|scope| {
        let a = scope.spawn(|| a.run_pending(&CancellationToken::default()).unwrap());
        let b = scope.spawn(|| b.run_pending(&CancellationToken::default()).unwrap());
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(results.0.processed + results.1.processed, 1);
    assert_eq!(
        f.lib
            .import_processing_stages(&op, None, 100)
            .unwrap()
            .records
            .len(),
        1
    );
    a.shutdown();
    b.shutdown();
}
#[test]
fn unknown_measurement_is_blocked_without_erasing_the_imported_value() {
    let (f, op, id) = setup_custom(
        policy(AnalysisField::Key, AnalysisAdoption::ReplaceSelected),
        false,
        |_, _| {},
        |path| {
            let mut out = hound::WavWriter::create(
                path,
                hound::WavSpec {
                    channels: 2,
                    sample_rate: 44100,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for _ in 0..44100 * 16 * 2 {
                out.write_sample(0i16).unwrap();
            }
            out.finalize().unwrap();
        },
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.blocked, 1);
    assert_eq!(s.failed, 0);
    assert_eq!(
        f.lib
            .import_processing_stages(&op, None, 10)
            .unwrap()
            .records[0]
            .state,
        "blocked"
    );
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    service.shutdown();
}
#[test]
fn resumed_operations_keep_their_frozen_analysis_implementation() {
    let (f, op, id) = setup(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        false,
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    f.lib.conn().unwrap().execute("UPDATE import_previews SET json=json_set(json,'$.processing.engine_source_sha256',?1) WHERE id=(SELECT preview_id FROM import_operations WHERE id=?2)",rusqlite::params!["0".repeat(64),op]).unwrap();
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.blocked, 1);
    assert_eq!(s.processed, 0);
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    service.shutdown();
}

#[test]
fn decoder_identity_is_frozen_when_the_service_exists_before_preview() {
    let expected = runtime().identity;
    let (f, op, _) = setup_config(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        false,
        |f, _| {
            let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
            service.shutdown();
        },
    );
    let frozen:Option<String>=f.lib.conn().unwrap().query_row("SELECT json_extract(p.json,'$.processing.decoder_identity') FROM import_previews p JOIN import_operations o ON o.preview_id=p.id WHERE o.id=?1",[&op],|r|r.get(0)).unwrap();
    assert_eq!(frozen.as_deref(), Some(expected.as_str()));
}
#[test]
fn resumed_operation_does_not_switch_to_a_different_decoder() {
    let (f, op, id) = setup(
        policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected),
        false,
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    f.lib.conn().unwrap().execute("UPDATE import_previews SET json=json_set(json,'$.processing.decoder_identity','different-decoder-profile') WHERE id=(SELECT preview_id FROM import_operations WHERE id=?1)",[&op]).unwrap();
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.blocked, 1);
    assert_eq!(s.processed, 0);
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    service.shutdown();
}
#[test]
fn cache_body_identity_mismatch_is_recomputed_instead_of_reused() {
    let p = policy(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected);
    let (f, op, id) = setup(p.clone(), false);
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        1
    );
    f.lib.conn().unwrap().execute("UPDATE import_processing_receipts SET identity_json=json_set(identity_json,'$.pcm_sha256',?1) WHERE operation_id=?2",rusqlite::params!["0".repeat(64),op]).unwrap();
    let mut t = f.lib.imported_item(&id).unwrap().observations[0]
        .track
        .clone();
    t.metadata.insert(
        "title".into(),
        Presence::Value(serde_json::json!("Fresh title")),
    );
    let i = f.inspect(vec![t], Completeness::WholeLibrary);
    let mut r = f.request(&i);
    r.policy = p;
    f.apply(&f.preview(r));
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.reused, 0);
    assert_eq!(s.processed, 1);
    service.shutdown();
}

#[test]
fn database_reopen_preserves_completed_stage_and_recovers_only_the_interrupted_one() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let mut p = policy(AnalysisField::Key, AnalysisAdoption::ReplaceSelected);
    p.analysis_fields
        .insert(AnalysisField::Energy, AnalysisAdoption::ReplaceSelected);
    let (f, op, id) = setup(p, false);
    let before = f.lib.imported_item(&id).unwrap().active_fields;
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    service.set_claim_hook(Box::new(move || {
        if seen.fetch_add(1, Ordering::SeqCst) == 1 {
            panic!("injected interruption after the second stage claim");
        }
    }));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || service.run_pending(&CancellationToken::default())
        ))
        .is_err()
    );
    let first = f
        .lib
        .import_processing_stages(&op, None, 100)
        .unwrap()
        .records;
    let key_attempt = first
        .iter()
        .find(|s| s.stage == "analysis.key")
        .unwrap()
        .attempt
        .clone();
    assert_eq!(
        first
            .iter()
            .find(|s| s.stage == "analysis.energy")
            .unwrap()
            .state,
        "running"
    );
    service.shutdown();
    drop(service);
    let f = f.reopen();
    let restarted = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let s = restarted
        .run_pending(&CancellationToken::default())
        .unwrap();
    assert_eq!(s.processed, 1);
    let rows = f
        .lib
        .import_processing_stages(&op, None, 100)
        .unwrap()
        .records;
    assert_eq!(
        rows.iter()
            .find(|s| s.stage == "analysis.key")
            .unwrap()
            .attempt,
        key_attempt
    );
    assert_eq!(
        rows.iter()
            .filter(|s| s.stage == "analysis.energy" && s.state == "abandoned")
            .count(),
        1
    );
    assert_eq!(
        rows.iter()
            .filter(|s| s.stage == "analysis.energy" && s.state == "proposed")
            .count(),
        1
    );
    assert_eq!(f.lib.imported_item(&id).unwrap().active_fields, before);
    assert!(f.lib.jobs().unwrap().is_empty());
    restarted.shutdown();
}

#[test]
fn unavailable_media_blocks_requested_analysis_without_creating_a_decode_cache() {
    let f = Fixture::new();
    let i = f.inspect(
        vec![track("Unavailable reference")],
        Completeness::WholeLibrary,
    );
    let mut r = f.request(&i);
    r.policy = policy(AnalysisField::Key, AnalysisAdoption::ReplaceSelected);
    let op = f.apply(&f.preview(r));
    let id = f.lib.tracks(TrackQuery::default()).unwrap().rows[0]
        .id
        .clone();
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let s = service.run_pending(&CancellationToken::default()).unwrap();
    assert_eq!(s.blocked, 1);
    assert_eq!(s.failed, 0);
    assert_eq!(f.lib.track(&id).unwrap().title, "Unavailable reference");
    assert!(!f.lib.inner.root.join("import-media-cache").exists());
    assert!(f.lib.analysis(&id).unwrap().is_none());
    assert!(
        f.lib
            .import_processing_stages(&op.operation_id, None, 10)
            .unwrap()
            .records[0]
            .proposal
            .is_none()
    );
    service.shutdown();
}
#[test]
fn local_edit_after_claim_is_preserved_and_the_proposal_keeps_its_old_guard() {
    let (f, op, id) = setup(
        policy(AnalysisField::Key, AnalysisAdoption::ReplaceSelected),
        true,
    );
    let before = f.lib.imported_item(&id).unwrap().active_fields["key"].clone();
    let service = ImportProcessingService::start(f.lib.clone(), runtime()).unwrap();
    let lib = f.lib.clone();
    let target = id.clone();
    let local = serde_json::json!({"presence":"value","raw":"User choice","camelot":"11A"});
    let edit = local.clone();
    service.set_claim_hook(Box::new(move||{let conn=lib.conn().unwrap();conn.execute("UPDATE import_active_fields SET value_json=?1,revision=revision+1 WHERE item_id=?2 AND field='key'",rusqlite::params![edit.to_string(),target]).unwrap();super::super::projection::rebuild_summary_tx(&conn,&target).unwrap();Library::bump(&conn).unwrap();}));
    assert_eq!(
        service
            .run_pending(&CancellationToken::default())
            .unwrap()
            .processed,
        1
    );
    assert_eq!(
        f.lib.imported_item(&id).unwrap().active_fields["key"],
        local
    );
    let stage = f
        .lib
        .import_processing_stages(&op, None, 10)
        .unwrap()
        .records
        .remove(0);
    assert_eq!(stage.identity["expected_value"], before);
    assert_eq!(
        f.lib.imported_item(&id).unwrap().active_fields["cues.hot"][0]["name"],
        "Original"
    );
    service.shutdown();
}
