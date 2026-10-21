use cuerithm_library::*;
use serde_json::json;

#[test]
fn native_scope_and_clear_are_explicit_and_unresolved_fields_remain_reviewable() {
    let options = RekordboxSyncOptions::default();
    assert!(options.metadata_fields.contains("title"));
    assert!(!options.materialize_smart_playlists);
    let request: RekordboxSyncRequest = serde_json::from_value(json!({
        "target_id":"server-discovery-id", "scope":{"scope":"full"},
        "options": options, "decisions":{"local":{"title":"clear_remote"}}
    }))
    .unwrap();
    assert_eq!(
        request.decisions["local"]["title"],
        NativeFieldDecision::ClearRemote
    );
    assert!(
        serde_json::from_value::<RekordboxSyncRequest>(json!({
            "target_id":"caller-path", "scope":{"scope":"full"}, "options":options,
            "database":"/caller/master.db", "decisions":{}
        }))
        .is_err()
    );
}
