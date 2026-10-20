#[path = "owned_rollups.rs"]
mod rollups;
#[path = "owned_state.rs"]
mod state;
use crate::{
    SyncLibrary, SyncOptions,
    ownership::{PlaylistBinding, PlaylistOwnershipReport, PlaylistOwnershipRequest},
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, params};
use state::{Catalog, Native, Ordinary, Smart};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

/// Apply locally acknowledged Engine playlist identities to a staged database.
pub fn write_staged_owned(
    path: &Path,
    device_root: &Path,
    library: &SyncLibrary,
    options: &SyncOptions,
    request: &PlaylistOwnershipRequest,
) -> Result<PlaylistOwnershipReport> {
    let mut db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    db.busy_timeout(std::time::Duration::from_secs(5))?;
    db.pragma_update(None, "foreign_keys", true)?;
    let (identity, major): (String, i64) = db.query_row(
        "SELECT uuid,schemaVersionMajor FROM Information LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(major == 3, "Engine DJ requires the current schema major 3");
    ensure!(!identity.is_empty(), "Engine database UUID is missing");
    ensure!(
        request.previous.identity.is_empty() && request.previous.bindings.is_empty()
            || request.previous.identity == identity,
        "Engine database identity changed"
    );
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let before = Catalog::read(&tx)?;
    let previous = previous_bindings(request)?;
    let active = active_nodes(library, request)?;
    let mut report = PlaylistOwnershipReport::default();
    report.current.identity = identity.clone();
    let mut removed = BTreeSet::new();
    for source in &request.removed {
        if let Some(binding) = previous.get(source) {
            if binding.created {
                let native = Native::parse(&binding.locator)?;
                verify(&before, binding)?;
                if before.contains(&native) {
                    removed.insert(native);
                }
                report.deleted.insert(source.clone());
            } else {
                report.retired.insert(source.clone());
            }
        }
    }
    let mut smart_rules = BTreeMap::new();
    for p in &library.playlists {
        if let Ok(Some(crate::smart_rules::NativeSmart::EngineDj(rules))) =
            crate::smart_rules::compile(p, library, options, crate::DjSoftware::EngineDj)
        {
            smart_rules.insert(p.id.clone(), rules);
        }
    }
    ensure!(
        smart_rules.is_empty() || before.smart_table,
        "Current Engine database has no Smartlist table"
    );
    let mut next:i64=tx.query_row("SELECT MAX(COALESCE((SELECT seq FROM sqlite_sequence WHERE name='Playlist'),0),COALESCE(MAX(id),0))+1 FROM Playlist",[],|r|r.get(0))?;
    let mut targets: BTreeMap<String, Target> = BTreeMap::new();
    let reserved: BTreeSet<_> = previous
        .values()
        .map(|b| Native::parse(&b.locator))
        .collect::<Result<_>>()?;
    let old_paths = before.paths()?;
    let mut pending: Vec<_> = library.playlists.iter().collect();
    while !pending.is_empty() {
        let count = pending.len();
        let mut deferred = Vec::new();
        for p in pending {
            let parent = if let Some(source) = &p.parent_id {
                if let Some(target) = targets.get(source) {
                    ordinary_id(&target.native)?
                } else if active.contains_key(source) {
                    deferred.push(p);
                    continue;
                } else {
                    let binding = previous
                        .get(source)
                        .context("Missing Engine source parent binding")?;
                    verify(&before, binding)?;
                    ordinary_id(&Native::parse(&binding.locator)?)?
                }
            } else {
                0
            };
            let prior = previous.get(&p.id);
            let prior_native = prior.map(|b| Native::parse(&b.locator)).transpose()?;
            if let Some(binding) = prior {
                verify(&before, binding)?;
            }
            let wants_smart = smart_rules.contains_key(&p.id);
            let same_kind = prior_native
                .as_ref()
                .is_some_and(|n| matches!(n, Native::Smart(_)) == wants_smart);
            let prior_exists = prior_native.as_ref().is_some_and(|n| before.contains(n));
            let (native, created) = if same_kind && prior_exists {
                (prior_native.clone().unwrap(), prior.unwrap().created)
            } else {
                if prior_exists {
                    ensure!(
                        prior.unwrap().created,
                        "Cannot replace an external Engine playlist representation"
                    );
                    removed.insert(prior_native.clone().unwrap());
                }
                let matched = if prior.is_none() {
                    if wants_smart {
                        let desired_parent_path =
                            source_parent_path(p, library, &targets, &previous, &old_paths)?;
                        let parent_path = old_paths.get(&parent).unwrap_or(&desired_parent_path);
                        before
                            .smart
                            .values()
                            .find(|s| {
                                s.name == p.name
                                    && &s.parent == parent_path
                                    && !reserved.contains(&Native::Smart(s.id.clone()))
                            })
                            .map(|s| Native::Smart(s.id.clone()))
                    } else {
                        before
                            .ordinary
                            .values()
                            .find(|n| {
                                n.parent == parent
                                    && n.name == p.name
                                    && !reserved.contains(&Native::Playlist(n.id))
                            })
                            .map(|n| Native::Playlist(n.id))
                    }
                } else {
                    None
                };
                if let Some(matched) = matched {
                    (matched, false)
                } else if wants_smart {
                    let native = Native::Smart(super::engine_smart_uuid(&p.id));
                    ensure!(
                        !before.contains(&native),
                        "New Engine smart UUID collides with an existing record"
                    );
                    (native, true)
                } else {
                    let id = next;
                    next = next
                        .checked_add(1)
                        .context("Engine playlist identity overflow")?;
                    (Native::Playlist(id), true)
                }
            };
            ensure!(
                !targets.values().any(|t| t.native == native),
                "Two source playlists resolve to one Engine native record"
            );
            targets.insert(
                p.id.clone(),
                Target {
                    native,
                    parent,
                    created,
                },
            );
        }
        ensure!(
            deferred.len() < count,
            "Engine source hierarchy contains a cycle"
        );
        pending = deferred;
    }
    let mut after = before.clone();
    for n in &removed {
        match n {
            Native::Playlist(id) => {
                after.ordinary.remove(id);
                after.members.remove(id);
            }
            Native::Smart(id) => {
                after.smart.remove(id);
            }
        }
    }
    for p in &library.playlists {
        let target = &targets[&p.id];
        if let Native::Playlist(id) = target.native {
            after.ordinary.insert(
                id,
                Ordinary {
                    id,
                    name: p.name.clone(),
                    parent: target.parent,
                    next: 0,
                },
            );
        }
    }
    for node in after.ordinary.values() {
        ensure!(
            node.parent == 0 || after.ordinary.contains_key(&node.parent),
            "Cannot remove an Engine folder containing an unplanned child"
        );
    }
    let paths = after.paths()?;
    let reverse = before.path_ids()?;
    for smart in after.smart.values_mut() {
        if let Some(parent) = reverse.get(&smart.parent) {
            ensure!(
                *parent == 0 || after.ordinary.contains_key(parent),
                "Cannot remove an Engine folder containing a foreign smartlist"
            );
            smart.parent = paths[parent].clone();
        }
        if let Some(next) = reverse.get(&smart.next_path)
            && let Some(path) = paths.get(next)
        {
            smart.next_path = path.clone();
        }
    }
    for p in &library.playlists {
        let target = &targets[&p.id];
        if let Native::Smart(id) = &target.native {
            let prior = after.smart.get(id);
            after.smart.insert(
                id.clone(),
                Smart {
                    id: id.clone(),
                    name: p.name.clone(),
                    parent: paths
                        .get(&target.parent)
                        .context("Engine smart parent is missing")?
                        .clone(),
                    next_uuid: prior.map(|s| s.next_uuid.clone()).unwrap_or_default(),
                    next_path: prior.map(|s| s.next_path.clone()).unwrap_or_default(),
                    rules: smart_rules[&p.id].clone(),
                },
            );
        }
    }
    validate_names(&after, &targets)?;
    let mut affected = BTreeSet::new();
    for native in removed.iter().chain(targets.values().map(|t| &t.native)) {
        if let Some(parent) = native_parent(&before, native, &old_paths)? {
            affected.insert(parent);
        }
        if let Some(parent) = native_parent(&after, native, &paths)? {
            affected.insert(parent);
        }
    }
    let replacements: BTreeMap<_, _> = targets
        .iter()
        .filter_map(|(source, target)| {
            previous
                .get(source)
                .map(|b| Native::parse(&b.locator).map(|old| (old, target.native.clone())))
        })
        .collect::<Result<_>>()?;
    merge_orders(
        &before,
        &mut after,
        library,
        &targets,
        &affected,
        &replacements,
    )?;
    for binding in previous.values() {
        let native = Native::parse(&binding.locator)?;
        if before.fingerprint(&native)? != after.fingerprint(&native)? {
            verify(&before, binding)?;
        }
    }
    publish_structure(&tx, &before, &after, &affected, &targets)?;
    let mut tracks = HashMap::new();
    for track in &library.tracks {
        ensure!(
            !tracks.contains_key(&track.id),
            "Duplicate Engine source track ID"
        );
        tracks.insert(
            track.id.clone(),
            super::write_track(&tx, device_root, &identity, track, options)?,
        );
    }
    for p in &library.playlists {
        if let Native::Playlist(id) = targets[&p.id].native {
            let members = p
                .track_ids
                .iter()
                .map(|source| {
                    Ok((
                        *tracks
                            .get(source)
                            .context("Engine playlist references an unselected track")?,
                        identity.clone(),
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            state::write_members(&tx, id, &members)?;
        }
    }
    let mut folders = BTreeSet::new();
    for n in removed.iter().chain(targets.values().map(|t| &t.native)) {
        add_ancestors(&before, n, &mut folders)?;
        add_ancestors(&after, n, &mut folders)?;
    }
    folders.retain(|id| after.ordinary.contains_key(id));
    let known_smart: BTreeMap<_, _> = library
        .playlists
        .iter()
        .filter_map(|p| {
            if let Native::Smart(id) = &targets[&p.id].native {
                Some((id.clone(), p))
            } else {
                None
            }
        })
        .map(|(id, p)| {
            Ok((
                id,
                p.track_ids
                    .iter()
                    .map(|source| {
                        Ok((
                            *tracks
                                .get(source)
                                .context("Engine smartlist refers to an unselected track")?,
                            identity.clone(),
                        ))
                    })
                    .collect::<Result<BTreeSet<_>>>()?,
            ))
        })
        .collect::<Result<_>>()?;
    rollups::rebuild(&tx, &after, &folders, &known_smart, &identity)?;
    let observed = Catalog::read(&tx)?;
    let mut bindings = previous
        .into_iter()
        .filter(|(id, _)| !request.removed.contains(id))
        .map(|(id, b)| (id, b.clone()))
        .collect::<BTreeMap<_, _>>();
    for (source, target) in &targets {
        let p = active[source];
        bindings.insert(
            source.clone(),
            PlaylistBinding {
                source_id: source.clone(),
                locator: target.native.locator(),
                parent_source_id: p.parent_id.clone(),
                created: target.created,
                fingerprint: observed
                    .fingerprint(&target.native)?
                    .context("Engine owned write did not produce its native object")?,
            },
        );
    }
    for binding in bindings.values_mut() {
        if targets.contains_key(&binding.source_id) {
            continue;
        }
        let native = Native::parse(&binding.locator)?;
        let old = before.fingerprint(&native)?;
        let new = observed.fingerprint(&native)?;
        if old != new {
            ensure!(
                old.as_deref() == Some(binding.fingerprint.as_str()),
                "An indirectly affected Engine playlist was edited externally"
            );
            binding.fingerprint = new.context("Unselected Engine binding disappeared")?;
        }
    }
    report.current.bindings = bindings.into_values().collect();
    tx.commit()?;
    Ok(report)
}
#[derive(Clone)]
struct Target {
    native: Native,
    parent: i64,
    created: bool,
}
fn previous_bindings(
    request: &PlaylistOwnershipRequest,
) -> Result<BTreeMap<String, &PlaylistBinding>> {
    let mut result = BTreeMap::new();
    let mut locators = BTreeSet::new();
    for binding in &request.previous.bindings {
        ensure!(
            result.insert(binding.source_id.clone(), binding).is_none()
                && locators.insert(Native::parse(&binding.locator)?),
            "Duplicate Engine ownership binding"
        );
    }
    Ok(result)
}
fn active_nodes<'a>(
    library: &'a SyncLibrary,
    request: &PlaylistOwnershipRequest,
) -> Result<BTreeMap<String, &'a crate::SyncPlaylist>> {
    let mut result = BTreeMap::new();
    for p in &library.playlists {
        ensure!(
            !p.id.is_empty()
                && !request.removed.contains(&p.id)
                && result.insert(p.id.clone(), p).is_none(),
            "Duplicate, empty or removed Engine source playlist"
        );
        if p.folder {
            ensure!(
                p.track_ids.is_empty(),
                "Engine source folders cannot contain direct tracks"
            );
        }
    }
    for p in &library.playlists {
        if let Some(parent) = p.parent_id.as_ref().and_then(|id| result.get(id)) {
            ensure!(
                parent.folder,
                "Engine source playlist parent is not a folder"
            );
        }
    }
    Ok(result)
}
fn verify(before: &Catalog, binding: &PlaylistBinding) -> Result<()> {
    if let Some(current) = before.fingerprint(&Native::parse(&binding.locator)?)? {
        ensure!(
            current == binding.fingerprint,
            "Engine playlist was edited in the target: {}",
            binding.source_id
        );
    }
    Ok(())
}
fn ordinary_id(native: &Native) -> Result<i64> {
    match native {
        Native::Playlist(id) => Ok(*id),
        Native::Smart(_) => anyhow::bail!("An Engine smartlist cannot contain child playlists"),
    }
}
fn source_parent_path(
    p: &crate::SyncPlaylist,
    library: &SyncLibrary,
    targets: &BTreeMap<String, Target>,
    previous: &BTreeMap<String, &PlaylistBinding>,
    old: &BTreeMap<i64, String>,
) -> Result<String> {
    let mut parts = Vec::new();
    let mut parent = p.parent_id.as_ref();
    let mut seen = BTreeSet::new();
    while let Some(id) = parent {
        ensure!(seen.insert(id), "Engine source hierarchy contains a cycle");
        if let Some(node) = library.playlists.iter().find(|node| &node.id == id) {
            parts.push(node.name.clone());
            parent = node.parent_id.as_ref();
        } else {
            let native = if let Some(t) = targets.get(id) {
                t.native.clone()
            } else {
                Native::parse(
                    &previous
                        .get(id)
                        .context("Engine source parent binding is missing")?
                        .locator,
                )?
            };
            let prefix = old
                .get(&ordinary_id(&native)?)
                .context("Engine native parent path is missing")?;
            parts.reverse();
            return Ok(format!("{prefix}{}", super::native_path(&parts)));
        }
    }
    parts.reverse();
    Ok(super::native_path(&parts))
}
fn native_parent(
    state: &Catalog,
    node: &Native,
    paths: &BTreeMap<i64, String>,
) -> Result<Option<i64>> {
    Ok(match node {
        Native::Playlist(id) => state.ordinary.get(id).map(|p| p.parent),
        Native::Smart(id) => state
            .smart
            .get(id)
            .map(|s| {
                paths
                    .iter()
                    .find(|(_, path)| **path == s.parent)
                    .map(|(id, _)| *id)
                    .context("Engine smart parent path is missing")
            })
            .transpose()?,
    })
}
fn natives_in_parent(
    state: &Catalog,
    parent: i64,
    paths: &BTreeMap<i64, String>,
) -> BTreeSet<Native> {
    state
        .ordinary
        .values()
        .filter(|n| n.parent == parent)
        .map(|n| Native::Playlist(n.id))
        .chain(
            state
                .smart
                .values()
                .filter(|n| n.parent == paths[&parent])
                .map(|n| Native::Smart(n.id.clone())),
        )
        .collect()
}
fn validate_names(state: &Catalog, targets: &BTreeMap<String, Target>) -> Result<()> {
    let paths = state.paths()?;
    for target in targets.values() {
        let (name, parent) = match &target.native {
            Native::Playlist(id) => {
                let p = &state.ordinary[id];
                (&p.name, paths[&p.parent].as_str())
            }
            Native::Smart(id) => {
                let p = &state.smart[id];
                ensure!(
                    !p.name.contains(';'),
                    "Engine smartlist name cannot contain semicolons"
                );
                (&p.name, p.parent.as_str())
            }
        };
        let matches = state
            .ordinary
            .values()
            .filter(|p| p.name == *name && paths[&p.parent] == parent)
            .count()
            + state
                .smart
                .values()
                .filter(|p| p.name == *name && p.parent == parent)
                .count();
        ensure!(
            matches == 1,
            "Engine playlist destination name collides with another native object"
        );
    }
    Ok(())
}
fn add_ancestors(state: &Catalog, node: &Native, output: &mut BTreeSet<i64>) -> Result<()> {
    let paths = state.paths()?;
    let Some(mut parent) = native_parent(state, node, &paths)? else {
        return Ok(());
    };
    let mut seen = BTreeSet::new();
    while parent != 0 {
        ensure!(seen.insert(parent), "Engine native ancestor cycle");
        output.insert(parent);
        parent = state
            .ordinary
            .get(&parent)
            .context("Engine ancestor is missing")?
            .parent;
    }
    Ok(())
}
fn merge_orders(
    before: &Catalog,
    after: &mut Catalog,
    library: &SyncLibrary,
    targets: &BTreeMap<String, Target>,
    affected: &BTreeSet<i64>,
    replacements: &BTreeMap<Native, Native>,
) -> Result<()> {
    let paths = after.paths()?;
    let parents: Vec<_> = affected
        .iter()
        .copied()
        .filter(|p| *p == 0 || after.ordinary.contains_key(p))
        .collect();
    for parent in parents {
        let desired: Vec<_> = library
            .playlists
            .iter()
            .filter(|p| targets[&p.id].parent == parent)
            .map(|p| targets[&p.id].native.clone())
            .collect();
        let candidates = natives_in_parent(after, parent, &paths);
        let existing = if parent == 0 || before.ordinary.contains_key(&parent) {
            before.order(parent)?
        } else {
            Vec::new()
        };
        let existing: Vec<_> = existing
            .into_iter()
            .map(|node| replacements.get(&node).cloned().unwrap_or(node))
            .filter(|n| candidates.contains(n))
            .collect();
        let selected: BTreeSet<_> = desired.iter().cloned().collect();
        let mut iter = desired.iter();
        let mut ordered = Vec::new();
        for node in existing {
            if selected.contains(&node) {
                if let Some(next) = iter.next() {
                    ordered.push(next.clone());
                }
            } else {
                ordered.push(node);
            }
        }
        ordered.extend(iter.cloned());
        ensure!(
            ordered.iter().cloned().collect::<BTreeSet<_>>() == candidates
                && ordered.len() == candidates.len(),
            "Engine merged sibling order is incomplete"
        );
        let ordinary: Vec<_> = ordered
            .iter()
            .filter_map(|n| {
                if let Native::Playlist(id) = n {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for (n, id) in ordinary.iter().enumerate() {
            after.ordinary.get_mut(id).unwrap().next = ordinary.get(n + 1).copied().unwrap_or(0);
        }
        for (n, node) in ordered.iter().enumerate() {
            if let Native::Smart(id) = node {
                let smart = after.smart.get_mut(id).unwrap();
                smart.next_uuid.clear();
                smart.next_path.clear();
                match ordered.get(n + 1) {
                    Some(Native::Playlist(id)) => smart.next_path = paths[id].clone(),
                    Some(Native::Smart(id)) => smart.next_uuid = id.clone(),
                    None => {}
                }
            }
        }
    }
    Ok(())
}
fn publish_structure(
    db: &Connection,
    before: &Catalog,
    after: &Catalog,
    affected: &BTreeSet<i64>,
    targets: &BTreeMap<String, Target>,
) -> Result<()> {
    let mut sentinel = before
        .ordinary
        .values()
        .map(|n| n.next)
        .min()
        .unwrap_or(0)
        .min(0);
    for node in before.ordinary.values().filter(|n| {
        affected.contains(&n.parent)
            || after
                .ordinary
                .get(&n.id)
                .is_some_and(|a| a.name != n.name || a.parent != n.parent)
    }) {
        sentinel = sentinel
            .checked_sub(1)
            .context("Engine sibling order overflow")?;
        if after
            .ordinary
            .get(&node.id)
            .is_none_or(|n| n.name != node.name || n.parent != node.parent)
        {
            db.execute(
                "UPDATE Playlist SET nextListId=?1,title=?2 WHERE id=?3",
                params![
                    sentinel,
                    format!("__cuerithm_{}", uuid::Uuid::new_v4()),
                    node.id
                ],
            )?;
        } else {
            db.execute(
                "UPDATE Playlist SET nextListId=?1 WHERE id=?2",
                params![sentinel, node.id],
            )?;
        }
    }
    let changed_smart: Vec<_> = after
        .smart
        .values()
        .filter(|s| before.smart.get(&s.id) != Some(*s))
        .collect();
    for node in &changed_smart {
        if before.smart.contains_key(&node.id) {
            db.execute(
                "UPDATE Smartlist SET parentPlaylistPath=?1 WHERE listUuid=?2",
                params![format!("__cuerithm_{};", uuid::Uuid::new_v4()), node.id],
            )?;
        }
    }
    for node in after.ordinary.values() {
        if !before.ordinary.contains_key(&node.id) {
            sentinel = sentinel
                .checked_sub(1)
                .context("Engine sibling order overflow")?;
            db.execute("INSERT INTO Playlist(id,title,parentListId,nextListId,isPersisted,isExplicitlyExported,lastEditTime) VALUES(?1,?2,?3,?4,1,0,datetime('now'))",params![node.id,node.name,node.parent,sentinel])?;
        } else if targets
            .values()
            .any(|t| t.native == Native::Playlist(node.id))
        {
            db.execute("UPDATE Playlist SET title=?1,parentListId=?2,isPersisted=1,lastEditTime=datetime('now') WHERE id=?3",params![node.name,node.parent,node.id])?;
        }
    }
    let mut dead: Vec<_> = before
        .ordinary
        .values()
        .filter(|n| !after.ordinary.contains_key(&n.id))
        .collect();
    let paths = before.paths()?;
    dead.sort_by_key(|n| std::cmp::Reverse(paths[&n.id].matches(';').count()));
    for node in dead {
        ensure!(
            !after.ordinary.values().any(|n| n.parent == node.id),
            "Engine deletion would cascade to a retained child"
        );
        db.execute("DELETE FROM PlaylistEntity WHERE listId=?1", [node.id])?;
        db.execute("DELETE FROM Playlist WHERE id=?1", [node.id])?;
    }
    for node in after
        .ordinary
        .values()
        .filter(|n| affected.contains(&n.parent))
    {
        db.execute(
            "UPDATE Playlist SET nextListId=?1 WHERE id=?2",
            params![node.next, node.id],
        )?;
    }
    for id in before
        .smart
        .keys()
        .filter(|id| !after.smart.contains_key(*id))
    {
        db.execute("DELETE FROM Smartlist WHERE listUuid=?1", [id])?;
    }
    for node in changed_smart {
        db.execute("INSERT INTO Smartlist(listUuid,title,parentPlaylistPath,nextListUuid,nextPlaylistPath,rules,lastEditTime) VALUES(?1,?2,?3,?4,?5,?6,datetime('now')) ON CONFLICT(listUuid) DO UPDATE SET title=excluded.title,parentPlaylistPath=excluded.parentPlaylistPath,nextListUuid=excluded.nextListUuid,nextPlaylistPath=excluded.nextPlaylistPath,rules=excluded.rules,lastEditTime=excluded.lastEditTime",params![node.id,node.name,node.parent,node.next_uuid,node.next_path,node.rules])?;
    }
    Ok(())
}
