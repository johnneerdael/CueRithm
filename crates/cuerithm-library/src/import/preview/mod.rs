mod apply;
mod candidates;
mod collections;
mod identity;
mod inspect;
use super::*;
use super::{
    collections::project_import_collection_tx,
    guard::MediaGuard,
    projection::{TrackProjection, field_group, project_import_track_tx, selected_source_values},
    reconcile::{choose_field, local_fields},
};
use crate::{
    Library,
    model::{bytes_path, now},
};
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Serialize, Deserialize)]
pub(super) struct PreviewMeta {
    pub(super) request: ImportPreviewRequest,
    pub(super) summary: ImportPreview,
    #[serde(default)]
    pub(super) processing: super::processing::FrozenProcessing,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Candidate {
    pub(super) external_id: String,
    pub(super) item_id: String,
    pub(super) local_track_id: Option<String>,
    pub(super) is_new: bool,
    pub(super) media: Option<MediaGuard>,
    pub(super) availability: String,
    pub(super) identity_change: Option<Value>,
    pub(super) identity_blocked: bool,
}
#[derive(Serialize, Deserialize)]
pub(super) struct TrackProposal {
    pub(super) candidate: Candidate,
    pub(super) fields: BTreeMap<String, Value>,
    pub(super) baselines: BTreeMap<String, (Value, Value)>,
    pub(super) provenance: BTreeMap<String, Value>,
    #[serde(default)]
    pub(super) process: bool,
}
#[derive(Serialize, Deserialize)]
struct CollectionProposal {
    id: String,
    collection: CollectionObservation,
    parent_id: Option<String>,
    apply: bool,
    inline: bool,
    observed: Value,
    accepted: Option<Value>,
    baseline_changed: bool,
}
fn digest(value: &impl Serialize) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}
fn revision(conn: &Connection) -> Result<u64> {
    Ok(
        conn.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
            r.get(0)
        })?,
    )
}
fn collection_id(source: &str, external: &str) -> Result<String> {
    Ok(format!(
        "import-collection-{}",
        digest(&(source, external))?
    ))
}
fn load_track(conn: &Connection, snapshot: &str, external: &str) -> Result<Box<TrackObservation>> {
    let raw:String=conn.query_row("SELECT json FROM import_observations WHERE snapshot_id=?1 AND kind='track' AND record_id=?2",params![snapshot,external],|r|r.get(0))?;
    match serde_json::from_str(&raw)? {
        ObservationRecord::Track(track) => Ok(track),
        _ => anyhow::bail!("Expected track observation"),
    }
}
fn readable(profile: &SourceProfile, field: &str) -> Result<bool> {
    let group = serde_json::to_value(field_group(field))?;
    Ok(profile
        .fields
        .get(field)
        .or_else(|| group.as_str().and_then(|g| profile.fields.get(g)))
        .is_some_and(|c| c.read))
}
fn baseline(
    conn: &Connection,
    source: &str,
    external: &str,
    field: &str,
) -> Result<Option<(Value, Value)>> {
    let raw:Option<(String,String)>=conn.query_row("SELECT source_json,local_json FROM import_applied_baselines WHERE source_id=?1 AND external_id=?2 AND field=?3",params![source,external,field],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    raw.map(|(s, l)| Ok((serde_json::from_str(&s)?, serde_json::from_str(&l)?)))
        .transpose()
}
fn decision<'a>(
    request: &'a ImportPreviewRequest,
    external: &str,
    field: &str,
) -> Option<&'a ImportDecision> {
    request
        .decisions
        .iter()
        .find(|d| d.external_id == external && d.field == field)
        .map(|d| &d.decision)
}
fn occupied(value: Option<&Value>) -> bool {
    value.is_some_and(|v| !v.is_null() && !v.as_array().is_some_and(Vec::is_empty))
}
impl Library {
    fn guarded_import_snapshot(&self, id: &str) -> Result<(Arc<ImportSnapshot>, String)> {
        let runtime = self
            .inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?;
        let snapshot = runtime
            .inspections
            .get(id)
            .context("Import inspection expired; inspect the source again")?
            .clone();
        let nonce: String = self.conn()?.query_row(
            "SELECT runtime_nonce FROM import_snapshots WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        ensure!(
            nonce == runtime.nonce,
            "Import inspection belongs to an earlier runtime"
        );
        Ok((snapshot, nonce))
    }
}
fn validate_custom_field(field: &str, value: &Value) -> Result<()> {
    ensure!(
        serde_json::to_vec(value)?.len() as u64 <= ImportLimits::default().text_bytes,
        "Import field exceeds byte limit"
    );
    match field {
        "key" => {
            let key: MusicalKey = serde_json::from_value(value.clone())?;
            let track = TrackObservation {
                external_id: "validated-field".into(),
                key: Some(key),
                ..Default::default()
            };
            validate_track_observation(&track, &ImportLimits::default())?;
        }
        "bpm" => {
            let bpm: ScalarBpm = serde_json::from_value(value.clone())?;
            let track = TrackObservation {
                external_id: "validated-field".into(),
                bpm: Some(bpm),
                ..Default::default()
            };
            validate_track_observation(&track, &ImportLimits::default())?;
        }
        "energy" => {
            let _: Presence<Value> = serde_json::from_value(value.clone())?;
        }
        "beatgrid" => {
            let grid: GridObservation = serde_json::from_value(value.clone())?;
            let track = TrackObservation {
                external_id: "validated-field".into(),
                grid: Some(grid),
                ..Default::default()
            };
            validate_track_observation(&track, &ImportLimits::default())?;
        }
        _ if field.starts_with("cues.") => {
            let cues: Vec<CueObservation> = serde_json::from_value(value.clone())?;
            ensure!(
                cues.iter()
                    .all(|c| super::projection::cue_field(c.family) == field),
                "Custom cue family mismatch"
            );
            let track = TrackObservation {
                external_id: "validated-field".into(),
                cues,
                ..Default::default()
            };
            validate_track_observation(&track, &ImportLimits::default())?;
        }
        _ if field.starts_with("metadata.") => {
            let _: Presence<Value> = serde_json::from_value(value.clone())?;
        }
        _ => {}
    }
    Ok(())
}

fn identity_change(
    conn: &Connection,
    source: &str,
    track: &TrackObservation,
) -> Result<Option<Value>> {
    let previous: Option<String> = conn
        .query_row(
            "SELECT identity_json FROM import_bindings WHERE source_id=?1 AND external_id=?2",
            params![source, track.external_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(previous) = previous else {
        return Ok(None);
    };
    let previous: Value = serde_json::from_str(&previous)?;
    let before: LocationEvidence = serde_json::from_value(
        previous
            .get("location")
            .cloned()
            .context("Missing source identity evidence")?,
    )?;
    let location_changed = before.original != track.locations.original
        || before.native_path != track.locations.native_path;
    let native_changed = if let Some(snapshot) = previous.get("snapshot_id").and_then(Value::as_str)
    {
        let old = load_track(conn, snapshot, &track.external_id)?;
        old.extensions
            .get("native_identity")
            .zip(track.extensions.get("native_identity"))
            .is_some_and(|(old, new)| old != new)
    } else {
        false
    };
    Ok((location_changed || native_changed).then_some(previous))
}

fn processing_required(
    conn: &Connection,
    source: &str,
    policy: &ImportPolicy,
    processing: &super::processing::FrozenProcessing,
    candidate: &Candidate,
    fields_changed: bool,
) -> Result<bool> {
    if !policy.requires_processing() || candidate.identity_blocked {
        return Ok(false);
    }
    if candidate.is_new || fields_changed || candidate.identity_change.is_some() {
        return Ok(true);
    }
    let previous:Option<(String,String,String)>=conn.query_row("SELECT p.json,e.proposal_json,o.id FROM import_operations o JOIN import_previews p ON p.id=o.preview_id JOIN import_preview_effects e ON e.preview_id=p.id WHERE o.source_id=?1 AND e.kind='track' AND json_extract(e.proposal_json,'$.candidate.item_id')=?2 ORDER BY o.rowid DESC LIMIT 1",params![source,candidate.item_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let Some((meta, proposal, operation)) = previous else {
        return Ok(true);
    };
    let previous: PreviewMeta = serde_json::from_str(&meta)?;
    let proposal: TrackProposal = serde_json::from_str(&proposal)?;
    let previous_decoder = if let Some(decoder) = &previous.processing.decoder_identity {
        Some(decoder.clone())
    } else {
        conn.query_row("SELECT json_extract(identity_json,'$.decoder') FROM import_processing_receipts WHERE operation_id=?1 AND state='proposed' ORDER BY rowid DESC LIMIT 1",[operation],|r|r.get::<_,Option<String>>(0)).optional()?.flatten()
    };
    let decoder_changed = previous_decoder
        .as_ref()
        .zip(processing.decoder_identity.as_ref())
        .is_some_and(|(before, after)| before != after);
    Ok(decoder_changed
        || serde_json::to_value(&previous.request.policy)? != serde_json::to_value(policy)?
        || previous.processing.effective_identity(policy)?
            != processing.effective_identity(policy)?
        || proposal.candidate.media != candidate.media)
}
