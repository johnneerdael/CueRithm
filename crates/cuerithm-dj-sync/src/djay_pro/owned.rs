//! Source-to-UUID ownership evidence and guarded playlist-only removal.
use super::*;
use crate::ownership::{
    PlaylistBinding, PlaylistOwnership, PlaylistOwnershipReport, PlaylistOwnershipRequest,
    fingerprint,
};
use serde_json::{Value as Json, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const IDENTITY: &str = "djay-pro:MediaLibrary.db:database2:TSAF";

struct Graph {
    playlists: BTreeMap<String, Object>,
    links: BTreeMap<String, Object>,
}
impl Graph {
    fn load(tx: &Transaction<'_>) -> Result<Self> {
        Ok(Self {
            playlists: objects(tx, PLAYLISTS)?.into_iter().collect(),
            links: objects(tx, LINKS)?.into_iter().collect(),
        })
    }
    fn parents(&self, id: &str) -> Result<BTreeSet<String>> {
        self.playlists
            .iter()
            .filter_map(|(key, object)| match strings(object, "childUUIDs") {
                Ok(children) if children.iter().any(|child| child == id) => Some(Ok(key.clone())),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }
    fn children(&self, id: &str) -> Result<BTreeSet<String>> {
        let mut children = self
            .playlists
            .get(id)
            .map(|object| strings(object, "childUUIDs"))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .collect::<BTreeSet<_>>();
        for (key, object) in &self.playlists {
            if string(object, "parentUUID")? == Some(id) {
                children.insert(key.clone());
            }
        }
        Ok(children)
    }
    fn owned_links(&self, id: &str) -> Result<Vec<String>> {
        self.links
            .iter()
            .filter_map(|(key, object)| match string(object, "playlistUUID") {
                Ok(Some(owner)) if owner == id => Some(Ok(key.clone())),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }
    fn link_users(&self, id: &str) -> Result<Vec<String>> {
        self.playlists
            .iter()
            .filter_map(|(key, object)| match strings(object, "itemUUIDs") {
                Ok(items) if items.iter().any(|item| item == id) => Some(Ok(key.clone())),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }
    fn check_links(&self, id: &str) -> Result<()> {
        let Some(object) = self.playlists.get(id) else {
            return Ok(());
        };
        for key in strings(object, "itemUUIDs")? {
            let link = self
                .links
                .get(&key)
                .context("djay playlist references a missing link")?;
            validate_link(&key, link, id)?;
            ensure!(
                self.link_users(&key)?.iter().all(|user| user == id),
                "djay playlist link is also referenced by another playlist"
            );
        }
        Ok(())
    }
    fn profile(&self, id: &str) -> Result<Json> {
        let object = self
            .playlists
            .get(id)
            .context("Bound djay playlist is missing")?;
        ensure!(
            object.class == "ADCMediaItemPlaylist" && string(object, "uuid")? == Some(id),
            "djay playlist UUID/class does not match its native key"
        );
        self.check_links(id)?;
        let items = strings(object, "itemUUIDs")?;
        let mut links = Vec::new();
        for key in &items {
            let object = &self.links[key];
            validate_link(key, object, id)?;
            links.push(json!({"uuid":key,"playlist":id,"media":string(object,"mediaItemUUID")?,"users":self.link_users(key)?}));
        }
        let mut orphans = Vec::new();
        for key in self.owned_links(id)? {
            if !items.contains(&key) {
                let object = &self.links[&key];
                validate_link(&key, object, id)?;
                orphans.push(json!({"uuid":key,"media":string(object,"mediaItemUUID")?,"users":self.link_users(&key)?}));
            }
        }
        Ok(
            json!({"uuid":id,"class":object.class,"name":string(object,"name")?,
            "kind":kind(object)?,"parent":string(object,"parentUUID")?.unwrap_or(ROOT),
            "children":strings(object,"childUUIDs")?,"back_children":self.children(id)?,
            "parents":self.parents(id)?,"items":links,"orphan_links":orphans,
            "filter_set":object.get("filterSet").map(canonical).transpose()?}),
        )
    }
    fn hash(&self, id: &str) -> Result<String> {
        fingerprint(&self.profile(id)?)
    }
}

pub(super) struct Plan {
    previous: BTreeMap<String, PlaylistBinding>,
    touched: BTreeSet<String>,
    removed: BTreeSet<String>,
    deletes: BTreeSet<String>,
    before_present: BTreeSet<String>,
}
impl Plan {
    pub(super) fn prepare(
        tx: &Transaction<'_>,
        library: &SyncLibrary,
        request: &PlaylistOwnershipRequest,
    ) -> Result<Self> {
        ensure!(
            request.previous.identity.is_empty() || request.previous.identity == IDENTITY,
            "djay ownership target identity changed"
        );
        ensure!(
            !request.previous.identity.is_empty() || request.previous.bindings.is_empty(),
            "djay ownership evidence has no target identity"
        );
        let graph = Graph::load(tx)?;
        let mut previous = BTreeMap::new();
        let mut locators = BTreeSet::new();
        for binding in &request.previous.bindings {
            ensure!(
                !binding.source_id.is_empty()
                    && previous
                        .insert(binding.source_id.clone(), binding.clone())
                        .is_none(),
                "Duplicate djay source ownership identity"
            );
            ensure!(
                binding.locator == stable_id("playlist", &binding.source_id, true)
                    && binding.locator != ROOT
                    && locators.insert(binding.locator.clone()),
                "Invalid or duplicate djay ownership locator"
            );
            ensure!(
                !binding.fingerprint.is_empty(),
                "djay binding has no structural fingerprint"
            );
        }
        for binding in previous.values() {
            let mut parent = binding.parent_source_id.as_ref();
            let mut seen = BTreeSet::from([binding.source_id.as_str()]);
            while let Some(id) = parent {
                ensure!(seen.insert(id), "djay ownership parent cycle");
                let ancestor = previous
                    .get(id)
                    .context("djay ownership parent binding is missing")?;
                parent = ancestor.parent_source_id.as_ref();
            }
        }
        let active: BTreeSet<_> = library
            .playlists
            .iter()
            .map(|playlist| playlist.id.as_str())
            .collect();
        let active_parent: BTreeMap<_, _> = library
            .playlists
            .iter()
            .map(|playlist| {
                (
                    stable_id("playlist", &playlist.id, true),
                    playlist
                        .parent_id
                        .as_ref()
                        .map_or_else(|| ROOT.to_owned(), |id| stable_id("playlist", id, true)),
                )
            })
            .collect();
        let mut touched = active_parent.keys().cloned().collect::<BTreeSet<_>>();
        let mut deletes = BTreeSet::new();
        for source in &request.removed {
            ensure!(
                !active.contains(source.as_str()),
                "djay playlist is both active and removed"
            );
            let binding = previous
                .get(source)
                .context("djay removal has no acknowledged ownership binding")?;
            if binding.created {
                deletes.insert(binding.locator.clone());
                touched.insert(binding.locator.clone());
            }
        }
        for id in touched.clone() {
            let expected = if let Some(object) = graph.playlists.get(&id) {
                string(object, "parentUUID")?.unwrap_or(ROOT).to_owned()
            } else if let Some((_, binding)) =
                previous.iter().find(|(_, binding)| binding.locator == id)
            {
                binding.parent_source_id.as_ref().map_or_else(
                    || ROOT.to_owned(),
                    |parent| stable_id("playlist", parent, true),
                )
            } else {
                active_parent
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| ROOT.to_owned())
            };
            let references = graph.parents(&id)?;
            ensure!(
                references.iter().all(|parent| parent == &expected),
                "djay playlist has an unplanned parent reference"
            );
            touched.insert(expected);
            touched.extend(references);
        }
        touched.extend(active_parent.values().cloned());
        for binding in previous
            .values()
            .filter(|binding| touched.contains(&binding.locator))
        {
            if graph.playlists.contains_key(&binding.locator) {
                ensure!(
                    graph.hash(&binding.locator)? == binding.fingerprint,
                    "Bound djay playlist changed since ownership acknowledgement: {}",
                    binding.source_id
                );
            } else {
                ensure!(
                    request.removed.contains(&binding.source_id) && binding.created,
                    "Bound djay playlist is missing: {}",
                    binding.source_id
                );
                ensure!(
                    graph.children(&binding.locator)?.is_empty()
                        && graph.owned_links(&binding.locator)?.is_empty(),
                    "Missing djay playlist still has protected child/link references"
                );
            }
        }
        for playlist in &library.playlists {
            let id = stable_id("playlist", &playlist.id, true);
            if graph.playlists.contains_key(&id) {
                graph.profile(&id)?;
            }
            graph.check_links(&id)?;
            for index in 0..playlist.track_ids.len() {
                let key = stable_id("playlist-item", &format!("{}:{index}", playlist.id), true);
                if let Some(object) = graph.links.get(&key) {
                    validate_link(&key, object, &id)?;
                    ensure!(
                        graph.link_users(&key)?.iter().all(|user| user == &id),
                        "Planned djay link belongs to another playlist"
                    );
                }
            }
            if !playlist.folder {
                for child in graph.children(&id)? {
                    ensure!(
                        deletes.contains(&child)
                            || active_parent
                                .get(&child)
                                .is_some_and(|parent| parent != &id),
                        "djay folder has a protected child and cannot become a leaf"
                    );
                }
            }
        }
        for id in &deletes {
            graph.check_links(id)?;
            for child in graph.children(id)? {
                ensure!(
                    deletes.contains(&child)
                        || active_parent.get(&child).is_some_and(|parent| parent != id),
                    "Owned djay folder contains an unowned or unplanned child"
                );
            }
            if let Some(object) = graph.playlists.get(id) {
                let items = strings(object, "itemUUIDs")?;
                ensure!(
                    graph
                        .owned_links(id)?
                        .iter()
                        .all(|link| items.contains(link)),
                    "Owned djay playlist has unplanned link backreferences"
                );
            }
        }
        Ok(Self {
            previous,
            touched,
            removed: request.removed.clone(),
            deletes,
            before_present: graph.playlists.keys().cloned().collect(),
        })
    }
    pub(super) fn remove(&self, tx: &Transaction<'_>) -> Result<bool> {
        let graph = Graph::load(tx)?;
        let mut ordered = self
            .deletes
            .iter()
            .map(|id| Ok((depth(&graph, id)?, id.clone())))
            .collect::<Result<Vec<_>>>()?;
        ordered.sort_by(|a, b| b.cmp(a));
        let mut changed = false;
        for (_, id) in ordered {
            if let Some(object) = graph.playlists.get(&id) {
                graph.check_links(&id)?;
                for key in strings(object, "itemUUIDs")? {
                    changed |= tx.execute(
                        "DELETE FROM database2 WHERE collection=?1 AND key=?2",
                        params![LINKS, key],
                    )? != 0;
                }
                changed |= tx.execute(
                    "DELETE FROM database2 WHERE collection=?1 AND key=?2",
                    params![PLAYLISTS, id],
                )? != 0;
            }
        }
        for (id, mut object) in graph.playlists {
            if self.deletes.contains(&id) {
                continue;
            }
            let before = strings(&object, "childUUIDs")?;
            let after = before
                .iter()
                .filter(|child| !self.deletes.contains(*child))
                .map(|child| text(child))
                .collect::<Vec<_>>();
            if after.len() != before.len() {
                object.set("childUUIDs", Value::Array(after));
                upsert(tx, PLAYLISTS, &id, &object)?;
                changed = true;
            }
        }
        Ok(changed)
    }
    pub(super) fn report(
        self,
        tx: &Transaction<'_>,
        library: &SyncLibrary,
    ) -> Result<PlaylistOwnershipReport> {
        let graph = Graph::load(tx)?;
        let mut bindings = self.previous;
        let mut deleted = BTreeSet::new();
        let mut retired = BTreeSet::new();
        for source in self.removed {
            let binding = bindings
                .remove(&source)
                .context("Missing djay ownership removal")?;
            if binding.created {
                deleted.insert(source);
            } else {
                retired.insert(source);
            }
        }
        for binding in bindings
            .values_mut()
            .filter(|binding| self.touched.contains(&binding.locator))
        {
            binding.fingerprint = graph.hash(&binding.locator)?;
        }
        for playlist in &library.playlists {
            let id = stable_id("playlist", &playlist.id, true);
            let created = bindings
                .get(&playlist.id)
                .map_or(!self.before_present.contains(&id), |binding| {
                    binding.created
                });
            bindings.insert(
                playlist.id.clone(),
                PlaylistBinding {
                    source_id: playlist.id.clone(),
                    locator: id.clone(),
                    parent_source_id: playlist.parent_id.clone(),
                    created,
                    fingerprint: graph.hash(&id)?,
                },
            );
        }
        Ok(PlaylistOwnershipReport {
            current: PlaylistOwnership {
                identity: IDENTITY.into(),
                bindings: bindings.into_values().collect(),
            },
            deleted,
            retired,
        })
    }
}
fn depth(graph: &Graph, id: &str) -> Result<usize> {
    let mut current = id;
    let mut seen = BTreeSet::new();
    let mut depth = 0;
    while let Some(object) = graph.playlists.get(current) {
        ensure!(seen.insert(current), "djay native parent cycle");
        let Some(parent) = string(object, "parentUUID")? else {
            break;
        };
        if parent == ROOT {
            break;
        }
        current = parent;
        depth += 1;
    }
    Ok(depth)
}
fn string<'a>(object: &'a Object, field: &str) -> Result<Option<&'a str>> {
    match object.get(field) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => bail!("Invalid djay playlist field {field}"),
    }
}
fn strings(object: &Object, field: &str) -> Result<Vec<String>> {
    match object.get(field) {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => {
            let mut seen = BTreeSet::new();
            values
                .iter()
                .map(|value| {
                    let Value::String(value) = value else {
                        bail!("Invalid djay playlist array {field}")
                    };
                    ensure!(
                        seen.insert(value),
                        "Duplicate djay playlist reference in {field}"
                    );
                    Ok(value.clone())
                })
                .collect()
        }
        _ => bail!("Invalid djay playlist array {field}"),
    }
}
fn kind(object: &Object) -> Result<u8> {
    match object.get("type") {
        None => Ok(u8::from(object.get("childUUIDs").is_some())),
        Some(Value::Small(value)) => Ok(*value),
        Some(Value::U16(value)) => Ok(u8::try_from(*value)?),
        Some(Value::Double(value))
            if value.is_finite() && value.fract() == 0.0 && (0.0..=2.0).contains(value) =>
        {
            Ok(*value as u8)
        }
        _ => bail!("Invalid djay playlist type"),
    }
}
fn validate_link(key: &str, link: &Object, owner: &str) -> Result<()> {
    ensure!(
        link.class == "ADCMediaItemPlaylistItem"
            && string(link, "uuid")? == Some(key)
            && string(link, "playlistUUID")? == Some(owner),
        "djay link identity/owner mismatch"
    );
    ensure!(
        string(link, "mediaItemUUID")?.is_some_and(|media| !media.is_empty()),
        "djay link has no media identity"
    );
    Ok(())
}
fn canonical(value: &Value) -> Result<Json> {
    Ok(match value {
        Value::String(value) | Value::Uri(value) => json!(value),
        Value::Null => Json::Null,
        Value::Bool(value) => json!(value),
        Value::Small(value) => json!(f64::from(*value)),
        Value::U16(value) => json!(f64::from(*value)),
        Value::U32(value) => json!(f64::from(*value)),
        Value::U64(value) => json!(value),
        Value::Double(value) | Value::Date(value) => {
            ensure!(value.is_finite(), "Nonfinite djay rule value");
            json!(value)
        }
        Value::Float(value) => {
            ensure!(value.is_finite(), "Nonfinite djay rule value");
            json!(f64::from(*value))
        }
        Value::Binary(value) => json!({"binary":value}),
        Value::Array(values) => Json::Array(values.iter().map(canonical).collect::<Result<_>>()?),
        Value::Class(object) => {
            json!({"class":object.class,"fields":object.fields.iter().map(|(key,value)|Ok((key.clone(),canonical(value)?))).collect::<Result<BTreeMap<_,_>>>()?})
        }
        Value::Fields(fields) => json!(
            fields
                .iter()
                .map(|(key, value)| Ok((key.clone(), canonical(value)?)))
                .collect::<Result<BTreeMap<_, _>>>()?
        ),
    })
}
