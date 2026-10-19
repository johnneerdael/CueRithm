//! Mirror CueRithm's playlists and folders into rekordbox, by name path, touching only what CueRithm manages.
//!
//! A playlist is managed once CueRithm has written it; the record of that lives beside the sync history. An
//! existing rekordbox playlist with the same path that CueRithm did not write is never taken over, and a
//! managed one that was edited in rekordbox since is a conflict, not an overwrite. Smart lists are written as
//! ordinary playlists holding their current members.
use crate::*;
use anyhow::Result;
use cuerithm_rekordbox::{
    PlaylistKind as RbKind, PlaylistRemoval, PlaylistWrite, RekordboxCollection,
    membership_fingerprint,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Managed {
    rb_id: String,
    path: Vec<String>,
    folder: bool,
    /// Fingerprint of the membership rekordbox held when CueRithm last wrote or read it.
    members: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct State {
    /// Keyed by CueRithm playlist ID.
    managed: BTreeMap<String, Managed>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistAction {
    Create,
    Update,
    Unchanged,
    Remove,
    /// A playlist with this path already exists in rekordbox and CueRithm did not write it.
    SkipExisting,
    /// rekordbox changed a managed playlist since CueRithm last wrote it.
    SkipChanged,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistPlanItem {
    pub path: Vec<String>,
    pub folder: bool,
    pub action: PlaylistAction,
    pub tracks: usize,
    /// Members that are not linked to rekordbox and so cannot be written.
    pub unlinked: usize,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaylistPlanSummary {
    pub items: Vec<PlaylistPlanItem>,
}
impl PlaylistPlanSummary {
    pub fn writes(&self) -> usize {
        self.items
            .iter()
            .filter(|i| {
                matches!(
                    i.action,
                    PlaylistAction::Create | PlaylistAction::Update | PlaylistAction::Remove
                )
            })
            .count()
    }
}
struct Desired {
    local_id: String,
    path: Vec<String>,
    folder: bool,
    position: i64,
    content_ids: Vec<String>,
    unlinked: usize,
}

fn rb_paths(collection: &RekordboxCollection) -> BTreeMap<Vec<String>, (String, bool)> {
    let live: BTreeMap<_, _> = collection
        .playlists
        .iter()
        .filter(|p| p.deleted == Some(false) && matches!(p.attribute, 0 | 1))
        .map(|p| (p.id.clone(), p))
        .collect();
    let mut paths = BTreeMap::new();
    for playlist in live.values() {
        let mut path = vec![playlist.name.clone()];
        let mut parent = playlist.parent_id.clone();
        let mut guard = 0;
        while let Some(id) = parent.filter(|p| p != "root") {
            let Some(up) = live.get(&id) else { break };
            path.insert(0, up.name.clone());
            parent = up.parent_id.clone();
            guard += 1;
            if guard > 64 {
                break;
            }
        }
        paths.insert(path, (playlist.id.clone(), playlist.attribute == 1));
    }
    paths
}
fn rb_members(collection: &RekordboxCollection, id: &str) -> Vec<String> {
    let mut links: Vec<_> = collection
        .playlist_links
        .iter()
        .filter(|l| l.playlist_id == id && l.deleted == Some(false))
        .collect();
    links.sort_by_key(|l| l.position);
    links.into_iter().map(|l| l.content_id.clone()).collect()
}

impl Library {
    fn native_playlist_state(&self, target: &str) -> Result<State> {
        let text: Option<String> = self
            .conn()?
            .query_row(
                "SELECT payload_json FROM maintenance_scans WHERE id=?1",
                [format!("native_playlists:{target}")],
                |r| r.get(0),
            )
            .ok();
        Ok(text
            .map(|t| serde_json::from_str(&t))
            .transpose()?
            .unwrap_or_default())
    }
    fn save_native_playlist_state(&self, target: &str, state: &State) -> Result<()> {
        self.conn()?.execute(
            "INSERT INTO maintenance_scans VALUES(?1,'native_playlist_state',?2,?3) ON CONFLICT(id) DO UPDATE SET payload_json=excluded.payload_json",
            params![format!("native_playlists:{target}"), serde_json::to_string(state)?, now()],
        )?;
        Ok(())
    }
    /// CueRithm's playlists as rekordbox would hold them: unique names per level, siblings in order, members
    /// reduced to the tracks linked to this target.
    fn desired_playlists(&self, target: &str) -> Result<Vec<Desired>> {
        let nodes: Vec<_> = self
            .playlist_state()?
            .nodes
            .into_iter()
            .filter(|n| !n.deleted)
            .collect();
        let linked: BTreeMap<String, String> = {
            let conn = self.conn()?;
            let mut q = conn.prepare("SELECT local_track_id,content_id FROM native_track_links WHERE target_id=?1 AND local_track_id IS NOT NULL AND status='linked'")?;
            q.query_map([target], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?
        };
        let by_id: BTreeMap<_, _> = nodes.iter().map(|n| (n.id.clone(), n)).collect();
        let mut children: BTreeMap<Option<String>, Vec<&PlaylistNode>> = BTreeMap::new();
        for node in &nodes {
            children
                .entry(node.parent_id.clone().filter(|p| by_id.contains_key(p)))
                .or_default()
                .push(node);
        }
        for list in children.values_mut() {
            list.sort_by_key(|n| (n.position, n.id.clone()));
        }
        let mut out = vec![];
        let mut stack: Vec<(Option<String>, Vec<String>)> = vec![(None, vec![])];
        while let Some((parent, prefix)) = stack.pop() {
            let mut taken: BTreeSet<String> = BTreeSet::new();
            let siblings = children.get(&parent).cloned().unwrap_or_default();
            let mut next = vec![];
            for (index, node) in siblings.iter().enumerate() {
                let mut name = node.name.trim().to_owned();
                if name.is_empty() {
                    name = "Untitled".into();
                }
                let base = name.clone();
                let mut n = 1;
                while !taken.insert(name.to_lowercase()) {
                    n += 1;
                    name = format!("{base} {n}");
                }
                let mut path = prefix.clone();
                path.push(name);
                let folder = format!("{:?}", node.kind) == "Folder";
                let (content_ids, unlinked) = if folder {
                    (vec![], 0)
                } else {
                    let members = self.playlist_members(&node.id)?;
                    let total = members.len();
                    let ids: Vec<String> = members
                        .iter()
                        .filter_map(|t| linked.get(t).cloned())
                        .collect();
                    let unlinked = total - ids.len();
                    (ids, unlinked)
                };
                out.push(Desired {
                    local_id: node.id.clone(),
                    path: path.clone(),
                    folder,
                    position: index as i64 + 1,
                    content_ids,
                    unlinked,
                });
                if folder {
                    next.push((Some(node.id.clone()), path));
                }
            }
            stack.extend(next.into_iter().rev());
        }
        Ok(out)
    }

    /// What mirroring would do against the library in `collection`, as a write plan and a summary.
    pub(crate) fn plan_native_playlists(
        &self,
        target: &str,
        collection: &RekordboxCollection,
    ) -> Result<(
        Vec<PlaylistWrite>,
        Vec<PlaylistRemoval>,
        PlaylistPlanSummary,
    )> {
        let desired = self.desired_playlists(target)?;
        let state = self.native_playlist_state(target)?;
        let existing = rb_paths(collection);
        let mut writes = vec![];
        let mut removals = vec![];
        let mut summary = PlaylistPlanSummary::default();
        let wanted_ids: BTreeSet<_> = desired.iter().map(|d| d.local_id.clone()).collect();
        for want in &desired {
            let item = |action| PlaylistPlanItem {
                path: want.path.clone(),
                folder: want.folder,
                action,
                tracks: want.content_ids.len(),
                unlinked: want.unlinked,
            };
            let managed = state.managed.get(&want.local_id);
            let found = existing.get(&want.path);
            match (found, managed) {
                (None, _) => {
                    writes.push(PlaylistWrite {
                        path: want.path.clone(),
                        kind: if want.folder {
                            RbKind::Folder
                        } else {
                            RbKind::Playlist
                        },
                        position: want.position,
                        tracks: want.content_ids.clone(),
                        expected_members: None,
                    });
                    summary.items.push(item(PlaylistAction::Create));
                }
                (Some((id, _)), Some(m)) if m.rb_id == *id => {
                    let current = rb_members(collection, id);
                    let current_hash = membership_fingerprint(&current);
                    let moved = collection
                        .playlists
                        .iter()
                        .find(|p| p.id == *id)
                        .is_some_and(|p| p.sequence != want.position);
                    if !want.folder && m.members.as_deref().is_some_and(|h| h != current_hash) {
                        summary.items.push(item(PlaylistAction::SkipChanged));
                    } else if moved || (!want.folder && current != want.content_ids) {
                        writes.push(PlaylistWrite {
                            path: want.path.clone(),
                            kind: if want.folder {
                                RbKind::Folder
                            } else {
                                RbKind::Playlist
                            },
                            position: want.position,
                            tracks: want.content_ids.clone(),
                            expected_members: (!want.folder).then_some(current_hash),
                        });
                        summary.items.push(item(PlaylistAction::Update));
                    } else {
                        summary.items.push(item(PlaylistAction::Unchanged));
                    }
                }
                // A folder that already exists simply holds the playlists below it; it is not altered.
                (Some((_, true)), None) if want.folder => {
                    summary.items.push(item(PlaylistAction::Unchanged))
                }
                (Some(_), _) => summary.items.push(item(PlaylistAction::SkipExisting)),
            }
        }
        // Playlists CueRithm wrote that no longer exist here are removed from rekordbox, unless it changed them.
        for (local_id, m) in &state.managed {
            if wanted_ids.contains(local_id) {
                continue;
            }
            let Some((id, _)) = existing.get(&m.path).filter(|(id, _)| *id == m.rb_id) else {
                continue;
            };
            let current_hash = membership_fingerprint(&rb_members(collection, id));
            let changed = !m.folder && m.members.as_deref().is_some_and(|h| h != current_hash);
            let foreign_children = m.folder
                && existing
                    .iter()
                    .any(|(path, _)| path.len() > m.path.len() && path.starts_with(&m.path));
            let action = if changed || foreign_children {
                PlaylistAction::SkipChanged
            } else {
                removals.push(PlaylistRemoval {
                    playlist_id: id.clone(),
                    expected_members: (!m.folder).then_some(current_hash),
                });
                PlaylistAction::Remove
            };
            summary.items.push(PlaylistPlanItem {
                path: m.path.clone(),
                folder: m.folder,
                action,
                tracks: 0,
                unlinked: 0,
            });
        }
        Ok((writes, removals, summary))
    }

    /// Record what rekordbox holds after a verified write: managed playlists get their IDs and membership,
    /// and removed ones are forgotten.
    pub(crate) fn acknowledge_native_playlists(
        &self,
        target: &str,
        summary: &PlaylistPlanSummary,
        after: &RekordboxCollection,
    ) -> Result<()> {
        let desired = self.desired_playlists(target)?;
        let mut state = self.native_playlist_state(target)?;
        let existing = rb_paths(after);
        let written: BTreeSet<&Vec<String>> = summary
            .items
            .iter()
            .filter(|i| matches!(i.action, PlaylistAction::Create | PlaylistAction::Update))
            .map(|i| &i.path)
            .collect();
        for want in desired.iter().filter(|d| written.contains(&d.path)) {
            if let Some((id, folder)) = existing.get(&want.path) {
                state.managed.insert(
                    want.local_id.clone(),
                    Managed {
                        rb_id: id.clone(),
                        path: want.path.clone(),
                        folder: *folder,
                        members: (!*folder).then(|| membership_fingerprint(&rb_members(after, id))),
                    },
                );
            }
        }
        for item in summary
            .items
            .iter()
            .filter(|i| i.action == PlaylistAction::Remove)
        {
            state.managed.retain(|_, m| m.path != item.path);
        }
        self.save_native_playlist_state(target, &state)
    }
}
