//! Actual effects are reviewed separately from proposal generation. The existing
//! metadata and organization journals own every native file mutation.
mod files;
use super::{guard::MediaGuard, preview::PreviewMeta, *};
use crate::{
    Library,
    model::{now, path_bytes},
};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEffectsPreview {
    pub preview_id: String,
    pub operation_id: String,
    pub ready: u64,
    pub blocked: u64,
    pub unchanged: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEffect {
    pub item_id: String,
    pub stage: String,
    pub outcome: String,
    pub reason: Option<String>,
    pub changes: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportEffectsPage {
    pub records: Vec<ImportEffect>,
    pub next_cursor: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct EffectRow {
    pub item: String,
    pub stage: String,
    pub attempt: String,
    pub before: BTreeMap<String, Value>,
    #[serde(default)]
    pub before_provenance: BTreeMap<String, Value>,
    pub values: BTreeMap<String, Value>,
    pub provenance: BTreeMap<String, Value>,
    pub media: Option<MediaGuard>,
    #[serde(default)]
    pub decoder: Option<String>,
    #[serde(default)]
    pub relocation_copy: bool,
    pub outcome: String,
    pub reason: Option<String>,
    pub tag_preview: Option<String>,
    pub tag_draft: Option<String>,
    pub organization: Option<(String, String)>,
}
#[derive(Default, Clone, Serialize, Deserialize)]
pub(super) struct EffectJournal {
    #[serde(default)]
    pub last_error: Option<String>,
    pub local_applied: bool,
    pub tag_batch: Option<String>,
    pub organization_batch: Option<String>,
    #[serde(default)]
    pub organization_preview: Option<(String, String)>,
    #[serde(default)]
    pub undo_organization_batch: Option<String>,
    pub undo_tag_batch: Option<String>,
    pub undone: bool,
}
fn encode(value: &impl Serialize) -> Result<String> {
    let s = serde_json::to_string(value)?;
    ensure!(
        s.len() as u64 <= cuerithm_dj_sync::import::ImportLimits::default().text_bytes,
        "Import effect exceeds record byte limit"
    );
    Ok(s)
}
impl Library {
    pub(super) fn import_media_runtime(&self) -> Result<cuerithm_media::MediaRuntime> {
        self.inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
            .media_runtime
            .clone()
            .context("Start the import processing service with the bundled media runtime")
    }
    pub fn preview_import_effects(
        &self,
        operation: &str,
        cancel: &CancellationToken,
    ) -> Result<ImportEffectsPreview> {
        cancel.check()?;
        let gate = self
            .inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
            .processing_gate
            .clone();
        let _gate = gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cancel.check()?;
        let (source,snapshot,policy,meta):(String,String,String,String)=self.conn()?.query_row("SELECT o.source_id,p.snapshot_id,o.policy_id,p.json FROM import_operations o JOIN import_previews p ON p.id=o.preview_id WHERE o.id=?1",[operation],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let meta: PreviewMeta = serde_json::from_str(&meta)?;
        ensure!(!self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM import_processing_receipts WHERE operation_id=?1 AND stage='publication.effects')",[operation],|r|r.get::<_,bool>(0))?,"Effects already published or require recovery; completed fields cannot be replayed");
        ensure!(!self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM import_processing_receipts WHERE operation_id=?1 AND state='running') OR EXISTS(SELECT 1 FROM import_operations WHERE id=?1 AND state IN ('processing_running','processing_pending'))",[operation],|r|r.get::<_,bool>(0))?,"Processing is still running or pending; complete or retry generation before effects review");
        let id = uuid::Uuid::new_v4().to_string();
        let mut summary = ImportEffectsPreview {
            preview_id: id.clone(),
            operation_id: operation.into(),
            ready: 0,
            blocked: 0,
            unchanged: 0,
        };
        let nonce = self
            .inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
            .nonce
            .clone();
        self.conn()?.execute(
            "INSERT INTO import_previews VALUES(?1,?2,?3,0,?4,'{}',?5,'building',?6)",
            params![id, source, snapshot, policy, nonce, now()],
        )?;
        let mut after = 0i64;
        let mut sequence = 0i64;
        let mut projected: Option<(String, BTreeMap<String, Value>)> = None;
        let mut projected_provenance = BTreeMap::new();
        loop {
            cancel.check()?;
            let row:Option<(i64,String,String,String,String,String,String)>=self.conn()?.query_row("SELECT rowid,item_id,stage,attempt,state,identity_json,receipt_json FROM import_processing_receipts WHERE operation_id=?1 AND stage!='publication.effects' AND rowid>?2 AND NOT EXISTS(SELECT 1 FROM import_processing_receipts n WHERE n.operation_id=import_processing_receipts.operation_id AND n.item_id=import_processing_receipts.item_id AND n.stage=import_processing_receipts.stage AND n.rowid>import_processing_receipts.rowid) ORDER BY rowid LIMIT 1",params![operation,after],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).optional()?;
            let Some((rowid, item, stage, attempt, state, identity, receipt)) = row else {
                break;
            };
            after = rowid;
            let identity: Value = serde_json::from_str(&identity)?;
            let receipt: Value = serde_json::from_str(&receipt)?;
            let before = if let Some((previous, values)) = &projected
                && previous == &item
            {
                values.clone()
            } else {
                super::reconcile::local_fields(&*self.conn()?, &item)?
            };
            if projected
                .as_ref()
                .is_none_or(|(previous, _)| previous != &item)
            {
                projected_provenance = active_provenance(&*self.conn()?, &item)?;
            }
            let mut effect = EffectRow {
                item,
                stage: stage.clone(),
                attempt,
                before,
                relocation_copy: matches!(
                    meta.request.policy.relocation,
                    cuerithm_dj_sync::import::RelocationPolicy::Copy { .. }
                ),
                decoder: identity
                    .get("decoder")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                before_provenance: projected_provenance.clone(),
                values: BTreeMap::new(),
                provenance: BTreeMap::new(),
                media: identity
                    .get("media_guard")
                    .filter(|v| !v.is_null())
                    .map(|v| serde_json::from_value(v.clone()))
                    .transpose()?,
                outcome: "ready".into(),
                reason: None,
                tag_preview: None,
                tag_draft: None,
                organization: None,
            };
            if state != "proposed" {
                effect.outcome = "blocked".into();
                effect.reason = Some(
                    receipt["error"]
                        .as_str()
                        .unwrap_or("Stage has no completed proposal")
                        .into(),
                );
            } else if let Some(proposal) = receipt.get("proposal") {
                if let Some(fields) = proposal.get("fields") {
                    effect.values = serde_json::from_value(fields.clone())?;
                } else if let Some(field) = proposal.get("field").and_then(Value::as_str) {
                    effect
                        .values
                        .insert(field.into(), proposal["value"].clone());
                }
                for field in effect.values.keys() {
                    let expected = identity
                        .get("expected_fields")
                        .and_then(|m| m.get(field))
                        .or_else(|| identity.get("expected_value"));
                    if expected.filter(|v| !v.is_null()) != effect.before.get(field) {
                        effect.outcome = "blocked".into();
                        effect.reason = Some(format!(
                            "Local {field} changed after processing; review a new operation"
                        ));
                    }
                    effect.provenance.insert(field.clone(),json!({"kind":"processing","operation_id":operation,"stage":stage,"attempt":effect.attempt,"identity":identity}));
                }
            }
            if effect.outcome == "ready"
                && let Err(e) =
                    self.validate_import_effect_configuration(&meta.processing, &effect, cancel)
            {
                effect.outcome = "blocked".into();
                effect.reason = Some(e.to_string());
            }
            if effect.outcome == "ready" {
                if let Some(media) = &effect.media
                    && let Err(e) = media.validate(cancel)
                {
                    effect.outcome = "blocked".into();
                    effect.reason = Some(e.to_string());
                }
                if effect.outcome == "ready"
                    && matches!(stage.as_str(), "file.tags" | "file.relocation")
                {
                    if let Err(e) = self.prepare_import_file_effect(
                        operation,
                        &mut effect,
                        &meta.request.policy,
                        cancel,
                    ) {
                        effect.outcome = "blocked".into();
                        effect.reason = Some(format!("{e:#}"));
                    }
                } else if effect.values.is_empty() {
                    effect.outcome = "unchanged".into();
                }
            }
            let mut next = effect.before.clone();
            if effect.outcome == "ready" {
                next.extend(effect.values.clone());
                projected_provenance.extend(effect.provenance.clone());
            }
            projected = Some((effect.item.clone(), next));
            match effect.outcome.as_str() {
                "ready" => summary.ready += 1,
                "blocked" => summary.blocked += 1,
                _ => summary.unchanged += 1,
            };
            let public = public_effect(&effect);
            self.conn()?.execute(
                "INSERT INTO import_preview_effects VALUES(?1,?2,'processing',?3,?4,?5)",
                params![
                    id,
                    sequence,
                    effect.item,
                    encode(&effect)?,
                    encode(&public)?
                ],
            )?;
            sequence += 1;
        }
        self.conn()?.execute(
            "UPDATE import_previews SET json=?1,state='effects_ready' WHERE id=?2",
            params![encode(&summary)?, id],
        )?;
        Ok(summary)
    }
    pub fn import_effects_page(
        &self,
        id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<ImportEffectsPage> {
        ensure!(
            (1..=1000).contains(&limit),
            "Effect page size must be1..1000"
        );
        let after = if let Some(s) = cursor {
            let (p, n): (String, i64) = serde_json::from_str(s)?;
            ensure!(p == id, "Effect cursor belongs to another preview");
            n
        } else {
            -1
        };
        let conn = self.conn()?;
        let mut q=conn.prepare("SELECT sequence,effect_json FROM import_preview_effects WHERE preview_id=?1 AND kind='processing' AND sequence>?2 ORDER BY sequence LIMIT ?3")?;
        let mut rows = q.query(params![id, after, limit + 1])?;
        let mut records = vec![];
        let mut bytes = 0;
        let mut last = after;
        let mut more = false;
        while let Some(row) = rows.next()? {
            let raw: String = row.get(1)?;
            if records.len() == limit as usize || bytes + raw.len() > 16 * 1024 * 1024 {
                more = true;
                break;
            }
            bytes += raw.len();
            last = row.get(0)?;
            records.push(serde_json::from_str(&raw)?);
        }
        Ok(ImportEffectsPage {
            records,
            next_cursor: if more {
                Some(serde_json::to_string(&(id, last))?)
            } else {
                None
            },
        })
    }
    pub fn apply_import_effects(
        &self,
        id: &str,
        cancel: &CancellationToken,
    ) -> Result<ImportProcessingSummary> {
        let gate = self
            .inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
            .processing_gate
            .clone();
        let _gate = gate
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (raw, nonce, state): (String, String, String) = self.conn()?.query_row(
            "SELECT json,runtime_nonce,state FROM import_previews WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let summary: ImportEffectsPreview = serde_json::from_str(&raw)?;
        if state == "effects_applied" {
            return self.import_effect_result(id);
        }
        ensure!(!self.conn()?.query_row("SELECT EXISTS(SELECT 1 FROM import_processing_receipts WHERE operation_id=?1 AND stage='publication.effects')",[&summary.operation_id],|r|r.get::<_,bool>(0))?,"Effects already have a publication journal; use guarded recovery or Undo instead of another preview");
        ensure!(
            state == "effects_ready",
            "Effects preview unavailable; recover interrupted publication"
        );
        ensure!(
            nonce
                == self
                    .inner
                    .import_runtime
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
                    .nonce,
            "Effects preview expired; create a new preview"
        );
        // Validate every field and native file before claiming any local effects.
        let mut after = -1;
        let mut projected: Option<(String, BTreeMap<String, Value>)> = None;
        while let Some((seq, effect)) = self.next_import_effect(id, after)? {
            after = seq;
            cancel.check()?;
            if effect.outcome == "ready" {
                self.validate_import_effect_current(&summary.operation_id, &effect, cancel)?;
                self.validate_import_relocation_references(&effect)?;
                let mut current = if let Some((previous, values)) = &projected
                    && previous == &effect.item
                {
                    values.clone()
                } else {
                    super::reconcile::local_fields(&*self.conn()?, &effect.item)?
                };
                ensure!(
                    current == effect.before,
                    "Library changed after effects preview"
                );
                current.extend(effect.values.clone());
                projected = Some((effect.item.clone(), current));
                if let Some(media) = &effect.media {
                    media.validate(cancel)?;
                }
            }
        }
        {
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            ensure!(tx.execute("UPDATE import_previews SET state='effects_applying' WHERE id=?1 AND state='effects_ready'",[id])?==1,"Effects preview already claimed");
            let mut q=tx.prepare("SELECT proposal_json FROM import_preview_effects WHERE preview_id=?1 AND kind='processing'")?;
            let mut rows = q.query([id])?;
            while let Some(row) = rows.next()? {
                let effect: EffectRow = serde_json::from_str(&row.get::<_, String>(0)?)?;
                if effect.outcome != "ready" {
                    continue;
                }
                tx.execute("INSERT INTO import_processing_receipts VALUES(?1,?2,'publication.effects',?3,'applying',?4,?5,?6)",params![summary.operation_id,effect.item,format!("{id}:{}:{}",effect.stage,effect.attempt),encode(&effect)?,encode(&EffectJournal::default())?,now()])?;
            }
            drop(rows);
            drop(q);
            tx.commit()?;
        }
        let result = self.resume_import_effects(&summary.operation_id, cancel)?;
        ensure!(
            result.recovery_required == 0,
            "Import publication requires recovery: {}",
            result.errors.join("; ")
        );
        self.conn()?.execute(
            "UPDATE import_previews SET state='effects_applied' WHERE id=?1",
            [id],
        )?;
        self.import_effect_result(id)
    }
    pub(super) fn validate_import_effect_current(
        &self,
        operation: &str,
        row: &EffectRow,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let raw:String=self.conn()?.query_row("SELECT p.json FROM import_previews p JOIN import_operations o ON o.preview_id=p.id WHERE o.id=?1",[operation],|r|r.get(0))?;
        let meta: PreviewMeta = serde_json::from_str(&raw)?;
        self.validate_import_effect_configuration(&meta.processing, row, cancel)
    }
    fn validate_import_effect_configuration(
        &self,
        frozen: &super::processing::FrozenProcessing,
        row: &EffectRow,
        cancel: &CancellationToken,
    ) -> Result<()> {
        if super::processing::validate_effect(frozen, &row.stage, cancel)? {
            ensure!(
                row.decoder.as_deref() == Some(self.import_media_runtime()?.identity.as_str()),
                "Decoder changed after processing; create a new preview"
            );
        }
        Ok(())
    }
    pub(super) fn next_import_effect(
        &self,
        id: &str,
        after: i64,
    ) -> Result<Option<(i64, EffectRow)>> {
        let row:Option<(i64,String)>=self.conn()?.query_row("SELECT sequence,proposal_json FROM import_preview_effects WHERE preview_id=?1 AND kind='processing' AND sequence>?2 ORDER BY sequence LIMIT 1",params![id,after],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        row.map(|(n, s)| Ok((n, serde_json::from_str(&s)?)))
            .transpose()
    }
    fn import_effect_result(&self, id: &str) -> Result<ImportProcessingSummary> {
        let summary: ImportEffectsPreview = serde_json::from_str(&self.conn()?.query_row(
            "SELECT json FROM import_previews WHERE id=?1",
            [id],
            |r| r.get::<_, String>(0),
        )?)?;
        Ok(ImportProcessingSummary {
            processed: summary.ready,
            blocked: summary.blocked,
            unchanged: summary.unchanged,
            ..Default::default()
        })
    }
}
fn public_effect(row: &EffectRow) -> ImportEffect {
    ImportEffect {
        item_id: row.item.clone(),
        stage: row.stage.clone(),
        outcome: row.outcome.clone(),
        reason: row.reason.clone(),
        changes: json!({"fields":row.values,"tag_preview_id":row.tag_preview,"organization":row.organization}),
    }
}
pub(crate) fn publication_metadata(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<cuerithm_cleanup::Metadata> {
    let fields = super::reconcile::local_fields(conn, id)?;
    let mut metadata = cuerithm_cleanup::Metadata::default();
    for field in cuerithm_cleanup::MetadataField::ALL {
        let name = serde_json::to_value(field)?
            .as_str()
            .context("Invalid metadata field")?
            .to_owned();
        let value = fields
            .get(&format!("metadata.{name}"))
            .or_else(|| {
                if name == "artist" {
                    fields.get("metadata.artists")
                } else if name == "genre" {
                    fields.get("metadata.genres")
                } else {
                    None
                }
            })
            .and_then(|v| v.get("value"));
        let values = match value {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => vec![],
        };
        metadata.set(*field, values);
    }
    Ok(metadata)
}

fn active_provenance(conn: &rusqlite::Connection, item: &str) -> Result<BTreeMap<String, Value>> {
    let mut query=conn.prepare("SELECT field,provenance_json FROM import_active_fields WHERE item_id=?1 AND substr(field,1,1)!='_'")?;
    query
        .query_map([item], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .map(|row| {
            let (k, v) = row?;
            Ok((k, serde_json::from_str(&v)?))
        })
        .collect()
}
