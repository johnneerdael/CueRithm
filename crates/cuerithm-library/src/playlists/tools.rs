use super::*;
use std::collections::BTreeMap;

struct ContextData {
    tree: Tree,
    known: BTreeSet<String>,
    stamp: SmartStamp,
    snapshot: Option<snapshot::Snapshot>,
    smart: BTreeMap<String, evaluate::Outcome>,
    created: i64,
}
fn selected(tree: &Tree, ids: &[String]) -> Result<Vec<String>> {
    ensure!(
        !ids.is_empty() && ids.len() <= 5000,
        "Select 1–5000 playlists"
    );
    ensure!(
        ids.iter().collect::<BTreeSet<_>>().len() == ids.len(),
        "Duplicate selected playlist"
    );
    for id in ids {
        tree::active(tree, id)?;
    }
    Ok(ids.to_vec())
}
fn tree_selected(tree: &Tree, ids: &[String]) -> Result<Vec<String>> {
    let wanted = selected(tree, ids)?.into_iter().collect::<BTreeSet<_>>();
    let mut roots = tree::children(tree, &None, true);
    roots.reverse();
    let mut result = vec![];
    while let Some(id) = roots.pop() {
        if wanted.contains(&id) {
            result.push(id.clone());
        }
        let mut children = tree::children(tree, &Some(id), true);
        children.reverse();
        roots.extend(children);
    }
    ensure!(
        result.len() == wanted.len(),
        "Selected playlist is not in the tree"
    );
    Ok(result)
}
fn prefix_name(name: &str, index: usize, count: usize, prefix: &PlaylistPrefix) -> Result<String> {
    ensure!(
        count > 0 && count <= 5000 && index < count,
        "Invalid prefix selection"
    );
    let value = match prefix {
        PlaylistPrefix::Text { text } => {
            ensure!(
                !text.is_empty() && text.chars().count() <= 255,
                "Enter a prefix of 1–255 characters"
            );
            format!("{text}{name}")
        }
        PlaylistPrefix::Number {
            start,
            leading_zero,
            replace_existing,
        } => {
            let last = u64::from(*start) + count as u64 - 1;
            let width = 2.max(last.to_string().len());
            let mut number = (u64::from(*start) + index as u64).to_string();
            if *leading_zero && number.len() < width {
                number = format!("{}{}", "0".repeat(width - number.len()), number);
            }
            let mut base = name;
            if *replace_existing {
                let bytes = name.as_bytes();
                let digits = bytes
                    .iter()
                    .take(3)
                    .take_while(|b| b.is_ascii_digit())
                    .count();
                if digits > 0 {
                    let mut end = digits;
                    if bytes.get(end) == Some(&b'.') {
                        end += 1;
                    }
                    while bytes.get(end) == Some(&b' ') {
                        end += 1;
                    }
                    if bytes.get(end) == Some(&b'-') {
                        end += 1;
                    }
                    while bytes.get(end) == Some(&b' ') {
                        end += 1;
                    }
                    if end > digits {
                        base = &name[end..];
                    } else if digits == bytes.len() {
                        base = "";
                    }
                }
            }
            format!("{number} {base}")
        }
    };
    Ok(cuerithm_smartlists::javascript_trim(&value).into())
}
fn has_smart(tree: &Tree, id: &str) -> Result<bool> {
    tree::active(tree, id)?;
    Ok(evaluate::ordered_descendants(tree, id)
        .iter()
        .any(|id| tree[id].node.kind == PlaylistKind::Smartlist))
}
fn siblings(
    tree: &Tree,
    parent: &Option<String>,
    ids: &Option<Vec<String>>,
) -> Result<Vec<String>> {
    if let Some(id) = parent {
        ensure!(
            tree::active(tree, id)?.node.kind == PlaylistKind::Folder,
            "Parent is not a folder"
        );
    }
    let all = tree::children(tree, parent, true);
    let chosen = ids.as_ref().unwrap_or(&all);
    if chosen.is_empty() {
        return Ok(vec![]);
    }
    let chosen = selected(tree, chosen)?.into_iter().collect::<BTreeSet<_>>();
    ensure!(
        chosen.iter().all(|id| tree[id].node.parent_id == *parent),
        "Select sibling playlists"
    );
    Ok(all.into_iter().filter(|id| chosen.contains(id)).collect())
}
impl ContextData {
    fn load(lib: &Library, request: &PlaylistToolRequest) -> Result<Self> {
        let mut conn = lib.conn()?;
        let tx = conn.transaction()?;
        let tree = load(&tx)?;
        let stamp = preferences::stamp(&tx)?;
        let known = tree::known_tracks(&tx)?;
        tree::validate(&tree, &known)?;
        tx.commit()?;
        drop(conn);
        let data = match request {
            PlaylistToolRequest::Combine {
                source_ids, mode, ..
            } => {
                let ids = selected(&tree, source_ids)?;
                matches!(mode, PlaylistCombineMode::AbsentFromAll)
                    || ids
                        .iter()
                        .map(|id| has_smart(&tree, id))
                        .collect::<Result<Vec<_>>>()?
                        .into_iter()
                        .any(|s| s)
            }
            PlaylistToolRequest::SortSiblings {
                parent_id,
                ids,
                order,
            } => {
                let ids = siblings(&tree, parent_id, ids)?;
                matches!(order, PlaylistSortOrder::TrackCountDescending)
                    && ids
                        .iter()
                        .any(|id| tree[id].node.kind == PlaylistKind::Smartlist)
            }
            PlaylistToolRequest::Snapshot { source_id, .. } => has_smart(&tree, source_id)?,
            _ => false,
        };
        let created = crate::now();
        if data {
            let snapshot = lib.smart_snapshot(created, None, true)?;
            let smart = evaluate::computed_memberships(&snapshot)?;
            Ok(Self {
                tree: snapshot.tree.clone(),
                known: snapshot.all_ids.clone(),
                stamp: snapshot.field_catalog.stamp.clone(),
                snapshot: Some(snapshot),
                smart,
                created,
            })
        } else {
            Ok(Self {
                tree,
                known,
                stamp,
                snapshot: None,
                smart: BTreeMap::new(),
                created,
            })
        }
    }
    fn members(&self, id: &str, expires: &mut Option<i64>) -> Result<Vec<String>> {
        let row = tree::active(&self.tree, id)?;
        let ids = if row.node.kind == PlaylistKind::Folder {
            evaluate::ordered_descendants(&self.tree, id)
        } else {
            vec![id.into()]
        };
        let mut result = vec![];
        let mut seen = BTreeSet::new();
        for id in ids {
            let row = &self.tree[&id];
            let members = match row.node.kind {
                PlaylistKind::Folder => continue,
                PlaylistKind::Playlist => &row.members,
                PlaylistKind::Smartlist => {
                    let outcome = self
                        .smart
                        .get(&id)
                        .context("Smart membership snapshot unavailable")?;
                    if let Some(next) = outcome.next {
                        *expires = Some(expires.map_or(next, |old| old.min(next)));
                    }
                    outcome.members.as_ref().with_context(|| {
                        format!(
                            "Smart playlist {} unavailable: {}",
                            row.node.name,
                            outcome.error.as_deref().unwrap_or("incomplete result")
                        )
                    })?
                }
            };
            for id in members {
                if seen.insert(id.clone()) {
                    result.push(id.clone());
                }
            }
        }
        Ok(result)
    }
    fn guard(&self, expires: Option<i64>) -> previews::Guard {
        previews::Guard {
            stamp: self.stamp.clone(),
            data_bound: self.snapshot.is_some(),
            zone: self.snapshot.as_ref().map(|s| s.zone.clone()),
            observations: self
                .snapshot
                .as_ref()
                .map(|s| s.observations.clone())
                .unwrap_or_default(),
            created: self.created,
            expires,
            engine: env!("CUERITHM_SMART_ENGINE_ID").into(),
        }
    }
}
fn insert(
    tree: &mut Tree,
    parent: Option<String>,
    kind: PlaylistKind,
    name: String,
    members: Vec<String>,
    created: &mut Vec<String>,
) -> Result<String> {
    if let Some(p) = &parent {
        ensure!(
            tree::active(tree, p)?.node.kind == PlaylistKind::Folder,
            "Parent is not a folder"
        );
    }
    ensure!(
        created.len() < 5000,
        "A playlist tool can create at most 5000 nodes"
    );
    let id = uuid::Uuid::new_v4().to_string();
    let position = tree::children(tree, &parent, true).len() as u32;
    tree.insert(
        id.clone(),
        StoredNode {
            last_edit_id: String::new(),
            members,
            node: PlaylistNode {
                id: id.clone(),
                parent_id: parent,
                kind,
                name,
                position,
                favorite_position: None,
                definition: None,
                deleted: false,
                deleted_by: None,
            },
        },
    );
    created.push(id.clone());
    Ok(id)
}
impl Library {
    pub fn preview_playlist_tool(
        &self,
        request: PlaylistToolRequest,
    ) -> Result<PlaylistToolPreview> {
        let context = ContextData::load(self, &request)?;
        let before = &context.tree;
        let mut after = before.clone();
        let mut created = vec![];
        let mut expires = None;
        match request {
            PlaylistToolRequest::Combine {
                source_ids,
                parent_id,
                name,
                mode,
            } => {
                let mut ids = selected(before, &source_ids)?;
                ids.sort_by_key(|id| before[id].node.position);
                let sources = ids
                    .iter()
                    .map(|id| context.members(id, &mut expires))
                    .collect::<Result<Vec<_>>>()?;
                let members = match mode {
                    PlaylistCombineMode::Union => {
                        let mut seen = BTreeSet::new();
                        sources
                            .iter()
                            .flatten()
                            .filter(|id| seen.insert((*id).clone()))
                            .cloned()
                            .collect()
                    }
                    PlaylistCombineMode::Intersection => {
                        let indexes: Vec<BTreeSet<&str>> = sources
                            .iter()
                            .skip(1)
                            .map(|s| s.iter().map(String::as_str).collect())
                            .collect();
                        sources[0]
                            .iter()
                            .filter(|id| indexes.iter().all(|s| s.contains(id.as_str())))
                            .cloned()
                            .collect()
                    }
                    PlaylistCombineMode::AbsentFromAll => {
                        let excluded: BTreeSet<_> = sources.iter().flatten().collect();
                        context
                            .snapshot
                            .as_ref()
                            .context("Library snapshot unavailable")?
                            .tracks
                            .iter()
                            .filter(|r| !r.track.archived && !excluded.contains(&r.track.id))
                            .map(|r| r.track.id.clone())
                            .collect()
                    }
                };
                insert(
                    &mut after,
                    parent_id,
                    PlaylistKind::Playlist,
                    name,
                    members,
                    &mut created,
                )?;
            }
            PlaylistToolRequest::SortSiblings {
                parent_id,
                ids,
                order,
            } => {
                let original = siblings(before, &parent_id, &ids)?;
                let mut sorted = original.clone();
                match order {
                    PlaylistSortOrder::NameAscending | PlaylistSortOrder::NameDescending => {
                        sorted.sort_by_cached_key(|id| {
                            cuerithm_smartlists::javascript_trim(&before[id].node.name)
                                .to_lowercase()
                                .encode_utf16()
                                .collect::<Vec<_>>()
                        });
                        if matches!(order, PlaylistSortOrder::NameDescending) {
                            sorted.reverse();
                        }
                    }
                    PlaylistSortOrder::TrackCountDescending => {
                        let counts = original
                            .iter()
                            .map(|id| {
                                Ok((
                                    id.clone(),
                                    match before[id].node.kind {
                                        PlaylistKind::Folder => 0,
                                        PlaylistKind::Playlist => before[id].members.len(),
                                        PlaylistKind::Smartlist => {
                                            context.members(id, &mut expires)?.len()
                                        }
                                    },
                                ))
                            })
                            .collect::<Result<BTreeMap<_, _>>>()?;
                        sorted.sort_by_key(|id| counts[id]);
                        sorted.reverse();
                    }
                }
                for (old, new) in original.iter().zip(sorted) {
                    after.get_mut(&new).unwrap().node.position = before[old].node.position;
                }
            }
            PlaylistToolRequest::ReorderMembers { id, track_ids } => {
                edits::apply(
                    &mut after,
                    &context.known,
                    PlaylistEdit::ReorderMembers { id, track_ids },
                    "preview",
                    &mut created,
                    0,
                    &mut 0,
                )?;
            }
            PlaylistToolRequest::Snapshot {
                source_id,
                parent_id,
                name,
            } => {
                tree::active(before, &source_id)?;
                let mut queue =
                    std::collections::VecDeque::from([(source_id, parent_id, Some(name))]);
                while let Some((old, parent, renamed)) = queue.pop_front() {
                    let row = &before[&old];
                    let kind = if row.node.kind == PlaylistKind::Folder {
                        PlaylistKind::Folder
                    } else {
                        PlaylistKind::Playlist
                    };
                    let members = if kind == PlaylistKind::Folder {
                        vec![]
                    } else {
                        context.members(&old, &mut expires)?
                    };
                    let new = insert(
                        &mut after,
                        parent,
                        kind,
                        renamed.unwrap_or_else(|| row.node.name.clone()),
                        members,
                        &mut created,
                    )?;
                    for child in tree::children(before, &Some(old), true) {
                        queue.push_back((child, Some(new.clone()), None));
                    }
                }
            }
            PlaylistToolRequest::Prefix { ids, prefix } => {
                let ids = tree_selected(before, &ids)?;
                for (index, id) in ids.iter().enumerate() {
                    after.get_mut(id).unwrap().node.name =
                        prefix_name(&before[id].node.name, index, ids.len(), &prefix)?;
                }
            }
            PlaylistToolRequest::Uppercase { ids } => {
                for id in selected(before, &ids)? {
                    after.get_mut(&id).unwrap().node.name = before[&id].node.name.to_uppercase();
                }
            }
        }
        tree::normalize(&mut after);
        tree::validate(&after, &context.known)?;
        previews::save(
            self,
            "tool",
            context.guard(expires),
            before,
            &after,
            created,
        )
    }
    pub fn apply_playlist_tool(&self, id: &str, expected_revision: u64) -> Result<PlaylistChange> {
        previews::apply(self, "tool", id, expected_revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_prefix_helpers_match_authored_references() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/playlist-tools/prefix-reference.json"
        ))
        .unwrap();
        let mut count = 0;
        for case in reference["numeric"].as_array().unwrap() {
            let prefix = PlaylistPrefix::Number {
                start: case["start"].as_u64().unwrap() as u32,
                leading_zero: case["pad"].as_bool().unwrap(),
                replace_existing: case["replace"].as_bool().unwrap(),
            };
            assert_eq!(
                prefix_name(
                    case["name"].as_str().unwrap(),
                    case["index"].as_u64().unwrap() as usize,
                    case["count"].as_u64().unwrap() as usize,
                    &prefix
                )
                .unwrap(),
                case["expected"].as_str().unwrap()
            );
            count += 1;
        }
        for case in reference["text"].as_array().unwrap() {
            let prefix = PlaylistPrefix::Text {
                text: case["prefix"].as_str().unwrap().into(),
            };
            assert_eq!(
                prefix_name(case["name"].as_str().unwrap(), 0, 1, &prefix).unwrap(),
                case["expected"].as_str().unwrap()
            );
            count += 1;
        }
        assert_eq!(count, 104);
    }
}
