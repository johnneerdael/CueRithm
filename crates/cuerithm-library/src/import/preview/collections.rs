use super::super::collections::{
    inline_collection_value, local_collection_value, source_collection_value,
};
use super::*;
pub(super) struct Context<'a> {
    pub request: &'a ImportPreviewRequest,
    pub source: &'a str,
    pub header: &'a ObservationHeader,
    pub profile: &'a SourceProfile,
    pub preview_id: &'a str,
}
pub(super) fn stage_collections(
    tx: &Connection,
    context: Context<'_>,
    summary: &mut ImportPreview,
    effect_sequence: &mut i64,
    cancel: &CancellationToken,
) -> Result<()> {
    let Context {
        request,
        source,
        header,
        profile,
        preview_id,
    } = context;
    if !request
        .policy
        .selected_fields
        .contains(&FieldGroup::Collections)
    {
        return Ok(());
    }
    let mut query=tx.prepare("SELECT json FROM import_observations WHERE snapshot_id=?1 AND kind='collection' ORDER BY sequence")?;
    let mut rows = query.query([&request.inspection_id])?;
    while let Some(row) = rows.next()? {
        cancel.check()?;
        let ObservationRecord::Collection(collection) =
            serde_json::from_str(&row.get::<_, String>(0)?)?
        else {
            unreachable!()
        };
        if !request.collections.contains(&collection.external_id) {
            continue;
        }
        let id = collection_id(source, &collection.external_id)?;
        let observed = source_collection_value(tx, &request.inspection_id, &collection, cancel)?;
        let before = local_collection_value(tx, &id, cancel)?;
        let applied = baseline(tx, source, &collection.external_id, "collection")?;
        let choice = decision(request, &collection.external_id, "collection");
        let custom = if let Some(ImportDecision::Custom(value)) = choice {
            let custom: CollectionObservation = serde_json::from_value(value.clone())?;
            ensure!(
                custom.external_id == collection.external_id,
                "Custom collection cannot change source identity"
            );
            let mut ids = std::collections::BTreeSet::new();
            for member in &custom.occurrences {
                cancel.check()?;
                ensure!(
                    ids.insert(&member.external_occurrence_id),
                    "Duplicate custom occurrence ID"
                );
                ensure!(tx.query_row("SELECT EXISTS(SELECT 1 FROM import_observations WHERE snapshot_id=?1 AND kind='track' AND record_id=?2)",params![request.inspection_id,member.track_external_id],|r|r.get::<_,bool>(0))?,"Unknown custom collection member");
            }
            Some(custom)
        } else {
            None
        };
        let canonical_choice = custom
            .as_ref()
            .map(inline_collection_value)
            .transpose()?
            .map(ImportDecision::Custom);
        let (mut proposed, conflict) = choose_field(
            &observed,
            before.as_ref(),
            applied.as_ref(),
            canonical_choice.as_ref().or(choice),
        );
        let blocked_reason = if matches!(header.completeness, Completeness::PartialWithErrors) {
            proposed = None;
            Some("Collection membership is incomplete; preserve the applied collection".into())
        } else if !profile.fields.get("collections").is_some_and(|c| c.read) {
            proposed = None;
            Some("This source profile does not qualify collection reads".into())
        } else {
            None
        };
        let apply = proposed
            .as_ref()
            .is_some_and(|v| Some(v) != before.as_ref());
        let baseline_changed = proposed
            .as_ref()
            .is_some_and(|v| applied.as_ref() != Some(&(observed.clone(), v.clone())));
        summary.collections_changed += u64::from(apply);
        summary.baselines_changed += u64::from(baseline_changed);
        summary.conflicts += u64::from(conflict);
        summary.blocked += u64::from(blocked_reason.is_some());
        let inline = custom.is_some();
        let mut accepted_collection = custom.unwrap_or(collection);
        if !inline {
            accepted_collection.occurrences.clear();
        }
        let parent_id = accepted_collection
            .parent_id
            .as_deref()
            .map(|p| collection_id(source, p))
            .transpose()?;
        if let Some(parent) = &accepted_collection.parent_id {
            ensure!(tx.query_row("SELECT EXISTS(SELECT 1 FROM import_observations WHERE snapshot_id=?1 AND kind='collection' AND record_id=?2) OR EXISTS(SELECT 1 FROM import_collections WHERE source_id=?3 AND external_id=?2)",params![request.inspection_id,parent,source],|r|r.get::<_,bool>(0))?,"Unknown custom collection parent");
        }
        let effect = ImportPreviewEffect {
            external_id: accepted_collection.external_id.clone(),
            item_id: id.clone(),
            kind: "collection".into(),
            fields: vec![ImportFieldEffect {
                field: "collection".into(),
                source: Some(observed.clone()),
                local: before,
                proposed: proposed.clone(),
                conflict,
                blocked_reason,
            }],
        };
        let proposal = CollectionProposal {
            id,
            collection: accepted_collection,
            parent_id,
            apply,
            inline,
            observed,
            accepted: proposed,
            baseline_changed,
        };
        tx.execute(
            "INSERT INTO import_preview_effects VALUES(?1,?2,'collection',?3,?4,?5)",
            params![
                preview_id,
                *effect_sequence,
                effect.external_id,
                serde_json::to_string(&proposal)?,
                serde_json::to_string(&effect)?
            ],
        )?;
        *effect_sequence += 1;
    }
    cancel.check()?;
    // Validate the resulting local hierarchy, including kept local parents and custom
    // decisions, instead of validating only the incoming source hierarchy.
    let invalid:bool=tx.query_row("WITH RECURSIVE planned(id,parent_id) AS (SELECT json_extract(proposal_json,'$.id'),json_extract(proposal_json,'$.parent_id') FROM import_preview_effects WHERE preview_id=?1 AND kind='collection' AND json_extract(proposal_json,'$.apply')=1), graph(id,parent_id) AS (SELECT id,parent_id FROM planned UNION ALL SELECT id,parent_id FROM import_collections WHERE source_id=?2 AND id NOT IN (SELECT id FROM planned)), walk(id,parent_id,depth) AS (SELECT id,parent_id,1 FROM graph UNION ALL SELECT w.id,g.parent_id,w.depth+1 FROM walk w JOIN graph g ON g.id=w.parent_id WHERE w.depth<=128) SELECT EXISTS(SELECT 1 FROM walk WHERE depth>128)",params![preview_id,source],|r|r.get(0))?;
    ensure!(
        !invalid,
        "Applied collection hierarchy contains a cycle or exceeds depth 128"
    );
    Ok(())
}
