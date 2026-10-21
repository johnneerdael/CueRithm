use super::storage::*;
use super::*;
use crate::model::now;
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use std::collections::BTreeSet;
fn patch(
    before: &CurationValues,
    patch: &CurationPatch,
    catalog: &CurationCatalog,
) -> Result<CurationValues> {
    let mut after = before.clone();
    let mut seen = BTreeSet::new();
    for edit in &patch.groups {
        let (group_id, values) = match edit {
            CurationGroupEdit::Set {
                group_id,
                value_ids,
            }
            | CurationGroupEdit::Add {
                group_id,
                value_ids,
            }
            | CurationGroupEdit::Remove {
                group_id,
                value_ids,
            } => (group_id, Some(value_ids)),
            CurationGroupEdit::Clear { group_id } => (group_id, None),
        };
        ensure!(
            seen.insert(group_id),
            "One operation per category group is required"
        );
        let group = catalog
            .groups
            .iter()
            .find(|group| group.id == *group_id)
            .context("Unknown or retired category group")?;
        if let Some(values) = values {
            ensure!(
                values.len() <= 4096
                    && values.iter().collect::<BTreeSet<_>>().len() == values.len(),
                "Choose unique category values"
            );
            for value in values {
                ensure!(
                    group.values.iter().any(|candidate| candidate.id == *value),
                    "Value does not belong to this active category"
                );
            }
        }
        let selected = after.categories.entry(group_id.clone()).or_default();
        match edit {
            CurationGroupEdit::Set { value_ids, .. } => *selected = value_ids.clone(),
            CurationGroupEdit::Add { value_ids, .. } => {
                if group.mode == CurationGroupMode::Single && !value_ids.is_empty() {
                    *selected = value_ids.clone()
                } else {
                    selected.extend(value_ids.clone());
                }
            }
            CurationGroupEdit::Remove { value_ids, .. } => {
                selected.retain(|id| !value_ids.contains(id))
            }
            CurationGroupEdit::Clear { .. } => selected.clear(),
        };
        selected.sort();
        selected.dedup();
        ensure!(
            group.mode != CurationGroupMode::Single || selected.len() <= 1,
            "Single-select category accepts at most one value"
        );
        if selected.is_empty() {
            after.categories.remove(group_id);
        }
    }
    match &patch.user_rating {
        CurationValueEdit::Keep => {}
        CurationValueEdit::Set { value } => {
            ensure!(
                (1..=5).contains(value),
                "User rating is1..5; use Clear to unset it"
            );
            after.user_rating = Some(*value)
        }
        CurationValueEdit::Clear => after.user_rating = None,
    };
    match &patch.notes {
        CurationValueEdit::Keep => {}
        CurationValueEdit::Set { value } => {
            ensure!(
                value.len() <= 65536 && !value.contains('\0'),
                "Curate notes exceed64KiB or contain NUL"
            );
            after.notes = value.clone()
        }
        CurationValueEdit::Clear => after.notes.clear(),
    };
    ensure!(
        after.categories.values().map(Vec::len).sum::<usize>() <= 4096,
        "Too many assigned category values"
    );
    Ok(after)
}
pub(super) fn snapshot(service: &CurationService, ids: &[String]) -> Result<CurationSnapshot> {
    ensure!(
        ids.len() <= 100_000 && ids.iter().collect::<BTreeSet<_>>().len() == ids.len(),
        "Curate selection requires unique tracks and at most100000 IDs"
    );
    let revision = { tax_revision(&*service.library.conn()?)? };
    let mut tracks = vec![];
    let mut bytes = 0usize;
    for id in ids {
        let row = public(head(&service.library, id)?);
        bytes = bytes.saturating_add(json(&row)?.len());
        ensure!(
            bytes <= MAX_JSON,
            "Complete Curate snapshot exceeds64MiB; narrow selection"
        );
        tracks.push(row);
    }
    let mut mixed = vec![];
    if let Some(first) = tracks.first() {
        if tracks
            .iter()
            .any(|row| row.desired.categories != first.desired.categories)
        {
            mixed.push("categories".into());
        }
        if tracks
            .iter()
            .any(|row| row.desired.user_rating != first.desired.user_rating)
        {
            mixed.push("user_rating".into());
        }
        if tracks
            .iter()
            .any(|row| row.desired.notes != first.desired.notes)
        {
            mixed.push("notes".into());
        }
    }
    Ok(CurationSnapshot {
        taxonomy_revision: revision,
        library_revision: service.library.revision()?,
        tracks,
        mixed,
    })
}
pub(super) fn update(
    service: &CurationService,
    request: CurationUpdateRequest,
    reexport: bool,
) -> Result<CurationReceipt> {
    valid_request(&request.request_id)?;
    ensure!(
        !request.tracks.is_empty()
            && request.tracks.len() <= 100_000
            && request
                .tracks
                .iter()
                .map(|row| &row.track_id)
                .collect::<BTreeSet<_>>()
                .len()
                == request.tracks.len(),
        "Choose unique Curate tracks; selection exceeds100000 tracks"
    );
    let hash = digest(&(reexport, &request))?;
    if let Some(receipt) = mutation(&service.library, &request.request_id, "update", &hash)? {
        return Ok(serde_json::from_str(&receipt)?);
    }
    let catalog = service.catalog()?;
    ensure!(
        catalog.revision == request.taxonomy_revision,
        "Category catalog changed; refresh before editing"
    );
    let mut prepared = vec![];
    let mut accepted = vec![];
    let mut budget = 0usize;
    for expected in &request.tracks {
        let before = head(&service.library, &expected.track_id)?;
        ensure!(
            before.revision == expected.revision,
            "Curate draft changed; reload before editing"
        );
        let after = patch(&before.desired, &request.patch, &catalog)?;
        let changed = after != before.desired || reexport;
        let revision = before.revision + u64::from(changed);
        let desired = json(&after)?;
        let committed = json(&before.committed)?;
        let input = json(if before.active.is_some() {
            &before.input
        } else {
            &before.current_input
        })?;
        // A queued edit follows the already owned in-flight result, even when
        // that result has not acknowledged its library projection yet.
        let predecessor = before
            .active
            .as_ref()
            .map(|id| super::publication::load(service, id))
            .transpose()?;
        let mut frozen = super::mapping::freeze(
            &catalog,
            &after,
            predecessor
                .as_ref()
                .map(|p| &p.public.after)
                .unwrap_or(&before.committed),
            reexport,
        )?;
        if reexport {
            for mut old in super::mapping::owned(service, before.last_native.clone())? {
                if !frozen.iter().any(|current| {
                    super::taxonomy::target_key(&current.mapping).ok()
                        == super::taxonomy::target_key(&old.mapping).ok()
                        && old
                            .mapping
                            .containers
                            .iter()
                            .all(|family| current.mapping.containers.contains(family))
                }) {
                    old.values.clear();
                    frozen.insert(0, old);
                }
            }
        }
        let mapping_json = json(&frozen)?;
        let native_required = !frozen.is_empty();
        budget = budget.saturating_add(desired.len() + committed.len());
        ensure!(
            budget <= MAX_JSON,
            "Complete Curate update exceeds64MiB; narrow selection"
        );
        accepted.push(CurationAccepted {
            track_id: expected.track_id.clone(),
            revision,
        });
        prepared.push((
            before,
            desired,
            committed,
            input,
            changed,
            revision,
            mapping_json,
            native_required,
        ));
    }
    let receipt = CurationReceipt {
        request_id: request.request_id.clone(),
        accepted,
        library_revision: service.library.revision()?,
    };
    let encoded = json(&receipt)?;
    let mut conn = service.library.conn()?;
    let tx = conn.transaction()?;
    ensure!(
        tax_revision(&tx)? == request.taxonomy_revision,
        "Category catalog changed while saving draft"
    );
    for (before, desired, committed, input, changed, revision, mapping_json, native_required) in
        &prepared
    {
        let current: Option<u64> = tx
            .query_row(
                "SELECT revision FROM curation_tracks WHERE track_id=?1",
                [&before.track_id],
                |row| row.get(0),
            )
            .optional()?;
        ensure!(
            current.unwrap_or(0) == before.revision,
            "Curate draft changed while saving"
        );
        ensure!(
            tx.query_row(
                "SELECT state IN ('ready','incoming') AND archived=0 FROM tracks WHERE id=?1",
                [&before.track_id],
                |row| row.get::<_, bool>(0)
            )?,
            "Curate track is no longer active"
        );
        if *changed {
            tx.execute("INSERT INTO curation_tracks(track_id,revision,acknowledged_revision,desired_json,committed_json,state,error,active_publication_id,last_publication_id,native_stale,input_json,updated,native_required,pending_mappings_json,pending_taxonomy_revision,pending_reexport)VALUES(?1,?2,?3,?4,?5,'pending',NULL,NULL,NULL,?8,?6,?7,?9,?10,?11,?12)ON CONFLICT(track_id)DO UPDATE SET revision=excluded.revision,desired_json=excluded.desired_json,state=CASE WHEN curation_tracks.active_publication_id IS NULL THEN 'pending' ELSE curation_tracks.state END,error=NULL,updated=excluded.updated,native_required=excluded.native_required,pending_mappings_json=excluded.pending_mappings_json,pending_taxonomy_revision=excluded.pending_taxonomy_revision,pending_reexport=excluded.pending_reexport,input_json=CASE WHEN curation_tracks.active_publication_id IS NULL AND curation_tracks.acknowledged_revision=?13 THEN excluded.input_json ELSE curation_tracks.input_json END,input_proof_json=CASE WHEN curation_tracks.active_publication_id IS NULL AND curation_tracks.acknowledged_revision=?13 THEN NULL ELSE curation_tracks.input_proof_json END",params![before.track_id,revision,before.acknowledged,desired,committed,input,now(),before.native_stale,native_required,mapping_json,catalog.revision,reexport,before.acknowledged])?;
        }
    }
    save_mutation(&tx, &request.request_id, "update", &hash, &encoded)?;
    tx.commit()?;
    drop(conn);
    for row in &receipt.accepted {
        service.emit(CurationEvent {
            track_id: Some(row.track_id.clone()),
            revision: row.revision,
            state: "pending".into(),
        });
    }
    service.wake();
    Ok(receipt)
}
pub(super) fn discard(
    service: &CurationService,
    track: &str,
    revision: u64,
) -> Result<CurationTrackState> {
    let _lease = service.publication_lease()?;
    let (current_revision, committed, active, archived): (u64,String,Option<String>,bool) = service.library.conn()?.query_row("SELECT c.revision,c.committed_json,c.active_publication_id,t.archived FROM curation_tracks c JOIN tracks t ON t.id=c.track_id WHERE c.track_id=?1",[track],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
    ensure!(!archived, "Restore this archived track before Discard");
    ensure!(
        current_revision == revision,
        "Curate draft changed; reload before discarding"
    );
    ensure!(
        discardable(&*service.library.conn()?, active.as_deref())?,
        "Published or running native work must finish or recover before Discard; use Undo for published work"
    );
    ensure!(
        committed.len() <= MAX_JSON,
        "Curate committed evidence exceeds bounded size"
    );
    let mut committed: CurationValues = serde_json::from_str(&committed)?;
    committed.categories = categories(&*service.library.conn()?, track)?;
    let text = json(&committed)?;
    let mut retired_child = None;
    {
        let _org = service
            .library
            .inner
            .organization_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Organization gate interrupted"))?;
        #[cfg(feature = "autotag")]
        let _tag = service
            .library
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Publication gate interrupted"))?;
        let child: Option<String> = active
            .as_ref()
            .map(|id| {
                service
                    .library
                    .conn()?
                    .query_row(
                        "SELECT child_batch_id FROM curation_publications WHERE id=?1",
                        [id],
                        |row| row.get(0),
                    )
                    .map_err(anyhow::Error::from)
            })
            .transpose()?
            .flatten();
        if let Some(child) = &child {
            #[cfg(feature = "autotag")]
            service.library.verify_unpublished_curation_child(child)?;
            #[cfg(not(feature = "autotag"))]
            anyhow::bail!("Native Curate recovery requires native tagging support: {child}");
        }
        let mut conn = service.library.conn()?;
        let tx = conn.transaction()?;
        ensure!(
            discardable(&tx, active.as_deref())?,
            "Native publication advanced before Discard"
        );
        ensure!(tx.execute("UPDATE curation_tracks SET revision=revision+1,acknowledged_revision=revision+1,desired_json=?1,committed_json=?1,state='saved',error=NULL,active_publication_id=NULL,updated=?2 WHERE track_id=?3 AND revision=?4 AND active_publication_id IS ?5",params![text,now(),track,revision,active])?==1,"Curate draft changed before Discard");
        if let Some(active) = &active {
            tx.execute(
                "UPDATE curation_publications SET state='discarded',error=NULL WHERE id=?1",
                [active],
            )?;
            if let Some(child) = child {
                retired_child = Some(child.clone());
                tx.execute("UPDATE metadata_edit_overrides SET state='discarded' WHERE batch_id=?1 AND state IN ('queued','claimed')",[&child])?;
                tx.execute("UPDATE metadata_edit_writes SET state='failed',error='Unpublished Curate intent discarded' WHERE batch_id=?1 AND state NOT IN ('committed','undone')",[&child])?;
                tx.execute("UPDATE metadata_edit_rows SET state='cancelled',error='Unpublished Curate intent discarded' WHERE batch_id=?1 AND state NOT IN ('written','undone')",[&child])?;
                tx.execute(
                    "UPDATE metadata_edit_batches SET state='cancelled' WHERE id=?1",
                    [&child],
                )?;
            }
            tx.execute("UPDATE metadata_edit_previews SET state='discarded' WHERE draft_id IN (SELECT draft_id FROM curation_metadata_drafts WHERE publication_id=?1) AND state IN ('preview','stale','retired')",[active])?;
            tx.execute("UPDATE metadata_edit_drafts SET state='discarded' WHERE id IN (SELECT draft_id FROM curation_metadata_drafts WHERE publication_id=?1)",[active])?;
        }
        tx.commit()?;
    }
    #[cfg(feature = "autotag")]
    {
        if let Some(child) = retired_child {
            service.library.retire_unwritten_metadata_stages(&child)?;
        }
        service.library.retire_metadata_previews()?;
    }
    #[cfg(not(feature = "autotag"))]
    let _ = retired_child;
    let result = public(head(&service.library, track)?);
    service.emit(CurationEvent {
        track_id: Some(track.into()),
        revision: result.desired_revision,
        state: "saved".into(),
    });
    Ok(result)
}
pub(super) fn pending(
    service: &CurationService,
    offset: usize,
    limit: usize,
) -> Result<CurationPendingPage> {
    page(offset, limit)?;
    let (total, rows) = {
        let conn = service.library.conn()?;
        let condition = "(c.revision!=c.acknowledged_revision OR c.active_publication_id IS NOT NULL OR c.native_stale=1 OR c.state IN ('error','recovery_required','waiting_incoming'))";
        let total = conn.query_row(
            &format!("SELECT (SELECT COUNT(*) FROM curation_tracks c WHERE {condition})+(SELECT COUNT(*) FROM curation_native_stale s WHERE NOT EXISTS(SELECT 1 FROM curation_tracks c WHERE c.track_id=s.track_id))"),
            [],
            |row| row.get(0),
        )?;
        let mut query=conn.prepare(&format!("SELECT track_id,title,revision,acknowledged_revision,state,error,native_stale,active_publication_id,archived FROM (SELECT c.track_id,t.title,c.revision,c.acknowledged_revision,c.state,c.error,c.native_stale,c.active_publication_id,c.updated,t.archived FROM curation_tracks c JOIN tracks t ON t.id=c.track_id WHERE {condition} UNION ALL SELECT t.id,t.title,0,0,'saved',NULL,1,NULL,0,t.archived FROM curation_native_stale s JOIN tracks t ON t.id=s.track_id WHERE NOT EXISTS(SELECT 1 FROM curation_tracks c WHERE c.track_id=s.track_id)) ORDER BY updated DESC,track_id LIMIT ?1 OFFSET ?2"))?;
        let rows = query
            .query_map(params![limit, offset], |row| {
                let state: String = row.get(4)?;
                let active: Option<String> = row.get(7)?;
                let revision: u64 = row.get(2)?;
                let acknowledged: u64 = row.get(3)?;
                let archived: bool = row.get(8)?;
                Ok(CurationPending {
                    track_id: row.get(0)?,
                    title: row.get(1)?,
                    desired_revision: revision,
                    acknowledged_revision: acknowledged,
                    can_retry: !archived
                        && matches!(
                            state.as_str(),
                            "error" | "recovery_required" | "waiting_incoming"
                        ),
                    can_discard: !archived
                        && discardable(&conn, active.as_deref()).unwrap_or(false)
                        && revision > acknowledged,
                    state,
                    error: row.get(5)?,
                    native_stale: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        (total, rows)
    };
    Ok(CurationPendingPage {
        total,
        offset,
        limit,
        rows,
    })
}
