//! Fresh read-only membership snapshots for long-running library operations.
use super::*;
use cuerithm_media::CancellationToken;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScopeGuard {
    pub stamp: SmartStamp,
    zone: Option<String>,
    observations: Vec<snapshot::Observation>,
    created: i64,
    expires: Option<i64>,
}
pub(crate) fn stamp(conn: &Connection) -> Result<SmartStamp> {
    preferences::stamp(conn)
}
impl ScopeGuard {
    #[cfg(feature = "autotag")]
    pub(crate) fn time_current(&self, now: i64) -> bool {
        now >= self.created && self.expires.is_none_or(|end| now < end)
    }
    pub fn current(&self, lib: &Library, cancel: &CancellationToken) -> Result<bool> {
        cancel.check()?;
        let current = {
            let conn = lib.conn()?;
            let stamp = preferences::stamp(&conn)?;
            let prefs = preferences::preferences(&conn)?;
            (stamp, prefs.preferences)
        };
        let now = crate::now();
        if current.0 != self.stamp
            || now < self.created
            || self.expires.is_some_and(|end| now >= end)
            || self
                .zone
                .as_ref()
                .is_some_and(|old| preferences::zone(&current.1).as_ref().ok() != Some(old))
        {
            return Ok(false);
        }
        for observed in &self.observations {
            cancel.check()?;
            if snapshot::observe(&observed.path) != *observed {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
pub(crate) fn capture(
    lib: &Library,
    ids: &[String],
    cancel: &CancellationToken,
) -> Result<(ScopeGuard, BTreeSet<String>)> {
    cancel.check()?;
    ensure!(
        ids.len() <= 5000 && ids.iter().collect::<BTreeSet<_>>().len() == ids.len(),
        "Invalid selected playlists"
    );
    let (tree, stamp) = {
        let mut conn = lib.conn()?;
        let tx = conn.transaction()?;
        let value = (load(&tx)?, preferences::stamp(&tx)?);
        tx.commit()?;
        value
    };
    let mut descendants = BTreeSet::new();
    for id in ids {
        let root = tree::active(&tree, id)?;
        if root.node.kind == PlaylistKind::Folder {
            descendants.extend(evaluate::ordered_descendants(&tree, id));
        } else {
            descendants.insert(id.clone());
        }
    }
    let has_smart = descendants
        .iter()
        .any(|id| tree[id].node.kind == PlaylistKind::Smartlist);
    let created = crate::now();
    let mut guard = ScopeGuard {
        stamp,
        created,
        expires: None,
        zone: None,
        observations: vec![],
    };
    let mut members = BTreeSet::new();
    if has_smart {
        let snapshot =
            lib.smart_snapshot_cancellable(created, None, true, Some(cancel.as_atomic()))?;
        ensure!(
            snapshot.field_catalog.stamp == guard.stamp,
            "Library changed while capturing scope; scan again"
        );
        let outcomes =
            evaluate::compute(&snapshot, &snapshot.tree, None, Some(cancel.as_atomic()))?;
        for id in descendants {
            cancel.check()?;
            let row = &snapshot.tree[&id];
            match row.node.kind {
                PlaylistKind::Folder => {}
                PlaylistKind::Playlist => members.extend(row.members.iter().cloned()),
                PlaylistKind::Smartlist => {
                    let outcome = outcomes
                        .iter()
                        .find(|o| o.id == id)
                        .context("Missing smart scope result")?;
                    members.extend(
                        outcome
                            .members
                            .as_ref()
                            .with_context(|| {
                                format!(
                                    "Smart scope unavailable: {}",
                                    outcome.error.as_deref().unwrap_or("incomplete")
                                )
                            })?
                            .iter()
                            .cloned(),
                    );
                    if let Some(next) = outcome.next {
                        guard.expires = Some(guard.expires.map_or(next, |old| old.min(next)));
                    }
                }
            }
        }
        guard.zone = Some(snapshot.zone);
        guard.observations = snapshot.observations;
    } else {
        for id in descendants {
            cancel.check()?;
            if tree[&id].node.kind == PlaylistKind::Playlist {
                members.extend(tree[&id].members.iter().cloned());
            }
        }
    }
    Ok((guard, members))
}
