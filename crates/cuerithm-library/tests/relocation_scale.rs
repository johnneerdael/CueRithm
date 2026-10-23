//! Explicit synthetic resource acceptance; never reads user music.
use cuerithm_library::{Library, RelocationRequest};
use cuerithm_media::{CancellationToken, MediaRuntime};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[test]
#[ignore = "Creates 1,000,001 files; run directly under /usr/bin/time -l"]
fn million_flat_candidates_resume_and_cancel() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let search = root.join("flat");
    std::fs::create_dir(&search).unwrap();
    let library = Library::open(&root.join("library.sqlite")).unwrap();
    let runtime = MediaRuntime::from_bundle(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin"),
    )
    .expect("Bundled media runtime required for scale acceptance");
    let source = root.join("target.flac");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../cuerithm-media/tests/fixtures/covered.flac"),
        &source,
    )
    .unwrap();
    let jobs = library
        .enqueue(
            std::slice::from_ref(&source),
            library.settings().unwrap().revision,
        )
        .unwrap();
    let job = library
        .jobs()
        .unwrap()
        .into_iter()
        .find(|j| j.id == jobs[0])
        .unwrap();
    let run = library.claim_next().unwrap().unwrap();
    library
        .fail_for_run(&run, "test", "Synthetic scale fixture")
        .unwrap();
    std::fs::remove_file(source).unwrap();
    let authored = Instant::now();
    for i in 0..1_000_001usize {
        std::fs::File::create(search.join(format!("candidate-{i:07}.flac"))).unwrap();
    }
    eprintln!(
        "authored_files=1000001 flat_directory=true creation_seconds={:.3}",
        authored.elapsed().as_secs_f64()
    );
    let request = RelocationRequest {
        mode: "selection".into(),
        track_ids: vec![job.track_id],
        search_root: Some(search.clone()),
        ..Default::default()
    };
    let started = Instant::now();
    let mut last = Instant::now();
    let mut longest = Duration::ZERO;
    let plan = library
        .plan_relocation(
            request.clone(),
            &runtime,
            &CancellationToken::default(),
            |p| {
                longest = longest.max(last.elapsed());
                last = Instant::now();
                if p.phase == "searching" {
                    eprintln!(
                        "progress candidates={} elapsed={:.3}",
                        p.completed,
                        started.elapsed().as_secs_f64()
                    );
                }
            },
        )
        .unwrap();
    assert!(plan.paused && plan.incomplete);
    assert!(
        plan.rows
            .iter()
            .all(|r| r.candidates.iter().all(|c| !c.preselected))
    );
    eprintln!(
        "first_cap_seconds={:.3} max_progress_gap_seconds={:.3}",
        started.elapsed().as_secs_f64(),
        longest.as_secs_f64()
    );
    assert!(longest < Duration::from_secs(1));
    // Add the valid fixture after the first cap, guaranteeing it was not indexed.
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../cuerithm-media/tests/fixtures/covered.flac"),
        search.join("target.flac"),
    )
    .unwrap();
    let continued = Instant::now();
    let resumed = library
        .continue_relocation(&plan.id, &runtime, &CancellationToken::default(), |_| {})
        .unwrap();
    assert!(!resumed.paused && !resumed.incomplete);
    assert_eq!(resumed.rows[0].candidates.len(), 1);
    assert!(resumed.rows[0].candidates[0].preselected);
    eprintln!(
        "resume_seconds={:.3} final_target_verified=true",
        continued.elapsed().as_secs_f64()
    );
    let cancellation = CancellationToken::default();
    let started = Instant::now();
    let result = library.plan_relocation(request, &runtime, &cancellation, |p| {
        if p.phase == "searching" && p.completed > 0 {
            cancellation.cancel();
        }
    });
    assert!(result.is_err());
    eprintln!(
        "cancellation_seconds={:.3}",
        started.elapsed().as_secs_f64()
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    eprintln!("total_authored_audio_paths=1000002 cleanup=task_owned_tempdir");
}

#[cfg(unix)]
#[test]
fn incomplete_search_requires_review_for_unverified_filename_match() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let database = root.join("library.sqlite");
    let library = Library::open(&database).unwrap();
    let runtime = MediaRuntime::from_bundle(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin"),
    )
    .expect("Bundled media runtime required for relocation acceptance");
    let source = root.join("target.flac");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../cuerithm-media/tests/fixtures/covered.flac"),
        &source,
    )
    .unwrap();
    let jobs = library
        .enqueue(
            std::slice::from_ref(&source),
            library.settings().unwrap().revision,
        )
        .unwrap();
    let job = library
        .jobs()
        .unwrap()
        .into_iter()
        .find(|j| j.id == jobs[0])
        .unwrap();
    let run = library.claim_next().unwrap().unwrap();
    library
        .fail_for_run(&run, "test", "Synthetic acceptance fixture")
        .unwrap();
    rusqlite::Connection::open(&database)
        .unwrap()
        .execute(
            "UPDATE tracks SET source_hash=NULL WHERE id=?1",
            [&job.track_id],
        )
        .unwrap();
    let search = root.join("search");
    std::fs::create_dir(&search).unwrap();
    std::fs::rename(source, search.join("target.flac")).unwrap();
    std::os::unix::fs::symlink(&root, search.join("unsearched")).unwrap();
    let plan = library
        .plan_relocation(
            RelocationRequest {
                mode: "selection".into(),
                track_ids: vec![job.track_id],
                search_root: Some(search),
                ..Default::default()
            },
            &runtime,
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap();
    assert!(plan.incomplete);
    assert_eq!(plan.rows[0].candidates.len(), 1);
    assert!(!plan.rows[0].candidates[0].preselected);
}
