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
pub(crate) fn row_values(
    columns: &[String],
    row: &rusqlite::Row<'_>,
) -> Result<BTreeMap<String, TargetValue>> {
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
    Ok(values)
}
/// The same fingerprint a collection read records for a row, so a writer can tell whether the row
/// still matches what was last observed.
pub(crate) fn row_fingerprint(
    conn: &Connection,
    table: &str,
    key: &str,
    id: &str,
) -> Result<Option<(String, BTreeMap<String, TargetValue>)>> {
    let mut q = conn.prepare(&format!("SELECT * FROM \"{table}\" WHERE \"{key}\"=?1"))?;
    let columns = q
        .column_names()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut cursor = q.query([id])?;
    let Some(row) = cursor.next()? else {
        return Ok(None);
    };
    let values = row_values(&columns, row)?;
    ensure!(
        cursor.next()?.is_none(),
        "Ambiguous target row identity in {table}"
    );
    Ok(Some((
        format!("{:x}", Sha256::digest(serde_json::to_vec(&values)?)),
        values,
    )))
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
        let values = row_values(&columns, row)?;
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
