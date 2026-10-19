#![cfg(feature = "autotag")]
use cuerithm_autotag::{
    AutotagConfig, CatalogMatch, CatalogTrack, ProviderBackend, ProviderId, SupportedTag, TagInput,
    Tagger,
};
use cuerithm_cleanup::{
    ArtistRules, CanonicalMapping, CleanupConfig, ConfiguredFix, GenreConversion, GenreRules,
    MetadataField, SmartFix,
};
use cuerithm_library::{Library, PreparationService, PreparationSettings};
use cuerithm_media::{MediaRuntime, file_sha256};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
fn rules() -> CleanupConfig {
    CleanupConfig {
        fixes: vec![
            ConfiguredFix {
                id: "spaces".into(),
                enabled: true,
                ignored_values: Default::default(),
                rule: SmartFix::ReplaceCharacters {
                    characters: "_".into(),
                },
            },
            ConfiguredFix {
                id: "garbage".into(),
                enabled: true,
                ignored_values: Default::default(),
                rule: SmartFix::RemoveGarbage {
                    field: MetadataField::Title,
                },
            },
        ],
        genres: GenreRules {
            enabled: true,
            mappings: vec![CanonicalMapping {
                from: "Deep House".into(),
                to: Some("House".into()),
            }],
            conversion: Some(GenreConversion {
                category: "Subgenres".into(),
                genres: ["Deep House".into()].into(),
                remove_genre: false,
            }),
            ..Default::default()
        },
        artists: ArtistRules {
            enabled: true,
            mappings: vec![CanonicalMapping {
                from: "Alias".into(),
                to: Some("Canonical".into()),
            }],
            ..Default::default()
        },
    }
}
fn setup(tagging: bool) -> (tempfile::TempDir, Library, PathBuf, MediaRuntime) {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("app/db")).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = dir.path().join("source.flac");
    std::fs::copy(
        root.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &source,
    )
    .unwrap();
    let mut wrapper = onetagger_tag::Tag::load_file(&source, true).unwrap();
    {
        let tag = wrapper.tag_mut();
        tag.set_field(
            onetagger_tag::Field::Title,
            vec!["Original__Title".into()],
            true,
        );
        tag.set_field(onetagger_tag::Field::Artist, vec!["Alias".into()], true);
        tag.set_field(onetagger_tag::Field::Genre, vec!["Deep House".into()], true);
        tag.save_file(&source).unwrap();
    }
    let model = lib.data_dir().join("test-energy.json");
    std::fs::write(&model,serde_json::to_vec(&serde_json::json!({"gamma":0.000001,"classes":[3,8],"n_support":[1,1],"support_vectors":[vec![0.;183],vec![1.;183]],"dual_coef":[[1.,-1.]],"intercept":[0.1]})).unwrap()).unwrap();
    let mut config = AutotagConfig::default();
    config.options.platforms = vec!["beatport".into()];
    config.options.tags = vec![
        SupportedTag::Title,
        SupportedTag::Artist,
        SupportedTag::Genre,
    ];
    config.options.overwrite = true;
    config.write_energy = false;
    let mut settings = PreparationSettings {
        stability_millis: 20,
        autotag_enabled: tagging,
        tagging: Some(config),
        cleanup_enabled: true,
        cleanup: Some(rules()),
        ..Default::default()
    };
    settings.engine.energy_model = Some(model);
    lib.save_settings(settings).unwrap();
    let rt = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    (dir, lib, source, rt)
}
struct Catalog {
    calls: AtomicUsize,
}
impl ProviderBackend for Catalog {
    fn id(&self) -> ProviderId {
        ProviderId::Beatport
    }
    fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(vec![CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "beatport".into(),
                title: "Catalog__Title".into(),
                artists: vec!["Alias".into()],
                genres: vec!["Deep House".into()],
                ..Default::default()
            },
        )])
    }
}
fn until(mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "cleanup preparation timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn saved_cleanup_rules_survive_restart_and_required_cleanup_has_a_configuration() {
    let (dir, lib, _, _) = setup(false);
    let settings = lib.settings().unwrap();
    assert!(settings.settings.ready().is_ok());
    let mut bad = settings.settings.clone();
    bad.cleanup = None;
    assert!(bad.ready().is_err());
    drop(lib);
    let lib = Library::open(&dir.path().join("app/db")).unwrap();
    assert_eq!(
        serde_json::to_value(lib.settings().unwrap().settings.cleanup).unwrap(),
        serde_json::to_value(Some(rules())).unwrap()
    );
}
#[test]
fn offline_cleanup_and_catalog_cleanup_publish_once_with_measured_analysis() {
    for tagging in [false, true] {
        let (_dir, lib, path, rt) = setup(tagging);
        let before = file_sha256(&path).unwrap();
        lib.enqueue(
            std::slice::from_ref(&path),
            lib.settings().unwrap().revision,
        )
        .unwrap();
        let track = lib.jobs().unwrap()[0].track_id.clone();
        let conn = rusqlite::Connection::open(lib.data_dir().join("db")).unwrap();
        conn.execute_batch("INSERT INTO custom_tag_categories VALUES('mood','Mood');INSERT INTO custom_tags VALUES('peak','mood','Peak');").unwrap();
        conn.execute("INSERT INTO track_custom_tags VALUES(?1,'peak')", [&track])
            .unwrap();
        drop(conn);
        let backend = Arc::new(Catalog {
            calls: AtomicUsize::new(0),
        });
        let service = PreparationService::start_with_tagger(
            lib.clone(),
            rt.clone(),
            1,
            Arc::new(Tagger::with_backends(vec![backend.clone()]).unwrap()),
        )
        .unwrap();
        service.enqueue_paths(std::slice::from_ref(&path)).unwrap();
        until(|| {
            assert!(
                !lib.jobs().unwrap().iter().any(|j| j.state == "attention"),
                "{:?}",
                lib.jobs().unwrap()
            );
            lib.counts().unwrap().tracks == 1
        });
        service.shutdown();
        let row = lib.track(&track).unwrap();
        assert_eq!(
            row.title,
            if tagging {
                "Catalog Title"
            } else {
                "Original Title"
            }
        );
        assert_eq!(row.artist, "Canonical");
        assert_eq!(row.genre, "House");
        assert!(row.key.is_some() && row.bpm.is_some() && row.energy.is_some());
        assert_ne!(file_sha256(&path).unwrap(), before);
        assert_eq!(lib.cue_revision_count(&track).unwrap(), 1);
        assert_eq!(backend.calls.load(Ordering::SeqCst), usize::from(tagging));
        let tags = lib.track_custom_tags(&track).unwrap();
        assert!(
            tags.iter()
                .any(|t| t.category == "Mood" && t.value == "Peak")
        );
        assert!(
            tags.iter()
                .any(|t| t.category == "Subgenres" && t.value == "Deep House")
        );
        let conn = rusqlite::Connection::open(lib.data_dir().join("db")).unwrap();
        let (writes,report,provenance):(u64,String,Option<String>)=conn.query_row("SELECT (SELECT COUNT(*) FROM tag_writes WHERE state='committed'),cleanup_json,tagging_json FROM tracks WHERE id=?1",[&track],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(writes, 1);
        let report: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(
            report["metadata"]["fields"]["genre"],
            serde_json::json!(["House"])
        );
        if tagging {
            let raw: serde_json::Value = serde_json::from_str(&provenance.unwrap()).unwrap();
            assert_eq!(
                raw["matches"][0]["track"]["genres"],
                serde_json::json!(["Deep House"])
            );
        } else {
            assert!(provenance.is_none());
        }
        lib.enqueue_discovered(
            std::slice::from_ref(&path),
            lib.settings().unwrap().revision,
        )
        .unwrap();
        assert_eq!(lib.jobs().unwrap().len(), 1);
        assert_eq!(lib.counts().unwrap().tracks, 1);
        assert_eq!(lib.cue_revision_count(&track).unwrap(), 1);
    }
}

#[test]
fn protected_values_and_noop_rules_keep_their_explicit_policy() {
    for noop in [false, true] {
        let (_dir, lib, path, rt) = setup(false);
        let mut settings = lib.settings().unwrap().settings;
        if noop {
            settings.cleanup = Some(CleanupConfig::default());
        } else {
            let rules = settings.cleanup.as_mut().unwrap();
            rules.genres.protected_values.insert("Deep House".into());
            rules.artists.protected_values.insert("Alias".into());
        }
        lib.save_settings(settings).unwrap();
        let before = std::fs::read(&path).unwrap();
        let backend = Arc::new(Catalog {
            calls: AtomicUsize::new(0),
        });
        let service = PreparationService::start_with_tagger(
            lib.clone(),
            rt,
            1,
            Arc::new(Tagger::with_backends(vec![backend.clone()]).unwrap()),
        )
        .unwrap();
        service.enqueue_paths(std::slice::from_ref(&path)).unwrap();
        until(|| {
            assert!(
                !lib.jobs().unwrap().iter().any(|j| j.state == "attention"),
                "{:?}",
                lib.jobs().unwrap()
            );
            lib.counts().unwrap().tracks == 1
        });
        service.shutdown();
        let row = lib.tracks(Default::default()).unwrap().rows.remove(0);
        assert_eq!(row.artist, "Alias");
        assert_eq!(row.genre, "Deep House");
        assert!(lib.track_custom_tags(&row.id).unwrap().is_empty());
        assert_eq!(backend.calls.load(Ordering::SeqCst), 0);
        if noop {
            assert_eq!(std::fs::read(path).unwrap(), before);
            assert_eq!(lib.tagging_storage().unwrap().original_files, 0);
        }
    }
}
#[test]
fn custom_tag_race_rolls_back_and_retries_without_recomputing_analysis() {
    let (_dir, lib, path, rt) = setup(false);
    lib.enqueue(
        std::slice::from_ref(&path),
        lib.settings().unwrap().revision,
    )
    .unwrap();
    let track = lib.jobs().unwrap()[0].track_id.clone();
    let conn = rusqlite::Connection::open(lib.data_dir().join("db")).unwrap();
    conn.execute_batch("INSERT INTO custom_tag_categories VALUES('mood','Mood');INSERT INTO custom_tags VALUES('peak','mood','Peak');CREATE TRIGGER mutate_tags AFTER INSERT ON tag_writes WHEN (SELECT COUNT(*) FROM tag_writes)=1 BEGIN UPDATE custom_tags SET name='Later value' WHERE id='peak'; END;").unwrap();
    conn.execute("INSERT INTO track_custom_tags VALUES(?1,'peak')", [&track])
        .unwrap();
    drop(conn);
    let before = std::fs::read(&path).unwrap();
    let service = PreparationService::start_with_workers(lib.clone(), rt.clone(), 1).unwrap();
    let events = service.subscribe();
    service.enqueue_paths(std::slice::from_ref(&path)).unwrap();
    until(|| lib.jobs().unwrap().iter().any(|j| j.state == "attention"));
    service.shutdown();
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(lib.cue_revision_count(&track).unwrap(), 0);
    assert_eq!(
        lib.track_custom_tags(&track).unwrap()[0].value,
        "Later value"
    );
    assert!(events.try_iter().any(|e| e.stage == "analysis"));
    rusqlite::Connection::open(lib.data_dir().join("db"))
        .unwrap()
        .execute_batch("DROP TRIGGER mutate_tags;")
        .unwrap();
    let id = lib.jobs().unwrap()[0].id.clone();
    lib.retry(&id, None).unwrap();
    let service = PreparationService::start_with_workers(lib.clone(), rt, 1).unwrap();
    let events = service.subscribe();
    service.enqueue_paths(std::slice::from_ref(&path)).unwrap();
    until(|| lib.counts().unwrap().tracks == 1);
    service.shutdown();
    assert!(!events.try_iter().any(|e| e.stage == "analysis"));
    assert_eq!(lib.cue_revision_count(&track).unwrap(), 1);
    assert!(
        lib.track_custom_tags(&track)
            .unwrap()
            .iter()
            .any(|t| t.value == "Later value")
    );
}
#[test]
fn superseded_settings_cannot_publish_older_cleanup_results() {
    use std::sync::atomic::AtomicBool;
    struct SlowCatalog {
        entered: Arc<AtomicBool>,
        release: Arc<AtomicBool>,
    }
    impl ProviderBackend for SlowCatalog {
        fn id(&self) -> ProviderId {
            ProviderId::Beatport
        }
        fn candidates(
            &self,
            input: &TagInput,
            config: &AutotagConfig,
        ) -> anyhow::Result<Vec<CatalogMatch>> {
            self.entered.store(true, Ordering::SeqCst);
            let start = Instant::now();
            while !self.release.load(Ordering::SeqCst) {
                anyhow::ensure!(
                    start.elapsed() < Duration::from_secs(15),
                    "fixture wait expired"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            Catalog {
                calls: AtomicUsize::new(0),
            }
            .candidates(input, config)
        }
    }
    let (_dir, lib, path, rt) = setup(true);
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let tagger = Arc::new(
        Tagger::with_backends(vec![Arc::new(SlowCatalog {
            entered: entered.clone(),
            release: release.clone(),
        })])
        .unwrap(),
    );
    let service = PreparationService::start_with_tagger(lib.clone(), rt, 1, tagger).unwrap();
    service.enqueue_paths(std::slice::from_ref(&path)).unwrap();
    until(|| entered.load(Ordering::SeqCst));
    let mut settings = lib.settings().unwrap().settings;
    settings.cleanup.as_mut().unwrap().artists.mappings[0].to = Some("New canonical".into());
    lib.save_settings(settings).unwrap();
    service.enqueue_paths(std::slice::from_ref(&path)).unwrap();
    release.store(true, Ordering::SeqCst);
    until(|| {
        assert!(
            !lib.jobs().unwrap().iter().any(|j| j.state == "attention"),
            "{:?}",
            lib.jobs().unwrap()
        );
        lib.counts().unwrap().tracks == 1
    });
    service.shutdown();
    let row = lib.tracks(Default::default()).unwrap().rows.remove(0);
    assert_eq!(row.artist, "New canonical");
    assert_eq!(lib.cue_revision_count(&row.id).unwrap(), 1);
}
