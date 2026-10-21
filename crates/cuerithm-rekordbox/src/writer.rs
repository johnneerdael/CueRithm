//! Guarded row staging on owned encrypted snapshots. This module deliberately
//! does not establish an accepted native application protocol profile.
use crate::*;
use rusqlite::{OptionalExtension, params_from_iter, types::Value as SqlValue};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeRowMutation {
    pub table: String,
    pub id: String,
    /// None means an insert into a currently absent identity.
    pub expected_fingerprint: Option<String>,
    /// Only explicit changes are written; unknown existing columns are retained.
    pub values: BTreeMap<String, TargetValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeWritePlan {
    pub source: PathBuf,
    pub schema_fingerprint: String,
    pub rows: Vec<NativeRowMutation>,
}

pub struct StagedNativeWrite {
    pub snapshot: DatabaseSnapshot,
    pub changed_rows: usize,
    pub write_capability: WriteCapability,
}

fn identifier(value: &str) -> Result<String> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
        "Unsupported native column identifier"
    );
    Ok(format!("\"{value}\""))
}

fn table_key(table: &str, id: &str) -> Result<&'static str> {
    match table {
        "agentRegistry" => {
            ensure!(
                id == "localUpdateCount",
                "Unsupported native registry mutation"
            );
            Ok("registry_id")
        }
        "djmdContent" | "djmdArtist" | "djmdAlbum" | "djmdGenre" | "djmdLabel" | "djmdKey"
        | "djmdCue" | "contentCue" | "djmdPlaylist" | "djmdSongPlaylist" | "djmdMyTag"
        | "djmdSongMyTag" | "contentFile" => Ok("ID"),
        _ => anyhow::bail!("Unsupported native staging table"),
    }
}

fn sql_value(value: &TargetValue) -> Result<SqlValue> {
    Ok(match value {
        TargetValue::Null => SqlValue::Null,
        TargetValue::Integer(v) => SqlValue::Integer(*v),
        TargetValue::Real(v) => {
            ensure!(v.is_finite(), "Nonfinite native staging value");
            SqlValue::Real(*v)
        }
        TargetValue::Text(v) => {
            ensure!(v.len() <= 1_048_576, "Native text exceeds staging budget");
            SqlValue::Text(v.clone())
        }
        TargetValue::Blob(v) => {
            ensure!(
                v.len() <= 16_777_216,
                "Native binary value exceeds staging budget"
            );
            SqlValue::Blob(v.clone())
        }
    })
}

struct ObservedRow {
    columns: BTreeSet<String>,
    values: Option<BTreeMap<String, TargetValue>>,
}

fn current_row(conn: &Connection, table: &str, key: &str, id: &str) -> Result<ObservedRow> {
    let mut query = conn.prepare(&format!(
        "SELECT * FROM {} WHERE {}=?1",
        identifier(table)?,
        identifier(key)?
    ))?;
    let columns: Vec<_> = query
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let values = query
        .query_row([id], |row| {
            columns
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let value = match row.get::<_, SqlValue>(i)? {
                        SqlValue::Null => TargetValue::Null,
                        SqlValue::Integer(v) => TargetValue::Integer(v),
                        SqlValue::Real(v) => TargetValue::Real(v),
                        SqlValue::Text(v) => TargetValue::Text(v),
                        SqlValue::Blob(v) => TargetValue::Blob(v),
                    };
                    Ok((name.clone(), value))
                })
                .collect::<rusqlite::Result<BTreeMap<_, _>>>()
        })
        .optional()?;
    Ok(ObservedRow {
        columns: columns.into_iter().collect(),
        values,
    })
}

/// Stage DML on a fresh coherent encrypted copy. Schema recognition and authored
/// DML tests do not prove rekordbox's ID/USN/asset protocol or permit live writes.
pub fn stage_native_write(
    plan: &NativeWritePlan,
    workspace: &Path,
    key: &DatabaseKey,
    cancel: &CancellationToken,
) -> Result<StagedNativeWrite> {
    cancel.check()?;
    validate_native_rows(plan)?;
    let source = ReadOnlyDatabase::open(&plan.source, key)?;
    let mut snapshot = source.capture(workspace, key, cancel)?;
    ensure!(
        snapshot.schema.fingerprint == plan.schema_fingerprint,
        "Native staging schema changed"
    );
    ensure!(
        snapshot.checks.foreign_key_issues == 0,
        "Native source has foreign-key issues"
    );
    let write_capability = validate_write_profile(&snapshot.schema)?;
    drop(source);
    let staged_path = snapshot
        .path
        .canonicalize()
        .context("Resolve owned native staging path")?;
    let mut conn = Connection::open_with_flags(
        &staged_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )?;
    set_key(&conn, key)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    let tx = conn.transaction()?;
    let changed_rows = apply_native_rows(&tx, plan, cancel)?;
    cancel.check()?;
    tx.commit()?;
    drop(conn);
    let staged = ReadOnlyDatabase::open(&snapshot.path, key)?;
    snapshot.checks = staged.checks()?;
    ensure!(
        snapshot.checks.integrity_ok && snapshot.checks.foreign_key_issues == 0,
        "Staged native database failed integrity checks"
    );
    snapshot.schema = staged.schema()?;
    ensure!(
        snapshot.schema.fingerprint == plan.schema_fingerprint,
        "Native staging changed schema"
    );
    drop(staged);
    cancel.check()?;
    snapshot.sha256 = file_sha256_cancellable(&snapshot.path, cancel)?;
    std::fs::File::options()
        .read(true)
        .write(true)
        .open(&snapshot.path)?
        .sync_all()?;
    Ok(StagedNativeWrite {
        snapshot,
        changed_rows,
        write_capability,
    })
}

fn validate_native_rows(plan: &NativeWritePlan) -> Result<()> {
    ensure!(
        plan.rows.len() <= 100_000,
        "Native row staging budget exceeded"
    );
    let mut seen = BTreeSet::new();
    for row in &plan.rows {
        table_key(&row.table, &row.id)?;
        ensure!(
            !row.id.is_empty() && row.id.len() <= 4096,
            "Invalid native row identity"
        );
        ensure!(
            seen.insert((&row.table, &row.id)),
            "Duplicate native row mutation"
        );
        ensure!(
            !row.values.is_empty() && row.values.len() <= 512,
            "Invalid native field count"
        );
        if row.table == "agentRegistry" {
            ensure!(
                row.expected_fingerprint.is_some() && row.values.keys().all(|name| name == "int_1"),
                "Unsupported registry staging fields"
            );
        }
        for (name, value) in &row.values {
            identifier(name)?;
            sql_value(value)?;
        }
    }
    Ok(())
}

/// Apply guarded DML within the caller's transaction; never commits by itself.
pub(crate) fn apply_native_rows(
    conn: &Connection,
    plan: &NativeWritePlan,
    cancel: &CancellationToken,
) -> Result<usize> {
    cancel.check()?;
    validate_native_rows(plan)?;
    let mut changed_rows = 0;
    for row in &plan.rows {
        cancel.check()?;
        let key_column = table_key(&row.table, &row.id)?;
        let ObservedRow {
            columns,
            values: before,
        } = current_row(conn, &row.table, key_column, &row.id)?;
        let observed = before
            .as_ref()
            .map(|values| -> Result<String> {
                Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(values)?)))
            })
            .transpose()?;
        ensure!(
            observed == row.expected_fingerprint,
            "Native staging row changed"
        );
        ensure!(
            row.values.keys().all(|name| columns.contains(name)),
            "Unknown native staging column"
        );
        if let Some(value) = row.values.get(key_column) {
            ensure!(
                matches!(value, TargetValue::Text(v) if v == &row.id)
                    || matches!(value, TargetValue::Integer(v) if v.to_string() == row.id),
                "Native row identity cannot be changed"
            );
        }
        if before.as_ref().is_some_and(|before| {
            row.values
                .iter()
                .all(|(name, value)| before.get(name) == Some(value))
        }) {
            continue;
        }
        let mut values: Vec<_> = row.values.values().map(sql_value).collect::<Result<_>>()?;
        if before.is_some() {
            let sets = row
                .values
                .keys()
                .enumerate()
                .map(|(i, name)| Ok(format!("{}=?{}", identifier(name)?, i + 1)))
                .collect::<Result<Vec<_>>>()?
                .join(",");
            values.push(SqlValue::Text(row.id.clone()));
            let affected = conn.execute(
                &format!(
                    "UPDATE {} SET {sets} WHERE {}=?{}",
                    identifier(&row.table)?,
                    identifier(key_column)?,
                    values.len()
                ),
                params_from_iter(values),
            )?;
            ensure!(affected == 1, "Native staging identity is not unique");
        } else {
            let mut fields: Vec<_> = row
                .values
                .keys()
                .map(|name| identifier(name))
                .collect::<Result<_>>()?;
            if !row.values.contains_key(key_column) {
                fields.push(identifier(key_column)?);
                values.push(SqlValue::Text(row.id.clone()));
            }
            let placeholders = (1..=fields.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(",");
            conn.execute(
                &format!(
                    "INSERT INTO {} ({}) VALUES({placeholders})",
                    identifier(&row.table)?,
                    fields.join(",")
                ),
                params_from_iter(values),
            )?;
        }
        changed_rows += 1;
    }
    cancel.check()?;
    Ok(changed_rows)
}
