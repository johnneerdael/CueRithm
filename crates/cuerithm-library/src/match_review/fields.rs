use super::*;
use anyhow::{Result, ensure};
use cuerithm_autotag::{
    CatalogTrack, MetadataPatch, MetadataSnapshot, MetadataTextField as F, TextEdit, TransferDate,
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
fn candidate_date(track: &CatalogTrack) -> Vec<String> {
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
    track: Option<&CatalogTrack>,
    request: &MatchReviewRequest,
    short_title: bool,
) -> Result<(Vec<MatchReviewFieldDiff>, MetadataPatch, Vec<String>)> {
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
            track.map(candidate_date).unwrap_or_default(),
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
        let mut allowed_actions = vec![MatchReviewFieldAction::Keep];
        if track.is_some() && held.is_none() {
            allowed_actions.push(MatchReviewFieldAction::Replace);
            if multi {
                allowed_actions.push(MatchReviewFieldAction::Append);
            }
        }
        let mut reason = None;
        let after = if !allowed_actions.contains(&action) {
            reason = Some(if held.is_some() {
                "Your saved Incoming edit sets this field and takes priority; edit the metadata again to change it"
            } else {
                "This action is not supported for this field"
            }
            .to_owned());
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
        if let Some(reason) = &reason {
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
