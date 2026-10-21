use cuerithm_library::*;
fn rules() -> PreparationPresetRules {
    let settings = PreparationSettings::default();
    PreparationPresetRules {
        cue_generation: settings.engine.lexicon_config.unwrap(),
        autotag_enabled: false,
        tagging: None,
        cleanup_enabled: false,
        cleanup: None,
        organization: PreparationPresetOrganization {
            mode: OrganizationSettingMode::LeaveInPlace,
            pattern: "{artist} - {title}".into(),
            missing: OrganizationMissingPolicy::Block,
            collision: OrganizationCollisionPolicy::Block,
            organization_optional: false,
        },
    }
}
#[cfg(feature = "autotag")]
#[test]
fn every_accepted_provider_is_portable_without_credentials_or_transmission_consent() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("library.sqlite")).unwrap();
    let mut draft = PreparationSettings::default();
    let mut tagging = cuerithm_autotag::AutotagConfig::default();
    tagging.options.platforms = cuerithm_autotag::ProviderId::ALL
        .iter()
        .map(|provider| provider.as_str().into())
        .collect();
    tagging.recognition.enabled = true;
    tagging.recognition.fingerprint_consent = true;
    tagging.solver.endpoint = Some("http://private-solver.invalid:8191".into());
    draft.tagging = Some(tagging.clone());
    let projection = lib.project_preparation_preset(&draft).unwrap();
    assert_eq!(
        projection
            .rules
            .tagging
            .as_ref()
            .unwrap()
            .options
            .platforms
            .len(),
        10
    );
    let preset = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "All accepted metadata providers".into(),
            rules: projection.rules,
            connections: projection.connections,
        })
        .unwrap();
    let exported = lib
        .export_preparation_preset(&preset.id, &preset.revision)
        .unwrap();
    assert!(!exported.document.contains("recognition"));
    assert!(!exported.document.contains("fingerprint_consent"));
    assert!(!exported.document.contains("private-solver"));
    assert!(!exported.document.contains("spotify"));
    assert!(!exported.document.contains("beatsource"));
    assert!(!exported.document.contains("junodownload"));
    let overlay = lib
        .overlay_preparation_preset(&draft, &preset.id, &preset.revision)
        .unwrap();
    assert!(
        overlay
            .tagging
            .as_ref()
            .unwrap()
            .recognition
            .fingerprint_consent
    );
    assert_eq!(
        overlay.tagging.as_ref().unwrap().solver.endpoint,
        tagging.solver.endpoint
    );
    let mut unsafe_import: serde_json::Value = serde_json::from_str(&exported.document).unwrap();
    unsafe_import["rules"]["tagging"]["recognition"] =
        serde_json::json!({"enabled":true,"fingerprint_consent":true});
    assert!(
        lib.import_preparation_preset(&unsafe_import.to_string(), None)
            .is_err()
    );
    unsafe_import = serde_json::from_str(&exported.document).unwrap();
    unsafe_import["rules"]["tagging"]["options"]["platforms"] = serde_json::json!(["junodownload"]);
    assert!(
        lib.import_preparation_preset(&unsafe_import.to_string(), None)
            .is_err()
    );
    unsafe_import = serde_json::from_str(&exported.document).unwrap();
    unsafe_import["rules"]["tagging"]["options"]["platforms"] = serde_json::json!(["beatsource"]);
    assert!(
        lib.import_preparation_preset(&unsafe_import.to_string(), None)
            .is_err()
    );
    unsafe_import = serde_json::from_str(&exported.document).unwrap();
    unsafe_import["rules"]["tagging"]["options"]["custom"]["volumo"] =
        serde_json::json!({"api_key":"fixture-volumo-private"});
    assert!(
        lib.import_preparation_preset(&unsafe_import.to_string(), None)
            .is_err()
    );
    unsafe_import = serde_json::from_str(&exported.document).unwrap();
    unsafe_import["rules"]["tagging"]["options"]["custom"]["sonovault"] =
        serde_json::json!({"api_key":"fixture-private"});
    assert!(
        lib.import_preparation_preset(&unsafe_import.to_string(), None)
            .is_err()
    );
}
#[test]
fn preset_revisions_are_durable_cas_guarded_and_do_not_select_global_settings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let db = root.join("data/library.sqlite");
    let lib = Library::open(&db).unwrap();
    let selected = lib.settings().unwrap().revision;
    let first = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Club".into(),
            rules: rules(),
            connections: Default::default(),
        })
        .unwrap();
    assert_eq!(lib.settings().unwrap().revision, selected);
    let mut changed = first.rules.clone();
    changed.cue_generation.min_cue_length_beats = 32;
    let next = lib
        .save_preparation_preset(PreparationPresetSave {
            id: Some(first.id.clone()),
            expected_revision: Some(first.revision.clone()),
            name: "Club renamed".into(),
            rules: changed,
            connections: first.connections.clone(),
        })
        .unwrap();
    assert_ne!(first.revision, next.revision);
    assert!(
        lib.save_preparation_preset(PreparationPresetSave {
            id: Some(first.id.clone()),
            expected_revision: Some(first.revision),
            name: "Stale".into(),
            rules: rules(),
            connections: Default::default()
        })
        .is_err()
    );
    drop(lib);
    let lib = Library::open(&db).unwrap();
    let saved = lib.preparation_preset(&next.id).unwrap();
    assert_eq!(saved.revision, next.revision);
    assert_eq!(saved.name, "Club renamed");
    assert_eq!(lib.settings().unwrap().revision, selected);
    let copy = lib
        .duplicate_preparation_preset(&saved.id, &saved.revision, "Club copy")
        .unwrap();
    assert_ne!(copy.id, saved.id);
    assert!(
        lib.delete_preparation_preset(&copy.id, "old-revision")
            .is_err()
    );
    lib.delete_preparation_preset(&copy.id, &copy.revision)
        .unwrap();
    assert!(lib.preparation_preset(&copy.id).is_err());
    assert_eq!(lib.preparation_presets().unwrap().len(), 1);
}
#[cfg(feature = "autotag")]
#[test]
fn portable_export_uses_an_allowlist_and_overlay_preserves_unsaved_globals() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let mut draft = PreparationSettings {
        stability_millis: 321,
        watch_folders: vec![root.join("SECRET_WATCH")],
        ..Default::default()
    };
    draft.engine.key_model = Some(root.join("SECRET_MODEL"));
    let mut config = cuerithm_autotag::AutotagConfig::default();
    config.solver.endpoint = Some("http://SECRET_SOLVER:8191".into());
    config.options.path = Some(root.join("SECRET_PROVIDER_PATH"));
    config.options.post_command = Some("SECRET_COMMAND".into());
    config.options.threads = 7;
    config.options.include_subfolders = false;
    let mut raw = serde_json::to_value(&config).unwrap();
    raw["options"]["spotify"] =
        serde_json::json!({"clientId":"SECRET_CLIENT","clientSecret":"SECRET_TOKEN"});
    raw["options"]["custom"]["unaccepted-provider"] = serde_json::json!({"key":"SECRET_CUSTOM"});
    config = serde_json::from_value(raw).unwrap();
    draft.tagging = Some(config);
    let projection = lib.project_preparation_preset(&draft).unwrap();
    let saved = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Portable".into(),
            rules: projection.rules,
            connections: projection.connections,
        })
        .unwrap();
    let exported = lib
        .export_preparation_preset(&saved.id, &saved.revision)
        .unwrap();
    assert!(!exported.document.contains("SECRET"));
    assert!(!exported.document.contains("solver"));
    assert!(!exported.document.contains("destination_root_id"));
    assert!(!exported.document.contains("threads"));
    let mut unsaved = draft.clone();
    unsaved.stability_millis = 999;
    unsaved.tagging.as_mut().unwrap().solver.endpoint = Some("http://UNSAVED_SOLVER:8191".into());
    unsaved
        .engine
        .lexicon_config
        .as_mut()
        .unwrap()
        .min_cue_length_beats = 123;
    let overlaid = lib
        .overlay_preparation_preset(&unsaved, &saved.id, &saved.revision)
        .unwrap();
    assert_eq!(overlaid.stability_millis, 999);
    assert_eq!(overlaid.watch_folders, unsaved.watch_folders);
    assert_eq!(overlaid.engine.key_model, unsaved.engine.key_model);
    assert_eq!(
        overlaid.tagging.as_ref().unwrap().solver.endpoint,
        unsaved.tagging.as_ref().unwrap().solver.endpoint
    );
    assert_eq!(overlaid.tagging.as_ref().unwrap().options.threads, 7);
    assert!(
        !overlaid
            .tagging
            .as_ref()
            .unwrap()
            .options
            .include_subfolders
    );
    assert_ne!(
        overlaid.engine.lexicon_config.unwrap().min_cue_length_beats,
        123
    );
    let mut unknown: serde_json::Value = serde_json::from_str(&exported.document).unwrap();
    unknown["rules"]["cue_generation"]["unknownNativePath"] = serde_json::json!("must reject");
    assert!(
        lib.preview_preparation_preset_import(&unknown.to_string())
            .is_err()
    );
    unknown = serde_json::from_str(&exported.document).unwrap();
    unknown["rules"]["tagging"]["solver"] = serde_json::json!({"endpoint":"http://untrusted"});
    assert!(
        lib.import_preparation_preset(&unknown.to_string(), Some("Invalid"))
            .is_err()
    );
}
#[test]
fn imported_copy_recipe_requires_explicit_local_reconnection() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let output = root.join("output");
    std::fs::create_dir(&output).unwrap();
    let grant = lib.register_organization_root(output).unwrap();
    let mut configured = rules();
    configured.organization.mode = OrganizationSettingMode::Copy;
    let saved = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Copy collection".into(),
            rules: configured,
            connections: PreparationPresetConnections {
                destination_root_id: Some(grant.id.clone()),
            },
        })
        .unwrap();
    let exported = lib
        .export_preparation_preset(&saved.id, &saved.revision)
        .unwrap();
    assert!(!exported.document.contains(&grant.id));
    let preview = lib
        .preview_preparation_preset_import(&exported.document)
        .unwrap();
    assert_eq!(preview.status, "needs_connection");
    let imported = lib
        .import_preparation_preset(&exported.document, Some("Imported collection"))
        .unwrap();
    assert_eq!(imported.status, "needs_connection");
    assert!(imported.connections.destination_root_id.is_none());
    assert!(
        lib.overlay_preparation_preset(
            &PreparationSettings::default(),
            &imported.id,
            &imported.revision
        )
        .is_err()
    );
    let connected = lib
        .save_preparation_preset(PreparationPresetSave {
            id: Some(imported.id),
            expected_revision: Some(imported.revision),
            name: imported.name,
            rules: imported.rules,
            connections: PreparationPresetConnections {
                destination_root_id: Some(grant.id),
            },
        })
        .unwrap();
    assert_eq!(connected.status, "ready");
}
fn save_named(lib: &Library, name: &str, beats: u32) -> PreparationPreset {
    let mut rules = rules();
    rules.cue_generation.min_cue_length_beats = beats;
    lib.save_preparation_preset(PreparationPresetSave {
        id: None,
        expected_revision: None,
        name: name.into(),
        rules,
        connections: Default::default(),
    })
    .unwrap()
}
#[test]
fn deepest_folder_rules_and_global_barriers_capture_each_source_immutably() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let parent = root.join("incoming");
    let child = parent.join("nested");
    std::fs::create_dir_all(&child).unwrap();
    let outer = parent.join("outer.flac");
    let inner = child.join("inner.flac");
    std::fs::write(&outer, b"outer source").unwrap();
    std::fs::write(&inner, b"inner source").unwrap();
    let a = save_named(&lib, "Outer", 32);
    let b = save_named(&lib, "Inner", 16);
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![child.clone(), parent.clone()];
    settings.engine.lexicon_energy = true;
    let base = lib.save_settings(settings.clone()).unwrap();
    let folders = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap();
    let outer_id = folders
        .iter()
        .find(|folder| folder.display_path == parent.to_string_lossy())
        .unwrap()
        .folder_id
        .clone();
    let inner_id = folders
        .iter()
        .find(|folder| folder.display_path == child.to_string_lossy())
        .unwrap()
        .folder_id
        .clone();
    let changes = vec![
        PreparationFolderChange {
            folder_id: outer_id.clone(),
            expected_revision: None,
            choice: PreparationFolderChoice::Preset {
                preset_id: a.id.clone(),
            },
        },
        PreparationFolderChange {
            folder_id: inner_id.clone(),
            expected_revision: None,
            choice: PreparationFolderChoice::Preset {
                preset_id: b.id.clone(),
            },
        },
    ];
    lib.save_settings_with_preparation_assignments(settings.clone(), &base, &changes)
        .unwrap();
    let selected = lib.settings().unwrap().revision;
    let ids = lib
        .enqueue_with_current_preparation(&[inner.clone(), outer.clone()], false)
        .unwrap();
    assert_eq!(ids.len(), 2);
    let mut claimed = vec![
        lib.claim_next().unwrap().unwrap(),
        lib.claim_next().unwrap().unwrap(),
    ];
    claimed.sort_by(|a, b| a.path.cmp(&b.path));
    for job in &claimed {
        let capture = job.snapshot.preset_capture.as_ref().unwrap();
        let expected = if job.path == inner { &b } else { &a };
        assert_eq!(capture.preset_id.as_deref(), Some(expected.id.as_str()));
        assert_eq!(
            capture.preset_revision.as_deref(),
            Some(expected.revision.as_str())
        );
        assert_eq!(
            job.snapshot
                .settings
                .engine
                .lexicon_config
                .as_ref()
                .unwrap()
                .min_cue_length_beats,
            expected.rules.cue_generation.min_cue_length_beats
        );
    }
    assert_eq!(
        lib.settings().unwrap().revision,
        selected,
        "materialization never selects a preset globally"
    );
    let folder = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .into_iter()
        .find(|folder| folder.folder_id == inner_id)
        .unwrap();
    let child_revision = folder.revision.unwrap();
    let barrier = PreparationFolderChange {
        folder_id: inner_id.clone(),
        expected_revision: Some(child_revision),
        choice: PreparationFolderChoice::Global,
    };
    lib.save_settings_with_preparation_assignments(settings.clone(), &selected, &[barrier])
        .unwrap();
    let snapshot = lib.current_preparation_snapshot_for_path(&inner).unwrap();
    assert!(
        snapshot
            .preset_capture
            .as_ref()
            .unwrap()
            .preset_id
            .is_none()
    );
    assert_eq!(
        snapshot
            .settings
            .engine
            .lexicon_config
            .as_ref()
            .unwrap()
            .min_cue_length_beats,
        settings
            .engine
            .lexicon_config
            .as_ref()
            .unwrap()
            .min_cue_length_beats
    );
    let folders = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap();
    let revision = folders
        .iter()
        .find(|folder| folder.folder_id == inner_id)
        .unwrap()
        .revision
        .clone();
    lib.save_settings_with_preparation_assignments(
        settings.clone(),
        &lib.settings().unwrap().revision,
        &[PreparationFolderChange {
            folder_id: inner_id,
            expected_revision: revision,
            choice: PreparationFolderChoice::Inherit,
        }],
    )
    .unwrap();
    assert_eq!(
        lib.current_preparation_snapshot_for_path(&inner)
            .unwrap()
            .preset_capture
            .unwrap()
            .preset_id,
        Some(a.id.clone())
    );
    let before = lib.settings().unwrap().revision;
    settings.stability_millis = 555;
    assert!(
        lib.save_settings_with_preparation_assignments(
            settings,
            &before,
            &[PreparationFolderChange {
                folder_id: outer_id,
                expected_revision: Some("stale".into()),
                choice: PreparationFolderChoice::Global
            }]
        )
        .is_err()
    );
    assert_eq!(lib.settings().unwrap().revision, before);
    let mut changed = a.rules.clone();
    changed.cue_generation.min_cue_length_beats = 8;
    lib.save_preparation_preset(PreparationPresetSave {
        id: Some(a.id.clone()),
        expected_revision: Some(a.revision.clone()),
        name: a.name.clone(),
        rules: changed,
        connections: a.connections.clone(),
    })
    .unwrap();
    for job in &claimed {
        let stored = lib.settings_revision(&job.settings_revision).unwrap();
        assert_ne!(
            stored
                .settings
                .engine
                .lexicon_config
                .unwrap()
                .min_cue_length_beats,
            8
        );
        assert_eq!(
            stored.preset_capture.as_ref().unwrap().preset_revision,
            job.snapshot
                .preset_capture
                .as_ref()
                .unwrap()
                .preset_revision
        );
    }
    lib.delete_preparation_preset(&b.id, &b.revision).unwrap();
    let captured_inner = claimed.iter().find(|job| job.path == inner).unwrap();
    assert_eq!(
        lib.settings_revision(&captured_inner.settings_revision)
            .unwrap()
            .preset_capture
            .unwrap()
            .preset_id,
        Some(b.id)
    );
    assert!(
        lib.delete_preparation_preset(&a.id, &lib.preparation_preset(&a.id).unwrap().revision)
            .is_err()
    );
}
fn wait_until(mut condition: impl FnMut() -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !condition() {
        assert!(std::time::Instant::now() < end, "condition did not settle");
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}
// Full organization resolves effective native metadata through the tagging-enabled reader.
#[cfg(feature = "autotag")]
#[test]
fn failed_discovery_admission_retries_unchanged_source_after_preset_reconnection() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let incoming = root.join("incoming");
    let output = root.join("managed");
    std::fs::create_dir(&incoming).unwrap();
    std::fs::create_dir(&output).unwrap();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = incoming.join("unchanged.flac");
    std::fs::copy(
        repo.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &source,
    )
    .unwrap();
    let before = cuerithm_media::file_sha256(&source).unwrap();
    let modified = std::fs::metadata(&source).unwrap().modified().unwrap();
    let grant = lib.register_organization_root(output.clone()).unwrap();
    let mut configured = rules();
    configured.organization.mode = OrganizationSettingMode::Copy;
    configured.organization.pattern = "{original_name}".into();
    let preset = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Disconnected intake".into(),
            rules: configured,
            connections: PreparationPresetConnections {
                destination_root_id: Some(grant.id),
            },
        })
        .unwrap();
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![incoming];
    settings.engine.lexicon_energy = true;
    settings.stability_millis = 10;
    let base = lib.save_settings(settings.clone()).unwrap();
    let folder = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    lib.save_settings_with_preparation_assignments(
        settings,
        &base,
        &[PreparationFolderChange {
            folder_id: folder.folder_id,
            expected_revision: None,
            choice: PreparationFolderChoice::Preset {
                preset_id: preset.id.clone(),
            },
        }],
    )
    .unwrap();
    std::fs::rename(&output, root.join("retained-managed")).unwrap();
    let runtime =
        cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin")).unwrap();
    let worker = PreparationService::start_with_workers(lib.clone(), runtime, 1).unwrap();
    wait_until(|| worker.status().last_error.is_some() || !lib.jobs().unwrap().is_empty());
    assert!(
        lib.jobs().unwrap().is_empty(),
        "a disconnected assigned preset must block admission instead of silently using global settings"
    );
    assert!(worker.status().last_error.is_some());
    std::fs::create_dir(&output).unwrap();
    let grant = lib.register_organization_root(output.clone()).unwrap();
    lib.save_preparation_preset(PreparationPresetSave {
        id: Some(preset.id),
        expected_revision: Some(preset.revision),
        name: preset.name,
        rules: preset.rules,
        connections: PreparationPresetConnections {
            destination_root_id: Some(grant.id),
        },
    })
    .unwrap();
    worker.wake();
    wait_until(|| {
        lib.counts().unwrap().tracks == 1
            || lib
                .jobs()
                .unwrap()
                .iter()
                .any(|job| job.state == "attention")
    });
    worker.shutdown();
    assert_eq!(
        lib.counts().unwrap().tracks,
        1,
        "Reconnected discovery failed: {:?}",
        lib.jobs().unwrap()
    );
    drop(worker);
    assert_eq!(lib.jobs().unwrap().len(), 1);
    assert_eq!(lib.jobs().unwrap()[0].attempts, 1);
    assert_eq!(cuerithm_media::file_sha256(&source).unwrap(), before);
    assert_eq!(
        std::fs::metadata(&source).unwrap().modified().unwrap(),
        modified
    );
    assert!(output.join("unchanged.flac").exists());
}
#[test]
fn a_captured_preset_never_follows_a_replaced_source_folder() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let incoming = root.join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let source = incoming.join("track.flac");
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::copy(
        repo.join("crates/camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
        &source,
    )
    .unwrap();
    let before = cuerithm_media::file_sha256(&source).unwrap();
    let preset = save_named(&lib, "Folder proof", 32);
    let mut settings = lib.settings().unwrap().settings;
    settings.engine.lexicon_energy = true;
    settings.watch_folders = vec![incoming.clone()];
    let base = lib.save_settings(settings.clone()).unwrap();
    let folder = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    lib.save_settings_with_preparation_assignments(
        settings,
        &base,
        &[PreparationFolderChange {
            folder_id: folder.folder_id,
            expected_revision: None,
            choice: PreparationFolderChoice::Preset {
                preset_id: preset.id,
            },
        }],
    )
    .unwrap();
    let id = lib
        .enqueue_with_current_preparation(std::slice::from_ref(&source), false)
        .unwrap()
        .remove(0);
    let retained = root.join("retained-input");
    std::fs::rename(&incoming, &retained).unwrap();
    std::fs::create_dir(&incoming).unwrap();
    std::fs::copy(retained.join("track.flac"), &source).unwrap();
    assert!(
        lib.current_preparation_snapshot_for_path(&source).is_err(),
        "same bytes under a new native folder must not silently inherit the old assignment"
    );
    let runtime =
        cuerithm_media::MediaRuntime::from_bundle(&repo.join("tools/media-runtime/bin")).unwrap();
    let worker = PreparationService::start_with_workers(lib.clone(), runtime, 1).unwrap();
    wait_until(|| {
        lib.jobs()
            .unwrap()
            .iter()
            .any(|job| job.id == id && job.state == "attention")
    });
    worker.shutdown();
    drop(worker);
    let job = lib
        .jobs()
        .unwrap()
        .into_iter()
        .find(|job| job.id == id)
        .unwrap();
    assert_eq!(job.attempts, 1);
    assert_eq!(lib.cue_revision_count(&job.track_id).unwrap(), 0);
    assert!(lib.analysis(&job.track_id).unwrap().is_none());
    assert_eq!(cuerithm_media::file_sha256(&source).unwrap(), before);
    assert_eq!(
        cuerithm_media::file_sha256(&retained.join("track.flac")).unwrap(),
        before
    );
}
#[cfg(feature = "autotag")]
#[test]
fn new_preset_captures_use_current_global_solver_without_rewriting_claimed_jobs_or_access_state() {
    use cuerithm_autotag::ChallengeStore;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let incoming = root.join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let source = incoming.join("a.flac");
    let second = incoming.join("b.flac");
    std::fs::write(&source, b"first").unwrap();
    std::fs::write(&second, b"second").unwrap();
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![incoming];
    settings.engine.lexicon_energy = true;
    let mut config = cuerithm_autotag::AutotagConfig::default();
    config.solver.endpoint = Some("http://127.0.0.1:8191".into());
    settings.tagging = Some(config);
    let base = lib.save_settings(settings.clone()).unwrap();
    let projection = lib.project_preparation_preset(&settings).unwrap();
    let preset = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Transport independent".into(),
            rules: projection.rules,
            connections: projection.connections,
        })
        .unwrap();
    let folder = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    lib.save_settings_with_preparation_assignments(
        settings.clone(),
        &base,
        &[PreparationFolderChange {
            folder_id: folder.folder_id,
            expected_revision: None,
            choice: PreparationFolderChoice::Preset {
                preset_id: preset.id.clone(),
            },
        }],
    )
    .unwrap();
    lib.mark_requires_solver("traxsource").unwrap();
    lib.enqueue_with_current_preparation(&[source], false)
        .unwrap();
    let claimed = lib.claim_next().unwrap().unwrap();
    settings.tagging.as_mut().unwrap().solver.endpoint = Some("http://127.0.0.1:8192".into());
    lib.save_settings(settings).unwrap();
    lib.enqueue_with_current_preparation(&[second], false)
        .unwrap();
    let later = lib.claim_next().unwrap().unwrap();
    assert_eq!(
        claimed
            .snapshot
            .settings
            .tagging
            .as_ref()
            .unwrap()
            .solver
            .endpoint
            .as_deref(),
        Some("http://127.0.0.1:8191")
    );
    assert_eq!(
        later
            .snapshot
            .settings
            .tagging
            .as_ref()
            .unwrap()
            .solver
            .endpoint
            .as_deref(),
        Some("http://127.0.0.1:8192")
    );
    assert_eq!(
        claimed
            .snapshot
            .preset_capture
            .as_ref()
            .unwrap()
            .preset_revision,
        later
            .snapshot
            .preset_capture
            .as_ref()
            .unwrap()
            .preset_revision
    );
    assert_ne!(claimed.settings_revision, later.settings_revision);
    let document = lib
        .export_preparation_preset(&preset.id, &preset.revision)
        .unwrap()
        .document;
    lib.import_preparation_preset(&document, Some("Unassigned import"))
        .unwrap();
    assert!(lib.requires_solver("traxsource").unwrap());
    assert!(!document.contains("solver"));
}
#[cfg(not(feature = "autotag"))]
#[test]
fn no_autotag_build_preserves_unsupported_rules_without_enabling_them() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("data/library.sqlite")).unwrap();
    let global = lib.settings().unwrap().revision;
    let mut configured = rules();
    configured.cleanup_enabled = true;
    configured.cleanup = Some(Default::default());
    let preset = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Preserved unsupported rules".into(),
            rules: configured,
            connections: Default::default(),
        })
        .unwrap();
    assert_eq!(preset.status, "unsupported");
    let document = lib
        .export_preparation_preset(&preset.id, &preset.revision)
        .unwrap()
        .document;
    let imported = lib
        .import_preparation_preset(&document, Some("Imported unsupported"))
        .unwrap();
    assert!(imported.rules.cleanup_enabled);
    assert_eq!(imported.status, "unsupported");
    assert_eq!(lib.settings().unwrap().revision, global);
}
#[test]
fn explicit_inherit_reconnect_refreshes_only_the_folder_grant() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let db = root.join("data/library.sqlite");
    let lib = Library::open(&db).unwrap();
    let incoming = root.join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![incoming.clone()];
    let base = lib.save_settings(settings.clone()).unwrap();
    let folder = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    assert!(folder.revision.is_none());
    let sql = rusqlite::Connection::open(&db).unwrap();
    let original: String = sql
        .query_row(
            "SELECT authority_json FROM preparation_preset_folders WHERE id=?1",
            [&folder.folder_id],
            |row| row.get(0),
        )
        .unwrap();
    std::fs::rename(&incoming, root.join("retained-input")).unwrap();
    std::fs::create_dir(&incoming).unwrap();
    let changed = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    assert_eq!(changed.status, "needs_connection");
    assert!(changed.revision.is_none());
    let before: String = sql
        .query_row(
            "SELECT authority_json FROM preparation_preset_folders WHERE id=?1",
            [&folder.folder_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        before, original,
        "reads must never regrant a replaced folder"
    );
    lib.save_settings_with_preparation_assignments(
        settings.clone(),
        &base,
        &[PreparationFolderChange {
            folder_id: folder.folder_id.clone(),
            expected_revision: None,
            choice: PreparationFolderChoice::Inherit,
        }],
    )
    .unwrap();
    let refreshed = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    assert_eq!(refreshed.status, "ready");
    assert!(matches!(refreshed.choice, PreparationFolderChoice::Inherit));
    assert!(refreshed.revision.is_none());
    let current: String = sql
        .query_row(
            "SELECT authority_json FROM preparation_preset_folders WHERE id=?1",
            [&folder.folder_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_ne!(current, original);
    assert_eq!(
        sql.query_row(
            "SELECT COUNT(*) FROM preparation_preset_assignments",
            [],
            |row| row.get::<_, u64>(0)
        )
        .unwrap(),
        0
    );
    assert!(lib.preparation_presets().unwrap().is_empty());
}
#[test]
fn same_choice_global_and_named_reconnect_preserve_older_capture_authorities() {
    for named in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let db = root.join("data/library.sqlite");
        let lib = Library::open(&db).unwrap();
        let incoming = root.join("incoming");
        std::fs::create_dir(&incoming).unwrap();
        let source = incoming.join("source.flac");
        std::fs::write(&source, b"unchanged source bytes").unwrap();
        let mut settings = lib.settings().unwrap().settings;
        settings.watch_folders = vec![incoming.clone()];
        let base = lib.save_settings(settings.clone()).unwrap();
        let choice = if named {
            PreparationFolderChoice::Preset {
                preset_id: save_named(&lib, "Same choice", 32).id,
            }
        } else {
            PreparationFolderChoice::Global
        };
        let folder = lib
            .preparation_preset_folders_for_settings(&settings)
            .unwrap()
            .remove(0);
        lib.save_settings_with_preparation_assignments(
            settings.clone(),
            &base,
            &[PreparationFolderChange {
                folder_id: folder.folder_id.clone(),
                expected_revision: None,
                choice: choice.clone(),
            }],
        )
        .unwrap();
        let old = lib.current_preparation_snapshot_for_path(&source).unwrap();
        let sql = rusqlite::Connection::open(&db).unwrap();
        let old_proof:String=sql.query_row("SELECT authority_json FROM preparation_preset_capture_authorities WHERE settings_revision=?1",[&old.revision],|row|row.get(0)).unwrap();
        std::fs::rename(&incoming, root.join("retained-source")).unwrap();
        std::fs::create_dir(&incoming).unwrap();
        std::fs::write(&source, b"unchanged source bytes").unwrap();
        let changed = lib
            .preparation_preset_folders_for_settings(&settings)
            .unwrap()
            .remove(0);
        assert_eq!(changed.status, "needs_connection");
        assert!(lib.current_preparation_snapshot_for_path(&source).is_err());
        lib.save_settings_with_preparation_assignments(
            settings.clone(),
            &lib.settings().unwrap().revision,
            &[PreparationFolderChange {
                folder_id: changed.folder_id,
                expected_revision: changed.revision,
                choice,
            }],
        )
        .unwrap();
        let current = lib.current_preparation_snapshot_for_path(&source).unwrap();
        assert_ne!(old.revision, current.revision);
        assert_ne!(
            old.preset_capture.unwrap().assignment_revision,
            current.preset_capture.unwrap().assignment_revision
        );
        let preserved:String=sql.query_row("SELECT authority_json FROM preparation_preset_capture_authorities WHERE settings_revision=?1",[&old.revision],|row|row.get(0)).unwrap();
        assert_eq!(
            preserved, old_proof,
            "a reconnect may authorize future captures, never rewrite an older source grant"
        );
        assert_eq!(
            lib.preparation_preset_folders_for_settings(&settings)
                .unwrap()[0]
                .status,
            "ready"
        );
    }
}
#[test]
fn existing_large_watch_sets_keep_global_intake_without_profile_assignment() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let source = root.join("source.flac");
    std::fs::write(&source, b"existing intake").unwrap();
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = (0..256)
        .map(|index| root.join(format!("offline-watch-{index}")))
        .collect();
    settings.watch_folders.push(root.clone());
    let selected = lib.save_settings(settings.clone()).unwrap();
    let captured = lib.current_preparation_snapshot_for_path(&source).unwrap();
    assert_eq!(captured.revision, selected);
    assert!(captured.preset_capture.is_none());
    assert_eq!(
        lib.preparation_preset_folders_for_settings(&settings)
            .unwrap()
            .len(),
        257
    );
}
