use crate::{
    SyncLibrary,
    ownership::{
        PlaylistBinding, PlaylistOwnership, PlaylistOwnershipReport, PlaylistOwnershipRequest,
        fingerprint,
    },
};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{BTreeMap, BTreeSet, HashMap};

const IDENTITY: &str = "pioneer-device-library-plus-1000";
fn node_fingerprint(db: &Connection, id: u32) -> Result<Option<String>> {
    let node: Option<(u32, String, u32)> = db
        .query_row(
            "SELECT playlist_id_parent,name,attribute FROM playlist WHERE playlist_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some(node) = node else { return Ok(None) };
    let members=db.prepare("SELECT content_id FROM playlist_content WHERE playlist_id=?1 ORDER BY sequenceNo,rowid")?.query_map([id],|r|r.get::<_,u32>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let children=db.prepare("SELECT playlist_id FROM playlist WHERE playlist_id_parent=?1 ORDER BY sequenceNo,playlist_id")?.query_map([id],|r|r.get::<_,u32>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some(fingerprint(&(id, node, members, children))?))
}
fn native_id(binding: &PlaylistBinding) -> Result<u32> {
    binding
        .locator
        .parse()
        .context("Invalid Pioneer playlist binding")
}

pub(super) struct Session {
    pub ids: HashMap<String, u32>,
    previous: BTreeMap<String, PlaylistBinding>,
    created: BTreeSet<String>,
    deleted: BTreeSet<String>,
    retired: BTreeSet<String>,
}
impl Session {
    pub fn prepare(
        db: &Connection,
        library: &SyncLibrary,
        request: &PlaylistOwnershipRequest,
    ) -> Result<Self> {
        ensure!(
            request.previous.identity.is_empty() || request.previous.identity == IDENTITY,
            "Pioneer library identity changed"
        );
        let previous: BTreeMap<_, _> = request
            .previous
            .bindings
            .iter()
            .map(|b| (b.source_id.clone(), b.clone()))
            .collect();
        ensure!(
            previous.len() == request.previous.bindings.len(),
            "Duplicate source ownership bindings"
        );
        let mut native = BTreeSet::new();
        for binding in previous.values() {
            let id = native_id(binding)?;
            ensure!(
                native.insert(id),
                "Pioneer native playlist has multiple source bindings"
            );
            if (!request.removed.contains(&binding.source_id) || binding.created)
                && let Some(current) = node_fingerprint(db, id)?
            {
                ensure!(
                    current == binding.fingerprint,
                    "Pioneer playlist was edited outside CueRithm; reconcile before syncing"
                );
            }
        }
        let mut session = Self {
            ids: HashMap::new(),
            previous,
            created: BTreeSet::new(),
            deleted: BTreeSet::new(),
            retired: BTreeSet::new(),
        };
        let mut next: u32 = db.query_row(
            "SELECT COALESCE(MAX(playlist_id),0)+1 FROM playlist",
            [],
            |r| r.get(0),
        )?;
        let mut pending: Vec<_> = library.playlists.iter().collect();
        while !pending.is_empty() {
            let count = pending.len();
            let mut deferred = Vec::new();
            for playlist in pending {
                ensure!(
                    !request.removed.contains(&playlist.id),
                    "Active playlist is also marked removed"
                );
                let parent = if let Some(parent) = &playlist.parent_id {
                    if let Some(id) = session.ids.get(parent) {
                        *id
                    } else {
                        deferred.push(playlist);
                        continue;
                    }
                } else {
                    0
                };
                let prior = session.previous.get(&playlist.id);
                let bound = prior.map(native_id).transpose()?;
                let present = if let Some(id) = bound {
                    node_fingerprint(db, id)?.is_some().then_some(id)
                } else {
                    None
                };
                // A missing bound ID is never rebound to a same-name foreign object.
                let matched = if prior.is_none() {
                    let candidates=db.prepare("SELECT playlist_id FROM playlist WHERE name=?1 AND playlist_id_parent=?2 ORDER BY playlist_id")?.query_map(params![playlist.name,parent],|r|r.get::<_,u32>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
                    ensure!(
                        candidates.len() <= 1,
                        "Ambiguous existing Pioneer playlist name"
                    );
                    candidates.first().copied()
                } else {
                    None
                };
                let id = if let Some(id) = present.or(matched) {
                    id
                } else {
                    let id = next;
                    next = next
                        .checked_add(1)
                        .context("Pioneer playlist ID overflow")?;
                    session.created.insert(playlist.id.clone());
                    id
                };
                if present.or(matched).is_some() {
                    let kind: u32 = db.query_row(
                        "SELECT attribute FROM playlist WHERE playlist_id=?1",
                        [id],
                        |r| r.get(0),
                    )?;
                    ensure!(
                        kind == u32::from(playlist.folder),
                        "Pioneer playlist kind conflicts with an existing native object"
                    );
                }
                ensure!(
                    !session.ids.values().any(|value| *value == id),
                    "Two source playlists resolve to one Pioneer playlist"
                );
                if prior.is_none() {
                    ensure!(
                        !native.contains(&id),
                        "Pioneer playlist belongs to another source binding"
                    );
                }
                db.execute("INSERT INTO playlist(playlist_id,name,attribute,playlist_id_parent,sequenceNo) VALUES(?1,?2,?3,?4,(SELECT COUNT(*) FROM playlist WHERE playlist_id_parent=?4)) ON CONFLICT(playlist_id) DO UPDATE SET name=excluded.name,attribute=excluded.attribute,playlist_id_parent=excluded.playlist_id_parent",params![id,playlist.name,u32::from(playlist.folder),parent])?;
                session.ids.insert(playlist.id.clone(), id);
            }
            ensure!(
                deferred.len() < count,
                "Invalid Pioneer playlist ownership hierarchy"
            );
            pending = deferred;
        }
        let mut removed: BTreeSet<u32> = BTreeSet::new();
        for source in &request.removed {
            if let Some(binding) = session.previous.get(source) {
                if binding.created {
                    removed.insert(native_id(binding)?);
                    session.deleted.insert(source.clone());
                } else {
                    session.retired.insert(source.clone());
                }
            }
        }
        while !removed.is_empty() {
            let mut progress = false;
            for id in removed.clone() {
                let children = db
                    .prepare("SELECT playlist_id FROM playlist WHERE playlist_id_parent=?1")?
                    .query_map([id], |r| r.get::<_, u32>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                ensure!(
                    children.iter().all(|child| removed.contains(child)),
                    "Pioneer folder contains an unowned or surviving playlist"
                );
                if !children.is_empty() {
                    continue;
                }
                db.execute("DELETE FROM playlist_content WHERE playlist_id=?1", [id])?;
                db.execute("DELETE FROM playlist WHERE playlist_id=?1", [id])?;
                removed.remove(&id);
                progress = true;
            }
            ensure!(progress, "Cyclic Pioneer deletion hierarchy");
        }
        let parents = db
            .prepare("SELECT DISTINCT playlist_id_parent FROM playlist")?
            .query_map([], |r| r.get::<_, u32>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for parent in parents {
            let desired: Vec<_> = library
                .playlists
                .iter()
                .filter(|p| p.parent_id.as_ref().map_or(0, |id| session.ids[id]) == parent)
                .map(|p| session.ids[&p.id])
                .collect();
            let selected: BTreeSet<_> = desired.iter().copied().collect();
            let mut pending = desired.into_iter();
            let existing=db.prepare("SELECT playlist_id FROM playlist WHERE playlist_id_parent=?1 ORDER BY sequenceNo,playlist_id")?.query_map([parent],|r|r.get::<_,u32>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
            let mut ordered = Vec::new();
            for id in existing {
                if selected.contains(&id) {
                    if let Some(next) = pending.next() {
                        ordered.push(next)
                    }
                } else {
                    ordered.push(id)
                }
            }
            ordered.extend(pending);
            for (position, id) in ordered.into_iter().enumerate() {
                db.execute(
                    "UPDATE playlist SET sequenceNo=?1 WHERE playlist_id=?2",
                    params![position, id],
                )?;
            }
        }
        Ok(session)
    }
    pub fn finish(self, db: &Connection, library: &SyncLibrary) -> Result<PlaylistOwnershipReport> {
        let mut bindings = self.previous;
        for id in self.deleted.iter().chain(&self.retired) {
            bindings.remove(id);
        }
        for playlist in &library.playlists {
            let id = self.ids[&playlist.id];
            let created = self.created.contains(&playlist.id)
                || bindings.get(&playlist.id).is_some_and(|b| b.created);
            bindings.insert(
                playlist.id.clone(),
                PlaylistBinding {
                    source_id: playlist.id.clone(),
                    locator: id.to_string(),
                    parent_source_id: playlist.parent_id.clone(),
                    created,
                    fingerprint: node_fingerprint(db, id)?
                        .context("Pioneer playlist missing after staging")?,
                },
            );
        }
        // Parent fingerprints also change when selected children move or disappear.
        for binding in bindings.values_mut() {
            binding.fingerprint = node_fingerprint(db, native_id(binding)?)?
                .context("Bound Pioneer playlist disappeared")?;
        }
        Ok(PlaylistOwnershipReport {
            current: PlaylistOwnership {
                identity: IDENTITY.into(),
                bindings: bindings.into_values().collect(),
            },
            deleted: self.deleted,
            retired: self.retired,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SyncPlaylist;
    fn library(names: &[(&str, Option<&str>, &str, bool)]) -> SyncLibrary {
        SyncLibrary {
            tracks: vec![],
            playlists: names
                .iter()
                .map(|(id, parent, name, folder)| SyncPlaylist {
                    id: (*id).into(),
                    parent_id: parent.map(str::to_owned),
                    name: (*name).into(),
                    folder: *folder,
                    ..Default::default()
                })
                .collect(),
        }
    }
    fn run(
        db: &Connection,
        library: &SyncLibrary,
        request: &PlaylistOwnershipRequest,
    ) -> PlaylistOwnershipReport {
        Session::prepare(db, library, request)
            .unwrap()
            .finish(db, library)
            .unwrap()
    }
    #[test]
    fn ids_survive_rename_swap_and_move_then_delete_without_tracks() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("usb.db");
        let db = super::super::database::prepare(&path, &path).unwrap();
        let first = run(
            &db,
            &library(&[
                ("f", None, "Folder", true),
                ("a", Some("f"), "A", false),
                ("b", Some("f"), "B", false),
            ]),
            &Default::default(),
        );
        assert!(first.current.bindings.iter().all(|b| b.created));
        let second = run(
            &db,
            &library(&[
                ("f", None, "Renamed", true),
                ("a", None, "B", false),
                ("b", Some("f"), "A", false),
            ]),
            &PlaylistOwnershipRequest {
                previous: first.current.clone(),
                ..Default::default()
            },
        );
        for old in &first.current.bindings {
            assert_eq!(
                old.locator,
                second
                    .current
                    .bindings
                    .iter()
                    .find(|b| b.source_id == old.source_id)
                    .unwrap()
                    .locator
            );
        }
        let removed = run(
            &db,
            &SyncLibrary::default(),
            &PlaylistOwnershipRequest {
                previous: second.current,
                removed: BTreeSet::from(["f".into(), "a".into(), "b".into()]),
            },
        );
        assert_eq!(removed.deleted.len(), 3);
        assert!(removed.current.bindings.is_empty());
        assert_eq!(
            db.query_row("SELECT COUNT(*) FROM playlist", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            0
        );
    }
    #[test]
    fn external_match_is_retired_and_foreign_edits_block_owned_deletion() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("usb.db");
        let db = super::super::database::prepare(&path, &path).unwrap();
        db.execute("INSERT INTO playlist(playlist_id,name,attribute,playlist_id_parent,sequenceNo)VALUES(9,'External',0,0,0)",[]).unwrap();
        let first = run(
            &db,
            &library(&[("a", None, "External", false), ("f", None, "Folder", true)]),
            &Default::default(),
        );
        assert!(
            !first
                .current
                .bindings
                .iter()
                .find(|b| b.source_id == "a")
                .unwrap()
                .created
        );
        let next = run(
            &db,
            &SyncLibrary::default(),
            &PlaylistOwnershipRequest {
                previous: first.current,
                removed: BTreeSet::from(["a".into()]),
            },
        );
        assert!(next.retired.contains("a"));
        assert_eq!(
            db.query_row("SELECT name FROM playlist WHERE playlist_id=9", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "External"
        );
        let folder = next
            .current
            .bindings
            .iter()
            .find(|b| b.source_id == "f")
            .unwrap()
            .locator
            .parse::<u32>()
            .unwrap();
        db.execute("INSERT INTO playlist(playlist_id,name,attribute,playlist_id_parent,sequenceNo)VALUES(99,'Foreign',0,?1,0)",[folder]).unwrap();
        let error = Session::prepare(
            &db,
            &SyncLibrary::default(),
            &PlaylistOwnershipRequest {
                previous: next.current,
                removed: BTreeSet::from(["f".into()]),
            },
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("edited outside"));
        assert_eq!(
            db.query_row(
                "SELECT COUNT(*) FROM playlist WHERE playlist_id IN(9,99)",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            2
        );
    }
    #[test]
    fn first_match_does_not_turn_an_external_folder_into_a_leaf() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("usb.db");
        let db = super::super::database::prepare(&path, &path).unwrap();
        db.execute("INSERT INTO playlist(playlist_id,name,attribute,playlist_id_parent,sequenceNo)VALUES(9,'External',1,0,0),(10,'Foreign child',0,9,0)",[]).unwrap();
        let result = Session::prepare(
            &db,
            &library(&[("a", None, "External", false)]),
            &Default::default(),
        );
        assert!(result.err().unwrap().to_string().contains("kind conflicts"));
        assert_eq!(
            db.query_row(
                "SELECT attribute FROM playlist WHERE playlist_id=9",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db.query_row(
                "SELECT playlist_id_parent FROM playlist WHERE playlist_id=10",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
            9
        );
    }
}
