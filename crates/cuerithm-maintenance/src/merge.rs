use crate::{ReviewGroup, ReviewState};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagCopy {
    pub from: String,
    pub to: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupMerge {
    pub group_id: String,
    pub mode: ReviewState,
    pub exact_audio: bool,
    pub keeper: String,
    pub archive_ids: Vec<String>,
    pub unarchive_keeper: bool,
    pub copies: Vec<TagCopy>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MergePlan {
    pub groups: Vec<GroupMerge>,
}
impl MergePlan {
    pub fn build(
        groups: &[ReviewGroup],
        selected: &[String],
        archived: &BTreeSet<String>,
    ) -> Result<Self> {
        ensure!(
            !selected.is_empty()
                && selected.iter().collect::<BTreeSet<_>>().len() == selected.len(),
            "Invalid duplicate group selection"
        );
        let index = groups
            .iter()
            .map(|g| (&g.id, g))
            .collect::<BTreeMap<_, _>>();
        ensure!(
            index.len() == groups.len(),
            "Duplicate review group identity"
        );
        let mut result = vec![];
        let mut used = BTreeSet::new();
        for id in selected {
            let group = index
                .get(id)
                .context("Group is not in this duplicate review")?;
            ensure!(
                group.ready(),
                "Duplicate group is locked, unavailable or ignored"
            );
            for track in &group.tracks {
                ensure!(
                    used.insert(track.candidate.id.clone()),
                    "Duplicate groups overlap"
                );
            }
            let keeper = group.keeper.clone().context("Missing duplicate keeper")?;
            let eligible = group
                .tracks
                .iter()
                .filter(|t| !group.ignored.contains(&t.candidate.id));
            let copies = if let Some(from) = &group.tags_from {
                if group.state == ReviewState::CopyCues {
                    eligible
                        .clone()
                        .filter(|t| &t.candidate.id != from)
                        .map(|t| TagCopy {
                            from: from.clone(),
                            to: t.candidate.id.clone(),
                        })
                        .collect()
                } else {
                    vec![TagCopy {
                        from: from.clone(),
                        to: keeper.clone(),
                    }]
                }
            } else {
                vec![]
            };
            let archive_ids: Vec<_> = if group.state == ReviewState::Unlocked {
                eligible
                    .filter(|t| t.candidate.id != keeper)
                    .map(|t| t.candidate.id.clone())
                    .collect()
            } else {
                vec![]
            };
            let unarchive_keeper = !archive_ids.is_empty() && archived.contains(&keeper);
            result.push(GroupMerge {
                group_id: id.clone(),
                mode: group.state,
                exact_audio: group.exact_audio,
                keeper,
                archive_ids,
                unarchive_keeper,
                copies,
            });
        }
        Ok(Self { groups: result })
    }
}
/// Keep the first replaced slot, removing a prior keeper entry in affected lists.
/// Unaffected lists retain their existing keeper position.
pub fn replace_members(
    members: &[String],
    removed: &[String],
    keeper: &str,
) -> Result<Vec<String>> {
    let removed = removed.iter().collect::<BTreeSet<_>>();
    ensure!(
        !keeper.is_empty() && !removed.iter().any(|id| id.as_str() == keeper),
        "Keeper is also being replaced"
    );
    ensure!(
        members.iter().collect::<BTreeSet<_>>().len() == members.len(),
        "Repeated playlist member"
    );
    if !members.iter().any(|id| removed.contains(id)) {
        return Ok(members.to_vec());
    }
    let mut inserted = false;
    let mut result = vec![];
    for id in members {
        if removed.contains(id) {
            if !inserted {
                result.push(keeper.into());
                inserted = true;
            }
        } else if id != keeper {
            result.push(id.clone());
        }
    }
    Ok(result)
}
