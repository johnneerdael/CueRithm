use cuerithm_dj_sync::{DjSoftware, SyncTarget};
use cuerithm_library::{
    Library, SaveSyncTargetConfiguration, SavedSyncTarget, SyncConfigurationBinding,
};

#[test]
fn saved_sync_targets_roundtrip_and_refuse_stale_edits_without_auto_sync() {
    let folder = tempfile::tempdir().unwrap();
    let library = Library::open(&folder.path().join("library")).unwrap();
    let home = folder.path().join("VirtualDJ");
    std::fs::create_dir(&home).unwrap();
    std::fs::write(home.join("database.xml"), "<VirtualDJ_Database/>").unwrap();
    let target = SavedSyncTarget::DjLibrary {
        target: SyncTarget {
            software: DjSoftware::VirtualDj,
            path: home,
            source_root: None,
            environment: Default::default(),
        },
    };
    let saved = library
        .save_sync_target_configuration(SaveSyncTargetConfiguration {
            id: None,
            expected_revision: None,
            name: "Home decks".into(),
            target: target.clone(),
        })
        .unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(library.list_sync_target_configurations().unwrap().len(), 1);
    let binding = SyncConfigurationBinding {
        id: saved.id.clone(),
        revision: saved.revision,
    };
    library
        .verify_sync_target_configuration(&binding, &saved.target)
        .unwrap();
    let next = library
        .save_sync_target_configuration(SaveSyncTargetConfiguration {
            id: Some(saved.id.clone()),
            expected_revision: Some(1),
            name: "Bedroom decks".into(),
            target: target.clone(),
        })
        .unwrap();
    assert_eq!(next.revision, 2);
    assert!(
        library
            .verify_sync_target_configuration(&binding, &target)
            .is_err()
    );
    assert!(
        library
            .delete_sync_target_configuration(&saved.id, 1)
            .is_err()
    );
    assert_eq!(
        library.list_sync_target_configurations().unwrap()[0].name,
        "Bedroom decks"
    );
    library
        .delete_sync_target_configuration(&saved.id, 2)
        .unwrap();
    assert!(
        library
            .list_sync_target_configurations()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        std::fs::read_to_string(folder.path().join("VirtualDJ/database.xml")).unwrap(),
        "<VirtualDJ_Database/>"
    );
    assert!(library.history_dj_sync().unwrap().is_empty());
    drop(library);
    assert!(
        Library::open(&folder.path().join("library"))
            .unwrap()
            .list_sync_target_configurations()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn saved_sync_target_validation_never_replaces_a_valid_configuration() {
    let folder = tempfile::tempdir().unwrap();
    let library = Library::open(&folder.path().join("library")).unwrap();
    let target = SavedSyncTarget::DjLibrary {
        target: SyncTarget {
            software: DjSoftware::DjayPro,
            path: folder.path().to_owned(),
            source_root: None,
            environment: Default::default(),
        },
    };
    for name in ["", "   ", "bad\nname"] {
        assert!(
            library
                .save_sync_target_configuration(SaveSyncTargetConfiguration {
                    id: None,
                    expected_revision: None,
                    name: name.into(),
                    target: target.clone(),
                })
                .is_err()
        );
    }
    assert!(
        library
            .list_sync_target_configurations()
            .unwrap()
            .is_empty()
    );
}
