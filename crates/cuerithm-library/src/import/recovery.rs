use super::publication::{EffectJournal, EffectRow};
use crate::{Library, model::now};
use anyhow::{Result, ensure};
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportRecoveryResult {
    pub recovered: u64,
    pub recovery_required: u64,
    pub errors: Vec<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportUndoResult {
    pub undone: u64,
    pub blocked: u64,
    pub errors: Vec<String>,
}
impl Library {
    pub fn recover_import_operation(
        &self,
        operation: &str,
        cancel: &CancellationToken,
    ) -> Result<ImportRecoveryResult> {
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
        self.resume_import_effects(operation, cancel)
    }
    pub(super) fn resume_import_effects(
        &self,
        operation: &str,
        cancel: &CancellationToken,
    ) -> Result<ImportRecoveryResult> {
        let mut after = 0i64;
        let mut result = ImportRecoveryResult::default();
        loop {
            cancel.check()?;
            let next:Option<(i64,String,String,String)>=self.conn()?.query_row("SELECT rowid,attempt,identity_json,receipt_json FROM import_processing_receipts WHERE operation_id=?1 AND stage='publication.effects' AND state IN ('applying','recovery_required') AND rowid>?2 ORDER BY rowid LIMIT 1",params![operation,after],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
            let Some((rowid, attempt, raw, journal)) = next else {
                break;
            };
            after = rowid;
            let row: EffectRow = serde_json::from_str(&raw)?;
            let mut journal: EffectJournal = serde_json::from_str(&journal)?;
            match self.publish_import_row(operation, &attempt, &row, &mut journal, cancel) {
                Ok(()) => {
                    journal.last_error = None;
                    self.save_import_journal(operation, &row.item, &attempt, "applied", &journal)?;
                    result.recovered += 1;
                }
                Err(e) => {
                    journal.last_error = Some(bounded_error(&e.to_string()));
                    self.save_import_journal(
                        operation,
                        &row.item,
                        &attempt,
                        "recovery_required",
                        &journal,
                    )?;
                    result.recovery_required += 1;
                    if result.errors.len() < 100 {
                        result
                            .errors
                            .push(bounded_error(&format!("{}: {e}", row.item)));
                    }
                }
            }
        }
        if result.recovery_required == 0 {
            self.conn()?.execute("UPDATE import_previews SET state='effects_applied' WHERE state='effects_applying' AND json_extract(json,'$.operation_id')=?1",[operation])?;
        }
        Ok(result)
    }
    pub(super) fn save_import_journal(
        &self,
        op: &str,
        item: &str,
        attempt: &str,
        state: &str,
        journal: &EffectJournal,
    ) -> Result<()> {
        self.conn()?.execute("UPDATE import_processing_receipts SET state=?1,receipt_json=?2,updated=?3 WHERE operation_id=?4 AND item_id=?5 AND stage='publication.effects' AND attempt=?6",params![state,serde_json::to_string(journal)?,now(),op,item,attempt])?;
        Ok(())
    }
    fn publish_import_row(
        &self,
        op: &str,
        attempt: &str,
        row: &EffectRow,
        journal: &mut EffectJournal,
        cancel: &CancellationToken,
    ) -> Result<()> {
        cancel.check()?;
        if !journal.local_applied {
            self.validate_import_effect_current(op, row, cancel)?;
            if let Some(media) = &row.media {
                self.validate_import_effect_media(op, row, media, cancel)?;
            }
            let mut conn = self.conn()?;
            let tx = conn.transaction()?;
            let current = super::reconcile::local_fields(&tx, &row.item)?;
            ensure!(
                current == row.before,
                "Library changed before effect publication"
            );
            for (field, value) in &row.values {
                write_active(
                    &tx,
                    &row.item,
                    field,
                    Some(value),
                    row.provenance.get(field),
                )?;
            }
            sync_projection(&tx, &row.item, &row.values)?;
            if !row.values.is_empty() {
                Library::bump(&tx)?;
            }
            journal.local_applied = true;
            tx.execute("UPDATE import_processing_receipts SET receipt_json=?1 WHERE operation_id=?2 AND item_id=?3 AND stage='publication.effects' AND attempt=?4",params![serde_json::to_string(journal)?,op,row.item,attempt])?;
            tx.commit()?;
        }
        self.publish_import_file_row(op, attempt, row, journal, cancel)
    }
    pub fn undo_import_operation(
        &self,
        operation: &str,
        cancel: &CancellationToken,
    ) -> Result<ImportUndoResult> {
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
        let mut before = i64::MAX;
        let mut result = ImportUndoResult::default();
        loop {
            cancel.check()?;
            let next:Option<(i64,String,String,String)>=self.conn()?.query_row("SELECT rowid,attempt,identity_json,receipt_json FROM import_processing_receipts WHERE operation_id=?1 AND stage='publication.effects' AND state='applied' AND rowid<?2 ORDER BY rowid DESC LIMIT 1",params![operation,before],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
            let Some((rowid, attempt, raw, journal)) = next else {
                break;
            };
            before = rowid;
            let row: EffectRow = serde_json::from_str(&raw)?;
            let mut journal: EffectJournal = serde_json::from_str(&journal)?;
            let outcome = (|| -> Result<()> {
                let current = super::reconcile::local_fields(&*self.conn()?, &row.item)?;
                for (field, value) in &row.values {
                    ensure!(
                        current.get(field) == Some(value),
                        "Later {field} edit prevents Undo"
                    );
                }
                self.undo_import_file_row(operation, &attempt, &row, &mut journal, cancel)?;
                let mut conn = self.conn()?;
                let tx = conn.transaction()?;
                for field in row.values.keys() {
                    write_active(
                        &tx,
                        &row.item,
                        field,
                        row.before.get(field),
                        row.before_provenance.get(field),
                    )?;
                }
                let restored = row
                    .values
                    .keys()
                    .filter_map(|field| {
                        row.before
                            .get(field)
                            .map(|value| (field.clone(), value.clone()))
                    })
                    .collect();
                sync_projection(&tx, &row.item, &restored)?;
                if !row.values.is_empty() {
                    Library::bump(&tx)?;
                }
                journal.undone = true;
                journal.last_error = None;
                tx.execute("UPDATE import_processing_receipts SET state='undone',receipt_json=?1,updated=?2 WHERE operation_id=?3 AND item_id=?4 AND stage='publication.effects' AND attempt=?5",params![serde_json::to_string(&journal)?,now(),operation,row.item,attempt])?;
                tx.commit()?;
                Ok(())
            })();
            match outcome {
                Ok(()) => result.undone += 1,
                Err(e) => {
                    result.blocked += 1;
                    journal.last_error = Some(bounded_error(&e.to_string()));
                    self.save_import_journal(operation, &row.item, &attempt, "applied", &journal)?;
                    if result.errors.len() < 100 {
                        result
                            .errors
                            .push(bounded_error(&format!("{}: {e}", row.item)));
                    }
                }
            }
        }
        Ok(result)
    }
}
pub(super) fn bounded_error(message: &str) -> String {
    let mut end = message.len().min(1024);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_owned()
}
fn write_active(
    conn: &Connection,
    item: &str,
    field: &str,
    value: Option<&Value>,
    provenance: Option<&Value>,
) -> Result<()> {
    ensure!(!field.starts_with('_'), "Reserved import field");
    if let Some(v) = value {
        conn.execute("INSERT INTO import_active_fields(item_id,field,value_json,provenance_json) VALUES(?1,?2,?3,?4) ON CONFLICT(item_id,field) DO UPDATE SET value_json=excluded.value_json,provenance_json=excluded.provenance_json,revision=revision+1",params![item,field,serde_json::to_string(v)?,serde_json::to_string(&provenance.cloned().unwrap_or(json!({"kind":"undo"})))?])?;
    } else {
        let column = match field {
            "key" => Some(("musical_key", true)),
            "bpm" => Some(("bpm", true)),
            "energy" => Some(("energy", true)),
            "metadata.duration" => Some(("duration", true)),
            "metadata.title" => Some(("title", false)),
            "metadata.artist" | "metadata.artists" => Some(("artist", false)),
            "metadata.album" => Some(("album", false)),
            "metadata.genre" | "metadata.genres" => Some(("genre", false)),
            _ => None,
        };
        if let Some((column, nullable)) = column {
            conn.execute(
                &format!(
                    "UPDATE tracks SET {column}={} WHERE id=?1",
                    if nullable { "NULL" } else { "''" }
                ),
                [item],
            )?;
        }
        conn.execute(
            "DELETE FROM import_active_fields WHERE item_id=?1 AND field=?2",
            params![item, field],
        )?;
    }
    Ok(())
}
pub(crate) fn sync_projection(
    conn: &Connection,
    id: &str,
    fields: &std::collections::BTreeMap<String, Value>,
) -> Result<()> {
    let native: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM tracks WHERE id=?1)",
        [id],
        |r| r.get(0),
    )?;
    if native {
        if let Some(value) = fields.get("metadata.duration") {
            super::projection::project_duration_tx(conn, id, value)?;
        }
        for (field, value) in fields {
            let column = match field.as_str() {
                "metadata.title" => "title",
                "metadata.artist" | "metadata.artists" => "artist",
                "metadata.album" => "album",
                "metadata.genre" | "metadata.genres" => "genre",
                _ => continue,
            };
            conn.execute(
                &format!("UPDATE tracks SET {column}=?1 WHERE id=?2"),
                params![super::native_shadow::text(Some(value)), id],
            )?;
        }
        for (field, column) in [("key", "musical_key"), ("bpm", "bpm"), ("energy", "energy")] {
            if let Some(value) = fields.get(field) {
                let v = if field == "key" {
                    super::projection::key_text(value).map(|key| Value::String(key.into()))
                } else {
                    value.get("value").cloned()
                };
                let v = v.unwrap_or(Value::Null);
                let sql = match v {
                    Value::String(s) => rusqlite::types::Value::Text(s),
                    Value::Number(n) => {
                        rusqlite::types::Value::Real(n.as_f64().unwrap_or_default())
                    }
                    _ => rusqlite::types::Value::Null,
                };
                conn.execute(
                    &format!("UPDATE tracks SET {column}=?1 WHERE id=?2"),
                    params![sql, id],
                )?;
            }
        }
        let (title, artist, album, genre): (String, String, String, String) = conn.query_row(
            "SELECT title,artist,album,genre FROM tracks WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        conn.execute(
            "UPDATE tracks SET search_text=?1,sort_title=?2,sort_artist=?3 WHERE id=?4",
            params![
                format!("{title} {artist} {album} {genre}").to_lowercase(),
                title.to_lowercase(),
                artist.to_lowercase(),
                id
            ],
        )?;
        super::native_shadow::capture(conn, id, fields)?;
    }
    super::projection::rebuild_summary_tx(conn, id)?;
    conn.execute(
        "UPDATE import_items SET revision=revision+1,updated=?1 WHERE id=?2",
        params![now(), id],
    )?;
    Ok(())
}
