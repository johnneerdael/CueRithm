use super::*;
use crate::{
    Library, TrackSummary,
    model::{bytes_path, now, path_bytes},
};
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::import::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

pub(crate) const LIBRARY_TRACKS_CTE: &str = r#"library_tracks AS (
 SELECT id,path,title,artist,album,genre,state,musical_key,bpm,energy,duration,source_hash,metadata_json,created,updated,archived,tagging_json,sort_title,sort_artist,search_text FROM tracks
 UNION ALL
 SELECT i.id,i.path,COALESCE(json_extract(s.value_json,'$.title'),'Imported item'),COALESCE(json_extract(s.value_json,'$.artist'),''),COALESCE(json_extract(s.value_json,'$.album'),''),COALESCE(json_extract(s.value_json,'$.genre'),''),'imported',json_extract(s.value_json,'$.key'),json_extract(s.value_json,'$.bpm'),json_extract(s.value_json,'$.energy'),json_extract(s.value_json,'$.duration'),COALESCE(i.source_hash,''),'{}',i.created,i.updated,COALESCE(json_extract(s.value_json,'$.archived'),0),NULL,COALESCE(json_extract(s.value_json,'$.sort_title'),''),COALESCE(json_extract(s.value_json,'$.sort_artist'),''),COALESCE(json_extract(s.value_json,'$.search_text'),'')
 FROM import_items i LEFT JOIN import_active_fields s ON s.item_id=i.id AND s.field='_summary' WHERE i.local_track_id IS NULL
)"#;
fn metadata_group(field: &str) -> FieldGroup {
    match field {
        "rating" | "user_rating" => FieldGroup::Ratings,
        "color" => FieldGroup::Color,
        "play_count" | "last_played" | "history" => FieldGroup::History,
        "title" | "artist" | "artists" | "album" | "album_artist" | "genre" | "genres"
        | "comment" | "label" | "remixer" | "mix" | "composer" | "lyricist" | "grouping"
        | "year" | "track_number" | "track_total" | "disc_number" | "disc_total" | "isrc"
        | "release_date" => FieldGroup::DescriptiveTags,
        _ => FieldGroup::CustomTags,
    }
}
pub(crate) fn cue_field(family: CueFamily) -> &'static str {
    match family {
        CueFamily::Hot => "cues.hot",
        CueFamily::Memory => "cues.memory",
        CueFamily::Main => "cues.main",
        CueFamily::Load => "cues.load",
        CueFamily::Loop => "cues.loop",
        CueFamily::Intro => "cues.intro",
        CueFamily::Outro => "cues.outro",
        CueFamily::AudibleRange => "cues.audible_range",
        CueFamily::Unknown => "cues.unknown",
        CueFamily::Marker => "cues.marker",
    }
}
pub(crate) fn field_group(field: &str) -> FieldGroup {
    if let Some(name) = field.strip_prefix("metadata.") {
        return metadata_group(name);
    }
    match field {
        "key" => FieldGroup::Key,
        "bpm" => FieldGroup::Bpm,
        "energy" => FieldGroup::Energy,
        "beatgrid" => FieldGroup::Beatgrid,
        "cues.hot" => FieldGroup::HotCues,
        "cues.memory" => FieldGroup::MemoryCues,
        "cues.main" => FieldGroup::MainCues,
        "cues.load" => FieldGroup::LoadCues,
        "cues.loop" => FieldGroup::Loops,
        "artwork" => FieldGroup::Artwork,
        _ => FieldGroup::Markers,
    }
}
pub(crate) fn selected_source_values(
    conn: &Connection,
    track: &TrackObservation,
    policy: &ImportPolicy,
) -> Result<BTreeMap<String, Value>> {
    let mut fields = BTreeMap::new();
    let mut metadata = track.metadata.clone();
    let mut key = track.key.clone();
    let mut bpm = track.bpm.clone();
    if policy.capture_file_tags
        && let Some(reference) = &track.file_tags
    {
        let blob: Option<Vec<u8>> = conn
            .query_row(
                "SELECT data FROM import_blobs WHERE sha256=?1",
                [&reference.sha256],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(bytes) = blob
            && let Ok(tags) = serde_json::from_slice::<FileTagObservation>(&bytes)
        {
            if key
                .as_ref()
                .is_none_or(|key| key.presence == FieldPresence::Absent)
                && let Some(raw) = tags.normalized.get("key").and_then(|values| values.first())
            {
                key = Some(MusicalKey {
                    presence: if raw.is_empty() {
                        FieldPresence::Empty
                    } else {
                        FieldPresence::Value
                    },
                    raw: Some(raw.clone()),
                    camelot: camelot_core::MusicalKey::from_camelot(
                        &raw.trim().to_ascii_uppercase(),
                    )
                    .ok()
                    .map(camelot_core::MusicalKey::camelot),
                    provenance: Some("file_tags".into()),
                    ..Default::default()
                });
            }
            if bpm
                .as_ref()
                .is_none_or(|bpm| bpm.presence == FieldPresence::Absent)
                && let Some(raw) = tags.normalized.get("bpm").and_then(|values| values.first())
            {
                if raw.is_empty() {
                    bpm = Some(ScalarBpm {
                        presence: FieldPresence::Empty,
                        provenance: Some("file_tags".into()),
                        ..Default::default()
                    });
                } else if let Ok(value) = raw.trim().parse::<f64>()
                    && value.is_finite()
                    && value > 0.0
                {
                    bpm = Some(ScalarBpm {
                        presence: FieldPresence::Value,
                        value: Some(value),
                        provenance: Some("file_tags".into()),
                        extensions: BTreeMap::from([("raw".into(), json!(raw))]),
                        ..Default::default()
                    });
                }
            }
            for (field, values) in tags.normalized {
                if matches!(field.as_str(), "key" | "bpm") {
                    continue;
                }
                if metadata
                    .get(&field)
                    .is_none_or(|value| matches!(value, Presence::Absent))
                {
                    let value = if matches!(field.as_str(), "artists" | "genres") {
                        json!(values)
                    } else {
                        values.first().map_or(Value::Null, |s| json!(s))
                    };
                    metadata.insert(field, Presence::Value(value));
                }
            }
            if policy.selected_fields.contains(&FieldGroup::Artwork) {
                fields.insert("artwork".into(), serde_json::to_value(tags.artwork)?);
            }
        }
    }
    for (field, value) in &metadata {
        let group = metadata_group(field);
        if !policy.selected_fields.contains(&group) || matches!(value, Presence::Absent) {
            continue;
        }
        if group == FieldGroup::DescriptiveTags
            && policy
                .descriptive_fields
                .as_ref()
                .is_some_and(|selected| !selected.contains(field))
        {
            continue;
        }
        fields.insert(format!("metadata.{field}"), serde_json::to_value(value)?);
    }
    if policy.selected_fields.contains(&FieldGroup::Key)
        && let Some(value) = &key
        && value.presence != FieldPresence::Absent
    {
        fields.insert("key".into(), serde_json::to_value(value)?);
    }
    if policy.selected_fields.contains(&FieldGroup::Bpm)
        && let Some(value) = &bpm
        && value.presence != FieldPresence::Absent
    {
        fields.insert("bpm".into(), serde_json::to_value(value)?);
    }
    if policy.selected_fields.contains(&FieldGroup::Energy)
        && !matches!(track.energy, Presence::Absent)
    {
        fields.insert("energy".into(), serde_json::to_value(&track.energy)?);
    }
    if policy.selected_fields.contains(&FieldGroup::Beatgrid)
        && let Some(value) = &track.grid
    {
        fields.insert("beatgrid".into(), serde_json::to_value(value)?);
    }
    for family in [
        CueFamily::Hot,
        CueFamily::Memory,
        CueFamily::Main,
        CueFamily::Load,
        CueFamily::Loop,
        CueFamily::Marker,
        CueFamily::Intro,
        CueFamily::Outro,
        CueFamily::AudibleRange,
        CueFamily::Unknown,
    ] {
        let field = cue_field(family);
        if policy.selected_fields.contains(&field_group(field))
            && !track
                .issues
                .iter()
                .any(|i| i.field.as_deref() == Some(field))
        {
            fields.insert(
                field.into(),
                serde_json::to_value(
                    track
                        .cues
                        .iter()
                        .filter(|c| c.family == family)
                        .collect::<Vec<_>>(),
                )?,
            );
        }
    }
    fields.retain(|field, _| {
        !track.issues.iter().any(|issue| {
            issue.field.as_deref().is_some_and(|failed| {
                failed == field
                    || field.strip_prefix("metadata.") == Some(failed)
                    || field.strip_prefix("cues.").is_some() && failed == "cues"
            })
        })
    });
    Ok(fields)
}
fn present_value(value: &Value) -> Option<&Value> {
    match value.get("presence").and_then(Value::as_str) {
        Some("value" | "empty") => value.get("value"),
        Some("null" | "absent" | "deleted") => None,
        _ => Some(value),
    }
}
fn text(value: Option<&Value>) -> String {
    match value.and_then(present_value) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("; "),
        _ => String::new(),
    }
}
pub(crate) fn active_fields_tx(conn: &Connection, id: &str) -> Result<BTreeMap<String, Value>> {
    let mut query=conn.prepare("SELECT field,value_json FROM import_active_fields WHERE item_id=?1 AND field NOT LIKE '\\_%' ESCAPE '\\' ORDER BY field")?;
    query
        .query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .map(|row| {
            let (field, value) = row?;
            Ok((field, serde_json::from_str(&value)?))
        })
        .collect()
}
fn cached_summary(conn: &Connection, id: &str) -> Result<Value> {
    let fields = active_fields_tx(conn, id)?;
    let title = text(fields.get("metadata.title"));
    let artist = text(
        fields
            .get("metadata.artists")
            .or_else(|| fields.get("metadata.artist")),
    );
    let album = text(fields.get("metadata.album"));
    let genre = text(
        fields
            .get("metadata.genre")
            .or_else(|| fields.get("metadata.genres")),
    );
    let key = fields
        .get("key")
        .and_then(|k| k.get("camelot").or_else(|| k.get("raw")))
        .and_then(Value::as_str);
    let bpm = fields
        .get("bpm")
        .and_then(|v| v.get("value"))
        .and_then(Value::as_f64);
    let energy = fields
        .get("energy")
        .and_then(present_value)
        .and_then(Value::as_u64)
        .filter(|e| *e <= 255);
    let archived = fields
        .get("library.archived")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let duration = fields
        .get("metadata.duration")
        .and_then(present_value)
        .and_then(Value::as_f64);
    let metadata: BTreeMap<_, _> = fields
        .iter()
        .filter_map(|(k, v)| {
            k.strip_prefix("metadata.").map(|k| {
                (
                    k.to_owned(),
                    present_value(v).cloned().unwrap_or(Value::Null),
                )
            })
        })
        .collect();
    Ok(
        json!({"title":if title.is_empty(){"Imported item"}else{&title},"artist":artist,"album":album,"genre":genre,"key":key,"bpm":bpm,"energy":energy,"duration":duration,"archived":archived,"metadata":metadata,"sort_title":title.to_lowercase(),"sort_artist":artist.to_lowercase(),"search_text":format!("{title} {artist} {album} {genre}").to_lowercase(),"analysis_state":"unanalysed"}),
    )
}
pub(crate) fn rebuild_summary_tx(conn: &Connection, id: &str) -> Result<()> {
    let value = cached_summary(conn, id)?;
    conn.execute("INSERT INTO import_active_fields(item_id,field,value_json,provenance_json) VALUES(?1,'_summary',?2,'{}') ON CONFLICT(item_id,field) DO UPDATE SET value_json=excluded.value_json,revision=revision+1",params![id,serde_json::to_string(&value)?])?;
    Ok(())
}
pub(crate) struct TrackProjection<'a> {
    pub source_id: &'a str,
    pub snapshot_id: &'a str,
    pub observation: &'a TrackObservation,
    pub item_id: &'a str,
    pub fields: &'a BTreeMap<String, Value>,
    pub local_track_id: Option<&'a str>,
    pub availability: Option<&'a str>,
    pub resolved_path: Option<&'a Path>,
}
pub(crate) fn project_import_track_tx(conn: &Connection, p: TrackProjection<'_>) -> Result<()> {
    ensure!(
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_snapshots WHERE id=?1 AND source_id=?2)",
            params![p.snapshot_id, p.source_id],
            |r| r.get::<_, bool>(0)
        )?,
        "Projection snapshot/source mismatch"
    );
    ensure!(
        p.local_track_id.is_none_or(|id| id == p.item_id),
        "Existing track identity must remain stable"
    );
    let availability = if let Some(availability) = p.availability {
        availability
    } else if let Some(path) = p.resolved_path {
        if path.is_file() {
            "available"
        } else {
            "missing"
        }
    } else {
        p.observation.locations.availability.as_str()
    };
    let availability = if availability.is_empty() {
        "not_probed"
    } else {
        availability
    };
    let path = p.resolved_path.map(path_bytes);
    conn.execute("INSERT INTO import_items(id,local_track_id,availability,path,created,updated) VALUES(?1,?2,?3,?4,?5,?5) ON CONFLICT(id) DO UPDATE SET availability=excluded.availability,path=COALESCE(excluded.path,import_items.path),updated=excluded.updated,revision=import_items.revision+1",params![p.item_id,p.local_track_id,availability,path,now()])?;
    let existing: Option<String> = conn
        .query_row(
            "SELECT item_id FROM import_bindings WHERE source_id=?1 AND external_id=?2",
            params![p.source_id, p.observation.external_id],
            |r| r.get(0),
        )
        .optional()?;
    ensure!(
        existing.as_deref().is_none_or(|id| id == p.item_id),
        "Source binding changed after preview"
    );
    conn.execute("INSERT INTO import_bindings(source_id,external_id,item_id,identity_json,observed_hash) VALUES(?1,?2,?3,?4,NULL) ON CONFLICT(source_id,external_id) DO UPDATE SET identity_json=excluded.identity_json",params![p.source_id,p.observation.external_id,p.item_id,json!({"snapshot_id":p.snapshot_id,"location":p.observation.locations}).to_string()])?;
    retain_legacy_rekordbox_binding(conn, &p)?;
    conn.execute("UPDATE import_memberships SET item_id=?1 WHERE item_id IS NULL AND external_track_id=?2 AND collection_id IN (SELECT id FROM import_collections WHERE source_id=?3)",params![p.item_id,p.observation.external_id,p.source_id])?;
    let provenance = json!({"kind":"source","source_id":p.source_id,"external_id":p.observation.external_id,"snapshot_id":p.snapshot_id});
    for (field, value) in p.fields {
        ensure!(!field.starts_with('_'), "Reserved import field");
        conn.execute("INSERT INTO import_active_fields(item_id,field,value_json,provenance_json) VALUES(?1,?2,?3,?4) ON CONFLICT(item_id,field) DO UPDATE SET value_json=excluded.value_json,provenance_json=excluded.provenance_json,revision=revision+1",params![p.item_id,field,serde_json::to_string(value)?,provenance.to_string()])?;
        if let Some(id) = p.local_track_id {
            let column = match field.as_str() {
                "metadata.title" => Some("title"),
                "metadata.artists" | "metadata.artist" => Some("artist"),
                "metadata.album" => Some("album"),
                "metadata.genre" | "metadata.genres" => Some("genre"),
                _ => None,
            };
            if let Some(column) = column {
                conn.execute(
                    &format!("UPDATE tracks SET {column}=?1,updated=?2 WHERE id=?3"),
                    params![text(Some(value)), now(), id],
                )?;
            }
            if field == "key" {
                conn.execute(
                    "UPDATE tracks SET musical_key=?1,updated=?2 WHERE id=?3",
                    params![
                        value
                            .get("camelot")
                            .or_else(|| value.get("raw"))
                            .and_then(Value::as_str),
                        now(),
                        id
                    ],
                )?;
            }
            if field == "bpm" {
                conn.execute(
                    "UPDATE tracks SET bpm=?1,updated=?2 WHERE id=?3",
                    params![value.get("value").and_then(Value::as_f64), now(), id],
                )?;
            }
            if field == "energy" {
                conn.execute(
                    "UPDATE tracks SET energy=?1,updated=?2 WHERE id=?3",
                    params![
                        present_value(value)
                            .and_then(Value::as_u64)
                            .filter(|e| *e <= 255),
                        now(),
                        id
                    ],
                )?;
            }
        }
    }
    if let Some(id) = p.local_track_id {
        let (title, artist, album, genre): (String, String, String, String) = conn.query_row(
            "SELECT title,artist,album,genre FROM tracks WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        conn.execute(
            "UPDATE tracks SET sort_title=?1,sort_artist=?2,search_text=?3 WHERE id=?4",
            params![
                title.to_lowercase(),
                artist.to_lowercase(),
                format!("{title} {artist} {album} {genre}").to_lowercase(),
                id
            ],
        )?;
    }
    if let Some(id) = p.local_track_id
        && !p.fields.is_empty()
    {
        super::native_shadow::capture(conn, id, p.fields)?;
    }
    rebuild_summary_tx(conn, p.item_id)?;
    Ok(())
}
pub(crate) fn decorate_summary(conn: &Connection, row: &mut TrackSummary) -> Result<()> {
    let data:Option<(String,Option<Vec<u8>>,String)>=conn.query_row("SELECT i.availability,i.path,COALESCE(s.value_json,'{}') FROM import_items i LEFT JOIN import_active_fields s ON s.item_id=i.id AND s.field='_summary' WHERE i.id=?1",[&row.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    if let Some((availability, path, summary)) = data {
        let summary: Value = serde_json::from_str(&summary)?;
        let mut query = conn.prepare(
            "SELECT DISTINCT source_id FROM import_bindings WHERE item_id=?1 ORDER BY source_id",
        )?;
        let sources = query
            .query_map([&row.id], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        let fields = super::reconcile::local_fields(conn, &row.id)?;
        for (field, value) in fields {
            if let Some(name) = field.strip_prefix("metadata.") {
                if let Some(value) = present_value(&value) {
                    row.metadata.insert(name.into(), value.clone());
                } else {
                    row.metadata.remove(name);
                }
            }
        }
        row.import_state = Some(ImportedItemState {
            source_ids: sources,
            playback_allowed: availability == "available" && path.is_some(),
            availability,
            analysis_state: summary
                .get("analysis_state")
                .and_then(Value::as_str)
                .unwrap_or("unanalysed")
                .into(),
        });
    }
    Ok(())
}
impl Library {
    pub fn imported_item(&self, id: &str) -> Result<ImportedItemDetail> {
        let conn = self.conn()?;
        let availability: String = conn
            .query_row(
                "SELECT availability FROM import_items WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .context("Unknown imported item")?;
        let stored_fields = active_fields_tx(&conn, id)?;
        let fields = super::reconcile::local_fields(&conn, id)?;
        let mut query=conn.prepare("SELECT field,provenance_json FROM import_active_fields WHERE item_id=?1 AND field NOT LIKE '\\_%' ESCAPE '\\' ORDER BY field")?;
        let mut provenance = query
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .map(|r| {
                let (k, v) = r?;
                Ok((k, serde_json::from_str(&v)?))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        for (field, value) in &fields {
            if stored_fields.get(field) != Some(value) {
                provenance.insert(
                    field.clone(),
                    json!({"kind":"local","surface":"existing_library"}),
                );
            }
        }
        let mut links=conn.prepare("SELECT source_id,external_id FROM import_bindings WHERE item_id=?1 ORDER BY source_id,external_id")?;
        let links = links
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut observations = Vec::new();
        for (source, external) in links {
            let raw:Option<(String,String)>=conn.query_row("SELECT s.id,o.json FROM import_snapshots s JOIN import_observations o ON o.snapshot_id=s.id WHERE s.source_id=?1 AND o.kind='track' AND o.record_id=?2 ORDER BY s.created DESC,s.rowid DESC LIMIT 1",params![source,external],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
            if let Some((snapshot_id, raw)) = raw
                && let ObservationRecord::Track(track) = serde_json::from_str(&raw)?
            {
                observations.push(ImportedTrackObservation {
                    source_id: source,
                    external_id: external,
                    snapshot_id,
                    track: *track,
                });
            }
        }
        let path: Option<Vec<u8>> =
            conn.query_row("SELECT path FROM import_items WHERE id=?1", [id], |r| {
                r.get(0)
            })?;
        Ok(ImportedItemDetail {
            id: id.into(),
            active_fields: fields,
            provenance,
            observations,
            playback_allowed: availability == "available" && path.is_some(),
            availability,
            analysis_state: "unanalysed".into(),
        })
    }
    pub fn import_export_readiness(&self, id: &str) -> Result<ImportExportReadiness> {
        let conn = self.conn()?;
        let local: Option<Option<String>> = conn
            .query_row(
                "SELECT local_track_id FROM import_items WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        ensure!(local.is_some(), "Unknown imported item");
        let prepared = if let Some(Some(local)) = local {
            conn.query_row("SELECT state='ready' AND analysis_json IS NOT NULL AND archived=0 FROM tracks WHERE id=?1",[local],|r|r.get::<_,bool>(0)).optional()?.unwrap_or(false)
        } else {
            false
        };
        let mut reasons = if prepared {
            Vec::new()
        } else {
            vec!["This imported item has no qualified prepared-media result; resolve and explicitly prepare required export data".into()]
        };
        let fields = active_fields_tx(&conn, id)?;
        for (field, value) in &fields {
            if !(field.starts_with("cues.") || field == "beatgrid")
                || value.is_null()
                || value.as_array().is_some_and(Vec::is_empty)
            {
                continue;
            }
            let provenance: Value = serde_json::from_str(&conn.query_row(
                "SELECT provenance_json FROM import_active_fields WHERE item_id=?1 AND field=?2",
                params![id, field],
                |r| r.get::<_, String>(0),
            )?)?;
            let measured = provenance.get("kind").and_then(Value::as_str) == Some("processing")
                && provenance
                    .get("identity")
                    .and_then(|i| i.get("pcm_sha256"))
                    .and_then(Value::as_str)
                    .is_some();
            if !measured {
                reasons.push(format!(
                    "Active imported {field} has no backend-qualified clock for the current media"
                ));
            }
        }
        Ok(ImportExportReadiness {
            ready: reasons.is_empty(),
            reasons,
        })
    }
}
pub(crate) fn imported_path(conn: &Connection, id: &str) -> Result<Option<std::path::PathBuf>> {
    let bytes: Option<Vec<u8>> = conn
        .query_row("SELECT path FROM import_items WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    bytes.map(|b| bytes_path(&b)).transpose()
}

pub(crate) fn track_summary_tx(conn: &Connection, id: &str) -> Result<TrackSummary> {
    let mut row=conn.query_row(&format!("WITH {LIBRARY_TRACKS_CTE} SELECT id,path,title,artist,album,genre,state,musical_key,bpm,energy,duration,source_hash,metadata_json,created,updated,archived,json_extract(tagging_json,'$.status') FROM library_tracks WHERE id=?1"),[id],crate::store::summary_row)?;
    crate::curation::project_summary(conn, &mut row)?;
    decorate_summary(conn, &mut row)?;
    Ok(row)
}

impl Library {
    pub(crate) fn archive_imported_reference(&self, id: &str, archived: bool) -> Result<bool> {
        let mut conn = self.conn()?;
        let imported_only: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_items WHERE id=?1 AND local_track_id IS NULL)",
            [id],
            |r| r.get(0),
        )?;
        if !imported_only {
            return Ok(false);
        }
        let fields = active_fields_tx(&conn, id)?;
        if fields
            .get("library.archived")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            == archived
        {
            return Ok(true);
        }
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO import_active_fields(item_id,field,value_json,provenance_json) VALUES(?1,'library.archived',?2,'{\"kind\":\"local\"}') ON CONFLICT(item_id,field) DO UPDATE SET value_json=excluded.value_json,provenance_json=excluded.provenance_json,revision=revision+1",params![id,serde_json::to_string(&archived)?])?;
        tx.execute(
            "UPDATE import_items SET revision=revision+1,updated=?1 WHERE id=?2",
            params![now(), id],
        )?;
        rebuild_summary_tx(&tx, id)?;
        Library::bump(&tx)?;
        tx.commit()?;
        Ok(true)
    }
}

fn retain_legacy_rekordbox_binding(conn: &Connection, p: &TrackProjection<'_>) -> Result<()> {
    let Some(local) = p.local_track_id else {
        return Ok(());
    };
    let raw: String = conn.query_row(
        "SELECT header_json FROM import_snapshots WHERE id=?1",
        [p.snapshot_id],
        |r| r.get(0),
    )?;
    let header: ObservationHeader = serde_json::from_str(&raw)?;
    if header.source.provider != SourceProvider::Rekordbox
        || p.observation
            .extensions
            .get("native_deleted")
            .and_then(Value::as_i64)
            != Some(0)
    {
        return Ok(());
    }
    let Some(identity) = header.extensions.get("native_database_identity") else {
        return Ok(());
    };
    let Some(database) = identity["DBID"].as_str() else {
        return Ok(());
    };
    let device = identity["DeviceID"].as_str().unwrap_or("");
    let Some(track) = p.observation.extensions.get("native_identity") else {
        return Ok(());
    };
    let Some(uuid) = track["uuid"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let Some(master) = track["master_song_id"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let target:Option<String>=conn.query_row("SELECT t.id FROM native_targets t JOIN native_track_links l ON l.target_id=t.id WHERE t.provider='rekordbox' AND t.database_id=?1 AND t.device_id=?2 AND l.content_id=?3 AND l.local_track_id=?4 AND json_extract(l.identity_json,'$.database_id')=?1 AND json_extract(l.identity_json,'$.content_id')=?3 AND json_extract(l.identity_json,'$.uuid')=?5 AND json_extract(l.identity_json,'$.master_song_id')=?6",params![database,device,p.observation.external_id,local,uuid,master],|r|r.get(0)).optional()?;
    if let Some(target) = target {
        ensure!(conn.execute("UPDATE import_bindings SET legacy_target_id=?3,legacy_content_id=?2 WHERE source_id=?1 AND external_id=?2 AND (legacy_target_id IS NULL OR legacy_target_id=?3)",params![p.source_id,p.observation.external_id,target])?==1,"Existing native import binding differs from the selected source");
    }
    Ok(())
}
