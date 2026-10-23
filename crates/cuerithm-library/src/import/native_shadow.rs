//! Track legacy editors independently from imported active values. A legacy column
//! or metadata surface can change while its imported projection remains unchanged.
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub(super) fn load(conn: &Connection, id: &str) -> Result<BTreeMap<String, Value>> {
    let raw:Option<String>=conn.query_row("SELECT value_json FROM import_active_fields WHERE item_id=?1 AND field='_native_shadow'",[id],|r|r.get(0)).optional()?;
    raw.map(|s| Ok(serde_json::from_str(&s)?))
        .unwrap_or_else(|| Ok(BTreeMap::new()))
}
pub(super) fn capture(conn: &Connection, id: &str, fields: &BTreeMap<String, Value>) -> Result<()> {
    let mut shadow = load(conn, id)?;
    let raw: String =
        conn.query_row("SELECT metadata_json FROM tracks WHERE id=?1", [id], |r| {
            r.get(0)
        })?;
    let metadata = metadata_fields(&serde_json::from_str::<Value>(&raw)?);
    for field in fields.keys() {
        if let Some(name) = field.strip_prefix("metadata.") {
            shadow.insert(field.clone(), native_presence(metadata.get(name)));
        }
    }
    if fields.contains_key("cues.hot") {
        shadow.insert(
            "_cue_revision".into(),
            cue_revision(conn, id)?.map_or(Value::Null, Value::String),
        );
    }
    conn.execute("INSERT INTO import_active_fields(item_id,field,value_json,provenance_json) VALUES(?1,'_native_shadow',?2,'{}') ON CONFLICT(item_id,field) DO UPDATE SET value_json=excluded.value_json,revision=revision+1",params![id,serde_json::to_string(&shadow)?])?;
    Ok(())
}
pub(super) fn native_presence(value: Option<&Value>) -> Value {
    value.map_or_else(
        || json!({"presence":"absent"}),
        |value| json!({"presence":"value","value":value}),
    )
}
pub(super) fn cue_revision(conn: &Connection, id: &str) -> Result<Option<String>> {
    Ok(conn.query_row("SELECT id FROM cue_revisions WHERE track_id=?1 ORDER BY created DESC,rowid DESC LIMIT 1",[id],|r|r.get(0)).optional()?)
}
pub(super) fn text(value: Option<&Value>) -> String {
    let value = value.and_then(|v| v.get("value"));
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("; "),
        _ => String::new(),
    }
}
/// metadata_json stores MediaInfo in prepared/edited tracks. Its technical
/// envelope is not a map of user tags; older local fixtures used a flat map.
pub(super) fn metadata_fields(value: &Value) -> BTreeMap<String, Value> {
    let object = if value.get("codec").is_some() && value.get("sample_rate").is_some() {
        value.get("tags").and_then(Value::as_object)
    } else {
        value.as_object()
    };
    object
        .into_iter()
        .flat_map(|m| m.iter())
        .filter(|(name, _)| {
            !matches!(
                name.to_ascii_lowercase().as_str(),
                "key" | "initialkey" | "bpm" | "tbpm"
            )
        })
        .map(|(name, value)| (name.to_ascii_lowercase(), value.clone()))
        .collect()
}
pub(super) fn capture_owned(conn: &Connection, id: &str) -> Result<()> {
    let raw: String =
        conn.query_row("SELECT metadata_json FROM tracks WHERE id=?1", [id], |r| {
            r.get(0)
        })?;
    let mut shadow = load(conn, id)?;
    shadow.insert("_imported_owner".into(), Value::Bool(true));
    for (name, value) in metadata_fields(&serde_json::from_str(&raw)?) {
        shadow.insert(format!("metadata.{name}"), native_presence(Some(&value)));
    }
    conn.execute(
        "UPDATE import_active_fields SET value_json=?1 WHERE item_id=?2 AND field='_native_shadow'",
        params![serde_json::to_string(&shadow)?, id],
    )?;
    Ok(())
}
