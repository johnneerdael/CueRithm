//! Pure, typed local smart-playlist evaluation. No database, filesystem or DSP I/O.
mod cues;
mod dates;
mod keys;
mod membership;
mod model;
mod numeric;
mod tags;
mod text;
use anyhow::{Context, Result, ensure};
use cuerithm_regex::Pattern;
pub use keys::{camelot as key_to_camelot, compatible as compatible_keys};
pub use model::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[derive(Debug)]
pub struct EvaluationContext {
    pub now_millis: i64,
    pub time_zone: chrono_tz::Tz,
    pub key_mixing_mode: KeyMixingMode,
    pub memberships: BTreeMap<String, BTreeSet<String>>,
    pub protected_genres: BTreeSet<String>,
}
impl EvaluationContext {
    pub fn new(now_millis: i64, time_zone: &str) -> Result<Self> {
        ensure!(
            chrono::DateTime::from_timestamp_millis(now_millis).is_some(),
            "Invalid evaluation clock"
        );
        Ok(Self {
            now_millis,
            time_zone: time_zone.parse()?,
            key_mixing_mode: KeyMixingMode::Harmonic,
            memberships: Default::default(),
            protected_genres: Default::default(),
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    Text,
    Number,
    Date,
    Boolean,
    Color,
    Tags,
    Categories,
    Exists,
    Tempo,
    Bpm,
    Membership,
    Cue,
    Key,
}
fn family(op: Operator) -> Family {
    use Operator::*;
    match op {
        StringContains | StringNotContains | StringEquals | StringNotEquals | StringStartsWith
        | StringNotStartsWith | StringEndsWith | StringNotEndsWith | StringRegExpMatch
        | StringNotRegExpMatch => Family::Text,
        NumberEquals
        | NumberNotEquals
        | NumberGreaterThan
        | NumberGreaterThanEquals
        | NumberLessThan
        | NumberLessThanEquals
        | NumberBetween => Family::Number,
        DateEquals | DateNotEquals | DateAfter | DateBefore | DateRecent | DateNotRecent
        | DateBetween => Family::Date,
        BooleanTrue | BooleanFalse => Family::Boolean,
        ColorEquals | ColorNotEquals => Family::Color,
        TagsIncludes | TagsNotIncludes | TagsAtleastOne | TagsExact => Family::Tags,
        TagCategoryNone | TagCategoryAtleastOneAnyCat | TagCategoryAtleastOneAllCAt => {
            Family::Categories
        }
        Exists | ExistsNot => Family::Exists,
        TempomarkerSameBpm | TempomarkerNotSameBpm => Family::Tempo,
        BpmExactNumber | BpmNotExactNumber => Family::Bpm,
        PlaylistInSpecificPlaylist
        | PlaylistNotInSpecificPlaylist
        | PlaylistInSpecificPlaylistFolder
        | PlaylistNotInSpecificPlaylistFolder
        | PlaylistInSpecificSmartlist
        | PlaylistNotInSpecificSmartlist => Family::Membership,
        CuepointAnyLabelIncludes
        | CuepointAnyColorEquals
        | CuepointNoLabelIncludes
        | CuepointNoColorEquals
        | CuepointHasLoop
        | CuepointHasNoLoop
        | CuepointHasActiveLoop
        | CuepointHasNoActiveLoop
        | CuepointHasMemoryCue
        | CuepointHasNoMemoryCue => Family::Cue,
        KeySimilar => Family::Key,
    }
}
fn reference_kind(operator: Operator) -> Option<PlaylistKind> {
    match operator {
        Operator::PlaylistInSpecificPlaylist | Operator::PlaylistNotInSpecificPlaylist => {
            Some(PlaylistKind::Playlist)
        }
        Operator::PlaylistInSpecificPlaylistFolder
        | Operator::PlaylistNotInSpecificPlaylistFolder => Some(PlaylistKind::Folder),
        Operator::PlaylistInSpecificSmartlist | Operator::PlaylistNotInSpecificSmartlist => {
            Some(PlaylistKind::Smartlist)
        }
        _ => None,
    }
}
impl SmartDefinition {
    /// Structural validation is independent of the current catalog and dependencies.
    pub fn validate_shape(&self) -> Result<()> {
        ensure!(
            self.rules.len() <= 128,
            "A smart list supports at most 128 rules"
        );
        let mut ids = BTreeSet::new();
        for r in &self.rules {
            ensure!(
                !r.id.is_empty() && r.id.len() <= 128 && ids.insert(&r.id),
                "Rule IDs must be nonempty, unique and at most 128 bytes"
            );
            ensure!(
                !r.field.is_empty() && r.field.len() <= 256,
                "Invalid rule field ID"
            );
            use Operator::*;
            use RuleValue as V;
            let valid = match (&r.operator, &r.value) {
                (_, V::None) => {
                    matches!(
                        family(r.operator),
                        Family::Boolean | Family::Exists | Family::Tempo | Family::Bpm
                    ) || matches!(
                        r.operator,
                        CuepointHasLoop
                            | CuepointHasNoLoop
                            | CuepointHasActiveLoop
                            | CuepointHasNoActiveLoop
                            | CuepointHasMemoryCue
                            | CuepointHasNoMemoryCue
                    )
                }
                (_, V::Text(s)) => {
                    s.len() <= 1_048_576
                        && (matches!(family(r.operator), Family::Text | Family::Key)
                            || matches!(
                                r.operator,
                                CuepointAnyLabelIncludes | CuepointNoLabelIncludes
                            ))
                }
                (_, V::Number(n)) => {
                    family(r.operator) == Family::Number
                        && r.operator != NumberBetween
                        && n.is_finite()
                }
                (NumberBetween, V::NumberRange { start, end }) => {
                    start.is_finite() && end.is_finite()
                }
                (DateBetween, V::DateRange { start, end }) => {
                    chrono::DateTime::from_timestamp_millis(*start).is_some()
                        && chrono::DateTime::from_timestamp_millis(*end).is_some()
                }
                (DateRecent | DateNotRecent, V::Relative { count, .. }) => {
                    count.unsigned_abs() <= 1_000_000
                }
                (DateEquals | DateNotEquals | DateAfter | DateBefore, V::Date(t)) => {
                    chrono::DateTime::from_timestamp_millis(*t).is_some()
                }
                (_, V::Color(c)) => {
                    matches!(
                        r.operator,
                        ColorEquals
                            | ColorNotEquals
                            | CuepointAnyColorEquals
                            | CuepointNoColorEquals
                    ) && c.as_ref().is_none_or(|s| s.len() <= 128)
                }
                (_, V::Ids(ids)) => {
                    matches!(family(r.operator), Family::Tags | Family::Categories)
                        && ids.len() <= 10000
                        && ids.iter().all(|s| !s.is_empty() && s.len() <= 128)
                        && ids.iter().collect::<BTreeSet<_>>().len() == ids.len()
                }
                (_, V::Playlist(p)) => {
                    reference_kind(r.operator) == Some(p.kind)
                        && !p.id.is_empty()
                        && p.id.len() <= 128
                        && p.path.len() <= 64
                        && p.path.iter().all(|s| s.chars().count() <= 255)
                }
                _ => false,
            };
            ensure!(
                valid,
                "Rule {} has invalid value/arity for {:?}",
                r.id,
                r.operator
            );
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct CompiledSmartList {
    definition: SmartDefinition,
    catalog: Arc<FieldCatalog>,
    regex: Vec<Option<Pattern>>,
}
impl CompiledSmartList {
    pub fn new(definition: SmartDefinition, catalog: Arc<FieldCatalog>) -> Result<Self> {
        definition.validate_shape()?;
        let mut regex = Vec::with_capacity(definition.rules.len());
        for rule in &definition.rules {
            let descriptor = catalog
                .fields
                .get(&rule.field)
                .with_context(|| format!("Unknown/unavailable smart field: {}", rule.field))?;
            ensure!(
                descriptor.unavailable_reason.is_none(),
                "Field unavailable: {}: {}",
                rule.field,
                descriptor.unavailable_reason.as_deref().unwrap_or_default()
            );
            let kind = descriptor.kind;
            let supported = match family(rule.operator) {
                Family::Text => matches!(kind, FieldKind::Text | FieldKind::TextGroup),
                Family::Number => kind == FieldKind::Number,
                Family::Date => kind == FieldKind::Date,
                Family::Boolean => kind == FieldKind::Boolean,
                Family::Color => kind == FieldKind::Color,
                Family::Tags => kind == FieldKind::Tags,
                Family::Categories => kind == FieldKind::Categories,
                Family::Exists => kind != FieldKind::TextGroup && kind != FieldKind::Playlist,
                Family::Tempo => kind == FieldKind::Tempos || rule.field == "tempomarkers",
                Family::Bpm => rule.field == "bpm",
                Family::Membership => kind == FieldKind::Playlist,
                Family::Cue => kind == FieldKind::Cues || rule.field == "cuepoints",
                Family::Key => rule.field == "key",
            };
            ensure!(
                supported,
                "Operator {:?} does not support field {}",
                rule.operator,
                rule.field
            );
            if let RuleValue::Ids(ids) = &rule.value {
                for id in ids {
                    ensure!(
                        if family(rule.operator) == Family::Tags {
                            catalog.tags.contains(id)
                        } else {
                            catalog.categories.contains_key(id)
                        },
                        "Unavailable tag/category ID: {id}"
                    );
                }
            }
            if let RuleValue::Playlist(reference) = &rule.value {
                let expected = match rule.operator {
                    Operator::PlaylistInSpecificPlaylistFolder
                    | Operator::PlaylistNotInSpecificPlaylistFolder => PlaylistKind::Folder,
                    Operator::PlaylistInSpecificSmartlist
                    | Operator::PlaylistNotInSpecificSmartlist => PlaylistKind::Smartlist,
                    _ => PlaylistKind::Playlist,
                };
                ensure!(
                    reference.kind == expected
                        && catalog.playlists.get(&reference.id) == Some(&expected),
                    "Unavailable playlist reference: {}",
                    reference.id
                );
            }
            if rule.operator == Operator::KeySimilar {
                let RuleValue::Text(s) = &rule.value else {
                    unreachable!()
                };
                ensure!(key_to_camelot(s).is_some(), "Invalid selected key");
            }
            regex.push(
                if matches!(
                    rule.operator,
                    Operator::StringRegExpMatch | Operator::StringNotRegExpMatch
                ) {
                    let RuleValue::Text(s) = &rule.value else {
                        unreachable!()
                    };
                    Some(Pattern::compile(s)?)
                } else {
                    None
                },
            );
        }
        Ok(Self {
            definition,
            catalog,
            regex,
        })
    }
    pub fn definition(&self) -> &SmartDefinition {
        &self.definition
    }
    pub fn matches(&self, track: &SmartTrack, context: &EvaluationContext) -> Result<bool> {
        let archived_rule = self
            .definition
            .rules
            .iter()
            .any(|r| r.field == "archived" && r.operator == Operator::BooleanTrue);
        if self.definition.rules.is_empty() || (track.archived && !archived_rule) {
            return Ok(false);
        }
        let mut values = Vec::with_capacity(self.definition.rules.len());
        for (rule, pattern) in self.definition.rules.iter().zip(&self.regex) {
            values.push(
                self.evaluate(rule, pattern.as_ref(), track, context)
                    .with_context(|| format!("Smart rule {}", rule.id))?,
            );
        }
        if !self.definition.match_all {
            return Ok(values.iter().any(|v| *v));
        }
        if self.definition.rules.iter().filter(|r| r.or).count() > 1 {
            Ok(values
                .iter()
                .zip(&self.definition.rules)
                .any(|(v, r)| *v && r.or)
                && values
                    .iter()
                    .zip(&self.definition.rules)
                    .all(|(v, r)| *v || r.or))
        } else {
            Ok(values.iter().all(|v| *v))
        }
    }
    fn evaluate(
        &self,
        rule: &SmartRule,
        pattern: Option<&Pattern>,
        track: &SmartTrack,
        context: &EvaluationContext,
    ) -> Result<bool> {
        use Operator::*;
        let field = &rule.field;
        let op = rule.operator;
        match family(op) {
            Family::Text => {
                let RuleValue::Text(query) = &rule.value else {
                    unreachable!()
                };
                if matches!(field.as_str(), "AnyTextField" | "EveryTextField") {
                    let mut values = Vec::new();
                    for (id, descriptor) in &self.catalog.fields {
                        if descriptor.kind == FieldKind::Text
                            && descriptor.any_text
                            && descriptor.unavailable_reason.is_none()
                        {
                            values.push(text::matches(track, id, op, query, pattern)?);
                        }
                    }
                    Ok(if field == "EveryTextField" {
                        values.iter().all(|v| *v)
                    } else {
                        values.iter().any(|v| *v)
                    })
                } else {
                    text::matches(track, field, op, query, pattern)
                }
            }
            Family::Number | Family::Bpm => numeric::matches(track, field, op, &rule.value),
            Family::Date => dates::matches(track, field, op, &rule.value, context),
            Family::Exists => {
                let value = track.value(field);
                let exists = match field.as_str() {
                    "tags" => track.tags.intersection(&self.catalog.tags).next().is_some(),
                    "waveform" => track.waveform,
                    "cuepoints" | "tempomarkers" | "cues" | "tempos" => true,
                    _ => match value {
                        FieldValue::Null => false,
                        FieldValue::Text(ref s) | FieldValue::Color(ref s) => {
                            !text::trim(s).is_empty()
                        }
                        FieldValue::Number(n)
                            if self.catalog.fields[field].kind == FieldKind::Number =>
                        {
                            n != 0.
                        }
                        _ => true,
                    },
                };
                Ok(if op == ExistsNot { !exists } else { exists })
            }
            Family::Boolean => {
                if field == "CustomHasLockedGenreCleanupGenre" && track.genres.is_empty() {
                    return Ok(false);
                }
                let value = match field.as_str() {
                    "CustomFileMissing" => match &track.file {
                        FileAvailability::Present => false,
                        FileAvailability::Missing => true,
                        FileAvailability::Unavailable(reason) => {
                            anyhow::bail!("File availability unknown: {reason}")
                        }
                    },
                    "CustomHasLockedGenreCleanupGenre" => track
                        .genres
                        .iter()
                        .any(|g| context.protected_genres.contains(g)),
                    _ => text::truthy(&track.value(field)),
                };
                Ok(if op == BooleanFalse { !value } else { value })
            }
            Family::Color => {
                let RuleValue::Color(query) = &rule.value else {
                    unreachable!()
                };
                let color = match track.value(field) {
                    FieldValue::Color(s) | FieldValue::Text(s) => Some(s),
                    FieldValue::Null => None,
                    _ => anyhow::bail!("Invalid color field"),
                };
                let absent = |s: &Option<String>| s.as_ref().is_none_or(|s| s.is_empty());
                let matched = (absent(query) && absent(&color)) || *query == color;
                Ok(if op == ColorNotEquals {
                    !matched
                } else {
                    matched
                })
            }
            Family::Tags | Family::Categories => {
                let RuleValue::Ids(ids) = &rule.value else {
                    unreachable!()
                };
                Ok(tags::matches(track, &self.catalog, op, ids))
            }
            Family::Tempo => {
                ensure!(
                    track.tempos.iter().all(|b| b.is_finite()),
                    "Nonfinite tempo marker"
                );
                let same = track
                    .tempos
                    .first()
                    .is_some_and(|first| track.tempos.iter().all(|b| b == first));
                Ok(if op == TempomarkerNotSameBpm {
                    track.tempos.len() > 1 && !same
                } else {
                    same
                })
            }
            Family::Cue => Ok(cues::matches(track, op, &rule.value)),
            Family::Membership => {
                let RuleValue::Playlist(reference) = &rule.value else {
                    unreachable!()
                };
                membership::matches(track, context, op, reference)
            }
            Family::Key => {
                let RuleValue::Text(key) = &rule.value else {
                    unreachable!()
                };
                Ok(track
                    .key
                    .as_deref()
                    .is_some_and(|k| compatible_keys(k, key, context.key_mixing_mode)))
            }
        }
    }
}
