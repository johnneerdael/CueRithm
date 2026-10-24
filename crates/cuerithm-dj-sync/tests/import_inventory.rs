use cuerithm_dj_sync::import::{ArtifactKind, SourceProvider, inventory::*};
use cuerithm_media::CancellationToken;
use std::{collections::BTreeSet, fs, path::Path};

fn file(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b"invented inventory fixture").unwrap();
}

#[test]
fn folder_inventory_keeps_engine_sqlite_profiles_and_stable_relative_ids() {
    let root = tempfile::tempdir().unwrap();
    for name in ["m.db", "stm.db", "hm.db", "p.db"] {
        file(root.path(), &format!("Engine Library/Database2/{name}"));
    }
    let artifacts = inventory_source(
        root.path(),
        SourceProvider::EngineDj,
        &CancellationToken::default(),
    )
    .unwrap();
    let ids = artifacts
        .iter()
        .map(|a| a.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        [
            "Engine Library/Database2/m.db",
            "Engine Library/Database2/stm.db",
            "Engine Library/Database2/hm.db"
        ]
        .into()
    );
    assert!(
        artifacts
            .iter()
            .all(|a| matches!(a.kind, ArtifactKind::Sqlite) && a.selected_root.is_none())
    );
    assert_eq!(
        serde_json::to_value(&artifacts).unwrap(),
        serde_json::to_value(
            inventory_source(
                root.path(),
                SourceProvider::EngineDj,
                &CancellationToken::default()
            )
            .unwrap()
        )
        .unwrap()
    );
}

#[test]
fn folder_inventory_retains_studio_allowlist_without_reading_account_or_browser_storage() {
    let root = tempfile::tempdir().unwrap();
    for relative in [
        "Database/studio.db",
        "Database/audio-library-table/a/record",
        "Database/track-structures-table/a/fixed",
        "Database/account-table/secret",
        "Cache/Database/audio-library-table.json",
        "Local Storage/secret",
    ] {
        file(root.path(), relative);
    }
    let artifacts = inventory_source(
        root.path(),
        SourceProvider::DjStudio,
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(artifacts.len(), 3);
    assert!(
        artifacts
            .iter()
            .all(|a| !a.id.contains("secret") && !a.id.starts_with("Cache"))
    );
}

#[cfg(unix)]
#[test]
fn folder_inventory_never_follows_symlinks_or_scans_excluded_music_folders() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    file(root.path(), "Subcrates/Selected.crate");
    file(root.path(), "Music/Not selected.crate");
    file(outside.path(), "Outside.crate");
    std::os::unix::fs::symlink(
        outside.path().join("Outside.crate"),
        root.path().join("Subcrates/Linked.crate"),
    )
    .unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("Linked directory")).unwrap();
    let artifacts = inventory_source(
        root.path(),
        SourceProvider::Serato3,
        &CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(
        artifacts.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
        vec!["Subcrates/Selected.crate"]
    );
    assert_eq!(
        fs::read(outside.path().join("Outside.crate")).unwrap(),
        b"invented inventory fixture"
    );
}

#[test]
fn folder_inventory_obeys_cancellation_and_hierarchy_limit() {
    let root = tempfile::tempdir().unwrap();
    file(root.path(), "Selected.crate");
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(inventory_source(root.path(), SourceProvider::Serato3, &cancel).is_err());
    let mut path = root.path().to_path_buf();
    for _ in 0..129 {
        path.push("d");
    }
    fs::create_dir_all(path).unwrap();
    let error = inventory_source(
        root.path(),
        SourceProvider::Serato3,
        &CancellationToken::default(),
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("hierarchy exceeds128"),
        "{error:#}"
    );
}
