use super::*;
use crate::Library;
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::import::*;
use rusqlite::{Connection, OptionalExtension, params};

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Consumed by the immutable preview service in Task 5"
    )
)]
pub(crate) fn project_import_collection_tx(
    conn: &Connection,
    source_id: &str,
    id: &str,
    collection: &CollectionObservation,
    parent_id: Option<&str>,
) -> Result<()> {
    conn.execute("INSERT INTO import_collections(id,source_id,external_id,parent_id,kind,name,json) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET parent_id=excluded.parent_id,kind=excluded.kind,name=excluded.name,json=excluded.json,revision=import_collections.revision+1",params![id,source_id,collection.external_id,parent_id,serde_json::to_string(&collection.kind)?,collection.name,serde_json::to_string(collection)?])?;
    conn.execute(
        "DELETE FROM import_memberships WHERE collection_id=?1",
        [id],
    )?;
    for (position, member) in collection.occurrences.iter().enumerate() {
        let item: Option<String> = conn
            .query_row(
                "SELECT item_id FROM import_bindings WHERE source_id=?1 AND external_id=?2",
                params![source_id, member.track_external_id],
                |r| r.get(0),
            )
            .optional()?;
        conn.execute(
            "INSERT INTO import_memberships VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                id,
                member.external_occurrence_id,
                item,
                member.track_external_id,
                position as i64,
                serde_json::to_string(member)?
            ],
        )?;
    }
    Ok(())
}
impl Library {
    pub fn imported_collection_page(
        &self,
        id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<ImportedCollectionPage> {
        ensure!(
            (1..=1000).contains(&limit),
            "Imported collection page size must be 1..=1000"
        );
        let conn = self.conn()?;
        let (kind, raw, revision): (String, String, u64) = conn
            .query_row(
                "SELECT kind,json,revision FROM import_collections WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .context("Unknown imported collection")?;
        let kind: CollectionKind = serde_json::from_str(&kind)?;
        let collection: CollectionObservation = serde_json::from_str(&raw)?;
        let ordered = collection
            .extensions
            .get("ordered")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(
                !(kind == CollectionKind::Crate
                    && collection.occurrences.iter().all(|m| m.position.is_none())),
            );
        let after = if let Some(cursor) = cursor {
            let (cursor_id, cursor_revision, position): (String, u64, i64) =
                serde_json::from_str(cursor)?;
            ensure!(
                cursor_id == id && cursor_revision == revision,
                "Imported collection changed; refresh the page"
            );
            position
        } else {
            -1
        };
        let total: u64 = conn.query_row(
            "SELECT COUNT(*) FROM import_memberships WHERE collection_id=?1",
            [id],
            |r| r.get(0),
        )?;
        let mut query=conn.prepare("SELECT occurrence_id,item_id,external_track_id,position,json FROM import_memberships WHERE collection_id=?1 AND position>?2 ORDER BY position,occurrence_id LIMIT ?3")?;
        let members = query
            .query_map(params![id, after, limit + 1], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, u64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let more = members.len() > limit as usize;
        let mut rows = Vec::new();
        for (occurrence_id, item_id, source_track_id, position, raw) in
            members.into_iter().take(limit as usize)
        {
            let source: MembershipObservation = serde_json::from_str(&raw)?;
            let track = item_id
                .as_deref()
                .map(|id| super::projection::track_summary_tx(&conn, id))
                .transpose()?;
            rows.push(ImportedOccurrence {
                occurrence_id,
                item_id,
                source_track_id,
                position,
                source_position: source.position,
                track,
            });
        }
        let next_cursor = if more {
            Some(serde_json::to_string(&(
                id,
                revision,
                rows.last().unwrap().position,
            ))?)
        } else {
            None
        };
        Ok(ImportedCollectionPage {
            id: id.into(),
            ordered,
            kind,
            rows,
            total,
            next_cursor,
        })
    }
}
