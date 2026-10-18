use crate::Library;
use anyhow::Result;
use cuerithm_media::MediaInfo;
use rusqlite::params;
use std::collections::BTreeSet;

impl Library {
    /// Discover columns across all stored tracks, independent of pagination.
    pub fn metadata_fields(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt =
            conn.prepare("SELECT metadata_json FROM tracks WHERE metadata_json IS NOT NULL AND json_type(metadata_json,'$.codec')='text'")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut fields = BTreeSet::new();
        for row in rows {
            let info: MediaInfo = serde_json::from_str(&row?)?;
            fields.extend(info.columns().into_keys());
        }
        Ok(fields.into_iter().collect())
    }
    pub fn metadata_refresh_candidates(&self) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare("SELECT id FROM tracks WHERE state='ready' AND metadata_json IS NOT NULL AND json_type(metadata_json,'$.properties') IS NULL")?;
        Ok(stmt
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
    /// Backfill old metadata only while the captured source still owns this row.
    pub fn update_legacy_metadata(
        &self,
        id: &str,
        source_hash: &str,
        info: &MediaInfo,
    ) -> Result<bool> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let changed = tx.execute("UPDATE tracks SET metadata_json=?1 WHERE id=?2 AND source_hash=?3 AND state='ready' AND json_type(metadata_json,'$.properties') IS NULL",
            params![serde_json::to_string(info)?,id,source_hash])? > 0;
        if changed {
            Self::bump(&tx)?;
        }
        tx.commit()?;
        Ok(changed)
    }
}
