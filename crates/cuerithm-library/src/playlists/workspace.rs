use super::*;
use rusqlite::OptionalExtension;
use std::collections::BTreeMap;
impl Library {
    pub fn playlist_workspace(&self) -> Result<PlaylistWorkspace> {
        self.playlist_workspace_at(crate::now())
    }
    pub fn playlist_workspace_at(&self, now: i64) -> Result<PlaylistWorkspace> {
        let conn = self.conn()?;
        let tree = load(&conn)?;
        let stamp = preferences::stamp(&conn)?;
        let prefs = preferences::preferences(&conn)?.preferences;
        let zone = preferences::zone(&prefs)?;
        let mut cached = BTreeMap::new();
        let mut q=conn.prepare("SELECT playlist_id,state,error,input_revision,playlist_revision,preference_revision,evaluated_at,next_refresh,last_good_revision,definition_hash FROM smart_playlist_status")?;
        for row in q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                snapshot::CachedStatus::from_row_at(r, 1)?,
            ))
        })? {
            let (id, status) = row?;
            cached.insert(id, status);
        }
        drop(q);
        let latest_undo=conn.query_row("SELECT id FROM playlist_edits WHERE kind='edit' AND undone_by IS NULL ORDER BY rowid DESC LIMIT 1",[],|r|r.get(0)).optional()?;
        let mut children: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
        let mut nodes = tree.values().map(|r| r.node.clone()).collect::<Vec<_>>();
        nodes.sort_by(|a, b| {
            (&a.parent_id, a.position, &a.id).cmp(&(&b.parent_id, b.position, &b.id))
        });
        for node in nodes.iter().filter(|n| !n.deleted) {
            children
                .entry(node.parent_id.clone())
                .or_default()
                .push(node.id.clone());
        }
        for ids in children.values_mut() {
            ids.sort_by_key(|id| (tree[id].node.position, id.clone()));
        }
        let mut order = vec![];
        let mut stack = children.get(&None).cloned().unwrap_or_default();
        stack.reverse();
        let mut seen = BTreeSet::new();
        while let Some(id) = stack.pop() {
            ensure!(seen.insert(id.clone()), "Playlist hierarchy is cyclic");
            order.push(id.clone());
            let mut next = children.get(&Some(id)).cloned().unwrap_or_default();
            next.reverse();
            stack.extend(next);
        }
        ensure!(
            order.len() == nodes.iter().filter(|n| !n.deleted).count(),
            "Playlist hierarchy is unavailable"
        );
        let mut statuses: BTreeMap<String, SmartListStatus> = BTreeMap::new();
        for id in order.iter().rev() {
            let row = &tree[id];
            let mut status = if row.node.kind == PlaylistKind::Smartlist {
                evaluate::projected_status(row, cached.remove(id), &stamp, &prefs, &zone, now)?
            } else {
                evaluate::empty_status(id, SmartState::Ready)
            };
            if row.node.kind == PlaylistKind::Folder {
                for child in children.get(&Some(id.clone())).into_iter().flatten() {
                    let s = &statuses[child];
                    if evaluate::state_priority(s.state) > evaluate::state_priority(status.state) {
                        status.state = s.state;
                    }
                    if let Some(error) = &s.error {
                        let message = format!("{}: {error}", tree[child].node.name);
                        status.error = Some(
                            status
                                .error
                                .map_or(message.clone(), |old| format!("{old}\n{message}")),
                        );
                    }
                    if let Some(next) = s.next_refresh {
                        status.next_refresh =
                            Some(status.next_refresh.map_or(next, |old| old.min(next)));
                    }
                }
            }
            statuses.insert(id.clone(), status);
        }
        Ok(PlaylistWorkspace {
            tree: PlaylistState {
                revision: stamp.playlist_revision,
                nodes,
            },
            statuses: order
                .into_iter()
                .map(|id| statuses.remove(&id).unwrap())
                .collect(),
            latest_undo,
        })
    }
}
