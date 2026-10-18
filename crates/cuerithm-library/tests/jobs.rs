use camelot_core::{
    AnalysisConfig, FileAnalysis, KeyEstimate, Mode, MusicalKey, TempoEstimate,
    energy::TrackEnergy,
    lexicon_cues::{StructuralCue, StructuralCueResult},
};
use cuerithm_library::{Library, PreparationSettings, TrackQuery, TrackView};
use cuerithm_media::MediaInfo;

fn result() -> FileAnalysis {
    let mut a =
        camelot_core::analyze_pcm(&vec![0.; 11025], 11025, &AnalysisConfig::default()).unwrap();
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
        beats_seconds: vec![0., 0.5],
        downbeats_seconds: None,
        downbeat_time: Some(0.),
        method: "test".into(),
    });
    a.energy = Some(TrackEnergy {
        level: 5,
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
    FileAnalysis {
        analysis: a,
        complete: true,
        verified_integrity: None,
        source_sample_rate: 44100,
        beat_sample_rate: None,
        neural_key_sample_rate: None,
        openkeyscan_sample_rate: None,
        source_timing: None,
        elapsed_seconds: 0.,
        media: None,
    }
}
fn info() -> MediaInfo {
    MediaInfo {
        codec: "flac".into(),
        sample_rate: 44100,
        channels: 2,
        bits_per_sample: Some(16),
        tags: Default::default(),
    }
}

#[test]
fn completion_is_atomic_preserves_previous_cues_and_rejects_incomplete_results() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let p = d.path().join("track.flac");
    std::fs::write(&p, b"source").unwrap();
    let revision = lib.settings().unwrap().revision;
    let id = lib.enqueue(std::slice::from_ref(&p), revision).unwrap()[0].clone();
    let job = lib.claim_next().unwrap().unwrap();
    let mut incomplete = result();
    incomplete.analysis.energy = None;
    assert!(
        lib.apply_analysis(&id, &incomplete, &info(), &job.source_sha256)
            .is_err()
    );
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 0);
    lib.apply_analysis(&id, &result(), &info(), &job.source_sha256)
        .unwrap();
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 1);
    let track_id = job.track_id;
    let settings = PreparationSettings {
        stability_millis: 30,
        ..Default::default()
    };
    let rev = lib.save_settings(settings).unwrap();
    let next = lib.enqueue(&[p], rev).unwrap()[0].clone();
    lib.claim_next().unwrap().unwrap();
    lib.fail(&next, "analysis", "decoder failed").unwrap();
    assert_eq!(
        lib.analysis(&track_id)
            .unwrap()
            .unwrap()
            .analysis
            .lexicon_cues
            .unwrap()
            .cues[0]
            .name,
        "Start"
    );
    assert_eq!(
        lib.tracks(TrackQuery {
            view: TrackView::Incoming,
            ..Default::default()
        })
        .unwrap()
        .total,
        1
    );
    lib.retry(&next, None).unwrap();
    let running = lib.claim_next().unwrap().unwrap();
    let mut changed = result();
    changed.analysis.lexicon_cues.as_mut().unwrap().cues[0].name = "Replacement".into();
    lib.apply_analysis(&next, &changed, &info(), &running.source_sha256)
        .unwrap();
    assert_eq!(lib.cue_revision_count(&track_id).unwrap(), 2);
    lib.set_archived(&track_id, true).unwrap();
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 0);
    assert_eq!(
        lib.tracks(TrackQuery {
            view: TrackView::Archive,
            ..Default::default()
        })
        .unwrap()
        .total,
        1
    );
    lib.set_archived(&track_id, false).unwrap();
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 1);
}

#[test]
fn unavailable_required_stages_and_superseded_jobs_cannot_promote() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let p = d.path().join("x.flac");
    std::fs::write(&p, b"x").unwrap();
    let rev = lib
        .save_settings(PreparationSettings {
            autotag_enabled: true,
            ..Default::default()
        })
        .unwrap();
    let id = lib.enqueue(std::slice::from_ref(&p), rev).unwrap()[0].clone();
    let job = lib.claim_next().unwrap().unwrap();
    assert!(
        lib.apply_analysis(&id, &result(), &info(), &job.source_sha256)
            .is_err()
    );
    std::fs::write(&p, b"new source").unwrap();
    let rev = lib.save_settings(PreparationSettings::default()).unwrap();
    lib.enqueue(&[p], rev).unwrap();
    assert!(
        lib.apply_analysis(&id, &result(), &info(), &job.source_sha256)
            .is_err()
    );
}

#[test]
fn invalid_numeric_analysis_cannot_commit_a_ready_but_unreadable_track() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let path = d.path().join("x.flac");
    std::fs::write(&path, b"x").unwrap();
    let id = lib
        .enqueue(&[path], lib.settings().unwrap().revision)
        .unwrap()[0]
        .clone();
    let job = lib.claim_next().unwrap().unwrap();
    let mut invalid = result();
    invalid.analysis.key.as_mut().unwrap().margin = f64::NAN;
    assert!(
        lib.apply_analysis(&id, &invalid, &info(), &job.source_sha256)
            .is_err()
    );
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 0);
}
