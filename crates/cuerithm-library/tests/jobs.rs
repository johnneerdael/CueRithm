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
        selection: None,
        selected_beatgrid: None,
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
        properties: Default::default(),
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

#[test]
fn unchanged_discovered_files_keep_completed_analysis_after_settings_updates() {
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
    let revision = lib
        .save_settings(PreparationSettings {
            stability_millis: 50,
            ..Default::default()
        })
        .unwrap();
    lib.enqueue_discovered(std::slice::from_ref(&path), revision.clone())
        .unwrap();
    assert_eq!(lib.tracks(TrackQuery::default()).unwrap().total, 1);
    assert!(lib.claim_next().unwrap().is_none());
    lib.enqueue(&[path], revision).unwrap();
    assert!(
        lib.claim_next().unwrap().is_some(),
        "explicit reanalysis still uses the new settings"
    );
}

#[test]
fn stale_attempts_cannot_mutate_a_reused_job_identity() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let path = d.path().join("x.flac");
    let rev = lib.settings().unwrap().revision;
    std::fs::write(&path, b"A").unwrap();
    lib.enqueue(std::slice::from_ref(&path), rev.clone())
        .unwrap();
    let old = lib.claim_next().unwrap().unwrap();
    std::fs::write(&path, b"B").unwrap();
    lib.enqueue(std::slice::from_ref(&path), rev.clone())
        .unwrap();
    std::fs::write(&path, b"A").unwrap();
    lib.enqueue(&[path], rev).unwrap();
    let current = lib.claim_next().unwrap().unwrap();
    assert_eq!(old.id, current.id);
    assert!(lib.set_stage_for_run(&old, "analysis").is_err());
    assert!(lib.fail_for_run(&old, "analysis", "old error").is_err());
    assert!(
        lib.apply_analysis_for_run(&old, &result(), &info())
            .is_err()
    );
    lib.apply_analysis_for_run(&current, &result(), &info())
        .unwrap();
    assert_eq!(lib.cue_revision_count(&current.track_id).unwrap(), 1);
}

#[test]
fn stale_source_retry_cannot_replace_a_newer_running_revision() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let path = d.path().join("x.flac");
    std::fs::write(&path, b"A").unwrap();
    lib.enqueue(
        std::slice::from_ref(&path),
        lib.settings().unwrap().revision,
    )
    .unwrap();
    let old = lib.claim_next().unwrap().unwrap();
    std::fs::write(&path, b"B").unwrap();
    let revision = lib
        .save_settings(PreparationSettings {
            stability_millis: 50,
            ..Default::default()
        })
        .unwrap();
    lib.enqueue(&[path], revision.clone()).unwrap();
    let current = lib.claim_next().unwrap().unwrap();
    assert!(lib.retry_for_run(&old).is_err());
    lib.set_stage_for_run(&current, "analysis").unwrap();
    lib.apply_analysis_for_run(&current, &result(), &info())
        .unwrap();
    assert_eq!(lib.settings().unwrap().revision, revision);
}

#[test]
fn track_rows_expose_arbitrary_metadata_without_changing_analysis() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let source = d.path().join("track.flac");
    std::fs::write(&source, b"source").unwrap();
    let id = lib
        .enqueue(&[source], lib.settings().unwrap().revision)
        .unwrap()[0]
        .clone();
    let job = lib.claim_next().unwrap().unwrap();
    let mut media = info();
    media.tags.insert("label".into(), "Example label".into());
    media.tags.insert("custom:mood".into(), "Warm".into());
    lib.apply_analysis(&id, &result(), &media, &job.source_sha256)
        .unwrap();
    let row = lib.tracks(TrackQuery::default()).unwrap().rows.remove(0);
    let json = serde_json::to_value(&row).unwrap();
    assert_eq!(json["metadata"]["tag.label"], "Example label");
    assert_eq!(json["metadata"]["audio.sample_rate"], 44100);
    assert_eq!(row.key.as_deref(), Some("8B"));
    let pending = d.path().join("pending.flac");
    std::fs::write(&pending, b"pending source").unwrap();
    lib.enqueue(&[pending], lib.settings().unwrap().revision)
        .unwrap();
    assert!(
        lib.metadata_fields()
            .unwrap()
            .contains(&"tag.custom:mood".to_owned())
    );
}

#[test]
fn legacy_metadata_refresh_is_source_guarded_and_preserves_cues_and_analysis() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("db");
    let lib = Library::open(&path).unwrap();
    let source = d.path().join("track.flac");
    std::fs::write(&source, b"source").unwrap();
    let id = lib
        .enqueue(&[source], lib.settings().unwrap().revision)
        .unwrap()[0]
        .clone();
    let job = lib.claim_next().unwrap().unwrap();
    lib.apply_analysis(&id, &result(), &info(), &job.source_sha256)
        .unwrap();
    let before = serde_json::to_value(lib.analysis(&job.track_id).unwrap()).unwrap();
    let cues = lib.cue_revision_count(&job.track_id).unwrap();
    rusqlite::Connection::open(path)
        .unwrap()
        .execute(
            "UPDATE tracks SET metadata_json=json_remove(metadata_json,'$.properties')",
            [],
        )
        .unwrap();
    assert_eq!(
        lib.metadata_refresh_candidates().unwrap(),
        vec![job.track_id.clone()]
    );
    let mut metadata = info();
    metadata
        .tags
        .insert("label".into(), "Backfilled label".into());
    assert!(
        !lib.update_legacy_metadata(&job.track_id, "stale source", &metadata)
            .unwrap()
    );
    assert!(
        lib.update_legacy_metadata(&job.track_id, &job.source_sha256, &metadata)
            .unwrap()
    );
    assert!(
        !lib.update_legacy_metadata(&job.track_id, &job.source_sha256, &info())
            .unwrap()
    );
    assert!(
        lib.metadata_fields()
            .unwrap()
            .contains(&"tag.label".to_owned())
    );
    assert_eq!(
        lib.track(&job.track_id).unwrap().metadata["tag.label"],
        "Backfilled label"
    );
    assert_eq!(lib.cue_revision_count(&job.track_id).unwrap(), cues);
    assert_eq!(
        serde_json::to_value(lib.analysis(&job.track_id).unwrap()).unwrap(),
        before
    );
    assert!(lib.metadata_refresh_candidates().unwrap().is_empty());
}

#[test]
fn published_fractional_grid_is_saved_without_replacing_raw_detector_output() {
    use camelot_core::lexicon_publication::{
        CueTemplate, Marker, StructuralMarker, publish_measured,
    };
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("db")).unwrap();
    let path = dir.path().join("original.flac");
    std::fs::write(&path, b"synthetic publication boundary").unwrap();
    lib.enqueue(&[path], lib.settings().unwrap().revision)
        .unwrap();
    let run = lib.claim_next().unwrap().unwrap();
    let mut result = result();
    result.analysis.duration_seconds = 160.;
    let tempo = result.analysis.tempo.as_mut().unwrap();
    tempo.bpm = 128.345;
    tempo.downbeat_time = Some(0.125);
    tempo.beats_seconds = vec![0.125];
    result.analysis.lexicon_cues.as_mut().unwrap().cues[0].time_seconds = 0.125;
    result.publication = Some(
        publish_measured(
            &[StructuralMarker {
                marker_type: Marker::Start,
                start_time_seconds: Some(0.125),
            }],
            128.345,
            0.125,
            160.,
            &CueTemplate::default(),
            &[],
        )
        .unwrap(),
    );
    lib.apply_analysis_for_run(&run, &result, &info()).unwrap();
    assert_eq!(lib.track(&run.track_id).unwrap().bpm, Some(128.35));
    let saved = lib.analysis(&run.track_id).unwrap().unwrap();
    assert_eq!(saved.analysis.tempo.unwrap().bpm, 128.345);
    assert_eq!(saved.publication.unwrap().grid.bpm, 128.35);
}

#[test]
fn repeated_archive_state_is_a_noop_for_jobs_analysis_and_revision() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::open(&d.path().join("db")).unwrap();
    let path = d.path().join("same-state.flac");
    std::fs::write(&path, b"A").unwrap();
    let job_id = lib
        .enqueue(
            std::slice::from_ref(&path),
            lib.settings().unwrap().revision,
        )
        .unwrap()[0]
        .clone();
    let job = lib.claim_next().unwrap().unwrap();
    lib.apply_analysis(&job_id, &result(), &info(), &job.source_sha256)
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    let revision = lib.revision().unwrap();
    lib.set_archived(&job.track_id, false).unwrap();
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(lib.track(&job.track_id).unwrap().state, "ready");
    assert_eq!(lib.jobs().unwrap()[0].state, "completed");
    assert_eq!(lib.cue_revision_count(&job.track_id).unwrap(), 1);
    lib.set_archived(&job.track_id, true).unwrap();
    let revision = lib.revision().unwrap();
    lib.set_archived(&job.track_id, true).unwrap();
    assert_eq!(lib.revision().unwrap(), revision);
    assert_eq!(lib.jobs().unwrap()[0].state, "completed");
    assert_eq!(lib.cue_revision_count(&job.track_id).unwrap(), 1);
}
