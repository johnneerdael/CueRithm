use super::model::*;
use anyhow::{Context, Result, ensure};
use cuerithm_smartlists::{PlaylistReference, RuleValue};
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
pub(crate) fn active<'a>(tree: &'a Tree, id: &str) -> Result<&'a StoredNode> {
    let row = tree
        .get(id)
        .with_context(|| format!("Unknown playlist: {id}"))?;
    ensure!(!row.node.deleted, "Playlist is removed: {id}");
    Ok(row)
}
pub(crate) fn children(tree: &Tree, parent: &Option<String>, alive_only: bool) -> Vec<String> {
    let mut nodes: Vec<_> = tree
        .values()
        .filter(|s| s.node.parent_id == *parent && (!alive_only || !s.node.deleted))
        .collect();
    nodes.sort_by_key(|s| (s.node.position, &s.node.id));
    nodes.iter().map(|s| s.node.id.clone()).collect()
}
pub(crate) fn descendants(tree: &Tree, id: &str, alive_only: bool) -> Vec<String> {
    let mut result = Vec::new();
    let mut queue = VecDeque::from([id.to_owned()]);
    let mut seen = BTreeSet::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        for child in children(tree, &Some(id.clone()), alive_only) {
            queue.push_back(child);
        }
        result.push(id);
    }
    result
}
pub(crate) fn place(
    tree: &mut Tree,
    id: &str,
    parent: Option<String>,
    position: u32,
) -> Result<()> {
    if let Some(p) = &parent {
        ensure!(
            active(tree, p)?.node.kind == PlaylistKind::Folder,
            "Parent is not a folder"
        );
    }
    ensure!(
        parent.as_deref() != Some(id),
        "A playlist cannot contain itself"
    );
    let mut siblings = children(tree, &parent, true);
    siblings.retain(|s| s != id);
    siblings.insert((position as usize).min(siblings.len()), id.into());
    tree.get_mut(id).context("Unknown playlist")?.node.parent_id = parent;
    for (position, sibling) in siblings.iter().enumerate() {
        tree.get_mut(sibling).unwrap().node.position = position as u32;
    }
    Ok(())
}
pub(crate) fn normalize(tree: &mut Tree) {
    let parents: BTreeSet<_> = tree
        .values()
        .filter(|s| !s.node.deleted)
        .map(|s| s.node.parent_id.clone())
        .collect();
    for parent in parents {
        for (position, id) in children(tree, &parent, true).iter().enumerate() {
            tree.get_mut(id).unwrap().node.position = position as u32;
        }
    }
    let mut favorites: Vec<_> = tree
        .values()
        .filter(|s| !s.node.deleted && s.node.favorite_position.is_some())
        .map(|s| (s.node.favorite_position.unwrap(), s.node.id.clone()))
        .collect();
    favorites.sort();
    for (position, (_, id)) in favorites.iter().enumerate() {
        tree.get_mut(id).unwrap().node.favorite_position = Some(position as u32);
    }
}
pub(crate) fn known_tracks(conn: &Connection) -> Result<BTreeSet<String>> {
    Ok(conn
        .prepare("SELECT id FROM tracks")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}
pub(crate) fn validate(tree: &Tree, tracks: &BTreeSet<String>) -> Result<()> {
    let mut positions = BTreeSet::new();
    let mut favorites = BTreeSet::new();
    for (id, row) in tree {
        let n = &row.node;
        ensure!(
            id == &n.id && !id.is_empty() && id.len() <= 128,
            "Invalid playlist ID"
        );
        ensure!(
            !n.name.trim().is_empty() && n.name.chars().count() <= 255,
            "Playlist names need 1–255 characters"
        );
        match (&n.kind, &n.definition) {
            (PlaylistKind::Smartlist, Some(d)) => d.validate_shape()?,
            (PlaylistKind::Smartlist, None) => anyhow::bail!("Smart playlist needs rules"),
            (_, Some(_)) => anyhow::bail!("Only smart playlists have rules"),
            _ => {}
        }
        ensure!(
            n.kind == PlaylistKind::Playlist || row.members.is_empty(),
            "Only ordinary playlists have manual members"
        );
        ensure!(
            row.members.iter().collect::<BTreeSet<_>>().len() == row.members.len(),
            "Duplicate playlist members"
        );
        for track in &row.members {
            ensure!(tracks.contains(track), "Unknown track: {track}");
        }
        if n.deleted {
            continue;
        }
        ensure!(
            positions.insert((n.parent_id.clone(), n.position)),
            "Playlist positions conflict with newer edits"
        );
        if let Some(p) = n.favorite_position {
            ensure!(
                favorites.insert(p),
                "Favorite order conflicts with newer edits"
            );
        }
        let mut parent = n.parent_id.as_deref();
        let mut seen = BTreeSet::from([id.as_str()]);
        while let Some(p) = parent {
            ensure!(seen.insert(p), "Playlist folder cycle");
            let ancestor = active(tree, p)?;
            ensure!(
                ancestor.node.kind == PlaylistKind::Folder,
                "Parent is not a folder"
            );
            parent = ancestor.node.parent_id.as_deref();
        }
    }
    validate_dependencies(tree)
}
fn validate_dependencies(tree: &Tree) -> Result<()> {
    dependency_order(tree).map(|_| ())
}
pub(crate) fn dependency_order(tree: &Tree) -> Result<Vec<String>> {
    let mut edges: BTreeMap<String, BTreeSet<String>> = tree
        .values()
        .filter(|s| !s.node.deleted && s.node.kind == PlaylistKind::Smartlist)
        .map(|s| (s.node.id.clone(), BTreeSet::new()))
        .collect();
    for (id, deps) in &mut edges {
        for rule in &tree[id].node.definition.as_ref().unwrap().rules {
            let RuleValue::Playlist(reference) = &rule.value else {
                continue;
            };
            let Some(target) = tree.get(&reference.id).filter(|s| !s.node.deleted) else {
                continue;
            };
            ensure!(
                target.node.kind == reference.kind,
                "Playlist reference kind changed"
            );
            match reference.kind {
                PlaylistKind::Smartlist => {
                    deps.insert(reference.id.clone());
                }
                PlaylistKind::Folder => {
                    for child in descendants(tree, &reference.id, true) {
                        if tree[&child].node.kind == PlaylistKind::Smartlist {
                            deps.insert(child);
                        }
                    }
                }
                PlaylistKind::Playlist => {}
            }
        }
    }
    let mut counts: BTreeMap<_, _> = edges.iter().map(|(id, d)| (id.clone(), d.len())).collect();
    let mut reverse: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, deps) in &edges {
        for d in deps {
            reverse.entry(d.clone()).or_default().push(id.clone());
        }
    }
    let mut queue: VecDeque<_> = counts
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| id.clone())
        .collect();
    let mut order = vec![];
    while let Some(id) = queue.pop_front() {
        order.push(id.clone());
        if let Some(dependents) = reverse.get(&id) {
            for d in dependents {
                let count = counts.get_mut(d).unwrap();
                *count -= 1;
                if *count == 0 {
                    queue.push_back(d.clone());
                }
            }
        }
    }
    ensure!(
        order.len() == edges.len(),
        "Smart playlist dependency cycle"
    );
    Ok(order)
}
pub(crate) fn resolve(tree: &Tree, reference: &PlaylistReference) -> Result<String> {
    if !reference.id.is_empty() {
        let n = active(tree, &reference.id)?;
        ensure!(
            n.node.kind == reference.kind,
            "Playlist reference kind changed"
        );
        return Ok(reference.id.clone());
    }
    ensure!(
        !reference.path.is_empty(),
        "Playlist reference has no ID or path"
    );
    let mut parent = None;
    for (i, name) in reference.path.iter().enumerate() {
        let candidates: Vec<_> = tree
            .values()
            .filter(|s| {
                !s.node.deleted
                    && s.node.parent_id == parent
                    && s.node.name == *name
                    && s.node.kind
                        == if i + 1 == reference.path.len() {
                            reference.kind
                        } else {
                            PlaylistKind::Folder
                        }
            })
            .collect();
        ensure!(
            candidates.len() == 1,
            "Unavailable or ambiguous playlist path"
        );
        let node = &candidates[0].node;
        ensure!(
            node.kind
                == if i + 1 == reference.path.len() {
                    reference.kind
                } else {
                    PlaylistKind::Folder
                },
            "Playlist path has wrong kind"
        );
        parent = Some(node.id.clone());
    }
    parent.context("Unavailable playlist reference")
}
