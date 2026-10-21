use crate::*;
#[test]
fn global_and_folder_rule_changes_cannot_produce_a_mixed_capture() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let lib = Library::open(&root.join("data/library.sqlite")).unwrap();
    let incoming = root.join("incoming");
    std::fs::create_dir(&incoming).unwrap();
    let source = incoming.join("source.flac");
    std::fs::write(&source, b"source snapshot").unwrap();
    let mut settings = lib.settings().unwrap().settings;
    settings.watch_folders = vec![incoming];
    let initial = lib.save_settings(settings.clone()).unwrap();
    let first = lib.project_preparation_preset(&settings).unwrap();
    let mut second = first.rules.clone();
    second.cue_generation.min_cue_length_beats = 16;
    let a = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "First".into(),
            rules: first.rules,
            connections: Default::default(),
        })
        .unwrap();
    let b = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Second".into(),
            rules: second,
            connections: Default::default(),
        })
        .unwrap();
    let folder = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    lib.save_settings_with_preparation_assignments(
        settings.clone(),
        &initial,
        &[PreparationFolderChange {
            folder_id: folder.folder_id.clone(),
            expected_revision: None,
            choice: PreparationFolderChoice::Preset { preset_id: a.id },
        }],
    )
    .unwrap();
    let folder = lib
        .preparation_preset_folders_for_settings(&settings)
        .unwrap()
        .remove(0);
    let old = lib.settings().unwrap().revision;
    let mut changed = false;
    let mut current = String::new();
    let captured = lib
        .current_preparation_snapshot_for_path_with(&source, || {
            if !changed {
                changed = true;
                settings.stability_millis = 555;
                current = lib.save_settings_with_preparation_assignments(
                    settings.clone(),
                    &old,
                    &[PreparationFolderChange {
                        folder_id: folder.folder_id.clone(),
                        expected_revision: folder.revision.clone(),
                        choice: PreparationFolderChoice::Preset {
                            preset_id: b.id.clone(),
                        },
                    }],
                )?;
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(
        captured.settings.stability_millis, 555,
        "capture must retry a concurrent atomic settings+assignment change, not combine old globals with a new preset"
    );
    let provenance = captured.preset_capture.unwrap();
    assert_eq!(provenance.base_settings_revision, current);
    assert_eq!(provenance.preset_id, Some(b.id));
}
#[test]
fn preset_save_returns_its_committed_revision_not_a_later_concurrent_edit() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("data/library.sqlite")).unwrap();
    let projection = lib
        .project_preparation_preset(&lib.settings().unwrap().settings)
        .unwrap();
    let saved = lib
        .save_preparation_preset(PreparationPresetSave {
            id: None,
            expected_revision: None,
            name: "Original".into(),
            rules: projection.rules,
            connections: projection.connections,
        })
        .unwrap();
    let result = lib
        .save_preparation_preset_with(
            PreparationPresetSave {
                id: Some(saved.id.clone()),
                expected_revision: Some(saved.revision),
                name: "My committed edit".into(),
                rules: saved.rules,
                connections: saved.connections,
            },
            || {
                let current = lib.preparation_preset(&saved.id)?;
                lib.save_preparation_preset(PreparationPresetSave {
                    id: Some(current.id),
                    expected_revision: Some(current.revision),
                    name: "Later concurrent edit".into(),
                    rules: current.rules,
                    connections: current.connections,
                })?;
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(
        result.name, "My committed edit",
        "a save acknowledgment must not bless a newer revision that this request never submitted"
    );
    let current = lib.preparation_preset(&saved.id).unwrap();
    assert_eq!(current.name, "Later concurrent edit");
    assert_ne!(result.revision, current.revision);
}
