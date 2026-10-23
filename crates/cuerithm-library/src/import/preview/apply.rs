use super::*;

impl Library {
    pub fn apply_import(
        &self,
        preview_id: &str,
        cancel: &CancellationToken,
    ) -> Result<ImportApplyResult> {
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
        let _organization = self
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        // A committed receipt is authoritative even when the source later changes.
        if let Some(receipt) = self
            .conn()?
            .query_row(
                "SELECT receipt_json FROM import_operations WHERE preview_id=?1",
                [preview_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            return Ok(serde_json::from_str(&receipt)?);
        }
        let (snapshot_id,nonce,source,policy_id,expected_revision,meta,state):(String,String,String,String,u64,String,String)=self.conn()?.query_row("SELECT snapshot_id,runtime_nonce,source_id,policy_id,local_revision,json,state FROM import_previews WHERE id=?1",[preview_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).context("Unknown import preview")?;
        ensure!(
            state == "ready",
            "Import preview was discarded or already consumed"
        );
        let meta: PreviewMeta = serde_json::from_str(&meta)?;
        let (snapshot, current_nonce) = self.guarded_import_snapshot(&snapshot_id)?;
        ensure!(
            nonce == current_nonce,
            "Import preview expired; inspect again"
        );
        snapshot.validate_source(cancel)?;
        let mut after = -1i64;
        loop {
            let next: Option<(i64, String)> = {
                let conn = self.conn()?;
                conn.query_row("SELECT sequence,proposal_json FROM import_preview_effects WHERE preview_id=?1 AND kind='track' AND sequence>?2 ORDER BY sequence LIMIT 1",params![preview_id,after],|r|Ok((r.get(0)?,r.get(1)?))).optional()?
            };
            let Some((sequence, raw)) = next else {
                break;
            };
            after = sequence;
            cancel.check()?;
            let proposal: TrackProposal = serde_json::from_str(&raw)?;
            if let Some(media) = proposal.candidate.media {
                media.validate(cancel)?;
            }
        }
        snapshot.validate_source(cancel)?;
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        if let Some(receipt) = tx
            .query_row(
                "SELECT receipt_json FROM import_operations WHERE preview_id=?1",
                [preview_id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            return Ok(serde_json::from_str(&receipt)?);
        }
        ensure!(
            revision(&tx)? == expected_revision,
            "Library changed after import preview; preview again"
        );
        let actual_state: String = tx.query_row(
            "SELECT state FROM import_previews WHERE id=?1",
            [preview_id],
            |r| r.get(0),
        )?;
        ensure!(actual_state == "ready", "Import preview no longer applies");
        let mut query=tx.prepare("SELECT kind,proposal_json FROM import_preview_effects WHERE preview_id=?1 ORDER BY sequence")?;
        let mut rows = query.query([preview_id])?;
        let mut changed = false;
        while let Some(row) = rows.next()? {
            cancel.check()?;
            let kind: String = row.get(0)?;
            let raw: String = row.get(1)?;
            if kind == "track" {
                let p: TrackProposal = serde_json::from_str(&raw)?;
                let c = &p.candidate;
                if c.identity_blocked {
                    continue;
                }
                let previous: Option<(String, Option<Vec<u8>>, Option<String>)> = tx
                    .query_row(
                        "SELECT availability,path,source_hash FROM import_items WHERE id=?1",
                        [&c.item_id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .optional()?;
                let next_path = c.media.as_ref().map(|m| crate::model::path_bytes(&m.path));
                let needs_projection =
                    previous
                        .as_ref()
                        .is_none_or(|(availability, path, source_hash)| {
                            availability != &c.availability
                                || c.media
                                    .as_ref()
                                    .is_some_and(|media| media.sha256 != *source_hash)
                                || next_path.as_ref().is_some_and(|p| path.as_ref() != Some(p))
                        })
                        || !p.fields.is_empty()
                        || !tx.query_row("SELECT EXISTS(SELECT 1 FROM import_bindings WHERE source_id=?1 AND external_id=?2 AND item_id=?3)",params![source,c.external_id,c.item_id],|r|r.get::<_,bool>(0))?;
                if needs_projection {
                    let track = load_track(&tx, &snapshot_id, &c.external_id)?;
                    project_import_track_tx(
                        &tx,
                        TrackProjection {
                            source_id: &source,
                            snapshot_id: &snapshot_id,
                            observation: &track,
                            item_id: &c.item_id,
                            fields: &p.fields,
                            local_track_id: c.local_track_id.as_deref(),
                            availability: Some(&c.availability),
                            resolved_path: c.media.as_ref().map(|m| m.path.as_path()),
                        },
                    )?;
                    if let Some(media) = &c.media {
                        tx.execute(
                            "UPDATE import_items SET source_hash=?1 WHERE id=?2",
                            params![media.sha256, c.item_id],
                        )?;
                    }
                    changed = true;
                }
                for (field, provenance) in &p.provenance {
                    if p.fields.contains_key(field) {
                        tx.execute("UPDATE import_active_fields SET provenance_json=?1 WHERE item_id=?2 AND field=?3",params![serde_json::to_string(provenance)?,c.item_id,field])?;
                    }
                }
                for (field, (observed, accepted)) in p.baselines {
                    tx.execute("INSERT INTO import_applied_baselines VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(source_id,external_id,field) DO UPDATE SET source_json=excluded.source_json,local_json=excluded.local_json,source_hash=excluded.source_hash,policy_revision=excluded.policy_revision",params![source,c.external_id,field,serde_json::to_string(&observed)?,serde_json::to_string(&accepted)?,digest(&observed)?,policy_id])?;
                }
                tx.execute("UPDATE import_bindings SET observed_hash=(SELECT semantic_hash FROM import_observations WHERE snapshot_id=?1 AND kind='track' AND record_id=?2) WHERE source_id=?3 AND external_id=?2",params![snapshot_id,c.external_id,source])?;
            } else {
                let p: CollectionProposal = serde_json::from_str(&raw)?;
                if p.apply {
                    let mut metadata = p.collection.clone();
                    metadata.occurrences.clear();
                    project_import_collection_tx(
                        &tx,
                        &source,
                        &p.id,
                        &metadata,
                        p.parent_id.as_deref(),
                    )?;
                    if p.inline {
                        for (position, member) in p.collection.occurrences.iter().enumerate() {
                            cancel.check()?;
                            super::super::collections::append_member(
                                &tx,
                                &source,
                                &p.id,
                                position as u64,
                                member,
                            )?;
                        }
                    } else {
                        let raw:String=tx.query_row("SELECT json FROM import_observations WHERE snapshot_id=?1 AND kind='collection' AND record_id=?2",params![snapshot_id,p.collection.external_id],|r|r.get(0))?;
                        let ObservationRecord::Collection(original) = serde_json::from_str(&raw)?
                        else {
                            unreachable!()
                        };
                        super::super::collections::for_source_members(
                            &tx,
                            &snapshot_id,
                            &original,
                            cancel,
                            |position, member| {
                                super::super::collections::append_member(
                                    &tx, &source, &p.id, position, member,
                                )
                            },
                        )?;
                    }
                    changed = true;
                }
                if p.baseline_changed
                    && let Some(accepted) = p.accepted
                {
                    tx.execute("INSERT INTO import_applied_baselines VALUES(?1,?2,'collection',?3,?4,?5,?6) ON CONFLICT(source_id,external_id,field) DO UPDATE SET source_json=excluded.source_json,local_json=excluded.local_json,source_hash=excluded.source_hash,policy_revision=excluded.policy_revision",params![source,p.collection.external_id,serde_json::to_string(&p.observed)?,serde_json::to_string(&accepted)?,digest(&p.observed)?,policy_id])?;
                }
            }
        }
        drop(rows);
        drop(query);
        let result = ImportApplyResult {
            operation_id: uuid::Uuid::new_v4().to_string(),
            tracks_created: meta.summary.tracks_created,
            fields_changed: meta.summary.fields_changed,
            collections_changed: meta.summary.collections_changed,
            processing_requested: meta.summary.processing_requested,
        };
        tx.execute("INSERT INTO import_operations(id,preview_id,source_id,policy_id,state,receipt_json,created,updated) VALUES(?1,?2,?3,?4,?5,?6,?7,?7)",params![result.operation_id,preview_id,source,policy_id,if result.processing_requested{"processing_pending"}else{"imported"},serde_json::to_string(&result)?,now()])?;
        tx.execute(
            "UPDATE import_previews SET state='applied' WHERE id=?1",
            [preview_id],
        )?;
        if changed {
            Library::bump(&tx)?;
        }
        cancel.check()?;
        tx.commit()?;
        Ok(result)
    }
    pub fn discard_import_preview(&self, preview_id: &str) -> Result<()> {
        let conn = self.conn()?;
        ensure!(conn.execute("UPDATE import_previews SET state='discarded' WHERE id=?1 AND state IN ('building','ready','effects_ready','discarded')",[preview_id])?==1,"Import preview cannot be discarded after durable apply starts; use guarded recovery or Undo");
        Ok(())
    }
    pub fn import_preview_page(
        &self,
        preview_id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<ImportPreviewPage> {
        ensure!(
            (1..=1000).contains(&limit),
            "Import preview page size must be 1..=1000"
        );
        let after = if let Some(cursor) = cursor {
            let (id, sequence): (String, i64) = serde_json::from_str(cursor)?;
            ensure!(
                id == preview_id && sequence >= 0,
                "Preview cursor belongs to another preview"
            );
            sequence
        } else {
            -1
        };
        let conn = self.conn()?;
        ensure!(
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM import_previews WHERE id=?1)",
                [preview_id],
                |r| r.get::<_, bool>(0)
            )?,
            "Unknown import preview"
        );
        let total = conn.query_row(
            "SELECT COUNT(*) FROM import_preview_effects WHERE preview_id=?1",
            [preview_id],
            |r| r.get(0),
        )?;
        let mut query=conn.prepare("SELECT sequence,effect_json FROM import_preview_effects WHERE preview_id=?1 AND sequence>?2 ORDER BY sequence LIMIT ?3")?;
        let mut rows = query.query(params![preview_id, after, limit + 1])?;
        let mut effects = Vec::new();
        let mut bytes = 0u64;
        let mut last = after;
        let mut more = false;
        while let Some(row) = rows.next()? {
            let raw: String = row.get(1)?;
            ensure!(
                raw.len() as u64 <= ImportLimits::default().blob_bytes,
                "Import effect exceeds page byte limit"
            );
            if effects.len() == limit as usize
                || (!effects.is_empty()
                    && bytes + raw.len() as u64 > ImportLimits::default().blob_bytes)
            {
                more = true;
                break;
            }
            bytes += raw.len() as u64;
            effects.push(serde_json::from_str(&raw)?);
            last = row.get(0)?;
        }
        Ok(ImportPreviewPage {
            effects,
            total,
            next_cursor: if more {
                Some(serde_json::to_string(&(preview_id, last))?)
            } else {
                None
            },
        })
    }
    pub fn inspect_import_refresh(
        &self,
        source_id: &str,
        cancel: &CancellationToken,
    ) -> Result<ImportInspection> {
        let raw:String=self.conn()?.query_row("SELECT request_json FROM import_snapshots WHERE source_id=?1 ORDER BY created DESC,rowid DESC LIMIT 1",[source_id],|r|r.get(0)).context("Unknown import source")?;
        let mut request: ImportInspectRequest = serde_json::from_str(&raw)?;
        request.binding = SourceBindingChoice::Replica {
            source_id: source_id.into(),
        };
        self.inspect_import(request, cancel)
    }
}
