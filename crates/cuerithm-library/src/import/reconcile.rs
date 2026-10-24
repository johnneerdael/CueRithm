use super::projection::active_fields_tx;
use super::*;
use anyhow::Result;
use cuerithm_dj_sync::import::*;
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Both source and local baselines advance only for accepted selected fields.
pub(crate) fn choose_field(
    source: &Value,
    local: Option<&Value>,
    baseline: Option<&(Value, Value)>,
    decision: Option<&ImportDecision>,
) -> (Option<Value>, bool) {
    if let Some(decision) = decision {
        return match decision {
            ImportDecision::KeepLocal => (None, false),
            ImportDecision::UseSource => (Some(source.clone()), false),
            ImportDecision::Custom(value) => (Some(value.clone()), false),
        };
    }
    if local == Some(source) {
        return (Some(source.clone()), false);
    }
    match (baseline, local) {
        (Some((previous_source, previous_local)), current) => {
            if source == previous_source {
                (None, false)
            } else if current == Some(previous_local) {
                (Some(source.clone()), false)
            } else {
                (None, true)
            }
        }
        (None, None) => (Some(source.clone()), false),
        (None, Some(_)) => (None, true),
    }
}
/// Compare artist/genre aliases without adding derived fields to the accepted
/// projection. A source can change representation after an earlier import.
pub(crate) fn aliased_local_value(fields: &BTreeMap<String, Value>, field: &str) -> Option<Value> {
    if fields.contains_key(field) {
        return None;
    }
    let (alias, plural) = match field {
        "metadata.artist" => ("metadata.artists", false),
        "metadata.artists" => ("metadata.artist", true),
        "metadata.genre" => ("metadata.genres", false),
        "metadata.genres" => ("metadata.genre", true),
        _ => return None,
    };
    let mut value = fields.get(alias)?.clone();
    if let Some(raw) = value.get_mut("value") {
        let converted = match (&*raw, plural) {
            (Value::String(text), true) => Some(json!([text])),
            (Value::Array(values), false) => values
                .iter()
                .map(Value::as_str)
                .collect::<Option<Vec<_>>>()
                .map(|values| json!(values.join("; "))),
            _ => None,
        };
        if let Some(converted) = converted {
            *raw = converted;
        }
    }
    Some(value)
}
pub(crate) fn local_fields(conn: &Connection, id: &str) -> Result<BTreeMap<String, Value>> {
    let mut fields = active_fields_tx(conn, id)?;
    type NativeFields = (
        String,
        String,
        String,
        String,
        Option<String>,
        Option<f64>,
        Option<u8>,
        Option<f64>,
        String,
    );
    let row:Option<NativeFields>=conn.query_row("SELECT title,artist,album,genre,musical_key,bpm,energy,duration,metadata_json FROM tracks WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?))).optional()?;
    if let Some((title, artist, album, genre, key, bpm, energy, duration, metadata)) = row {
        let shadow = super::native_shadow::load(conn, id)?;
        let artist_imported =
            fields.contains_key("metadata.artist") || fields.contains_key("metadata.artists");
        let genre_imported =
            fields.contains_key("metadata.genre") || fields.contains_key("metadata.genres");
        for (name, value) in [
            ("title", title),
            ("artist", artist.clone()),
            ("album", album),
            ("genre", genre.clone()),
        ] {
            let field = format!("metadata.{name}");
            let alias = match name {
                "artist" => Some("metadata.artists"),
                "genre" => Some("metadata.genres"),
                _ => None,
            };
            if !fields.contains_key(&field)
                && alias.is_some_and(|alias| super::native_shadow::text(fields.get(alias)) == value)
            {
                continue;
            }
            if super::native_shadow::text(fields.get(&field)) != value
                || !value.is_empty() && !fields.contains_key(&field)
            {
                fields.insert(field, json!({"presence":"value","value":value}));
            }
        }
        for (name, value, imported) in [
            ("artists", artist, artist_imported),
            ("genres", genre, genre_imported),
        ] {
            let field = format!("metadata.{name}");
            if (fields.contains_key(&field) || !imported && !value.is_empty())
                && super::native_shadow::text(fields.get(&field)) != value
            {
                fields.insert(field, json!({"presence":"value","value":[value]}));
            }
        }
        let expected_key = fields.get("key").and_then(super::projection::key_text);
        if expected_key != key.as_deref() {
            fields.insert(
                "key".into(),
                serde_json::to_value(MusicalKey {
                    presence: if key.is_some() {
                        FieldPresence::Value
                    } else {
                        FieldPresence::Null
                    },
                    raw: key.clone(),
                    camelot: key,
                    ..Default::default()
                })?,
            );
        }
        let expected_bpm = fields
            .get("bpm")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_f64);
        if expected_bpm != bpm {
            fields.insert(
                "bpm".into(),
                serde_json::to_value(ScalarBpm {
                    presence: if bpm.is_some() {
                        FieldPresence::Value
                    } else {
                        FieldPresence::Null
                    },
                    value: bpm,
                    ..Default::default()
                })?,
            );
        }
        let expected_energy = fields
            .get("energy")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_u64)
            .filter(|v| *v <= 255);
        if expected_energy != energy.map(u64::from) {
            fields.insert(
                "energy".into(),
                energy.map_or_else(
                    || json!({"presence":"null"}),
                    |energy| json!({"presence":"value","value":energy}),
                ),
            );
        }
        let expected_duration = fields
            .get("metadata.duration")
            .and_then(super::projection::duration_value);
        if expected_duration != duration {
            fields.insert(
                "metadata.duration".into(),
                duration.map_or_else(
                    || json!({"presence":"null"}),
                    |duration| json!({"presence":"value","value":duration}),
                ),
            );
        }
        let metadata =
            super::native_shadow::metadata_fields(&serde_json::from_str::<Value>(&metadata)?);
        let names: std::collections::BTreeSet<String> = metadata
            .keys()
            .cloned()
            .chain(
                shadow
                    .keys()
                    .filter_map(|field| field.strip_prefix("metadata.").map(str::to_owned)),
            )
            .collect();
        for name in names {
            if matches!(
                name.as_str(),
                "title" | "artist" | "artists" | "album" | "genre" | "genres" | "duration"
            ) {
                continue;
            }
            let field = format!("metadata.{name}");
            let current = super::native_shadow::native_presence(metadata.get(&name));
            if (!fields.contains_key(&field)
                && shadow.get("_imported_owner") != Some(&Value::Bool(true)))
                || shadow
                    .get(&field)
                    .is_some_and(|previous| previous != &current)
            {
                fields.insert(field, current);
            }
        }
        let current_revision = super::native_shadow::cue_revision(conn, id)?;
        let expected_revision = shadow.get("_cue_revision");
        if expected_revision
            != Some(
                &current_revision
                    .as_ref()
                    .map_or(Value::Null, |id| json!(id)),
            )
        {
            let cues:Option<String>=conn.query_row("SELECT json FROM cue_revisions WHERE track_id=?1 ORDER BY created DESC,rowid DESC LIMIT 1",[id],|r|r.get(0)).optional()?;
            if let Some(cues) = cues {
                let value: Value = serde_json::from_str(&cues)?;
                fields.insert("cues.hot".into(), value);
            } else if !fields.contains_key("cues.hot") || expected_revision.is_some() {
                let analysis: Option<String> =
                    conn.query_row("SELECT analysis_json FROM tracks WHERE id=?1", [id], |r| {
                        r.get(0)
                    })?;
                if let Some(analysis) = analysis {
                    let analysis: Value = serde_json::from_str(&analysis)?;
                    if let Some(mut publication) = analysis
                        .get("publication")
                        .filter(|v| v.get("cues").is_some_and(Value::is_array))
                        .cloned()
                    {
                        publication["schema"] = json!("lexicon-publication-v1");
                        fields.insert("cues.hot".into(), publication);
                    }
                }
            }
        }
    }
    if super::performance::bound_unknown_occupancy(conn, id)? {
        fields.insert("_hotcue_occupancy".into(), json!("unknown"));
    }
    Ok(fields)
}
