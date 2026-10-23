use cuerithm_library::{Library, LibraryGenerations};

#[test]
fn activation_is_sealed_and_previous_library_is_retained() {
    let root = tempfile::tempdir().unwrap();
    let generations = LibraryGenerations::new(root.path()).unwrap();
    let legacy = Library::open(&root.path().join("library.sqlite")).unwrap();
    drop(legacy);
    assert_eq!(generations.active_id().unwrap(), "legacy");
    let stage = generations.create_staging().unwrap();
    let staged = Library::open(&stage.database).unwrap();
    drop(staged);
    let seal = generations.seal(&stage.id).unwrap();
    generations.schedule(&stage.id, "legacy", &seal).unwrap();
    assert_eq!(generations.active_id().unwrap(), "legacy");
    let opened = generations.open_active().unwrap();
    assert_eq!(generations.active_id().unwrap(), stage.id);
    drop(opened);
    assert!(root.path().join("library.sqlite").exists());
    assert!(
        generations
            .retained()
            .unwrap()
            .iter()
            .any(|g| g.id == "legacy")
    );
}

#[test]
fn modified_sealed_generation_does_not_replace_current_library() {
    let root = tempfile::tempdir().unwrap();
    let generations = LibraryGenerations::new(root.path()).unwrap();
    drop(Library::open(&root.path().join("library.sqlite")).unwrap());
    let stage = generations.create_staging().unwrap();
    drop(Library::open(&stage.database).unwrap());
    let seal = generations.seal(&stage.id).unwrap();
    generations.schedule(&stage.id, "legacy", &seal).unwrap();
    let db = rusqlite::Connection::open(&stage.database).unwrap();
    db.execute("UPDATE meta SET value=value+1 WHERE key='revision'", [])
        .unwrap();
    drop(db);
    assert!(generations.open_active().is_ok());
    assert_eq!(generations.active_id().unwrap(), "legacy");
}

#[test]
fn generation_paths_cannot_escape_and_stale_active_choice_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let generations = LibraryGenerations::new(root.path()).unwrap();
    assert!(generations.database_path("../../outside").is_err());
    let stage = generations.create_staging().unwrap();
    drop(Library::open(&stage.database).unwrap());
    let seal = generations.seal(&stage.id).unwrap();
    assert!(generations.schedule(&stage.id, "wrong", &seal).is_err());
}

#[test]
fn open_old_library_blocks_activation_and_seal_rejects_open_staging() {
    let root = tempfile::tempdir().unwrap();
    let generations = LibraryGenerations::new(root.path()).unwrap();
    let old = Library::open(&root.path().join("library.sqlite")).unwrap();
    let stage = generations.create_staging().unwrap();
    let staging = Library::open(&stage.database).unwrap();
    assert!(generations.seal(&stage.id).is_err());
    drop(staging);
    let seal = generations.seal(&stage.id).unwrap();
    generations.schedule(&stage.id, "legacy", &seal).unwrap();
    assert!(generations.open_active().is_err());
    assert_eq!(generations.active_id().unwrap(), "legacy");
    drop(old);
    drop(generations.open_active().unwrap());
    assert_eq!(generations.active_id().unwrap(), stage.id);
}
#[test]
fn torn_pointer_recovers_last_good_and_missing_stage_rolls_back() {
    let root = tempfile::tempdir().unwrap();
    let generations = LibraryGenerations::new(root.path()).unwrap();
    drop(Library::open(&root.path().join("library.sqlite")).unwrap());
    let stage = generations.create_staging().unwrap();
    drop(Library::open(&stage.database).unwrap());
    let seal = generations.seal(&stage.id).unwrap();
    generations.schedule(&stage.id, "legacy", &seal).unwrap();
    std::fs::write(root.path().join("active-library.json"), b"{torn").unwrap();
    std::fs::remove_dir_all(stage.database.parent().unwrap()).unwrap();
    drop(generations.open_active().unwrap());
    assert_eq!(generations.active_id().unwrap(), "legacy");
    assert!(!root.path().join("library-activation.json").exists());
}
#[test]
fn retained_ignores_unsealed_staging_and_corrupt_generations() {
    let root = tempfile::tempdir().unwrap();
    let generations = LibraryGenerations::new(root.path()).unwrap();
    drop(Library::open(&root.path().join("library.sqlite")).unwrap());
    let stage = generations.create_staging().unwrap();
    drop(Library::open(&stage.database).unwrap());
    assert!(generations.retained().unwrap().is_empty());
    let _seal = generations.seal(&stage.id).unwrap();
    assert_eq!(generations.retained().unwrap().len(), 1);
    std::fs::write(&stage.database, b"corrupt").unwrap();
    assert!(generations.retained().unwrap().is_empty());
}
#[test]
fn switched_pointer_crash_finishes_opening_the_reviewed_generation() {
    let root = tempfile::tempdir().unwrap();
    let generations = LibraryGenerations::new(root.path()).unwrap();
    drop(Library::open(&root.path().join("library.sqlite")).unwrap());
    let stage = generations.create_staging().unwrap();
    drop(Library::open(&stage.database).unwrap());
    let seal = generations.seal(&stage.id).unwrap();
    generations.schedule(&stage.id, "legacy", &seal).unwrap();
    // Simulate a crash after publishing the active pointer and before opening.
    use sha2::{Digest, Sha256};
    let checksum = format!(
        "{:x}",
        Sha256::digest(format!("cuerithm-active-v1:{}", stage.id))
    );
    std::fs::write(
        root.path().join("active-library.json"),
        serde_json::to_vec(&serde_json::json!({"generation":stage.id,"checksum":checksum}))
            .unwrap(),
    )
    .unwrap();
    drop(generations.open_active().unwrap());
    assert_eq!(generations.active_id().unwrap(), stage.id);
    assert!(!root.path().join("library-activation.json").exists());
}
#[test]
fn cancelling_scheduled_restart_keeps_old_active_and_sealed_staging_retained() {
    let root = tempfile::tempdir().unwrap();
    let generations = LibraryGenerations::new(root.path()).unwrap();
    drop(Library::open(&root.path().join("library.sqlite")).unwrap());
    let stage = generations.create_staging().unwrap();
    drop(Library::open(&stage.database).unwrap());
    let seal = generations.seal(&stage.id).unwrap();
    generations.schedule(&stage.id, "legacy", &seal).unwrap();
    assert!(generations.cancel_scheduled().unwrap());
    assert!(!generations.cancel_scheduled().unwrap());
    drop(generations.open_active().unwrap());
    assert_eq!(generations.active_id().unwrap(), "legacy");
    assert!(
        generations
            .retained()
            .unwrap()
            .iter()
            .any(|g| g.id == stage.id)
    );
}
