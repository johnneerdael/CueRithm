use super::*;
fn add_playlist(library: &Library, id: &str, name: &str) {
    let connection = library.conn().unwrap();
    connection.execute("INSERT INTO playlists(id,kind,name,position,last_edit_id) VALUES(?1,'playlist',?2,0,'fixture')",params![id,name]).unwrap();
    Library::bump(&connection).unwrap();
}
#[test]
fn published_unacknowledged_sync_blocks_new_previews_and_recovers_same_creation_proof() {
    let (_dir, library, target) = super::tests::fixture();
    add_playlist(&library, "owned", "Owned");
    let preview = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    let root = library.data_dir().join("sync");
    cuerithm_dj_sync::apply_sync(&root, &preview.id).unwrap();
    assert!(ownership::read(&root, &target).unwrap().stores.is_empty());
    assert!(
        library
            .history_dj_sync()
            .unwrap()
            .iter()
            .any(|h| h.id == preview.id && h.needs_acknowledgement)
    );
    assert!(
        library
            .preview_dj_sync(super::tests::request(&target, DjSyncScope::Changed))
            .unwrap_err()
            .to_string()
            .contains("needs recovery")
    );
    library.apply_dj_sync(&preview.id).unwrap();
    let ledger = ownership::read(&root, &target).unwrap();
    assert!(
        ledger
            .stores
            .values()
            .flat_map(|(_, s)| &s.bindings)
            .any(|b| b.source_id == "owned" && b.created)
    );
    assert!(
        !library
            .history_dj_sync()
            .unwrap()
            .iter()
            .any(|h| h.needs_acknowledgement)
    );
    assert_eq!(
        library
            .preview_dj_sync(super::tests::request(&target, DjSyncScope::Changed))
            .unwrap()
            .tracks,
        0
    );
}
#[test]
fn persisted_transition_recovery_is_idempotent_without_a_second_native_write() {
    let (_dir, library, target) = super::tests::fixture();
    add_playlist(&library, "owned", "Owned");
    let preview = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    library.apply_dj_sync(&preview.id).unwrap();
    let root = library.data_dir().join("sync");
    let transaction = FileTransaction::open(&root, &preview.id).unwrap();
    let before = digest(&ownership::read(&root, &target).unwrap()).unwrap();
    let bytes = fs::read(&target.path).unwrap();
    let mut saved = read_library_journal(&transaction).unwrap();
    saved.acknowledged = false;
    atomic_json(&transaction.path().join("library.json"), &saved).unwrap();
    invalidate_baseline(&root, &target).unwrap();
    library.apply_dj_sync(&preview.id).unwrap();
    assert_eq!(
        before,
        digest(&ownership::read(&root, &target).unwrap()).unwrap()
    );
    assert_eq!(fs::read(&target.path).unwrap(), bytes);
    assert!(
        library
            .apply_dj_sync(&preview.id)
            .unwrap_err()
            .to_string()
            .contains("already acknowledged")
    );
}
#[test]
fn changed_ledger_or_replaced_database_blocks_publication_before_any_write() {
    let (_dir, library, target) = super::tests::fixture();
    add_playlist(&library, "owned", "Owned");
    let first = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    library.apply_dj_sync(&first.id).unwrap();
    let root = library.data_dir().join("sync");
    let preview = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    let mut ledger = ownership::read(&root, &target).unwrap();
    ledger.generation += 1;
    ownership::save(&root, &target, &ledger).unwrap();
    let bytes = fs::read(&target.path).unwrap();
    assert!(
        library
            .apply_dj_sync(&preview.id)
            .unwrap_err()
            .to_string()
            .contains("ownership changed")
    );
    assert_eq!(fs::read(&target.path).unwrap(), bytes);
    let replacement = target.path.with_extension("replacement");
    fs::write(&replacement, &bytes).unwrap();
    fs::rename(replacement, &target.path).unwrap();
    assert!(
        library
            .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
            .unwrap_err()
            .to_string()
            .contains("database or device was replaced")
    );
}
#[test]
fn source_reorder_changes_playlist_baseline_even_when_every_node_and_member_is_unchanged() {
    let mut library = SyncLibrary {
        tracks: vec![],
        playlists: vec![
            cuerithm_dj_sync::SyncPlaylist {
                id: "a".into(),
                name: "A".into(),
                ..Default::default()
            },
            cuerithm_dj_sync::SyncPlaylist {
                id: "b".into(),
                name: "B".into(),
                ..Default::default()
            },
        ],
    };
    let first = playlist_hashes(&library).unwrap();
    library.playlists.swap(0, 1);
    assert_ne!(first, playlist_hashes(&library).unwrap());
}
#[test]
fn engine_drive_library_folder_and_database_share_one_target_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let drive = dir.path().canonicalize().unwrap();
    let folder = drive.join("Engine Library");
    let database = folder.join("Database2/m.db");
    fs::create_dir_all(database.parent().unwrap()).unwrap();
    fs::write(&database, b"synthetic database location").unwrap();
    let resolve = |path: PathBuf| {
        canonical_target(SyncTarget {
            environment: Default::default(),
            software: DjSoftware::EngineDj,
            path,
            source_root: None,
        })
        .unwrap()
    };
    let expected = digest(&resolve(database.clone())).unwrap();
    for path in [drive, folder, database.parent().unwrap().into()] {
        assert_eq!(digest(&resolve(path)).unwrap(), expected);
    }
}
#[test]
fn native_restore_completion_recovers_local_state_and_blocks_new_previews_until_finished() {
    let (_dir, library, target) = super::tests::fixture();
    add_playlist(&library, "owned", "Owned");
    let preview = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    library.apply_dj_sync(&preview.id).unwrap();
    let root = library.data_dir().join("sync");
    cuerithm_dj_sync::restore_sync(&root, &preview.id).unwrap();
    assert!(ownership::read(&root, &target).unwrap().contains("owned"));
    assert!(
        library
            .history_dj_sync()
            .unwrap()
            .iter()
            .any(|h| h.needs_restore_acknowledgement)
    );
    assert!(
        library
            .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
            .unwrap_err()
            .to_string()
            .contains("needs recovery")
    );
    library.restore_dj_sync(&preview.id).unwrap();
    assert!(!ownership::read(&root, &target).unwrap().contains("owned"));
    assert!(read_baseline(&root, &target).unwrap().is_none());
    assert!(
        !library
            .history_dj_sync()
            .unwrap()
            .iter()
            .any(|h| h.needs_restore_acknowledgement)
    );
    let restored = digest(&ownership::read(&root, &target).unwrap()).unwrap();
    library.restore_dj_sync(&preview.id).unwrap();
    assert_eq!(
        restored,
        digest(&ownership::read(&root, &target).unwrap()).unwrap()
    );
    let transaction = FileTransaction::open(&root, &preview.id).unwrap();
    let mut saved = read_library_journal(&transaction).unwrap();
    saved.restore_acknowledged = false;
    atomic_json(&transaction.path().join("library.json"), &saved).unwrap();
    let bytes = fs::read(&target.path).unwrap();
    let replacement = target.path.with_extension("replacement");
    fs::write(&replacement, &bytes).unwrap();
    fs::rename(replacement, &target.path).unwrap();
    assert!(
        library
            .restore_dj_sync(&preview.id)
            .unwrap_err()
            .to_string()
            .contains("database or device was replaced")
    );
    assert_eq!(fs::read(&target.path).unwrap(), bytes);
}
#[test]
fn first_preview_and_acknowledged_restore_reject_identical_database_replacements() {
    let (_dir, library, target) = super::tests::fixture();
    let preview = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    let replace = || {
        let bytes = fs::read(&target.path).unwrap();
        let temp = target.path.with_extension("replacement");
        fs::write(&temp, &bytes).unwrap();
        fs::rename(temp, &target.path).unwrap();
        bytes
    };
    let bytes = replace();
    assert!(
        library
            .apply_dj_sync(&preview.id)
            .unwrap_err()
            .to_string()
            .contains("replaced after preview")
    );
    assert_eq!(fs::read(&target.path).unwrap(), bytes);
    let next = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    library.apply_dj_sync(&next.id).unwrap();
    let bytes = replace();
    assert!(
        library
            .restore_dj_sync(&next.id)
            .unwrap_err()
            .to_string()
            .contains("database or device was replaced")
    );
    assert_eq!(fs::read(&target.path).unwrap(), bytes);
}
#[test]
fn contexts_omit_already_acknowledged_or_never_exported_tombstones() {
    let (_dir, library, target) = super::tests::fixture();
    add_playlist(&library, "owned", "Owned");
    library
        .apply_playlist_edit(
            crate::PlaylistEdit::Remove { id: "owned".into() },
            library.playlist_state().unwrap().revision,
        )
        .unwrap();
    let ledger = ownership::OwnershipLedger {
        stores: BTreeMap::from([(
            target.path.clone(),
            (
                None,
                cuerithm_dj_sync::ownership::PlaylistOwnership::default(),
            ),
        )]),
        ..Default::default()
    };
    let requests =
        ownership::contexts(&library.conn().unwrap(), &ledger, &DjSyncScope::Changed).unwrap();
    assert!(requests.values().all(|(_, r)| r.removed.is_empty()));
}
#[test]
fn djay_changed_after_an_acknowledged_deletion_is_a_valid_noop() {
    let (_dir, library, old_target) = super::tests::fixture();
    add_playlist(&library, "owned", "Owned");
    let database = old_target.path.parent().unwrap().join("MediaLibrary.db");
    let db = Connection::open(&database).unwrap();
    db.execute_batch("CREATE TABLE database2(rowid INTEGER PRIMARY KEY,collection TEXT NOT NULL,key TEXT NOT NULL,data BLOB,metadata BLOB,UNIQUE(collection,key));CREATE TABLE yap2(extension TEXT,key TEXT,data);").unwrap();
    drop(db);
    let target = SyncTarget {
        environment: Default::default(),
        software: DjSoftware::DjayPro,
        path: database,
        source_root: None,
    };
    let first = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    library.apply_dj_sync(&first.id).unwrap();
    library
        .apply_playlist_edit(
            crate::PlaylistEdit::Remove { id: "owned".into() },
            library.playlist_state().unwrap().revision,
        )
        .unwrap();
    let removed = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Changed))
        .unwrap();
    library.apply_dj_sync(&removed.id).unwrap();
    let again = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Changed))
        .unwrap();
    assert_eq!(again.tracks, 0);
    assert_eq!(again.playlists, 0);
    assert_eq!(again.deleted_playlists, 0);
}
#[test]
fn virtual_dj_case_only_subtree_rename_applies_and_restores_exact_directory_spelling() {
    fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let relative = path.strip_prefix(root).unwrap().to_path_buf();
                if entry.file_type().unwrap().is_dir() {
                    result.insert(relative, None);
                    visit(root, &path, result);
                } else {
                    result.insert(relative, Some(fs::read(path).unwrap()));
                }
            }
        }
        let mut result = BTreeMap::new();
        visit(root, root, &mut result);
        result
    }
    let (_dir, library, target) = super::tests::fixture();
    library.conn().unwrap().execute_batch("INSERT INTO playlists(id,kind,name,position,last_edit_id)VALUES('folder','folder','Sets',0,'fixture');INSERT INTO playlists(id,parent_id,kind,name,position,last_edit_id)VALUES('leaf','folder','playlist','Intro',0,'fixture');").unwrap();
    let first = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Full))
        .unwrap();
    library.apply_dj_sync(&first.id).unwrap();
    let lists = target.path.parent().unwrap().join("MyLists");
    let original = tree(&lists);
    for (id, name) in [("folder", "sets"), ("leaf", "intro")] {
        library
            .apply_playlist_edit(
                crate::PlaylistEdit::Rename {
                    id: id.into(),
                    name: name.into(),
                },
                library.playlist_state().unwrap().revision,
            )
            .unwrap();
    }
    let changed = library
        .preview_dj_sync(super::tests::request(&target, DjSyncScope::Changed))
        .unwrap();
    library.apply_dj_sync(&changed.id).unwrap();
    let after = tree(&lists);
    assert_ne!(original, after);
    assert!(after.keys().any(|p| p.to_string_lossy().contains("sets-")));
    assert!(after.keys().any(|p| p.to_string_lossy().contains("intro-")));
    library.restore_dj_sync(&changed.id).unwrap();
    assert_eq!(original, tree(&lists));
}
