use crate::*;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

fn chunk(name: &[u8; 4], value: &str) -> Vec<u8> {
    let body = value
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    let mut bytes = name.to_vec();
    bytes.extend((body.len() as u32).to_be_bytes());
    bytes.extend(body);
    bytes
}
fn crate_file(root: &Path, name: &str) {
    let directory = root.join("_Serato_/Subcrates");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join(format!("{name}.crate")),
        chunk(b"vrsn", "1.0/Serato ScratchLive Crate"),
    )
    .unwrap();
}
fn request(selection: ImportSourceSelection) -> ImportInspectRequest {
    ImportInspectRequest {
        provider: SourceProvider::Serato3,
        selection,
        binding: SourceBindingChoice::Automatic,
        origin: None,
        path_mappings: vec![],
        capture_file_tags: false,
        include_history: false,
        include_queue: false,
    }
}
fn names(lib: &Library, inspection: &ImportInspection) -> BTreeSet<String> {
    lib.import_page(
        &inspection.inspection_id,
        ImportPageQuery {
            kind: ImportRecordKind::Collection,
            ..Default::default()
        },
    )
    .unwrap()
    .records
    .into_iter()
    .map(|record| match record {
        ObservationRecord::Collection(collection) => collection.name,
        _ => panic!("Expected collection"),
    })
    .collect()
}
fn initial_import(lib: &Library, request: ImportInspectRequest) -> ImportInspection {
    let cancel = CancellationToken::default();
    let inspection = lib.inspect_import(request, &cancel).unwrap();
    let preview = lib
        .preview_import(
            ImportPreviewRequest {
                inspection_id: inspection.inspection_id.clone(),
                ..Default::default()
            },
            &cancel,
        )
        .unwrap();
    lib.apply_import(&preview.preview_id, &cancel).unwrap();
    inspection
}

#[test]
fn folder_refresh_discovers_added_crates_and_preserves_explicit_additional_files() {
    let data = tempfile::tempdir().unwrap();
    let main = tempfile::tempdir().unwrap();
    let extra = tempfile::tempdir().unwrap();
    crate_file(main.path(), "Existing");
    crate_file(extra.path(), "Selected extra");
    let cancel = CancellationToken::default();
    let mut selection = ImportSourceSelection::from_folder(
        SourceProvider::Serato3,
        &main.path().join("_Serato_"),
        &cancel,
    )
    .unwrap();
    let explicit = ArtifactRequest::stable("Selected extra.crate").with_selected_root(
        "selected-volume-42/Selected extra.crate",
        &extra.path().join("_Serato_/Subcrates"),
    );
    selection.artifacts.push(explicit.clone());
    let path = data.path().join("library.db");
    let lib = Library::open(&path).unwrap();
    let first = initial_import(&lib, request(selection));
    assert_eq!(
        names(&lib, &first),
        ["Existing".into(), "Selected extra".into()].into()
    );
    drop(lib);
    crate_file(main.path(), "Added");
    crate_file(extra.path(), "Unselected neighbor");
    let lib = Library::open(&path).unwrap();
    let refreshed = lib
        .inspect_import_refresh(&first.source_id, &cancel)
        .unwrap();
    assert_eq!(refreshed.source_id, first.source_id);
    assert_eq!(
        names(&lib, &refreshed),
        ["Existing".into(), "Added".into(), "Selected extra".into()].into()
    );
    assert!(
        refreshed.header.scope["artifacts"]
            .as_array()
            .unwrap()
            .contains(&json!(explicit.id))
    );
    let raw: String = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT request_json FROM import_snapshots WHERE id=?1",
            [&refreshed.inspection_id],
            |r| r.get(0),
        )
        .unwrap();
    let stored: ImportInspectRequest = serde_json::from_str(&raw).unwrap();
    let extras = stored
        .selection
        .artifacts
        .iter()
        .filter(|a| a.selected_root.is_some())
        .collect::<Vec<_>>();
    assert_eq!(serde_json::to_value(extras).unwrap(), json!([explicit]));
    assert!(stored.selection.folder_inventory);
}

#[test]
fn folder_refresh_replaces_removed_and_renamed_primary_crates_before_capture() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    crate_file(root.path(), "Old name");
    crate_file(root.path(), "Deleted");
    let cancel = CancellationToken::default();
    let lib = Library::open(&data.path().join("library.db")).unwrap();
    let selection = ImportSourceSelection::from_folder(
        SourceProvider::Serato3,
        &root.path().join("_Serato_"),
        &cancel,
    )
    .unwrap();
    let first = initial_import(&lib, request(selection));
    fs::rename(
        root.path().join("_Serato_/Subcrates/Old name.crate"),
        root.path().join("_Serato_/Subcrates/Renamed.crate"),
    )
    .unwrap();
    fs::remove_file(root.path().join("_Serato_/Subcrates/Deleted.crate")).unwrap();
    let refreshed = lib
        .inspect_import_refresh(&first.source_id, &cancel)
        .unwrap();
    assert_eq!(refreshed.source_id, first.source_id);
    assert_eq!(names(&lib, &refreshed), ["Renamed".into()].into());
    assert_eq!(
        refreshed.header.scope["artifacts"],
        json!(["Subcrates/Renamed.crate"])
    );
    // Selected-scope observations do not authorize deletion of previously imported collections.
    assert_eq!(
        lib.conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM import_collections", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn folder_refresh_initial_inspection_reinventories_a_delayed_folder_selection() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    crate_file(root.path(), "Selected first");
    let cancel = CancellationToken::default();
    let selection = ImportSourceSelection::from_folder(
        SourceProvider::Serato3,
        &root.path().join("_Serato_"),
        &cancel,
    )
    .unwrap();
    fs::remove_file(root.path().join("_Serato_/Subcrates/Selected first.crate")).unwrap();
    crate_file(root.path(), "Current");
    let lib = Library::open(&data.path().join("library.db")).unwrap();
    let inspection = lib.inspect_import(request(selection), &cancel).unwrap();
    assert_eq!(names(&lib, &inspection), ["Current".into()].into());
}

#[test]
fn refresh_scope_legacy_and_explicit_artifact_selections_do_not_expand() {
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    crate_file(root.path(), "Selected");
    crate_file(root.path(), "Neighbor");
    let cancel = CancellationToken::default();
    let folder = ImportSourceSelection::from_folder(
        SourceProvider::Serato3,
        &root.path().join("_Serato_"),
        &cancel,
    )
    .unwrap();
    let mut legacy = serde_json::to_value(&folder).unwrap();
    legacy.as_object_mut().unwrap().remove("folder_inventory");
    let mut selection: ImportSourceSelection = serde_json::from_value(legacy).unwrap();
    assert!(!selection.folder_inventory);
    selection
        .artifacts
        .retain(|a| a.id == "Subcrates/Selected.crate");
    assert!(
        serde_json::to_value(&selection)
            .unwrap()
            .get("folder_inventory")
            .is_none()
    );
    let lib = Library::open(&data.path().join("library.db")).unwrap();
    let first = initial_import(&lib, request(selection));
    crate_file(root.path(), "Later neighbor");
    let refreshed = lib
        .inspect_import_refresh(&first.source_id, &cancel)
        .unwrap();
    assert_eq!(names(&lib, &refreshed), ["Selected".into()].into());
    assert_eq!(
        refreshed.header.scope["artifacts"],
        json!(["Subcrates/Selected.crate"])
    );
}

#[test]
fn refresh_scope_folder_marker_keeps_existing_root_identity_bytes() {
    #[derive(serde::Serialize)]
    struct LegacyRoot<'a> {
        #[serde(with = "cuerithm_dj_sync::path_serde")]
        root: &'a Path,
        artifacts: Vec<ArtifactRequest>,
    }
    let data = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    crate_file(root.path(), "Selected");
    let cancel = CancellationToken::default();
    let folder = ImportSourceSelection::from_folder(
        SourceProvider::Serato3,
        &root.path().join("_Serato_"),
        &cancel,
    )
    .unwrap();
    let expected = format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&LegacyRoot {
                root: &folder.root,
                artifacts: vec![]
            })
            .unwrap()
        )
    );
    let lib = Library::open(&data.path().join("library.db")).unwrap();
    let first = lib
        .inspect_import(request(folder.clone()), &cancel)
        .unwrap();
    let stored: String = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT location_key FROM import_sources WHERE id=?1",
            [&first.source_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, expected);
    let mut fixed = folder;
    fixed.folder_inventory = false;
    let again = lib.inspect_import(request(fixed), &cancel).unwrap();
    assert_eq!(again.source_id, first.source_id);
    assert_eq!(
        lib.conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM import_sources", [], |r| r
                .get::<_, u64>(0))
            .unwrap(),
        1
    );
}
