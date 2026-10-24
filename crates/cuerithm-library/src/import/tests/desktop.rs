use super::reconcile::{Fixture, track};
use crate::*;
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;

#[test]
fn preview_history_is_paged_and_redacted() {
    let f = Fixture::new();
    for title in ["First", "Second"] {
        let i = f.inspect(vec![track(title)], Completeness::WholeLibrary);
        let p = f.preview(f.request(&i));
        f.apply(&p);
    }
    let first = f
        .lib
        .import_operation_page(ImportOperationQuery {
            limit: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(first.records.len(), 1);
    assert_eq!(first.total, 2);
    let second = f
        .lib
        .import_operation_page(ImportOperationQuery {
            limit: 1,
            cursor: first.next_cursor.clone(),
            ..Default::default()
        })
        .unwrap();
    assert_ne!(
        first.records[0].operation_id,
        second.records[0].operation_id
    );
    assert!(second.next_cursor.is_none());
    let raw = serde_json::to_string(&first).unwrap();
    assert!(!raw.contains(f.root.path().to_str().unwrap()));
    assert!(!raw.contains("identity_json") && !raw.contains("request_json"));
    assert!(
        f.lib
            .import_operation_page(ImportOperationQuery {
                limit: 1001,
                ..Default::default()
            })
            .is_err()
    );
    let i = f.inspect(vec![track("Third")], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    assert!(
        f.lib
            .import_operation_page(ImportOperationQuery {
                limit: 1,
                cursor: first.next_cursor,
                ..Default::default()
            })
            .is_err()
    );
}
#[test]
fn restart_rejects_stale_preview() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("Invented")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    let f = f.reopen();
    assert!(
        f.lib
            .apply_import(&p.preview_id, &CancellationToken::default())
            .is_err()
    );
    assert_eq!(f.lib.counts().unwrap().tracks, 0);
}
#[test]
fn durable_effect_authority_cannot_be_discarded() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("Invented")], Completeness::WholeLibrary);
    let p = f.preview(f.request(&i));
    for state in ["effects_applying", "effects_applied"] {
        f.lib
            .conn()
            .unwrap()
            .execute(
                "UPDATE import_previews SET state=?1 WHERE id=?2",
                rusqlite::params![state, p.preview_id],
            )
            .unwrap();
        assert!(
            f.lib.discard_import_preview(&p.preview_id).is_err(),
            "{state}"
        );
    }
}
#[test]
fn running_processing_cannot_be_frozen_as_an_effects_preview() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("Invented")], Completeness::WholeLibrary);
    let result = f.apply(&f.preview(f.request(&i)));
    let item = f.lib.tracks(TrackQuery::default()).unwrap().rows[0]
        .id
        .clone();
    f.lib.conn().unwrap().execute("INSERT INTO import_processing_receipts VALUES(?1,?2,'analysis.energy','running-attempt','running','{}','{}',1)",rusqlite::params![result.operation_id,item]).unwrap();
    assert!(
        f.lib
            .preview_import_effects(&result.operation_id, &CancellationToken::default())
            .is_err()
    );
}
#[test]
fn history_bounds_stage_issues_without_losing_total_counts() {
    let f = Fixture::new();
    let i = f.inspect(vec![track("Invented")], Completeness::WholeLibrary);
    let result = f.apply(&f.preview(f.request(&i)));
    let item = f.lib.tracks(TrackQuery::default()).unwrap().rows[0]
        .id
        .clone();
    for n in 0..30 {
        let receipt = serde_json::json!({"error":"invented Ω".repeat(500)}).to_string();
        f.lib.conn().unwrap().execute("INSERT INTO import_processing_receipts VALUES(?1,?2,?3,'attempt','blocked','{}',?4,1)",rusqlite::params![result.operation_id,item,format!("stage.{n}"),receipt]).unwrap();
    }
    let h = f
        .lib
        .import_operation_page(ImportOperationQuery::default())
        .unwrap();
    assert_eq!(h.records[0].stages.get("blocked"), Some(&30));
    assert_eq!(h.records[0].issues.len(), 20);
    assert!(h.records[0].issues.iter().all(|i| i.message.len() <= 1024));
}
#[test]
fn ipc_contract_matches_rust_fixture() {
    use serde_json::json;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/development/fixtures/library-import-ipc.json"
    ))
    .unwrap();
    let actual = json!({"policy":ImportPolicy::default(),"presence":Presence::Value("Invented title"),"capability":FieldCapability{read:true,write:false,lossless:true,evidence:EvidenceLevel::D2,reason:None},"clock_status":ClockStatus::UnresolvedNative,"result":ImportApplyResult{operation_id:"opaque-operation".into(),tracks_created:1,fields_changed:1,collections_changed:0,processing_requested:false}});
    assert_eq!(actual, fixture);
}

#[test]
fn discovered_import_media_never_enters_incoming() {
    let f = Fixture::new();
    let path = f.root.path().join("invented.flac");
    std::fs::write(&path, b"invented bytes; all-off import must not decode").unwrap();
    let mut observed = track("Imported");
    #[cfg(windows)]
    {
        observed.locations.platform = Some(std::env::consts::OS.into());
    }
    observed.locations.native_path = Some(path.clone());
    observed.locations.original = Some(path.to_string_lossy().into_owned());
    observed.locations.resolved = Some(path.to_string_lossy().into_owned());
    observed.locations.availability = "available".into();
    let i = f.inspect(vec![observed], Completeness::WholeLibrary);
    f.apply(&f.preview(f.request(&i)));
    assert_eq!(f.lib.counts().unwrap().tracks, 1);
    assert!(
        f.lib
            .enqueue_with_current_preparation(std::slice::from_ref(&path), true)
            .unwrap()
            .is_empty()
    );
    std::fs::write(&path, b"later authorized tag bytes cause a watcher event").unwrap();
    assert!(
        f.lib
            .enqueue_with_current_preparation(&[path], true)
            .unwrap()
            .is_empty()
    );
    assert!(f.lib.jobs().unwrap().is_empty());
    let item = f.lib.tracks(TrackQuery::default()).unwrap().rows.remove(0);
    assert!(f.lib.analysis(&item.id).unwrap().is_none());
}
