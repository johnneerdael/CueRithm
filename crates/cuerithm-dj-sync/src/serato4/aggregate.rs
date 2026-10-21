use super::{container, register_functions};
mod ownership;
use anyhow::{Context, Result, ensure};
pub use ownership::refresh_master_owned;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params, params_from_iter, types::Value};
use std::{
    collections::HashMap,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

struct SourceContainer {
    id: i64,
    parent: Option<i64>,
    name: String,
    kind: i64,
    order: i64,
}

/// Refresh a staged master aggregate from a staged root/external/network database.
/// `original_location` is the destination URI already registered by Serato in
/// `master.connection`; it must not be the temporary staged filename.
pub fn refresh_master(
    master: &Path,
    staged_location: &Path,
    original_location: &Path,
) -> Result<()> {
    refresh_master_inner(master, staged_location, original_location, None).map(|_| ())
}

fn refresh_master_inner(
    master: &Path,
    staged_location: &Path,
    original_location: &Path,
    ownership_request: Option<(
        &crate::ownership::PlaylistOwnershipRequest,
        &crate::ownership::PlaylistOwnershipReport,
    )>,
) -> Result<crate::ownership::PlaylistOwnershipReport> {
    let mut dst = Connection::open_with_flags(master, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    register_functions(&dst)?;
    let src = Connection::open_with_flags(staged_location, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let reference = super::native_serato_path(original_location)?;
    let mut matches = dst
        .prepare("SELECT location_id,database_uri FROM connection")?
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, uri)| super::normalize(uri) == super::normalize(&reference));
    let location = matches
        .next()
        .context(
            "Location is not registered in Serato master.sqlite; initialize it in Serato first",
        )?
        .0;
    ensure!(
        matches.next().is_none(),
        "Ambiguous Serato location registration"
    );
    let tx = dst.transaction()?;
    let mut ownership = ownership_request
        .map(|(request, local)| ownership::State::new(&tx, request, local, location))
        .transpose()?;
    let source_has_rules = has_rules_table(&src)?;
    let destination_has_rules = has_rules_table(&tx)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let mut revision = src.query_row("SELECT revision FROM master LIMIT 1", [], |r| {
        r.get::<_, i64>(0)
    })?;
    let space: i64 = tx.query_row(
        "SELECT id FROM space WHERE name='Serato Library' COLLATE NOCASE",
        [],
        |r| r.get(0),
    )?;
    let source_space: i64 = src.query_row(
        "SELECT id FROM space WHERE name='Serato Library' COLLATE NOCASE",
        [],
        |r| r.get(0),
    )?;
    let root: Option<i64> = tx
        .query_row(
            "SELECT id FROM container WHERE type=0 AND space_id=?",
            [space],
            |r| r.get(0),
        )
        .optional()?;
    let root = if let Some(root) = root {
        root
    } else {
        container(&tx, "Serato Library root", 0, 0, space, &mut revision, now)?
    };
    let destination_columns = columns(&tx, "asset")?;
    let columns: Vec<String> = columns(&src, "asset")?
        .into_iter()
        .filter(|c| !["id", "revision", "external_id", "location_id"].contains(&c.as_str()))
        .filter(|c| destination_columns.contains(c))
        .collect();
    let query = format!(
        "SELECT id,{} FROM asset",
        columns
            .iter()
            .map(|c| quote(c))
            .collect::<Vec<_>>()
            .join(",")
    );
    let rows = src
        .prepare(&query)?
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (0..columns.len())
                    .map(|i| r.get::<_, Value>(i + 1))
                    .collect::<rusqlite::Result<Vec<_>>>()?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut assets = HashMap::new();
    for (source_id, values) in rows {
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM asset WHERE location_id=? AND external_id=?",
                params![location, source_id],
                |r| r.get(0),
            )
            .optional()?;
        let id = if let Some(id) = existing {
            update(&tx, "asset", id, &columns, &values)?;
            id
        } else {
            let mut cols = columns.clone();
            cols.extend(["external_id".into(), "location_id".into()]);
            let mut vals = values;
            vals.extend([Value::Integer(source_id), Value::Integer(location)]);
            insert(&tx, "asset", &cols, &vals)?
        };
        tx.execute("INSERT INTO space_asset(asset_id,space_id) SELECT ?1,?2 WHERE NOT EXISTS(SELECT 1 FROM space_asset WHERE asset_id=?1 AND space_id=?2)",params![id,space])?;
        let link: i64 = tx.query_row(
            "SELECT id FROM space_asset WHERE asset_id=? AND space_id=?",
            params![id, space],
            |r| r.get(0),
        )?;
        assets.insert(source_id, (id, link));
    }
    let mut containers = HashMap::new();
    let source_containers=src.prepare("SELECT id,parent_id,name,type,list_order FROM container WHERE space_id=? AND type<>0 ORDER BY list_order")?
        .query_map([source_space],|r|Ok(SourceContainer{id:r.get(0)?,parent:r.get(1)?,name:r.get(2)?,kind:r.get(3)?,order:r.get(4)?}))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let source_root: i64 = src.query_row(
        "SELECT id FROM container WHERE type=0 AND space_id=?",
        [source_space],
        |r| r.get(0),
    )?;
    containers.insert(source_root, root);
    let mut pending = source_containers;
    while !pending.is_empty() {
        let before = pending.len();
        let mut later = Vec::new();
        for source in pending {
            let Some(parent) = source.parent.and_then(|p| containers.get(&p).copied()) else {
                later.push(source);
                continue;
            };
            let previous:Option<i64>=tx.query_row("SELECT container_id FROM location_container WHERE location_id=? AND external_container_id=?",params![location,source.id],|row|row.get(0)).optional()?;
            let target = resolve_container(
                &tx,
                &source,
                parent,
                space,
                location,
                (&mut revision, now),
                ownership.as_ref(),
            )?;
            tx.execute(
                "UPDATE container SET list_order=? WHERE id=?",
                params![source.order, target],
            )?;
            tx.execute("INSERT INTO location_container(container_id,location_id,external_container_id) SELECT ?1,?2,?3 WHERE NOT EXISTS(SELECT 1 FROM location_container WHERE location_id=?2 AND external_container_id=?3)",params![target,location,source.id])?;
            tx.execute("UPDATE location_container SET container_id=? WHERE location_id=? AND external_container_id=?",params![target,location,source.id])?;
            copy_rules(
                &tx,
                &src,
                &source,
                target,
                source_has_rules,
                destination_has_rules,
            )?;
            if let Some(previous) = previous.filter(|previous| *previous != target) {
                if let Some(owned) = &mut ownership {
                    owned.cleanup(&tx, previous)?;
                } else {
                    remove_unreferenced_container(&tx, previous, destination_has_rules)?;
                }
            }
            containers.insert(source.id, target);
        }
        ensure!(
            later.len() < before,
            "Serato source crate hierarchy has an unresolved parent"
        );
        pending = later;
    }
    tx.execute("DELETE FROM container_asset WHERE location_container_id IN (SELECT id FROM location_container WHERE location_id=?)",[location])?;
    let links=src.prepare("SELECT ca.id,ca.container_id,sa.asset_id,ca.list_order FROM container_asset ca JOIN space_asset sa ON sa.id=ca.space_asset_id WHERE sa.space_id=?")?
        .query_map([source_space],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
    for (external_id, source_container, source_asset, order) in links {
        let Some(container) = containers.get(&source_container) else {
            continue;
        };
        let Some((asset, link)) = assets.get(&source_asset) else {
            continue;
        };
        let location_container:i64=tx.query_row("SELECT id FROM location_container WHERE location_id=? AND container_id=? AND external_container_id=?",params![location,container,source_container],|r|r.get(0))?;
        tx.execute("INSERT INTO container_asset(asset_id,location_container_id,space_asset_id,external_container_asset_id,list_order) VALUES (?,?,?,?,?)",params![asset,location_container,link,external_id,order])?;
    }
    let report = if let Some(ownership) = ownership {
        ownership.finish(&tx, location, space, &containers)?
    } else {
        crate::ownership::PlaylistOwnershipReport::default()
    };
    tx.commit()?;
    Ok(report)
}

fn has_rules_table(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='smart_crate_rules')",[],|row|row.get(0))?)
}

fn resolve_container(
    conn: &Connection,
    source: &SourceContainer,
    parent: i64,
    space: i64,
    location: i64,
    clock: (&mut i64, i64),
    ownership: Option<&ownership::State>,
) -> Result<i64> {
    let (revision, now) = clock;
    let exact: Option<i64> = conn
        .query_row(
            "SELECT id FROM container WHERE parent_id=? AND type=? AND name=? COLLATE NOCASE",
            params![parent, source.kind, source.name],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(owned) = ownership {
        if let Some(preferred) =
            owned.preferred(conn, source.id, parent, &source.name, source.kind)?
        {
            return Ok(preferred);
        }
    } else if exact.is_some() {
        return container(
            conn,
            &source.name,
            parent,
            source.kind,
            space,
            revision,
            now,
        );
    }
    let previous:Option<i64>=conn.query_row("SELECT container_id FROM location_container WHERE location_id=? AND external_container_id=?",params![location,source.id],|row|row.get(0)).optional()?;
    if let Some(previous) = previous {
        let foreign:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM location_container WHERE container_id=? AND location_id<>?)",params![previous,location],|row|row.get(0))?;
        let children: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM container WHERE parent_id=?)",
            [previous],
            |row| row.get(0),
        )?;
        if !foreign && (source.kind != 2 || !children) {
            ensure!(
                exact.is_none_or(|id| id == previous),
                "Serato aggregate destination name is occupied by another native crate"
            );
            *revision = revision
                .checked_add(1)
                .context("Serato revision overflow")?;
            conn.execute(
                "UPDATE container SET name=?,parent_id=?,type=?,space_id=?,revision=? WHERE id=?",
                params![source.name, parent, source.kind, space, *revision, previous],
            )?;
            return Ok(previous);
        }
    }
    let exact: Option<i64> = conn
        .query_row(
            "SELECT id FROM container WHERE parent_id=? AND type=? AND name=? COLLATE NOCASE",
            params![parent, source.kind, source.name],
            |row| row.get(0),
        )
        .optional()?;
    if exact.is_some() {
        return container(
            conn,
            &source.name,
            parent,
            source.kind,
            space,
            revision,
            now,
        );
    }
    container(
        conn,
        &source.name,
        parent,
        source.kind,
        space,
        revision,
        now,
    )
}

fn copy_rules(
    destination: &Connection,
    source: &Connection,
    container: &SourceContainer,
    target: i64,
    source_has_rules: bool,
    destination_has_rules: bool,
) -> Result<()> {
    if container.kind != 2 {
        if destination_has_rules {
            destination.execute(
                "DELETE FROM smart_crate_rules WHERE container_id=?",
                [target],
            )?;
        }
        return Ok(());
    }
    ensure!(
        source_has_rules && destination_has_rules,
        "Native Serato smart crates require the smart_crate_rules table in both databases"
    );
    let (revision, version, rules): (i64, i64, Value) = source
        .query_row(
            "SELECT revision,version,rules FROM smart_crate_rules WHERE container_id=?",
            [container.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .context("Native Serato smart crate has no rules record")?;
    let updated = destination.execute(
        "UPDATE smart_crate_rules SET revision=?,version=?,rules=? WHERE container_id=?",
        params![revision, version, rules, target],
    )?;
    if updated == 0 {
        destination.execute(
            "INSERT INTO smart_crate_rules(container_id,revision,version,rules) VALUES(?,?,?,?)",
            params![target, revision, version, rules],
        )?;
    }
    Ok(())
}

/// A remapped crate can remain shared with another location. Remove only an
/// obsolete aggregate container which no longer has location, child or direct
/// membership references; never delete another location's smart rules.
fn remove_unreferenced_container(conn: &Connection, id: i64, has_rules: bool) -> Result<()> {
    let referenced:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM location_container WHERE container_id=?1) OR EXISTS(SELECT 1 FROM container WHERE parent_id=?1) OR EXISTS(SELECT 1 FROM container_asset WHERE container_id=?1)",[id],|row|row.get(0))?;
    if referenced || super::owned::deletion_references(conn, id, false)? {
        return Ok(());
    }
    if has_rules {
        conn.execute("DELETE FROM smart_crate_rules WHERE container_id=?", [id])?;
    }
    conn.execute("DELETE FROM container WHERE id=?", [id])?;
    Ok(())
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    Ok(conn
        .prepare(&format!("PRAGMA table_info({})", quote(table)))?
        .query_map([], |r| r.get(1))?
        .collect::<rusqlite::Result<_>>()?)
}
fn insert(conn: &Connection, table: &str, columns: &[String], values: &[Value]) -> Result<i64> {
    conn.execute(
        &format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote(table),
            columns
                .iter()
                .map(|c| quote(c))
                .collect::<Vec<_>>()
                .join(","),
            vec!["?"; values.len()].join(",")
        ),
        params_from_iter(values),
    )?;
    Ok(conn.last_insert_rowid())
}
fn update(
    conn: &Connection,
    table: &str,
    id: i64,
    columns: &[String],
    values: &[Value],
) -> Result<()> {
    let mut values = values.to_vec();
    values.push(Value::Integer(id));
    conn.execute(
        &format!(
            "UPDATE {} SET {} WHERE id=?",
            quote(table),
            columns
                .iter()
                .map(|c| format!("{}=?", quote(c)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        params_from_iter(values),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "aggregate_tests.rs"]
mod tests;
