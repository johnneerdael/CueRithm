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
impl RecordImage {
    pub(crate) fn table(&self, name: &str) -> Option<&TableImage> {
        self.tables.iter().find(|t| t.table == name)
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
        for (table, deleted) in &plan.deleted {
            if let Some(detached) = plan.detached.get_mut(table) {
                detached.retain(|rowid| !deleted.contains(rowid));
            }
        }
        plan.detached.retain(|_, rows| !rows.is_empty());
        Ok(plan)
    }

    /// Reads every affected row so it can be put back later.
    pub(crate) fn capture(&self, conn: &Connection) -> Result<RecordImage> {
        let tables = schema(conn)?;
        let mut image = RecordImage::default();
        for name in self.order.iter().filter(|n| !self.derived.contains(n)) {
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

/// What `restore` left alone because it had changed since the removal.
#[derive(Debug, Default)]
pub(crate) struct RestoreReport {
    /// Rows that stay in place (a rekordbox link) which were not reattached, by table.
    pub detached_kept: Vec<(String, usize)>,
}

/// Puts a captured set of rows back. Rows are inserted by their declared columns
/// and take fresh rowids. Fails, leaving the caller to roll back, if anything no
/// longer fits: a key now in use, or a parent that is gone. A row that stayed in
/// place is touched only if it is still exactly as the removal left it, and then
/// only in the columns the removal changed.
pub(crate) fn restore(tx: &Transaction<'_>, image: &RecordImage) -> Result<RestoreReport> {
    tx.execute_batch("PRAGMA defer_foreign_keys = ON")?;
    let mut report = RestoreReport::default();
    for table in image.tables.iter().filter(|t| !t.detached) {
        let names: Vec<String> = table.columns.iter().map(|c| quote(c)).collect();
        let sql = format!(
            "INSERT INTO {}({}) VALUES ({})",
            quote(&table.table),
            names.join(","),
            placeholders(names.len())
        );
        let mut statement = tx.prepare(&sql)?;
        for (_, values) in &table.rows {
            let mut bound: Vec<Box<dyn ToSql>> = Vec::with_capacity(names.len());
            for value in values {
                bound.push(from_json(value)?);
            }
            statement
                .execute(params_from_iter(bound.iter().map(|b| b.as_ref())))
                .with_context(|| format!("Restore a {} record", table.table))?;
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
