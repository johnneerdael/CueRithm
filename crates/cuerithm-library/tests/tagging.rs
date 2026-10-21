#![cfg(feature = "autotag")]
use camelot_core::{
    AnalysisConfig, FileAnalysis, KeyEstimate, Mode, MusicalKey, TempoEstimate,
    energy::TrackEnergy,
    lexicon_cues::{StructuralCue, StructuralCueResult},
    prepared::PreparedMediaIdentity,
};
use cuerithm_autotag::{
    AnalysisTags, AutotagConfig, CatalogMatch, CatalogTrack, MatchStatus, StagedTagWrite,
    SupportedTag, TagInput, TagMatchOutcome, TagWritePlan,
};
use cuerithm_library::{JobRun, Library, PreparationSettings};
use cuerithm_media::{MediaRuntime, file_sha256};
use std::path::{Path, PathBuf};
struct Fixture {
    _dir: tempfile::TempDir,
    db: PathBuf,
    source: PathBuf,
    lib: Library,
    rt: MediaRuntime,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("app/library.sqlite");
        let source = dir.path().join("music/source.flac");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::copy(
            root.join("crates/cuerithm-media/tests/fixtures/metadata.flac"),
            &source,
        )
        .unwrap();
        let lib = Library::open(&db).unwrap();
        lib.save_settings(PreparationSettings {
            autotag_enabled: true,
            ..Default::default()
        })
        .unwrap();
        let rt = MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
        Self {
            _dir: dir,
            db,
            source,
            lib,
            rt,
        }
    }
    fn prepare(&self, title: &str) -> (JobRun, StagedTagWrite, FileAnalysis) {
        self.prepare_with_art(title, false)
    }
    fn prepare_with_art(
        &self,
        title: &str,
        external_art: bool,
    ) -> (JobRun, StagedTagWrite, FileAnalysis) {
        self.lib
            .enqueue(
                std::slice::from_ref(&self.source),
                self.lib.settings().unwrap().revision,
            )
            .unwrap();
        let run = self.lib.claim_next().unwrap().unwrap();
        let audio = self.rt.decode(&self.source, self.lib.data_dir()).unwrap();
        let input = TagInput {
            path: self.source.clone(),
            source_sha256: audio.source_sha256.clone(),
            duration_seconds: audio.clock.duration_seconds(),
            media: audio.info.clone(),
        };
        let outcome = TagMatchOutcome {
            status: MatchStatus::Matched,
            matches: vec![CatalogMatch::new(
                1.,
                CatalogTrack {
                    platform: "beatport".into(),
                    title: title.into(),
                    ..Default::default()
                },
            )],
            attempted: vec![],
            warnings: vec![],
        };
        let mut config = AutotagConfig::default();
        config.options.tags = vec![SupportedTag::Title];
        if external_art {
            config.options.tags.push(SupportedTag::AlbumArt);
            config.options.album_art_file = true;
        }
        let mut plan =
            TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default()).unwrap();
        if external_art {
            plan = plan
                .with_artwork(
                    std::fs::read(
                        Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("../../apps/desktop/src-tauri/icons/32x32.png"),
                    )
                    .unwrap(),
                )
                .unwrap();
        }
        let staged = plan
            .stage_prepared(&self.rt, &audio, self.lib.data_dir())
            .unwrap();
        let mut a =
            camelot_core::analyze_pcm(&vec![0.; 5512], 11025, &AnalysisConfig::default()).unwrap();
        a.duration_seconds = audio.clock.duration_seconds();
        a.key = Some(KeyEstimate {
            key: MusicalKey::new(0, Mode::Major).unwrap(),
            camelot: "8B".into(),
            margin: 0.5,
            features: None,
            candidates: vec![],
        });
        a.tempo = Some(TempoEstimate {
            bpm: 120.,
            strength: None,
            candidates: vec![],
            beats_seconds: vec![0., 0.25],
            downbeats_seconds: None,
            downbeat_time: Some(0.),
            method: "test".into(),
        });
        a.energy = Some(TrackEnergy {
            level: 6,
            method: "test".into(),
        });
        a.lexicon_cues = Some(StructuralCueResult {
            cues: vec![StructuralCue {
                time_seconds: 0.,
                name: "Start".into(),
                color: "Red".into(),
            }],
            emergency_loop: None,
        });
        let result = FileAnalysis {
            publication: None,
            analysis: a,
            complete: true,
            verified_integrity: None,
            source_sample_rate: 44100,
            beat_sample_rate: None,
            neural_key_sample_rate: None,
            openkeyscan_sample_rate: None,
            source_timing: None,
            elapsed_seconds: 0.,
            media: Some(PreparedMediaIdentity {
                export_timing: None,
                cue_genre: None,
                decoder: audio.decoder_identity.clone(),
                codec: audio.info.codec.clone(),
                source_sha256: audio.source_sha256.clone(),
                pcm_sha256: audio.pcm_sha256().unwrap(),
                clock: audio.clock.clone(),
            }),
        };
        (run, staged, result)
    }
}
#[test]
fn publication_commits_current_identity_metadata_and_one_cue_revision() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let before = run.source_sha256.clone();
    let after = staged.source_after_sha256.clone();
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    assert_eq!(file_sha256(&f.source).unwrap(), before);
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
    f.lib.publish_preparation(&run, &journal).unwrap();
    assert_eq!(file_sha256(&f.source).unwrap(), after);
    assert_eq!(f.lib.source_hash(&run.track_id).unwrap(), after);
    assert_eq!(f.lib.track(&run.track_id).unwrap().title, "New title");
    assert_eq!(f.lib.cue_revision_count(&run.track_id).unwrap(), 1);
    assert_eq!(
        f.lib
            .enqueue_discovered(
                std::slice::from_ref(&f.source),
                run.settings_revision.clone()
            )
            .unwrap(),
        vec![run.id.clone()]
    );
    assert!(f.lib.claim_next().unwrap().is_none());
}
#[test]
fn committed_publication_reports_the_catalog_outcome_on_the_track_summary() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    assert!(
        !f.lib
            .track(&run.track_id)
            .unwrap()
            .metadata
            .contains_key("library.tag_status")
    );
    f.lib.publish_preparation(&run, &journal).unwrap();
    let listed = f
        .lib
        .tracks(Default::default())
        .unwrap()
        .rows
        .into_iter()
        .find(|t| t.id == run.track_id)
        .unwrap();
    assert_eq!(listed.metadata["library.tag_status"], "matched");
    assert_eq!(
        f.lib.track(&run.track_id).unwrap().metadata["library.tag_status"],
        "matched"
    );
}
#[test]
fn obsolete_attempt_and_user_edits_cannot_be_overwritten() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    f.lib.retry(&run.id, None).unwrap();
    let next = f.lib.claim_next().unwrap().unwrap();
    assert!(next.attempt > run.attempt);
    assert!(f.lib.publish_preparation(&run, &journal).is_err());
    assert_eq!(file_sha256(&f.source).unwrap(), run.source_sha256);
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    std::fs::write(&f.source, b"new user edit").unwrap();
    assert!(f.lib.publish_preparation(&run, &journal).is_err());
    assert_eq!(std::fs::read(&f.source).unwrap(), b"new user edit");
    assert_eq!(f.lib.cue_revision_count(&run.track_id).unwrap(), 0);
}
#[test]
fn restart_before_or_after_replacement_rolls_back_and_requeues() {
    for replaced in [false, true] {
        let f = Fixture::new();
        let (run, mut staged, result) = f.prepare("New title");
        let _journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
        if replaced {
            std::fs::copy(&staged.path, &f.source).unwrap();
        }
        let db = f.db.clone();
        let source = f.source.clone();
        drop(f.lib);
        let lib = Library::open(&db).unwrap();
        assert_eq!(file_sha256(&source).unwrap(), run.source_sha256);
        assert_eq!(lib.counts().unwrap().tracks, 0);
        assert_eq!(lib.cue_revision_count(&run.track_id).unwrap(), 0);
        assert!(lib.claim_next().unwrap().is_some());
    }
}
#[test]
fn recovery_preserves_later_user_edits_and_backups() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let _journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    std::fs::copy(&staged.path, &f.source).unwrap();
    std::fs::write(&f.source, b"later user edit").unwrap();
    let backup = staged.backup_path.clone();
    let db = f.db.clone();
    let source = f.source.clone();
    drop(f.lib);
    let lib = Library::open(&db).unwrap();
    assert_eq!(std::fs::read(source).unwrap(), b"later user edit");
    assert_eq!(file_sha256(&backup).unwrap(), run.source_sha256);
    assert_eq!(lib.cue_revision_count(&run.track_id).unwrap(), 0);
    assert_eq!(lib.jobs().unwrap()[0].state, "attention");
}
#[test]
fn database_failure_restores_file_and_previous_cue_state() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    rusqlite::Connection::open(&f.db).unwrap().execute_batch("CREATE TRIGGER fail_cue BEFORE INSERT ON cue_revisions BEGIN SELECT RAISE(ABORT,'injected commit failure');END;").unwrap();
    assert!(f.lib.publish_preparation(&run, &journal).is_err());
    assert_eq!(file_sha256(&f.source).unwrap(), run.source_sha256);
    assert_eq!(f.lib.cue_revision_count(&run.track_id).unwrap(), 0);
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
}
#[test]
fn restoring_pre_tag_source_can_prepare_again_without_identity_collision() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    f.lib.publish_preparation(&run, &journal).unwrap();
    std::fs::copy(&staged.backup_path, &f.source).unwrap();
    let (again, mut second, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&again, &mut second, &result).unwrap();
    f.lib.publish_preparation(&again, &journal).unwrap();
    assert_eq!(f.lib.cue_revision_count(&again.track_id).unwrap(), 2);
}
#[test]
fn failed_copy_releases_journal_ownership_instead_of_blocking_retries() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    std::fs::write(&staged.path, b"damaged stage").unwrap();
    assert!(f.lib.publish_preparation(&run, &journal).is_err());
    let state: String = rusqlite::Connection::open(&f.db)
        .unwrap()
        .query_row("SELECT state FROM tag_writes WHERE id=?1", [journal], |r| {
            r.get(0)
        })
        .unwrap();
    assert_ne!(state, "prepared");
    assert_eq!(file_sha256(&f.source).unwrap(), run.source_sha256);
}
#[test]
fn rollback_restores_audio_and_cover_together_and_keeps_earlier_cues() {
    let f = Fixture::new();
    let (run, mut first, result) = f.prepare("First");
    let journal = f.lib.begin_tag_write(&run, &mut first, &result).unwrap();
    f.lib.publish_preparation(&run, &journal).unwrap();
    let original = file_sha256(&f.source).unwrap();
    let cover = f.source.parent().unwrap().join("cover.jpg");
    std::fs::write(&cover, b"original cover").unwrap();
    let (run, mut staged, result) = f.prepare_with_art("Second", true);
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    rusqlite::Connection::open(&f.db).unwrap().execute_batch("CREATE TRIGGER fail_second BEFORE INSERT ON cue_revisions BEGIN SELECT RAISE(ABORT,'injected');END;").unwrap();
    assert!(f.lib.publish_preparation(&run, &journal).is_err());
    assert_eq!(file_sha256(&f.source).unwrap(), original);
    assert_eq!(std::fs::read(&cover).unwrap(), b"original cover");
    assert_eq!(f.lib.cue_revision_count(&run.track_id).unwrap(), 1);
    assert!(f.lib.analysis(&run.track_id).unwrap().is_some());
}
#[test]
fn archive_and_damaged_backup_reject_publication() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    std::fs::write(&staged.backup_path, b"damaged backup").unwrap();
    assert!(f.lib.begin_tag_write(&run, &mut staged, &result).is_err());
    assert_eq!(file_sha256(&f.source).unwrap(), run.source_sha256);
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    f.lib.set_archived(&run.track_id, true).unwrap();
    assert!(f.lib.publish_preparation(&run, &journal).is_err());
    assert_eq!(file_sha256(&f.source).unwrap(), run.source_sha256);
}
#[test]
fn schema_one_upgrades_without_dropping_existing_tables_or_metadata() {
    for old_version in [1, 2, 3, 4] {
        let d = tempfile::tempdir().unwrap();
        let db = d.path().join("legacy.sqlite");
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(include_str!("../migrations/001_initial.sql"))
            .unwrap();
        if old_version >= 2 {
            conn.execute_batch(include_str!("../migrations/002_tag_writes.sql"))
                .unwrap();
        }
        if old_version >= 3 {
            conn.execute_batch(include_str!("../migrations/003_preparation_cache.sql"))
                .unwrap();
        }
        if old_version == 4 {
            conn.execute_batch(include_str!("../migrations/004_cleanup.sql"))
                .unwrap();
        }
        conn.execute("INSERT INTO tracks(id,path,title,source_hash,created,updated) VALUES('old',?1,'Legacy title','hash',1,1)",[b"unix:/legacy.flac".to_vec()]).unwrap();
        drop(conn);
        let _lib = Library::open(&db).unwrap();
        let conn = rusqlite::Connection::open(&db).unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 22);
        let title: String = conn
            .query_row("SELECT title FROM tracks WHERE id='old'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Legacy title");
    }
}

#[test]
fn watcher_does_not_enqueue_an_owned_intermediate_tag_write() {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    std::fs::copy(&staged.path, &f.source).unwrap();
    let ids = f
        .lib
        .enqueue_discovered(
            std::slice::from_ref(&f.source),
            run.settings_revision.clone(),
        )
        .unwrap();
    assert_eq!(ids, vec![run.id.clone()]);
    assert_eq!(f.lib.jobs().unwrap()[0].attempts, run.attempt);
}

fn assert_finished_stage_cleanup(fail: bool) {
    let f = Fixture::new();
    let (run, mut staged, result) = f.prepare("New title");
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    if fail {
        rusqlite::Connection::open(&f.db).unwrap().execute_batch("CREATE TRIGGER fail_cleanup_test BEFORE INSERT ON cue_revisions BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    }
    assert_eq!(f.lib.publish_preparation(&run, &journal).is_err(), fail);
    assert!(
        !staged.path.exists(),
        "redundant full staged audio must be removed"
    );
    assert_eq!(file_sha256(&staged.backup_path).unwrap(), run.source_sha256);
    let storage = f.lib.tagging_storage().unwrap();
    assert_eq!(storage.original_files, 1);
    assert_eq!(
        storage.original_bytes,
        std::fs::metadata(&staged.backup_path).unwrap().len()
    );
    assert_eq!(storage.pending_cleanup, 0);
    let complete: bool = rusqlite::Connection::open(&f.db)
        .unwrap()
        .query_row(
            "SELECT cleanup_complete FROM tag_writes WHERE id=?1",
            [&journal],
            |r| r.get(0),
        )
        .unwrap();
    assert!(complete);
    let db = f.db.clone();
    drop(f.lib);
    for _ in 0..2 {
        let lib = Library::open(&db).unwrap();
        assert!(!staged.path.exists());
        drop(lib);
    }
}
#[test]
fn committed_publication_cleans_staged_audio_and_records_completion() {
    assert_finished_stage_cleanup(false);
}
#[test]
fn rolled_back_publication_cleans_staged_audio_and_records_completion() {
    assert_finished_stage_cleanup(true);
}
#[test]
fn no_op_publication_retains_no_redundant_audio_copies() {
    let f = Fixture::new();
    let (run, old_staged, result) = f.prepare("Unused");
    drop(old_staged);
    let audio = f.rt.decode(&f.source, f.lib.data_dir()).unwrap();
    let input = TagInput {
        path: f.source.clone(),
        source_sha256: audio.source_sha256.clone(),
        media: audio.info.clone(),
        duration_seconds: audio.clock.duration_seconds(),
    };
    let mut config = AutotagConfig::default();
    config.options.tags.clear();
    config.write_energy = false;
    let outcome = TagMatchOutcome {
        status: MatchStatus::NoMatch,
        matches: vec![],
        attempted: vec![],
        warnings: vec![],
    };
    let plan = TagWritePlan::build(input, &outcome, &config, &AnalysisTags::default()).unwrap();
    let mut staged = plan
        .stage_prepared(&f.rt, &audio, f.lib.data_dir())
        .unwrap();
    assert_eq!(staged.source_before_sha256, staged.source_after_sha256);
    let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
    f.lib.publish_preparation(&run, &journal).unwrap();
    assert!(!staged.path.exists());
    assert!(!staged.backup_path.exists());
    assert_eq!(file_sha256(&f.source).unwrap(), run.source_sha256);
}

#[test]
fn organization_tag_checkpoint_survives_every_tag_publication_restart_gap() {
    // Intent only, native publication before SQLite commit, and committed tags
    // before organization all resume without an energy model/provider/DSP pass.
    for gap in 0..4 {
        let f = Fixture::new();
        let mut settings = f.lib.settings().unwrap().settings;
        settings.organization = cuerithm_library::OrganizationSettings {
            mode: cuerithm_library::OrganizationSettingMode::RenameInPlace,
            pattern: "{original_name}".into(),
            ..Default::default()
        };
        f.lib.save_settings(settings).unwrap();
        let (run, mut staged, result) = f.prepare("Checkpoint final title");
        let output = staged.source_after_sha256.clone();
        let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
        let sql = rusqlite::Connection::open(&f.db).unwrap();
        assert_eq!(
            sql.query_row(
                "SELECT state FROM preparation_checkpoints WHERE job_id=?1",
                [&run.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "tagging",
            "the resumable intent must exist before audio bytes publish"
        );
        drop(sql);
        if gap == 1 {
            std::fs::copy(&staged.path, &f.source).unwrap();
        }
        if gap == 3 {
            let sql = rusqlite::Connection::open(&f.db).unwrap();
            let encoded: Vec<u8> = sql
                .query_row(
                    "SELECT publish_path FROM tag_write_files WHERE write_id=?1 AND ordinal=0",
                    [&journal],
                    |r| r.get(0),
                )
                .unwrap();
            // A rolled-back publication can leave its known before bytes in
            // the old private path. A retry must reserve a fresh attempt path.
            std::fs::copy(
                &f.source,
                cuerithm_library::organization::NativePath(encoded)
                    .path()
                    .unwrap(),
            )
            .unwrap();
        }

        if gap == 2 {
            f.lib.publish_preparation(&run, &journal).unwrap();
            assert_eq!(f.lib.track(&run.track_id).unwrap().state, "incoming");
        }
        drop(staged);
        let Fixture {
            _dir,
            db,
            source,
            lib,
            rt,
        } = f;
        drop(lib);
        let lib = Library::open(&db).unwrap();
        let service =
            cuerithm_library::PreparationService::start_with_workers(lib.clone(), rt, 1).unwrap();
        let start = std::time::Instant::now();
        loop {
            let job = lib
                .jobs()
                .unwrap()
                .into_iter()
                .find(|j| j.id == run.id)
                .unwrap();
            if job.state == "completed" {
                break;
            }
            assert_ne!(job.state, "attention", "gap {gap}: {:?}", job);
            assert!(
                start.elapsed() < std::time::Duration::from_secs(10),
                "gap {gap}: {:?}",
                job
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        service.shutdown();
        assert_eq!(file_sha256(&source).unwrap(), output, "gap {gap}");
        assert_eq!(
            lib.cue_revision_count(&run.track_id).unwrap(),
            1,
            "gap {gap}"
        );
        assert_eq!(lib.track(&run.track_id).unwrap().state, "ready");
        drop(_dir);
    }
}

#[test]
fn solver_only_retry_keeps_prepared_and_tagging_journal_revisions() {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };
    struct NoCatalog(AtomicUsize);
    impl cuerithm_autotag::ProviderBackend for NoCatalog {
        fn id(&self) -> cuerithm_autotag::ProviderId {
            cuerithm_autotag::ProviderId::Beatport
        }
        fn candidates(&self, _: &TagInput, _: &AutotagConfig) -> anyhow::Result<Vec<CatalogMatch>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            anyhow::bail!("accepted checkpoint must not contact a catalog")
        }
    }
    for prepared in [false, true] {
        let f = Fixture::new();
        let mut settings = f.lib.settings().unwrap().settings;
        settings.tagging = Some(AutotagConfig::default());
        settings.organization = cuerithm_library::OrganizationSettings {
            mode: cuerithm_library::OrganizationSettingMode::RenameInPlace,
            pattern: "{original_name}".into(),
            ..Default::default()
        };
        f.lib.save_settings(settings.clone()).unwrap();
        let (run, mut staged, result) = f.prepare("Accepted checkpoint title");
        let output = staged.source_after_sha256.clone();
        let journal = f.lib.begin_tag_write(&run, &mut staged, &result).unwrap();
        if prepared {
            f.lib.publish_preparation(&run, &journal).unwrap();
        }
        f.lib
            .fail_for_run(&run, "organizing", "fixture interruption")
            .unwrap();
        let before = file_sha256(&f.source).unwrap();
        let mut engine_change = settings.clone();
        engine_change.engine.sample_rate = 22050;
        engine_change.tagging.as_mut().unwrap().solver.endpoint =
            Some("http://127.0.0.1:8191".into());
        let engine_revision = f.lib.save_settings(engine_change).unwrap();
        assert!(
            f.lib.retry(&run.id, Some(engine_revision)).is_err(),
            "real analysis changes must not replace an accepted checkpoint"
        );
        assert_eq!(file_sha256(&f.source).unwrap(), before);
        settings.tagging.as_mut().unwrap().solver.endpoint = Some("http://127.0.0.1:8191".into());
        settings.tagging.as_mut().unwrap().solver.timeout_seconds = 41;
        let transport_revision = f.lib.save_settings(settings).unwrap();
        assert_ne!(transport_revision, run.settings_revision);
        f.lib
            .retry(&run.id, Some(transport_revision))
            .expect("transport-only retry must retain the accepted tag journal");
        let sql = rusqlite::Connection::open(&f.db).unwrap();
        let revisions:(String,String,String)=sql.query_row("SELECT j.settings_revision,p.settings_revision,w.settings_revision FROM jobs j JOIN preparation_checkpoints p ON p.job_id=j.id JOIN tag_writes w ON w.id=?1 WHERE j.id=?2",rusqlite::params![journal,run.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(
            revisions,
            (
                run.settings_revision.clone(),
                run.settings_revision.clone(),
                run.settings_revision.clone()
            )
        );
        drop(sql);
        let catalog = Arc::new(NoCatalog(AtomicUsize::new(0)));
        let service = cuerithm_library::PreparationService::start_with_tagger(
            f.lib.clone(),
            f.rt.clone(),
            1,
            Arc::new(cuerithm_autotag::Tagger::with_backends(vec![catalog.clone()]).unwrap()),
        )
        .unwrap();
        let started = Instant::now();
        loop {
            let job = f
                .lib
                .jobs()
                .unwrap()
                .into_iter()
                .find(|j| j.id == run.id)
                .unwrap();
            if job.state == "completed" {
                break;
            }
            assert_ne!(job.state, "attention", "{job:?}");
            assert!(
                started.elapsed() < Duration::from_secs(10),
                "checkpoint retry timed out: {job:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        service.shutdown();
        assert_eq!(catalog.0.load(Ordering::SeqCst), 0);
        assert_eq!(file_sha256(&f.source).unwrap(), output);
        assert_eq!(f.lib.cue_revision_count(&run.track_id).unwrap(), 1);
        assert_eq!(
            f.lib.track(&run.track_id).unwrap().title,
            "Accepted checkpoint title"
        );
    }
}
