use crate::*;
use cuerithm_dj_sync::import::readers::rekordbox::RekordboxReader;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use cuerithm_rekordbox::DatabaseKey;
use serde_json::{Value, json};
use std::collections::BTreeSet;
#[path = "../../../../cuerithm-dj-sync/tests/import_fixture_support/rekordbox.rs"]
mod fixture;
fn inspect(lib: &Library, root: &std::path::Path) -> ImportInspection {
    let reader = RekordboxReader::with_provisioned_key(
        DatabaseKey::new(b"generated collection key").unwrap(),
        Default::default(),
    );
    lib.inspect_rekordbox_import(
        ImportInspectRequest {
            provider: SourceProvider::Rekordbox,
            selection: ImportSourceSelection {
                root: root.into(),
                artifacts: vec![ArtifactRequest::sqlite("master.db")],
            },
            binding: SourceBindingChoice::Automatic,
            origin: None,
            path_mappings: vec![],
            capture_file_tags: false,
            include_history: false,
            include_queue: false,
        },
        reader,
        &CancellationToken::default(),
    )
    .unwrap()
}
fn preview(lib: &Library, inspection: &ImportInspection) -> ImportPreview {
    lib.preview_import(
        ImportPreviewRequest {
            inspection_id: inspection.inspection_id.clone(),
            tracks: ImportRecordSelection::Ids(BTreeSet::from(["1".into()])),
            collections: ImportRecordSelection::Ids(BTreeSet::new()),
            ..Default::default()
        },
        &CancellationToken::default(),
    )
    .unwrap()
}
#[test]
fn rekordbox_general_import_never_enqueues_incoming() {
    let root = tempfile::tempdir().unwrap();
    drop(fixture::fixture(&root.path().join("master.db")));
    let data = tempfile::tempdir().unwrap();
    let lib = Library::open(&data.path().join("library.db")).unwrap();
    let inspection = inspect(&lib, root.path());
    let preview = preview(&lib, &inspection);
    lib.apply_import(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    assert!(lib.jobs().unwrap().is_empty());
    assert_eq!(
        lib.conn()
            .unwrap()
            .query_row("SELECT count(*) FROM tracks", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    let (state, id): (String, String) = lib
        .conn()
        .unwrap()
        .query_row("SELECT state,id FROM import_items", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(state, "imported");
    assert_eq!(
        lib.imported_item(&id).unwrap().active_fields["metadata.title"]["value"],
        "Signal"
    );
}
fn native_state(lib: &Library) -> Value {
    let db = lib.conn().unwrap();
    let target: (String, String, u64) = db
        .query_row(
            "SELECT identity_json,observation_hash,revision FROM native_targets",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    let link: (String, String, String, String) = db
        .query_row(
            "SELECT identity_json,baseline_json,status,local_track_id FROM native_track_links",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    json!({"target":target,"link":link})
}
#[test]
fn shared_rekordbox_import_reuses_native_link_without_acknowledging_owners_or_baselines() {
    let root = tempfile::tempdir().unwrap();
    drop(fixture::fixture(&root.path().join("master.db")));
    let data = tempfile::tempdir().unwrap();
    let lib = Library::open(&data.path().join("library.db")).unwrap();
    lib.conn().unwrap().execute("INSERT INTO tracks(id,path,title,source_hash,state,created,updated) VALUES('existing',?1,'Local','retained','ready',1,1)",[crate::model::path_bytes(std::path::Path::new("/Synthetic/existing.flac"))]).unwrap();
    let identity = json!({"database_id":"db-alpha","device_id":"device-a","database_version":"version-a","local_update_count":20});
    let track_identity = json!({"database_id":"db-alpha","content_id":"1","uuid":"uuid-1","master_song_id":"master-1","local_usn":20});
    lib.conn().unwrap().execute("INSERT INTO native_targets VALUES('native','rekordbox','db-alpha','device-a',?1,'schema','retained-native-hash','[]',7,1,1)",[identity.to_string()]).unwrap();
    let baseline =
        json!({"title":{"local":"Local","remote":"Earlier","owner":"shared","acknowledged":true}});
    lib.conn().unwrap().execute("INSERT INTO native_track_links VALUES('native','1','existing',?1,'{}','retained-row-hash',?2,'linked',1)",rusqlite::params![track_identity.to_string(),baseline.to_string()]).unwrap();
    let before = native_state(&lib);
    let inspection = inspect(&lib, root.path());
    let preview = preview(&lib, &inspection);
    lib.apply_import(&preview.preview_id, &CancellationToken::default())
        .unwrap();
    let bound: String = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT item_id FROM import_bindings WHERE external_id='1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(bound, "existing");
    let legacy: (Option<String>, Option<String>) = lib
        .conn()
        .unwrap()
        .query_row(
            "SELECT legacy_target_id,legacy_content_id FROM import_bindings WHERE external_id='1'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(legacy, (Some("native".into()), Some("1".into())));
    assert_eq!(lib.track("existing").unwrap().title, "Local");
    assert_eq!(native_state(&lib), before);
    assert!(lib.jobs().unwrap().is_empty());
}

#[test]
fn rekordbox_refresh_reuses_runtime_provisioning_without_persisting_keys() {
    let root = tempfile::tempdir().unwrap();
    drop(fixture::fixture(&root.path().join("master.db")));
    let data = tempfile::tempdir().unwrap();
    let lib = Library::open(&data.path().join("library.db")).unwrap();
    let first = inspect(&lib, root.path());
    let refreshed = lib
        .inspect_import_refresh(&first.source_id, &CancellationToken::default())
        .unwrap();
    assert_eq!(refreshed.source_id, first.source_id);
    assert!(lib.jobs().unwrap().is_empty());
}
