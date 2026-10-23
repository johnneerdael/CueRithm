//! Reversible removal of library records.
//!
//! A snapshot holds every database row that belongs to a track, found by following the
//! schema's own foreign keys, so records added by later migrations are included without
//! changing this module. Only rows are involved: audio files and sidecars are never read,
//! moved or deleted here.
// Applying and undoing a removal belong to the coordinated merge service, which needs the
// catalog-tagging feature; the preview only reads.
#![cfg_attr(not(feature = "autotag"), allow(dead_code))]
use anyhow::{Context, Result, ensure};
use rusqlite::{
    Connection, OptionalExtension, params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) enum Cell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}
impl Cell {
    pub(super) fn capture(value: ValueRef<'_>) -> Result<Self> {
        Ok(match value {
            ValueRef::Null => Self::Null,
            ValueRef::Integer(n) => Self::Integer(n),
            ValueRef::Real(n) => {
                ensure!(n.is_finite(), "Nonfinite library value");
                Self::Real(n)
            }
            ValueRef::Text(t) => Self::Text(std::str::from_utf8(t)?.into()),
            ValueRef::Blob(b) => Self::Blob(b.into()),
        })
    }
    pub(super) fn sql(&self) -> SqlValue {
        match self {
            Self::Null => SqlValue::Null,
            Self::Integer(n) => SqlValue::Integer(*n),
            Self::Real(n) => SqlValue::Real(*n),
            Self::Text(s) => SqlValue::Text(s.clone()),
            Self::Blob(b) => SqlValue::Blob(b.clone()),
        }
    }
    pub(super) fn json(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Integer(n) => json!(n),
            Self::Real(n) => json!(n),
            Self::Text(s) => json!(s),
            Self::Blob(b) => json!(b),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct StoredRow {
    rowid: i64,
    values: Vec<Cell>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TableRows {
    table: String,
    columns: Vec<String>,
    rows: Vec<StoredRow>,
}
/// Everything needed to put removed records back exactly as they were, including their
/// original row order. Parents are stored before the rows that refer to them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct RecordSnapshot {
    tables: Vec<TableRows>,
}
impl RecordSnapshot {
    /// Rows captured per table, for showing what a removal takes with it.
    pub(super) fn counts(&self) -> BTreeMap<&str, usize> {
        self.tables
            .iter()
            .map(|t| (t.table.as_str(), t.rows.len()))
            .collect()
    }
    /// Identity and file location of each removed track record.
    pub(super) fn tracks(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let Some(set) = self.tables.iter().find(|t| t.table == "tracks") else {
            return Ok(vec![]);
        };
        let column = |name: &str| {
            set.columns
                .iter()
                .position(|c| c == name)
                .with_context(|| format!("Track snapshot lacks {name}"))
        };
        let (id, path) = (column("id")?, column("path")?);
        set.rows
            .iter()
            .map(|row| match (&row.values[id], &row.values[path]) {
                (Cell::Text(id), Cell::Blob(path)) => Ok((id.clone(), path.clone())),
                _ => anyhow::bail!("Invalid track snapshot"),
            })
            .collect()
    }
}

fn ident(name: &str) -> Result<&str> {
    ensure!(
        !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "Unsupported table or column name"
    );
    Ok(name)
}
fn quote(name: &str) -> Result<String> {
    Ok(format!("\"{}\"", ident(name)?))
}
fn column_names(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let stmt = conn.prepare(&format!("SELECT * FROM {} LIMIT 0", quote(table)?))?;
    Ok(stmt.column_names().into_iter().map(str::to_owned).collect())
}
fn primary_key(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1) WHERE pk>0 ORDER BY pk")?;
    let names = stmt
        .query_map([table], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    ensure!(
        !names.is_empty(),
        "Foreign key refers to a table without a key"
    );
    Ok(names)
}
/// True when the table's only key column is an alias of the row id, which must then not be
/// inserted twice.
fn has_rowid_alias(conn: &Connection, table: &str) -> Result<bool> {
    let mut stmt = conn.prepare("SELECT type FROM pragma_table_info(?1) WHERE pk>0")?;
    let keys = stmt
        .query_map([table], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(keys.len() == 1 && keys[0].eq_ignore_ascii_case("INTEGER"))
}

fn row_id_taken(conn: &Connection, table: &str, rowid: i64) -> Result<bool> {
    Ok(conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE rowid=?1)",
            quote(table)?
        ),
        [rowid],
        |r| r.get(0),
    )?)
}

struct Edge {
    child: String,
    from: Vec<String>,
    to: Vec<String>,
}
/// Foreign keys of the live schema, grouped by the table they point at.
fn child_edges(conn: &Connection) -> Result<BTreeMap<String, Vec<Edge>>> {
    let tables = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut edges: BTreeMap<String, Vec<Edge>> = BTreeMap::new();
    for child in tables {
        ident(&child)?;
        let mut stmt = conn.prepare(
            "SELECT id,\"table\",\"from\",\"to\" FROM pragma_foreign_key_list(?1) ORDER BY id,seq",
        )?;
        let rows = stmt
            .query_map([&child], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        #[allow(clippy::type_complexity)]
        let mut grouped: BTreeMap<i64, (String, Vec<String>, Vec<Option<String>>)> =
            BTreeMap::new();
        for (id, parent, from, to) in rows {
            let entry = grouped
                .entry(id)
                .or_insert_with(|| (parent, vec![], vec![]));
            entry.1.push(from);
            entry.2.push(to);
        }
        for (parent, from, to) in grouped.into_values() {
            let to = if to.iter().all(Option::is_some) {
                to.into_iter().flatten().collect()
            } else {
                primary_key(conn, &parent)?
            };
            ensure!(from.len() == to.len(), "Unsupported foreign key");
            edges.entry(parent).or_default().push(Edge {
                child: child.clone(),
                from,
                to,
            });
        }
    }
    Ok(edges)
}

fn read_row(conn: &Connection, table: &str, rowid: i64) -> Result<(Vec<String>, Vec<Cell>)> {
    let mut stmt =
        conn.prepare_cached(&format!("SELECT * FROM {} WHERE rowid=?1", quote(table)?))?;
    let columns: Vec<String> = stmt.column_names().into_iter().map(str::to_owned).collect();
    let mut rows = stmt.query([rowid])?;
    let row = rows
        .next()?
        .context("Record changed while it was captured")?;
    let values = (0..columns.len())
        .map(|i| Cell::capture(row.get_ref(i)?))
        .collect::<Result<Vec<_>>>()?;
    Ok((columns, values))
}

/// Capture the given tracks and every record that depends on them.
pub(super) fn capture(conn: &Connection, track_ids: &[String]) -> Result<RecordSnapshot> {
    let edges = child_edges(conn)?;
    let mut snapshot = RecordSnapshot::default();
    let mut slots: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: BTreeSet<(String, i64)> = BTreeSet::new();
    let mut queue: VecDeque<(String, i64)> = VecDeque::new();
    for id in track_ids {
        let rowid: i64 = conn
            .query_row("SELECT rowid FROM tracks WHERE id=?1", [id], |r| r.get(0))
            .optional()?
            .context("Track no longer exists")?;
        if seen.insert(("tracks".into(), rowid)) {
            queue.push_back(("tracks".into(), rowid));
        }
    }
    while let Some((table, rowid)) = queue.pop_front() {
        let (columns, values) = read_row(conn, &table, rowid)?;
        let slot = *slots.entry(table.clone()).or_insert_with(|| {
            snapshot.tables.push(TableRows {
                table: table.clone(),
                columns: columns.clone(),
                rows: vec![],
            });
            snapshot.tables.len() - 1
        });
        for edge in edges.get(&table).into_iter().flatten() {
            let keys = edge
                .to
                .iter()
                .map(|name| {
                    columns
                        .iter()
                        .position(|c| c == name)
                        .map(|i| values[i].sql())
                        .context("Unsupported foreign key target")
                })
                .collect::<Result<Vec<_>>>()?;
            if keys.iter().any(|v| matches!(v, SqlValue::Null)) {
                continue;
            }
            let predicate = edge
                .from
                .iter()
                .enumerate()
                .map(|(i, name)| Ok(format!("{}=?{}", quote(name)?, i + 1)))
                .collect::<Result<Vec<_>>>()?
                .join(" AND ");
            let mut stmt = conn.prepare_cached(&format!(
                "SELECT rowid FROM {} WHERE {predicate}",
                quote(&edge.child)?
            ))?;
            let children = stmt
                .query_map(params_from_iter(keys), |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for child in children {
                if seen.insert((edge.child.clone(), child)) {
                    queue.push_back((edge.child.clone(), child));
                }
            }
        }
        snapshot.tables[slot].rows.push(StoredRow { rowid, values });
    }
    Ok(snapshot)
}

/// Delete exactly the captured rows. Foreign keys are checked when the transaction commits,
/// so a removal that would orphan other records fails as a whole and changes nothing.
pub(super) fn delete(conn: &Connection, snapshot: &RecordSnapshot) -> Result<()> {
    conn.pragma_update(None, "defer_foreign_keys", true)?;
    for set in snapshot.tables.iter().rev() {
        let sql = format!("DELETE FROM {} WHERE rowid=?1", quote(&set.table)?);
        for row in set.rows.iter().rev() {
            conn.execute(&sql, [row.rowid])?;
        }
    }
    Ok(())
}

/// Put the captured rows back, with their original row ids.
pub(super) fn restore(conn: &Connection, snapshot: &RecordSnapshot) -> Result<()> {
    conn.pragma_update(None, "defer_foreign_keys", true)?;
    for set in &snapshot.tables {
        ensure!(
            column_names(conn, &set.table)? == set.columns,
            "Unsupported record snapshot schema"
        );
        let alias = has_rowid_alias(conn, &set.table)?;
        for row in &set.rows {
            let mut names = vec![];
            let mut values = vec![];
            // Keep the original row position when it is free. SQLite reuses the ids of
            // deleted rows, so a record added since then may hold it; the restored record
            // then takes the next free position. Nothing refers to a row id (records refer
            // to each other by their own keys), so only the display order can differ.
            if !alias && !row_id_taken(conn, &set.table, row.rowid)? {
                names.push("rowid".to_owned());
                values.push(SqlValue::Integer(row.rowid));
            }
            for (name, cell) in set.columns.iter().zip(&row.values) {
                names.push(quote(name)?);
                values.push(cell.sql());
            }
            let marks = (1..=values.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(",");
            conn.execute(
                &format!(
                    "INSERT INTO {}({}) VALUES({marks})",
                    quote(&set.table)?,
                    names.join(",")
                ),
                params_from_iter(values),
            )
            .with_context(|| format!("Cannot restore a {} record", set.table))?;
        }
    }
    for set in &snapshot.tables {
        let restored = set.rows.iter().map(|r| r.rowid).collect::<BTreeSet<_>>();
        let mut stmt =
            conn.prepare(&format!("PRAGMA foreign_key_check({})", ident(&set.table)?))?;
        let broken = stmt
            .query_map([], |r| r.get::<_, Option<i64>>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ensure!(
            !broken
                .iter()
                .flatten()
                .any(|rowid| restored.contains(rowid)),
            "A restored record refers to something that no longer exists"
        );
    }
    Ok(())
}

/// Why a track record must not be removed right now, if it must not.
pub(super) fn busy_reason(conn: &Connection, track_id: &str) -> Result<Option<&'static str>> {
    let state: Option<String> = conn
        .query_row("SELECT state FROM tracks WHERE id=?1", [track_id], |r| {
            r.get(0)
        })
        .optional()?;
    let Some(state) = state else {
        return Ok(Some("The track is no longer in the library."));
    };
    if state != "ready" {
        return Ok(Some("The track is still being prepared."));
    }
    let unfinished: i64 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM jobs WHERE track_id=?1 AND state IN ('queued','running','attention'))+(SELECT COUNT(*) FROM tag_writes WHERE track_id=?1 AND state='prepared')+(SELECT COUNT(*) FROM cleanup_writes WHERE track_id=?1 AND state='prepared')",
        [track_id],
        |r| r.get(0),
    )?;
    Ok((unfinished > 0).then_some("The track has unfinished work."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE tracks(id TEXT PRIMARY KEY, path BLOB NOT NULL UNIQUE, title TEXT);
             CREATE TABLE jobs(id TEXT PRIMARY KEY, track_id TEXT NOT NULL REFERENCES tracks(id));
             CREATE TABLE cues(id TEXT PRIMARY KEY, track_id TEXT NOT NULL REFERENCES tracks(id), job_id TEXT REFERENCES jobs(id));
             CREATE TABLE checkpoints(job_id TEXT PRIMARY KEY REFERENCES jobs(id), note BLOB);
             CREATE TABLE playlist_members(playlist TEXT NOT NULL, track_id TEXT NOT NULL REFERENCES tracks(id), position INTEGER NOT NULL, PRIMARY KEY(playlist, track_id));
             CREATE TABLE numbered(n INTEGER PRIMARY KEY, track_id TEXT NOT NULL REFERENCES tracks(id));
             INSERT INTO tracks VALUES('keep', X'6B', 'Keep'),('gone', X'67', 'Gone'),('gone2', X'6732', 'Gone 2');
             INSERT INTO jobs VALUES('j-keep','keep'),('j-gone','gone'),('j-gone2','gone2');
             INSERT INTO cues VALUES('c1','gone','j-gone'),('c2','keep','j-keep'),('c3','gone2','j-gone2');
             INSERT INTO checkpoints VALUES('j-gone', X'00FF'),('j-keep', NULL);
             INSERT INTO playlist_members VALUES('set','gone',3),('set','keep',4);
             INSERT INTO numbered VALUES(7,'gone');",
        )
        .unwrap();
        conn
    }
    fn dump(conn: &Connection) -> String {
        let mut out = vec![];
        for (table, order) in [
            ("tracks", "rowid"),
            ("jobs", "rowid"),
            ("cues", "rowid"),
            ("checkpoints", "rowid"),
            ("playlist_members", "rowid"),
            ("numbered", "rowid"),
        ] {
            let mut stmt = conn
                .prepare(&format!("SELECT rowid,* FROM {table} ORDER BY {order}"))
                .unwrap();
            let count = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..count)
                        .map(|i| Ok(Cell::capture(r.get_ref(i)?).unwrap().json().to_string()))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            out.push(format!("{table}: {rows:?}"));
        }
        out.join("\n")
    }

    #[test]
    fn capture_follows_foreign_keys_through_every_level() {
        let conn = library();
        let snapshot = capture(&conn, &["gone".into()]).unwrap();
        let counts = snapshot.counts();
        assert_eq!(counts["tracks"], 1);
        assert_eq!(counts["jobs"], 1);
        assert_eq!(counts["cues"], 1);
        assert_eq!(counts["checkpoints"], 1);
        assert_eq!(counts["playlist_members"], 1);
        assert_eq!(counts["numbered"], 1);
        assert_eq!(
            snapshot.tracks().unwrap(),
            vec![("gone".into(), vec![b'g'])]
        );
    }

    #[test]
    fn delete_then_restore_returns_every_row_with_its_original_row_id() {
        let mut conn = library();
        let before = dump(&conn);
        let snapshot = capture(&conn, &["gone".into(), "gone2".into()]).unwrap();
        let tx = conn.transaction().unwrap();
        delete(&tx, &snapshot).unwrap();
        tx.commit().unwrap();
        let after = dump(&conn);
        assert!(!after.contains("Gone"), "{after}");
        assert!(after.contains("Keep"));
        let tx = conn.transaction().unwrap();
        restore(&tx, &snapshot).unwrap();
        tx.commit().unwrap();
        assert_eq!(dump(&conn), before);
    }

    #[test]
    fn removing_a_parent_without_its_children_is_refused_at_commit() {
        let mut conn = library();
        let tx = conn.transaction().unwrap();
        tx.pragma_update(None, "defer_foreign_keys", true).unwrap();
        tx.execute("DELETE FROM tracks WHERE id='gone'", [])
            .unwrap();
        assert!(tx.commit().is_err());
        assert!(dump(&conn).contains("Gone"));
    }

    #[test]
    fn restore_takes_the_next_free_row_position_when_a_new_record_reused_the_old_one() {
        let mut conn = library();
        let snapshot = capture(&conn, &["gone2".into()]).unwrap();
        let tx = conn.transaction().unwrap();
        delete(&tx, &snapshot).unwrap();
        tx.commit().unwrap();
        // A record added since reuses the freed row id.
        conn.execute("INSERT INTO tracks VALUES('new', X'6E', 'New')", [])
            .unwrap();
        let reused: i64 = conn
            .query_row("SELECT rowid FROM tracks WHERE id='new'", [], |r| r.get(0))
            .unwrap();
        let tx = conn.transaction().unwrap();
        restore(&tx, &snapshot).unwrap();
        tx.commit().unwrap();
        let restored: i64 = conn
            .query_row("SELECT rowid FROM tracks WHERE id='gone2'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_ne!(restored, reused);
        assert_eq!(
            conn.query_row("SELECT title FROM tracks WHERE id='gone2'", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "Gone 2"
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM cues WHERE track_id='gone2'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn restore_refuses_when_a_record_with_the_same_identity_exists() {
        let mut conn = library();
        let snapshot = capture(&conn, &["gone".into()]).unwrap();
        let tx = conn.transaction().unwrap();
        // Not removed first: the identity is occupied.
        assert!(restore(&tx, &snapshot).is_err());
    }

    #[test]
    fn restore_refuses_a_record_whose_parent_has_disappeared() {
        let mut conn = library();
        let snapshot = capture(&conn, &["gone".into()]).unwrap();
        let tx = conn.transaction().unwrap();
        delete(&tx, &snapshot).unwrap();
        tx.commit().unwrap();
        conn.execute("DELETE FROM playlist_members", []).unwrap();
        conn.execute("DELETE FROM cues WHERE track_id='keep'", [])
            .unwrap();
        // A parent the snapshot depends on outside itself vanishes.
        let mut other = snapshot.clone();
        other.tables.push(TableRows {
            table: "jobs".into(),
            columns: vec!["id".into(), "track_id".into()],
            rows: vec![StoredRow {
                rowid: 99,
                values: vec![Cell::Text("orphan".into()), Cell::Text("missing".into())],
            }],
        });
        let tx = conn.transaction().unwrap();
        assert!(restore(&tx, &other).is_err());
    }
}
