use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Operator {
    StringContains,
    StringNotContains,
    StringEquals,
    StringNotEquals,
    StringStartsWith,
    StringNotStartsWith,
    StringEndsWith,
    StringNotEndsWith,
    StringRegExpMatch,
    StringNotRegExpMatch,
    NumberEquals,
    NumberNotEquals,
    NumberGreaterThan,
    NumberGreaterThanEquals,
    NumberLessThan,
    NumberLessThanEquals,
    NumberBetween,
    DateEquals,
    DateNotEquals,
    DateAfter,
    DateBefore,
    DateRecent,
    DateNotRecent,
    DateBetween,
    ColorEquals,
    ColorNotEquals,
    BooleanTrue,
    BooleanFalse,
    TagsIncludes,
    TagsNotIncludes,
    TagsAtleastOne,
    TagsExact,
    TagCategoryNone,
    TagCategoryAtleastOneAnyCat,
    TagCategoryAtleastOneAllCAt,
    Exists,
    ExistsNot,
    TempomarkerSameBpm,
    TempomarkerNotSameBpm,
    BpmExactNumber,
    BpmNotExactNumber,
    PlaylistInSpecificPlaylist,
    PlaylistNotInSpecificPlaylist,
    PlaylistInSpecificPlaylistFolder,
    PlaylistNotInSpecificPlaylistFolder,
    PlaylistInSpecificSmartlist,
    PlaylistNotInSpecificSmartlist,
    CuepointAnyLabelIncludes,
    CuepointAnyColorEquals,
    CuepointNoLabelIncludes,
    CuepointNoColorEquals,
    CuepointHasLoop,
    CuepointHasNoLoop,
    CuepointHasActiveLoop,
    CuepointHasNoActiveLoop,
    CuepointHasMemoryCue,
    CuepointHasNoMemoryCue,
    KeySimilar,
}
impl Operator {
    pub const ALL: &'static [Self] = &[
        Self::StringContains,
        Self::StringNotContains,
        Self::StringEquals,
        Self::StringNotEquals,
        Self::StringStartsWith,
        Self::StringNotStartsWith,
        Self::StringEndsWith,
        Self::StringNotEndsWith,
        Self::StringRegExpMatch,
        Self::StringNotRegExpMatch,
        Self::NumberEquals,
        Self::NumberNotEquals,
        Self::NumberGreaterThan,
        Self::NumberGreaterThanEquals,
        Self::NumberLessThan,
        Self::NumberLessThanEquals,
        Self::NumberBetween,
        Self::DateEquals,
        Self::DateNotEquals,
        Self::DateAfter,
        Self::DateBefore,
        Self::DateRecent,
        Self::DateNotRecent,
        Self::DateBetween,
        Self::ColorEquals,
        Self::ColorNotEquals,
        Self::BooleanTrue,
        Self::BooleanFalse,
        Self::TagsIncludes,
        Self::TagsNotIncludes,
        Self::TagsAtleastOne,
        Self::TagsExact,
        Self::TagCategoryNone,
        Self::TagCategoryAtleastOneAnyCat,
        Self::TagCategoryAtleastOneAllCAt,
        Self::Exists,
        Self::ExistsNot,
        Self::TempomarkerSameBpm,
        Self::TempomarkerNotSameBpm,
        Self::BpmExactNumber,
        Self::BpmNotExactNumber,
        Self::PlaylistInSpecificPlaylist,
        Self::PlaylistNotInSpecificPlaylist,
        Self::PlaylistInSpecificPlaylistFolder,
        Self::PlaylistNotInSpecificPlaylistFolder,
        Self::PlaylistInSpecificSmartlist,
        Self::PlaylistNotInSpecificSmartlist,
        Self::CuepointAnyLabelIncludes,
        Self::CuepointAnyColorEquals,
        Self::CuepointNoLabelIncludes,
        Self::CuepointNoColorEquals,
        Self::CuepointHasLoop,
        Self::CuepointHasNoLoop,
        Self::CuepointHasActiveLoop,
        Self::CuepointHasNoActiveLoop,
        Self::CuepointHasMemoryCue,
        Self::CuepointHasNoMemoryCue,
        Self::KeySimilar,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateUnit {
    Days,
    Weeks,
    Months,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyMixingMode {
    Harmonic,
    Fuzzy,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistKind {
    Playlist,
    Folder,
    Smartlist,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaylistReference {
    pub id: String,
    pub kind: PlaylistKind,
    #[serde(default)]
    pub path: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RuleValue {
    None,
    Text(String),
    Number(f64),
    NumberRange { start: f64, end: f64 },
    Date(i64),
    DateRange { start: i64, end: i64 },
    Relative { count: i64, unit: DateUnit },
    Color(Option<String>),
    Ids(Vec<String>),
    Playlist(PlaylistReference),
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartRule {
    pub id: String,
    pub field: String,
    pub operator: Operator,
    pub value: RuleValue,
    #[serde(default)]
    pub or: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartDefinition {
    pub match_all: bool,
    pub rules: Vec<SmartRule>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FieldValue {
    Null,
    Text(String),
    Number(f64),
    Date(i64),
    Boolean(bool),
    Color(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    Text,
    Number,
    Date,
    Boolean,
    Color,
    Tags,
    Categories,
    Cues,
    Tempos,
    Playlist,
    TextGroup,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDescriptor {
    pub label: String,
    pub kind: FieldKind,
    pub any_text: bool,
    pub unavailable_reason: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldCatalog {
    pub fields: BTreeMap<String, FieldDescriptor>,
    pub tags: BTreeSet<String>,
    pub categories: BTreeMap<String, BTreeSet<String>>,
    pub playlists: BTreeMap<String, PlaylistKind>,
}
impl FieldCatalog {
    pub fn standard() -> Self {
        let mut c = Self::default();
        for (kind, ids) in [
            (
                FieldKind::Text,
                &[
                    "title",
                    "artist",
                    "album",
                    "albumArtist",
                    "genre",
                    "comment",
                    "grouping",
                    "label",
                    "remixer",
                    "mix",
                    "producer",
                    "composer",
                    "lyricist",
                    "key",
                    "location",
                    "fileType",
                    "extra1",
                    "extra2",
                    "isrc",
                    "catalogNumber",
                ] as &[_],
            ),
            (
                FieldKind::Number,
                &[
                    "bpm",
                    "duration",
                    "year",
                    "energy",
                    "rating",
                    "bitrate",
                    "sizeBytes",
                    "sampleRate",
                    "trackNumber",
                    "cuepoints",
                    "tempomarkers",
                ],
            ),
            (
                FieldKind::Date,
                &["dateAdded", "dateModified", "releaseDate"],
            ),
            (
                FieldKind::Boolean,
                &[
                    "archived",
                    "incoming",
                    "waveform",
                    "IsAnalyzed",
                    "CustomFileMissing",
                    "CustomHasLockedGenreCleanupGenre",
                ],
            ),
            (FieldKind::Color, &["color"]),
            (FieldKind::Tags, &["tags"]),
            (FieldKind::Categories, &["categories"]),
            (FieldKind::Cues, &["cues"]),
            (FieldKind::Tempos, &["tempos"]),
            (FieldKind::Playlist, &["playlist"]),
            (FieldKind::TextGroup, &["AnyTextField", "EveryTextField"]),
        ] {
            for id in ids {
                c.fields.insert(
                    (*id).into(),
                    FieldDescriptor {
                        label: (*id).into(),
                        kind,
                        any_text: kind == FieldKind::Text
                            && !matches!(*id, "location" | "fileType"),
                        unavailable_reason: None,
                    },
                );
            }
        }
        c
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Cue,
    Loop,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCue {
    pub label: Option<String>,
    pub color: Option<String>,
    pub kind: CueKind,
    pub active_loop: bool,
    pub memory: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reason", rename_all = "snake_case")]
pub enum FileAvailability {
    Present,
    Missing,
    Unavailable(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartTrack {
    pub id: String,
    pub fields: BTreeMap<String, FieldValue>,
    pub tags: BTreeSet<String>,
    pub genres: Vec<String>,
    pub cues: Vec<SmartCue>,
    pub tempos: Vec<f64>,
    pub key: Option<String>,
    pub archived: bool,
    pub waveform: bool,
    pub file: FileAvailability,
}
impl SmartTrack {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            fields: Default::default(),
            tags: Default::default(),
            genres: vec![],
            cues: vec![],
            tempos: vec![],
            key: None,
            archived: false,
            waveform: false,
            file: FileAvailability::Unavailable("File availability has not been checked".into()),
        }
    }
    pub(crate) fn value(&self, field: &str) -> FieldValue {
        match field {
            "archived" => FieldValue::Boolean(self.archived),
            "waveform" => FieldValue::Boolean(self.waveform),
            "key" => self
                .key
                .clone()
                .map(FieldValue::Text)
                .unwrap_or(FieldValue::Null),
            "cuepoints" => FieldValue::Number(self.cues.len() as f64),
            "tempomarkers" => FieldValue::Number(self.tempos.len() as f64),
            "IsAnalyzed" => FieldValue::Boolean(!self.tempos.is_empty()),
            _ => self.fields.get(field).cloned().unwrap_or(FieldValue::Null),
        }
    }
}
