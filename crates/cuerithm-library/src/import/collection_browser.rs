//! Read-only, bounded discovery of applied imported hierarchies. Occurrences
//! remain in their existing paged store, separate from editable local playlists.
use super::*;
use crate::Library;
use anyhow::{Result, ensure};
use rusqlite::params;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    nonce: String,
    revision: u64,
    source: Option<String>,
    parent: Option<String>,
    after: String,
}
impl Library {
    pub fn imported_collection_list(
        &self,
        query: ImportCollectionListQuery,
    ) -> Result<ImportedCollectionList> {
        ensure!(
            query.source_id.as_ref().is_none_or(|id| id.len() <= 128)
                && query.parent_id.as_ref().is_none_or(|id| id.len() <= 128),
            "Oversized imported collection filter"
        );
        ensure!(
            (1..=1000).contains(&query.limit),
            "Imported collection list page size must be 1..=1000"
        );
        ensure!(
            query
                .cursor
                .as_ref()
                .is_none_or(|cursor| cursor.len() <= 4096),
            "Oversized collection cursor"
        );
        let nonce = self
            .inner
            .import_runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("Import runtime interrupted"))?
            .nonce
            .clone();
        let conn = self.conn()?;
        let revision: u64 =
            conn.query_row("SELECT value FROM meta WHERE key='revision'", [], |row| {
                row.get(0)
            })?;
        let after = if let Some(cursor) = query.cursor {
            let cursor: Cursor = serde_json::from_str(&cursor)?;
            ensure!(
                cursor.nonce == nonce
                    && cursor.revision == revision
                    && cursor.source == query.source_id
                    && cursor.parent == query.parent_id,
                "Stale collection cursor; library changed, reload imported collections"
            );
            cursor.after
        } else {
            String::new()
        };
        let total=conn.query_row("SELECT COUNT(*) FROM import_collections WHERE (?1 IS NULL OR source_id=?1) AND ((?2 IS NULL AND parent_id IS NULL) OR parent_id=?2)",params![query.source_id,query.parent_id],|row|row.get(0))?;
        let mut statement=conn.prepare("SELECT c.id,c.source_id,s.provider,c.parent_id,substr(c.name,1,4096),length(c.name)>4096,c.kind,(SELECT COUNT(*) FROM import_memberships m WHERE m.collection_id=c.id) FROM import_collections c JOIN import_sources s ON s.id=c.source_id WHERE (?1 IS NULL OR c.source_id=?1) AND ((?2 IS NULL AND c.parent_id IS NULL) OR c.parent_id=?2) AND c.id>?3 ORDER BY c.id LIMIT ?4")?;
        let mut rows = statement.query(params![
            query.source_id,
            query.parent_id,
            after,
            query.limit + 1
        ])?;
        let mut records = Vec::new();
        let mut last = after;
        let mut more = false;
        let mut bytes = 0;
        while let Some(row) = rows.next()? {
            if records.len() == query.limit as usize {
                more = true;
                break;
            }
            let entry = ImportedCollectionEntry {
                id: row.get(0)?,
                source_id: row.get(1)?,
                provider: serde_json::from_str(&row.get::<_, String>(2)?)?,
                parent_id: row.get(3)?,
                name: row.get(4)?,
                name_truncated: row.get(5)?,
                kind: serde_json::from_str(&row.get::<_, String>(6)?)?,
                occurrences: row.get(7)?,
            };
            let size = serde_json::to_vec(&entry)?.len();
            if !records.is_empty() && bytes + size > 4 * 1024 * 1024 {
                more = true;
                break;
            }
            bytes += size;
            last = entry.id.clone();
            records.push(entry);
        }
        Ok(ImportedCollectionList {
            records,
            total,
            next_cursor: if more {
                Some(serde_json::to_string(&Cursor {
                    nonce,
                    revision,
                    source: query.source_id,
                    parent: query.parent_id,
                    after: last,
                })?)
            } else {
                None
            },
        })
    }
}
