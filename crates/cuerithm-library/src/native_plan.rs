//! Turn a reconciliation preview into the rekordbox write plan it implies, and say what was left out.
use crate::native_reconcile::{FieldDecision, NativeSyncPreview};
use anyhow::{Context, Result};
use cuerithm_rekordbox::{ContentFields, CueRow, MyTagRef, TrackWrite, WritePlan};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedTrack {
    pub content_id: String,
    pub local_track_id: Option<String>,
    /// Fields that would be written, using Cuerithm's names.
    pub fields: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedField {
    pub content_id: String,
    pub field: String,
    pub decision: FieldDecision,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncPlanSummary {
    pub tracks: Vec<PlannedTrack>,
    /// Fields that need a decision first: conflicts, unowned fields and empty local values.
    pub skipped: Vec<SkippedField>,
    /// Tracks the plan cannot touch, with the reason, such as a changed identity.
    pub blocked: Vec<(String, String)>,
    /// Fields rekordbox holds newer values for; they are never written back.
    pub pull_available: usize,
}

fn string(value: &Value) -> Option<String> {
    Some(value.as_str().unwrap_or_default().to_owned())
}
fn integer(value: &Value) -> Option<i64> {
    value.as_i64()
}

/// The write plan for every field a preview marks as Cuerithm's to push. Nothing else is planned.
pub fn plan_from_preview(preview: &NativeSyncPreview) -> Result<(WritePlan, SyncPlanSummary)> {
    let mut plan = WritePlan::default();
    let mut summary = SyncPlanSummary::default();
    for track in &preview.tracks {
        if let Some(reason) = &track.blocked {
            summary
                .blocked
                .push((track.content_id.clone(), reason.clone()));
            continue;
        }
        let mut fields = ContentFields::default();
        let mut cues: Option<Vec<CueRow>> = None;
        let mut tags: Option<Vec<MyTagRef>> = None;
        let mut written = vec![];
        for diff in &track.fields {
            match diff.decision {
                FieldDecision::PushLocal => {}
                FieldDecision::PullRemote => {
                    summary.pull_available += 1;
                    continue;
                }
                FieldDecision::Unchanged | FieldDecision::Converged => continue,
                decision => {
                    summary.skipped.push(SkippedField {
                        content_id: track.content_id.clone(),
                        field: diff.field.clone(),
                        decision,
                    });
                    continue;
                }
            }
            let value = &diff.local;
            match diff.field.as_str() {
                "title" => fields.title = string(value),
                "artist" => fields.artist = string(value),
                "album" => fields.album = string(value),
                "genre" => fields.genre = string(value),
                "label" => fields.label = string(value),
                "remixer" => fields.remixer = string(value),
                "composer" => fields.composer = string(value),
                "comment" => fields.comment = string(value),
                "lyricist" => fields.lyricist = string(value),
                "mix_name" => fields.subtitle = string(value),
                "key" => fields.key = string(value),
                "release_date" => fields.release_date = string(value),
                "bpm" => fields.bpm_hundredths = integer(value),
                "release_year" => fields.release_year = integer(value),
                "track_number" => fields.track_number = integer(value),
                "disc_number" => fields.disc_number = integer(value),
                "rating" => fields.rating = integer(value),
                "tag_ids" => {
                    tags = Some(
                        value
                            .as_array()
                            .map(|a| a.as_slice())
                            .unwrap_or_default()
                            .iter()
                            .filter_map(Value::as_str)
                            .filter_map(|t| t.split_once(crate::native_sync::TAG_SEPARATOR))
                            .map(|(category, tag)| MyTagRef {
                                category: category.into(),
                                tag: tag.into(),
                            })
                            .collect(),
                    )
                }
                "cues" => {
                    cues = Some(
                        serde_json::from_value(value.clone())
                            .context("The planned cues are not cue rows")?,
                    )
                }
                _ => continue,
            }
            written.push(diff.field.clone());
        }
        if written.is_empty() {
            continue;
        }
        plan.tracks.push(TrackWrite {
            content_id: track.content_id.clone(),
            expected_fingerprint: track
                .expected_fingerprint
                .clone()
                .context("A track to write has no recorded observation")?,
            fields,
            cues,
            tags,
        });
        summary.tracks.push(PlannedTrack {
            content_id: track.content_id.clone(),
            local_track_id: track.local_track_id.clone(),
            fields: written,
        });
    }
    Ok((plan, summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NativeFieldOwner;
    use crate::native_reconcile::{FieldDiff, TrackPreview};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn diff(field: &str, decision: FieldDecision, local: Value) -> FieldDiff {
        FieldDiff {
            field: field.into(),
            decision,
            local,
            remote: Value::Null,
            baseline_local: Value::Null,
            baseline_remote: Value::Null,
            owner: NativeFieldOwner::Local,
            acknowledged: true,
        }
    }
    fn preview(tracks: Vec<TrackPreview>) -> NativeSyncPreview {
        NativeSyncPreview {
            target_id: "t".into(),
            revision: 1,
            tracks,
            summary: BTreeMap::new(),
            writes_enabled: false,
            write_blocker: String::new(),
        }
    }
    fn track(id: &str, fields: Vec<FieldDiff>) -> TrackPreview {
        TrackPreview {
            content_id: id.into(),
            local_track_id: Some(format!("local-{id}")),
            status: "linked".into(),
            expected_fingerprint: Some(format!("fp-{id}")),
            blocked: None,
            fields,
        }
    }

    #[test]
    fn only_fields_cuerithm_owns_and_changed_are_planned() {
        let rows = json!([{"in_msec": 1000, "out_msec": -1, "kind": 1, "active_loop": false,
            "beat_loop_size": null, "color": -1, "color_table_index": 41, "comment": "Start"}]);
        let preview = preview(vec![track(
            "9",
            vec![
                diff("title", FieldDecision::PushLocal, json!("New")),
                diff("bpm", FieldDecision::PushLocal, json!(12850)),
                diff("key", FieldDecision::PushLocal, json!("8A")),
                diff("mix_name", FieldDecision::PushLocal, json!("Club Mix")),
                diff("cues", FieldDecision::PushLocal, rows),
                diff("genre", FieldDecision::PullRemote, Value::Null),
                diff("artist", FieldDecision::Conflict, json!("Mine")),
                diff("album", FieldDecision::NeedsOwner, json!("Mine")),
                diff("label", FieldDecision::EmptyLocal, Value::Null),
                diff("comment", FieldDecision::Converged, json!("same")),
            ],
        )]);
        let (plan, summary) = plan_from_preview(&preview).unwrap();
        assert_eq!(plan.tracks.len(), 1);
        let write = &plan.tracks[0];
        assert_eq!(write.expected_fingerprint, "fp-9");
        assert_eq!(write.fields.title.as_deref(), Some("New"));
        assert_eq!(write.fields.bpm_hundredths, Some(12850));
        assert_eq!(write.fields.key.as_deref(), Some("8A"));
        assert_eq!(write.fields.subtitle.as_deref(), Some("Club Mix"));
        assert!(write.fields.genre.is_none() && write.fields.artist.is_none());
        assert_eq!(
            write.cues.as_ref().unwrap()[0].comment.as_deref(),
            Some("Start")
        );
        assert_eq!(
            summary.tracks[0].fields,
            ["title", "bpm", "key", "mix_name", "cues"]
        );
        assert_eq!(summary.pull_available, 1);
        assert_eq!(
            summary
                .skipped
                .iter()
                .map(|s| (s.field.as_str(), s.decision))
                .collect::<Vec<_>>(),
            [
                ("artist", FieldDecision::Conflict),
                ("album", FieldDecision::NeedsOwner),
                ("label", FieldDecision::EmptyLocal)
            ]
        );
    }

    #[test]
    fn tags_are_planned_as_category_and_tag_pairs() {
        let tags = json!(["Mood\u{1f}Dark", "Vibe\u{1f}Warm"]);
        let (plan, summary) = plan_from_preview(&preview(vec![track(
            "9",
            vec![diff("tag_ids", FieldDecision::PushLocal, tags)],
        )]))
        .unwrap();
        let written = plan.tracks[0].tags.as_ref().unwrap();
        assert_eq!(
            written
                .iter()
                .map(|t| (t.category.as_str(), t.tag.as_str()))
                .collect::<Vec<_>>(),
            [("Mood", "Dark"), ("Vibe", "Warm")]
        );
        assert_eq!(summary.tracks[0].fields, ["tag_ids"]);
    }

    #[test]
    fn blocked_tracks_and_tracks_with_nothing_to_push_produce_no_write() {
        let mut blocked = track(
            "1",
            vec![diff("title", FieldDecision::PushLocal, json!("x"))],
        );
        blocked.blocked = Some("identity_changed".into());
        let only_pull = track(
            "2",
            vec![diff("title", FieldDecision::PullRemote, Value::Null)],
        );
        let (plan, summary) = plan_from_preview(&preview(vec![blocked, only_pull])).unwrap();
        assert!(plan.tracks.is_empty());
        assert_eq!(
            summary.blocked,
            [("1".to_string(), "identity_changed".to_string())]
        );
        assert_eq!(summary.pull_available, 1);
        let mut unobserved = track(
            "3",
            vec![diff("title", FieldDecision::PushLocal, json!("x"))],
        );
        unobserved.expected_fingerprint = None;
        assert!(plan_from_preview(&preview(vec![unobserved])).is_err());
    }
}
