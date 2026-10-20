use super::*;
use crate::ownership::{
    PlaylistBinding, PlaylistOwnership, PlaylistOwnershipReport, PlaylistOwnershipRequest,
    fingerprint,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Locator {
    pub space: i64,
    pub container: i64,
}

/// Recover only unambiguous standard local/removable paths. Persist the original
/// share root for network UUID databases: their filename cannot identify a share.
pub fn source_root_for_database(path: &Path) -> Result<Option<PathBuf>> {
    if path.file_name().and_then(|n| n.to_str()) == Some("root.sqlite") {
        return Ok(None);
    }
    ensure!(
        path.file_name().and_then(|n| n.to_str()) == Some("location.sqlite"),
        "Persist the original source root for Serato network/UUID locations"
    );
    let library = path.parent().context("Missing Serato Library directory")?;
    ensure!(
        library.file_name().and_then(|n| n.to_str()) == Some("Library"),
        "Invalid Serato location path"
    );
    let serato = library.parent().context("Missing _Serato_ directory")?;
    ensure!(
        serato.file_name().and_then(|n| n.to_str()) == Some("_Serato_"),
        "Invalid Serato location path"
    );
    Ok(Some(
        serato
            .parent()
            .context("Missing Serato volume root")?
            .to_path_buf(),
    ))
}

pub fn write_database_owned(
    path: &Path,
    source_root: Option<&Path>,
    library: &SyncLibrary,
    options: &SyncOptions,
    request: &PlaylistOwnershipRequest,
) -> Result<PlaylistOwnershipReport> {
    ensure!(
        path.extension().and_then(|n| n.to_str()) == Some("sqlite")
            && path.file_name().and_then(|n| n.to_str()) != Some("master.sqlite"),
        "Select a Serato location database"
    );
    ensure!(
        source_root.is_some() || path.file_name().and_then(|n| n.to_str()) == Some("root.sqlite"),
        "Nonlocal Serato databases require their original source root"
    );
    super::write_database_inner_owned(path, source_root, library, options, Some(request))
}

pub(super) fn identity(conn: &Connection) -> Result<String> {
    // serato_db.uuid identifies this individual DB; master.uuid is the master
    // registration and can be shared by several location DBs.
    let value: rusqlite::types::Value = conn
        .query_row("SELECT uuid FROM serato_db", [], |r| r.get(0))
        .context("Serato database has no native serato_db UUID")?;
    let uuid = match value {
        rusqlite::types::Value::Blob(b) => uuid::Uuid::from_slice(&b)?,
        rusqlite::types::Value::Text(s) => uuid::Uuid::parse_str(&s)?,
        _ => bail!("Invalid native Serato database UUID"),
    };
    ensure!(!uuid.is_nil(), "Serato database UUID is nil");
    Ok(format!("serato4:{uuid}"))
}
pub(super) fn table(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name=? AND type IN ('table','view'))",
        [name],
        |r| r.get(0),
    )?)
}
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn value(v: rusqlite::types::Value) -> serde_json::Value {
    use rusqlite::types::Value::*;
    match v {
        Null => serde_json::Value::Null,
        Integer(i) => i.into(),
        Real(f) => serde_json::json!(f),
        Text(s) => s.into(),
        Blob(b) => serde_json::json!({"blob":b}),
    }
}
fn rows(conn: &Connection, sql: &str, id: i64) -> Result<Vec<Vec<serde_json::Value>>> {
    let mut stmt = conn.prepare(sql)?;
    let count = stmt.column_count();
    let mut result = stmt
        .query_map([id], |r| {
            (0..count)
                .map(|i| r.get(i).map(value))
                .collect::<rusqlite::Result<Vec<_>>>()
        })?
        .collect::<rusqlite::Result<Vec<Vec<serde_json::Value>>>>()?;
    result.sort_by_cached_key(|row| serde_json::to_string(row).expect("JSON values serialize"));
    Ok(result)
}
/// Structural state only: revisions and connection timestamps change on normal
/// Serato operations and are deliberately outside this concurrency proof.
pub(super) fn state(conn: &Connection, id: i64) -> Result<Option<serde_json::Value>> {
    let core = rows(
        conn,
        "SELECT id,parent_id,name,type,list_order,space_id,portable_id,color FROM container WHERE id=?",
        id,
    )?;
    if core.is_empty() {
        return Ok(None);
    }
    let members = rows(
        conn,
        "SELECT ca.space_asset_id,ca.list_order,sa.asset_id,sa.space_id,a.portable_id FROM container_asset ca LEFT JOIN space_asset sa ON sa.id=ca.space_asset_id LEFT JOIN asset a ON a.id=sa.asset_id WHERE ca.container_id=? ORDER BY ca.list_order,ca.space_asset_id",
        id,
    )?;
    let children = rows(
        conn,
        "SELECT id,name,type,list_order FROM container WHERE parent_id=? ORDER BY id",
        id,
    )?;
    let rules = if table(conn, "smart_crate_rules")? {
        rows(
            conn,
            "SELECT version,rules FROM smart_crate_rules WHERE container_id=? ORDER BY version,rules",
            id,
        )?
    } else {
        vec![]
    };
    let mut references = BTreeMap::new();
    for t in [
        "alias_container",
        "container_keep_alive_list_tbl",
        "location_container",
    ] {
        if table(conn, t)? {
            references.insert(
                t,
                rows(
                    conn,
                    &format!(
                        "SELECT * FROM {} WHERE container_id=? ORDER BY container_id",
                        quote(t)
                    ),
                    id,
                )?,
            );
        }
    }
    Ok(Some(
        serde_json::json!({"container":core,"members":members,"children":children,"rules":rules,"references":references}),
    ))
}
pub(super) fn verify(conn: &Connection, b: &PlaylistBinding) -> Result<Locator> {
    let loc: Locator =
        serde_json::from_str(&b.locator).context("Invalid Serato ownership locator")?;
    ensure!(
        loc.container > 0 && loc.space > 0,
        "Protected Serato root container"
    );
    let state = state(conn, loc.container)?
        .context("Bound Serato crate is missing; reconcile the target")?;
    ensure!(
        fingerprint(&state)? == b.fingerprint,
        "Serato crate '{}' changed after the previous sync",
        b.source_id
    );
    let (space, kind): (i64, i64) = conn.query_row(
        "SELECT space_id,type FROM container WHERE id=?",
        [loc.container],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    ensure!(
        space == loc.space && [1, 2].contains(&kind),
        "Serato crate identity was reused or is protected"
    );
    Ok(loc)
}
/// No alias, keep-alive, location mapping or unfamiliar inbound FK may be
/// removed as a side effect of deleting an owned source container.
pub(super) fn deletion_references(
    conn: &Connection,
    id: i64,
    allow_locations: bool,
) -> Result<bool> {
    for t in [
        "alias_container",
        "container_keep_alive_list_tbl",
        "location_container",
    ] {
        if allow_locations && t == "location_container" {
            continue;
        }
        if table(conn, t)?
            && conn.query_row(
                &format!(
                    "SELECT EXISTS(SELECT 1 FROM {} WHERE container_id=?)",
                    quote(t)
                ),
                [id],
                |r| r.get::<_, bool>(0),
            )?
        {
            return Ok(true);
        }
    }
    let tables = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for t in tables {
        let fks = conn
            .prepare(&format!("PRAGMA foreign_key_list({})", quote(&t)))?
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (target, from, to) in fks {
            if target.eq_ignore_ascii_case("container") && to.as_deref().is_none_or(|to| to == "id")
            {
                if ["container", "container_asset", "smart_crate_rules"].contains(&t.as_str())
                    || (allow_locations && t == "location_container")
                {
                    continue;
                }
                if conn.query_row(
                    &format!(
                        "SELECT EXISTS(SELECT 1 FROM {} WHERE {}=?)",
                        quote(&t),
                        quote(&from)
                    ),
                    [id],
                    |r| r.get::<_, bool>(0),
                )? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

pub(super) fn write_playlists_owned(
    conn: &Connection,
    library: &SyncLibrary,
    options: &SyncOptions,
    tracks: &HashMap<String, i64>,
    destination: (i64, i64),
    clock: (&mut i64, i64),
    request: &PlaylistOwnershipRequest,
) -> Result<PlaylistOwnershipReport> {
    let (root, space) = destination;
    let (revision, now) = clock;
    let identity = identity(conn)?;
    ensure!(
        request.previous.identity == identity
            || (request.previous.identity.is_empty() && request.previous.bindings.is_empty()),
        "Serato database identity changed; reconcile ownership"
    );
    let mut bindings: BTreeMap<String, PlaylistBinding> = BTreeMap::new();
    let mut native = HashMap::new();
    for b in &request.previous.bindings {
        ensure!(
            bindings.insert(b.source_id.clone(), b.clone()).is_none(),
            "Duplicate Serato source binding"
        );
        let loc: Locator = serde_json::from_str(&b.locator)?;
        ensure!(
            loc.space == space && loc.container > 0,
            "Serato ownership belongs to a different space or protected root"
        );
        ensure!(
            native.insert(loc.container, b.source_id.clone()).is_none(),
            "Two sources claim one native Serato container"
        );
    }
    let active: HashSet<_> = library.playlists.iter().map(|p| p.id.clone()).collect();
    ensure!(
        active.len() == library.playlists.len(),
        "Duplicate Serato playlist source IDs"
    );
    ensure!(
        active.iter().all(|id| !request.removed.contains(id)),
        "Active Serato source is also removed"
    );
    let mut affected: BTreeSet<String> = active.iter().cloned().collect();
    affected.extend(
        request
            .removed
            .iter()
            .filter(|id| bindings.get(*id).is_some_and(|b| b.created))
            .cloned(),
    );
    // Child names/order/parent changes also change the direct parent's proof.
    for id in affected.clone() {
        if let Some(parent) = bindings.get(&id).and_then(|b| b.parent_source_id.clone()) {
            affected.insert(parent);
        }
    }
    for p in &library.playlists {
        if let Some(parent) = &p.parent_id {
            affected.insert(parent.clone());
        }
    }
    let mut resolved = HashMap::new();
    for id in &affected {
        if let Some(b) = bindings.get(id) {
            if request.removed.contains(id) && b.created {
                let loc: Locator = serde_json::from_str(&b.locator)?;
                if state(conn, loc.container)?.is_none() {
                    resolved.insert(id.clone(), loc.container);
                    continue;
                }
            }
            resolved.insert(id.clone(), verify(conn, b)?.container);
        }
    }
    let before: HashSet<i64> = conn
        .prepare("SELECT id FROM container")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    // Free every bound name before final assignments, making rename swaps atomic.
    for id in active.iter().chain(request.removed.iter()) {
        if let Some(b) = bindings
            .get(id)
            .filter(|b| active.contains(id) || b.created)
        {
            let loc: Locator = serde_json::from_str(&b.locator)?;
            conn.execute(
                "UPDATE container SET name=? WHERE id=?",
                params![
                    format!("CueRithm staging {}", uuid::Uuid::new_v4()),
                    loc.container
                ],
            )?;
        }
    }
    let has_smart = table(conn, "smart_crate_rules")?;
    let mut pending: Vec<_> = library.playlists.iter().collect();
    let mut names = HashSet::new();
    while !pending.is_empty() {
        let count = pending.len();
        let mut later = Vec::new();
        for p in pending {
            let parent = match &p.parent_id {
                None => root,
                Some(id) => {
                    if active.contains(id)
                        && !names
                            .iter()
                            .any(|(source, _, _): &(String, i64, String)| source == id)
                    {
                        later.push(p);
                        continue;
                    }
                    if let Some(id) = resolved.get(id) {
                        *id
                    } else if let Some(b) = bindings.get(id) {
                        verify(conn, b)?.container
                    } else {
                        later.push(p);
                        continue;
                    }
                }
            };
            ensure!(
                !request
                    .removed
                    .contains(p.parent_id.as_deref().unwrap_or("")),
                "Serato parent is removed"
            );
            ensure!(
                !names
                    .iter()
                    .any(|(_, pa, n)| *pa == parent && *n == p.name.to_lowercase()),
                "Duplicate Serato sibling names"
            );
            let native_rules = match crate::smart_rules::compile(
                p,
                library,
                options,
                crate::DjSoftware::Serato4,
            ) {
                Ok(Some(crate::smart_rules::NativeSmart::Serato4(r))) => Some(r),
                _ => None,
            };
            ensure!(
                native_rules.is_none() || has_smart,
                "Serato smart rules table is missing"
            );
            let kind = if native_rules.is_some() { 2 } else { 1 };
            let id = if let Some(id) = resolved.get(&p.id).copied() {
                ensure!(
                    !p.name.contains(SMART_CRATE_DELIMITER),
                    "Reserved Serato smart delimiter in crate name"
                );
                let child: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM container WHERE parent_id=?)",
                    [id],
                    |r| r.get(0),
                )?;
                ensure!(
                    kind != 2 || !child,
                    "A Serato folder with children cannot become smart"
                );
                use sha2::{Digest, Sha256};
                let hash = format!("{:x}", Sha256::digest(p.id.as_bytes()));
                let name = if kind == 2 {
                    format!("{}{SMART_CRATE_DELIMITER}{}", p.name, &hash[..32])
                } else {
                    p.name.clone()
                };
                let collision:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM container WHERE parent_id=? AND name=? COLLATE NOCASE AND id<>?)",params![parent,name,id],|r|r.get(0))?;
                ensure!(
                    !collision,
                    "Serato destination name is occupied by another native crate"
                );
                conn.execute("UPDATE container SET name=?,parent_id=?,type=?,space_id=?,revision=?,list_order=CASE WHEN parent_id<>? THEN (SELECT COALESCE(MAX(c.list_order),0)+1 FROM container c WHERE c.parent_id=?) ELSE list_order END WHERE id=?",params![name,parent,kind,space,next_revision(revision)?,parent,parent,id])?;
                id
            } else {
                smart_container(conn, p, parent, kind == 2, space, revision, now)?
            };
            ensure!(
                !resolved
                    .iter()
                    .any(|(source, native)| source != &p.id && *native == id),
                "Two Serato sources resolve to the same native crate"
            );
            resolved.insert(p.id.clone(), id);
            names.insert((p.id.clone(), parent, p.name.to_lowercase()));
            super::write_playlist_contents(conn, p, id, native_rules, has_smart, tracks, revision)?;
            let created = bindings
                .get(&p.id)
                .map_or(!before.contains(&id), |b| b.created);
            bindings.insert(
                p.id.clone(),
                PlaylistBinding {
                    source_id: p.id.clone(),
                    locator: serde_json::to_string(&Locator {
                        space,
                        container: id,
                    })?,
                    parent_source_id: p.parent_id.clone(),
                    created,
                    fingerprint: String::new(),
                },
            );
        }
        ensure!(
            later.len() < count,
            "Serato playlist hierarchy contains a missing parent or cycle"
        );
        pending = later;
    }
    for p in &library.playlists {
        let mut seen = HashSet::new();
        let mut parent = Some(resolved[&p.id]);
        while let Some(id) = parent {
            ensure!(
                seen.insert(id),
                "Serato move would create a native parent cycle"
            );
            parent = conn
                .query_row("SELECT parent_id FROM container WHERE id=?", [id], |r| {
                    r.get::<_, Option<i64>>(0)
                })
                .context("Serato native parent is missing")?;
        }
    }
    // Reuse only selected siblings' existing order slots. Unselected native
    // crates keep both their relative order and their stored list_order.
    let mut desired: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for p in &library.playlists {
        let parent = if let Some(source) = &p.parent_id {
            *resolved.get(source).context("Unresolved Serato parent")?
        } else {
            root
        };
        desired.entry(parent).or_default().push(resolved[&p.id]);
    }
    for (parent, ids) in desired {
        let selected: HashSet<_> = ids.iter().copied().collect();
        let slots = conn
            .prepare(
                "SELECT id,list_order FROM container WHERE parent_id=? ORDER BY list_order,id",
            )?
            .query_map([parent], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let selected_slots: Vec<_> = slots
            .iter()
            .filter(|(id, _)| selected.contains(id))
            .collect();
        ensure!(
            selected_slots.len() == ids.len(),
            "Serato sibling ordering lost a selected crate"
        );
        if selected_slots.iter().map(|(id, _)| *id).collect::<Vec<_>>() != ids {
            ensure!(
                selected_slots.windows(2).all(|pair| pair[0].1 < pair[1].1),
                "Ambiguous native Serato sibling order requires reconciliation"
            );
            let maximum = slots
                .iter()
                .map(|(_, order)| *order)
                .max()
                .unwrap_or(0)
                .max(0);
            for (index, id) in ids.iter().enumerate() {
                let temporary = maximum
                    .checked_add(index as i64 + 1)
                    .context("Serato sibling order overflow")?;
                conn.execute(
                    "UPDATE container SET list_order=? WHERE id=?",
                    params![temporary, id],
                )?;
            }
            for (id, slot) in ids.iter().zip(selected_slots) {
                conn.execute(
                    "UPDATE container SET list_order=?,revision=? WHERE id=?",
                    params![slot.1, next_revision(revision)?, id],
                )?;
            }
        }
    }
    let mut deleted = BTreeSet::new();
    let mut retired = BTreeSet::new();
    let mut removing = Vec::new();
    for id in &request.removed {
        let Some(b) = bindings.get(id) else { continue };
        if !b.created {
            retired.insert(id.clone());
            continue;
        }
        let loc: Locator = serde_json::from_str(&b.locator)?;
        removing.push((id.clone(), loc.container));
    }
    while !removing.is_empty() {
        let before = removing.len();
        let mut later = Vec::new();
        for (source, id) in removing {
            if state(conn, id)?.is_none() {
                deleted.insert(source);
                continue;
            }
            ensure!(
                !deletion_references(conn, id, false)?,
                "Owned Serato crate has foreign aliases or references"
            );
            let children: Vec<i64> = conn
                .prepare("SELECT id FROM container WHERE parent_id=?")?
                .query_map([id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            ensure!(
                children
                    .iter()
                    .all(|child| later.iter().any(|(_, id)| id == child)
                        || request
                            .removed
                            .iter()
                            .filter_map(|s| resolved.get(s))
                            .any(|id| id == child)),
                "Owned Serato crate has foreign or retained children"
            );
            if !children.is_empty() {
                later.push((source, id));
                continue;
            }
            conn.execute("DELETE FROM container_asset WHERE container_id=?", [id])?;
            if has_smart {
                conn.execute("DELETE FROM smart_crate_rules WHERE container_id=?", [id])?;
            }
            conn.execute(
                "DELETE FROM container WHERE id=? AND type IN (1,2) AND space_id=?",
                params![id, space],
            )?;
            next_revision(revision)?;
            deleted.insert(source);
        }
        ensure!(
            later.len() < before,
            "Serato deletion hierarchy has a cycle"
        );
        removing = later;
    }
    for id in &request.removed {
        bindings.remove(id);
    }
    for (id, b) in &mut bindings {
        if affected.contains(id) {
            let loc: Locator = serde_json::from_str(&b.locator)?;
            b.fingerprint = fingerprint(
                &state(conn, loc.container)?.context("Serato crate disappeared during staging")?,
            )?;
        }
    }
    Ok(PlaylistOwnershipReport {
        current: PlaylistOwnership {
            identity,
            bindings: bindings.into_values().collect(),
        },
        deleted,
        retired,
    })
}

#[cfg(test)]
#[path = "owned_tests.rs"]
mod tests;
