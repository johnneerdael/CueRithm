use super::snapshot::{Observation, Snapshot};
use super::*;
use cuerithm_smartlists::{
    CompiledSmartList, EvaluationContext, Operator, RuleValue, SmartDefinition,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Debug)]
pub(super) struct Outcome {
    pub(super) id: String,
    pub(super) members: Option<Vec<String>>,
    pub(super) error: Option<String>,
    pub(super) signature: String,
    pub(super) next: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cuerithm_smartlists::{DateUnit, SmartRule};

    #[test]
    fn prepared_previews_reject_changed_files_and_expired_clocks() {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::open(&dir.path().join("db")).unwrap();
        let path = dir.path().join("Preview test.flac");
        std::fs::write(&path, b"Authored playlist input").unwrap();
        lib.enqueue(
            std::slice::from_ref(&path),
            lib.settings().unwrap().revision,
        )
        .unwrap();
        lib.conn()
            .unwrap()
            .execute("UPDATE tracks SET state='ready'", [])
            .unwrap();
        let rules = |field: &str, operator, value| SmartDefinition {
            match_all: true,
            rules: vec![SmartRule {
                id: "r".into(),
                field: field.into(),
                operator,
                value,
                or: false,
            }],
        };
        let missing = rules("CustomFileMissing", Operator::BooleanTrue, RuleValue::None);
        let prepared = lib
            .prepare_smart_preview(missing, SmartPreviewScope::default(), 1_792_324_800_000)
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(
            lib.commit_smart_preview(prepared, 1_792_324_800_001)
                .is_err()
        );
        let recent = rules(
            "dateAdded",
            Operator::DateRecent,
            RuleValue::Relative {
                count: 1,
                unit: DateUnit::Days,
            },
        );
        let prepared = lib
            .prepare_smart_preview(recent, SmartPreviewScope::default(), 1_792_324_800_000)
            .unwrap();
        assert!(
            lib.commit_smart_preview(prepared, 1_792_324_860_000)
                .is_err()
        );
        assert_eq!(
            lib.conn()
                .unwrap()
                .query_row("SELECT COUNT(*) FROM playlist_previews", [], |r| r
                    .get::<_, u64>(0))
                .unwrap(),
            0
        );
    }
}
#[derive(Debug)]
pub struct SmartEvaluation {
    stamp: SmartStamp,
    zone: String,
    at: i64,
    expires: Option<i64>,
    outcomes: Vec<Outcome>,
    unchanged: Vec<String>,
    observations: Vec<Observation>,
    baseline: BTreeMap<String, snapshot::CachedStatus>,
}
struct PreparedSmartPreview {
    preview: SmartPreview,
    payload: serde_json::Value,
    zone: String,
    observations: Vec<Observation>,
}
pub(crate) fn allows_archive(definition: &SmartDefinition) -> bool {
    definition
        .rules
        .iter()
        .any(|r| r.field == "archived" && r.operator == Operator::BooleanTrue)
}
pub(crate) fn ordered_descendants(tree: &Tree, id: &str) -> Vec<String> {
    let mut result = vec![];
    let mut stack = vec![id.to_owned()];
    while let Some(id) = stack.pop() {
        let mut children = tree::children(tree, &Some(id.clone()), true);
        children.reverse();
        stack.extend(children);
        result.push(id);
    }
    result
}
fn membership_context(
    tree: &Tree,
    successful: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut members: BTreeMap<String, BTreeSet<String>> = tree
        .values()
        .filter(|r| !r.node.deleted && r.node.kind == PlaylistKind::Playlist)
        .map(|r| (r.node.id.clone(), r.members.iter().cloned().collect()))
        .collect();
    members.extend(
        successful
            .iter()
            .map(|(id, m)| (id.clone(), m.iter().cloned().collect())),
    );
    for row in tree
        .values()
        .filter(|r| !r.node.deleted && r.node.kind == PlaylistKind::Folder)
    {
        let mut all = BTreeSet::new();
        let mut complete = true;
        for id in ordered_descendants(tree, &row.node.id) {
            if tree[&id].node.kind == PlaylistKind::Folder {
                continue;
            }
            if let Some(m) = members.get(&id) {
                all.extend(m.iter().cloned());
            } else {
                complete = false;
                break;
            }
        }
        if complete {
            members.insert(row.node.id.clone(), all);
        }
    }
    members
}
pub(super) fn blocked(
    definition: &SmartDefinition,
    snapshot: &Snapshot,
    row: &snapshot::SnapshotTrack,
) -> Result<()> {
    for r in &definition.rules {
        if matches!(r.field.as_str(), "AnyTextField" | "EveryTextField") {
            for (field, descriptor) in &snapshot.field_catalog.catalog.fields {
                if descriptor.any_text
                    && descriptor.unavailable_reason.is_none()
                    && let Some(reason) = row.unavailable.get(field)
                {
                    anyhow::bail!(
                        "Track {} field {} unavailable: {}",
                        row.track.id,
                        field,
                        reason
                    );
                }
            }
        } else if let Some(reason) = row.unavailable.get(&r.field) {
            anyhow::bail!(
                "Track {} field {} unavailable: {}",
                row.track.id,
                r.field,
                reason
            );
        }
    }
    Ok(())
}
fn compute(
    snapshot: &Snapshot,
    tree: &Tree,
    selected: Option<(&str, &BTreeSet<String>)>,
    stop: Option<&AtomicBool>,
) -> Result<Vec<Outcome>> {
    let order = tree::dependency_order(tree)?;
    let mut successful = BTreeMap::new();
    let mut outcomes = vec![];
    let mut deadlines: BTreeMap<String, Option<i64>> = BTreeMap::new();
    let mut catalog = snapshot.field_catalog.catalog.clone();
    catalog.playlists = tree
        .values()
        .filter(|r| !r.node.deleted)
        .map(|r| (r.node.id.clone(), r.node.kind))
        .collect();
    let catalog = Arc::new(catalog);
    for id in order {
        ensure!(
            !stop.is_some_and(|s| s.load(Ordering::Acquire)),
            "Smart refresh cancelled"
        );
        let definition = tree[&id]
            .node
            .definition
            .as_ref()
            .context("Missing smart rules")?;
        let result = (|| -> Result<Vec<String>> {
            let compiled = CompiledSmartList::new(definition.clone(), catalog.clone())?;
            let mut context = EvaluationContext::new(snapshot.now, &snapshot.zone)?;
            context.key_mixing_mode = snapshot.preferences.key_mixing_mode;
            context.protected_genres = snapshot.protected.clone();
            context.memberships = membership_context(tree, &successful);
            // Resolve dependencies even when there are no candidate tracks.
            for rule in &definition.rules {
                if let RuleValue::Playlist(reference) = &rule.value {
                    ensure!(
                        context.memberships.contains_key(&reference.id),
                        "Playlist dependency unavailable: {}",
                        reference.id
                    );
                }
            }
            let mut members = vec![];
            for row in &snapshot.tracks {
                ensure!(
                    !stop.is_some_and(|s| s.load(Ordering::Acquire)),
                    "Smart refresh cancelled"
                );
                if row.track.archived && !allows_archive(definition) {
                    continue;
                }
                if let Some((target, selected)) = selected
                    && target == id
                    && !selected.contains(&row.track.id)
                {
                    continue;
                }
                blocked(definition, snapshot, row)?;
                if compiled.matches(&row.track, &context)? {
                    members.push(row.track.id.clone());
                }
            }
            Ok(members)
        })();
        let (members, error) = match result {
            Ok(m) => {
                successful.insert(id.clone(), m.clone());
                (Some(m), None)
            }
            Err(e) => (None, Some(format!("{e:#}"))),
        };
        let mut next = snapshot::dynamic(definition).then(|| snapshot.now.saturating_add(60000));
        for rule in &definition.rules {
            if let RuleValue::Playlist(reference) = &rule.value {
                let ids = if tree
                    .get(&reference.id)
                    .is_some_and(|n| n.node.kind == PlaylistKind::Folder)
                {
                    ordered_descendants(tree, &reference.id)
                } else {
                    vec![reference.id.clone()]
                };
                for dependency in ids {
                    if let Some(Some(deadline)) = deadlines.get(&dependency) {
                        next = Some(next.map_or(*deadline, |old| old.min(*deadline)));
                    }
                }
            }
        }
        deadlines.insert(id.clone(), next);
        outcomes.push(Outcome {
            id,
            members,
            error,
            signature: snapshot::signature(definition, &snapshot.zone, &snapshot.preferences)?,
            next,
        });
    }
    Ok(outcomes)
}
pub(super) fn computed_memberships(snapshot: &Snapshot) -> Result<BTreeMap<String, Outcome>> {
    Ok(compute(snapshot, &snapshot.tree, None, None)?
        .into_iter()
        .map(|o| (o.id.clone(), o))
        .collect())
}
fn cache_matches(status: &snapshot::CachedStatus, stamp: &SmartStamp, signature: &str) -> bool {
    matches!(status.state.as_str(), "ready" | "error")
        && status.input == Some(stamp.input_revision)
        && status.playlist == Some(stamp.playlist_revision)
        && status.preference == Some(stamp.preference_revision)
        && status.signature.as_deref() == Some(signature)
}
fn empty_status(id: &str, state: SmartState) -> SmartListStatus {
    SmartListStatus {
        playlist_id: id.into(),
        state,
        error: None,
        evaluated_at: None,
        next_refresh: None,
        last_good_revision: None,
    }
}
fn state_priority(state: SmartState) -> u8 {
    match state {
        SmartState::Ready => 0,
        SmartState::Pending => 1,
        SmartState::Stale => 2,
        SmartState::Error => 3,
    }
}
fn smart_status(
    conn: &Connection,
    node: &StoredNode,
    stamp: &SmartStamp,
    prefs: &PlaylistPreferences,
    zone: &str,
    now_millis: i64,
) -> Result<SmartListStatus> {
    let id = &node.node.id;
    let cached=conn.query_row("SELECT state,error,input_revision,playlist_revision,preference_revision,evaluated_at,next_refresh,last_good_revision,definition_hash FROM smart_playlist_status WHERE playlist_id=?1",[id],snapshot::CachedStatus::from_row).optional()?;
    let Some(cached) = cached else {
        return Ok(empty_status(id, SmartState::Pending));
    };
    let definition = node
        .node
        .definition
        .as_ref()
        .context("Missing smart definition")?;
    let signature = snapshot::signature(definition, zone, prefs)?;
    let fresh = cache_matches(&cached, stamp, &signature)
        && (!snapshot::dynamic(definition) || cached.next.is_some())
        && cached.next.is_none_or(|n| n > now_millis)
        && cached.at.is_some_and(|a| a <= now_millis);
    Ok(SmartListStatus {
        playlist_id: id.clone(),
        state: if cached.state == "pending" {
            SmartState::Pending
        } else if !fresh || cached.state == "stale" {
            SmartState::Stale
        } else if cached.state == "error" {
            SmartState::Error
        } else {
            SmartState::Ready
        },
        error: cached.error,
        evaluated_at: cached.at,
        next_refresh: cached.next,
        last_good_revision: cached.good,
    })
}
impl Library {
    pub(super) fn smart_lists_need_refresh(&self, now: i64) -> Result<bool> {
        let conn = self.conn()?;
        let stamp = preferences::stamp(&conn)?;
        let prefs = preferences::preferences(&conn)?.preferences;
        let zone = preferences::zone(&prefs)?;
        let _ = EvaluationContext::new(now, &zone)?;
        let mut q = conn.prepare("SELECT p.definition_json,s.state,s.error,s.input_revision,s.playlist_revision,s.preference_revision,s.evaluated_at,s.next_refresh,s.last_good_revision,s.definition_hash FROM playlists p LEFT JOIN smart_playlist_status s ON s.playlist_id=p.id WHERE p.kind='smartlist' AND p.deleted=0")?;
        for row in q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                if r.get::<_, Option<String>>(1)?.is_some() {
                    Some(snapshot::CachedStatus::from_row_at(r, 1)?)
                } else {
                    None
                },
            ))
        })? {
            let (definition, cached) = row?;
            let Some(cached) = cached else {
                return Ok(true);
            };
            let definition: SmartDefinition = serde_json::from_str(&definition)?;
            let signature = snapshot::signature(&definition, &zone, &prefs)?;
            if !cache_matches(&cached, &stamp, &signature)
                || (snapshot::dynamic(&definition) && cached.next.is_none())
                || cached.next.is_some_and(|n| n <= now)
                || cached.at.is_none_or(|at| at > now)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn evaluate_smart_lists(&self, now_millis: i64) -> Result<SmartEvaluation> {
        self.evaluate_smart_lists_cancellable(now_millis, None)
    }
    pub(super) fn evaluate_smart_lists_cancellable(
        &self,
        now_millis: i64,
        stop: Option<&AtomicBool>,
    ) -> Result<SmartEvaluation> {
        let snapshot = self.smart_snapshot_cancellable(now_millis, None, true, stop)?;
        let stamp = snapshot.field_catalog.stamp.clone();
        let mut ids = vec![];
        let mut dirty = false;
        for row in snapshot
            .tree
            .values()
            .filter(|r| !r.node.deleted && r.node.kind == PlaylistKind::Smartlist)
        {
            let definition = row.node.definition.as_ref().unwrap();
            ids.push(row.node.id.clone());
            let signature = snapshot::signature(definition, &snapshot.zone, &snapshot.preferences)?;
            dirty |= snapshot.statuses.get(&row.node.id).is_none_or(|s| {
                !cache_matches(s, &stamp, &signature)
                    || (snapshot::dynamic(definition) && s.next.is_none())
                    || s.next.is_some_and(|next| next <= now_millis)
                    || s.at.is_none_or(|at| at > now_millis)
            });
        }
        let outcomes = if dirty {
            compute(&snapshot, &snapshot.tree, None, stop)?
        } else {
            vec![]
        };
        ensure!(
            !stop.is_some_and(|s| s.load(Ordering::Acquire)),
            "Smart refresh cancelled"
        );
        let expires = outcomes.iter().filter_map(|o| o.next).min();
        Ok(SmartEvaluation {
            stamp,
            zone: snapshot.zone,
            at: now_millis,
            expires,
            outcomes,
            unchanged: if dirty { vec![] } else { ids },
            observations: snapshot.observations,
            baseline: snapshot.statuses,
        })
    }
    pub fn commit_smart_evaluation(
        &self,
        evaluation: SmartEvaluation,
        now_millis: i64,
    ) -> Result<SmartRefresh> {
        ensure!(
            now_millis >= evaluation.at && evaluation.expires.is_none_or(|end| now_millis < end),
            "Smart evaluation expired; retry with the current clock"
        );
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Metadata access interrupted"))?;
        for observation in &evaluation.observations {
            ensure!(
                snapshot::observe(&observation.path) == *observation,
                "File availability changed during smart evaluation"
            );
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(
            preferences::stamp(&tx)? == evaluation.stamp,
            "Smart inputs changed; retry evaluation"
        );
        let prefs = preferences::preferences(&tx)?;
        ensure!(
            preferences::zone(&prefs.preferences)? == evaluation.zone,
            "Time zone changed; retry evaluation"
        );
        let mut visible_changed = false;
        let mut evaluated = vec![];
        for outcome in &evaluation.outcomes {
            let old = tx.query_row("SELECT state,error,input_revision,playlist_revision,preference_revision,evaluated_at,next_refresh,last_good_revision,definition_hash FROM smart_playlist_status WHERE playlist_id=?1", [&outcome.id], snapshot::CachedStatus::from_row).optional()?;
            ensure!(
                old.as_ref() == evaluation.baseline.get(&outcome.id),
                "A different smart evaluation is already published; retry from its snapshot"
            );
            let mut q=tx.prepare("SELECT track_id FROM smart_playlist_members WHERE playlist_id=?1 ORDER BY position")?;
            let old_members: Vec<String> = q
                .query_map([&outcome.id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            drop(q);
            let state = if outcome.error.is_some() {
                "error"
            } else {
                "ready"
            };
            let membership_changed = outcome.members.as_ref().is_some_and(|m| m != &old_members);
            visible_changed |= membership_changed
                || old
                    .as_ref()
                    .is_none_or(|s| s.state != state || s.error != outcome.error);
            if let Some(members) = &outcome.members
                && membership_changed
            {
                tx.execute(
                    "DELETE FROM smart_playlist_members WHERE playlist_id=?1",
                    [&outcome.id],
                )?;
                for (position, id) in members.iter().enumerate() {
                    tx.execute(
                        "INSERT INTO smart_playlist_members VALUES(?1,?2,?3)",
                        params![outcome.id, id, position as u64],
                    )?;
                }
            }
            let last_good = if outcome.members.is_some() {
                Some(evaluation.stamp.input_revision)
            } else {
                old.as_ref().and_then(|s| s.good)
            };
            tx.execute("INSERT INTO smart_playlist_status(playlist_id,state,error,input_revision,playlist_revision,preference_revision,evaluated_at,next_refresh,last_good_revision,definition_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(playlist_id) DO UPDATE SET state=excluded.state,error=excluded.error,input_revision=excluded.input_revision,playlist_revision=excluded.playlist_revision,preference_revision=excluded.preference_revision,evaluated_at=excluded.evaluated_at,next_refresh=excluded.next_refresh,last_good_revision=excluded.last_good_revision,definition_hash=excluded.definition_hash",params![outcome.id,state,outcome.error,evaluation.stamp.input_revision,evaluation.stamp.playlist_revision,evaluation.stamp.preference_revision,evaluation.at,outcome.next,last_good,outcome.signature])?;
            evaluated.push(outcome.id.clone());
        }
        if visible_changed {
            Library::bump(&tx)?;
        }
        let mut q=tx.prepare("SELECT s.playlist_id,s.error FROM smart_playlist_status s JOIN playlists p ON p.id=s.playlist_id WHERE p.deleted=0 AND s.state='error' ORDER BY s.playlist_id")?;
        let errors = q
            .query_map([], |r| {
                Ok(SmartListFailure {
                    playlist_id: r.get(0)?,
                    message: r
                        .get::<_, Option<String>>(1)?
                        .unwrap_or_else(|| "Smart evaluation failed".into()),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(q);
        let result = SmartRefresh {
            stamp: evaluation.stamp,
            evaluated,
            unchanged: evaluation.unchanged,
            errors,
        };
        tx.commit()?;
        Ok(result)
    }
    pub fn refresh_smart_lists(&self, now_millis: i64) -> Result<SmartRefresh> {
        let started = std::time::Instant::now();
        let evaluation = self.evaluate_smart_lists(now_millis)?;
        self.commit_smart_evaluation(
            evaluation,
            now_millis.saturating_add(started.elapsed().as_millis().min(i64::MAX as u128) as i64),
        )
    }
    pub fn playlist_evaluation_status(&self, id: &str) -> Result<SmartListStatus> {
        self.playlist_evaluation_status_at(id, crate::now())
    }
    pub fn playlist_evaluation_status_at(
        &self,
        id: &str,
        now_millis: i64,
    ) -> Result<SmartListStatus> {
        let conn = self.conn()?;
        let tree = load(&conn)?;
        let node = tree::active(&tree, id)?;
        let stamp = preferences::stamp(&conn)?;
        let prefs = preferences::preferences(&conn)?.preferences;
        let zone = preferences::zone(&prefs)?;
        let mut result = empty_status(id, SmartState::Ready);
        if node.node.kind == PlaylistKind::Smartlist {
            return smart_status(&conn, node, &stamp, &prefs, &zone, now_millis);
        }
        if node.node.kind == PlaylistKind::Folder {
            for child in ordered_descendants(&tree, id)
                .iter()
                .filter_map(|id| tree.get(id))
                .filter(|n| n.node.kind == PlaylistKind::Smartlist)
            {
                let child_status = smart_status(&conn, child, &stamp, &prefs, &zone, now_millis)?;
                if state_priority(child_status.state) > state_priority(result.state) {
                    result.state = child_status.state;
                }
                if let Some(error) = child_status.error {
                    let error = format!("{}: {}", child.node.name, error);
                    result.error = Some(match result.error {
                        Some(previous) => format!("{previous}\n{error}"),
                        None => error,
                    });
                }
                if let Some(next) = child_status.next_refresh {
                    result.next_refresh =
                        Some(result.next_refresh.map_or(next, |old| old.min(next)));
                }
            }
        }
        Ok(result)
    }
    pub fn preview_smart_list(
        &self,
        definition: SmartDefinition,
        scope: SmartPreviewScope,
        now_millis: i64,
    ) -> Result<SmartPreview> {
        let started = std::time::Instant::now();
        let prepared = self.prepare_smart_preview(definition, scope, now_millis)?;
        self.commit_smart_preview(
            prepared,
            now_millis.saturating_add(started.elapsed().as_millis().min(i64::MAX as u128) as i64),
        )
    }
    fn prepare_smart_preview(
        &self,
        definition: SmartDefinition,
        scope: SmartPreviewScope,
        now_millis: i64,
    ) -> Result<PreparedSmartPreview> {
        definition.validate_shape()?;
        let snapshot = self.smart_snapshot(now_millis, Some(&definition), true)?;
        let mut tree = snapshot.tree.clone();
        let target = scope
            .playlist_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if scope.playlist_id.is_some() {
            ensure!(
                tree::active(&tree, &target)?.node.kind == PlaylistKind::Smartlist,
                "Preview owner is not a smart playlist"
            );
            tree.get_mut(&target).unwrap().node.definition = Some(definition.clone());
        } else {
            tree.insert(
                target.clone(),
                StoredNode {
                    last_edit_id: String::new(),
                    members: vec![],
                    node: PlaylistNode {
                        id: target.clone(),
                        parent_id: None,
                        kind: PlaylistKind::Smartlist,
                        name: "Preview".into(),
                        position: u32::MAX,
                        favorite_position: None,
                        definition: Some(definition.clone()),
                        deleted: false,
                        deleted_by: None,
                    },
                },
            );
        }
        let selected = scope
            .track_ids
            .as_ref()
            .map(|ids| ids.iter().cloned().collect::<BTreeSet<_>>());
        if let Some(ids) = &selected {
            ensure!(
                ids.iter().all(|id| snapshot.all_ids.contains(id)),
                "Unknown selected track ID"
            );
        }
        let results = compute(
            &snapshot,
            &tree,
            selected.as_ref().map(|ids| (target.as_str(), ids)),
            None,
        )?;
        let outcome = results
            .into_iter()
            .find(|r| r.id == target)
            .context("Preview evaluation missing")?;
        let members = outcome.members.ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                outcome
                    .error
                    .unwrap_or_else(|| "Smart preview failed".into())
            )
        })?;
        let id = uuid::Uuid::new_v4().to_string();
        let preview = SmartPreview {
            id: id.clone(),
            track_ids: members,
            stamp: snapshot.field_catalog.stamp.clone(),
            created: now_millis,
            expires: outcome.next,
        };
        let payload = serde_json::json!({"definition":definition,"scope":scope,"preview":preview,"zone":snapshot.zone,"observations":snapshot.observations});
        Ok(PreparedSmartPreview {
            preview,
            payload,
            zone: snapshot.zone,
            observations: snapshot.observations,
        })
    }
    fn commit_smart_preview(
        &self,
        prepared: PreparedSmartPreview,
        now_millis: i64,
    ) -> Result<SmartPreview> {
        let PreparedSmartPreview {
            preview,
            payload,
            zone,
            observations,
        } = prepared;
        ensure!(
            now_millis >= preview.created && preview.expires.is_none_or(|end| now_millis < end),
            "Smart preview expired; retry with the current clock"
        );
        #[cfg(feature = "autotag")]
        let _gate = self
            .inner
            .tag_gate
            .lock()
            .map_err(|_| anyhow::anyhow!("Metadata access interrupted"))?;
        for observation in &observations {
            ensure!(
                snapshot::observe(&observation.path) == *observation,
                "File availability changed during smart preview"
            );
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(
            preferences::stamp(&tx)? == preview.stamp,
            "Smart inputs changed during preview"
        );
        ensure!(
            preferences::zone(&preferences::preferences(&tx)?.preferences)? == zone,
            "Time zone changed during smart preview"
        );
        tx.execute(
            "INSERT INTO playlist_previews VALUES(?1,'smart',?2,?3,?4,?5,?6,?7)",
            params![
                preview.id,
                payload.to_string(),
                preview.stamp.playlist_revision,
                preview.stamp.input_revision,
                preview.stamp.preference_revision,
                preview.created,
                preview.expires
            ],
        )?;
        tx.commit()?;
        Ok(preview)
    }
}
use rusqlite::OptionalExtension;
