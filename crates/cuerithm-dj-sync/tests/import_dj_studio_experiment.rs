#![cfg(feature = "experimental-djstudio-write")]
use cuerithm_dj_sync::import::experimental::*;
use cuerithm_media::CancellationToken;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
fn patch() -> ExistingRecordPatch {
    ExistingRecordPatch {
        idempotency_key: "metadata-case".into(),
        metadata: BTreeMap::from([
            ("title".into(), "Owned title".into()),
            ("artist".into(), "Owned artist".into()),
            ("comment".into(), "Owned comment".into()),
        ]),
        effective_key: Some(13),
    }
}
fn hashes(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.file_name().unwrap() == ".cuerithm-experiment" {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out)
            } else {
                out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                    cuerithm_media::file_sha256(&path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
fn audio(root: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(root.join("Database/audio-library-table/3f/file_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")).unwrap()).unwrap()
}
#[test]
fn existing_record_patch_preserves_unowned_persistence() {
    let root = generate_disposable_fixture().unwrap();
    let before = hashes(root.path());
    let original = audio(root.path());
    let preview =
        preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).unwrap();
    let receipt = apply_djstudio_experiment(&preview, &CancellationToken::default()).unwrap();
    assert!(!receipt.production_write_enabled);
    let current = audio(root.path());
    assert_eq!(current["tag"]["title"], "Owned title");
    assert_eq!(current["tag"]["artist"], "Owned artist");
    assert_eq!(current["tag"]["comment"], "Owned comment");
    assert_eq!(current["camelotKey"], 13);
    for field in [
        "originalCamelotKey",
        "noteKey",
        "mikKey",
        "structureKey",
        "beatGrids",
        "bpm",
        "bpmLine",
        "cueData",
        "externalRec",
        "unowned",
    ] {
        assert_eq!(
            current.get(field),
            original.get(field),
            "changed unowned {field}"
        );
    }
    let db = rusqlite::Connection::open_with_flags(
        root.path().join("Database/studio.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let row: (String, String, i64, i64) = db
        .query_row(
            "SELECT title,artist,key,analyzed FROM Tracks WHERE id=101",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(row, ("Owned title".into(), "Owned artist".into(), 13, 1));
    assert_eq!(
        db.query_row("SELECT count(*) FROM PlaylistTracks", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        db.query_row("SELECT identifier FROM TrackLibraryMap LIMIT 1", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "external-opaque-id"
    );
    drop(db);
    let after = hashes(root.path());
    for (path, hash) in &before {
        if path != "Database/studio.db"
            && !path.starts_with("Database/audio-library-table/")
            && path != "Cache/Database/audio-library-table.json"
        {
            assert_eq!(
                after.get(path),
                Some(hash),
                "unowned persistence changed {path}"
            );
        }
    }
    assert!(
        !root
            .path()
            .join("Cache/Database/audio-library-table.json")
            .exists()
    );
    let again = apply_djstudio_experiment(&preview, &CancellationToken::default()).unwrap();
    assert_eq!(again.operation_id, receipt.operation_id);
    assert_eq!(hashes(root.path()), after);
}
#[test]
fn failure_at_every_replace_restores_original_hashes() {
    let count = {
        let root = generate_disposable_fixture().unwrap();
        preview_djstudio_experiment(&root, patch(), &CancellationToken::default())
            .unwrap()
            .transition_count()
    };
    for at in 0..count {
        let root = generate_disposable_fixture().unwrap();
        let original = hashes(root.path());
        let preview =
            preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).unwrap();
        root.inject_fault(Some(ExperimentFault::FailAt(at)));
        assert!(
            apply_djstudio_experiment(&preview, &CancellationToken::default()).is_err(),
            "fault {at} was not reached"
        );
        assert_eq!(hashes(root.path()), original, "rollback {at}");
        root.inject_fault(None);
        let receipt = apply_djstudio_experiment(&preview, &CancellationToken::default()).unwrap();
        assert!(!receipt.production_write_enabled);
    }
}
#[test]
fn experiment_cannot_address_live_or_unknown_root() {
    let root = generate_disposable_fixture().unwrap();
    root.simulate_process_running(true);
    assert!(preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).is_err());
    root.simulate_process_running(false);
    let marker = root.path().join(".fixture.json");
    let mut value: Value = serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    value["renderer"] = json!("unknown renderer");
    std::fs::write(&marker, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).is_err());
}
#[test]
fn stale_hash_and_out_of_scope_patch_never_change_other_files() {
    let root = generate_disposable_fixture().unwrap();
    let mut invalid = patch();
    invalid.metadata.insert("bpm".into(), "130".into());
    assert!(preview_djstudio_experiment(&root, invalid, &CancellationToken::default()).is_err());
    let mut invalid = patch();
    invalid.effective_key = Some(-1);
    assert!(preview_djstudio_experiment(&root, invalid, &CancellationToken::default()).is_err());
    let preview =
        preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).unwrap();
    let path=root.path().join("Database/audio-library-table/3f/file_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    let mut value = audio(root.path());
    value["tag"]["title"] = json!("later external edit");
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let before = hashes(root.path());
    assert!(apply_djstudio_experiment(&preview, &CancellationToken::default()).is_err());
    assert_eq!(hashes(root.path()), before);
}
#[test]
fn interrupted_publication_and_recovery_are_idempotent() {
    let root = generate_disposable_fixture().unwrap();
    let before = hashes(root.path());
    let preview =
        preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).unwrap();
    root.inject_fault(Some(ExperimentFault::InterruptAt(2)));
    assert!(apply_djstudio_experiment(&preview, &CancellationToken::default()).is_err());
    assert_ne!(hashes(root.path()), before);
    root.inject_fault(None);
    let recovered = recover_djstudio_experiment(&root, &CancellationToken::default()).unwrap();
    assert_eq!(recovered.restored, 1);
    assert_eq!(hashes(root.path()), before);
    assert_eq!(
        recover_djstudio_experiment(&root, &CancellationToken::default())
            .unwrap()
            .restored,
        0
    );
}

#[test]
fn interrupted_recovery_resumes_without_overwriting_later_edits() {
    let root = generate_disposable_fixture().unwrap();
    let before = hashes(root.path());
    let preview =
        preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).unwrap();
    root.inject_fault(Some(ExperimentFault::InterruptAt(4)));
    assert!(apply_djstudio_experiment(&preview, &CancellationToken::default()).is_err());
    root.inject_fault(Some(ExperimentFault::InterruptAt(101)));
    assert!(recover_djstudio_experiment(&root, &CancellationToken::default()).is_err());
    assert_ne!(hashes(root.path()), before);
    assert_eq!(
        recover_djstudio_experiment(&root, &CancellationToken::default())
            .unwrap()
            .restored,
        1
    );
    assert_eq!(hashes(root.path()), before);

    root.inject_fault(Some(ExperimentFault::InterruptAt(2)));
    assert!(apply_djstudio_experiment(&preview, &CancellationToken::default()).is_err());
    let path = root
        .path()
        .join("Database/projects-table/0/synthetic-project");
    std::fs::write(path, b"later project edit").unwrap();
    let edited = hashes(root.path());
    assert!(recover_djstudio_experiment(&root, &CancellationToken::default()).is_err());
    assert_eq!(hashes(root.path()), edited);
}

#[test]
fn native_profile_and_exclusive_lease_are_required() {
    use fs2::FileExt;
    let root = generate_disposable_fixture().unwrap();
    let lease = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.path().join(".cuerithm-experiment/lease"))
        .unwrap();
    lease.lock_exclusive().unwrap();
    assert!(preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).is_err());
    FileExt::unlock(&lease).unwrap();
    let marker = root.path().join(".fixture.json");
    let mut value: Value = serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    value["native"] = json!("unknown native version");
    std::fs::write(marker, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).is_err());
}

#[test]
fn root_replacement_and_cancellation_cannot_publish() {
    let root = generate_disposable_fixture().unwrap();
    let preview =
        preview_djstudio_experiment(&root, patch(), &CancellationToken::default()).unwrap();
    let token = CancellationToken::default();
    token.cancel();
    let before = hashes(root.path());
    assert!(apply_djstudio_experiment(&preview, &token).is_err());
    assert_eq!(hashes(root.path()), before);
    let moved = root.path().with_extension("retained-generated-evidence");
    std::fs::rename(root.path(), &moved).unwrap();
    std::fs::create_dir(root.path()).unwrap();
    let guard = root.path().join("unrelated");
    std::fs::write(&guard, b"do not touch").unwrap();
    assert!(apply_djstudio_experiment(&preview, &CancellationToken::default()).is_err());
    assert_eq!(std::fs::read(guard).unwrap(), b"do not touch");
}
