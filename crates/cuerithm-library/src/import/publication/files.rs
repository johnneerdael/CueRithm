use super::*;
use cuerithm_dj_sync::import::{ImportPolicy, RelocationPolicy};
impl Library {
    pub(super) fn prepare_import_file_effect(
        &self,
        op: &str,
        row: &mut EffectRow,
        policy: &ImportPolicy,
        cancel: &CancellationToken,
    ) -> Result<()> {
        self.validate_import_source_processes(row)?;
        let media = row.media.as_ref().context("Imported media unresolved")?;
        media.validate(cancel)?;
        if row.stage == "file.relocation"
            && !matches!(policy.relocation, RelocationPolicy::Copy { .. })
        {
            let refs: u64 = self.conn()?.query_row(
                "SELECT COUNT(*) FROM import_bindings WHERE item_id=?1",
                [&row.item],
                |r| r.get(0),
            )?;
            ensure!(
                refs == 0,
                "Source reference repair is not qualified for this imported library; keep the original path or explicitly copy"
            );
        }
        self.promote_import_file_owner(&row.item, media, cancel)?;
        if row.stage == "file.tags" {
            #[cfg(feature = "autotag")]
            {
                let fields = self.final_import_effect_fields(op, &row.item)?;
                let patch = tag_patch(&fields, policy)?;
                let service =
                    crate::MetadataEditService::new(self.clone(), self.import_media_runtime()?);
                let draft =
                    service.create_draft_cancellable(std::slice::from_ref(&row.item), cancel)?;
                let draft = service.update_draft(
                    &draft.id,
                    draft.revision,
                    crate::MetadataEditorPatch {
                        fields: patch,
                        ..Default::default()
                    },
                )?;
                let preview = service.preview(&draft.id, draft.revision, cancel)?;
                if preview.rows.iter().all(|r| r.outcome == "unchanged") {
                    row.outcome = "unchanged".into();
                } else {
                    ensure!(
                        preview.can_apply,
                        "{}",
                        preview
                            .rows
                            .iter()
                            .filter_map(|r| r.error.clone())
                            .collect::<Vec<_>>()
                            .join("; ")
                    );
                }
                row.tag_preview = Some(preview.id);
                row.tag_draft = Some(draft.id);
                row.file_changes = json!({"audio_tags":preview.rows.iter().map(|change|json!({"title":change.title,"path":media.path.to_string_lossy(),"before":change.before,"after":change.after,"fields":change.fields})).collect::<Vec<_>>()});
                return Ok(());
            }
            #[cfg(not(feature = "autotag"))]
            anyhow::bail!("Audio-tag writing requires the tagging-enabled build");
        }
        let (mode, rule, root) = match &policy.relocation {
            RelocationPolicy::Off => {
                row.outcome = "unchanged".into();
                return Ok(());
            }
            RelocationPolicy::RenameInPlace { rule } => {
                (crate::OrganizationSettingMode::RenameInPlace, rule, None)
            }
            RelocationPolicy::Move { root_id, rule } => (
                crate::OrganizationSettingMode::Move,
                rule,
                Some(root_id.clone()),
            ),
            RelocationPolicy::Copy { root_id, rule } => (
                crate::OrganizationSettingMode::Copy,
                rule,
                Some(root_id.clone()),
            ),
        };
        // Render the final reviewed metadata, including pending proposals.
        let final_fields = self.final_import_effect_fields(op, &row.item)?;
        ensure!(
            final_fields == row.before,
            "Apply reviewed metadata effects before creating a relocation preview"
        );
        let preview = self.preview_import_organization(
            crate::OrganizationPreviewRequest {
                scope: crate::OrganizationScope::Tracks {
                    track_ids: vec![row.item.clone()],
                },
                settings: crate::OrganizationSettings {
                    mode,
                    pattern: rule.clone(),
                    destination_root_id: root,
                    ..Default::default()
                },
            },
            BTreeMap::from([(row.item.clone(), naming_context(&final_fields))]),
            cancel,
        )?;
        let page = self.organization_preview_page(&preview.id, 0, 1)?;
        ensure!(
            preview.totals.blocked == 0,
            "{}",
            page.rows
                .iter()
                .filter_map(|r| r.reason.clone())
                .collect::<Vec<_>>()
                .join("; ")
        );
        if preview.totals.unchanged > 0 {
            row.outcome = "unchanged".into();
        } else {
            ensure!(preview.can_apply, "Organization has no applicable result");
        }
        row.organization = Some((preview.id, preview.revision));
        row.file_changes = json!({"files":page.rows});
        Ok(())
    }
    fn promote_import_file_owner(
        &self,
        id: &str,
        media: &MediaGuard,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let exists: bool = self.conn()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM tracks WHERE id=?1)",
            [id],
            |r| r.get(0),
        )?;
        if exists {
            return Ok(());
        }
        media.validate(cancel)?;
        let info = self
            .import_media_runtime()?
            .probe_cancellable(&media.path, cancel)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO tracks(id,path,title,source_hash,state,metadata_json,duration,created,updated) VALUES(?1,?2,'',?3,'imported',?4,?5,?6,?6)",params![id,path_bytes(&media.path),media.sha256,serde_json::to_string(&info)?,Option::<f64>::None,now()])?;
        tx.execute(
            "UPDATE import_items SET local_track_id=id WHERE id=?1 AND local_track_id IS NULL",
            [id],
        )?;
        let fields = super::super::projection::active_fields_tx(&tx, id)?;
        super::super::recovery::sync_projection(&tx, id, &fields)?;
        super::super::native_shadow::capture_owned(&tx, id)?;
        tx.commit()?;
        Ok(())
    }
    fn final_import_effect_fields(&self, op: &str, id: &str) -> Result<BTreeMap<String, Value>> {
        let mut fields = super::super::reconcile::local_fields(&*self.conn()?, id)?;
        let conn = self.conn()?;
        let mut q=conn.prepare(&format!("SELECT identity_json,receipt_json FROM import_processing_receipts r WHERE operation_id=?1 AND item_id=?2 AND state='proposed' AND stage NOT LIKE 'file.%' AND stage!='publication.effects' AND NOT EXISTS(SELECT 1 FROM import_processing_receipts n WHERE n.operation_id=r.operation_id AND n.item_id=r.item_id AND n.stage=r.stage AND n.rowid>r.rowid) ORDER BY {},stage",super::super::processing::STAGE_ORDER_SQL))?;
        let mut rows = q.query(params![op, id])?;
        while let Some(row) = rows.next()? {
            let identity: Value = serde_json::from_str(&row.get::<_, String>(0)?)?;
            let receipt: Value = serde_json::from_str(&row.get::<_, String>(1)?)?;
            let proposal = &receipt["proposal"];
            if let Some(map) = proposal.get("fields").and_then(Value::as_object) {
                for (field, value) in map {
                    let expected = identity.get("expected_fields").and_then(|m| m.get(field));
                    ensure!(
                        expected.filter(|v| !v.is_null()) == fields.get(field),
                        "Local {field} changed after processing"
                    );
                    fields.insert(field.clone(), value.clone());
                }
            } else if let Some(field) = proposal["field"].as_str() {
                ensure!(
                    identity.get("expected_value").filter(|v| !v.is_null()) == fields.get(field),
                    "Local {field} changed after processing"
                );
                fields.insert(field.into(), proposal["value"].clone());
            }
        }
        Ok(fields)
    }
    pub(in crate::import) fn publish_import_file_row(
        &self,
        op: &str,
        attempt: &str,
        row: &EffectRow,
        journal: &mut EffectJournal,
        cancel: &CancellationToken,
    ) -> Result<()> {
        if row.tag_preview.is_none() && row.organization.is_none() {
            return Ok(());
        }
        self.validate_import_source_processes(row)?;
        if journal.file_projection.is_none() {
            journal.file_projection = Some(super::super::reconcile::local_fields(
                &*self.conn()?,
                &row.item,
            )?);
            self.save_import_journal(op, &row.item, attempt, "applying", journal)?;
        }
        #[cfg(feature = "autotag")]
        if let Some(preview) = &row.tag_preview {
            let service =
                crate::MetadataEditService::new(self.clone(), self.import_media_runtime()?);
            if journal.tag_batch.is_none() {
                journal.tag_batch=self.conn()?.query_row("SELECT id FROM metadata_edit_batches WHERE draft_id=?1 AND kind='apply' ORDER BY created,rowid DESC LIMIT 1",[row.tag_draft.as_deref().context("Missing tag draft authority")?],|r|r.get(0)).optional()?;
            }
            let batch = if let Some(id) = &journal.tag_batch {
                service.recover(id, cancel)?
            } else {
                service.rebase_import_preview(preview, &row.item, &row.before)?;
                service.apply_import_linked(preview,cancel,||{let id:String=self.conn()?.query_row("SELECT id FROM metadata_edit_batches WHERE draft_id=?1 AND kind='apply' ORDER BY created,rowid DESC LIMIT 1",[row.tag_draft.as_deref().context("Missing tag draft")?],|r|r.get(0))?;journal.tag_batch=Some(id);self.save_import_journal(op,&row.item,attempt,"applying",journal)})?
            };
            journal.tag_batch = Some(batch.id);
            self.save_import_journal(op, &row.item, attempt, "applying", journal)?;
            ensure!(
                !batch.can_recover
                    && batch
                        .rows
                        .iter()
                        .all(|r| matches!(r.outcome.as_str(), "written" | "unchanged")),
                "Tag publication requires recovery: {}",
                batch.error.unwrap_or_default()
            );
        }
        self.validate_import_relocation_references(row)?;
        if let Some((original, _)) = &row.organization {
            if journal.organization_preview.is_none() {
                let fresh = self.rebase_import_organization(original, cancel)?;
                journal.organization_preview = Some((fresh.id, fresh.revision));
                self.save_import_journal(op, &row.item, attempt, "applying", journal)?;
            }
            let (id, revision) = journal
                .organization_preview
                .as_ref()
                .context("Missing organization proof")?;
            if journal.organization_batch.is_none() {
                journal.organization_batch=self.conn()?.query_row("SELECT id FROM organization_batches WHERE preview_id=?1 ORDER BY created,rowid DESC LIMIT 1",[id],|r|r.get(0)).optional()?;
            }
            let batch = if let Some(id) = &journal.organization_batch {
                self.recover_organization_batch(id, cancel)?
            } else {
                self.apply_organization_preview(id, revision, cancel)?
            };
            journal.organization_batch = Some(batch.id);
            self.save_import_journal(op, &row.item, attempt, "applying", journal)?;
            ensure!(
                batch.totals.recovery_required == 0
                    && batch.totals.failed == 0
                    && batch.totals.blocked == 0,
                "Relocation requires recovery"
            );
        }
        self.refresh_import_file_projection(
            &row.item,
            journal
                .file_projection
                .as_ref()
                .context("Missing file projection")?,
        )?;
        Ok(())
    }
    pub(in crate::import) fn undo_import_file_row(
        &self,
        op: &str,
        attempt: &str,
        row: &EffectRow,
        journal: &mut EffectJournal,
        cancel: &CancellationToken,
    ) -> Result<()> {
        if journal.tag_batch.is_none() && journal.organization_batch.is_none() {
            return Ok(());
        }
        self.validate_import_source_processes(row)?;
        if journal.undo_file_projection.is_none() {
            journal.undo_file_projection = Some(super::super::reconcile::local_fields(
                &*self.conn()?,
                &row.item,
            )?);
            self.save_import_journal(op, &row.item, attempt, "applied", journal)?;
        }
        if let Some(batch) = journal.organization_batch.clone() {
            if journal.undo_organization_batch.is_none() {
                journal.undo_organization_batch=self.conn()?.query_row("SELECT id FROM organization_batches WHERE undo_of=?1 ORDER BY created,rowid DESC LIMIT 1",[&batch],|r|r.get(0)).optional()?;
            }
            let undo = if let Some(id) = &journal.undo_organization_batch {
                self.recover_organization_batch(id, cancel)?
            } else {
                self.undo_organization_batch(&batch, cancel)?
            };
            journal.undo_organization_batch = Some(undo.id);
            self.save_import_journal(op, &row.item, attempt, "applied", journal)?;
            ensure!(
                undo.totals.failed == 0
                    && undo.totals.blocked == 0
                    && undo.totals.recovery_required == 0,
                "Relocation Undo requires recovery"
            );
        }
        #[cfg(feature = "autotag")]
        if let Some(batch) = journal.tag_batch.clone() {
            let service =
                crate::MetadataEditService::new(self.clone(), self.import_media_runtime()?);
            if journal.undo_tag_batch.is_none() {
                journal.undo_tag_batch=self.conn()?.query_row("SELECT id FROM metadata_edit_batches WHERE undo_of=?1 AND kind='undo' ORDER BY created,rowid DESC LIMIT 1",[&batch],|r|r.get(0)).optional()?;
            }
            let owned_relocation_undo:Option<String>=self.conn()?.query_row("SELECT json_extract(receipt_json,'$.undo_organization_batch') FROM import_processing_receipts WHERE operation_id=?1 AND item_id=?2 AND stage='publication.effects' AND state='undone' AND json_extract(identity_json,'$.stage')='file.relocation' AND json_extract(receipt_json,'$.undo_organization_batch') IS NOT NULL ORDER BY rowid DESC LIMIT 1",params![op,row.item],|r|r.get(0)).optional()?;
            if let Some(organization_undo) = &owned_relocation_undo
                && journal.undo_tag_batch.is_none()
            {
                service.rebase_import_undo(&batch, &row.item, &row.before, organization_undo)?;
            }
            let undo = if let Some(id) = &journal.undo_tag_batch {
                service.recover(id, cancel)?
            } else {
                service.undo_linked_with(&batch,cancel,None,||{
                    let id:String=self.conn()?.query_row("SELECT id FROM metadata_edit_batches WHERE undo_of=?1 AND kind='undo' ORDER BY created,rowid DESC LIMIT 1",[&batch],|r|r.get(0))?;
                    journal.undo_tag_batch=Some(id);self.save_import_journal(op,&row.item,attempt,"applied",journal)
                })?
            };
            journal.undo_tag_batch = Some(undo.id);
            self.save_import_journal(op, &row.item, attempt, "applied", journal)?;
            ensure!(
                !undo.can_recover
                    && undo
                        .rows
                        .iter()
                        .all(|r| matches!(r.outcome.as_str(), "undone" | "unchanged")),
                "Tag Undo blocked: {}",
                undo.error.unwrap_or_default()
            );
        }
        self.refresh_import_file_projection(
            &row.item,
            journal
                .undo_file_projection
                .as_ref()
                .context("Missing Undo file projection")?,
        )?;
        Ok(())
    }
    pub(in crate::import) fn validate_import_relocation_references(
        &self,
        row: &EffectRow,
    ) -> Result<()> {
        self.validate_import_source_processes(row)?;
        if row.organization.is_some() && !row.relocation_copy {
            let refs: u64 = self.conn()?.query_row(
                "SELECT COUNT(*) FROM import_bindings WHERE item_id=?1",
                [&row.item],
                |r| r.get(0),
            )?;
            ensure!(
                refs == 0,
                "Source reference repair is not qualified for this imported library; preview again"
            );
        }
        Ok(())
    }
    fn validate_import_source_processes(&self, row: &EffectRow) -> Result<()> {
        if row.stage != "file.tags" && (row.organization.is_none() || row.relocation_copy) {
            return Ok(());
        }
        let providers = {
            let conn = self.conn()?;
            let mut query=conn.prepare("SELECT DISTINCT s.provider FROM import_bindings b JOIN import_sources s ON s.id=b.source_id WHERE b.item_id=?1")?;
            query
                .query_map([&row.item], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        for provider in providers {
            cuerithm_dj_sync::discovery::ensure_import_source_closed(serde_json::from_str(
                &provider,
            )?)?;
        }
        Ok(())
    }
    pub(in crate::import) fn validate_import_effect_media(
        &self,
        op: &str,
        row: &EffectRow,
        media: &MediaGuard,
        cancel: &CancellationToken,
    ) -> Result<()> {
        #[cfg(not(feature = "autotag"))]
        let _ = (op, row);
        if media.validate(cancel).is_ok() {
            return Ok(());
        }
        #[cfg(feature = "autotag")]
        if row.stage == "file.relocation" {
            let raw:Option<String>=self.conn()?.query_row("SELECT receipt_json FROM import_processing_receipts WHERE operation_id=?1 AND item_id=?2 AND stage='publication.effects' AND state='applied' AND json_extract(identity_json,'$.stage')='file.tags' ORDER BY rowid DESC LIMIT 1",params![op,row.item],|r|r.get(0)).optional()?;
            if let Some(raw) = raw {
                let journal: EffectJournal = serde_json::from_str(&raw)?;
                let service =
                    crate::MetadataEditService::new(self.clone(), self.import_media_runtime()?);
                service.verify_import_output(
                    journal
                        .tag_batch
                        .as_deref()
                        .context("Missing owned tag output")?,
                    &media.path,
                    media
                        .sha256
                        .as_deref()
                        .context("Missing original media hash")?,
                )?;
                cancel.check()?;
                return Ok(());
            }
        }
        media.validate(cancel)
    }
    fn refresh_import_file_projection(
        &self,
        id: &str,
        fields: &BTreeMap<String, Value>,
    ) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute("UPDATE import_items SET path=(SELECT path FROM tracks WHERE id=?1),source_hash=(SELECT source_hash FROM tracks WHERE id=?1) WHERE id=?1 AND local_track_id IS NOT NULL",[id])?;
        let columns = fields
            .iter()
            .filter(|(field, _)| {
                matches!(
                    field.as_str(),
                    "metadata.title"
                        | "metadata.artist"
                        | "metadata.artists"
                        | "metadata.album"
                        | "metadata.genre"
                        | "metadata.genres"
                        | "key"
                        | "bpm"
                        | "energy"
                )
            })
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        super::super::recovery::sync_projection(&tx, id, &columns)?;
        tx.commit()?;
        Ok(())
    }
}
#[cfg(feature = "autotag")]
fn tag_patch(
    fields: &BTreeMap<String, Value>,
    policy: &ImportPolicy,
) -> Result<cuerithm_autotag::MetadataPatch> {
    use cuerithm_autotag::{MetadataPatch, MetadataTextField, TextEdit, ValueEdit};
    let mut patch = MetadataPatch::default();
    for selected in &policy.write_audio_tags {
        let name = selected.strip_prefix("metadata.").unwrap_or(selected);
        let field = if matches!(name, "key" | "bpm" | "energy") {
            name.to_owned()
        } else {
            format!("metadata.{name}")
        };
        if !super::super::projection::field_selected(policy, &field) {
            continue;
        }
        let alias = match name {
            "artist" => Some("metadata.artists"),
            "artists" => Some("metadata.artist"),
            "genre" => Some("metadata.genres"),
            "genres" => Some("metadata.genre"),
            _ => None,
        };
        let value = fields
            .get(&field)
            .or_else(|| alias.and_then(|alias| fields.get(alias)))
            .context(format!(
                "Selected audio-tag field {name} has no accepted value"
            ))?;
        if name == "bpm" {
            patch.bpm = match value.get("value").and_then(Value::as_f64) {
                Some(v) => ValueEdit::Set { value: v },
                None => ValueEdit::Clear,
            };
            continue;
        }
        let native = match name {
            "artists" | "artist" => MetadataTextField::Artist,
            "genres" | "genre" => MetadataTextField::Genre,
            "mix" => MetadataTextField::Version,
            _ => serde_json::from_value(json!(name)).context(format!(
                "Audio-tag field {name} requires a qualified native writer"
            ))?,
        };
        let values = if name == "key" {
            super::super::projection::key_text(value)
                .map(|key| vec![key.to_owned()])
                .unwrap_or_default()
        } else {
            match value.get("value") {
                Some(Value::String(v)) => vec![v.clone()],
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .context("Tag text is not a string")
                    })
                    .collect::<Result<Vec<_>>>()?,
                None => vec![],
                _ => anyhow::bail!("Selected tag {name} has unsupported native representation"),
            }
        };
        patch.text.insert(
            native,
            if values.is_empty() {
                TextEdit::Clear
            } else {
                TextEdit::Set { values }
            },
        );
    }
    patch.validate()?;
    Ok(patch)
}
fn naming_context(fields: &BTreeMap<String, Value>) -> cuerithm_organize::template::NamingContext {
    let values = |name: &str| -> Vec<String> {
        match fields
            .get(&format!("metadata.{name}"))
            .and_then(|v| v.get("value"))
        {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => vec![],
        }
    };
    let first = |name: &str| values(name).first().cloned();
    cuerithm_organize::template::NamingContext {
        artists: if fields.contains_key("metadata.artists") {
            values("artists")
        } else {
            values("artist")
        },
        title: first("title"),
        album: first("album"),
        album_artists: values("album_artist"),
        genres: if fields.contains_key("metadata.genres") {
            values("genres")
        } else {
            values("genre")
        },
        label: first("label"),
        version: first("mix"),
        release_date: first("release_date"),
        key: fields
            .get("key")
            .and_then(super::super::projection::key_text)
            .map(str::to_owned),
        bpm_hundredths: fields
            .get("bpm")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite() && *v > 0.0)
            .map(|v| (v * 100.0).round() as u32),
        energy: fields
            .get("energy")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_u64)
            .and_then(|v| u8::try_from(v).ok()),
        ..Default::default()
    }
}
