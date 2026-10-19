#![cfg(feature = "autotag")]
use cuerithm_autotag::{
    AnalysisTagSource, AutotagConfig, CatalogMatch, CatalogTrack, ProviderBackend, ProviderId,
    TagInput, Tagger,
};
use cuerithm_library::{Library, PreparationService, PreparationSettings};
use cuerithm_media::{MediaRuntime, file_sha256};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
struct Catalog {
    calls: AtomicUsize,
    fail: AtomicBool,
    empty: bool,
}
impl ProviderBackend for Catalog {
    fn id(&self) -> ProviderId {
        ProviderId::Beatport
    }
    fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            anyhow::bail!("HTTP 403 fixture");
        }
        Ok(if self.empty {
            vec![]
        } else {
            vec![CatalogMatch::new(
                1.,
                CatalogTrack {
                    platform: "beatport".into(),
                    title: "Catalog title".into(),
                    artists: vec!["Catalog artist".into()],
                    genres: vec!["House".into()],
                    key: Some("F#m".into()),
                    bpm: Some(199),
                    ..Default::default()
                },
            )]
        })
    }
}
fn setup() -> (
    tempfile::TempDir,
    Library,
    PathBuf,
    MediaRuntime,
    PreparationSettings,
) {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("app/db")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = d.path().join("source.flac");
    std::fs::copy(
        root.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &source,
    )
    .unwrap();
    let model = lib.data_dir().join("test-model.json");
    std::fs::write(&model,serde_json::to_vec(&serde_json::json!({"gamma":0.000001,"classes":[3,8],"n_support":[1,1],"support_vectors":[vec![0.;183],vec![1.;183]],"dual_coef":[[1.,-1.]],"intercept":[0.1]})).unwrap()).unwrap();
    let mut settings = PreparationSettings {
        stability_millis: 20,
        autotag_enabled: true,
        ..Default::default()
    };
    settings.engine.energy_model = Some(model);
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["beatport".into()];
    config.options.tags = vec![
        cuerithm_autotag::SupportedTag::Title,
        cuerithm_autotag::SupportedTag::Artist,
        cuerithm_autotag::SupportedTag::BPM,
        cuerithm_autotag::SupportedTag::Key,
    ];
    config.analysis_tag_source = AnalysisTagSource::Catalog;
    settings.tagging = Some(config);
    let runtime = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    (d, lib, source, runtime, settings)
}
const TRACK_WAIT_SECONDS: u64 = 120;
fn until(lib: &Library, done: impl FnMut() -> bool) {
    until_with_timeout(Duration::from_secs(TRACK_WAIT_SECONDS), lib, done);
}
fn until_with_timeout(timeout: Duration, lib: &Library, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < timeout,
            "preparation timed out after {timeout:?}; jobs: {:?}",
            lib.jobs()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn configured_tagging_is_automatic_and_does_not_replace_measured_analysis() {
    let (_d, lib, source, rt, settings) = setup();
    lib.save_settings(settings).unwrap();
    let catalog = Arc::new(Catalog {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(false),
        empty: false,
    });
    let tagger = Arc::new(Tagger::with_backends(vec![catalog.clone()]).unwrap());
    let service =
        PreparationService::start_with_tagger(lib.clone(), rt.clone(), 2, tagger).unwrap();
    let events = service.subscribe();
    service
        .enqueue_paths(std::slice::from_ref(&source))
        .unwrap();
    until(&lib, || ready(&lib));
    service.shutdown();
    let row = lib.tracks(Default::default()).unwrap().rows.remove(0);
    assert_eq!(row.title, "Catalog title");
    assert!(row.bpm.unwrap() < 150.);
    assert_ne!(row.bpm, Some(199.));
    assert_eq!(row.source_sha256, file_sha256(&source).unwrap());
    let metadata = rt.probe(&source).unwrap();
    assert_eq!(
        metadata
            .tags
            .get("bpm")
            .or_else(|| metadata.tags.get("tbpm"))
            .map(String::as_str),
        Some("199.00")
    );
    assert_eq!(lib.cue_revision_count(&row.id).unwrap(), 1);
    assert_eq!(catalog.calls.load(Ordering::SeqCst), 1);
    let stages: Vec<_> = events.try_iter().map(|e| e.stage).collect();
    for pair in [
        ["analysis", "matching"],
        ["matching", "tagging"],
        ["tagging", "writing_tags"],
        ["writing_tags", "completed"],
    ] {
        assert!(
            stages.iter().position(|s| s == pair[0]).unwrap()
                < stages.iter().position(|s| s == pair[1]).unwrap()
        );
    }
    assert_eq!(
        lib.enqueue_discovered(
            std::slice::from_ref(&source),
            lib.settings().unwrap().revision
        )
        .unwrap()
        .len(),
        1
    );
    assert!(lib.claim_next().unwrap().is_none());
}
#[test]
fn disabled_tagging_remains_offline_and_no_match_obeys_policy() {
    for enabled in [false, true] {
        let (_d, lib, source, rt, mut settings) = setup();
        settings.autotag_enabled = enabled;
        let original = file_sha256(&source).unwrap();
        lib.save_settings(settings).unwrap();
        let catalog = Arc::new(Catalog {
            calls: AtomicUsize::new(0),
            fail: AtomicBool::new(false),
            empty: true,
        });
        let tagger = Arc::new(Tagger::with_backends(vec![catalog.clone()]).unwrap());
        let service = PreparationService::start_with_tagger(lib.clone(), rt, 1, tagger).unwrap();
        service
            .enqueue_paths(std::slice::from_ref(&source))
            .unwrap();
        until(&lib, || ready(&lib));
        service.shutdown();
        assert_eq!(catalog.calls.load(Ordering::SeqCst), usize::from(enabled));
        if !enabled {
            assert_eq!(file_sha256(&source).unwrap(), original);
        }
    }
}
#[test]
fn provider_failure_keeps_original_and_ready_history_recoverable() {
    let (_d, lib, source, rt, settings) = setup();
    lib.save_settings(settings).unwrap();
    let catalog = Arc::new(Catalog {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(true),
        empty: false,
    });
    let tagger = Arc::new(Tagger::with_backends(vec![catalog.clone()]).unwrap());
    let original = file_sha256(&source).unwrap();
    let service = PreparationService::start_with_tagger(lib.clone(), rt, 1, tagger).unwrap();
    let events = service.subscribe();
    let ids = service
        .enqueue_paths(std::slice::from_ref(&source))
        .unwrap();
    until(&lib, || {
        lib.jobs().unwrap().iter().any(|j| j.state == "attention")
    });
    assert_eq!(lib.counts().unwrap().tracks, 0);
    assert_eq!(file_sha256(&source).unwrap(), original);
    catalog.fail.store(false, Ordering::SeqCst);
    service.retry(ids[0].clone(), None).unwrap();
    until(&lib, || ready(&lib));
    service.shutdown();
    assert_eq!(lib.counts().unwrap().incoming, 0);
    assert_eq!(
        events.try_iter().filter(|e| e.stage == "analysis").count(),
        1,
        "unchanged provider retry must reuse completed analysis"
    );
}

fn ready(lib: &Library) -> bool {
    let jobs = lib.jobs().unwrap();
    assert!(
        !jobs.iter().any(|j| j.state == "attention"),
        "Preparation failed: {jobs:?}"
    );
    lib.counts().unwrap().tracks == 1
}
#[test]
fn edits_during_provider_wait_are_reanalyzed_before_any_publication() {
    struct Waiting {
        started: AtomicBool,
        release: AtomicBool,
    }
    impl ProviderBackend for Waiting {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
            self.started.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(vec![CatalogMatch::new(
                1.,
                CatalogTrack {
                    platform: "beatport".into(),
                    title: "Tagged after edit".into(),
                    ..Default::default()
                },
            )])
        }
    }
    let (_d, lib, source, rt, settings) = setup();
    lib.save_settings(settings).unwrap();
    let backend = Arc::new(Waiting {
        started: AtomicBool::new(false),
        release: AtomicBool::new(false),
    });
    let tagger = Arc::new(Tagger::with_backends(vec![backend.clone()]).unwrap());
    let service = PreparationService::start_with_tagger(lib.clone(), rt, 1, tagger).unwrap();
    service
        .enqueue_paths(std::slice::from_ref(&source))
        .unwrap();
    until(&lib, || backend.started.load(Ordering::SeqCst));
    {
        let mut wrapper = onetagger_tag::Tag::load_file(&source, true).unwrap();
        let tag = wrapper.tag_mut();
        tag.set_field(onetagger_tag::Field::Title, vec!["User edit".into()], true);
        tag.save_file(&source).unwrap();
    }
    let user_hash = file_sha256(&source).unwrap();
    backend.release.store(true, Ordering::SeqCst);
    until(&lib, || ready(&lib));
    service.shutdown();
    let row = lib.tracks(Default::default()).unwrap().rows.remove(0);
    assert_eq!(lib.cue_revision_count(&row.id).unwrap(), 1);
    let before: String = rusqlite::Connection::open(lib.data_dir().join("db"))
        .unwrap()
        .query_row(
            "SELECT before_hash FROM tag_writes WHERE state='committed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, user_hash);
}
#[test]
fn no_match_attention_policy_does_not_promote_or_change_file() {
    let (_d, lib, source, rt, mut settings) = setup();
    settings.tagging.as_mut().unwrap().no_match = cuerithm_autotag::NoMatchPolicy::Attention;
    lib.save_settings(settings).unwrap();
    let hash = file_sha256(&source).unwrap();
    let catalog = Arc::new(Catalog {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(false),
        empty: true,
    });
    let service = PreparationService::start_with_tagger(
        lib.clone(),
        rt,
        1,
        Arc::new(Tagger::with_backends(vec![catalog]).unwrap()),
    )
    .unwrap();
    service
        .enqueue_paths(std::slice::from_ref(&source))
        .unwrap();
    until(&lib, || {
        lib.jobs().unwrap().iter().any(|j| j.state == "attention")
    });
    service.shutdown();
    assert_eq!(file_sha256(&source).unwrap(), hash);
    assert_eq!(lib.counts().unwrap().tracks, 0);
    assert!(
        lib.jobs().unwrap()[0]
            .error
            .as_ref()
            .unwrap()
            .contains("review required")
    );
}
#[test]
fn saved_tagging_configuration_survives_reopen() {
    let (_d, lib, _source, _rt, mut settings) = setup();
    settings.tagging.as_mut().unwrap().options.strictness = 0.93;
    settings.tagging.as_mut().unwrap().write_energy = false;
    let revision = lib.save_settings(settings).unwrap();
    let db = lib.data_dir().join("db");
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let settings = lib.settings().unwrap();
    assert_eq!(settings.revision, revision);
    let config = settings.settings.tagging.unwrap();
    assert_eq!(config.options.strictness, 0.93);
    assert!(!config.write_energy);
    assert!(settings.settings.autotag_enabled);
}

#[test]
fn album_matching_reuses_the_selected_release_for_only_the_incoming_batch() {
    struct AlbumCatalog {
        searches: AtomicUsize,
        albums: AtomicUsize,
    }
    fn row() -> CatalogTrack {
        CatalogTrack {
            platform: "beatport".into(),
            title: "Signal".into(),
            artists: vec!["Fixture Artist".into()],
            release_id: Some("fixture-release".into()),
            duration: Duration::from_secs(64),
            ..Default::default()
        }
    }
    impl ProviderBackend for AlbumCatalog {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
            self.searches.fetch_add(1, Ordering::SeqCst);
            Ok(vec![CatalogMatch::new(1., row())])
        }
        fn album(
            &self,
            id: &str,
            _: &AutotagConfig,
        ) -> anyhow::Result<Option<cuerithm_autotag::CatalogAlbum>> {
            assert_eq!(id, "fixture-release");
            self.albums.fetch_add(1, Ordering::SeqCst);
            Ok(Some(cuerithm_autotag::CatalogAlbum {
                id: id.into(),
                name: "Fixture".into(),
                tracks: vec![row()],
            }))
        }
    }
    let (_d, lib, source, rt, mut settings) = setup();
    settings.tagging.as_mut().unwrap().options.album_tagging = true;
    settings
        .tagging
        .as_mut()
        .unwrap()
        .options
        .album_tagging_ratio = 0.3;
    lib.save_settings(settings).unwrap();
    let mut paths = Vec::new();
    for i in 0..3 {
        let path = source.with_file_name(format!("{i}.flac"));
        std::fs::copy(&source, &path).unwrap();
        let mut wrapper = onetagger_tag::Tag::load_file(&path, true).unwrap();
        let tag = wrapper.tag_mut();
        tag.set_field(onetagger_tag::Field::Title, vec!["Signal".into()], true);
        tag.set_field(
            onetagger_tag::Field::Artist,
            vec!["Fixture Artist".into()],
            true,
        );
        tag.save_file(&path).unwrap();
        paths.push(path);
    }
    let catalog = Arc::new(AlbumCatalog {
        searches: AtomicUsize::new(0),
        albums: AtomicUsize::new(0),
    });
    let service = PreparationService::start_with_tagger(
        lib.clone(),
        rt,
        1,
        Arc::new(Tagger::with_backends(vec![catalog.clone()]).unwrap()),
    )
    .unwrap();
    service.enqueue_paths(&paths).unwrap();
    // One worker prepares three complete 64-second fixtures sequentially.
    // Keep the same per-track budget used by the single-track tests.
    until_with_timeout(
        Duration::from_secs(TRACK_WAIT_SECONDS * paths.len() as u64),
        &lib,
        || {
            assert!(
                !lib.jobs().unwrap().iter().any(|j| j.state == "attention"),
                "{:?}",
                lib.jobs().unwrap()
            );
            lib.counts().unwrap().tracks == 3
        },
    );
    service.shutdown();
    assert_eq!(catalog.searches.load(Ordering::SeqCst), 1);
    assert_eq!(catalog.albums.load(Ordering::SeqCst), 1);
    for row in lib.tracks(Default::default()).unwrap().rows {
        assert_eq!(lib.cue_revision_count(&row.id).unwrap(), 1);
    }
}

#[test]
fn supersession_after_journal_insert_does_not_block_the_replacement_attempt() {
    let (_d, lib, source, rt, settings) = setup();
    lib.save_settings(settings).unwrap();
    rusqlite::Connection::open(lib.data_dir().join("db")).unwrap().execute_batch("CREATE TRIGGER supersede_first_write AFTER INSERT ON tag_writes WHEN (SELECT COUNT(*) FROM tag_writes)=1 BEGIN UPDATE jobs SET state='queued',stage='queued',attempts=attempts+1 WHERE id=NEW.job_id; END;").unwrap();
    let catalog = Arc::new(Catalog {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(false),
        empty: false,
    });
    let service = PreparationService::start_with_tagger(
        lib.clone(),
        rt,
        1,
        Arc::new(Tagger::with_backends(vec![catalog]).unwrap()),
    )
    .unwrap();
    service.enqueue_paths(&[source]).unwrap();
    until(&lib, || ready(&lib));
    service.shutdown();
    let conn = rusqlite::Connection::open(lib.data_dir().join("db")).unwrap();
    let pending: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM tag_writes WHERE state='prepared'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pending, 0);
    assert_eq!(
        lib.cue_revision_count(&lib.tracks(Default::default()).unwrap().rows[0].id)
            .unwrap(),
        1
    );
}

#[test]
fn analysis_cache_survives_restart_and_settings_changes_invalidate_it() {
    let (_d, lib, source, rt, settings) = setup();
    lib.save_settings(settings.clone()).unwrap();
    let catalog = Arc::new(Catalog {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(true),
        empty: false,
    });
    let tagger = Arc::new(Tagger::with_backends(vec![catalog.clone()]).unwrap());
    let service =
        PreparationService::start_with_tagger(lib.clone(), rt.clone(), 1, tagger.clone()).unwrap();
    let id = service.enqueue_paths(&[source]).unwrap().remove(0);
    until(&lib, || {
        lib.jobs().unwrap().iter().any(|j| j.state == "attention")
    });
    service.shutdown();
    drop(service);
    let db = lib.data_dir().join("db");
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let service = PreparationService::start_with_tagger(lib.clone(), rt, 1, tagger).unwrap();
    let events = service.subscribe();
    service.retry(id.clone(), None).unwrap();
    until(&lib, || {
        catalog.calls.load(Ordering::SeqCst) >= 2
            && lib.jobs().unwrap().iter().any(|j| j.state == "attention")
    });
    assert_eq!(
        events.try_iter().filter(|e| e.stage == "analysis").count(),
        0,
        "restart should reuse full analysis"
    );
    let mut changed = settings;
    changed
        .engine
        .lexicon_config
        .as_mut()
        .unwrap()
        .min_cue_length_beats = 96;
    let revision = lib.save_settings(changed).unwrap();
    catalog.fail.store(false, Ordering::SeqCst);
    service.retry(id, Some(revision)).unwrap();
    until(&lib, || ready(&lib));
    service.shutdown();
    assert_eq!(
        events.try_iter().filter(|e| e.stage == "analysis").count(),
        1,
        "new settings require new analysis"
    );
}

#[test]
fn explicit_analyze_again_recomputes_an_unchanged_offline_track() {
    let (_d, lib, source, rt, mut settings) = setup();
    settings.autotag_enabled = false;
    lib.save_settings(settings).unwrap();
    let catalog = Arc::new(Catalog {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(false),
        empty: false,
    });
    let service = PreparationService::start_with_tagger(
        lib.clone(),
        rt,
        1,
        Arc::new(Tagger::with_backends(vec![catalog.clone()]).unwrap()),
    )
    .unwrap();
    let events = service.subscribe();
    let before = file_sha256(&source).unwrap();
    service
        .enqueue_paths(std::slice::from_ref(&source))
        .unwrap();
    until(&lib, || ready(&lib));
    let row = lib.tracks(Default::default()).unwrap().rows.remove(0);
    service.reprepare(row.id.clone()).unwrap();
    until(&lib, || lib.cue_revision_count(&row.id).unwrap() == 2);
    service.shutdown();
    assert_eq!(
        events.try_iter().filter(|e| e.stage == "analysis").count(),
        2
    );
    assert_eq!(file_sha256(&source).unwrap(), before);
    assert_eq!(catalog.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn lexicon_timing_cache_survives_restart_but_a_backend_change_recomputes() {
    let (_d, lib, source, rt, mut settings) = setup();
    settings.engine.source_profile_timing = false;
    settings.engine.lexicon_timing = true;
    settings.engine.energy_model = None;
    settings.engine.lexicon_energy = true;
    lib.save_settings(settings.clone()).unwrap();
    let catalog = Arc::new(Catalog {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(true),
        empty: false,
    });
    let tagger = Arc::new(Tagger::with_backends(vec![catalog.clone()]).unwrap());
    let service =
        PreparationService::start_with_tagger(lib.clone(), rt.clone(), 1, tagger.clone()).unwrap();
    let id = service.enqueue_paths(&[source]).unwrap().remove(0);
    until(&lib, || {
        lib.jobs().unwrap().iter().any(|j| j.state == "attention")
    });
    service.shutdown();
    drop(service);
    let db = lib.data_dir().join("db");
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let service = PreparationService::start_with_tagger(lib.clone(), rt, 1, tagger).unwrap();
    let events = service.subscribe();
    service.retry(id.clone(), None).unwrap();
    until(&lib, || {
        catalog.calls.load(Ordering::SeqCst) >= 2
            && lib.jobs().unwrap().iter().any(|j| j.state == "attention")
    });
    assert_eq!(
        events.try_iter().filter(|e| e.stage == "analysis").count(),
        0,
        "restart should reuse full analysis"
    );
    let mut changed = settings;
    changed.engine.lexicon_timing = false;
    changed.engine.source_profile_timing = true;
    let revision = lib.save_settings(changed).unwrap();
    catalog.fail.store(false, Ordering::SeqCst);
    service.retry(id, Some(revision)).unwrap();
    until(&lib, || ready(&lib));
    service.shutdown();
    let row = lib.tracks(Default::default()).unwrap().rows.remove(0);
    assert_ne!(
        lib.analysis(&row.id)
            .unwrap()
            .unwrap()
            .analysis
            .tempo
            .unwrap()
            .method,
        "lexicon-timing-v1"
    );
    assert_eq!(
        events.try_iter().filter(|e| e.stage == "analysis").count(),
        1,
        "switching timing backends must invalidate the cached analysis"
    );
}

#[test]
fn reanalysis_with_tagging_disabled_clears_the_previous_tag_status() {
    let (_d, lib, source, rt, settings) = setup();
    lib.save_settings(settings.clone()).unwrap();
    let catalog = Arc::new(Catalog {
        calls: AtomicUsize::new(0),
        fail: AtomicBool::new(false),
        empty: false,
    });
    let service = PreparationService::start_with_tagger(
        lib.clone(),
        rt,
        1,
        Arc::new(Tagger::with_backends(vec![catalog]).unwrap()),
    )
    .unwrap();
    service
        .enqueue_paths(std::slice::from_ref(&source))
        .unwrap();
    until(&lib, || ready(&lib));
    let row = lib.tracks(Default::default()).unwrap().rows.remove(0);
    assert!(row.metadata.contains_key("library.tag_status"));
    let mut off = settings;
    off.autotag_enabled = false;
    lib.save_settings(off).unwrap();
    service.reprepare(row.id.clone()).unwrap();
    until(&lib, || lib.cue_revision_count(&row.id).unwrap() == 2);
    service.shutdown();
    assert!(
        !lib.track(&row.id)
            .unwrap()
            .metadata
            .contains_key("library.tag_status")
    );
}
