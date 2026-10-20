use super::*;
use rusqlite::OptionalExtension;
impl Library {
    pub fn undo_playlist_edit(&self, id: &str, expected_revision: u64) -> Result<PlaylistChange> {
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(
            revision(&tx)? == expected_revision,
            "Playlist changed; refresh before Undo"
        );
        let (kind, before, after, undone): (String, String, String, Option<String>) = tx
            .query_row(
                "SELECT kind,before_json,after_json,undone_by FROM playlist_edits WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?
            .context("Unknown playlist operation")?;
        ensure!(
            kind == "edit" && undone.is_none(),
            "This operation cannot be undone again"
        );
        let before: Delta = serde_json::from_str(&before)?;
        let after: Delta = serde_json::from_str(&after)?;
        let current = load(&tx)?;
        let mut repaired = BTreeSet::new();
        for (id, expected) in &after {
            if current.get(id) == expected.as_ref() {
                continue;
            }
            if let (Some(actual), Some(expected)) = (current.get(id), expected) {
                let mut comparable = actual.clone();
                comparable.node.definition = expected.node.definition.clone();
                let managed: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM automatic_playlist_nodes WHERE playlist_id=?1)",
                    [id],
                    |r| r.get(0),
                )?;
                // Automatic repair changes only the definition. All other
                // contents AND the user edit identity must still match exactly.
                if managed && actual.node.kind == PlaylistKind::Smartlist && comparable == *expected
                {
                    repaired.insert(id.clone());
                    continue;
                }
            }
            anyhow::bail!("Newer playlist edits prevent Undo: {id}");
        }
        let removing: std::collections::BTreeSet<_> = before
            .iter()
            .filter(|(_, row)| row.is_none())
            .map(|(id, _)| id)
            .collect();
        for (other, row) in &current {
            if !removing.contains(other) {
                ensure!(
                    !row.node
                        .parent_id
                        .as_ref()
                        .is_some_and(|id| removing.contains(id)),
                    "Newer child playlists prevent Undo"
                );
                if let Some(definition) = &row.node.definition {
                    for rule in &definition.rules {
                        if let cuerithm_smartlists::RuleValue::Playlist(reference) = &rule.value {
                            ensure!(
                                !removing.contains(&reference.id),
                                "Newer smart playlist references prevent Undo"
                            );
                        }
                    }
                }
            }
        }
        let mut restored = current.clone();
        for (id, row) in &before {
            if let Some(row) = row {
                let mut row = row.clone();
                if repaired.contains(id) {
                    row.node.definition = current[id].node.definition.clone();
                }
                restored.insert(id.clone(), row);
            } else {
                restored.remove(id);
            }
        }
        tree::validate(&restored, &tree::known_tracks(&tx)?)?;
        let (a, b) = diff(&current, &restored);
        persist(&tx, &a, &b)?;
        let undo = uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO playlist_edits(id,kind,before_json,after_json,source_edit_id,created) VALUES(?1,'undo',?2,?3,?4,?5)",params![undo,serde_json::to_string(&a)?,serde_json::to_string(&b)?,id,now()])?;
        tx.execute(
            "UPDATE playlist_edits SET undone_by=?1 WHERE id=?2",
            params![undo, id],
        )?;
        let change = PlaylistChange {
            edit_id: Some(undo),
            revision: revision(&tx)?,
            created_ids: vec![],
            affected_ids: b.keys().cloned().collect(),
        };
        tx.commit()
            .context("Newer playlist dependencies prevent Undo")?;
        Ok(change)
    }
}
