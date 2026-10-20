use super::*;
use crate::ownership::{
    PlaylistBinding, PlaylistOwnership, PlaylistOwnershipReport, PlaylistOwnershipRequest,
    fingerprint,
};
use crate::serato4::owned::{self, Locator};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashSet};

#[derive(Clone, Serialize, Deserialize)]
struct Mapping {
    location: i64,
    external: i64,
    container: i64,
    space: i64,
    created: bool,
}
#[derive(Serialize, Deserialize)]
struct AggregateLocator {
    mappings: Vec<Mapping>,
}

/// Keep a separate ownership ledger for master.sqlite. Feed each returned report
/// into the next location's request when refreshing several staged locations.
pub fn refresh_master_owned(
    master: &Path,
    staged_location: &Path,
    original_location: &Path,
    request: &PlaylistOwnershipRequest,
    location_report: &PlaylistOwnershipReport,
) -> Result<PlaylistOwnershipReport> {
    let source = Connection::open_with_flags(staged_location, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    ensure!(
        owned::identity(&source)? == location_report.current.identity,
        "Serato location ownership report belongs to a different database"
    );
    drop(source);
    super::refresh_master_inner(
        master,
        staged_location,
        original_location,
        Some((request, location_report)),
    )
}

pub(super) struct State {
    identity: String,
    bindings: BTreeMap<String, PlaylistBinding>,
    maps: BTreeMap<String, Vec<Mapping>>,
    before: HashSet<i64>,
    created: HashSet<i64>,
    local: PlaylistOwnershipReport,
}
fn snapshot(conn: &Connection, maps: &[Mapping]) -> Result<serde_json::Value> {
    let mut values = Vec::new();
    for m in maps {
        let state = owned::state(conn, m.container)?
            .context("Bound Serato master container disappeared")?;
        let members=conn.prepare("SELECT lc.location_id,lc.external_container_id,ca.space_asset_id,ca.list_order FROM container_asset ca JOIN location_container lc ON ca.location_container_id=lc.id WHERE lc.container_id=? ORDER BY lc.location_id,lc.external_container_id,ca.list_order,ca.space_asset_id")?.query_map([m.container],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        let target:Option<i64>=conn.query_row("SELECT container_id FROM location_container WHERE location_id=? AND external_container_id=?",params![m.location,m.external],|r|r.get(0)).optional()?;
        ensure!(
            target == Some(m.container),
            "Bound Serato aggregate location mapping changed"
        );
        values.push(serde_json::json!({"location":m.location,"external":m.external,"state":state,"members":members}));
    }
    Ok(serde_json::json!(values))
}
impl State {
    pub(super) fn new(
        conn: &Connection,
        request: &PlaylistOwnershipRequest,
        local: &PlaylistOwnershipReport,
        _location: i64,
    ) -> Result<Self> {
        let identity = format!("{}:aggregate", owned::identity(conn)?);
        ensure!(
            request.previous.identity == identity
                || (request.previous.identity.is_empty() && request.previous.bindings.is_empty()),
            "Serato master identity changed; reconcile ownership"
        );
        let before = conn
            .prepare("SELECT id FROM container")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<HashSet<_>>>()?;
        let space: i64 = conn.query_row(
            "SELECT id FROM space WHERE name='Serato Library' COLLATE NOCASE",
            [],
            |r| r.get(0),
        )?;
        let mut bindings = BTreeMap::new();
        let mut maps = BTreeMap::new();
        let mut created = HashSet::new();
        for b in &request.previous.bindings {
            let loc: AggregateLocator = serde_json::from_str(&b.locator)
                .context("Invalid Serato aggregate ownership locator")?;
            ensure!(
                !loc.mappings.is_empty()
                    && loc
                        .mappings
                        .iter()
                        .all(|m| m.container > 0 && m.space == space),
                "Protected Serato aggregate container"
            );
            ensure!(
                fingerprint(&snapshot(conn, &loc.mappings)?)? == b.fingerprint,
                "Serato aggregate '{}' changed after the previous sync",
                b.source_id
            );
            for m in &loc.mappings {
                if m.created {
                    created.insert(m.container);
                }
            }
            ensure!(
                bindings.insert(b.source_id.clone(), b.clone()).is_none(),
                "Duplicate Serato aggregate source binding"
            );
            maps.insert(b.source_id.clone(), loc.mappings);
        }
        // Reserve exclusive bound names before final assignments so two source
        // crates can swap names without selecting each other's native row.
        let mut reserved = HashSet::new();
        for (source, entries) in &maps {
            if !local
                .current
                .bindings
                .iter()
                .any(|b| &b.source_id == source)
            {
                continue;
            }
            for m in entries {
                if m.location == _location && reserved.insert(m.container) {
                    let foreign:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM location_container WHERE container_id=? AND location_id<>?)",params![m.container,_location],|r|r.get(0))?;
                    if !foreign {
                        conn.execute(
                            "UPDATE container SET name=? WHERE id=?",
                            params![
                                format!("CueRithm staging {}", uuid::Uuid::new_v4()),
                                m.container
                            ],
                        )?;
                    }
                }
            }
        }
        Ok(Self {
            identity,
            bindings,
            maps,
            before,
            created,
            local: local.clone(),
        })
    }
    pub(super) fn preferred(
        &self,
        conn: &Connection,
        external: i64,
        parent: i64,
        name: &str,
        kind: i64,
    ) -> Result<Option<i64>> {
        let source = self
            .local
            .current
            .bindings
            .iter()
            .find(|b| {
                serde_json::from_str::<Locator>(&b.locator).is_ok_and(|l| l.container == external)
            })
            .map(|b| &b.source_id);
        let Some(entries) = source.and_then(|s| self.maps.get(s)) else {
            return Ok(None);
        };
        for entry in entries {
            let matches:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM container WHERE id=? AND parent_id=? AND name=? COLLATE NOCASE AND type=?)",params![entry.container,parent,name,kind],|r|r.get(0))?;
            if matches {
                return Ok(Some(entry.container));
            }
        }
        Ok(None)
    }
    pub(super) fn cleanup(&mut self, conn: &Connection, id: i64) -> Result<()> {
        if self.created.contains(&id) {
            super::remove_unreferenced_container(conn, id, super::has_rules_table(conn)?)?;
        }
        Ok(())
    }
    pub(super) fn finish(
        mut self,
        conn: &Connection,
        location: i64,
        space: i64,
        containers: &HashMap<i64, i64>,
    ) -> Result<PlaylistOwnershipReport> {
        let current: BTreeMap<_, _> = self
            .local
            .current
            .bindings
            .iter()
            .map(|b| (b.source_id.clone(), b))
            .collect();
        let mut candidates = BTreeSet::new();
        let mut removed_created = BTreeSet::new();
        let mut removed_external = BTreeSet::new();
        for (source, maps) in &mut self.maps {
            let old: Vec<_> = maps
                .iter()
                .filter(|m| m.location == location && !current.contains_key(source))
                .cloned()
                .collect();
            for m in old {
                // Retiring a source match keeps the authoritative native source
                // crate and its aggregate mapping. Only a proven local deletion
                // removes its location membership and mapping.
                if self.local.deleted.contains(source) {
                    ensure!(
                        !containers.contains_key(&m.external),
                        "Deleted Serato source container still exists"
                    );
                    conn.execute("DELETE FROM container_asset WHERE location_container_id IN(SELECT id FROM location_container WHERE location_id=? AND external_container_id=?)",params![location,m.external])?;
                    conn.execute("DELETE FROM location_container WHERE location_id=? AND external_container_id=?",params![location,m.external])?;
                    candidates.insert(m.container);
                    if m.created {
                        removed_created.insert(source.clone());
                    } else {
                        removed_external.insert(source.clone());
                    }
                } else {
                    removed_external.insert(source.clone());
                }
            }
            maps.retain(|m| m.location != location || current.contains_key(source));
        }
        // Mapping-first rename/move can split a formerly shared aggregate row.
        // Retain creation proof on every mapping which shares that native row.
        for (source, b) in &current {
            let local: Locator = serde_json::from_str(&b.locator)?;
            let target = *containers
                .get(&local.container)
                .context("Managed Serato crate has no aggregate mapping")?;
            let maps = self.maps.entry(source.clone()).or_default();
            for old in maps
                .iter()
                .filter(|m| m.location == location && m.container != target)
            {
                candidates.insert(old.container);
            }
            maps.retain(|m| m.location != location);
            let created = self.created.contains(&target) || !self.before.contains(&target);
            if created {
                self.created.insert(target);
            }
            maps.push(Mapping {
                location,
                external: local.container,
                container: target,
                space,
                created,
            });
            maps.sort_by_key(|m| (m.location, m.external));
            self.bindings
                .entry(source.clone())
                .or_insert_with(|| (*b).clone())
                .parent_source_id = b.parent_source_id.clone();
        }
        // A shared aggregate can outlive the location which originally created
        // it. Transfer that proof to all retained mappings before dropping it.
        for maps in self.maps.values_mut() {
            for m in maps {
                m.created =
                    self.created.contains(&m.container) || !self.before.contains(&m.container);
            }
        }
        // Children first; retain foreign children, aliases and other locations.
        loop {
            let mut removed = false;
            for id in &candidates {
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM container WHERE id=?)",
                    [id],
                    |r| r.get(0),
                )?;
                if exists {
                    self.cleanup(conn, *id)?;
                    let after: bool = conn.query_row(
                        "SELECT EXISTS(SELECT 1 FROM container WHERE id=?)",
                        [id],
                        |r| r.get(0),
                    )?;
                    removed |= !after;
                }
            }
            if !removed {
                break;
            }
        }
        let mut deleted = BTreeSet::new();
        let mut retired = BTreeSet::new();
        let mut output = Vec::new();
        for (source, maps) in self.maps {
            if maps.is_empty() {
                if removed_created.contains(&source) {
                    deleted.insert(source);
                } else if removed_external.contains(&source) {
                    retired.insert(source);
                }
                continue;
            }
            let mut b = self
                .bindings
                .remove(&source)
                .context("Missing Serato aggregate source binding")?;
            b.locator = serde_json::to_string(&AggregateLocator {
                mappings: maps.clone(),
            })?;
            b.created = maps.iter().any(|m| m.created);
            b.fingerprint = fingerprint(&snapshot(conn, &maps)?)?;
            output.push(b);
        }
        Ok(PlaylistOwnershipReport {
            current: PlaylistOwnership {
                identity: self.identity,
                bindings: output,
            },
            deleted,
            retired,
        })
    }
}
