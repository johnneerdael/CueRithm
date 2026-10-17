use camelot_source_analysis::{analyze_file, analyze_timing_file, source_sha256};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn timing_only_matches_request_timing_without_classifying_results() {
    let path = fixture("pulses64-stereo16.flac");
    let timing = analyze_timing_file(&path).unwrap();
    let features = analyze_file(&path).unwrap();
    assert_eq!(timing.bpm.to_bits(), features.analysis.tempo.to_bits());
    assert_eq!(
        timing.downbeat_time.to_bits(),
        features.analysis.downbeat_time.to_bits()
    );
    assert_eq!(timing.full_source_frames, 64 * 44100);
    assert_eq!(timing.full_source_frames, features.full_source_frames);
    assert_eq!(timing.analyzed_frames, features.analyzed_frames);
    assert_eq!(timing.profile, features.profile);
    assert!(timing.bpm.is_finite() && timing.bpm > 0.);
    assert!(timing.beats_seconds.len() > 2);
    assert!(timing.beats_seconds.iter().all(|x| x.is_finite()));
    assert!(timing.beats_seconds.windows(2).all(|v| v[0] < v[1]));
    let json = serde_json::to_value(timing).unwrap();
    assert!(json.get("strength").is_none());
    assert!(json.get("key").is_none());
    assert!(json.get("cues").is_none());
}

#[test]
fn timing_only_rejects_unverified_profiles_and_unresolved_short_input() {
    let wrong_rate = analyze_timing_file(&fixture("stereo16-48000.flac")).unwrap_err();
    assert!(format!("{wrong_rate:#}").contains("44100"));
    assert!(analyze_timing_file(&fixture("stereo16-44100.flac")).is_err());
}

#[test]
fn compiled_source_identity_is_explicit_and_sha256_shaped() {
    let digest = source_sha256();
    assert_eq!(digest.len(), 64);
    assert!(
        digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
}
