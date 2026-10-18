use cuerithm_library::{Library, PreparationService, PreparationSettings, TrackQuery};
use cuerithm_media::MediaRuntime;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn runtime() -> MediaRuntime {
    MediaRuntime::from_bundle(
        &std::env::var_os("CUERITHM_MEDIA_RUNTIME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
            }),
    )
    .unwrap()
}
fn configured(lib: &Library, folder: PathBuf) -> PreparationSettings {
    let model = lib.data_dir().join("test-model.json");
    std::fs::write(
        &model,
        serde_json::to_vec(&serde_json::json!({
            "gamma":0.000001,"classes":[3,8],"n_support":[1,1],
            "support_vectors":[vec![0.;183],vec![1.;183]],"dual_coef":[[1.,-1.]],"intercept":[0.1]
        }))
        .unwrap(),
    )
    .unwrap();
    let mut s = PreparationSettings {
        watch_folders: vec![folder],
        stability_millis: 20,
        ..Default::default()
    };
    s.engine.energy_model = Some(model);
    s
}
fn wait(lib: &Library, count: u64) {
    let start = Instant::now();
    loop {
        if lib.tracks(TrackQuery::default()).unwrap().total == count {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(60 * count.max(3)),
            "jobs: {:?}",
            lib.jobs().unwrap()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
#[test]
fn parallel_preparation_keeps_results_and_event_order_consistent() {
    let d = tempfile::tempdir().unwrap();
    let incoming = d.path().join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    for name in ["a.flac", "b.flac"] {
        std::fs::copy(&source, incoming.join(name)).unwrap();
    }
    let lib = Library::open(&d.path().join("app/db")).unwrap();
    let mut settings = configured(&lib, incoming);
    settings.stability_millis = 300;
    lib.save_settings(settings).unwrap();
    let service = PreparationService::start_with_workers(lib.clone(), runtime(), 2).unwrap();
    let events = service.subscribe();
    let start = Instant::now();
    loop {
        if lib
            .jobs()
            .unwrap()
            .iter()
            .filter(|j| j.state == "running")
            .count()
            == 2
        {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "two jobs must run concurrently"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    wait(&lib, 2);
    service.shutdown();
    let received: Vec<_> = events.try_iter().collect();
    assert!(
        received
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    let jobs = lib.jobs().unwrap();
    assert_eq!(jobs.len(), 2);
    let one = lib.analysis(&jobs[0].track_id).unwrap().unwrap();
    let two = lib.analysis(&jobs[1].track_id).unwrap().unwrap();
    for field in ["key", "tempo", "energy", "lexicon_cues"] {
        assert_eq!(
            serde_json::to_value(&one.analysis).unwrap()[field],
            serde_json::to_value(&two.analysis).unwrap()[field]
        );
    }
}

#[test]
fn all_common_formats_are_prepared_and_promoted_by_the_real_background_worker() {
    let d = tempfile::tempdir().unwrap();
    let incoming = d.path().join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let archive = std::fs::File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fixtures/media.tar.gz"),
    )
    .unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(&incoming)
        .unwrap();
    let lib = Library::open(&d.path().join("app/library.sqlite")).unwrap();
    let settings = configured(&lib, incoming);
    lib.save_settings(settings.clone()).unwrap();
    let service = PreparationService::start(lib.clone(), runtime()).unwrap();
    wait(&lib, 14);
    let jobs = lib.jobs().unwrap();
    assert_eq!(jobs.len(), 14);
    assert!(jobs.iter().all(|j| j.state == "completed"), "{jobs:?}");
    let old = lib.analysis(&jobs[0].track_id).unwrap().unwrap();
    let mut new = settings;
    new.engine
        .lexicon_config
        .as_mut()
        .unwrap()
        .min_cue_length_beats = 128;
    let revision = lib.save_settings(new).unwrap();
    service.retry(jobs[0].id.clone(), Some(revision)).unwrap();
    wait(&lib, 14);
    assert_eq!(lib.cue_revision_count(&jobs[0].track_id).unwrap(), 2);
    assert_ne!(
        lib.analysis(&jobs[0].track_id)
            .unwrap()
            .unwrap()
            .analysis
            .config_sha256,
        old.analysis.config_sha256
    );
}
#[test]
fn file_copy_changes_are_requeued_and_missing_setup_remains_in_incoming() {
    let d = tempfile::tempdir().unwrap();
    let incoming = d.path().join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let lib = Library::open(&d.path().join("app/library.sqlite")).unwrap();
    let path = incoming.join("copy.flac");
    let source = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
    )
    .unwrap();
    std::fs::write(&path, &source[..512]).unwrap();
    lib.save_settings(configured(&lib, incoming.clone()))
        .unwrap();
    let service = PreparationService::start(lib.clone(), runtime()).unwrap();
    service.enqueue_paths(std::slice::from_ref(&path)).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    std::fs::write(&path, &source).unwrap();
    wait(&lib, 1);
    assert_eq!(
        lib.jobs().unwrap().len(),
        1,
        "superseded copies do not duplicate the track"
    );
    let mut settings = configured(&lib, incoming);
    settings.autotag_enabled = true;
    let rev = lib.save_settings(settings).unwrap();
    let job = lib.jobs().unwrap()[0].id.clone();
    service.retry(job, Some(rev)).unwrap();
    let start = Instant::now();
    loop {
        if lib.jobs().unwrap().iter().any(|j| j.state == "attention") {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 0);
    assert!(
        lib.analysis(&lib.jobs().unwrap()[0].track_id)
            .unwrap()
            .is_some()
    );
}

#[test]
fn offline_watch_folder_reappears_and_interrupted_jobs_resume_after_reopen() {
    let d = tempfile::tempdir().unwrap();
    let incoming = d.path().join("later");
    let db = d.path().join("app/library.sqlite");
    let lib = Library::open(&db).unwrap();
    lib.save_settings(configured(&lib, incoming.clone()))
        .unwrap();
    let service = PreparationService::start(lib.clone(), runtime()).unwrap();
    let events = service.subscribe();
    std::thread::sleep(Duration::from_millis(250));
    assert!(!service.status().watch_errors.is_empty());
    std::fs::create_dir(&incoming).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    std::fs::copy(&source, incoming.join("new.flac")).unwrap();
    wait(&lib, 1);
    let mut last = 0;
    while let Ok(event) = events.try_recv() {
        assert!(event.sequence > last);
        last = event.sequence;
        if event.state == "completed" {
            assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 1);
        }
    }
    assert!(last > 0);
    drop(service);
    let second = incoming.join("second.flac");
    std::fs::copy(source, &second).unwrap();
    let rev = lib.settings().unwrap().revision;
    lib.enqueue(&[second], rev).unwrap();
    lib.claim_next().unwrap().unwrap();
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let _service = PreparationService::start(lib.clone(), runtime()).unwrap();
    wait(&lib, 2);
    assert_eq!(lib.jobs().unwrap().len(), 2);
}
