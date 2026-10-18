use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Guard {
    pub stamp: SmartStamp,
    pub data_bound: bool,
    pub zone: Option<String>,
    pub observations: Vec<snapshot::Observation>,
    pub created: i64,
    pub expires: Option<i64>,
    pub engine: String,
}
#[derive(Serialize, Deserialize)]
struct SavedPlan {
    guard: Guard,
    before: Delta,
    after: Delta,
    created_ids: Vec<String>,
}
fn validate(tx: &rusqlite::Transaction<'_>, guard: &Guard, now: i64) -> Result<()> {
    ensure!(
        guard.engine == env!("CUERITHM_SMART_ENGINE_ID"),
        "Playlist logic changed; preview again"
    );
    ensure!(
        now >= guard.created && guard.expires.is_none_or(|end| now < end),
        "Playlist preview expired; preview again"
    );
    let current = preferences::stamp(tx)?;
    ensure!(
        current.playlist_revision == guard.stamp.playlist_revision,
        "Playlists changed; preview again"
    );
    if guard.data_bound {
        ensure!(
            current == guard.stamp,
            "Library inputs changed; preview again"
        );
        ensure!(
            Some(preferences::zone(
                &preferences::preferences(tx)?.preferences
            )?) == guard.zone,
            "Time zone changed; preview again"
        );
        for observed in &guard.observations {
            ensure!(
                snapshot::observe(&observed.path) == *observed,
                "File availability changed; preview again"
            );
        }
    }
    Ok(())
}
pub(super) fn save(
    lib: &Library,
    kind: &str,
    guard: Guard,
    before: &Tree,
    after: &Tree,
    created_ids: Vec<String>,
) -> Result<PlaylistToolPreview> {
    let (old, new) = diff(before, after);
    let mut changes = vec![];
    let mut stack = tree::children(after, &None, true);
    stack.reverse();
    while let Some(id) = stack.pop() {
        let row = &after[&id];
        if new.contains_key(&id) {
            changes.push(PlaylistPreviewItem {
                id: id.clone(),
                name: row.node.name.clone(),
                previous_name: before.get(&id).map(|r| r.node.name.clone()),
                kind: row.node.kind,
                parent_id: row.node.parent_id.clone(),
                position: row.node.position,
                track_count: row.members.len(),
                definition: row.node.definition.clone(),
                is_new: !before.contains_key(&id),
            });
        }
        let mut children = tree::children(after, &Some(id), true);
        children.reverse();
        stack.extend(children);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let preview = PlaylistToolPreview {
        id: id.clone(),
        stamp: guard.stamp.clone(),
        created: guard.created,
        expires: guard.expires,
        changes,
    };
    let plan = SavedPlan {
        guard,
        before: old,
        after: new,
        created_ids,
    };
    #[cfg(feature = "autotag")]
    let _gate = if plan.guard.data_bound {
        Some(
            lib.inner
                .tag_gate
                .lock()
                .map_err(|_| anyhow::anyhow!("Metadata access interrupted"))?,
        )
    } else {
        None
    };
    let mut conn = lib.conn()?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    validate(&tx, &plan.guard, crate::now())?;
    tx.execute(
        "INSERT INTO playlist_previews VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            id,
            kind,
            serde_json::to_string(&plan)?,
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
pub(super) fn apply(lib: &Library, kind: &str, id: &str, expected: u64) -> Result<PlaylistChange> {
    let text: String = lib
        .conn()?
        .query_row(
            "SELECT payload_json FROM playlist_previews WHERE id=?1 AND kind=?2",
            params![id, kind],
            |r| r.get(0),
        )
        .optional()?
        .context("Unknown or consumed playlist preview")?;
    let plan: SavedPlan = serde_json::from_str(&text)?;
    ensure!(
        expected == plan.guard.stamp.playlist_revision,
        "Preview revision does not match"
    );
    #[cfg(feature = "autotag")]
    let _gate = if plan.guard.data_bound {
        Some(
            lib.inner
                .tag_gate
                .lock()
                .map_err(|_| anyhow::anyhow!("Metadata access interrupted"))?,
        )
    } else {
        None
    };
    let mut conn = lib.conn()?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let saved: Option<String> = tx
        .query_row(
            "SELECT payload_json FROM playlist_previews WHERE id=?1 AND kind=?2",
            params![id, kind],
            |r| r.get(0),
        )
        .optional()?;
    ensure!(
        saved.as_deref() == Some(text.as_str()),
        "Playlist preview changed or was consumed"
    );
    validate(&tx, &plan.guard, crate::now())?;
    let before = load(&tx)?;
    for (id, row) in &plan.before {
        ensure!(
            before.get(id) == row.as_ref(),
            "Playlist changed after preview"
        );
    }
    let mut after = before.clone();
    for (id, row) in plan.after {
        if let Some(row) = row {
            after.insert(id, row);
        } else {
            after.remove(&id);
        }
    }
    let added: BTreeSet<_> = after
        .keys()
        .filter(|id| !before.contains_key(*id))
        .cloned()
        .collect();
    ensure!(
        plan.created_ids.iter().collect::<BTreeSet<_>>().len() == plan.created_ids.len()
            && added == plan.created_ids.iter().cloned().collect::<BTreeSet<_>>(),
        "Invalid preview creation identities"
    );
    let change = commit_tree_change(
        &tx,
        &before,
        after,
        plan.created_ids,
        expected,
        uuid::Uuid::new_v4().to_string(),
    )?;
    tx.execute("DELETE FROM playlist_previews WHERE id=?1", [id])?;
    tx.commit()?;
    Ok(change)
}
use rusqlite::OptionalExtension;
