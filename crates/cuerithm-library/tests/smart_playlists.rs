use cuerithm_library::*;
use cuerithm_smartlists::{
    KeyMixingMode, Operator, PlaylistReference, RuleValue, SmartDefinition, SmartRule,
};
use rusqlite::{Connection, params};
#[cfg(feature = "autotag")]
#[path = "support/cleanup_fixture.rs"]
#[allow(dead_code)]
mod cleanup_fixture;

#[cfg(feature = "autotag")]
#[test]
fn native_star_ratings_are_not_the_raw_vorbis_percentage() {
    let f = cleanup_fixture::Fixture::new(1);
    let mut tag = onetagger_tag::Tag::load_file(&f.paths[0], true).unwrap();
    tag.tag_mut().set_rating(4, true);
    tag.tag_mut().save_file(&f.paths[0]).unwrap();
    let info = f.rt.probe(&f.paths[0]).unwrap();
    assert_eq!(info.tags.get("rating").map(String::as_str), Some("80"));
    let conn = Connection::open(&f.db).unwrap();
    conn.execute(
        "UPDATE tracks SET metadata_json=?1,source_hash=?2 WHERE id=?3",
        params![
            serde_json::to_string(&info).unwrap(),
            cuerithm_media::file_sha256(&f.paths[0]).unwrap(),
            f.ids[0]
        ],
    )
    .unwrap();
    let before = std::fs::read(&f.paths[0]).unwrap();
    let preview = f
        .lib
        .preview_smart_list(
            definition("rating", Operator::NumberEquals, RuleValue::Number(4.)),
            SmartPreviewScope::default(),
            NOW,
        )
        .unwrap();
    assert_eq!(preview.track_ids, f.ids);
    assert_eq!(std::fs::read(&f.paths[0]).unwrap(), before);
}

#[test]
fn workspace_batches_tree_statuses_and_durable_latest_undo_without_changing_inputs() {
    let f = fixture();
    create(&f.lib, "folder", None, PlaylistKind::Folder, None);
    create(
        &f.lib,
        "recent",
        Some("folder"),
        PlaylistKind::Smartlist,
        Some(definition(
            "dateAdded",
            Operator::DateRecent,
            RuleValue::Relative {
                count: 1,
                unit: cuerithm_smartlists::DateUnit::Days,
            },
        )),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    let input = f.lib.smart_input_revision().unwrap();
    let state = f.lib.playlist_workspace_at(NOW).unwrap();
    assert_eq!(state.tree.nodes.len(), 2);
    assert!(state.latest_undo.is_some());
    assert!(state.statuses.iter().all(|s| s.state == SmartState::Ready));
    let expired = f.lib.playlist_workspace_at(NOW + 60_000).unwrap();
    assert!(
        expired
            .statuses
            .iter()
            .all(|s| s.state == SmartState::Stale)
    );
    create(
        &f.lib,
        "bad",
        Some("folder"),
        PlaylistKind::Smartlist,
        Some(definition(
            "cloudUpload",
            Operator::BooleanTrue,
            RuleValue::None,
        )),
    );
    f.lib.refresh_smart_lists(NOW + 1).unwrap();
    let state = f.lib.playlist_workspace_at(NOW + 1).unwrap();
    assert_eq!(
        state
            .statuses
            .iter()
            .find(|s| s.playlist_id == "folder")
            .unwrap()
            .state,
        SmartState::Error
    );
    assert!(
        state
            .statuses
            .iter()
            .find(|s| s.playlist_id == "folder")
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("bad")
    );
    let undo = state.latest_undo.unwrap();
    f.lib
        .undo_playlist_edit(&undo, state.tree.revision)
        .unwrap();
    assert!(
        f.lib
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .all(|n| n.id != "bad")
    );
    assert_eq!(f.lib.smart_input_revision().unwrap(), input);
}

#[test]
fn saving_smart_preview_pins_inputs_and_owner_and_consumes_only_on_success() {
    let f = fixture();
    let rules = definition("bpm", Operator::NumberEquals, RuleValue::Number(128.));
    let old = f
        .lib
        .preview_smart_list(rules.clone(), SmartPreviewScope::default(), NOW)
        .unwrap();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute(
        "UPDATE tracks SET title='Later title' WHERE id=?1",
        [&f.ids[0]],
    )
    .unwrap();
    assert!(
        f.lib
            .save_smart_preview(
                &old.id,
                None,
                "Captured".into(),
                None,
                old.stamp.playlist_revision
            )
            .is_err()
    );
    assert!(f.lib.playlist_state().unwrap().nodes.is_empty());
    let preview = f
        .lib
        .preview_smart_list(rules.clone(), SmartPreviewScope::default(), NOW)
        .unwrap();
    let saved = f
        .lib
        .save_smart_preview(
            &preview.id,
            None,
            "Captured".into(),
            None,
            preview.stamp.playlist_revision,
        )
        .unwrap();
    let id = saved.created_ids[0].clone();
    assert!(
        f.lib
            .save_smart_preview(
                &preview.id,
                None,
                "Again".into(),
                None,
                preview.stamp.playlist_revision
            )
            .is_err()
    );
    let other = f
        .lib
        .preview_smart_list(rules.clone(), SmartPreviewScope::default(), NOW)
        .unwrap();
    assert!(
        f.lib
            .save_smart_preview(
                &other.id,
                Some(&id),
                "Wrong owner".into(),
                None,
                other.stamp.playlist_revision
            )
            .is_err()
    );
    let editing = f
        .lib
        .preview_smart_list(
            rules,
            SmartPreviewScope {
                playlist_id: Some(id.clone()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
    f.lib
        .save_smart_preview(
            &editing.id,
            Some(&id),
            "Renamed".into(),
            None,
            editing.stamp.playlist_revision,
        )
        .unwrap();
    assert_eq!(f.lib.playlist_state().unwrap().nodes[0].name, "Renamed");
}
const NOW: i64 = 1792324800000;

#[test]
fn upgraded_evaluator_does_not_reuse_legacy_memberships_without_a_code_identity() {
    use sha2::{Digest, Sha256};
    let f = fixture();
    let rules = definition("bpm", Operator::NumberEquals, RuleValue::Number(128.));
    create(
        &f.lib,
        "smart",
        None,
        PlaylistKind::Smartlist,
        Some(rules.clone()),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    let prefs = f.lib.get_playlist_preferences().unwrap().preferences;
    let old = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&serde_json::json!({
                "definition": rules, "zone":"UTC", "mixing_mode":prefs.key_mixing_mode
            }))
            .unwrap()
        )
    );
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute(
        "UPDATE smart_playlist_status SET definition_hash=?1 WHERE playlist_id='smart'",
        [old],
    )
    .unwrap();
    let refreshed = f.lib.refresh_smart_lists(NOW + 1).unwrap();
    assert_eq!(refreshed.evaluated, vec!["smart".to_owned()]);
    assert!(refreshed.unchanged.is_empty());
}

#[test]
fn background_refresh_tracks_library_edits_and_stops_without_preparing_audio() {
    let f = fixture();
    create(
        &f.lib,
        "smart",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "bpm",
            Operator::NumberEquals,
            RuleValue::Number(128.),
        )),
    );
    let source = std::fs::read(f.dir.path().join("Alpha.flac")).unwrap();
    let analysis = serde_json::to_value(f.lib.analysis(&f.ids[0]).unwrap()).unwrap();
    let jobs = serde_json::to_value(f.lib.jobs().unwrap()).unwrap();
    let preparation = f.lib.settings().unwrap().revision;
    let cues = f.lib.cue_revision_count(&f.ids[0]).unwrap();
    let service = SmartPlaylistService::start(f.lib.clone()).unwrap();
    let wait = |expected: Vec<String>| {
        let started = std::time::Instant::now();
        loop {
            let status = f.lib.playlist_evaluation_status("smart").unwrap();
            if status.state == SmartState::Ready
                && f.lib.playlist_members("smart").unwrap() == expected
            {
                break;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(5),
                "{status:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    };
    wait(vec![f.ids[0].clone()]);
    f.lib.set_archived(&f.ids[0], true).unwrap();
    wait(vec![]);
    assert!(service.last_error().unwrap().is_none());
    service.shutdown();
    f.lib.set_archived(&f.ids[0], false).unwrap();
    assert_eq!(
        f.lib.playlist_evaluation_status("smart").unwrap().state,
        SmartState::Stale
    );
    let restarted = SmartPlaylistService::start(f.lib.clone()).unwrap();
    wait(vec![f.ids[0].clone()]);
    restarted.shutdown();
    assert_eq!(
        std::fs::read(f.dir.path().join("Alpha.flac")).unwrap(),
        source
    );
    assert_eq!(
        serde_json::to_value(f.lib.analysis(&f.ids[0]).unwrap()).unwrap(),
        analysis
    );
    assert_eq!(serde_json::to_value(f.lib.jobs().unwrap()).unwrap(), jobs);
    assert_eq!(f.lib.settings().unwrap().revision, preparation);
    assert_eq!(f.lib.cue_revision_count(&f.ids[0]).unwrap(), cues);
}

#[test]
fn dynamic_evaluation_cannot_publish_at_its_expiry_boundary() {
    let f = fixture();
    create(
        &f.lib,
        "recent",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "dateAdded",
            Operator::DateRecent,
            RuleValue::Relative {
                count: 1,
                unit: cuerithm_smartlists::DateUnit::Days,
            },
        )),
    );
    let prepared = f.lib.evaluate_smart_lists(NOW).unwrap();
    assert!(
        f.lib
            .commit_smart_evaluation(prepared, NOW + 60_000)
            .is_err()
    );
    assert!(f.lib.playlist_members("recent").unwrap().is_empty());
}

#[test]
fn corrected_clock_can_refresh_fresh_snapshots_without_overwriting_newer_work() {
    let f = fixture();
    create(
        &f.lib,
        "recent",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "dateAdded",
            Operator::DateRecent,
            RuleValue::Relative {
                count: 1,
                unit: cuerithm_smartlists::DateUnit::Days,
            },
        )),
    );
    f.lib.refresh_smart_lists(NOW + 2 * 86_400_000).unwrap();
    assert!(f.lib.playlist_members("recent").unwrap().is_empty());
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert_eq!(f.lib.playlist_members("recent").unwrap().len(), 2);
}

#[test]
fn dependent_status_and_preview_inherit_relative_expiry() {
    let f = fixture();
    create(
        &f.lib,
        "recent",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "dateAdded",
            Operator::DateRecent,
            RuleValue::Relative {
                count: 1,
                unit: cuerithm_smartlists::DateUnit::Days,
            },
        )),
    );
    let rules = definition(
        "playlist",
        Operator::PlaylistInSpecificSmartlist,
        RuleValue::Playlist(PlaylistReference {
            id: "recent".into(),
            kind: PlaylistKind::Smartlist,
            path: vec![],
        }),
    );
    create(
        &f.lib,
        "dependent",
        None,
        PlaylistKind::Smartlist,
        Some(rules.clone()),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert_eq!(
        f.lib
            .playlist_evaluation_status_at("dependent", NOW + 60_000)
            .unwrap()
            .state,
        SmartState::Stale
    );
    let preview = f
        .lib
        .preview_smart_list(
            rules,
            SmartPreviewScope {
                playlist_id: Some("dependent".into()),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
    assert_eq!(preview.expires, Some(NOW + 60_000));
}

#[test]
fn standard_metadata_fields_resolve_common_date_track_and_album_artist_tags() {
    let f = fixture();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    let text: String = conn
        .query_row(
            "SELECT metadata_json FROM tracks WHERE id=?1",
            [&f.ids[0]],
            |r| r.get(0),
        )
        .unwrap();
    let mut info: serde_json::Value = serde_json::from_str(&text).unwrap();
    info["tags"].as_object_mut().unwrap().remove("year");
    info["tags"]["date"] = "2024-09-21".into();
    info["tags"]["track"] = "7/12".into();
    info["tags"]["album_artist"] = "Album artist alias".into();
    conn.execute(
        "UPDATE tracks SET metadata_json=?1 WHERE id=?2",
        params![info.to_string(), f.ids[0]],
    )
    .unwrap();
    for (field, op, value) in [
        ("year", Operator::NumberEquals, RuleValue::Number(2024.)),
        ("trackNumber", Operator::NumberEquals, RuleValue::Number(7.)),
        (
            "albumArtist",
            Operator::StringEquals,
            RuleValue::Text("Album artist alias".into()),
        ),
        (
            "tag.track",
            Operator::StringEquals,
            RuleValue::Text("7/12".into()),
        ),
    ] {
        let preview = f
            .lib
            .preview_smart_list(
                definition(field, op, value),
                SmartPreviewScope::default(),
                NOW,
            )
            .unwrap();
        assert_eq!(preview.track_ids, vec![f.ids[0].clone()], "{field}");
    }
}

#[cfg(feature = "autotag")]
#[test]
fn cleanup_publication_invalidates_the_snapshot_without_playlist_file_writes() {
    let f = cleanup_fixture::Fixture::new(1);
    create(
        &f.lib,
        "smart",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "title",
            Operator::StringContains,
            RuleValue::Text("signal".into()),
        )),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    let captured = f.lib.evaluate_smart_lists(NOW + 1).unwrap();
    let cleanup = f.service();
    let preview = cleanup
        .preview_cleanup(CleanupScope::AllReady, cleanup_fixture::config())
        .unwrap();
    let applied = cleanup
        .apply_cleanup(
            &preview.id,
            &preview
                .proposals
                .iter()
                .map(|p| p.id.clone())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    assert_eq!(applied.applied, f.ids);
    let bytes = std::fs::read(&f.paths[0]).unwrap();
    let analysis = serde_json::to_value(f.lib.analysis(&f.ids[0]).unwrap()).unwrap();
    let cues = f.lib.cue_revision_count(&f.ids[0]).unwrap();
    assert!(f.lib.commit_smart_evaluation(captured, NOW + 2).is_err());
    f.lib.refresh_smart_lists(NOW + 2).unwrap();
    assert_eq!(f.lib.playlist_members("smart").unwrap(), f.ids);
    assert_eq!(std::fs::read(&f.paths[0]).unwrap(), bytes);
    assert_eq!(
        serde_json::to_value(f.lib.analysis(&f.ids[0]).unwrap()).unwrap(),
        analysis
    );
    assert_eq!(f.lib.cue_revision_count(&f.ids[0]).unwrap(), cues);
}

#[test]
fn real_preparation_publication_invalidates_an_earlier_smart_snapshot() {
    use std::{
        path::Path,
        time::{Duration, Instant},
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Prepared test.flac");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::copy(
        root.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &path,
    )
    .unwrap();
    let lib = Library::open(&dir.path().join("app/db")).unwrap();
    let mut settings = lib.settings().unwrap().settings;
    settings.engine.lexicon_energy = true;
    settings.stability_millis = 20;
    let revision = lib.save_settings(settings).unwrap();
    lib.enqueue(std::slice::from_ref(&path), revision).unwrap();
    create(
        &lib,
        "smart",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "bpm",
            Operator::NumberGreaterThan,
            RuleValue::Number(100.),
        )),
    );
    let captured = lib.evaluate_smart_lists(NOW).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let rt =
        cuerithm_media::MediaRuntime::from_bundle(&root.join("tools/media-runtime/bin")).unwrap();
    let worker = PreparationService::start_with_workers(lib.clone(), rt.clone(), 1).unwrap();
    let start = Instant::now();
    while lib.counts().unwrap().tracks != 1 {
        let jobs = lib.jobs().unwrap();
        assert!(!jobs.iter().any(|j| j.state == "attention"), "{jobs:?}");
        assert!(start.elapsed() < Duration::from_secs(60), "{jobs:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
    worker.shutdown();
    let id = lib.jobs().unwrap()[0].track_id.clone();
    let analysis = serde_json::to_value(lib.analysis(&id).unwrap()).unwrap();
    let cues = lib.cue_revision_count(&id).unwrap();
    assert!(lib.commit_smart_evaluation(captured, NOW + 1).is_err());
    lib.refresh_smart_lists(NOW + 1).unwrap();
    assert_eq!(lib.playlist_members("smart").unwrap(), vec![id.clone()]);
    let audio = rt.decode(&path, &dir.path().join("decoded")).unwrap();
    let cache = lib.data_dir().join("cache/waveforms");
    cuerithm_library::waveform::cached_waveform(&audio, 16384, &cache).unwrap();
    let wave_path = std::fs::read_dir(&cache)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let good = std::fs::read(&wave_path).unwrap();
    let wave_rule = definition("waveform", Operator::BooleanTrue, RuleValue::None);
    create(
        &lib,
        "wave",
        None,
        PlaylistKind::Smartlist,
        Some(wave_rule.clone()),
    );
    lib.refresh_smart_lists(NOW + 2).unwrap();
    assert_eq!(lib.playlist_members("wave").unwrap(), vec![id.clone()]);
    let mut bad: serde_json::Value = serde_json::from_slice(&good).unwrap();
    bad["bins"][0]["min"] = 2.into();
    bad["bins"][0]["max"] = (-2).into();
    std::fs::write(&wave_path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(
        lib.preview_smart_list(wave_rule, SmartPreviewScope::default(), NOW + 3)
            .is_err()
    );
    let captured = lib.evaluate_smart_lists(NOW + 60_002).unwrap();
    std::fs::write(&wave_path, good).unwrap();
    assert!(lib.commit_smart_evaluation(captured, NOW + 60_003).is_err());
    std::fs::remove_file(wave_path).unwrap();
    lib.refresh_smart_lists(NOW + 60_003).unwrap();
    assert!(lib.playlist_members("wave").unwrap().is_empty());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_eq!(
        serde_json::to_value(lib.analysis(&id).unwrap()).unwrap(),
        analysis
    );
    assert_eq!(lib.cue_revision_count(&id).unwrap(), cues);
    assert_eq!(lib.jobs().unwrap()[0].attempts, 1);
}
struct Fixture {
    dir: tempfile::TempDir,
    lib: Library,
    ids: Vec<String>,
}
fn definition(field: &str, op: Operator, value: RuleValue) -> SmartDefinition {
    SmartDefinition {
        match_all: true,
        rules: vec![SmartRule {
            id: "rule".into(),
            field: field.into(),
            operator: op,
            value,
            or: false,
        }],
    }
}
fn create(
    lib: &Library,
    id: &str,
    parent: Option<&str>,
    kind: PlaylistKind,
    rules: Option<SmartDefinition>,
) {
    lib.apply_playlist_edit(
        PlaylistEdit::Create {
            id: Some(id.into()),
            parent_id: parent.map(str::to_owned),
            kind,
            name: id.into(),
            definition: rules,
        },
        lib.playlist_state().unwrap().revision,
    )
    .unwrap();
}
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let lib = Library::open(&dir.path().join("db")).unwrap();
    let mut ids = vec![];
    let preferences = lib.get_playlist_preferences().unwrap();
    lib.save_playlist_preferences(
        PlaylistPreferences {
            time_zone: Some("UTC".into()),
            key_mixing_mode: KeyMixingMode::Harmonic,
            ..Default::default()
        },
        preferences.revision,
    )
    .unwrap();
    for (index, title) in ["Alpha", "Beta", "Incoming"].iter().enumerate() {
        let path = dir.path().join(format!("{title}.flac"));
        std::fs::write(
            &path,
            b"synthetic input; no DSP is invoked in a playlist query",
        )
        .unwrap();
        let job = lib
            .enqueue(&[path], lib.settings().unwrap().revision)
            .unwrap()[0]
            .clone();
        let id = lib
            .jobs()
            .unwrap()
            .iter()
            .find(|j| j.id == job)
            .unwrap()
            .track_id
            .clone();
        ids.push(id.clone());
        if index == 2 {
            continue;
        }
        let info = serde_json::json!({"codec":"flac","sample_rate":44100,"channels":2,"bits_per_sample":16,"tags":{"artist":"Earth, Wind & Fire","bpm":"333","initialkey":"1B","energy":"1","label":"flattened label","year":"2025"},"properties":{"audio.bit_rate":"320000","format.size":"10000"}});
        let analysis = serde_json::json!({"analysis":{"engine":"fixture","engine_source_sha256":"fixture","config_sha256":"fixture","duration_seconds":240.0,"sample_rate":44100,"key":{"key":{"tonic":9,"mode":"minor"},"camelot":"8A","margin":0.1,"candidates":[]},"tempo":{"bpm":if index==0{128.0}else{130.0},"candidates":[],"beats_seconds":[0.0,0.46875],"downbeats_seconds":null,"method":"fixture"},"cues":null,"lexicon_cues":{"cues":[{"time_seconds":1.0,"name":"Intro","color":"#ff0000"}],"emergency_loop":{"time_seconds":200.0,"loop_beats":16}},"energy":{"level":if index==0{6}else{8},"method":"fixture"}},"complete":true,"verified_integrity":true,"source_sample_rate":44100,"elapsed_seconds":0.0});
        let _: camelot_core::FileAnalysis = serde_json::from_value(analysis.clone()).unwrap();
        let native = serde_json::json!({"fields":{"title":[title],"artist":["Earth, Wind & Fire"],"genre":["House"],"label":["Native label"]},"custom_tags":[]});
        let conn = Connection::open(dir.path().join("db")).unwrap();
        conn.execute("update tracks set state='ready',title=?1,sort_title=lower(?1),search_text=lower(?1),artist='Earth, Wind & Fire',genre='House',bpm=?2,energy=?3,musical_key='8A',duration=240,metadata_json=?4,native_metadata_json=?5,analysis_json=?6,created=?7,updated=?7 where id=?8",params![title,if index==0{128.0}else{130.0},if index==0{6}else{8},info.to_string(),native.to_string(),analysis.to_string(),NOW-3600000,id]).unwrap();
        conn.execute(
            "update jobs set state='completed',stage='completed' where id=?1",
            [&job],
        )
        .unwrap();
        conn.execute(
            "insert into cue_revisions values(?1,?2,?3,?4,1)",
            params![
                format!("cue-{index}"),
                id,
                job,
                analysis["analysis"]["lexicon_cues"].to_string()
            ],
        )
        .unwrap();
    }
    let conn = Connection::open(dir.path().join("db")).unwrap();
    conn.execute_batch("insert into custom_tag_categories values('category','Mood');insert into custom_tags values('tag','category','Dark');").unwrap();
    conn.execute("insert into track_custom_tags values(?1,'tag')", [&ids[0]])
        .unwrap();
    Fixture { dir, lib, ids }
}
#[test]
fn real_snapshot_uses_measured_analysis_native_fields_tags_and_current_cue_facts() {
    let f = fixture();
    let before = f.lib.analysis(&f.ids[0]).unwrap().unwrap();
    let cue_count = f.lib.cue_revision_count(&f.ids[0]).unwrap();
    for (id, rules) in [
        (
            "bpm",
            definition("bpm", Operator::NumberEquals, RuleValue::Number(128.)),
        ),
        (
            "native",
            definition(
                "label",
                Operator::StringEquals,
                RuleValue::Text("Native label".into()),
            ),
        ),
        (
            "tagged",
            definition(
                "tags",
                Operator::TagsIncludes,
                RuleValue::Ids(vec!["tag".into()]),
            ),
        ),
        (
            "loop",
            definition(
                "cuepoints",
                Operator::CuepointHasActiveLoop,
                RuleValue::None,
            ),
        ),
    ] {
        create(&f.lib, id, None, PlaylistKind::Smartlist, Some(rules));
    }
    let refresh = f.lib.refresh_smart_lists(NOW).unwrap();
    assert!(refresh.errors.is_empty());
    assert_eq!(
        f.lib.playlist_members("bpm").unwrap(),
        vec![f.ids[0].clone()]
    );
    assert_eq!(
        f.lib.playlist_members("tagged").unwrap(),
        vec![f.ids[0].clone()]
    );
    assert_eq!(f.lib.playlist_members("native").unwrap().len(), 2);
    assert_eq!(f.lib.playlist_members("loop").unwrap().len(), 2);
    let catalog = f.lib.smart_field_catalog().unwrap();
    assert!(catalog.catalog.fields.contains_key("tag.bpm"));
    assert!(!catalog.catalog.fields.contains_key("danceability"));
    assert!(
        catalog.catalog.fields["popularity"]
            .unavailable_reason
            .is_some()
    );
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(f.lib.analysis(&f.ids[0]).unwrap().unwrap()).unwrap()
    );
    assert_eq!(f.lib.cue_revision_count(&f.ids[0]).unwrap(), cue_count);
}
#[test]
fn dependencies_publish_together_and_errors_retain_last_good_without_partial_dependents() {
    let f = fixture();
    create(&f.lib, "folder", None, PlaylistKind::Folder, None);
    create(
        &f.lib,
        "z-source",
        Some("folder"),
        PlaylistKind::Smartlist,
        Some(definition(
            "bpm",
            Operator::NumberEquals,
            RuleValue::Number(128.),
        )),
    );
    create(
        &f.lib,
        "a-dependent",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "playlist",
            Operator::PlaylistInSpecificPlaylistFolder,
            RuleValue::Playlist(PlaylistReference {
                id: "folder".into(),
                kind: PlaylistKind::Folder,
                path: vec![],
            }),
        )),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert_eq!(
        f.lib.playlist_members("a-dependent").unwrap(),
        vec![f.ids[0].clone()]
    );
    let bad = definition(
        "tags",
        Operator::TagsIncludes,
        RuleValue::Ids(vec!["deleted".into()]),
    );
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::SetDefinition {
                id: "z-source".into(),
                definition: bad,
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let result = f.lib.refresh_smart_lists(NOW + 1).unwrap();
    assert_eq!(result.errors.len(), 2);
    assert_eq!(
        f.lib.playlist_members("z-source").unwrap(),
        vec![f.ids[0].clone()]
    );
    assert_eq!(
        f.lib.playlist_members("a-dependent").unwrap(),
        vec![f.ids[0].clone()]
    );
    assert_eq!(
        f.lib
            .playlist_evaluation_status("a-dependent")
            .unwrap()
            .state,
        SmartState::Error
    );
    assert_eq!(
        f.lib.playlist_evaluation_status("folder").unwrap().state,
        SmartState::Error
    );
}
#[test]
fn stale_evaluations_are_rejected_after_data_tree_or_preference_changes() {
    let f = fixture();
    create(
        &f.lib,
        "smart",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "bpm",
            Operator::NumberEquals,
            RuleValue::Number(128.),
        )),
    );
    let prepared = f.lib.evaluate_smart_lists(NOW).unwrap();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute("update tracks set title='Edited' where id=?1", [&f.ids[0]])
        .unwrap();
    assert!(f.lib.commit_smart_evaluation(prepared, NOW).is_err());
    assert!(f.lib.playlist_members("smart").unwrap().is_empty());
    let prepared = f.lib.evaluate_smart_lists(NOW).unwrap();
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::Rename {
                id: "smart".into(),
                name: "Changed".into(),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    assert!(f.lib.commit_smart_evaluation(prepared, NOW).is_err());
    let prepared = f.lib.evaluate_smart_lists(NOW).unwrap();
    let prefs = f.lib.get_playlist_preferences().unwrap();
    f.lib
        .save_playlist_preferences(
            PlaylistPreferences {
                time_zone: Some("Europe/Amsterdam".into()),
                key_mixing_mode: KeyMixingMode::Fuzzy,
                ..Default::default()
            },
            prefs.revision,
        )
        .unwrap();
    assert!(f.lib.commit_smart_evaluation(prepared, NOW).is_err());
}
#[test]
fn progress_and_result_publication_do_not_cause_an_endless_refresh_loop() {
    let f = fixture();
    create(
        &f.lib,
        "smart",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "bpm",
            Operator::NumberGreaterThan,
            RuleValue::Number(100.),
        )),
    );
    let prepared = f.lib.evaluate_smart_lists(NOW).unwrap();
    let input = f.lib.smart_input_revision().unwrap();
    let job = f.lib.claim_next().unwrap().unwrap();
    f.lib.set_stage(&job.id, "decoding").unwrap();
    f.lib.commit_smart_evaluation(prepared, NOW).unwrap();
    assert_eq!(f.lib.smart_input_revision().unwrap(), input);
    let again = f.lib.refresh_smart_lists(NOW + 1).unwrap();
    assert!(again.evaluated.is_empty());
    assert_eq!(again.unchanged, vec!["smart".to_owned()]);
}
#[test]
fn independent_preferences_never_change_preparation_or_enqueue_work() {
    let f = fixture();
    let settings = f.lib.settings().unwrap().revision;
    let jobs = f.lib.jobs().unwrap().len();
    let input = f.lib.smart_input_revision().unwrap();
    let prefs = f.lib.get_playlist_preferences().unwrap();
    let saved = f
        .lib
        .save_playlist_preferences(
            PlaylistPreferences {
                time_zone: Some("Asia/Tokyo".into()),
                key_mixing_mode: KeyMixingMode::Fuzzy,
                ..Default::default()
            },
            prefs.revision,
        )
        .unwrap();
    assert!(saved.revision > prefs.revision);
    assert_eq!(f.lib.settings().unwrap().revision, settings);
    assert_eq!(f.lib.jobs().unwrap().len(), jobs);
    assert_eq!(f.lib.smart_input_revision().unwrap(), input);
    assert!(
        f.lib
            .save_playlist_preferences(saved.preferences, prefs.revision)
            .is_err()
    );
}
#[test]
fn playlist_query_cursors_selection_and_manual_order_stay_bound_to_scope() {
    let f = fixture();
    for id in ["a", "b"] {
        create(&f.lib, id, None, PlaylistKind::Playlist, None);
    }
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "a".into(),
                track_ids: vec![f.ids[1].clone(), f.ids[0].clone()],
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "b".into(),
                track_ids: vec![f.ids[0].clone()],
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let query = TrackQuery {
        playlist_id: Some("a".into()),
        sort: TrackSort::PlaylistOrder,
        limit: 1,
        ..Default::default()
    };
    let page = f.lib.tracks(query.clone()).unwrap();
    assert_eq!(page.rows[0].id, f.ids[1]);
    assert_eq!(
        f.lib.track_ids(query.clone()).unwrap(),
        vec![f.ids[1].clone(), f.ids[0].clone()]
    );
    assert!(
        f.lib
            .tracks(TrackQuery {
                playlist_id: Some("b".into()),
                cursor: page.next_cursor,
                ..query
            })
            .is_err()
    );
}
#[test]
fn preview_and_relative_cutoffs_are_clocked_pinned_and_never_return_partial_success() {
    let f = fixture();
    let rules = definition(
        "dateAdded",
        Operator::DateRecent,
        RuleValue::Relative {
            count: 1,
            unit: cuerithm_smartlists::DateUnit::Days,
        },
    );
    create(
        &f.lib,
        "recent",
        None,
        PlaylistKind::Smartlist,
        Some(rules.clone()),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert_eq!(f.lib.playlist_members("recent").unwrap().len(), 2);
    let preview = f
        .lib
        .preview_smart_list(rules, SmartPreviewScope::default(), NOW)
        .unwrap();
    assert_eq!(preview.track_ids.len(), 2);
    assert!(
        f.lib
            .preview_smart_list(
                definition("cloudUpload", Operator::BooleanTrue, RuleValue::None),
                SmartPreviewScope::default(),
                NOW
            )
            .is_err()
    );
    let prepared = f.lib.evaluate_smart_lists(NOW + 60000).unwrap();
    assert!(
        f.lib
            .commit_smart_evaluation(prepared, NOW + 120001)
            .is_err()
    );
    f.lib.refresh_smart_lists(NOW + 2 * 86400000).unwrap();
    assert!(f.lib.playlist_members("recent").unwrap().is_empty());
}

#[test]
fn native_import_facts_are_explicit_and_latest_revision_wins_without_rewriting_analysis() {
    let f = fixture();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    let job = f
        .lib
        .jobs()
        .unwrap()
        .iter()
        .find(|j| j.track_id == f.ids[0])
        .unwrap()
        .id
        .clone();
    let facts = NativePlaylistFacts {
        version: 1,
        source: "synthetic-native-import".into(),
        cues: vec![cuerithm_smartlists::SmartCue {
            label: Some("Marker".into()),
            color: Some("#00ff00".into()),
            kind: cuerithm_smartlists::CueKind::Loop,
            active_loop: true,
            memory: true,
        }],
        tempos: vec![128., 130.],
    };
    let current = serde_json::json!({"cues":[],"emergency_loop":null,"native_facts":facts});
    conn.execute(
        "insert into cue_revisions values('native',?1,?2,?3,2)",
        params![f.ids[0], job, current.to_string()],
    )
    .unwrap();
    create(
        &f.lib,
        "memory",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "cuepoints",
            Operator::CuepointHasMemoryCue,
            RuleValue::None,
        )),
    );
    create(
        &f.lib,
        "variable",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "tempomarkers",
            Operator::TempomarkerNotSameBpm,
            RuleValue::None,
        )),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert_eq!(
        f.lib.playlist_members("memory").unwrap(),
        vec![f.ids[0].clone()]
    );
    assert_eq!(
        f.lib.playlist_members("variable").unwrap(),
        vec![f.ids[0].clone()]
    );
}
#[test]
fn older_clocked_results_and_partial_sql_publications_are_rejected_atomically() {
    let f = fixture();
    for id in ["a", "b"] {
        create(
            &f.lib,
            id,
            None,
            PlaylistKind::Smartlist,
            Some(definition(
                "bpm",
                Operator::NumberGreaterThan,
                RuleValue::Number(100.),
            )),
        );
    }
    let older = f.lib.evaluate_smart_lists(NOW).unwrap();
    let newer = f.lib.evaluate_smart_lists(NOW + 1).unwrap();
    f.lib.commit_smart_evaluation(newer, NOW + 1).unwrap();
    assert!(f.lib.commit_smart_evaluation(older, NOW + 1).is_err());
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::SetDefinition {
                id: "a".into(),
                definition: definition("bpm", Operator::NumberEquals, RuleValue::Number(128.)),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::SetDefinition {
                id: "b".into(),
                definition: definition("bpm", Operator::NumberEquals, RuleValue::Number(128.)),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute_batch("CREATE TRIGGER injected_smart_failure BEFORE INSERT ON smart_playlist_members WHEN NEW.playlist_id='b' BEGIN SELECT RAISE(FAIL,'injected'); END;").unwrap();
    let before = f.lib.revision().unwrap();
    assert!(f.lib.refresh_smart_lists(NOW + 2).is_err());
    assert_eq!(f.lib.playlist_members("a").unwrap().len(), 2);
    assert_eq!(f.lib.playlist_members("b").unwrap().len(), 2);
    assert_eq!(f.lib.revision().unwrap(), before);
}
#[test]
fn missing_files_refresh_on_time_and_changed_observations_cannot_publish() {
    let f = fixture();
    create(
        &f.lib,
        "missing",
        None,
        PlaylistKind::Smartlist,
        Some(definition(
            "CustomFileMissing",
            Operator::BooleanTrue,
            RuleValue::None,
        )),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert!(f.lib.playlist_members("missing").unwrap().is_empty());
    let prepared = f.lib.evaluate_smart_lists(NOW + 60000).unwrap();
    std::fs::remove_file(f.dir.path().join("Alpha.flac")).unwrap();
    assert!(
        f.lib
            .commit_smart_evaluation(prepared, NOW + 60000)
            .is_err()
    );
    f.lib.refresh_smart_lists(NOW + 60001).unwrap();
    assert_eq!(
        f.lib.playlist_members("missing").unwrap(),
        vec![f.ids[0].clone()]
    );
}
#[test]
fn archive_rules_and_folder_queries_preserve_archived_manual_intent() {
    let f = fixture();
    create(&f.lib, "folder", None, PlaylistKind::Folder, None);
    create(
        &f.lib,
        "manual",
        Some("folder"),
        PlaylistKind::Playlist,
        None,
    );
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "manual".into(),
                track_ids: vec![f.ids[0].clone()],
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.lib.set_archived(&f.ids[0], true).unwrap();
    let q = TrackQuery {
        playlist_id: Some("folder".into()),
        ..Default::default()
    };
    assert_eq!(f.lib.tracks(q.clone()).unwrap().total, 0);
    assert_eq!(
        f.lib
            .tracks(TrackQuery {
                view: TrackView::Archive,
                ..q.clone()
            })
            .unwrap()
            .total,
        1
    );
    create(
        &f.lib,
        "archived",
        Some("folder"),
        PlaylistKind::Smartlist,
        Some(definition(
            "archived",
            Operator::BooleanTrue,
            RuleValue::None,
        )),
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    let folder_rows = f.lib.tracks(q).unwrap();
    assert_eq!(folder_rows.total, 1);
    assert!(folder_rows.rows[0].archived);
    let smart_rows = f
        .lib
        .tracks(TrackQuery {
            playlist_id: Some("archived".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(smart_rows.rows.len(), 1);
    assert!(smart_rows.rows[0].archived);
    assert!(f.lib.track(&f.ids[0]).unwrap().archived);
    assert!(
        f.lib
            .tracks(TrackQuery::default())
            .unwrap()
            .rows
            .iter()
            .all(|row| !row.archived)
    );
}
#[test]
fn previews_reject_cycles_and_selection_does_not_restrict_dependency_evaluation() {
    let f = fixture();
    create(&f.lib, "folder", None, PlaylistKind::Folder, None);
    create(
        &f.lib,
        "child",
        Some("folder"),
        PlaylistKind::Smartlist,
        Some(definition(
            "bpm",
            Operator::NumberEquals,
            RuleValue::Number(128.),
        )),
    );
    let cycle = definition(
        "playlist",
        Operator::PlaylistInSpecificPlaylistFolder,
        RuleValue::Playlist(PlaylistReference {
            id: "folder".into(),
            kind: PlaylistKind::Folder,
            path: vec![],
        }),
    );
    assert!(
        f.lib
            .preview_smart_list(
                cycle,
                SmartPreviewScope {
                    playlist_id: Some("child".into()),
                    ..Default::default()
                },
                NOW
            )
            .is_err()
    );
    let member = definition(
        "playlist",
        Operator::PlaylistInSpecificSmartlist,
        RuleValue::Playlist(PlaylistReference {
            id: "child".into(),
            kind: PlaylistKind::Smartlist,
            path: vec![],
        }),
    );
    let preview = f
        .lib
        .preview_smart_list(
            member,
            SmartPreviewScope {
                track_ids: Some(vec![f.ids[1].clone()]),
                ..Default::default()
            },
            NOW,
        )
        .unwrap();
    assert!(preview.track_ids.is_empty());
}

#[test]
fn published_cue_revisions_supply_names_colors_loop_and_memory_facts() {
    let f = fixture();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    let job = f
        .lib
        .jobs()
        .unwrap()
        .into_iter()
        .find(|j| j.track_id == f.ids[0])
        .unwrap()
        .id;
    let json = serde_json::json!({"schema":"lexicon-publication-v1","grid":{"bpm":128.35,"startTime":0.125},"cues":[{"name":"S.O.S.","startTime":128.,"type":"5","position":7,"color":"magenta","endTime":136.,"activeLoop":true,"data":{"rekordbox":{"originalNum":-1}}}],"implementation_sha256":"fixture","inputs_sha256":"fixture"});
    conn.execute(
        "INSERT INTO cue_revisions VALUES('published',?1,?2,?3,3)",
        params![f.ids[0], job, json.to_string()],
    )
    .unwrap();
    drop(conn);
    for (name, op) in [
        ("memory", Operator::CuepointHasMemoryCue),
        ("active", Operator::CuepointHasActiveLoop),
    ] {
        create(
            &f.lib,
            name,
            None,
            PlaylistKind::Smartlist,
            Some(definition("cuepoints", op, RuleValue::None)),
        );
    }
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert_eq!(
        f.lib.playlist_members("memory").unwrap(),
        vec![f.ids[0].clone()]
    );
    // Beta retains its legacy emergency-loop facts alongside Alpha's publication.
    assert_eq!(f.lib.playlist_members("active").unwrap(), f.ids[..2]);
}

#[test]
fn release_dates_from_full_tags_use_the_library_calendar_for_preview_cache_and_refresh() {
    use cuerithm_smartlists::{DateUnit, calendar_day_epoch};
    let f = fixture();
    let preferences = f.lib.get_playlist_preferences().unwrap();
    f.lib
        .save_playlist_preferences(
            PlaylistPreferences {
                time_zone: Some("America/Los_Angeles".into()),
                key_mixing_mode: KeyMixingMode::Harmonic,
                ..Default::default()
            },
            preferences.revision,
        )
        .unwrap();
    let day = calendar_day_epoch("2026-10-18", "America/Los_Angeles").unwrap();
    let previous = calendar_day_epoch("2026-10-17", "America/Los_Angeles").unwrap();
    let next = calendar_day_epoch("2026-10-19", "America/Los_Angeles").unwrap();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    for alias in [
        "releasedate",
        "release_date",
        "release date",
        "tdrl",
        "date",
    ] {
        conn.execute(
            "UPDATE tracks SET metadata_json=json_set(metadata_json,'$.tags',json(?1)) WHERE id=?2",
            params![
                serde_json::json!({alias:"2026-10-18"}).to_string(),
                f.ids[0]
            ],
        )
        .unwrap();
        for (operator, value) in [
            (Operator::DateEquals, RuleValue::Date(day)),
            (
                Operator::DateBetween,
                RuleValue::DateRange {
                    start: previous,
                    end: day,
                },
            ),
            (
                Operator::DateRecent,
                RuleValue::Relative {
                    count: 1,
                    unit: DateUnit::Days,
                },
            ),
        ] {
            let preview = f
                .lib
                .preview_smart_list(
                    definition("releaseDate", operator, value),
                    SmartPreviewScope::default(),
                    next - 1,
                )
                .unwrap();
            assert_eq!(preview.track_ids, [f.ids[0].clone()], "release tag {alias}");
        }
    }
    let rules = definition("releaseDate", Operator::DateEquals, RuleValue::Date(day));
    create(
        &f.lib,
        "released",
        None,
        PlaylistKind::Smartlist,
        Some(rules),
    );
    f.lib.refresh_smart_lists(next - 1).unwrap();
    assert_eq!(
        f.lib.playlist_members("released").unwrap(),
        [f.ids[0].clone()]
    );
    assert_eq!(
        f.lib
            .track_ids(TrackQuery {
                playlist_id: Some("released".into()),
                ..Default::default()
            })
            .unwrap(),
        [f.ids[0].clone()]
    );
    conn.execute("UPDATE tracks SET metadata_json=json_set(metadata_json,'$.tags.date','2026-10-17') WHERE id=?1", [&f.ids[0]]).unwrap();
    assert_eq!(
        f.lib
            .playlist_evaluation_status_at("released", next - 1)
            .unwrap()
            .state,
        SmartState::Stale
    );
    f.lib.refresh_smart_lists(next - 1).unwrap();
    assert!(f.lib.playlist_members("released").unwrap().is_empty());
}

#[test]
fn partial_invalid_and_timestamp_release_tags_stay_unknown_without_inventing_days() {
    use cuerithm_smartlists::calendar_day_epoch;
    let f = fixture();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    for value in [
        "2026",
        "2026-10",
        "2026-02-30",
        "2026-10-18T00:00:00Z",
        "",
        "not a date",
    ] {
        conn.execute(
            "UPDATE tracks SET metadata_json=json_set(metadata_json,'$.tags',json(?1)) WHERE id=?2",
            params![serde_json::json!({"date":value}).to_string(), f.ids[0]],
        )
        .unwrap();
        let preview = f
            .lib
            .preview_smart_list(
                definition(
                    "releaseDate",
                    Operator::DateBetween,
                    RuleValue::DateRange {
                        start: calendar_day_epoch("2026-01-01", "UTC").unwrap(),
                        end: calendar_day_epoch("2026-12-31", "UTC").unwrap(),
                    },
                ),
                SmartPreviewScope::default(),
                NOW,
            )
            .unwrap();
        assert!(preview.track_ids.is_empty(), "unknown date {value}");
    }
    conn.execute(
        "UPDATE tracks SET metadata_json=json_set(metadata_json,'$.tags',json(?1)) WHERE id=?2",
        params![
            serde_json::json!({"date":"2026-10", "releasedate":"2026-10-18"}).to_string(),
            f.ids[0]
        ],
    )
    .unwrap();
    assert_eq!(
        f.lib
            .preview_smart_list(
                definition(
                    "releaseDate",
                    Operator::DateEquals,
                    RuleValue::Date(calendar_day_epoch("2026-10-18", "UTC").unwrap())
                ),
                SmartPreviewScope::default(),
                NOW
            )
            .unwrap()
            .track_ids,
        [f.ids[0].clone()]
    );
}

#[test]
fn incoming_opt_in_is_consistent_across_preview_memberships_folders_paging_ids_and_lifecycle() {
    let f = fixture();
    let incoming = definition("incoming", Operator::BooleanTrue, RuleValue::None);
    assert_eq!(
        f.lib
            .preview_smart_list(incoming.clone(), SmartPreviewScope::default(), NOW)
            .unwrap()
            .track_ids,
        [f.ids[2].clone()]
    );
    assert_eq!(
        f.lib
            .preview_smart_list(
                definition("incoming", Operator::BooleanFalse, RuleValue::None),
                SmartPreviewScope::default(),
                NOW
            )
            .unwrap()
            .track_ids,
        f.ids[..2]
    );
    create(&f.lib, "folder", None, PlaylistKind::Folder, None);
    // The manual path contributes no Incoming opt-in; the later smart path must add it.
    create(
        &f.lib,
        "manual",
        Some("folder"),
        PlaylistKind::Playlist,
        None,
    );
    create(
        &f.lib,
        "incoming",
        Some("folder"),
        PlaylistKind::Smartlist,
        Some(incoming),
    );
    create(
        &f.lib,
        "default",
        None,
        PlaylistKind::Smartlist,
        Some(definition("title", Operator::Exists, RuleValue::None)),
    );
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "manual".into(),
                track_ids: vec![f.ids[0].clone(), f.ids[2].clone()],
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert_eq!(
        f.lib.playlist_members("incoming").unwrap(),
        [f.ids[2].clone()]
    );
    assert_eq!(f.lib.playlist_members("default").unwrap(), f.ids[..2]);
    for scope in ["incoming", "folder"] {
        let q = TrackQuery {
            playlist_id: Some(scope.into()),
            limit: 1,
            ..Default::default()
        };
        let expected = if scope == "incoming" {
            vec![f.ids[2].clone()]
        } else {
            vec![f.ids[0].clone(), f.ids[2].clone()]
        };
        assert_eq!(f.lib.track_ids(q.clone()).unwrap(), expected);
        let first = f.lib.tracks(q.clone()).unwrap();
        let mut rows = first.rows;
        if let Some(cursor) = first.next_cursor {
            rows.extend(
                f.lib
                    .tracks(TrackQuery {
                        cursor: Some(cursor),
                        ..q.clone()
                    })
                    .unwrap()
                    .rows,
            );
        }
        assert_eq!(rows.into_iter().map(|r| r.id).collect::<Vec<_>>(), expected);
        let incoming_rows = f
            .lib
            .tracks(TrackQuery {
                view: TrackView::Incoming,
                ..q
            })
            .unwrap();
        assert_eq!(incoming_rows.rows[0].id, f.ids[2]);
        assert_eq!(incoming_rows.rows[0].state, "incoming");
    }
    assert_eq!(f.lib.track_ids(TrackQuery::default()).unwrap(), f.ids[..2]);
    assert_eq!(
        f.lib
            .track_ids(TrackQuery {
                playlist_id: Some("manual".into()),
                ..Default::default()
            })
            .unwrap(),
        [f.ids[0].clone()]
    );
    // Explicit smart scope also feeds playlist tools, independently of cached SQL queries.
    let copied = f
        .lib
        .preview_playlist_tool(PlaylistToolRequest::Snapshot {
            source_id: "incoming".into(),
            parent_id: None,
            name: "Incoming snapshot".into(),
        })
        .unwrap();
    let copied = f
        .lib
        .apply_playlist_tool(&copied.id, copied.stamp.playlist_revision)
        .unwrap();
    assert_eq!(
        f.lib
            .manual_playlist_members(&copied.created_ids[0])
            .unwrap(),
        [f.ids[2].clone()]
    );
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute("UPDATE tracks SET state='ready' WHERE id=?1", [&f.ids[2]])
        .unwrap();
    assert_eq!(
        f.lib
            .playlist_evaluation_status_at("incoming", NOW)
            .unwrap()
            .state,
        SmartState::Stale
    );
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert!(f.lib.playlist_members("incoming").unwrap().is_empty());
    assert_eq!(f.lib.playlist_members("default").unwrap(), f.ids);
}

#[test]
fn ordinary_generation_and_library_complement_stay_prepared_only_with_incoming_candidates() {
    let f = fixture();
    for title in ["Incoming only", "Alpha"] {
        let conn = Connection::open(f.dir.path().join("db")).unwrap();
        conn.execute(
            "UPDATE tracks SET title=?1 WHERE id=?2",
            params![title, f.ids[2]],
        )
        .unwrap();
        let preview = f
            .lib
            .preview_smart_generation(SmartGenerationRequest {
                target: GenerationTarget::NewFolder {
                    parent_id: None,
                    name: "Generated".into(),
                },
                dimensions: vec![GenerationDimension::Field {
                    field: "title".into(),
                    include_unobserved: false,
                }],
                combine: false,
            })
            .unwrap();
        assert_eq!(
            preview
                .items
                .iter()
                .map(|item| (item.name.as_str(), item.track_count))
                .collect::<Vec<_>>(),
            [("Alpha", 1), ("Beta", 1)]
        );
    }
    create(&f.lib, "base", None, PlaylistKind::Playlist, None);
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::AddMembers {
                id: "base".into(),
                track_ids: vec![f.ids[0].clone()],
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    let preview = f
        .lib
        .preview_playlist_tool(PlaylistToolRequest::Combine {
            source_ids: vec!["base".into()],
            parent_id: None,
            name: "Complement".into(),
            mode: PlaylistCombineMode::AbsentFromAll,
        })
        .unwrap();
    let change = f
        .lib
        .apply_playlist_tool(&preview.id, preview.stamp.playlist_revision)
        .unwrap();
    assert_eq!(
        f.lib
            .manual_playlist_members(&change.created_ids[0])
            .unwrap(),
        [f.ids[1].clone()]
    );
}

#[test]
fn archived_incoming_tracks_require_both_opt_ins_in_smart_evaluation_and_tables() {
    let f = fixture();
    f.lib.set_archived(&f.ids[2], true).unwrap();
    for field in ["incoming", "archived"] {
        assert!(
            f.lib
                .preview_smart_list(
                    definition(field, Operator::BooleanTrue, RuleValue::None),
                    SmartPreviewScope::default(),
                    NOW
                )
                .unwrap()
                .track_ids
                .is_empty()
        );
    }
    let mut both = definition("incoming", Operator::BooleanTrue, RuleValue::None);
    both.rules.push(SmartRule {
        id: "archive".into(),
        field: "archived".into(),
        operator: Operator::BooleanTrue,
        value: RuleValue::None,
        or: false,
    });
    assert_eq!(
        f.lib
            .preview_smart_list(both.clone(), SmartPreviewScope::default(), NOW)
            .unwrap()
            .track_ids,
        [f.ids[2].clone()]
    );
    create(&f.lib, "both", None, PlaylistKind::Smartlist, Some(both));
    f.lib.refresh_smart_lists(NOW).unwrap();
    let rows = f
        .lib
        .tracks(TrackQuery {
            playlist_id: Some("both".into()),
            ..Default::default()
        })
        .unwrap()
        .rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, f.ids[2]);
    assert!(rows[0].archived);
    assert_eq!(rows[0].state, "incoming");
}

fn enable_automatic(
    lib: &Library,
    automatic: serde_json::Value,
) -> anyhow::Result<PlaylistPreferencesSnapshot> {
    let old = lib.get_playlist_preferences()?;
    let mut json = serde_json::to_value(old.preferences)?;
    json["automatic"] = automatic;
    lib.save_playlist_preferences(serde_json::from_value(json)?, old.revision)
}
#[test]
fn automatic_lists_expand_all_dimensions_and_keep_manual_organization() {
    let f = fixture();
    create(&f.lib, "Artists", None, PlaylistKind::Folder, None);
    let undo = f.lib.playlist_workspace_at(NOW).unwrap().latest_undo;
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute(r#"UPDATE tracks SET native_metadata_json=json_set(native_metadata_json,'$.fields.artist',json('["Alpha, Beta feat. Gamma"]')),metadata_json=json_set(metadata_json,'$.tags.remixer','Delta; Alpha','$.tags.release_time','2026-09-12','$.tags.date','2025-01-01') WHERE id=?1"#, [&f.ids[0]]).unwrap();
    conn.execute(r#"UPDATE tracks SET metadata_json=json_set(metadata_json,'$.tags.date','2026-09'),native_metadata_json=json_set(native_metadata_json,'$.fields.artist',json('["Alphabet"]'),'$.fields.title',json('["Track (Echo Remix)"]')) WHERE id=?1"#, [&f.ids[1]]).unwrap();
    enable_automatic(&f.lib, serde_json::json!({"artists":true,"genres":true,"release_dates":true,"keys":true,"bpm":true,"bpm_ranges":[{"start":128.,"end":130.}]})).unwrap();
    f.lib.refresh_smart_lists(NOW).unwrap();
    let state = f.lib.playlist_state().unwrap();
    let list = |name: &str| {
        state
            .nodes
            .iter()
            .find(|n| n.name == name && n.kind == PlaylistKind::Smartlist)
            .unwrap_or_else(|| panic!("missing automatic list {name}"))
    };
    for name in ["Alpha", "Beta", "Gamma", "Delta"] {
        assert_eq!(
            f.lib.playlist_members(&list(name).id).unwrap(),
            [f.ids[0].clone()]
        );
    }
    assert_eq!(
        f.lib.playlist_members(&list("Echo").id).unwrap(),
        [f.ids[1].clone()]
    );
    assert_eq!(
        f.lib.playlist_members(&list("House").id).unwrap(),
        f.ids[..2]
    );
    assert_eq!(
        f.lib.playlist_members(&list("128–130 BPM").id).unwrap(),
        f.ids[..2]
    );
    for (name, ids) in [
        ("September, 2026", f.ids[..2].to_vec()),
        ("Warm-Up September, 2026", vec![]),
        ("Dancefloor September, 2026", vec![f.ids[0].clone()]),
        ("Anthems September, 2026", vec![f.ids[1].clone()]),
    ] {
        assert_eq!(f.lib.playlist_members(&list(name).id).unwrap(), ids);
    }
    assert_eq!(
        state
            .nodes
            .iter()
            .filter(|n| n.parent_id == list("September, 2026").parent_id
                && n.kind == PlaylistKind::Smartlist)
            .count(),
        4
    );
    assert_eq!(
        state.nodes.iter().filter(|n| n.name == "Artists").count(),
        2
    );
    assert_eq!(f.lib.playlist_members(&list("8A").id).unwrap(), f.ids[..2]);
    assert_eq!(f.lib.playlist_workspace_at(NOW).unwrap().latest_undo, undo);
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert_eq!(f.lib.playlist_state().unwrap(), state);
    conn.execute("UPDATE tracks SET metadata_json=json_set(metadata_json,'$.tags.release_time','2027-01-05') WHERE id=?1", [&f.ids[0]]).unwrap();
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert!(
        f.lib
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .any(|n| n.name == "January, 2027")
    );
    enable_automatic(&f.lib, serde_json::json!({})).unwrap();
    assert!(
        f.lib
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .any(|n| n.name == "January, 2027" && !n.deleted)
    );
}
#[test]
fn automatic_preferences_validate_ranges_and_default_for_existing_libraries() {
    let f = fixture();
    assert!(
        enable_automatic(
            &f.lib,
            serde_json::json!({"bpm_ranges":[{"start":130.,"end":120.}]})
        )
        .is_err()
    );
    assert!(
        enable_automatic(
            &f.lib,
            serde_json::json!({"bpm_ranges":[{"start":0.,"end":120.}]})
        )
        .is_err()
    );
    assert!(
        enable_automatic(
            &f.lib,
            serde_json::json!({"bpm_ranges":vec![serde_json::json!({"start":120.,"end":130.});11]})
        )
        .is_err()
    );
    let defaults: PlaylistPreferences =
        serde_json::from_value(serde_json::json!({"time_zone":null,"key_mixing_mode":"harmonic"}))
            .unwrap();
    assert_eq!(
        serde_json::to_value(defaults).unwrap()["automatic"]["artists"],
        false
    );
}

#[test]
fn automatic_service_expands_after_restart_and_never_creates_before_opt_in() {
    let f = fixture();
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert!(f.lib.playlist_state().unwrap().nodes.is_empty());
    enable_automatic(
        &f.lib,
        serde_json::json!({"release_dates":true,"artists":true}),
    )
    .unwrap();
    drop(f.lib);
    let library = Library::open(&f.dir.path().join("db")).unwrap();
    let service = SmartPlaylistService::start(library.clone()).unwrap();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute("UPDATE tracks SET metadata_json=json_set((SELECT metadata_json FROM tracks WHERE id=?2),'$.tags.release_time','2028-12-04') WHERE id=?1",params![f.ids[2],f.ids[0]]).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        !library
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .any(|n| n.name == "December, 2028")
    );
    conn.execute(
        "UPDATE tracks SET state='ready',artist='Future Artist' WHERE id=?1",
        [&f.ids[2]],
    )
    .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if library
            .playlist_state()
            .unwrap()
            .nodes
            .iter()
            .any(|n| n.name == "December, 2028")
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "service did not expand: {:?}",
            service.last_error()
        );
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    service.shutdown();
    assert!(service.last_error().unwrap().is_none());
    let state = library.playlist_state().unwrap();
    let month = state
        .nodes
        .iter()
        .find(|n| n.name == "December, 2028")
        .unwrap();
    library.refresh_smart_lists(NOW).unwrap();
    assert_eq!(
        library.playlist_members(&month.id).unwrap(),
        [f.ids[2].clone()]
    );
    let source = std::fs::read(f.dir.path().join("Incoming.flac")).unwrap();
    assert_eq!(
        source,
        b"synthetic input; no DSP is invoked in a playlist query"
    );
}
#[test]
fn automatic_keys_use_harmonic_aliases_wraparound_and_live_membership() {
    let f = fixture();
    enable_automatic(&f.lib, serde_json::json!({"keys":true})).unwrap();
    let state = f.lib.playlist_state().unwrap();
    assert_eq!(
        state
            .nodes
            .iter()
            .filter(|n| n.kind == PlaylistKind::Smartlist)
            .count(),
        24
    );
    let one = state.nodes.iter().find(|n| n.name == "1A").unwrap();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    for (key, expected) in [
        ("1A", true),
        ("12A", true),
        ("2A", true),
        ("1B", true),
        ("2B", false),
        ("12B", false),
        ("G#m", true),
        ("6m", true),
        ("unknown", false),
    ] {
        conn.execute("UPDATE tracks SET analysis_json=json_set(analysis_json,'$.analysis.key.camelot',?1) WHERE id=?2",params![key,f.ids[0]]).unwrap();
        f.lib.refresh_smart_lists(NOW).unwrap();
        assert_eq!(
            f.lib.playlist_members(&one.id).unwrap().contains(&f.ids[0]),
            expected,
            "{key}"
        );
    }
}

#[test]
fn automatic_artists_use_cleanup_remixer_values_instead_of_stale_source_tags() {
    let f = fixture();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute("UPDATE tracks SET metadata_json=json_set(metadata_json,'$.tags.remixer','Old Remixer'),native_metadata_json=json_set(native_metadata_json,'$.fields.remixer',json(?1)) WHERE id=?2",params![serde_json::json!(["Clean Remixer", "Second Remixer"]).to_string(),f.ids[0]]).unwrap();
    enable_automatic(&f.lib, serde_json::json!({"artists":true})).unwrap();
    f.lib.refresh_smart_lists(NOW).unwrap();
    let state = f.lib.playlist_state().unwrap();
    assert!(!state.nodes.iter().any(|n| n.name == "Old Remixer"));
    for name in ["Clean Remixer", "Second Remixer"] {
        let node = state.nodes.iter().find(|n| n.name == name).unwrap();
        assert_eq!(
            f.lib.playlist_members(&node.id).unwrap(),
            [f.ids[0].clone()]
        );
    }
}

#[test]
fn automatic_settings_and_generated_nodes_rollback_together_on_failure() {
    let f = fixture();
    let before = serde_json::to_value(f.lib.get_playlist_preferences().unwrap()).unwrap();
    let tree = f.lib.playlist_state().unwrap();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute_batch("CREATE TRIGGER reject_automatic BEFORE INSERT ON playlists WHEN NEW.kind='smartlist' BEGIN SELECT RAISE(ABORT,'fixture write failure'); END;").unwrap();
    assert!(enable_automatic(&f.lib, serde_json::json!({"keys":true})).is_err());
    assert_eq!(
        serde_json::to_value(f.lib.get_playlist_preferences().unwrap()).unwrap(),
        before
    );
    assert_eq!(f.lib.playlist_state().unwrap(), tree);
}
#[test]
fn automatic_months_use_all_energy_boundaries_and_keep_unknown_energy_in_all_tracks() {
    let f = fixture();
    let conn = Connection::open(f.dir.path().join("db")).unwrap();
    conn.execute("UPDATE tracks SET metadata_json=json_set(metadata_json,'$.tags.date','2026-09-12') WHERE id=?1",[&f.ids[0]]).unwrap();
    enable_automatic(&f.lib, serde_json::json!({"release_dates":true})).unwrap();
    let state = f.lib.playlist_state().unwrap();
    for energy in [
        None,
        Some(0),
        Some(1),
        Some(5),
        Some(6),
        Some(7),
        Some(8),
        Some(10),
    ] {
        if let Some(e) = energy {
            conn.execute("UPDATE tracks SET analysis_json=json_set(analysis_json,'$.analysis.energy',json(?1)) WHERE id=?2",params![serde_json::json!({"level":e,"method":"fixture"}).to_string(),f.ids[0]]).unwrap();
        } else {
            conn.execute("UPDATE tracks SET analysis_json=json_set(analysis_json,'$.analysis.energy',NULL) WHERE id=?1",[&f.ids[0]]).unwrap();
        }
        f.lib.refresh_smart_lists(NOW).unwrap();
        for (prefix, expected) in [
            ("", true),
            ("Warm-Up ", energy.is_some_and(|e| (1..=5).contains(&e))),
            ("Dancefloor ", energy.is_some_and(|e| (6..=7).contains(&e))),
            ("Anthems ", energy.is_some_and(|e| (8..=10).contains(&e))),
        ] {
            let name = format!("{prefix}September, 2026");
            let node = state.nodes.iter().find(|n| n.name == name).unwrap();
            assert_eq!(
                f.lib
                    .playlist_members(&node.id)
                    .unwrap()
                    .contains(&f.ids[0]),
                expected,
                "{name}: {energy:?}"
            );
        }
    }
}

#[test]
fn automatic_enabled_lists_repair_rules_while_retaining_names_and_folders() {
    let f = fixture();
    enable_automatic(&f.lib, serde_json::json!({"keys":true})).unwrap();
    let state = f.lib.playlist_state().unwrap();
    let id = state
        .nodes
        .iter()
        .find(|n| n.name == "8A")
        .unwrap()
        .id
        .clone();
    create(&f.lib, "custom-parent", None, PlaylistKind::Folder, None);
    let wrong = definition(
        "title",
        Operator::StringEquals,
        RuleValue::Text("never matches".into()),
    );
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::Batch {
                edits: vec![
                    PlaylistEdit::Rename {
                        id: id.clone(),
                        name: "My harmonic list".into(),
                    },
                    PlaylistEdit::Move {
                        id: id.clone(),
                        parent_id: Some("custom-parent".into()),
                        position: 0,
                    },
                    PlaylistEdit::SetDefinition {
                        id: id.clone(),
                        definition: wrong.clone(),
                    },
                ],
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.lib.refresh_smart_lists(NOW).unwrap();
    let state = f.lib.playlist_state().unwrap();
    let node = state.nodes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(node.name, "My harmonic list");
    assert_eq!(node.parent_id.as_deref(), Some("custom-parent"));
    assert_eq!(f.lib.playlist_members(&id).unwrap(), f.ids[..2]);
    enable_automatic(&f.lib, serde_json::json!({})).unwrap();
    f.lib
        .apply_playlist_edit(
            PlaylistEdit::SetDefinition {
                id: id.clone(),
                definition: wrong.clone(),
            },
            f.lib.playlist_state().unwrap().revision,
        )
        .unwrap();
    f.lib.refresh_smart_lists(NOW).unwrap();
    assert!(f.lib.playlist_members(&id).unwrap().is_empty());
    assert_eq!(
        f.lib
            .playlist_state()
            .unwrap()
            .nodes
            .into_iter()
            .find(|n| n.id == id)
            .unwrap()
            .definition,
        Some(wrong)
    );
}
