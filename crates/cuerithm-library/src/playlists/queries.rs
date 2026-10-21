use super::*;
use crate::{TrackPage, TrackQuery, TrackSort, TrackView};
use rusqlite::named_params;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
pub(crate) fn scoped_members(
    conn: &Connection,
    tree: &Tree,
    id: &str,
) -> Result<Vec<(String, bool, bool)>> {
    let root = tree::active(tree, id)?;
    let ids = if root.node.kind == PlaylistKind::Folder {
        evaluate::ordered_descendants(tree, id)
    } else {
        vec![id.to_owned()]
    };
    let mut members: Vec<(String, bool, bool)> = vec![];
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for id in ids {
        let node = &tree[&id];
        let (rows, archive, incoming) = match node.node.kind {
            PlaylistKind::Folder => continue,
            PlaylistKind::Playlist => (node.members.clone(), false, false),
            PlaylistKind::Smartlist => {
                let mut q=conn.prepare("SELECT track_id FROM smart_playlist_members WHERE playlist_id=?1 ORDER BY position")?;
                let rows = q
                    .query_map([&id], |r| r.get(0))?
                    .collect::<rusqlite::Result<Vec<String>>>()?;
                (
                    rows,
                    node.node
                        .definition
                        .as_ref()
                        .is_some_and(evaluate::allows_archive),
                    node.node
                        .definition
                        .as_ref()
                        .is_some_and(evaluate::allows_incoming),
                )
            }
        };
        for track in rows {
            if let Some(position) = index.get(&track).copied() {
                members[position].1 |= archive;
                members[position].2 |= incoming;
            } else {
                index.insert(track.clone(), members.len());
                members.push((track, archive, incoming));
            }
        }
    }
    Ok(members)
}
impl Library {
    pub fn playlist_members(&self, id: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        Ok(scoped_members(&conn, &load(&conn)?, id)?
            .into_iter()
            .map(|(id, _, _)| id)
            .collect())
    }
}
struct SqlQuery {
    scope: String,
    source: &'static str,
    condition: String,
    order: String,
    needle: String,
    curation: String,
    revision: u64,
    query_id: String,
    offset: u64,
}
fn build(conn: &Connection, query: &TrackQuery) -> Result<SqlQuery> {
    ensure!((1..=500).contains(&query.limit), "Invalid page size");
    let revision: u64 = conn.query_row("SELECT value FROM meta WHERE key='revision'", [], |r| {
        r.get(0)
    })?;
    ensure!(
        query
            .expected_revision
            .is_none_or(|expected| expected == revision),
        "Library changed; refresh selection before selecting all"
    );
    crate::curation::validate_filter(conn, query.curation.as_ref())?;
    let mut filter = query.curation.clone();
    if let Some(filter) = &mut filter {
        filter.notes = filter.notes.to_lowercase();
    }
    let curation = serde_json::to_string(&filter)?;
    let query_id = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&serde_json::json!([
            query.view,
            query.search,
            query.sort,
            query.descending,
            query.limit,
            query.playlist_id,
            query.curation
        ]))?)
    );
    let offset = if let Some(cursor) = &query.cursor {
        let parts: Vec<_> = cursor.split(':').collect();
        ensure!(
            parts.len() == 3 && parts[0] == revision.to_string() && parts[2] == query_id,
            "Stale cursor; refresh the library"
        );
        parts[1].parse()?
    } else {
        0
    };
    let scoped = query.playlist_id.is_some();
    let scope = if let Some(id) = &query.playlist_id {
        let tree = load(conn)?;
        serde_json::to_string(
            &scoped_members(conn, &tree, id)?
                .iter()
                .map(|(id, archive, incoming)| serde_json::json!({"id":id,"archive":archive,"incoming":incoming}))
                .collect::<Vec<_>>(),
        )?
    } else {
        "[]".into()
    };
    let condition = match (&query.view, scoped) {
        (TrackView::Tracks, false) => "t.state='ready' AND t.archived=0",
        (TrackView::Tracks, true) => {
            "(t.state='ready' OR (t.state='incoming' AND s.incoming=1)) AND (t.archived=0 OR s.archive=1)"
        }
        (TrackView::Incoming, _) => "t.state='incoming' AND t.archived=0",
        (TrackView::Archive, _) => "t.archived=1",
    };
    let column = match query.sort {
        TrackSort::Title => "t.sort_title",
        TrackSort::Artist => "t.sort_artist",
        TrackSort::Bpm => "t.bpm",
        TrackSort::Energy => "t.energy",
        TrackSort::Added => "t.created",
        TrackSort::UserRating => {
            "COALESCE((SELECT json_extract(committed_json,'$.user_rating') FROM curation_tracks WHERE track_id=t.id),0)"
        }
        TrackSort::PlaylistOrder => {
            ensure!(scoped, "Choose a playlist before using playlist order");
            "s.ordinal"
        }
    };
    Ok(SqlQuery {
        scope,
        source: if scoped {
            "tracks t JOIN scope s ON s.id=t.id"
        } else {
            "tracks t"
        },
        condition: format!(
            "{condition} AND instr(t.search_text,:needle)>0 AND ({})",
            crate::curation::FILTER_SQL
        ),
        curation,
        order: format!(
            "{column} {},t.id ASC",
            if query.descending { "DESC" } else { "ASC" }
        ),
        needle: query.search.to_lowercase(),
        revision,
        query_id,
        offset,
    })
}
const CTE: &str = "WITH scope AS (SELECT json_extract(value,'$.id') id,CAST(key AS INTEGER) ordinal,json_extract(value,'$.archive') archive,json_extract(value,'$.incoming') incoming FROM json_each(:scope))";
pub(crate) fn tracks(lib: &Library, query: TrackQuery) -> Result<TrackPage> {
    let conn = lib.conn()?;
    let sql = build(&conn, &query)?;
    let total: u64 = conn.query_row(
        &format!(
            "{CTE} SELECT COUNT(*) FROM {} WHERE {}",
            sql.source, sql.condition
        ),
        named_params! {":scope":sql.scope,":needle":sql.needle,":curation":sql.curation},
        |r| r.get(0),
    )?;
    let mut q=conn.prepare(&format!("{CTE} SELECT t.id,t.path,t.title,t.artist,t.album,t.genre,t.state,t.musical_key,t.bpm,t.energy,t.duration,t.source_hash,t.metadata_json,t.created,t.updated,t.archived,json_extract(t.tagging_json,'$.status') FROM {} WHERE {} ORDER BY {} LIMIT :limit OFFSET :offset",sql.source,sql.condition,sql.order))?;
    let mut rows=q.query_map(named_params!{":scope":sql.scope,":needle":sql.needle,":curation":sql.curation,":limit":query.limit,":offset":sql.offset},crate::store::summary_row)?.collect::<rusqlite::Result<Vec<_>>>()?;
    for row in &mut rows {
        crate::curation::project_summary(&conn, row)?;
    }
    let next = sql.offset + rows.len() as u64;
    Ok(TrackPage {
        rows,
        total,
        revision: sql.revision,
        next_cursor: (next < total).then(|| format!("{}:{next}:{}", sql.revision, sql.query_id)),
    })
}
pub(crate) fn track_ids(lib: &Library, query: TrackQuery) -> Result<Vec<String>> {
    let conn = lib.conn()?;
    let sql = build(&conn, &query)?;
    let mut q = conn.prepare(&format!(
        "{CTE} SELECT t.id FROM {} WHERE {} ORDER BY {}",
        sql.source, sql.condition, sql.order
    ))?;
    Ok(q.query_map(
        named_params! {":scope":sql.scope,":needle":sql.needle,":curation":sql.curation},
        |r| r.get(0),
    )?
    .collect::<rusqlite::Result<_>>()?)
}
