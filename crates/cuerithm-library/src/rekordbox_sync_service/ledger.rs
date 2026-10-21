use super::*;
use rusqlite::{
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
};
const TABLES: &[(&str, &str)] = &[
    ("native_targets", "id"),
    ("native_records", "target_id"),
    ("native_track_links", "target_id"),
    ("native_sync_track_ownership", "target_id"),
    ("native_reconciliation_plans", "target_id"),
    ("native_playlist_bindings", "target_id"),
    ("native_playlist_acknowledgements", "target_id"),
];
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableRows {
    name: String,
    columns: Vec<String>,
    rows: Vec<Vec<TargetValue>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct NativeLedger {
    target: String,
    tables: Vec<TableRows>,
}
pub(super) fn capture(connection: &Connection, target: &str) -> Result<NativeLedger> {
    let mut tables = vec![];
    for (name, key) in TABLES {
        let mut stmt = connection.prepare(&format!(
            "SELECT * FROM {name} WHERE {key}=?1 ORDER BY rowid"
        ))?;
        let columns = stmt
            .column_names()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let count = columns.len();
        let rows = stmt
            .query_map([target], |r| {
                (0..count)
                    .map(|i| {
                        Ok(match r.get_ref(i)? {
                            ValueRef::Null => TargetValue::Null,
                            ValueRef::Integer(v) => TargetValue::Integer(v),
                            ValueRef::Real(v) => TargetValue::Real(v),
                            ValueRef::Text(v) => TargetValue::Text(
                                std::str::from_utf8(v)
                                    .map_err(|e| {
                                        rusqlite::Error::FromSqlConversionFailure(
                                            i,
                                            rusqlite::types::Type::Text,
                                            Box::new(e),
                                        )
                                    })?
                                    .to_owned(),
                            ),
                            ValueRef::Blob(v) => TargetValue::Blob(v.to_vec()),
                        })
                    })
                    .collect::<rusqlite::Result<Vec<_>>>()
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        tables.push(TableRows {
            name: (*name).into(),
            columns,
            rows,
        });
    }
    Ok(NativeLedger {
        target: target.into(),
        tables,
    })
}
pub(super) fn restore(connection: &Connection, ledger: &NativeLedger) -> Result<()> {
    for (name, key) in TABLES.iter().rev() {
        connection.execute(
            &format!("DELETE FROM {name} WHERE {key}=?1"),
            [&ledger.target],
        )?;
    }
    for table in &ledger.tables {
        ensure!(
            TABLES.iter().any(|(name, _)| *name == table.name),
            "Unsupported native ledger table"
        );
        let columns = table
            .columns
            .iter()
            .map(|c| {
                ensure!(
                    c.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                    "Invalid ledger column"
                );
                Ok(format!("\"{c}\""))
            })
            .collect::<Result<Vec<_>>>()?
            .join(",");
        let values = (0..table.columns.len())
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(",");
        for row in &table.rows {
            let row = row
                .iter()
                .map(|v| match v {
                    TargetValue::Null => SqlValue::Null,
                    TargetValue::Integer(v) => SqlValue::Integer(*v),
                    TargetValue::Real(v) => SqlValue::Real(*v),
                    TargetValue::Text(v) => SqlValue::Text(v.clone()),
                    TargetValue::Blob(v) => SqlValue::Blob(v.clone()),
                })
                .collect::<Vec<_>>();
            connection.execute(
                &format!("INSERT INTO {} ({columns}) VALUES ({values})", table.name),
                params_from_iter(row),
            )?;
        }
    }
    Ok(())
}
