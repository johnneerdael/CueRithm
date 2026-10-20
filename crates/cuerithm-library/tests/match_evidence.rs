#![cfg(feature = "autotag")]

use cuerithm_autotag::{AutotagConfig, MatchStatus, TagInput, TagMatchOutcome};
use cuerithm_library::{JobRun, Library, PreparationSettings};
use cuerithm_media::MediaInfo;

fn setup() -> (tempfile::TempDir, Library, JobRun, TagInput) {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(&dir.path().join("data/library.db")).unwrap();
    let source = dir.path().join("source.flac");
    std::fs::write(
        &source,
        b"synthetic source identity, no decoding in this test",
    )
    .unwrap();
    let settings = PreparationSettings {
        autotag_enabled: true,
        tagging: Some(AutotagConfig::default()),
        ..Default::default()
    };
    let revision = library.save_settings(settings).unwrap();
    library.enqueue(&[source], revision).unwrap();
    let run = library.claim_next().unwrap().unwrap();
    let input = TagInput {
        path: run.path.clone(),
        source_sha256: run.source_sha256.clone(),
        duration_seconds: 240.,
        media: MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            tags: [("title".into(), "Synthetic recording".into())].into(),
            properties: Default::default(),
        },
    };
    (dir, library, run, input)
}

fn no_match() -> TagMatchOutcome {
    TagMatchOutcome {
        status: MatchStatus::NoMatch,
        matches: vec![],
        attempted: vec![],
        warnings: vec![],
    }
}

#[test]
fn evidence_survives_attention_and_restart_with_attempt_and_source_identity() {
    let (dir, library, run, input) = setup();
    library
        .record_match_outcome(&run, &input, &no_match())
        .unwrap();
    library
        .fail_for_run(&run, "matching", "No catalog match")
        .unwrap();
    let saved = library
        .preparation_match_snapshot(&run.id)
        .unwrap()
        .unwrap();
    assert_eq!(saved.source_sha256, run.source_sha256);
    assert_eq!(saved.settings_revision, run.settings_revision);
    assert_eq!(saved.attempt, run.attempt);
    assert_eq!(saved.track_id, run.track_id);
    assert_eq!(saved.outcome["status"], "no_match");
    assert!(
        !serde_json::to_string(&saved)
            .unwrap()
            .contains("source.flac")
    );
    drop(library);
    let reopened = Library::open(&dir.path().join("data/library.db")).unwrap();
    assert_eq!(
        reopened
            .preparation_match_snapshot(&run.id)
            .unwrap()
            .unwrap()
            .attempt,
        run.attempt
    );
}

#[test]
fn superseded_attempt_cannot_replace_current_matching_evidence() {
    let (_dir, library, old, input) = setup();
    library
        .record_match_outcome(&old, &input, &no_match())
        .unwrap();
    library
        .fail_for_run(&old, "matching", "Review needed")
        .unwrap();
    library.retry(&old.id, None).unwrap();
    let current = library.claim_next().unwrap().unwrap();
    library
        .record_match_outcome(&current, &input, &no_match())
        .unwrap();
    assert!(
        library
            .record_match_outcome(&old, &input, &no_match())
            .is_err()
    );
    assert_eq!(
        library
            .preparation_match_snapshot(&current.id)
            .unwrap()
            .unwrap()
            .attempt,
        current.attempt
    );
}

#[test]
fn changed_or_mismatched_source_cannot_publish_evidence() {
    let (_dir, library, run, mut input) = setup();
    input.source_sha256 = "wrong".into();
    assert!(
        library
            .record_match_outcome(&run, &input, &no_match())
            .is_err()
    );
    input.source_sha256 = run.source_sha256.clone();
    std::fs::write(&run.path, b"changed original").unwrap();
    assert!(
        library
            .record_match_outcome(&run, &input, &no_match())
            .is_err()
    );
    assert!(
        library
            .preparation_match_snapshot(&run.id)
            .unwrap()
            .is_none()
    );
}
