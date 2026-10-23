//! Removes library records for tracks and puts them back exactly.
//!
//! A track is the root of a large web of rows (jobs, cue revisions, links, tag
//! and metadata history, ...). Instead of naming those tables by hand, the web
//! is discovered from the schema's own foreign keys, so a table added later is
//! covered without touching this file. Every affected row is captured as data
//! before it is deleted, which is what makes removal reversible.
//!
//! Policy for a child row that points at a removed row:
//! - any required (NOT NULL) reference: the row only means something with its
//!   parent, so it is removed with it;
//! - only optional references: the row stays and the reference is cleared. In
//!   this schema that is a rekordbox link, which then reads as a DJ-software
//!   track that has no library counterpart.
//!
//! Playlist membership is kept out of this web on purpose: it has ordered
//! positions that the playlist code owns, so the caller rewrites it.
//!
//! This is the only engine that removes and restores track records. Duplicate merges
//! that delete files and Missing & broken removals both go through it (see
//! `maintenance::track_removal`), so what is saved, what is purged and how Undo
//! checks the library cannot differ between them.
use anyhow::{Context, Result, bail, ensure};
use rusqlite::{Connection, ToSql, Transaction, params_from_iter, types::ValueRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// One table's rows, as data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TableImage {
    pub table: String,
    pub columns: Vec<String>,
    /// Declared primary key columns. Rows are identified by these, never by SQLite's
    /// internal rowid, which is handed out again once a row is deleted.
    #[serde(default)]
    pub key: Vec<String>,
    /// Rows that stay in place with an optional reference cleared, not deleted rows.
    pub detached: bool,
    /// For detached rows: the columns the removal changed and the values it left there.
    /// Undo puts the saved values back only where these are still what the removal left.
    #[serde(default)]
    pub owned: Vec<String>,
    #[serde(default)]
    pub held: Vec<Value>,
    /// Saved rows, in their original order. The number is only informational.
    pub rows: Vec<(i64, Vec<Value>)>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct RecordImage {
    pub tables: Vec<TableImage>,
}
/// Identity of a track record that a removal took out of the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemovedTrack {
    pub id: String,
    /// The file location the record had, as stored. A track that is not linked to a local
    /// file has none (and no content hash either).
    pub path: Option<Vec<u8>>,
    pub hash: Option<String>,
    pub title: String,
}
impl RecordImage {
    pub(crate) fn table(&self, name: &str) -> Option<&TableImage> {
        self.tables.iter().find(|t| t.table == name)
    }
    /// Identity, file location and source hash of each removed track record.
    pub(crate) fn removed_tracks(&self) -> Result<Vec<RemovedTrack>> {
        let Some(table) = self.table("tracks").filter(|t| !t.detached) else {
            return Ok(vec![]);
        };
        let at = |name: &str| table.columns.iter().position(|c| c == name);
        let (Some(id), Some(path), Some(hash)) = (at("id"), at("path"), at("source_hash")) else {
            bail!("The saved records are damaged");
        };
        let title = at("title");
        table
            .rows
            .iter()
            .map(|(_, values)| {
                Ok(RemovedTrack {
                    id: values[id]
                        .as_str()
                        .context("The saved records are damaged")?
                        .to_owned(),
                    path: match &values[path] {
                        Value::Null => None,
                        other => Some(bytes_of(other).context("The saved records are damaged")?),
                    },
                    hash: values[hash].as_str().map(str::to_owned),
                    title: title
                        .and_then(|at| values[at].as_str())
                        .unwrap_or_default()
                        .to_owned(),
                })
            })
            .collect()
    }
}

struct Column {
    name: String,
    required: bool,
}
struct ForeignKey {
    parent: String,
    from: Vec<String>,
    to: Vec<String>,
}
struct Table {
    columns: Vec<Column>,
    keys: Vec<ForeignKey>,
    primary: Vec<String>,
}
fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
fn schema(conn: &Connection) -> Result<BTreeMap<String, Table>> {
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut tables = BTreeMap::new();
    for name in names {
        let mut columns = Vec::new();
        let mut primary = Vec::new();
        let mut statement = conn.prepare(&format!("PRAGMA table_info({})", quote(&name)))?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        for row in rows {
            let (column, kind, not_null, pk) = row?;
            if pk > 0 {
                primary.push((pk, column.clone(), kind.to_ascii_uppercase()));
            }
            columns.push(Column {
                required: not_null || pk > 0,
                name: column,
            });
        }
        primary.sort();
        let mut grouped: BTreeMap<i64, ForeignKey> = BTreeMap::new();
        let mut statement = conn.prepare(&format!("PRAGMA foreign_key_list({})", quote(&name)))?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        for row in rows {
            let (id, parent, from, to) = row?;
            let key = grouped.entry(id).or_insert_with(|| ForeignKey {
                parent,
                from: vec![],
                to: vec![],
            });
            key.from.push(from);
            key.to.push(to.unwrap_or_default());
        }
        tables.insert(
            name,
            Table {
                columns,
                keys: grouped.into_values().collect(),
                primary: primary.into_iter().map(|(_, column, _)| column).collect(),
            },
        );
    }
    // A reference to the parent's primary key names no column; resolve it.
    let primaries: BTreeMap<String, Vec<String>> = tables
        .iter()
        .map(|(name, table)| (name.clone(), table.primary.clone()))
        .collect();
    for table in tables.values_mut() {
        for key in &mut table.keys {
            if key.to.iter().any(String::is_empty) {
                key.to = primaries.get(&key.parent).cloned().unwrap_or_default();
            }
        }
    }
    Ok(tables)
}

/// What a removal changes on a row that stays: its optional references are
/// cleared, and a rekordbox link is marked as having no library track.
fn detach_set(name: &str, table: &Table) -> Vec<(String, Value)> {
    let mut set: Vec<(String, Value)> = table
        .keys
        .iter()
        .filter(|key| {
            !key.from
                .iter()
                .any(|from| table.columns.iter().any(|c| &c.name == from && c.required))
        })
        .flat_map(|key| key.from.iter())
        .map(|from| (from.clone(), Value::Null))
        .collect();
    if name == "native_track_links" {
        // The existing status for a DJ-software track without a library match.
        set.push((
            "status".into(),
            Value::String("not_in_local_library".into()),
        ));
    }
    set
}

fn placeholders(count: usize) -> String {
    vec!["?"; count].join(",")
}
/// Everything that goes when `track_ids` go.
pub(crate) struct Plan {
    /// Caches derived from a track: deleted with it, but not saved for Undo.
    derived: Vec<String>,
    order: Vec<String>,
    deleted: BTreeMap<String, BTreeSet<i64>>,
    detached: BTreeMap<String, BTreeSet<i64>>,
}
impl Plan {
    /// `skip` names tables the caller handles itself; `derived` names caches that
    /// go with the track and are not worth keeping for Undo.
    pub(crate) fn new(
        conn: &Connection,
        track_ids: &[String],
        skip: &[&str],
        derived: &[&str],
    ) -> Result<Self> {
        let tables = schema(conn)?;
        ensure!(
            tables.contains_key("tracks"),
            "Library schema has no tracks"
        );
        let mut plan = Plan {
            derived: derived.iter().map(|t| t.to_string()).collect(),
            order: vec!["tracks".into()],
            deleted: BTreeMap::new(),
            detached: BTreeMap::new(),
        };
        let mut roots = BTreeSet::new();
        for chunk in track_ids.chunks(400) {
            let mut statement = conn.prepare(&format!(
                "SELECT rowid FROM tracks WHERE id IN ({})",
                placeholders(chunk.len())
            ))?;
            for row in statement.query_map(params_from_iter(chunk), |row| row.get::<_, i64>(0))? {
                roots.insert(row?);
            }
        }
        plan.deleted.insert("tracks".into(), roots.clone());
        let mut queue = vec![("tracks".to_owned(), roots)];
        while let Some((parent, fresh)) = queue.pop() {
            let fresh: Vec<i64> = fresh.into_iter().collect();
            for (child, table) in &tables {
                if skip.contains(&child.as_str()) {
                    continue;
                }
                for key in table.keys.iter().filter(|key| key.parent == parent) {
                    ensure!(
                        !key.to.is_empty() && key.to.len() == key.from.len(),
                        "Cannot resolve a reference from {child} to {parent}"
                    );
                    let required = key.from.iter().any(|from| {
                        table
                            .columns
                            .iter()
                            .any(|column| &column.name == from && column.required)
                    });
                    let join = key
                        .from
                        .iter()
                        .zip(&key.to)
                        .map(|(from, to)| format!("c.{}=p.{}", quote(from), quote(to)))
                        .collect::<Vec<_>>()
                        .join(" AND ");
                    let mut found = BTreeSet::new();
                    for chunk in fresh.chunks(400) {
                        let mut statement = conn.prepare(&format!(
                            "SELECT c.rowid FROM {} c JOIN {} p ON {join} WHERE p.rowid IN ({})",
                            quote(child),
                            quote(&parent),
                            placeholders(chunk.len())
                        ))?;
                        for row in statement
                            .query_map(params_from_iter(chunk), |row| row.get::<_, i64>(0))?
                        {
                            found.insert(row?);
                        }
                    }
                    if required {
                        let known = plan.deleted.entry(child.clone()).or_default();
                        let added: BTreeSet<i64> = found.difference(known).copied().collect();
                        if !added.is_empty() {
                            known.extend(&added);
                            if !plan.order.contains(child) {
                                plan.order.push(child.clone());
                            }
                            queue.push((child.clone(), added));
                        }
                    } else if !found.is_empty() {
                        plan.detached
                            .entry(child.clone())
                            .or_default()
                            .extend(found);
                        if !plan.order.contains(child) {
                            plan.order.push(child.clone());
                        }
                    }
                }
            }
        }
        // Portable records carry logical references in JSON rather than SQLite
        // foreign keys. Save and remove the track-owned records with the track.
        if tables.contains_key("portable_domain_records") {
            let mut owned = BTreeSet::new();
            for chunk in track_ids.chunks(400) {
                let marks = placeholders(chunk.len());
                let sql = format!(
                    "SELECT rowid FROM portable_domain_records WHERE \
                     (kind='tracks' AND id IN ({marks})) OR \
                     json_extract(json,'$.value.track_id') IN ({marks})"
                );
                let mut statement = conn.prepare(&sql)?;
                for row in statement
                    .query_map(params_from_iter(chunk.iter().chain(chunk.iter())), |row| {
                        row.get::<_, i64>(0)
                    })?
                {
                    owned.insert(row?);
                }
            }
            if !owned.is_empty() {
                plan.order.push("portable_domain_records".into());
                plan.deleted.insert("portable_domain_records".into(), owned);
            }
        }
        for (table, deleted) in &plan.deleted {
            if let Some(detached) = plan.detached.get_mut(table) {
                detached.retain(|rowid| !deleted.contains(rowid));
            }
        }
        plan.detached.retain(|_, rows| !rows.is_empty());
        Ok(plan)
    }

    /// How many rows of each table go (deleted or detached), caches included. This is what a
    /// preview shows; nothing is read beyond the keys.
    pub(crate) fn counts(&self) -> BTreeMap<String, usize> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for (table, rows) in self.deleted.iter().chain(&self.detached) {
            *counts.entry(table.clone()).or_default() += rows.len();
        }
        counts
    }

    /// The derived tables that Undo does not need. A derived table stays in the image
    /// when a row that is kept (deleted for Undo, or detached) refers to it, so a restore
    /// can never put back a row whose parent was not saved.
    fn dropped(&self, tables: &BTreeMap<String, Table>) -> BTreeSet<String> {
        let mut dropped: BTreeSet<String> = self
            .derived
            .iter()
            .filter(|name| self.order.contains(name))
            .cloned()
            .collect();
        loop {
            let needed: Vec<String> = dropped
                .iter()
                .filter(|parent| {
                    self.order.iter().any(|child| {
                        !dropped.contains(child)
                            && tables
                                .get(child)
                                .is_some_and(|t| t.keys.iter().any(|key| &key.parent == *parent))
                    })
                })
                .cloned()
                .collect();
            if needed.is_empty() {
                return dropped;
            }
            for name in needed {
                dropped.remove(&name);
            }
        }
    }

    /// Reads every affected row so it can be put back later.
    pub(crate) fn capture(&self, conn: &Connection) -> Result<RecordImage> {
        let tables = schema(conn)?;
        let dropped = self.dropped(&tables);
        let mut image = RecordImage::default();
        for name in self.order.iter().filter(|n| !dropped.contains(*n)) {
            let table = &tables[name];
            let columns: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
            for (detached, rows) in [
                (false, self.deleted.get(name)),
                (true, self.detached.get(name)),
            ] {
                let Some(rows) = rows.filter(|rows| !rows.is_empty()) else {
                    continue;
                };
                let rowids: Vec<i64> = rows.iter().copied().collect();
                let mut captured = Vec::new();
                for chunk in rowids.chunks(400) {
                    let select = columns
                        .iter()
                        .map(|c| quote(c))
                        .collect::<Vec<_>>()
                        .join(",");
                    let mut statement = conn.prepare(&format!(
                        "SELECT rowid,{select} FROM {} WHERE rowid IN ({}) ORDER BY rowid",
                        quote(name),
                        placeholders(chunk.len())
                    ))?;
                    let mut query = statement.query(params_from_iter(chunk))?;
                    while let Some(row) = query.next()? {
                        let mut values = Vec::with_capacity(columns.len());
                        for (index, column) in columns.iter().enumerate() {
                            values.push(to_json(row.get_ref(index + 1)?, column, name)?);
                        }
                        captured.push((row.get::<_, i64>(0)?, values));
                    }
                }
                let (owned, held): (Vec<String>, Vec<Value>) = if detached {
                    detach_set(name, table).into_iter().unzip()
                } else {
                    (vec![], vec![])
                };
                image.tables.push(TableImage {
                    table: name.clone(),
                    columns: columns.clone(),
                    key: table.primary.clone(),
                    detached,
                    owned,
                    held,
                    rows: captured,
                });
            }
        }
        Ok(image)
    }
}

fn to_json(value: ValueRef<'_>, column: &str, table: &str) -> Result<Value> {
    Ok(match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(v) => Value::from(v),
        ValueRef::Real(v) => {
            ensure!(
                v.is_finite(),
                "{table}.{column} holds a number that cannot be saved"
            );
            Value::from(v)
        }
        ValueRef::Text(bytes) => Value::String(
            std::str::from_utf8(bytes)
                .with_context(|| format!("{table}.{column} is not valid text"))?
                .to_owned(),
        ),
        ValueRef::Blob(bytes) => {
            let mut object = serde_json::Map::new();
            object.insert(
                "$blob".into(),
                Value::String(bytes.iter().map(|b| format!("{b:02x}")).collect()),
            );
            Value::Object(object)
        }
    })
}
/// A saved text or blob value as bytes.
pub(crate) fn bytes_of(value: &Value) -> Option<Vec<u8>> {
    match value {
        Value::String(s) => Some(s.as_bytes().to_vec()),
        Value::Object(map) => {
            let hex = map.get("$blob")?.as_str()?;
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
                .collect()
        }
        _ => None,
    }
}
fn from_json(value: &Value) -> Result<Box<dyn ToSql>> {
    Ok(match value {
        Value::Null => Box::new(rusqlite::types::Null),
        Value::Bool(v) => Box::new(i64::from(*v)),
        Value::Number(n) => match n.as_i64() {
            Some(v) => Box::new(v),
            None => Box::new(
                n.as_f64()
                    .context("Unrepresentable number in saved record")?,
            ),
        },
        Value::String(s) => Box::new(s.clone()),
        Value::Object(map) => {
            let hex = map
                .get("$blob")
                .and_then(Value::as_str)
                .context("Unknown saved value")?;
            ensure!(hex.len().is_multiple_of(2), "Damaged saved blob");
            let mut bytes = Vec::with_capacity(hex.len() / 2);
            for pair in hex.as_bytes().chunks(2) {
                bytes.push(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?);
            }
            Box::new(bytes)
        }
        Value::Array(_) => bail!("Unknown saved value"),
    })
}

impl Plan {
    /// Deletes the captured rows, children first, and clears the optional
    /// references of the rows that stay.
    pub(crate) fn apply(&self, tx: &Transaction<'_>) -> Result<()> {
        tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
        let tables = schema(tx)?;
        for name in self.order.iter().rev() {
            if let Some(rows) = self.detached.get(name) {
                let set = detach_set(name, &tables[name]);
                let assignments = set
                    .iter()
                    .map(|(column, _)| format!("{}=?", quote(column)))
                    .collect::<Vec<_>>()
                    .join(",");
                for chunk in rows.iter().copied().collect::<Vec<_>>().chunks(400) {
                    let mut bound: Vec<Box<dyn ToSql>> = Vec::new();
                    for (_, value) in &set {
                        bound.push(from_json(value)?);
                    }
                    bound.extend(chunk.iter().map(|rowid| Box::new(*rowid) as Box<dyn ToSql>));
                    tx.execute(
                        &format!(
                            "UPDATE {} SET {assignments} WHERE rowid IN ({})",
                            quote(name),
                            placeholders(chunk.len())
                        ),
                        params_from_iter(bound.iter().map(|b| b.as_ref())),
                    )?;
                }
            }
            if let Some(rows) = self.deleted.get(name) {
                for chunk in rows.iter().copied().collect::<Vec<_>>().chunks(400) {
                    tx.execute(
                        &format!(
                            "DELETE FROM {} WHERE rowid IN ({})",
                            quote(name),
                            placeholders(chunk.len())
                        ),
                        params_from_iter(chunk),
                    )?;
                }
            }
        }
        Ok(())
    }
}

/// Rows that other systems keep up to date: the link between a library track and a record
/// in a DJ program or a sync target. Something may have recreated, re-pointed or removed such
/// a row since the track was removed, and that newer state wins: a row is only put back when
/// nothing holds its place, and a link that now points elsewhere is never taken over. (The
/// rekordbox track link is not listed: its row stays in place, so it is detached instead.)
pub(crate) const LINK_TABLES: [&str; 2] = ["sync_links", "native_sync_track_ownership"];

/// What `restore` left alone because it had changed since the removal.
#[derive(Debug, Default)]
pub(crate) struct RestoreReport {
    /// Rows that stay in place (a rekordbox link) which were not reattached, by table.
    pub detached_kept: Vec<(String, usize)>,
    /// Link rows that were not put back because something now holds their place, by table.
    pub links_kept: Vec<(String, usize)>,
}
impl RestoreReport {
    /// Rows left exactly as they are now because something changed them after the removal.
    #[cfg_attr(not(feature = "autotag"), allow(dead_code))]
    pub(crate) fn left_alone(&self) -> usize {
        self.detached_kept
            .iter()
            .chain(&self.links_kept)
            .map(|(_, count)| count)
            .sum()
    }
}

/// Puts a captured set of rows back. Rows are inserted by their declared columns
/// and take fresh rowids. Fails, leaving the caller to roll back, if anything no
/// longer fits: a key now in use, or a parent that is gone. The exception is a link row
/// (`LINK_TABLES`): where something now holds its place it is left alone and counted. A
/// row that stayed in place is touched only if it is still exactly as the removal left
/// it, and then only in the columns the removal changed.
pub(crate) fn restore(tx: &Transaction<'_>, image: &RecordImage) -> Result<RestoreReport> {
    tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    let mut report = RestoreReport::default();
    for table in image.tables.iter().filter(|t| !t.detached) {
        let names: Vec<String> = table.columns.iter().map(|c| quote(c)).collect();
        let link = LINK_TABLES.contains(&table.table.as_str());
        let sql = format!(
            "INSERT {}INTO {}({}) VALUES ({})",
            if link { "OR IGNORE " } else { "" },
            quote(&table.table),
            names.join(","),
            placeholders(names.len())
        );
        let mut statement = tx.prepare(&sql)?;
        let mut kept = 0;
        for (_, values) in &table.rows {
            let mut bound: Vec<Box<dyn ToSql>> = Vec::with_capacity(names.len());
            for value in values {
                bound.push(from_json(value)?);
            }
            let inserted = statement
                .execute(params_from_iter(bound.iter().map(|b| b.as_ref())))
                .with_context(|| format!("Restore a {} record", table.table))?;
            kept += usize::from(inserted == 0);
        }
        if kept > 0 {
            report.links_kept.push((table.table.clone(), kept));
        }
    }
    for table in image.tables.iter().filter(|t| t.detached) {
        ensure!(
            table.owned.len() == table.held.len() && !table.owned.is_empty(),
            "The saved {} rows are damaged",
            table.table
        );
        let owned = |column: &String| table.owned.iter().position(|o| o == column);
        let set = table
            .owned
            .iter()
            .map(|c| format!("{}=?", quote(c)))
            .collect::<Vec<_>>()
            .join(",");
        let expect = table
            .columns
            .iter()
            .map(|c| format!("{} IS ?", quote(c)))
            .collect::<Vec<_>>()
            .join(" AND ");
        let mut statement = tx.prepare(&format!(
            "UPDATE {} SET {set} WHERE {expect}",
            quote(&table.table)
        ))?;
        let mut kept = 0;
        for (_, values) in &table.rows {
            let mut bound: Vec<Box<dyn ToSql>> = Vec::new();
            for column in &table.owned {
                let at = table
                    .columns
                    .iter()
                    .position(|c| c == column)
                    .context("Damaged saved row")?;
                bound.push(from_json(&values[at])?);
            }
            for (at, column) in table.columns.iter().enumerate() {
                bound.push(from_json(
                    owned(column).map_or(&values[at], |i| &table.held[i]),
                )?);
            }
            let changed = statement.execute(params_from_iter(bound.iter().map(|b| b.as_ref())))?;
            kept += usize::from(changed == 0);
        }
        if kept > 0 {
            report.detached_kept.push((table.table.clone(), kept));
        }
    }
    let mut broken = BTreeSet::new();
    for table in &image.tables {
        let mut statement = tx.prepare(&format!(
            "PRAGMA foreign_key_check({})",
            quote(&table.table)
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(format!(
                "{} -> {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(2)?
            ))
        })?;
        for row in rows {
            broken.insert(row?);
        }
    }
    ensure!(
        broken.is_empty(),
        "Something these records depended on is gone ({})",
        broken.into_iter().collect::<Vec<_>>().join(", ")
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: [&str; 0] = [];

    fn library() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE tracks(id TEXT PRIMARY KEY, path BLOB NOT NULL UNIQUE, title TEXT, source_hash TEXT NOT NULL);
             CREATE TABLE jobs(id TEXT PRIMARY KEY, track_id TEXT NOT NULL REFERENCES tracks(id));
             CREATE TABLE cues(id TEXT PRIMARY KEY, track_id TEXT NOT NULL REFERENCES tracks(id), job_id TEXT REFERENCES jobs(id));
             CREATE TABLE checkpoints(job_id TEXT PRIMARY KEY REFERENCES jobs(id), note BLOB);
             CREATE TABLE playlist_members(playlist TEXT NOT NULL, track_id TEXT NOT NULL REFERENCES tracks(id), position INTEGER NOT NULL, PRIMARY KEY(playlist, track_id));
             CREATE TABLE numbered(n INTEGER PRIMARY KEY, track_id TEXT NOT NULL REFERENCES tracks(id));
             INSERT INTO tracks VALUES('keep', X'6B', 'Keep', 'hk'),('gone', X'67', 'Gone', 'hg'),('gone2', X'6732', 'Gone 2', 'hg2');
             INSERT INTO jobs VALUES('j-keep','keep'),('j-gone','gone'),('j-gone2','gone2');
             INSERT INTO cues VALUES('c1','gone','j-gone'),('c2','keep','j-keep'),('c3','gone2','j-gone2');
             INSERT INTO checkpoints VALUES('j-gone', X'00FF'),('j-keep', NULL);
             INSERT INTO playlist_members VALUES('set','gone',3),('set','keep',4);
             INSERT INTO numbered VALUES(7,'gone');",
        )
        .unwrap();
        conn
    }
    fn with_links(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE sync_links(track_id TEXT NOT NULL REFERENCES tracks(id), target TEXT NOT NULL, external_id TEXT NOT NULL, PRIMARY KEY(track_id,target), UNIQUE(target,external_id));
             CREATE TABLE native_targets(id TEXT PRIMARY KEY);
             CREATE TABLE native_track_links(target_id TEXT NOT NULL REFERENCES native_targets(id), content_id TEXT NOT NULL, local_track_id TEXT REFERENCES tracks(id), baseline TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'linked', PRIMARY KEY(target_id,content_id));
             INSERT INTO native_targets VALUES('desk');
             INSERT INTO sync_links VALUES('gone','rekordbox','external-1');
             INSERT INTO native_track_links(target_id,content_id,local_track_id,baseline) VALUES('desk','content-1','gone','old baseline');",
        )
        .unwrap();
    }
    fn has(conn: &Connection, track: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tracks WHERE id=?1)",
            [track],
            |r| r.get(0),
        )
        .unwrap()
    }
    fn show(value: ValueRef<'_>) -> String {
        match value {
            ValueRef::Text(text) => String::from_utf8_lossy(text).into_owned(),
            ValueRef::Blob(bytes) => format!("blob {bytes:02x?}"),
            other => format!("{other:?}"),
        }
    }
    /// Every row of the listed tables by declared columns, in key order: SQLite's own row
    /// numbers are not compared because Undo does not restore them.
    fn dump(conn: &Connection) -> String {
        let mut out = vec![];
        for (table, order) in [
            ("tracks", "id"),
            ("jobs", "id"),
            ("cues", "id"),
            ("checkpoints", "job_id"),
            ("playlist_members", "playlist,track_id"),
            ("numbered", "n"),
        ] {
            let mut stmt = conn
                .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
                .unwrap();
            let count = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..count)
                        .map(|i| Ok(show(r.get_ref(i)?)))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            out.push(format!("{table}: {rows:?}"));
        }
        out.join("\n")
    }
    fn remove(conn: &mut Connection, ids: &[&str], skip: &[&str], derived: &[&str]) -> RecordImage {
        let ids: Vec<String> = ids.iter().map(|id| id.to_string()).collect();
        let tx = conn.transaction().unwrap();
        let plan = Plan::new(&tx, &ids, skip, derived).unwrap();
        let image = plan.capture(&tx).unwrap();
        plan.apply(&tx).unwrap();
        tx.commit().unwrap();
        image
    }
    fn put_back(conn: &mut Connection, image: &RecordImage) -> Result<RestoreReport> {
        let tx = conn.transaction().unwrap();
        let report = restore(&tx, image)?;
        tx.commit().unwrap();
        Ok(report)
    }

    #[test]
    fn the_plan_follows_foreign_keys_through_every_level() {
        let conn = library();
        let ids = vec!["gone".to_owned()];
        let plan = Plan::new(&conn, &ids, &NONE, &NONE).unwrap();
        let counts = plan.counts();
        for table in [
            "tracks",
            "jobs",
            "cues",
            "checkpoints",
            "playlist_members",
            "numbered",
        ] {
            assert_eq!(counts[table], 1, "{table}");
        }
        // The caller rewrites playlist positions itself, so they are not part of the plan.
        let plan = Plan::new(&conn, &ids, &["playlist_members"], &NONE).unwrap();
        assert!(!plan.counts().contains_key("playlist_members"));
        let removed = plan.capture(&conn).unwrap().removed_tracks().unwrap();
        assert_eq!(
            removed,
            vec![RemovedTrack {
                id: "gone".into(),
                path: Some(vec![b'g']),
                hash: Some("hg".into()),
                title: "Gone".into()
            }]
        );
    }

    #[test]
    fn removal_then_restore_returns_every_row_by_its_declared_columns() {
        let mut conn = library();
        conn.execute(
            "DELETE FROM playlist_members WHERE track_id IN ('gone','gone2')",
            [],
        )
        .unwrap();
        let before = dump(&conn);
        let image = remove(&mut conn, &["gone", "gone2"], &["playlist_members"], &NONE);
        let after = dump(&conn);
        assert!(!after.contains("Gone"), "{after}");
        assert!(after.contains("Keep"));
        put_back(&mut conn, &image).unwrap();
        assert_eq!(dump(&conn), before);
    }

    #[test]
    fn a_removal_that_would_orphan_other_records_fails_as_a_whole() {
        let mut conn = library();
        // The playlist entry is not part of the plan, so the track cannot go.
        let ids = vec!["gone".to_owned()];
        let tx = conn.transaction().unwrap();
        let plan = Plan::new(&tx, &ids, &["playlist_members"], &NONE).unwrap();
        plan.apply(&tx).unwrap();
        assert!(tx.commit().is_err());
        assert!(dump(&conn).contains("Gone"));
    }

    #[test]
    fn derived_rows_are_not_saved_unless_a_kept_row_refers_to_them() {
        let mut conn = library();
        conn.execute_batch(
            "CREATE TABLE derived_cache(id TEXT PRIMARY KEY, track_id TEXT NOT NULL REFERENCES tracks(id));
             CREATE TABLE derived_parent(id TEXT PRIMARY KEY, track_id TEXT NOT NULL REFERENCES tracks(id));
             CREATE TABLE keeps_parent(id TEXT PRIMARY KEY, parent TEXT NOT NULL REFERENCES derived_parent(id));
             INSERT INTO derived_cache VALUES('c','gone');
             INSERT INTO derived_parent VALUES('p','gone');",
        )
        .unwrap();
        let ids = vec!["gone".to_owned()];
        let derived = ["derived_cache", "derived_parent"];
        let plan = Plan::new(&conn, &ids, &["playlist_members"], &derived).unwrap();
        // The preview counts them; the saved image does not hold them.
        assert_eq!(plan.counts()["derived_cache"], 1);
        let image = plan.capture(&conn).unwrap();
        assert!(image.table("derived_cache").is_none());
        assert!(image.table("derived_parent").is_none());
        assert!(image.table("tracks").is_some() && image.table("jobs").is_some());
        // A kept row that refers to a derived table keeps that table in the image.
        conn.execute("INSERT INTO keeps_parent VALUES('k','p')", [])
            .unwrap();
        let plan = Plan::new(&conn, &ids, &["playlist_members"], &derived).unwrap();
        let image = plan.capture(&conn).unwrap();
        assert!(
            image.table("derived_parent").is_some(),
            "a kept row refers to it"
        );
        assert!(image.table("derived_cache").is_none());
        // And the removal still goes through and comes back whole.
        conn.execute("DELETE FROM playlist_members WHERE track_id='gone'", [])
            .unwrap();
        let before = dump(&conn);
        let image = remove(&mut conn, &["gone"], &["playlist_members"], &derived);
        assert!(!has(&conn, "gone"));
        put_back(&mut conn, &image).unwrap();
        assert_eq!(dump(&conn), before);
    }

    #[test]
    fn restore_never_depends_on_rows_positions_that_new_records_took() {
        let mut conn = library();
        conn.execute("DELETE FROM playlist_members WHERE track_id='gone'", [])
            .unwrap();
        let image = remove(&mut conn, &["gone"], &["playlist_members"], &NONE);
        // Unrelated inserts are given the freed row positions of the track and of its rows.
        conn.execute_batch(
            "INSERT INTO tracks VALUES('new', X'6E', 'New', 'hn');
             INSERT INTO jobs VALUES('j-new','new');
             INSERT INTO cues VALUES('c-new','new','j-new');
             INSERT INTO checkpoints VALUES('j-new', NULL);",
        )
        .unwrap();
        let reused: i64 = conn
            .query_row("SELECT rowid FROM tracks WHERE id='new'", [], |r| r.get(0))
            .unwrap();
        put_back(&mut conn, &image).unwrap();
        let after = dump(&conn);
        for kept in ["New", "j-new", "c-new"] {
            assert!(after.contains(kept), "{kept} must stay");
        }
        for restored in ["Gone", "j-gone", "c1"] {
            assert!(after.contains(restored), "{restored} must come back");
        }
        assert_eq!(
            conn.query_row("SELECT rowid FROM tracks WHERE id='new'", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            reused,
            "the newer record keeps its position"
        );
        for table in ["tracks", "jobs", "cues"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(count, 4, "{table}");
        }
    }

    #[test]
    fn links_that_changed_since_the_removal_are_left_alone_and_never_taken_over() {
        let mut conn = library();
        with_links(&conn);
        conn.execute("DELETE FROM playlist_members WHERE track_id='gone'", [])
            .unwrap();
        let image = remove(&mut conn, &["gone"], &["playlist_members"], &NONE);
        // The rekordbox link stays as a DJ-software track nobody here owns.
        let (local, status): (Option<String>, String) = conn
            .query_row(
                "SELECT local_track_id,status FROM native_track_links",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((local, status.as_str()), (None, "not_in_local_library"));
        // After the removal the sync linked the same external records to another track.
        conn.execute_batch(
            "INSERT INTO sync_links VALUES('keep','rekordbox','external-1');
             UPDATE native_track_links SET local_track_id='keep', baseline='newer baseline', status='linked';",
        )
        .unwrap();
        let report = put_back(&mut conn, &image).unwrap();
        assert_eq!(report.left_alone(), 2);
        assert_eq!(report.links_kept, vec![("sync_links".to_owned(), 1)]);
        assert_eq!(
            report.detached_kept,
            vec![("native_track_links".to_owned(), 1)]
        );
        let (owner, baseline): (String, String) = conn
            .query_row(
                "SELECT local_track_id,baseline FROM native_track_links",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (owner.as_str(), baseline.as_str()),
            ("keep", "newer baseline")
        );
        let links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sync_links WHERE track_id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(links, 0);
        assert!(dump(&conn).contains("Gone"), "everything else came back");
    }

    #[test]
    fn links_nobody_has_touched_are_put_back_as_the_removal_left_them() {
        let mut conn = library();
        with_links(&conn);
        conn.execute("DELETE FROM playlist_members WHERE track_id='gone'", [])
            .unwrap();
        let image = remove(&mut conn, &["gone"], &["playlist_members"], &NONE);
        let report = put_back(&mut conn, &image).unwrap();
        assert_eq!(report.left_alone(), 0);
        let (owner, baseline, status): (String, String, String) = conn
            .query_row(
                "SELECT local_track_id,baseline,status FROM native_track_links",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            (owner.as_str(), baseline.as_str(), status.as_str()),
            ("gone", "old baseline", "linked")
        );
        let links: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sync_links WHERE track_id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(links, 1);
    }

    #[test]
    fn restore_refuses_when_a_record_with_the_same_identity_exists() {
        let mut conn = library();
        let ids = vec!["gone".to_owned()];
        let image = Plan::new(&conn, &ids, &NONE, &NONE)
            .unwrap()
            .capture(&conn)
            .unwrap();
        // Not removed first: the identity is occupied.
        assert!(put_back(&mut conn, &image).is_err());
    }

    #[test]
    fn restore_refuses_a_record_whose_parent_has_disappeared() {
        let mut conn = library();
        conn.execute("DELETE FROM playlist_members WHERE track_id='gone'", [])
            .unwrap();
        let mut image = remove(&mut conn, &["gone"], &["playlist_members"], &NONE);
        // A parent the image depends on outside itself vanishes.
        image.tables.push(TableImage {
            table: "jobs".into(),
            columns: vec!["id".into(), "track_id".into()],
            key: vec!["id".into()],
            detached: false,
            owned: vec![],
            held: vec![],
            rows: vec![(99, vec!["orphan".into(), "missing".into()])],
        });
        assert!(put_back(&mut conn, &image).is_err());
        assert!(
            !has(&conn, "gone"),
            "nothing is half restored once the caller rolls back"
        );
    }
}
