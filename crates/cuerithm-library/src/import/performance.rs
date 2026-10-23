//! Source occupancy is evidence, independent of a decoded empty cue array.
use anyhow::Result;
use cuerithm_dj_sync::import::*;
use rusqlite::Connection;
use serde_json::Value;
pub(super) fn binding_identity(snapshot_id: &str, track: &TrackObservation) -> Value {
    serde_json::json!({"snapshot_id":snapshot_id,"location":track.locations,"hotcue_occupancy":if unknown_hotcue_occupancy(track){"unknown"}else{"observed"}})
}
pub(super) fn unknown_hotcue_occupancy(track: &TrackObservation) -> bool {
    track
        .extensions
        .get("hotcue_occupancy")
        .and_then(Value::as_str)
        == Some("unknown")
        || track.cues.iter().any(|cue| {
            cue.family == CueFamily::Unknown
                || matches!(cue.family, CueFamily::Hot | CueFamily::Loop)
                    && cue.flags.get("slot_mapping_qualified") == Some(&Value::Bool(false))
        })
        || track.issues.iter().any(|issue| {
            matches!(
                issue.field.as_deref(),
                Some("cues" | "cues.hot" | "cues/grid" | "quickCues")
            ) || issue.code.contains("hotcue_occupancy")
        })
}
pub(super) fn bound_unknown_occupancy(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM import_bindings WHERE item_id=?1 AND json_extract(identity_json,'$.hotcue_occupancy')='unknown')",[id],|r|r.get(0))?)
}
pub(super) fn has_hot_loops(value: &Value) -> bool {
    value.as_array().is_some_and(|cues| {
        cues.iter().any(|cue| {
            cue.get("source_slot").is_some_and(|v| !v.is_null())
                && cue
                    .pointer("/flags/slot_namespace")
                    .or_else(|| cue.pointer("/flags/source_slot_role"))
                    .and_then(Value::as_str)
                    != Some("saved_loop")
        })
    })
}
pub(super) fn hotcue_occupied(fields: &std::collections::BTreeMap<String, Value>) -> bool {
    fields
        .get("cues.hot")
        .is_some_and(|v| !v.is_null() && !v.as_array().is_some_and(Vec::is_empty))
        || fields.get("cues.loop").is_some_and(has_hot_loops)
}
