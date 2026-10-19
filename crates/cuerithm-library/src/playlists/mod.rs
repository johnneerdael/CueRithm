mod edits;
mod evaluate;
mod generator;
mod history;
mod model;
mod preferences;
pub(crate) use preferences::zone;
mod previews;
pub(crate) mod queries;
pub(crate) mod scope;
mod service;
mod snapshot;
mod tools;
mod tree;
mod workspace;
use crate::{Library, now};
use anyhow::{Context, Result, ensure};
use cuerithm_smartlists::PlaylistReference;
pub use evaluate::SmartEvaluation;
use model::{Delta, StoredNode, Tree};
pub use model::{
    GeneratedSmartList, GenerationDimension, GenerationPreview, GenerationTarget,
    NativePlaylistFacts, PlaylistChange, PlaylistCombineMode, PlaylistEdit, PlaylistKind,
    PlaylistNode, PlaylistPreferences, PlaylistPreferencesSnapshot, PlaylistPrefix,
    PlaylistPreviewItem, PlaylistSortOrder, PlaylistState, PlaylistToolPreview,
    PlaylistToolRequest, PlaylistWorkspace, SmartCategoryChoice, SmartDefinition,
    SmartFieldCatalog, SmartGenerationRequest, SmartListFailure, SmartListStatus, SmartPreview,
    SmartPreviewScope, SmartRefresh, SmartStamp, SmartState, SmartTagChoice,
};
use rusqlite::{Connection, TransactionBehavior, params};
pub use service::SmartPlaylistService;
use std::collections::BTreeSet;

pub(crate) fn load(conn: &Connection) -> Result<Tree> {
    let mut tree = Tree::new();
    let mut statement=conn.prepare("SELECT id,parent_id,kind,name,position,favorite_position,definition_json,deleted,deleted_by,last_edit_id FROM playlists")?;
    let rows = statement
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, u32>(4)?,
                r.get::<_, Option<u32>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, bool>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (
        id,
        parent_id,
        kind,
        name,
        position,
        favorite_position,
        definition,
        deleted,
        deleted_by,
        last_edit_id,
    ) in rows
    {
        let kind = match kind.as_str() {
            "playlist" => PlaylistKind::Playlist,
            "folder" => PlaylistKind::Folder,
            "smartlist" => PlaylistKind::Smartlist,
            _ => anyhow::bail!("Unknown stored playlist kind"),
        };
        tree.insert(
            id.clone(),
            StoredNode {
                last_edit_id,
                node: PlaylistNode {
                    id,
                    parent_id,
                    kind,
                    name,
                    position,
                    favorite_position,
                    definition: definition.map(|s| serde_json::from_str(&s)).transpose()?,
                    deleted,
                    deleted_by,
                },
                members: vec![],
            },
        );
    }
    let mut statement = conn.prepare(
        "SELECT playlist_id,track_id FROM playlist_members ORDER BY playlist_id,position",
    )?;
    for row in statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (id, track) = row?;
        tree.get_mut(&id)
            .context("Orphaned playlist membership")?
            .members
            .push(track);
    }
    Ok(tree)
}
fn revision(conn: &Connection) -> Result<u64> {
    Ok(conn.query_row(
        "SELECT value FROM meta WHERE key='playlist_revision'",
        [],
        |r| r.get(0),
    )?)
}
fn diff(before: &Tree, after: &Tree) -> (Delta, Delta) {
    let ids: BTreeSet<_> = before.keys().chain(after.keys()).cloned().collect();
    let mut a = Delta::new();
    let mut b = Delta::new();
    for id in ids {
        if before.get(&id) != after.get(&id) {
            a.insert(id.clone(), before.get(&id).cloned());
            b.insert(id.clone(), after.get(&id).cloned());
        }
    }
    (a, b)
}
fn persist(conn: &Connection, before: &Delta, after: &Delta) -> Result<()> {
    for (id, row) in after {
        if let Some(row) = row {
            let n = &row.node;
            let kind = match n.kind {
                PlaylistKind::Playlist => "playlist",
                PlaylistKind::Folder => "folder",
                PlaylistKind::Smartlist => "smartlist",
            };
            conn.execute("INSERT INTO playlists(id,parent_id,kind,name,position,favorite_position,definition_json,deleted,deleted_by,last_edit_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(id) DO UPDATE SET parent_id=excluded.parent_id,kind=excluded.kind,name=excluded.name,position=excluded.position,favorite_position=excluded.favorite_position,definition_json=excluded.definition_json,deleted=excluded.deleted,deleted_by=excluded.deleted_by,last_edit_id=excluded.last_edit_id",params![id,n.parent_id,kind,n.name,n.position,n.favorite_position,n.definition.as_ref().map(serde_json::to_string).transpose()?,n.deleted,n.deleted_by,row.last_edit_id])?;
            if before.get(id).and_then(Option::as_ref).map(|r| &r.members) != Some(&row.members) {
                conn.execute("DELETE FROM playlist_members WHERE playlist_id=?1", [id])?;
                for (position, track) in row.members.iter().enumerate() {
                    conn.execute(
                        "INSERT INTO playlist_members VALUES(?1,?2,?3)",
                        params![id, track, position as u64],
                    )?;
                }
            }
            if n.kind == PlaylistKind::Smartlist {
                conn.execute("INSERT OR IGNORE INTO smart_playlist_status(playlist_id,state) VALUES(?1,'pending')",[id])?;
            }
        }
    }
    // Parent references are deferred, so an Undo can remove a new subtree atomically.
    for (id, row) in after {
        if row.is_none() {
            conn.execute("DELETE FROM playlists WHERE id=?1", [id])?;
        }
    }
    conn.execute(
        "UPDATE meta SET value=value+1 WHERE key='playlist_revision'",
        [],
    )?;
    conn.execute(
        "UPDATE smart_playlist_status SET state='stale' WHERE state='ready'",
        [],
    )?;
    Library::bump(conn)?;
    Ok(())
}
fn commit_tree_change(
    tx: &rusqlite::Transaction<'_>,
    before: &Tree,
    mut after: Tree,
    created: Vec<String>,
    expected: u64,
    id: String,
) -> Result<PlaylistChange> {
    let current = revision(tx)?;
    ensure!(
        current == expected,
        "Playlist changed; refresh before editing"
    );
    let tracks = tree::known_tracks(tx)?;
    tree::validate(before, &tracks)?;
    tree::normalize(&mut after);
    tree::validate(&after, &tracks)?;
    for (key, row) in &mut after {
        if before.get(key) != Some(&*row) {
            row.last_edit_id = id.clone();
        }
    }
    let (a, b) = diff(before, &after);
    if a.is_empty() {
        return Ok(PlaylistChange {
            edit_id: None,
            revision: current,
            created_ids: vec![],
            affected_ids: vec![],
        });
    }
    persist(tx, &a, &b)?;
    tx.execute("INSERT INTO playlist_edits(id,kind,before_json,after_json,created) VALUES(?1,'edit',?2,?3,?4)",params![id,serde_json::to_string(&a)?,serde_json::to_string(&b)?,now()])?;
    Ok(PlaylistChange {
        edit_id: Some(id),
        revision: revision(tx)?,
        created_ids: created,
        affected_ids: b.keys().cloned().collect(),
    })
}

impl Library {
    pub fn playlist_state(&self) -> Result<PlaylistState> {
        let conn = self.conn()?;
        let tree = load(&conn)?;
        let mut nodes: Vec<_> = tree.into_values().map(|s| s.node).collect();
        nodes.sort_by(|a, b| {
            (&a.parent_id, a.position, &a.id).cmp(&(&b.parent_id, b.position, &b.id))
        });
        Ok(PlaylistState {
            revision: revision(&conn)?,
            nodes,
        })
    }
    /// Raw ordinary membership retains archived/Incoming IDs. Track queries decide visibility.
    pub fn manual_playlist_members(&self, id: &str) -> Result<Vec<String>> {
        let conn = self.conn()?;
        let tree = load(&conn)?;
        let row = tree::active(&tree, id)?;
        ensure!(
            row.node.kind == PlaylistKind::Playlist,
            "Not an ordinary playlist"
        );
        Ok(row.members.clone())
    }
    pub fn smart_input_revision(&self) -> Result<u64> {
        Ok(self.conn()?.query_row(
            "SELECT value FROM meta WHERE key='smart_input_revision'",
            [],
            |r| r.get(0),
        )?)
    }
    pub fn resolve_playlist_reference(&self, reference: &PlaylistReference) -> Result<String> {
        let conn = self.conn()?;
        tree::resolve(&load(&conn)?, reference)
    }
    pub fn apply_playlist_edit(
        &self,
        edit: PlaylistEdit,
        expected_revision: u64,
    ) -> Result<PlaylistChange> {
        let mut conn = self.conn()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure!(
            revision(&tx)? == expected_revision,
            "Playlist changed; refresh before editing"
        );
        let before = load(&tx)?;
        let tracks = tree::known_tracks(&tx)?;
        tree::validate(&before, &tracks)?;
        let mut after = before.clone();
        let mut created = vec![];
        let operation = uuid::Uuid::new_v4().to_string();
        edits::apply(
            &mut after,
            &tracks,
            edit,
            &operation,
            &mut created,
            0,
            &mut 0,
        )?;
        let change =
            commit_tree_change(&tx, &before, after, created, expected_revision, operation)?;
        tx.commit()?;
        Ok(change)
    }
    pub fn replace_playlist_track_refs(
        &self,
        old_ids: &[String],
        keeper: &str,
        expected_revision: u64,
    ) -> Result<PlaylistChange> {
        self.apply_playlist_edit(
            PlaylistEdit::ReplaceTrackRefs {
                old_ids: old_ids.to_vec(),
                keeper: keeper.into(),
            },
            expected_revision,
        )
    }
}
