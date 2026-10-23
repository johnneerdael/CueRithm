use super::*;
use crate::Library;
use anyhow::{Context, Result, ensure};
use cuerithm_dj_sync::import::*;
use cuerithm_media::CancellationToken;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) fn project_import_collection_tx(
    conn: &Connection,
    source_id: &str,
    id: &str,
    collection: &CollectionObservation,
    parent_id: Option<&str>,
) -> Result<()> {
    let mut metadata = collection.clone();
    metadata.occurrences.clear();
    conn.execute("INSERT INTO import_collections(id,source_id,external_id,parent_id,kind,name,json) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(id) DO UPDATE SET parent_id=excluded.parent_id,kind=excluded.kind,name=excluded.name,json=excluded.json,revision=import_collections.revision+1",params![id,source_id,collection.external_id,parent_id,serde_json::to_string(&collection.kind)?,collection.name,serde_json::to_string(&metadata)?])?;
    conn.execute(
        "DELETE FROM import_memberships WHERE collection_id=?1",
        [id],
    )?;
    for (position, member) in collection.occurrences.iter().enumerate() {
        append_member(conn, source_id, id, position as u64, member)?;
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
        let has_positions:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM import_memberships WHERE collection_id=?1 AND json_extract(json,'$.position') IS NOT NULL)",[id],|r|r.get(0))?;
        let ordered = collection
            .extensions
            .get("ordered")
            .and_then(Value::as_bool)
            .unwrap_or(kind != CollectionKind::Crate || has_positions);
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

pub(crate) fn append_member(
    conn: &Connection,
    source: &str,
    id: &str,
    position: u64,
    member: &MembershipObservation,
) -> Result<()> {
    let item: Option<String> = conn
        .query_row(
            "SELECT item_id FROM import_bindings WHERE source_id=?1 AND external_id=?2",
            params![source, member.track_external_id],
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
            position,
            serde_json::to_string(member)?
        ],
    )?;
    Ok(())
}
pub(crate) fn for_source_members(
    conn: &Connection,
    snapshot: &str,
    collection: &CollectionObservation,
    cancel: &CancellationToken,
    mut visit: impl FnMut(u64, &MembershipObservation) -> Result<()>,
) -> Result<()> {
    let mut position = 0u64;
    for member in &collection.occurrences {
        cancel.check()?;
        visit(position, member)?;
        position += 1;
    }
    let mut statement=conn.prepare("SELECT json FROM import_observations WHERE snapshot_id=?1 AND kind='membership' AND collection_id=?2 ORDER BY sequence")?;
    let mut rows = statement.query(params![snapshot, collection.external_id])?;
    while let Some(row) = rows.next()? {
        cancel.check()?;
        if let ObservationRecord::Membership { occurrence, .. } =
            serde_json::from_str(&row.get::<_, String>(0)?)?
        {
            visit(position, &occurrence)?;
            position += 1;
        }
    }
    Ok(())
}
struct MembershipDigest {
    digest: Sha256,
    count: u64,
}
impl MembershipDigest {
    fn new() -> Self {
        Self {
            digest: Sha256::new(),
            count: 0,
        }
    }
    fn push(&mut self, member: &MembershipObservation) -> Result<()> {
        let raw = serde_json::to_vec(member)?;
        self.digest.update((raw.len() as u64).to_le_bytes());
        self.digest.update(raw);
        self.count += 1;
        Ok(())
    }
    fn finish(self, collection: &CollectionObservation) -> Result<Value> {
        let mut metadata = collection.clone();
        metadata.occurrences.clear();
        Ok(
            json!({"metadata":metadata,"membership_count":self.count,"membership_sha256":format!("{:x}",self.digest.finalize())}),
        )
    }
}
pub(crate) fn source_collection_value(
    conn: &Connection,
    snapshot: &str,
    collection: &CollectionObservation,
    cancel: &CancellationToken,
) -> Result<Value> {
    let mut digest = MembershipDigest::new();
    for_source_members(conn, snapshot, collection, cancel, |_, member| {
        digest.push(member)
    })?;
    digest.finish(collection)
}
pub(crate) fn local_collection_value(
    conn: &Connection,
    id: &str,
    cancel: &CancellationToken,
) -> Result<Option<Value>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT json FROM import_collections WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(raw) = raw else { return Ok(None) };
    let collection: CollectionObservation = serde_json::from_str(&raw)?;
    let mut digest = MembershipDigest::new();
    let mut query=conn.prepare("SELECT json FROM import_memberships WHERE collection_id=?1 ORDER BY position,occurrence_id")?;
    let mut rows = query.query([id])?;
    while let Some(row) = rows.next()? {
        cancel.check()?;
        digest.push(&serde_json::from_str(&row.get::<_, String>(0)?)?)?;
    }
    Ok(Some(digest.finish(&collection)?))
}
pub(crate) fn inline_collection_value(collection: &CollectionObservation) -> Result<Value> {
    let mut digest = MembershipDigest::new();
    for member in &collection.occurrences {
        digest.push(member)?;
    }
    digest.finish(collection)
}
