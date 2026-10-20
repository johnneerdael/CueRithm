use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, params};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub(super) enum Native {
    Playlist(i64),
    Smart(String),
}
impl Native {
    pub fn locator(&self) -> String {
        match self {
            Self::Playlist(id) => format!("playlist:{id}"),
            Self::Smart(id) => format!("smartlist:{id}"),
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        if let Some(id) = value.strip_prefix("playlist:") {
            let id = id.parse()?;
            ensure!(id > 0, "Invalid Engine playlist identity");
            Ok(Self::Playlist(id))
        } else if let Some(id) = value.strip_prefix("smartlist:") {
            ensure!(!id.is_empty(), "Missing Engine smartlist identity");
            Ok(Self::Smart(id.into()))
        } else {
            anyhow::bail!("Unknown Engine playlist locator")
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct Ordinary {
    pub id: i64,
    pub name: String,
    pub parent: i64,
    pub next: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Smart {
    pub id: String,
    pub name: String,
    pub parent: String,
    pub next_uuid: String,
    pub next_path: String,
    pub rules: String,
}
#[derive(Clone, Default)]
pub(super) struct Catalog {
    pub ordinary: BTreeMap<i64, Ordinary>,
    pub smart: BTreeMap<String, Smart>,
    pub smart_table: bool,
    pub members: BTreeMap<i64, Vec<(i64, String)>>,
}
impl Catalog {
    pub fn read(db: &Connection) -> Result<Self> {
        let ordinary = db
            .prepare(
                "SELECT id,title,COALESCE(parentListId,0),COALESCE(nextListId,0) FROM Playlist",
            )?
            .query_map([], |r| {
                let id = r.get(0)?;
                Ok((
                    id,
                    Ordinary {
                        id,
                        name: r.get(1)?,
                        parent: r.get(2)?,
                        next: r.get(3)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let smart_table = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='Smartlist')",
            [],
            |r| r.get(0),
        )?;
        let smart = if smart_table {
            db.prepare("SELECT listUuid,title,COALESCE(parentPlaylistPath,''),COALESCE(nextListUuid,''),COALESCE(nextPlaylistPath,''),rules FROM Smartlist")?.query_map([],|r|{let id=r.get::<_,String>(0)?;Ok((id.clone(),Smart{id,name:r.get(1)?,parent:r.get(2)?,next_uuid:r.get(3)?,next_path:r.get(4)?,rules:r.get(5)?}))})?.collect::<rusqlite::Result<_>>()?
        } else {
            BTreeMap::new()
        };
        let mut result = Self {
            ordinary,
            smart,
            smart_table,
            members: BTreeMap::new(),
        };
        for id in result.ordinary.keys() {
            result.members.insert(*id, direct_members(db, *id)?);
        }
        Ok(result)
    }
    pub fn paths(&self) -> Result<BTreeMap<i64, String>> {
        let mut result = BTreeMap::from([(0, String::new())]);
        for id in self.ordinary.keys() {
            let mut current = *id;
            let mut seen = BTreeSet::new();
            let mut parts = Vec::new();
            while current != 0 {
                ensure!(
                    seen.insert(current),
                    "Engine native hierarchy contains a cycle"
                );
                let node = self
                    .ordinary
                    .get(&current)
                    .context("Engine native parent is missing")?;
                parts.push(node.name.clone());
                current = node.parent;
            }
            parts.reverse();
            result.insert(*id, super::super::native_path(&parts));
        }
        Ok(result)
    }
    pub fn path_ids(&self) -> Result<BTreeMap<String, i64>> {
        let mut result = BTreeMap::new();
        for (id, path) in self.paths()? {
            ensure!(
                result.insert(path, id).is_none() || self.smart.is_empty(),
                "Engine smartlist paths are ambiguous"
            );
        }
        Ok(result)
    }
    pub fn contains(&self, native: &Native) -> bool {
        match native {
            Native::Playlist(id) => self.ordinary.contains_key(id),
            Native::Smart(id) => self.smart.contains_key(id),
        }
    }
    pub fn fingerprint(&self, native: &Native) -> Result<Option<String>> {
        match native {
            Native::Playlist(id) => {
                let Some(node) = self.ordinary.get(id) else {
                    return Ok(None);
                };
                let paths = self.paths()?;
                let mut children: Vec<_> = self
                    .ordinary
                    .values()
                    .filter(|n| n.parent == *id)
                    .map(|n| Native::Playlist(n.id))
                    .collect();
                children.extend(
                    self.smart
                        .values()
                        .filter(|n| n.parent == paths[id])
                        .map(|n| Native::Smart(n.id.clone())),
                );
                children.sort();
                Ok(Some(crate::ownership::fingerprint(&(
                    native,
                    &node.name,
                    node.parent,
                    self.members.get(id),
                    children,
                ))?))
            }
            Native::Smart(id) => {
                let Some(node) = self.smart.get(id) else {
                    return Ok(None);
                };
                let rules: serde_json::Value =
                    serde_json::from_str(&node.rules).unwrap_or_else(|_| node.rules.clone().into());
                Ok(Some(crate::ownership::fingerprint(&(
                    native,
                    &node.name,
                    &node.parent,
                    rules,
                ))?))
            }
        }
    }
    pub fn ordinary_order(&self, parent: i64) -> Result<Vec<Native>> {
        let rows: BTreeMap<_, _> = self
            .ordinary
            .values()
            .filter(|n| n.parent == parent)
            .map(|n| (n.id, n.next))
            .collect();
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let pointed: BTreeSet<_> = rows.values().copied().filter(|id| *id != 0).collect();
        let heads: Vec<_> = rows
            .keys()
            .filter(|id| !pointed.contains(id))
            .copied()
            .collect();
        ensure!(heads.len() == 1, "Engine sibling order is ambiguous");
        let mut id = heads[0];
        let mut seen = BTreeSet::new();
        let mut order = Vec::new();
        while id != 0 {
            ensure!(seen.insert(id), "Engine sibling order contains a cycle");
            order.push(Native::Playlist(id));
            id = *rows.get(&id).context("Engine sibling link is missing")?;
        }
        ensure!(
            order.len() == rows.len(),
            "Engine sibling order is incomplete"
        );
        Ok(order)
    }
    pub fn order(&self, parent: i64) -> Result<Vec<Native>> {
        let paths = self.paths()?;
        let reverse = self.path_ids()?;
        let parent_path = paths
            .get(&parent)
            .context("Engine order parent is missing")?;
        let mut predecessors: BTreeMap<Option<Native>, Native> = BTreeMap::new();
        let mut count = 0;
        for smart in self.smart.values().filter(|s| &s.parent == parent_path) {
            count += 1;
            let next = self.smart_next(smart, &reverse)?;
            if let Some(next) = &next {
                match next {
                    Native::Playlist(id) => ensure!(
                        self.ordinary.get(id).is_some_and(|n| n.parent == parent),
                        "Engine smart order points outside its parent"
                    ),
                    Native::Smart(id) => ensure!(
                        self.smart.get(id).is_some_and(|n| n.parent == *parent_path),
                        "Engine smart order points outside its parent"
                    ),
                }
            }
            ensure!(
                predecessors
                    .insert(next, Native::Smart(smart.id.clone()))
                    .is_none(),
                "Engine smartlist order is ambiguous"
            );
        }
        fn prefix(
            next: Option<Native>,
            pred: &BTreeMap<Option<Native>, Native>,
            seen: &mut BTreeSet<Native>,
            out: &mut Vec<Native>,
        ) -> Result<()> {
            if let Some(node) = pred.get(&next) {
                ensure!(
                    seen.insert(node.clone()),
                    "Engine smartlist order contains a cycle"
                );
                prefix(Some(node.clone()), pred, seen, out)?;
                out.push(node.clone());
            }
            Ok(())
        }
        let ordinary = self.ordinary_order(parent)?;
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        for node in ordinary {
            prefix(Some(node.clone()), &predecessors, &mut seen, &mut result)?;
            result.push(node);
        }
        prefix(None, &predecessors, &mut seen, &mut result)?;
        ensure!(seen.len() == count, "Engine smartlist order is incomplete");
        Ok(result)
    }
    fn smart_next(&self, node: &Smart, paths: &BTreeMap<String, i64>) -> Result<Option<Native>> {
        ensure!(
            node.next_uuid.is_empty() || node.next_path.is_empty(),
            "Engine smartlist has two successors"
        );
        if !node.next_uuid.is_empty() {
            Ok(Some(Native::Smart(node.next_uuid.clone())))
        } else if !node.next_path.is_empty() {
            Ok(Some(Native::Playlist(
                *paths
                    .get(&node.next_path)
                    .context("Engine smartlist successor path is missing")?,
            )))
        } else {
            Ok(None)
        }
    }
}
pub(super) fn direct_members(db: &Connection, list: i64) -> Result<Vec<(i64, String)>> {
    let rows=db.prepare("SELECT id,trackId,COALESCE(databaseUuid,''),COALESCE(nextEntityId,0) FROM PlaylistEntity WHERE listId=?1 AND COALESCE(membershipReference,0)=0")?.query_map([list],|r|Ok((r.get::<_,i64>(0)?,(r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?))))?.collect::<rusqlite::Result<BTreeMap<_,_>>>()?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let pointed: BTreeSet<_> = rows
        .values()
        .filter_map(|(_, _, next)| rows.contains_key(next).then_some(*next))
        .collect();
    let heads: Vec<_> = rows
        .keys()
        .filter(|id| !pointed.contains(id))
        .copied()
        .collect();
    ensure!(
        heads.len() == 1,
        "Engine direct membership order is ambiguous"
    );
    let mut next = heads[0];
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    while let Some((track, uuid, id)) = rows.get(&next) {
        ensure!(seen.insert(next), "Engine membership contains a cycle");
        result.push((*track, uuid.clone()));
        next = *id;
    }
    ensure!(
        result.len() == rows.len(),
        "Engine direct membership is incomplete"
    );
    Ok(result)
}
pub(super) fn write_members(db: &Connection, list: i64, members: &[(i64, String)]) -> Result<()> {
    db.execute(
        "DELETE FROM PlaylistEntity WHERE listId=?1 AND COALESCE(membershipReference,0)=0",
        [list],
    )?;
    let mut prior = None;
    let mut seen = BTreeSet::new();
    for (track, uuid) in members {
        if !seen.insert((track, uuid)) {
            continue;
        }
        db.execute("INSERT INTO PlaylistEntity(listId,trackId,databaseUuid,nextEntityId,membershipReference) VALUES(?1,?2,?3,0,0)",params![list,track,uuid])?;
        let id = db.last_insert_rowid();
        if let Some(prior) = prior {
            db.execute(
                "UPDATE PlaylistEntity SET nextEntityId=?1 WHERE id=?2",
                params![id, prior],
            )?;
        }
        prior = Some(id);
    }
    Ok(())
}
