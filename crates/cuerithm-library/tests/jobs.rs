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

#[test]
fn restoring_historical_source_replaces_current_results_again() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let path = d.path().join("x.flac");
    let revision = lib.settings().unwrap().revision;
    let mut track_id = String::new();
    for (index, source) in [b"A", b"B", b"A"].into_iter().enumerate() {
        std::fs::write(&path, source).unwrap();
        let id = lib
            .enqueue(std::slice::from_ref(&path), revision.clone())
            .unwrap()[0]
            .clone();
        let job = lib
            .claim_next()
            .unwrap()
            .expect("restored source must run again");
        track_id = job.track_id.clone();
        assert_eq!(lib.source_hash(&track_id).unwrap(), job.source_sha256);
        assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 0);
        let mut analysis = result();
        analysis.analysis.lexicon_cues.as_mut().unwrap().cues[0].name = format!("revision {index}");
        lib.apply_analysis(&id, &analysis, &info(), &job.source_sha256)
            .unwrap();
        assert_eq!(lib.cue_revision_count(&track_id).unwrap(), index as u64 + 1);
    }
    assert_eq!(
        lib.analysis(&track_id)
            .unwrap()
            .unwrap()
            .analysis
            .lexicon_cues
            .unwrap()
            .cues[0]
            .name,
        "revision 2"
    );
}

#[test]
fn restored_incoming_jobs_are_claimable_after_restart() {
    for state in ["queued", "running", "attention"] {
        let d = tempfile::tempdir().unwrap();
        let db = d.path().join("db");
        let path = d.path().join("x.flac");
        std::fs::write(&path, b"A").unwrap();
        let track_id;
        {
            let lib = Library::open(&db).unwrap();
            let rev = lib.settings().unwrap().revision;
            let id = lib
                .enqueue(std::slice::from_ref(&path), rev.clone())
                .unwrap()[0]
                .clone();
            track_id = lib.jobs().unwrap()[0].track_id.clone();
            if state != "queued" {
                lib.claim_next().unwrap().unwrap();
            }
            if state == "attention" {
                lib.fail(&id, "analysis", "test").unwrap();
            }
            lib.set_archived(&track_id, true).unwrap();
            // Watcher hints cannot restart archived work.
            lib.enqueue(std::slice::from_ref(&path), rev).unwrap();
            assert!(lib.claim_next().unwrap().is_none());
            assert!(
                lib.retry(&id, None).is_err(),
                "archived preparation requires Restore first"
            );
            std::fs::write(&path, b"B").unwrap();
            lib.set_archived(&track_id, false).unwrap();
        }
        let lib = Library::open(&db).unwrap();
        let job = lib
            .claim_next()
            .unwrap()
            .expect("restored Incoming must remain actionable");
        assert_eq!(job.track_id, track_id);
        assert_eq!(
            job.source_sha256,
            cuerithm_media::file_sha256(&path).unwrap()
        );
    }
}

#[test]
fn stream_tags_are_persisted_and_searchable() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rt =
        cuerithm_media::MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    for name in ["tagged.opus", "tagged.ogg"] {
        let path = root.join("crates/cuerithm-media/tests/fixtures").join(name);
        let id = lib
            .enqueue(
                std::slice::from_ref(&path),
                lib.settings().unwrap().revision,
            )
            .unwrap()[0]
            .clone();
        let job = lib.claim_next().unwrap().unwrap();
        lib.apply_analysis(
            &id,
            &result(),
            &rt.probe(&path).unwrap(),
            &job.source_sha256,
        )
        .unwrap();
        let row = lib.track(&job.track_id).unwrap();
        assert_eq!(
            (
                row.title.as_str(),
                row.artist.as_str(),
                row.album.as_str(),
                row.genre.as_str()
            ),
            ("Éclair", "Studio Artist", "Original Test", "House")
        );
    }
    assert_eq!(
        lib.tracks(TrackQuery {
            search: "éclair".into(),
            ..Default::default()
        })
        .unwrap()
        .total,
        2
    );
}

#[test]
fn restoring_a_missing_archived_source_stays_actionable() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let path = d.path().join("x.flac");
    std::fs::write(&path, b"A").unwrap();
    let id = lib
        .enqueue(
            std::slice::from_ref(&path),
            lib.settings().unwrap().revision,
        )
        .unwrap()[0]
        .clone();
    let job = lib.claim_next().unwrap().unwrap();
    lib.apply_analysis(&id, &result(), &info(), &job.source_sha256)
        .unwrap();
    lib.set_archived(&job.track_id, true).unwrap();
    std::fs::remove_file(&path).unwrap();
    lib.set_archived(&job.track_id, false).unwrap();
    assert_eq!(lib.jobs().unwrap()[0].state, "attention");
    assert!(
        lib.jobs().unwrap()[0]
            .error
            .as_ref()
            .unwrap()
            .contains("Source unavailable")
    );
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 0);
    assert_eq!(lib.cue_revision_count(&job.track_id).unwrap(), 1);
}
