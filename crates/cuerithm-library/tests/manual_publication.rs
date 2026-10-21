use camelot_core::{
    AnalysisConfig, FileAnalysis, KeyEstimate, Mode, MusicalKey, TempoEstimate,
    energy::TrackEnergy,
    lexicon_cues::{StructuralCue, StructuralCueResult},
    lexicon_publication::{CueTemplate, PublishedCue, PublishedType, SavedGrid},
};
use cuerithm_library::*;
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

fn fixture() -> (tempfile::TempDir, Library, String) {
    let dir = tempfile::tempdir().unwrap();
    let library = Library::open(&dir.path().join("db")).unwrap();
    let path = dir.path().join("track.flac");
    std::fs::write(&path, b"authored source fixture").unwrap();
    let id = library
        .enqueue(&[path], library.settings().unwrap().revision)
        .unwrap()[0]
        .clone();
    let run = library.claim_next().unwrap().unwrap();
    let mut analysis = result();
    analysis.analysis.duration_seconds = 60.;
    analysis.publication = Some(
        camelot_core::lexicon_publication::publish_measured(
            &[camelot_core::lexicon_publication::StructuralMarker {
                marker_type: camelot_core::lexicon_publication::Marker::Start,
                start_time_seconds: Some(0.),
            }],
            120.,
            0.,
            60.,
            &CueTemplate::default(),
            &[],
        )
        .unwrap(),
    );
    library
        .apply_analysis(&id, &analysis, &info(), &run.source_sha256)
        .unwrap();
    (dir, library, run.track_id)
}
fn edited(library: &Library, id: &str) -> ManualPublicationRequest {
    let snapshot = library.manual_publication_snapshot(id).unwrap();
    ManualPublicationRequest {
        guard: snapshot.guard,
        expected_revision: snapshot.library_revision,
        grid: SavedGrid {
            bpm: 60.,
            start_time: 0.125,
        },
        cues: vec![PublishedCue {
            name: "My loop".into(),
            start_time: 2.,
            kind: PublishedType::Loop,
            position: 2,
            color: "blue".into(),
            end_time: Some(6.),
            active_loop: Some(true),
            data: None,
        }],
    }
}
#[test]
fn manual_publication_is_durable_undoable_and_preserves_measured_analysis_and_source() {
    let (dir, library, id) = fixture();
    let before = serde_json::to_value(library.analysis(&id).unwrap().unwrap().analysis).unwrap();
    let source = std::fs::read(library.track_path(&id).unwrap()).unwrap();
    let request = edited(&library, &id);
    let preview = library.preview_manual_publication(request).unwrap();
    assert_eq!(library.track(&id).unwrap().bpm, Some(120.));
    let applied = library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    assert_eq!(library.track(&id).unwrap().bpm, Some(60.));
    let saved = library.analysis(&id).unwrap().unwrap();
    assert_eq!(serde_json::to_value(saved.analysis).unwrap(), before);
    assert_eq!(saved.publication.unwrap().cues[0].end_time, Some(6.));
    assert_eq!(
        std::fs::read(library.track_path(&id).unwrap()).unwrap(),
        source
    );
    drop(library);
    let library = Library::open(&dir.path().join("db")).unwrap();
    assert!(library.manual_publication_history(&id).unwrap()[0].can_undo);
    assert_eq!(
        library
            .analysis(&id)
            .unwrap()
            .unwrap()
            .publication
            .unwrap()
            .cues[0]
            .active_loop,
        Some(true)
    );
    library
        .undo_manual_publication(&applied.id, library.revision().unwrap())
        .unwrap();
    assert_eq!(library.track(&id).unwrap().bpm, Some(120.));
    assert_eq!(
        library
            .analysis(&id)
            .unwrap()
            .unwrap()
            .publication
            .unwrap()
            .cues[0]
            .name,
        "Start"
    );
    assert_eq!(library.cue_revision_count(&id).unwrap(), 3);
}
#[test]
fn invalid_loops_slots_grid_source_and_preparation_are_rejected() {
    let (_dir, library, id) = fixture();
    let mut request = edited(&library, &id);
    request.cues[0].end_time = Some(2.);
    assert!(library.preview_manual_publication(request).is_err());
    let mut request = edited(&library, &id);
    request.cues.push(request.cues[0].clone());
    assert!(library.preview_manual_publication(request).is_err());
    let mut request = edited(&library, &id);
    request.grid.start_time = -0.01;
    assert!(library.preview_manual_publication(request).is_err());
    let mut request = edited(&library, &id);
    request.grid.bpm = f64::NAN;
    assert!(library.preview_manual_publication(request).is_err());
    let preview = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    std::fs::write(library.track_path(&id).unwrap(), b"changed source").unwrap();
    assert!(
        library
            .apply_manual_publication(&preview.id, preview.library_revision)
            .is_err()
    );
}
#[test]
fn stale_previews_and_undo_reject_aba_and_unowned_revisions() {
    let (_dir, library, id) = fixture();
    let preview = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    library.set_archived(&id, true).unwrap();
    library.set_archived(&id, false).unwrap();
    assert!(
        library
            .apply_manual_publication(&preview.id, library.revision().unwrap())
            .is_err()
    );
    let (_dir, library, id) = fixture();
    let first = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    let first = library
        .apply_manual_publication(&first.id, first.library_revision)
        .unwrap();
    let next = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    library
        .apply_manual_publication(&next.id, next.library_revision)
        .unwrap();
    assert!(
        library
            .undo_manual_publication(&first.id, library.revision().unwrap())
            .is_err()
    );
}
#[test]
fn templates_preview_without_mutation_and_preserve_manual_slots_by_default() {
    let (_dir, library, id) = fixture();
    let preview = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    let mut template = CueTemplate {
        overwrite: true,
        ..CueTemplate::default()
    };
    template.rows[0].name = "Template start".into();
    let named = library
        .save_cue_template(
            None,
            "Practice".into(),
            template,
            library.revision().unwrap(),
        )
        .unwrap();
    let snapshot = library.manual_publication_snapshot(&id).unwrap();
    let preview = library
        .preview_cue_template(TemplatePublicationRequest {
            template_id: named.id,
            guards: vec![snapshot.guard],
            expected_revision: snapshot.library_revision,
            preserve_manual: true,
        })
        .unwrap();
    assert!(
        preview.changes[0]
            .after
            .cues
            .iter()
            .any(|c| c.name == "My loop")
    );
    assert!(
        !preview.changes[0]
            .after
            .cues
            .iter()
            .any(|c| c.name == "Template start")
    );
    assert_eq!(preview.changes[0].after.grid.bpm, 60.);
    assert_eq!(
        library
            .analysis(&id)
            .unwrap()
            .unwrap()
            .publication
            .unwrap()
            .cues[0]
            .name,
        "My loop"
    );
    library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
}

#[test]
fn monotonic_entity_epoch_rejects_aba_even_when_library_revision_is_unchanged() {
    let (dir, library, id) = fixture();
    let preview = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
    conn.execute("UPDATE tracks SET title='Changed' WHERE id=?1", [&id])
        .unwrap();
    conn.execute("UPDATE tracks SET title='track' WHERE id=?1", [&id])
        .unwrap();
    assert_eq!(library.revision().unwrap(), preview.library_revision);
    let error = library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap_err();
    assert!(error.to_string().contains("Track changed"));
    assert_eq!(library.track(&id).unwrap().bpm, Some(120.));
}
#[test]
fn queued_preparation_blocks_manual_preview_and_undo_without_losing_history() {
    let (_dir, library, id) = fixture();
    let preview = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    let result = library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    let request = edited(&library, &id);
    let job = library.jobs().unwrap()[0].id.clone();
    library.retry(&job, None).unwrap();
    assert!(library.preview_manual_publication(request).is_err());
    assert!(
        library
            .undo_manual_publication(&result.id, library.revision().unwrap())
            .is_err()
    );
    assert!(!library.manual_publication_history(&id).unwrap()[0].can_undo);
}
#[test]
fn sync_projects_saved_manual_grid_loop_and_slot() {
    let (_dir, library, id) = fixture();
    let preview = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    let sync = library.dj_sync_library(DjSyncScope::Full).unwrap();
    let value = serde_json::to_value(sync).unwrap();
    let track = &value["tracks"][0];
    assert_eq!(track["bpm"], 60.);
    assert_eq!(track["beatgrid"][0]["seconds"], 0.125);
    assert_eq!(track["cues"][0]["name"], "My loop");
    assert_eq!(track["cues"][0]["index"], 2);
    assert_eq!(track["cues"][0]["start_seconds"], 2.);
    assert_eq!(track["cues"][0]["end_seconds"], 6.);
}
#[test]
fn templates_keep_named_anchors_offsets_and_abort_a_stale_batch_atomically() {
    let (dir, library, id) = fixture();
    let request = edited(&library, &id);
    let preview = library.preview_manual_publication(request).unwrap();
    library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    let template = CueTemplate {
        overwrite: true,
        enable_custom_anchors: true,
        custom_anchors: serde_json::json!({"start":"My loop"}),
        rows: vec![camelot_core::lexicon_publication::TemplateRow {
            checked: true,
            marker_type: camelot_core::lexicon_publication::Marker::Normal,
            name: "Four beats after my loop".into(),
            cue_type: camelot_core::lexicon_publication::CueType::HotCue,
            color: "red".into(),
            label: None,
            anchor: Some(camelot_core::lexicon_publication::Marker::Start),
            position: Some(camelot_core::lexicon_publication::OffsetPosition::After),
            beats_offset: Some(4.),
            loop_duration: None,
        }],
        ..CueTemplate::default()
    };
    let named = library
        .save_cue_template(
            None,
            "Anchored".into(),
            template,
            library.revision().unwrap(),
        )
        .unwrap();
    let first = library.manual_publication_snapshot(&id).unwrap();
    let preview = library
        .preview_cue_template(TemplatePublicationRequest {
            template_id: named.id.clone(),
            guards: vec![first.guard],
            expected_revision: first.library_revision,
            preserve_manual: false,
        })
        .unwrap();
    assert_eq!(preview.changes[0].after.cues[0].start_time, 6.);
    let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
    conn.execute(
        "UPDATE tracks SET title='Intervening edit' WHERE id=?1",
        [&id],
    )
    .unwrap();
    assert!(
        library
            .apply_manual_publication(&preview.id, preview.library_revision)
            .is_err()
    );
    assert_eq!(
        library
            .analysis(&id)
            .unwrap()
            .unwrap()
            .publication
            .unwrap()
            .cues[0]
            .name,
        "My loop"
    );
    let mut bad = named.template;
    bad.custom_anchors = serde_json::json!({"start":"missing"});
    let named = library
        .save_cue_template(
            None,
            "Missing anchor".into(),
            bad,
            library.revision().unwrap(),
        )
        .unwrap();
    let first = library.manual_publication_snapshot(&id).unwrap();
    assert!(
        library
            .preview_cue_template(TemplatePublicationRequest {
                template_id: named.id,
                guards: vec![first.guard],
                expected_revision: first.library_revision,
                preserve_manual: false
            })
            .is_err()
    );
}

#[test]
fn batch_template_apply_and_restart_undo_are_atomic_across_selected_tracks() {
    let (dir, library, first) = fixture();
    let source = dir.path().join("second.opus");
    std::fs::write(&source, b"second authored fixture").unwrap();
    let job = library
        .enqueue(&[source], library.settings().unwrap().revision)
        .unwrap()[0]
        .clone();
    let run = library.claim_next().unwrap().unwrap();
    let analysis = library.analysis(&first).unwrap().unwrap();
    library
        .apply_analysis(&job, &analysis, &info(), &run.source_sha256)
        .unwrap();
    let second = run.track_id;
    let mut template = CueTemplate {
        overwrite: true,
        ..CueTemplate::default()
    };
    template.rows[0].name = "Practice start".into();
    let named = library
        .save_cue_template(None, "Batch".into(), template, library.revision().unwrap())
        .unwrap();
    let request = |library: &Library| TemplatePublicationRequest {
        template_id: named.id.clone(),
        guards: vec![
            library.manual_publication_snapshot(&first).unwrap().guard,
            library.manual_publication_snapshot(&second).unwrap().guard,
        ],
        expected_revision: library.revision().unwrap(),
        preserve_manual: true,
    };
    let stale = library.preview_cue_template(request(&library)).unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
    conn.execute(
        "UPDATE tracks SET title='Changed second' WHERE id=?1",
        [&second],
    )
    .unwrap();
    drop(conn);
    assert!(
        library
            .apply_manual_publication(&stale.id, stale.library_revision)
            .is_err()
    );
    assert_eq!(library.cue_revision_count(&first).unwrap(), 1);
    assert_eq!(library.cue_revision_count(&second).unwrap(), 1);
    let preview = library.preview_cue_template(request(&library)).unwrap();
    assert_eq!(preview.changes.len(), 2);
    let saved = library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    assert_eq!(
        library
            .analysis(&first)
            .unwrap()
            .unwrap()
            .publication
            .unwrap()
            .cues[0]
            .name,
        "Practice start"
    );
    assert_eq!(
        library
            .analysis(&second)
            .unwrap()
            .unwrap()
            .publication
            .unwrap()
            .cues[0]
            .name,
        "Practice start"
    );
    drop(library);
    let library = Library::open(&dir.path().join("db")).unwrap();
    library
        .undo_manual_publication(&saved.id, library.revision().unwrap())
        .unwrap();
    for id in [&first, &second] {
        assert_eq!(
            library
                .analysis(id)
                .unwrap()
                .unwrap()
                .publication
                .unwrap()
                .cues[0]
                .name,
            "Start"
        );
        assert_eq!(library.cue_revision_count(id).unwrap(), 3);
    }
}

#[test]
fn non_replacing_template_does_not_erase_ownership_of_untouched_manual_corrections() {
    let (_dir, library, id) = fixture();
    let preview = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    let mut keep = CueTemplate {
        overwrite: false,
        ..CueTemplate::default()
    };
    keep.rows.clear();
    let named = library
        .save_cue_template(
            None,
            "Keep current".into(),
            keep,
            library.revision().unwrap(),
        )
        .unwrap();
    let snapshot = library.manual_publication_snapshot(&id).unwrap();
    let preview = library
        .preview_cue_template(TemplatePublicationRequest {
            template_id: named.id,
            guards: vec![snapshot.guard],
            expected_revision: snapshot.library_revision,
            preserve_manual: false,
        })
        .unwrap();
    library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    let mut replace = CueTemplate {
        overwrite: true,
        ..CueTemplate::default()
    };
    replace.rows[0].name = "Replace start".into();
    let named = library
        .save_cue_template(
            None,
            "Replacement".into(),
            replace,
            library.revision().unwrap(),
        )
        .unwrap();
    let snapshot = library.manual_publication_snapshot(&id).unwrap();
    let preview = library
        .preview_cue_template(TemplatePublicationRequest {
            template_id: named.id,
            guards: vec![snapshot.guard],
            expected_revision: snapshot.library_revision,
            preserve_manual: true,
        })
        .unwrap();
    assert!(
        preview.changes[0]
            .after
            .cues
            .iter()
            .any(|cue| cue.name == "My loop")
    );
    assert!(
        !preview.changes[0]
            .after
            .cues
            .iter()
            .any(|cue| cue.name == "Replace start")
    );
}

#[test]
fn regenerated_publication_does_not_inherit_manual_slot_edits_or_deletion_ownership() {
    for delete in [false, true] {
        let (_dir, library, id) = fixture();
        let measured = library.analysis(&id).unwrap().unwrap();
        let snapshot = library.manual_publication_snapshot(&id).unwrap();
        let mut cues = snapshot.publication.cues;
        if delete {
            cues.clear();
        } else {
            cues[0].name = "Manual start".into();
        }
        let preview = library
            .preview_manual_publication(ManualPublicationRequest {
                guard: snapshot.guard,
                expected_revision: snapshot.library_revision,
                grid: snapshot.publication.grid,
                cues,
            })
            .unwrap();
        library
            .apply_manual_publication(&preview.id, preview.library_revision)
            .unwrap();
        let mut template = CueTemplate {
            overwrite: true,
            ..CueTemplate::default()
        };
        template.rows[0].name = "Template start".into();
        let named = library
            .save_cue_template(
                None,
                "Replacement".into(),
                template,
                library.revision().unwrap(),
            )
            .unwrap();
        let current = library.manual_publication_snapshot(&id).unwrap();
        let preserved = library
            .preview_cue_template(TemplatePublicationRequest {
                template_id: named.id.clone(),
                guards: vec![current.guard],
                expected_revision: current.library_revision,
                preserve_manual: true,
            })
            .unwrap();
        assert!(preserved.changes[0].preserved_manual);
        if delete {
            assert!(preserved.changes[0].after.cues.is_empty());
        } else {
            assert_eq!(preserved.changes[0].after.cues[0].name, "Manual start");
        }
        let job = library.jobs().unwrap()[0].id.clone();
        library.retry(&job, None).unwrap();
        let run = library.claim_next().unwrap().unwrap();
        library
            .apply_analysis_for_run(&run, &measured, &info())
            .unwrap();
        let regenerated = library.manual_publication_snapshot(&id).unwrap();
        assert_eq!(regenerated.publication.cues[0].name, "Start");
        let preview = library
            .preview_cue_template(TemplatePublicationRequest {
                template_id: named.id,
                guards: vec![regenerated.guard],
                expected_revision: regenerated.library_revision,
                preserve_manual: true,
            })
            .unwrap();
        assert_eq!(
            preview.changes[0].after.cues[0].name, "Template start",
            "regenerated cues must not inherit old manual slot ownership (delete={delete})"
        );
        assert!(!preview.changes[0].preserved_manual);
    }
}

#[test]
fn replacement_cue_revision_invalidates_manual_ownership_and_revision_restore_recovers_it() {
    let (dir, library, id) = fixture();
    let preview = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    let mut template = CueTemplate {
        overwrite: true,
        ..CueTemplate::default()
    };
    template.rows[0].name = "Template start".into();
    let named = library
        .save_cue_template(
            None,
            "Replacement".into(),
            template,
            library.revision().unwrap(),
        )
        .unwrap();
    let owned = library.manual_publication_snapshot(&id).unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
    let before: String = conn
        .query_row(
            "SELECT analysis_json FROM tracks WHERE id=?1",
            [&id],
            |row| row.get(0),
        )
        .unwrap();
    let mut replacement: serde_json::Value = serde_json::from_str(&before).unwrap();
    replacement["publication"]["cues"][0]["position"] = 0.into();
    replacement["publication"]["cues"][0]["name"] = "Donor start".into();
    let mut publication = replacement["publication"].clone();
    publication["schema"] = "lexicon-publication-v1".into();
    conn.execute(
        "INSERT INTO cue_revisions VALUES ('donor-replacement',?1,NULL,?2,?3)",
        rusqlite::params![id, publication.to_string(), i64::MAX],
    )
    .unwrap();
    conn.execute(
        "UPDATE tracks SET analysis_json=?1 WHERE id=?2",
        rusqlite::params![replacement.to_string(), id],
    )
    .unwrap();
    let donor = library.manual_publication_snapshot(&id).unwrap();
    let preview = library
        .preview_cue_template(TemplatePublicationRequest {
            template_id: named.id.clone(),
            guards: vec![donor.guard],
            expected_revision: donor.library_revision,
            preserve_manual: true,
        })
        .unwrap();
    assert_eq!(preview.changes[0].after.cues[0].name, "Template start");
    assert!(!preview.changes[0].preserved_manual);
    // Donor Undo restores the old cue rows/IDs. The original manual state remains
    // usable when its actual owning publication becomes current again.
    conn.execute("DELETE FROM cue_revisions WHERE id='donor-replacement'", [])
        .unwrap();
    conn.execute(
        "UPDATE tracks SET analysis_json=?1 WHERE id=?2",
        rusqlite::params![before, id],
    )
    .unwrap();
    let restored = library.manual_publication_snapshot(&id).unwrap();
    assert_eq!(restored.guard.cue_revision_id, owned.guard.cue_revision_id);
    let preview = library
        .preview_cue_template(TemplatePublicationRequest {
            template_id: named.id,
            guards: vec![restored.guard],
            expected_revision: restored.library_revision,
            preserve_manual: true,
        })
        .unwrap();
    assert!(
        preview.changes[0]
            .after
            .cues
            .iter()
            .any(|cue| cue.name == "My loop")
    );
    assert!(
        !preview.changes[0]
            .after
            .cues
            .iter()
            .any(|cue| cue.name == "Template start")
    );
    assert!(preview.changes[0].preserved_manual);
}

#[test]
fn undo_rebinds_restored_manual_protection_to_its_new_cue_revision() {
    let (_dir, library, id) = fixture();
    let first = library
        .preview_manual_publication(edited(&library, &id))
        .unwrap();
    library
        .apply_manual_publication(&first.id, first.library_revision)
        .unwrap();
    let mut request = edited(&library, &id);
    request.cues[0].name = "Adjusted loop".into();
    let second = library.preview_manual_publication(request).unwrap();
    library
        .apply_manual_publication(&second.id, second.library_revision)
        .unwrap();
    library
        .undo_manual_publication(&second.id, library.revision().unwrap())
        .unwrap();
    let template = CueTemplate {
        overwrite: true,
        ..CueTemplate::default()
    };
    let named = library
        .save_cue_template(
            None,
            "After undo".into(),
            template,
            library.revision().unwrap(),
        )
        .unwrap();
    let snapshot = library.manual_publication_snapshot(&id).unwrap();
    let preview = library
        .preview_cue_template(TemplatePublicationRequest {
            template_id: named.id,
            guards: vec![snapshot.guard],
            expected_revision: snapshot.library_revision,
            preserve_manual: true,
        })
        .unwrap();
    assert_eq!(preview.changes[0].after.cues.len(), 1);
    assert_eq!(preview.changes[0].after.cues[0].name, "My loop");
    assert!(preview.changes[0].preserved_manual);
}

#[test]
fn batch_history_uses_database_eligibility_and_undo_alone_checks_source_files() {
    let (dir, library, first) = fixture();
    let source = dir.path().join("history-second.flac");
    std::fs::write(&source, b"second authored history fixture").unwrap();
    let job = library
        .enqueue(
            std::slice::from_ref(&source),
            library.settings().unwrap().revision,
        )
        .unwrap()[0]
        .clone();
    let run = library.claim_next().unwrap().unwrap();
    let measured = library.analysis(&first).unwrap().unwrap();
    library
        .apply_analysis(&job, &measured, &info(), &run.source_sha256)
        .unwrap();
    let second = run.track_id;
    let mut template = CueTemplate {
        overwrite: true,
        ..CueTemplate::default()
    };
    template.rows[0].name = "Batch start".into();
    let named = library
        .save_cue_template(
            None,
            "History batch".into(),
            template,
            library.revision().unwrap(),
        )
        .unwrap();
    let preview = library
        .preview_cue_template(TemplatePublicationRequest {
            template_id: named.id,
            guards: vec![
                library.manual_publication_snapshot(&first).unwrap().guard,
                library.manual_publication_snapshot(&second).unwrap().guard,
            ],
            expected_revision: library.revision().unwrap(),
            preserve_manual: true,
        })
        .unwrap();
    let applied = library
        .apply_manual_publication(&preview.id, preview.library_revision)
        .unwrap();
    std::fs::remove_file(source).unwrap();
    // History promises only current DB ownership, so inspecting one track does
    // not need to open unavailable sources belonging to its batch partners.
    assert!(library.manual_publication_history(&first).unwrap()[0].can_undo);
    assert!(
        library
            .undo_manual_publication(&applied.id, library.revision().unwrap())
            .is_err()
    );
    assert_eq!(library.cue_revision_count(&first).unwrap(), 2);
    assert_eq!(library.cue_revision_count(&second).unwrap(), 2);
    // Prove the entire batch's cheap ownership checks precede any source proof
    // during Undo: the first file is gone, while a later member has a stale epoch.
    std::fs::remove_file(library.track_path(&first).unwrap()).unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("db")).unwrap();
    conn.execute(
        "UPDATE tracks SET title='Changed batch partner' WHERE id=?1",
        [&second],
    )
    .unwrap();
    assert!(!library.manual_publication_history(&first).unwrap()[0].can_undo);
    let error = library
        .undo_manual_publication(&applied.id, library.revision().unwrap())
        .unwrap_err();
    assert!(
        error.to_string().contains("Track changed"),
        "stale DB ownership must reject before trying missing source files: {error:#}"
    );
}
