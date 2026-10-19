use super::{model::*, tree::*};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeSet, VecDeque};
fn manual<'a>(tree: &'a mut Tree, id: &str) -> Result<&'a mut StoredNode> {
    ensure!(
        active(tree, id)?.node.kind == PlaylistKind::Playlist,
        "Smart playlists and folders have no manual members"
    );
    Ok(tree.get_mut(id).unwrap())
}
fn tracks_exist(ids: &[String], tracks: &BTreeSet<String>) -> Result<()> {
    for id in ids {
        ensure!(tracks.contains(id), "Unknown track: {id}");
    }
    Ok(())
}
pub(crate) fn apply(
    tree: &mut Tree,
    tracks: &BTreeSet<String>,
    edit: PlaylistEdit,
    operation: &str,
    created: &mut Vec<String>,
    depth: usize,
    budget: &mut usize,
) -> Result<()> {
    *budget += 1;
    ensure!(
        depth <= 8 && *budget <= 5000,
        "Playlist edit batch limit exceeded"
    );
    match edit {
        PlaylistEdit::Create {
            id,
            parent_id,
            kind,
            name,
            definition,
        } => {
            let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            ensure!(!tree.contains_key(&id), "Playlist ID already exists");
            if let Some(parent) = &parent_id {
                ensure!(
                    active(tree, parent)?.node.kind == PlaylistKind::Folder,
                    "Parent is not a folder"
                );
            }
            let position = children(tree, &parent_id, true).len() as u32;
            tree.insert(
                id.clone(),
                StoredNode {
                    last_edit_id: String::new(),
                    node: PlaylistNode {
                        id: id.clone(),
                        parent_id,
                        kind,
                        name,
                        position,
                        favorite_position: None,
                        definition,
                        deleted: false,
                        deleted_by: None,
                    },
                    members: vec![],
                },
            );
            created.push(id);
        }
        PlaylistEdit::Rename { id, name } => {
            active(tree, &id)?;
            tree.get_mut(&id).unwrap().node.name = name;
        }
        PlaylistEdit::Move {
            id,
            parent_id,
            position,
        } => {
            active(tree, &id)?;
            place(tree, &id, parent_id, position)?;
        }
        PlaylistEdit::SetDefinition { id, definition } => {
            ensure!(
                active(tree, &id)?.node.kind == PlaylistKind::Smartlist,
                "Only smart playlists have rules"
            );
            tree.get_mut(&id).unwrap().node.definition = Some(definition);
        }
        PlaylistEdit::Duplicate {
            id,
            parent_id,
            name,
        } => {
            active(tree, &id)?;
            let source = tree.clone();
            if let Some(parent) = &parent_id {
                ensure!(
                    active(tree, parent)?.node.kind == PlaylistKind::Folder,
                    "Parent is not a folder"
                );
            }
            let mut queue = VecDeque::from([(id.clone(), parent_id, true)]);
            while let Some((original, parent, root)) = queue.pop_front() {
                let mut copied = source[&original].clone();
                let new_id = uuid::Uuid::new_v4().to_string();
                copied.node.id = new_id.clone();
                copied.node.parent_id = parent.clone();
                copied.node.position = children(tree, &parent, true).len() as u32;
                copied.node.favorite_position = None;
                copied.node.deleted_by = None;
                if root && let Some(name) = &name {
                    copied.node.name = name.clone();
                }
                for child in children(&source, &Some(original.clone()), true) {
                    queue.push_back((child, Some(new_id.clone()), false));
                }
                tree.insert(new_id.clone(), copied);
                created.push(new_id);
            }
        }
        PlaylistEdit::Remove { id } => {
            active(tree, &id)?;
            for child in descendants(tree, &id, true) {
                let n = &mut tree.get_mut(&child).unwrap().node;
                n.deleted = true;
                n.deleted_by = Some(operation.into());
            }
        }
        PlaylistEdit::Restore {
            id,
            parent_id,
            position,
        } => {
            let n = &tree.get(&id).context("Unknown removed playlist")?.node;
            ensure!(n.deleted, "Playlist is not removed");
            let group = n
                .deleted_by
                .clone()
                .context("Missing playlist removal identity")?;
            let restoring: Vec<_> = descendants(tree, &id, false)
                .into_iter()
                .filter(|child| {
                    tree[child].node.deleted
                        && tree[child].node.deleted_by.as_deref() == Some(group.as_str())
                })
                .collect();
            let mut existing: Vec<_> = tree
                .values()
                .filter(|s| !s.node.deleted)
                .filter_map(|s| s.node.favorite_position.map(|p| (p, s.node.id.clone())))
                .collect();
            existing.sort();
            let mut favorites: Vec<_> = existing.into_iter().map(|(_, id)| id).collect();
            let mut restored_favorites: Vec<_> = restoring
                .iter()
                .filter_map(|id| tree[id].node.favorite_position.map(|p| (p, id.clone())))
                .collect();
            restored_favorites.sort();
            for (slot, id) in restored_favorites {
                favorites.insert((slot as usize).min(favorites.len()), id);
            }
            for child in restoring {
                let n = &mut tree.get_mut(&child).unwrap().node;
                n.deleted = false;
                n.deleted_by = None;
            }
            for (position, id) in favorites.iter().enumerate() {
                tree.get_mut(id).unwrap().node.favorite_position = Some(position as u32);
            }
            place(tree, &id, parent_id, position)?;
        }
        PlaylistEdit::Favorite { id, position } => {
            active(tree, &id)?;
            let mut ids: Vec<_> = tree
                .values()
                .filter(|s| {
                    !s.node.deleted && s.node.favorite_position.is_some() && s.node.id != id
                })
                .map(|s| (s.node.favorite_position.unwrap(), s.node.id.clone()))
                .collect();
            ids.sort();
            let mut ids: Vec<_> = ids.into_iter().map(|(_, id)| id).collect();
            tree.get_mut(&id).unwrap().node.favorite_position = None;
            if let Some(position) = position {
                ids.insert((position as usize).min(ids.len()), id);
            }
            for (position, id) in ids.iter().enumerate() {
                tree.get_mut(id).unwrap().node.favorite_position = Some(position as u32);
            }
        }
        PlaylistEdit::AddMembers { id, track_ids } => {
            tracks_exist(&track_ids, tracks)?;
            let row = manual(tree, &id)?;
            let mut seen: BTreeSet<_> = row.members.iter().cloned().collect();
            for track in track_ids {
                if seen.insert(track.clone()) {
                    row.members.push(track);
                }
            }
        }
        PlaylistEdit::RemoveMembers { id, track_ids } => {
            let row = manual(tree, &id)?;
            ensure!(
                track_ids.iter().all(|id| row.members.contains(id)),
                "Track is not a playlist member"
            );
            let selected: BTreeSet<_> = track_ids.into_iter().collect();
            row.members.retain(|id| !selected.contains(id));
        }
        PlaylistEdit::ReorderMembers { id, track_ids } => {
            let row = manual(tree, &id)?;
            let selected: BTreeSet<_> = track_ids.iter().cloned().collect();
            ensure!(
                selected.len() == track_ids.len()
                    && selected.iter().all(|id| row.members.contains(id)),
                "Reorder requires distinct existing members"
            );
            let mut values = track_ids.into_iter();
            for id in &mut row.members {
                if selected.contains(id) {
                    *id = values.next().unwrap();
                }
            }
        }
        PlaylistEdit::ReplaceTrackRefs { old_ids, keeper } => {
            tracks_exist(&old_ids, tracks)?;
            tracks_exist(std::slice::from_ref(&keeper), tracks)?;
            let old: BTreeSet<_> = old_ids.into_iter().collect();
            for row in tree
                .values_mut()
                .filter(|s| s.node.kind == PlaylistKind::Playlist)
            {
                let mut seen = BTreeSet::new();
                row.members = row
                    .members
                    .iter()
                    .map(|id| {
                        if old.contains(id) {
                            keeper.clone()
                        } else {
                            id.clone()
                        }
                    })
                    .filter(|id| seen.insert(id.clone()))
                    .collect();
            }
        }
        PlaylistEdit::Batch { edits } => {
            for edit in edits {
                apply(tree, tracks, edit, operation, created, depth + 1, budget)?;
            }
        }
    }
    Ok(())
}
