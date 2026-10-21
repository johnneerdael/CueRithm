use super::*;
use anyhow::{Result, ensure};
use cuerithm_autotag::{
    CatalogMatch, MetadataPatch, MetadataSnapshot, MetadataTextField as F, TextEdit, TransferDate,
    ValueEdit,
};
fn values(value: Option<&String>) -> Vec<String> {
    value
        .filter(|v| !v.trim().is_empty())
        .cloned()
        .into_iter()
        .collect()
}
fn date(value: Option<&TransferDate>) -> Vec<String> {
    value
        .map(|d| match (d.month, d.day) {
            (Some(m), Some(day)) => format!("{:04}-{m:02}-{day:02}", d.year),
            (Some(m), None) => format!("{:04}-{m:02}", d.year),
            _ => format!("{:04}", d.year),
        })
        .into_iter()
        .collect()
}
fn parse_date(value: &str) -> Result<TransferDate> {
    let parts = value.split('-').collect::<Vec<_>>();
    ensure!(
        !parts.is_empty() && parts.len() <= 3,
        "Invalid selected release date"
    );
    let value = TransferDate {
        year: parts[0].parse()?,
        month: parts.get(1).map(|s| s.parse()).transpose()?,
        day: parts.get(2).map(|s| s.parse()).transpose()?,
    };
    cuerithm_autotag::validate_date(&value)?;
    Ok(value)
}
fn candidate_date(candidate: &CatalogMatch) -> Vec<String> {
    if candidate.facts.is_some() {
        return date(candidate.selected_release_date());
    }
    let track = &candidate.track;
    track
        .release_date
        .map(|d| d.to_string())
        .or_else(|| track.release_year.map(|y| format!("{y:04}")))
        .into_iter()
        .collect()
}
/// What a saved Incoming edit leaves in a field. Preparation applies that edit over any accepted
/// review, so the review must start from, and cannot replace, the value it will produce.
fn held_value(
    saved: &MetadataPatch,
    id: &str,
    field: Option<F>,
    before: &[String],
) -> Option<Vec<String>> {
    fn one<T: ToString>(edit: &ValueEdit<T>) -> Option<Vec<String>> {
        match edit {
            ValueEdit::Keep => None,
            ValueEdit::Set { value } => Some(vec![value.to_string()]),
            ValueEdit::Clear => Some(vec![]),
        }
    }
    if let Some(field) = field {
        return match saved.text.get(&field)? {
            TextEdit::Keep => None,
            TextEdit::Set { values } => Some(values.clone()),
            TextEdit::Clear => Some(vec![]),
            TextEdit::Append { values } => {
                let mut out = before.to_vec();
                for value in values {
                    if !out.contains(value) {
                        out.push(value.clone());
                    }
                }
                Some(out)
            }
        };
    }
    match id {
        "releaseDate" => match &saved.release_date {
            ValueEdit::Keep => None,
            ValueEdit::Set { value } => Some(date(Some(value))),
            ValueEdit::Clear => Some(vec![]),
        },
        "publishDate" => match &saved.original_date {
            ValueEdit::Keep => None,
            ValueEdit::Set { value } => Some(date(Some(value))),
            ValueEdit::Clear => Some(vec![]),
        },
        "trackNumber" => one(&saved.track_number),
        "trackTotal" => one(&saved.track_total),
        "discNumber" => one(&saved.disc_number),
        "bpm" => one(&saved.bpm),
        _ => None,
    }
}
pub(super) fn make_fields(
    native: &MetadataSnapshot,
    saved: Option<&MetadataPatch>,
    candidate: Option<&CatalogMatch>,
    request: &MatchReviewRequest,
    short_title: bool,
) -> Result<(Vec<MatchReviewFieldDiff>, MetadataPatch, Vec<String>)> {
    let track = candidate.map(|matched| &matched.track);
    ensure!(
        request.fields.len() <= 64,
        "Too many review field decisions"
    );
    let mut seen = std::collections::BTreeSet::new();
    for decision in &request.fields {
        ensure!(
            seen.insert(decision.field.to_ascii_lowercase()),
            "Duplicate review field decision"
        );
    }
    type FieldSpec<'a> = (&'a str, &'a str, Option<F>, bool, Vec<String>, Vec<String>);
    let mut specs: Vec<FieldSpec<'_>> = vec![];
    for (id, label, field, multi) in [
        ("title", "Title", F::Title, false),
        ("artist", "Artists", F::Artist, true),
        ("album", "Album", F::Album, false),
        ("albumArtist", "Album artists", F::AlbumArtist, true),
        ("genre", "Genres", F::Genre, true),
        ("style", "Styles", F::Style, true),
        ("label", "Label", F::Label, false),
        ("version", "Mix / version", F::Version, false),
        ("remixer", "Remixers", F::Remixer, true),
        ("catalogNumber", "Catalog number", F::CatalogNumber, false),
        ("isrc", "ISRC", F::Isrc, false),
        ("key", "File key", F::Key, false),
    ] {
        let before = native.text.get(&field).cloned().unwrap_or_default();
        let proposed = track
            .map(|t| match field {
                F::Title => vec![if short_title {
                    t.title.clone()
                } else {
                    t.full_title()
                }],
                F::Artist => t.artists.clone(),
                F::Album => values(t.album.as_ref()),
                F::AlbumArtist => t.album_artists.clone(),
                F::Genre => t.genres.clone(),
                F::Style => t.styles.clone(),
                F::Label => values(t.label.as_ref()),
                F::Version => values(t.version.as_ref()),
                F::Remixer => t.remixers.clone(),
                F::CatalogNumber => values(t.catalog_number.as_ref()),
                F::Isrc => values(t.isrc.as_ref()),
                F::Key => values(t.key.as_ref()),
                _ => vec![],
            })
            .unwrap_or_default();
        specs.push((id, label, Some(field), multi, before, proposed));
    }
    specs.extend([
        (
            "releaseDate",
            "Release date",
            None,
            false,
            date(native.release_date.as_ref()),
            candidate.map(candidate_date).unwrap_or_default(),
        ),
        (
            "publishDate",
            "Original release date",
            None,
            false,
            date(native.original_date.as_ref()),
            candidate
                .map(|candidate| {
                    if let Some(facts) = candidate.musicbrainz() {
                        date(facts.recording_first_release_date.as_ref())
                    } else if candidate.facts.is_some() {
                        vec![]
                    } else {
                        candidate
                            .track
                            .publish_date
                            .map(|value| value.to_string())
                            .or_else(|| {
                                candidate
                                    .track
                                    .publish_year
                                    .map(|year| format!("{year:04}"))
                            })
                            .into_iter()
                            .collect()
                    }
                })
                .unwrap_or_default(),
        ),
        (
            "trackNumber",
            "Track number",
            None,
            false,
            native
                .track_number
                .map(|v| v.to_string())
                .into_iter()
                .collect(),
            track
                .and_then(|t| t.track_number.as_ref())
                .map(ToString::to_string)
                .into_iter()
                .collect(),
        ),
        (
            "trackTotal",
            "Track total",
            None,
            false,
            native
                .track_total
                .map(|v| v.to_string())
                .into_iter()
                .collect(),
            track
                .and_then(|t| t.track_total)
                .map(|v| v.to_string())
                .into_iter()
                .collect(),
        ),
        (
            "discNumber",
            "Disc number",
            None,
            false,
            native
                .disc_number
                .map(|v| v.to_string())
                .into_iter()
                .collect(),
            track
                .and_then(|t| t.disc_number)
                .map(|v| v.to_string())
                .into_iter()
                .collect(),
        ),
        (
            "bpm",
            "File BPM",
            None,
            false,
            native.bpm.map(|v| v.to_string()).into_iter().collect(),
            track
                .and_then(|t| t.bpm)
                .map(|v| v.to_string())
                .into_iter()
                .collect(),
        ),
    ]);
    if let Some(facts) = candidate.and_then(CatalogMatch::musicbrainz) {
        specs.push((
            "trackId",
            "MusicBrainz recording ID",
            None,
            false,
            vec![],
            vec![facts.recording_id.to_string()],
        ));
        specs.push((
            "releaseId",
            "MusicBrainz release ID",
            None,
            false,
            vec![],
            facts
                .release_id
                .as_ref()
                .map(ToString::to_string)
                .into_iter()
                .collect(),
        ));
    }
    let mut patch = MetadataPatch::default();
    let mut fields = vec![];
    let mut blockers = vec![];
    for (id, label, native_field, multi, before, proposed) in specs {
        let held = saved.and_then(|saved| held_value(saved, id, native_field, &before));
        let before = held.clone().unwrap_or(before);
        let action = request
            .fields
            .iter()
            .find(|d| d.field.eq_ignore_ascii_case(id))
            .map(|d| d.action)
            .unwrap_or(MatchReviewFieldAction::Keep);
        let read_only_id = matches!(id, "trackId" | "releaseId")
            && candidate.and_then(CatalogMatch::musicbrainz).is_some();
        let policy_allows = candidate.is_none_or(|candidate| {
            serde_json::from_value::<cuerithm_autotag::SupportedTag>(serde_json::Value::String(
                id.into(),
            ))
            .is_ok_and(|field| {
                cuerithm_autotag::provider_native_field_allowed(&candidate.track.platform, field)
            })
        });
        let capability_reason = if read_only_id {
            Some("Scoped identifiers are read-only review evidence. Native ID editing is not supported here.".to_owned())
        } else if !policy_allows {
            Some(
                "This source does not permit writing this native field; keep the existing value."
                    .to_owned(),
            )
        } else {
            None
        };
        let mut allowed_actions = vec![MatchReviewFieldAction::Keep];
        if track.is_some() && capability_reason.is_none() && held.is_none() {
            allowed_actions.push(MatchReviewFieldAction::Replace);
            if multi {
                allowed_actions.push(MatchReviewFieldAction::Append);
            }
        }
        let mut reason = capability_reason;
        let after = if !allowed_actions.contains(&action) {
            reason.get_or_insert_with(|| {
                if held.is_some() {
                    "Your saved Incoming edit sets this field and takes priority; edit the metadata again to change it"
                } else {
                    "This action is not supported for this field"
                }
                .to_owned()
            });
            before.clone()
        } else if action != MatchReviewFieldAction::Keep && proposed.is_empty() {
            reason = Some(
                "The selected candidate has no value; Keep existing or use the metadata editor"
                    .to_owned(),
            );
            before.clone()
        } else {
            match action {
                MatchReviewFieldAction::Keep => before.clone(),
                MatchReviewFieldAction::Replace => proposed.clone(),
                MatchReviewFieldAction::Append => {
                    let mut out = before.clone();
                    for value in &proposed {
                        if !out.contains(value) {
                            out.push(value.clone());
                        }
                    }
                    out
                }
            }
        };
        if action != MatchReviewFieldAction::Keep && reason.is_none() {
            let write = (|| -> Result<()> {
                if let Some(field) = native_field {
                    patch.text.insert(
                        field,
                        TextEdit::Set {
                            values: after.clone(),
                        },
                    );
                } else {
                    let value = after
                        .first()
                        .ok_or_else(|| anyhow::anyhow!("Missing selected field"))?;
                    match id {
                        "releaseDate" => {
                            patch.release_date = ValueEdit::Set {
                                value: parse_date(value)?,
                            }
                        }
                        "publishDate" => {
                            patch.original_date = ValueEdit::Set {
                                value: parse_date(value)?,
                            }
                        }
                        "trackNumber" => {
                            patch.track_number = ValueEdit::Set {
                                value: value.parse()?,
                            }
                        }
                        "trackTotal" => {
                            patch.track_total = ValueEdit::Set {
                                value: value.parse()?,
                            }
                        }
                        "discNumber" => {
                            patch.disc_number = ValueEdit::Set {
                                value: value.parse()?,
                            }
                        }
                        "bpm" => {
                            patch.bpm = ValueEdit::Set {
                                value: value.parse()?,
                            }
                        }
                        _ => {}
                    }
                }
                Ok(())
            })();
            if let Err(error) = write {
                reason = Some(format!(
                    "Selected value cannot be represented safely: {error:#}"
                ));
            }
        }
        if action != MatchReviewFieldAction::Keep
            && let Some(reason) = &reason
        {
            blockers.push(format!("{label}: {reason}"));
        }
        fields.push(MatchReviewFieldDiff {
            field: id.into(),
            label: label.into(),
            before,
            proposed,
            after,
            action,
            allowed_actions,
            reason,
        });
    }
    for choice in &request.fields {
        if !choice.field.eq_ignore_ascii_case("albumArt")
            && !fields
                .iter()
                .any(|f| f.field.eq_ignore_ascii_case(&choice.field))
        {
            blockers.push(format!(
                "Unsupported review field '{}'; use the typed metadata editor",
                choice.field
            ));
        }
    }
    if let Err(error) = patch.validate() {
        blockers.push(format!("Selected metadata is invalid: {error:#}"));
    }
    Ok((fields, patch, blockers))
}

#[cfg(test)]
mod catalog_facts_tests {
    use super::*;
    use cuerithm_autotag::{CatalogFacts, CatalogMatch, CatalogTrack};
    fn candidate() -> CatalogMatch {
        let mut candidate = CatalogMatch::new(
            1.,
            CatalogTrack {
                platform: "musicbrainz".into(),
                title: "Signal".into(),
                artists: vec!["Artist".into()],
                track_id: Some("11111111-1111-4111-8111-111111111111".into()),
                release_id: Some("22222222-2222-4222-8222-222222222222".into()),
                release_year: Some(2024),
                ..Default::default()
            },
        );
        candidate.facts=Some(CatalogFacts::MusicBrainz(serde_json::from_value(serde_json::json!({
            "schema_version":1,"recording_id":"11111111-1111-4111-8111-111111111111","release_id":"22222222-2222-4222-8222-222222222222","release_group_id":null,"release_track_id":"33333333-3333-4333-8333-333333333333","scope":"release_track",
            "recording_title":"Signal","track_title":"Signal","recording_disambiguation":null,
            "recording_credit":[],"track_credit":[],"release_credit":[],"isrcs":[],"recording_length_ms":null,"track_length_ms":null,
            "selected_release_date":{"year":2024,"month":6,"day":null},"recording_first_release_date":{"year":2020,"month":null,"day":null},"release_group_first_release_date":null,"release_events":[],
            "position":{"medium_position":1,"track_position":1,"printed_number":"A1","medium_track_count":2,"medium_count":1},
            "release":{"title":"Signal EP","status":"Official","country":null,"barcode":null,"media_format":"Vinyl","labels":[]},"cover":null,
            "source_policy":{"adapter_revision":"fixture","policy_revision":"fixture","data_class":"core"}
        })).unwrap()));
        candidate.validate().unwrap();
        candidate
    }
    fn request() -> MatchReviewRequest {
        MatchReviewRequest {
            track_id: "fixture".into(),
            generation: "generation".into(),
            action: MatchReviewAction::UseSelected,
            candidate_id: Some("candidate".into()),
            fields: vec![MatchReviewFieldDecision {
                field: "releaseDate".into(),
                action: MatchReviewFieldAction::Replace,
            }],
            acknowledge_conflicts: false,
            allow_id3_upgrade: false,
        }
    }
    #[test]
    fn selected_release_month_survives_review_without_inventing_a_day() {
        let native: MetadataSnapshot = serde_json::from_value(serde_json::json!({})).unwrap();
        let candidate = candidate();
        let (fields, patch, blockers) =
            make_fields(&native, None, Some(&candidate), &request(), false).unwrap();
        assert!(blockers.is_empty());
        assert_eq!(
            fields
                .iter()
                .find(|field| field.field == "releaseDate")
                .unwrap()
                .proposed,
            vec!["2024-06"]
        );
        assert!(matches!(
            patch.release_date,
            ValueEdit::Set {
                value: TransferDate {
                    year: 2024,
                    month: Some(6),
                    day: None
                }
            }
        ));
    }
    #[test]
    fn saved_original_date_takes_priority_over_selected_recording_date() {
        let native: MetadataSnapshot = serde_json::from_value(serde_json::json!({})).unwrap();
        let candidate = candidate();
        let mut saved = MetadataPatch::default();
        saved.original_date = ValueEdit::Set {
            value: TransferDate {
                year: 2018,
                month: Some(3),
                day: None,
            },
        };
        let mut request = request();
        request.fields = vec![MatchReviewFieldDecision {
            field: "publishDate".into(),
            action: MatchReviewFieldAction::Replace,
        }];
        let (fields, patch, blockers) =
            make_fields(&native, Some(&saved), Some(&candidate), &request, false).unwrap();
        let field = fields
            .iter()
            .find(|field| field.field == "publishDate")
            .unwrap();
        assert_eq!(field.before, vec!["2018-03"]);
        assert_eq!(field.after, vec!["2018-03"]);
        assert_eq!(field.proposed, vec!["2020"]);
        assert_eq!(field.allowed_actions, vec![MatchReviewFieldAction::Keep]);
        assert!(
            field
                .reason
                .as_deref()
                .unwrap()
                .contains("saved Incoming edit")
        );
        assert!(!blockers.is_empty());
        assert!(patch.is_noop());
        request.fields.clear();
        let (_, patch, blockers) =
            make_fields(&native, Some(&saved), Some(&candidate), &request, false).unwrap();
        assert!(blockers.is_empty());
        assert!(patch.is_noop());
    }
    #[test]
    fn musicbrainz_identifier_choices_are_explicitly_read_only_in_review() {
        let native: MetadataSnapshot = serde_json::from_value(serde_json::json!({})).unwrap();
        let candidate = candidate();
        let mut request = request();
        request.fields.clear();
        let (fields, patch, blockers) =
            make_fields(&native, None, Some(&candidate), &request, false).unwrap();
        assert!(blockers.is_empty());
        let id = fields
            .iter()
            .find(|field| field.field == "trackId")
            .expect("Scoped IDs must explain their read-only capability");
        assert_eq!(id.allowed_actions, vec![MatchReviewFieldAction::Keep]);
        assert!(id.reason.as_deref().unwrap().contains("read-only"));
        assert!(patch.is_noop());
    }
}
