//! Read-only reconciliation of Cuerithm tracks against the recorded rekordbox observations.
//!
//! This decides, per field, what a future synchronization would have to do. It performs no write
//! to a target database or to the library, and a preview is never an acknowledgement.
use crate::native_sync::{NativeFieldBaseline, NativeFieldOwner, local_fields, remote_fields};
use crate::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// What a synchronization would have to do for one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldDecision {
    /// Both sides still match what was last recorded.
    Unchanged,
    /// Both sides now hold the same value; nothing to write.
    Converged,
    /// Only Cuerithm changed (or owns the initial value): write it to rekordbox.
    PushLocal,
    /// Only rekordbox changed (or owns the initial value): bring it into Cuerithm.
    PullRemote,
    /// Both sides changed to different values, or one side changed a field the other owns.
    Conflict,
    /// The two sides differ and nobody owns the field yet.
    NeedsOwner,
    /// Cuerithm has no value; replacing rekordbox's value with nothing needs an explicit choice.
    EmptyLocal,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDiff {
    pub field: String,
    pub decision: FieldDecision,
    pub local: Value,
    pub remote: Value,
    pub baseline_local: Value,
    pub baseline_remote: Value,
    pub owner: NativeFieldOwner,
    pub acknowledged: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackPreview {
    pub content_id: String,
    pub local_track_id: Option<String>,
    pub status: String,
    /// Why this track cannot be reconciled, such as a changed identity or missing local track.
    pub blocked: Option<String>,
    /// Only fields that need attention; unchanged fields are counted in the summary.
    pub fields: Vec<FieldDiff>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeSyncPreview {
    pub target_id: String,
    pub revision: u64,
    pub tracks: Vec<TrackPreview>,
    /// Counts by decision name across every compared field, plus `blocked_tracks`.
    pub summary: BTreeMap<String, usize>,
    /// Direct writes stay off until a schema is verified against the real target application.
    pub writes_enabled: bool,
    pub write_blocker: String,
}

fn empty(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Array(items) => items.iter().all(empty),
        Value::String(text) => text.is_empty(),
        _ => false,
    }
}
fn same(a: &Value, b: &Value) -> bool {
    // A missing value and an empty one describe the same thing.
    a == b || (empty(a) && empty(b))
}

/// The decision for one field from its recorded baseline and the current values on both sides.
pub fn decide(base: &NativeFieldBaseline, local: &Value, remote: &Value) -> FieldDecision {
    if same(local, remote) {
        return if same(local, &base.local) && same(remote, &base.remote) {
            FieldDecision::Unchanged
        } else {
            FieldDecision::Converged
        };
    }
    if base.owner == NativeFieldOwner::Unowned {
        return FieldDecision::NeedsOwner;
    }
    let local_changed = !same(local, &base.local);
    let remote_changed = !same(remote, &base.remote);
    let direction = if base.acknowledged {
        match (local_changed, remote_changed) {
            (true, false) => FieldDecision::PushLocal,
            (false, true) => FieldDecision::PullRemote,
            // Both moved apart, or the recorded agreement no longer describes either side.
            _ => return FieldDecision::Conflict,
        }
    } else {
        // An initial observation has no agreed baseline, so ownership alone picks the direction.
        match base.owner {
            NativeFieldOwner::Local => FieldDecision::PushLocal,
            NativeFieldOwner::Remote => FieldDecision::PullRemote,
            _ => return FieldDecision::Conflict,
        }
    };
    match (direction, base.owner) {
        // Editing a field in the tool that does not own it is a conflict, not an override.
        (FieldDecision::PushLocal, NativeFieldOwner::Remote)
        | (FieldDecision::PullRemote, NativeFieldOwner::Local) => FieldDecision::Conflict,
        (FieldDecision::PushLocal, _) if empty(local) => FieldDecision::EmptyLocal,
        (direction, _) => direction,
    }
}

impl Library {
    /// Compare every linked track with its latest rekordbox observation. Read-only.
    pub fn native_sync_preview(&self, target: &str) -> Result<NativeSyncPreview> {
        let state = self.native_target(target)?;
        let conn = self.conn()?;
        let links = crate::native_sync::links(&conn, target)?;
        let mut tracks = vec![];
        let mut summary: BTreeMap<String, usize> = BTreeMap::new();
        for (content_id, link) in links {
            let observation = conn
                .query_row(
                    "SELECT observation_json FROM native_track_links WHERE target_id=?1 AND content_id=?2",
                    rusqlite::params![target, content_id],
                    |r| r.get::<_, Option<String>>(0),
                )?
                .map(|text| serde_json::from_str::<cuerithm_rekordbox::ImportedTrack>(&text))
                .transpose()?;
            let blocked = match (&link.local, &observation, link.status.as_str()) {
                (_, _, status) if status != "linked" => Some(status.to_owned()),
                (None, _, _) => Some("no_local_track".to_owned()),
                (_, None, _) => Some("no_observation".to_owned()),
                _ => None,
            };
            let mut preview = TrackPreview {
                content_id,
                local_track_id: link.local.clone(),
                status: link.status.clone(),
                blocked,
                fields: vec![],
            };
            if preview.blocked.is_some() {
                *summary.entry("blocked_tracks".into()).or_default() += 1;
            } else if let (Some(local_id), Some(observation)) = (&link.local, &observation) {
                let local = local_fields(&conn, local_id)?;
                let remote = remote_fields(observation)?;
                for (field, base) in &link.fields {
                    let l = local.get(field).cloned().unwrap_or(Value::Null);
                    let r = remote.get(field).cloned().unwrap_or(Value::Null);
                    let decision = decide(base, &l, &r);
                    let name = serde_json::to_value(decision)?
                        .as_str()
                        .unwrap_or_default()
                        .to_owned();
                    *summary.entry(name).or_default() += 1;
                    if decision != FieldDecision::Unchanged {
                        preview.fields.push(FieldDiff {
                            field: field.clone(),
                            decision,
                            local: l,
                            remote: r,
                            baseline_local: base.local.clone(),
                            baseline_remote: base.remote.clone(),
                            owner: base.owner,
                            acknowledged: base.acknowledged,
                        });
                    }
                }
            }
            if preview.blocked.is_some() || !preview.fields.is_empty() {
                tracks.push(preview);
            }
        }
        Ok(NativeSyncPreview {
            target_id: target.into(),
            revision: state.revision,
            tracks,
            summary,
            writes_enabled: false,
            write_blocker: "No rekordbox schema revision has been verified for direct writes against the real application, so synchronization can be previewed but not applied.".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn base(
        local: Value,
        remote: Value,
        owner: NativeFieldOwner,
        acknowledged: bool,
    ) -> NativeFieldBaseline {
        NativeFieldBaseline {
            local,
            remote,
            owner,
            acknowledged,
        }
    }
    #[test]
    fn agreed_baselines_follow_whichever_side_changed() {
        use FieldDecision::*;
        use NativeFieldOwner::Shared;
        let b = base(json!(["A"]), json!(["A"]), Shared, true);
        assert_eq!(decide(&b, &json!(["A"]), &json!(["A"])), Unchanged);
        assert_eq!(decide(&b, &json!(["B"]), &json!(["A"])), PushLocal);
        assert_eq!(decide(&b, &json!(["A"]), &json!(["C"])), PullRemote);
        assert_eq!(decide(&b, &json!(["B"]), &json!(["C"])), Conflict);
        assert_eq!(decide(&b, &json!(["B"]), &json!(["B"])), Converged);
    }
    #[test]
    fn initial_observations_are_decided_by_ownership_and_never_acknowledged_by_equality() {
        use FieldDecision::*;
        use NativeFieldOwner::*;
        let differing = |owner| base(json!(["A"]), json!(["Z"]), owner, false);
        assert_eq!(
            decide(&differing(Local), &json!(["A"]), &json!(["Z"])),
            PushLocal
        );
        assert_eq!(
            decide(&differing(Remote), &json!(["A"]), &json!(["Z"])),
            PullRemote
        );
        assert_eq!(
            decide(&differing(Shared), &json!(["A"]), &json!(["Z"])),
            Conflict
        );
        assert_eq!(
            decide(&differing(Unowned), &json!(["A"]), &json!(["Z"])),
            NeedsOwner
        );
        // Initially equal values stay equal observations; they are not an agreed baseline.
        let equal = base(json!(["A"]), json!(["A"]), Local, false);
        assert_eq!(decide(&equal, &json!(["A"]), &json!(["A"])), Unchanged);
    }
    #[test]
    fn editing_a_field_the_other_tool_owns_is_a_conflict() {
        use FieldDecision::*;
        use NativeFieldOwner::*;
        let local_owned = base(json!(["A"]), json!(["A"]), Local, true);
        assert_eq!(decide(&local_owned, &json!(["A"]), &json!(["R"])), Conflict);
        let remote_owned = base(json!(["A"]), json!(["A"]), Remote, true);
        assert_eq!(
            decide(&remote_owned, &json!(["L"]), &json!(["A"])),
            Conflict
        );
        assert_eq!(
            decide(&remote_owned, &json!(["A"]), &json!(["R"])),
            PullRemote
        );
        let unowned = base(json!(["A"]), json!(["A"]), Unowned, true);
        assert_eq!(decide(&unowned, &json!(["L"]), &json!(["A"])), NeedsOwner);
    }
    #[test]
    fn an_empty_local_value_never_silently_blanks_a_remote_one() {
        use FieldDecision::*;
        use NativeFieldOwner::Local;
        let b = base(Value::Null, json!(128.5), Local, false);
        assert_eq!(decide(&b, &Value::Null, &json!(128.5)), EmptyLocal);
        // Missing and empty text are the same absence.
        let text = base(json!([]), json!([]), Local, true);
        assert_eq!(decide(&text, &json!([""]), &json!([])), Unchanged);
    }
}
