use crate::*;
use rusqlite::types::ValueRef;
use std::collections::{BTreeMap, BTreeSet};
impl TargetRecord {
    pub(crate) fn text(&self, name: &str) -> Result<Option<String>> {
        Ok(match self.values.get(name) {
            None | Some(TargetValue::Null) => None,
            Some(TargetValue::Text(v)) => Some(v.clone()),
            Some(TargetValue::Integer(v)) if name.ends_with("ID") || name == "ID" => {
                Some(v.to_string())
            }
            _ => anyhow::bail!("Invalid text field {}.{name}", self.table),
        })
    }
    pub(crate) fn int(&self, name: &str) -> Result<Option<i64>> {
        Ok(match self.values.get(name) {
            None | Some(TargetValue::Null) => None,
            Some(TargetValue::Integer(v)) => Some(*v),
            Some(TargetValue::Real(v))
                if v.fract() == 0. && *v >= i64::MIN as f64 && *v < (i64::MAX as f64) =>
            {
                Some(*v as i64)
            }
            _ => anyhow::bail!("Invalid integer field {}.{name}", self.table),
        })
    }
    pub(crate) fn number(&self, name: &str) -> Result<Option<f64>> {
        Ok(match self.values.get(name) {
            None | Some(TargetValue::Null) => None,
            Some(TargetValue::Integer(v)) => Some(*v as f64),
            Some(TargetValue::Real(v)) => Some(*v),
            _ => anyhow::bail!("Invalid numeric field {}.{name}", self.table),
        })
    }
    pub(crate) fn flag(&self, name: &str) -> Result<Option<bool>> {
        Ok(match self.int(name)? {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        })
    }
    pub(crate) fn unsigned(&self, name: &str) -> Result<Option<u64>> {
        Ok(self.int(name)?.and_then(|v| u64::try_from(v).ok()))
    }
    pub(crate) fn u32(&self, name: &str) -> Result<Option<u32>> {
        Ok(self.unsigned(name)?.and_then(|v| u32::try_from(v).ok()))
    }
}
pub(crate) fn read_records(
    conn: &Connection,
    table: &str,
    key: &str,
    count: &mut usize,
    max: usize,
    cancel: &CancellationToken,
) -> Result<Vec<TargetRecord>> {
    ensure!(
        matches!(
            table,
            "djmdProperty"
                | "agentRegistry"
                | "djmdContent"
                | "djmdArtist"
                | "djmdAlbum"
                | "djmdGenre"
                | "djmdLabel"
                | "djmdKey"
                | "djmdCue"
                | "contentCue"
                | "djmdPlaylist"
                | "djmdSongPlaylist"
                | "djmdMyTag"
                | "djmdSongMyTag"
                | "contentFile"
        ),
        "Unsupported import table"
    );
    let condition = if table == "agentRegistry" {
        " WHERE registry_id='localUpdateCount'"
    } else {
        ""
    };
    let mut q = conn
        .prepare(&format!(
            "SELECT * FROM \"{table}\"{condition} ORDER BY rowid"
        ))
        .with_context(|| format!("Unsupported target schema: {table}"))?;
    let columns = q
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut cursor = q.query([])?;
    let mut output = vec![];
    let mut seen = BTreeSet::new();
    while let Some(row) = cursor.next()? {
        cancel.check()?;
        *count = count.checked_add(1).context("Import row count overflow")?;
        ensure!(
            *count <= max,
            "Collection read budget exceeded; no partial import returned"
        );
        let mut values = BTreeMap::new();
        for (index, name) in columns.iter().enumerate() {
            let value = match row.get_ref(index)? {
                ValueRef::Null => TargetValue::Null,
                ValueRef::Integer(v) => TargetValue::Integer(v),
                ValueRef::Real(v) => {
                    ensure!(v.is_finite(), "Nonfinite target value");
                    TargetValue::Real(v)
                }
                ValueRef::Text(v) => TargetValue::Text(
                    std::str::from_utf8(v)
                        .context("Unsupported target text encoding")?
                        .into(),
                ),
                ValueRef::Blob(v) => TargetValue::Blob(v.to_vec()),
            };
            values.insert(name.clone(), value);
        }
        let id = match values.get(key) {
            Some(TargetValue::Text(v)) => v.clone(),
            Some(TargetValue::Integer(v)) => v.to_string(),
            _ => anyhow::bail!("Missing target identity: {table}.{key}"),
        };
        ensure!(
            !id.is_empty() && seen.insert(id.clone()),
            "Ambiguous target row identity in {table}"
        );
        let fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(&values)?));
        output.push(TargetRecord {
            table: table.into(),
            id,
            values,
            fingerprint,
        });
    }
    Ok(output)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeColumn {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub default: Option<String>,
    pub primary: bool,
}
impl ReadOnlyDatabase {
    /// Inspect column contracts without retaining or exposing target rows.
    pub fn table_columns(&self, table: &str) -> Result<Vec<NativeColumn>> {
        ensure!(
            matches!(
                table,
                "djmdContent"
                    | "djmdArtist"
                    | "djmdAlbum"
                    | "djmdGenre"
                    | "djmdLabel"
                    | "djmdKey"
                    | "djmdCue"
                    | "contentCue"
                    | "contentFile"
                    | "djmdPlaylist"
                    | "djmdSongPlaylist"
                    | "djmdMyTag"
                    | "djmdSongMyTag"
                    | "agentRegistry"
            ),
            "Unsupported native table"
        );
        let mut query = self
            .conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
        Ok(query
            .query_map([], |row| {
                Ok(NativeColumn {
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    required: row.get(3)?,
                    default: row.get(4)?,
                    primary: row.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }
    /// Auxiliary native asset references stay separate from the original import contract.
    pub fn read_asset_records(
        &self,
        max_rows: usize,
        cancel: &CancellationToken,
    ) -> Result<Vec<TargetRecord>> {
        ensure!(max_rows > 0, "Native asset row budget must be positive");
        let present: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='contentFile')",
            [],
            |r| r.get(0),
        )?;
        if !present {
            return Ok(Vec::new());
        }
        read_records(&self.conn, "contentFile", "ID", &mut 0, max_rows, cancel)
    }
}
