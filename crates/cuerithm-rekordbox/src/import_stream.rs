//! Bounded music-record cursor for the shared importer. No connection/key escapes.
use crate::*;
use rusqlite::types::ValueRef;
use std::collections::BTreeMap;
pub const COLLECTION_IMPORT_TABLES: &[&str] = &[
    "djmdProperty",
    "agentRegistry",
    "djmdArtist",
    "djmdAlbum",
    "djmdGenre",
    "djmdLabel",
    "djmdKey",
    "djmdContent",
    "djmdCue",
    "contentCue",
    "djmdPlaylist",
    "djmdSongPlaylist",
    "djmdMyTag",
    "djmdSongMyTag",
];
#[derive(Debug, Clone, Copy)]
pub struct CollectionRecordLimits {
    pub max_rows: usize,
    pub text_bytes: usize,
    pub blob_bytes: usize,
}
impl Default for CollectionRecordLimits {
    fn default() -> Self {
        Self {
            max_rows: 2_000_000,
            text_bytes: 16 * 1024 * 1024,
            blob_bytes: 64 * 1024 * 1024,
        }
    }
}
impl ReadOnlyDatabase {
    /// Visit only the existing direct-reader music allowlist. The caller must
    /// retain/enforce unique (table,id) keys, e.g. in a disk spool PRIMARY KEY.
    pub fn visit_collection_records(
        &self,
        limits: CollectionRecordLimits,
        cancel: &CancellationToken,
        mut visit: impl FnMut(TargetRecord) -> Result<()>,
    ) -> Result<DatabaseSchema> {
        cancel.check()?;
        ensure!(
            limits.max_rows > 0,
            "Collection row budget must be positive"
        );
        let tx = self.conn.unchecked_transaction()?;
        let schema = crate::schema_on(&tx)?;
        ensure!(
            schema.user_version == 0 && schema.application_id == 0,
            "Unsupported direct collection database headers"
        );
        // Validate the complete shape before emitting rows; never execute a source view.
        for table in COLLECTION_IMPORT_TABLES {
            let (kind, sql): (String, String) = tx.query_row(
                "SELECT type,sql FROM sqlite_schema WHERE name=?1",
                [table],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            ensure!(
                kind == "table" && !sql.to_ascii_uppercase().contains("VIRTUAL TABLE"),
                "Direct collection requires ordinary music tables"
            );
        }
        let mut count = 0usize;
        for table in COLLECTION_IMPORT_TABLES {
            cancel.check()?;
            let key = match *table {
                "djmdProperty" => "DBID",
                "agentRegistry" => "registry_id",
                _ => "ID",
            };
            let filter = if *table == "agentRegistry" {
                " WHERE registry_id='localUpdateCount'"
            } else {
                ""
            };
            let mut statement =
                tx.prepare(&format!("SELECT * FROM \"{table}\"{filter} ORDER BY rowid"))?;
            let names = statement
                .column_names()
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                cancel.check()?;
                count = count
                    .checked_add(1)
                    .context("Collection row count overflow")?;
                ensure!(count <= limits.max_rows, "Collection read budget exceeded");
                let mut values = BTreeMap::new();
                let mut bytes = 0usize;
                for (index, name) in names.iter().enumerate() {
                    let value = match row.get_ref(index)? {
                        ValueRef::Null => TargetValue::Null,
                        ValueRef::Integer(v) => TargetValue::Integer(v),
                        ValueRef::Real(v) => {
                            ensure!(v.is_finite(), "Nonfinite collection value");
                            TargetValue::Real(v)
                        }
                        ValueRef::Text(v) => {
                            ensure!(v.len() <= limits.text_bytes, "Collection text byte budget");
                            bytes = bytes.checked_add(v.len()).context("Record size overflow")?;
                            TargetValue::Text(
                                std::str::from_utf8(v)
                                    .context("Unsupported collection text encoding")?
                                    .into(),
                            )
                        }
                        ValueRef::Blob(v) => {
                            ensure!(v.len() <= limits.blob_bytes, "Collection blob byte budget");
                            bytes = bytes.checked_add(v.len()).context("Record size overflow")?;
                            TargetValue::Blob(v.to_vec())
                        }
                    };
                    ensure!(bytes <= limits.blob_bytes, "Collection record byte budget");
                    values.insert(name.clone(), value);
                }
                let id = match values.get(key) {
                    Some(TargetValue::Text(v)) => v.clone(),
                    Some(TargetValue::Integer(v)) => v.to_string(),
                    _ => anyhow::bail!("Missing target row identity"),
                };
                ensure!(!id.is_empty(), "Empty target row identity");
                let fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(&values)?));
                visit(TargetRecord {
                    table: (*table).into(),
                    id,
                    values,
                    fingerprint,
                })?;
            }
        }
        tx.commit()?;
        Ok(schema)
    }
}
