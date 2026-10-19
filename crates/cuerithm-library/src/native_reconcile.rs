//! Read-only reconciliation of Cuerithm tracks against the recorded rekordbox observations.
//!
//! This decides, per field, what a future synchronization would have to do. It performs no write
//! to a target database or to the library, and a preview is never an acknowledgement.
use crate::native_sync::{
    NativeFieldBaseline, NativeFieldOwner, TEXT_FIELDS, local_fields, remote_fields,
};
use crate::*;
use crate::{CueExportOptions, rekordbox_cue_rows};
use anyhow::Result;
use camelot_core::lexicon_publication::PublishedCue;
use cuerithm_rekordbox::ImportedCue;
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
    /// The fingerprint of rekordbox's row when it was last observed; a write is refused if the row differs.
    pub expected_fingerprint: Option<String>,
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

/// Every field a synchronization can write, for assigning initial ownership.
pub fn native_writable_fields() -> Vec<&'static str> {
    let mut fields: Vec<&'static str> = TEXT_FIELDS
        .iter()
        .copied()
        .filter(|f| writable(f))
        .collect();
    fields.extend([
        "key",
        "bpm",
        "release_year",
        "release_date",
        "track_number",
        "disc_number",
        "rating",
        "cues",
    ]);
    fields
}
/// Fields a synchronization can write. Everything else (duration, paths, tag IDs) is observation only.
pub(crate) fn writable(field: &str) -> bool {
    (TEXT_FIELDS.contains(&field) && field != "album_artist")
        || matches!(
            field,
            "key"
                | "bpm"
                | "release_year"
                | "release_date"
                | "track_number"
                | "disc_number"
                | "rating"
                | "cues"
        )
}
/// A key as Camelot notation: `8A`, `8B` or a classic name such as `Am`, `C#m` or `Gb`. Anything else is
/// kept as written, upper-cased, so an unfamiliar spelling still compares equal to itself.
pub(crate) fn camelot_key(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let upper = text.to_uppercase();
    let digits = upper.trim_end_matches(['A', 'B']);
    if !digits.is_empty()
        && digits.len() <= 2
        && digits.bytes().all(|c| c.is_ascii_digit())
        && upper.len() == digits.len() + 1
        && camelot_core::MusicalKey::from_camelot(&upper).is_ok()
    {
        return Some(upper);
    }
    let mut chars = text.chars();
    let tonic: i32 = match chars.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return Some(upper),
    };
    let rest: String = chars.collect();
    let (shift, rest): (i32, &str) = match rest.chars().next() {
        Some('#') => (1, &rest[1..]),
        Some('b') | Some('\u{266d}') => (-1, &rest[rest.chars().next().unwrap().len_utf8()..]),
        _ => (0, rest.as_str()),
    };
    let minor = match rest.trim().to_lowercase().as_str() {
        "" | "maj" | "major" => false,
        "m" | "min" | "minor" => true,
        _ => return Some(upper),
    };
    let tonic = (tonic + shift).rem_euclid(12) as u8;
    camelot_core::MusicalKey::new(
        tonic,
        if minor {
            camelot_core::Mode::Minor
        } else {
            camelot_core::Mode::Major
        },
    )
    .ok()
    .map(|k| k.camelot())
}
fn text_of(value: &Value) -> Value {
    let joined = match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("; "),
        Value::String(s) => s.trim().to_owned(),
        _ => String::new(),
    };
    if joined.is_empty() {
        Value::Null
    } else {
        Value::String(joined)
    }
}
fn number_of(value: &Value) -> Value {
    let number = match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    };
    match number.filter(|n| n.is_finite() && *n != 0.) {
        Some(n) => serde_json::json!(n.round() as i64),
        None => Value::Null,
    }
}
fn sorted_rows(mut rows: Vec<cuerithm_rekordbox::CueRow>) -> Value {
    rows.sort_by(|a, b| {
        (a.in_msec, a.out_msec, a.kind, &a.comment)
            .cmp(&(b.in_msec, b.out_msec, b.kind, &b.comment))
    });
    if rows.is_empty() {
        Value::Null
    } else {
        serde_json::to_value(rows).unwrap_or(Value::Null)
    }
}
/// Writable fields in one comparable form. `local` is the Cuerithm side (published cues, Camelot keys, tags
/// as strings); the other side is rekordbox's observation. Cues become the exact rows that would be written.
pub(crate) fn canonical_fields(
    map: &BTreeMap<String, Value>,
    local: bool,
) -> BTreeMap<String, Value> {
    let get = |name: &str| map.get(name).cloned().unwrap_or(Value::Null);
    let mut out = BTreeMap::new();
    for field in map.keys().filter(|f| writable(f)) {
        let value = get(field);
        let canonical = match field.as_str() {
            f if TEXT_FIELDS.contains(&f) => text_of(&value),
            "release_date" => text_of(&value),
            "key" => value
                .as_str()
                .and_then(camelot_key)
                .map_or(Value::Null, Value::String),
            "bpm" => match value.as_f64().filter(|b| b.is_finite() && *b > 0.) {
                Some(bpm) => serde_json::json!((bpm * 100.).round() as i64),
                None => Value::Null,
            },
            "cues" if local => {
                let bpm = get("bpm").as_f64().unwrap_or(0.);
                serde_json::from_value::<Vec<PublishedCue>>(value)
                    .map(|cues| {
                        sorted_rows(
                            rekordbox_cue_rows(&cues, bpm, CueExportOptions::default()).rows,
                        )
                    })
                    .unwrap_or(Value::Null)
            }
            "cues" => serde_json::from_value::<Vec<ImportedCue>>(value)
                .map(|cues| {
                    sorted_rows(
                        cues.iter()
                            .map(|c| cuerithm_rekordbox::CueRow {
                                in_msec: (c.start_seconds.unwrap_or(0.) * 1000.).round() as i64,
                                out_msec: c.end_seconds.map_or(-1, |e| (e * 1000.).round() as i64),
                                kind: c.kind,
                                active_loop: c.active_loop,
                                beat_loop_size: c.beat_loop_size,
                                color: c.color,
                                color_table_index: c.color_table_index,
                                comment: Some(c.comment.clone()).filter(|t| !t.is_empty()),
                            })
                            .collect(),
                    )
                })
                .unwrap_or(Value::Null),
            _ => number_of(&value),
        };
        out.insert(field.clone(), canonical);
    }
    out
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
                expected_fingerprint: observation.as_ref().map(|o| o.raw_hash.clone()),
                blocked,
                fields: vec![],
            };
            if preview.blocked.is_some() {
                *summary.entry("blocked_tracks".into()).or_default() += 1;
            } else if let (Some(local_id), Some(observation)) = (&link.local, &observation) {
                let local = canonical_fields(&local_fields(&conn, local_id)?, true);
                let remote = canonical_fields(&remote_fields(observation)?, false);
                let base_local = canonical_fields(
                    &link
                        .fields
                        .iter()
                        .map(|(k, f)| (k.clone(), f.local.clone()))
                        .collect(),
                    true,
                );
                let base_remote = canonical_fields(
                    &link
                        .fields
                        .iter()
                        .map(|(k, f)| (k.clone(), f.remote.clone()))
                        .collect(),
                    false,
                );
                for (field, base) in link.fields.iter().filter(|(f, _)| writable(f)) {
                    let l = local.get(field).cloned().unwrap_or(Value::Null);
                    let r = remote.get(field).cloned().unwrap_or(Value::Null);
                    let comparable = NativeFieldBaseline {
                        local: base_local.get(field).cloned().unwrap_or(Value::Null),
                        remote: base_remote.get(field).cloned().unwrap_or(Value::Null),
                        owner: base.owner,
                        acknowledged: base.acknowledged,
                    };
                    let decision = decide(&comparable, &l, &r);
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
                            baseline_local: comparable.local,
                            baseline_remote: comparable.remote,
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

    #[test]
    fn keys_compare_as_camelot_whatever_notation_each_side_uses() {
        for (text, camelot) in [
            ("8A", Some("8A")),
            ("8a", Some("8A")),
            ("12B", Some("12B")),
            ("Am", Some("8A")),
            ("A minor", Some("8A")),
            ("C", Some("8B")),
            ("C#m", Some("12A")),
            ("Dbm", Some("12A")),
            ("Gb", Some("2B")),
            ("F#", Some("2B")),
            ("Ebm", Some("2A")),
            ("", None),
            ("  ", None),
        ] {
            assert_eq!(camelot_key(text).as_deref(), camelot, "{text:?}");
        }
        // Unfamiliar spellings stay comparable to themselves instead of being guessed at.
        assert_eq!(camelot_key("10M").as_deref(), Some("10M"));
        assert_eq!(camelot_key("13A").as_deref(), Some("13A"));
    }
    #[test]
    fn both_sides_reduce_to_the_same_form_for_equal_content() {
        let local: BTreeMap<String, Value> = [
            ("title", json!(["Song"])),
            ("artist", json!(["A", "B"])),
            ("key", json!("8A")),
            ("bpm", json!(128.5)),
            ("rating", json!("3")),
            ("duration_seconds", json!(200.0)),
            ("comment", json!([])),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let remote: BTreeMap<String, Value> = [
            ("title", json!(["Song"])),
            ("artist", json!(["A; B"])),
            ("key", json!("Am")),
            ("bpm", json!(128.5)),
            ("rating", json!(3)),
            ("duration_seconds", json!(199.0)),
            ("comment", json!([""])),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let (l, r) = (
            canonical_fields(&local, true),
            canonical_fields(&remote, false),
        );
        assert_eq!(l, r);
        assert_eq!(l["bpm"], json!(12850));
        assert_eq!(l["artist"], json!("A; B"));
        assert_eq!(l["comment"], Value::Null);
        assert!(!l.contains_key("duration_seconds"));
    }
    #[test]
    fn published_cues_and_observed_cues_meet_in_the_rows_that_would_be_written() {
        use camelot_core::lexicon_publication::{PublishedCue, PublishedType};
        let published = vec![PublishedCue {
            name: "Drop".into(),
            start_time: 40.25,
            kind: PublishedType::Cue,
            position: 1,
            color: json!("red"),
            end_time: None,
            active_loop: None,
            data: None,
        }];
        let observed = json!([{
            "id": "9", "uuid": null, "content_uuid": null, "kind": 2, "slot": 1, "memory": false,
            "start_seconds": 40.25, "end_seconds": null, "active_loop": false, "comment": "Drop",
            "color": -1, "color_table_index": 41, "beat_loop_size": null, "raw_hash": "x"
        }]);
        let local = BTreeMap::from([
            ("cues".to_owned(), serde_json::to_value(&published).unwrap()),
            ("bpm".to_owned(), json!(128.0)),
        ]);
        let remote = BTreeMap::from([("cues".to_owned(), observed)]);
        assert_eq!(
            canonical_fields(&local, true)["cues"],
            canonical_fields(&remote, false)["cues"]
        );
        assert_ne!(canonical_fields(&local, true)["cues"], Value::Null);
        // A cue list in some other shape is treated as having no comparable cues rather than a mismatch.
        let odd = BTreeMap::from([("cues".to_owned(), json!({"native": true}))]);
        assert_eq!(canonical_fields(&odd, true)["cues"], Value::Null);
    }
}
