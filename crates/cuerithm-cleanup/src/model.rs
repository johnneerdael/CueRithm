use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataField {
    Title,
    Artist,
    Album,
    AlbumArtist,
    Genre,
    Comment,
    Grouping,
    Label,
    Remixer,
    Mix,
    Producer,
    Composer,
    Lyricist,
}
impl MetadataField {
    pub const ALL: &[Self] = &[
        Self::Title,
        Self::Artist,
        Self::Album,
        Self::AlbumArtist,
        Self::Genre,
        Self::Comment,
        Self::Grouping,
        Self::Label,
        Self::Remixer,
        Self::Mix,
        Self::Producer,
        Self::Composer,
        Self::Lyricist,
    ];
    pub fn contributor(self) -> bool {
        matches!(
            self,
            Self::Artist
                | Self::AlbumArtist
                | Self::Remixer
                | Self::Producer
                | Self::Composer
                | Self::Lyricist
        )
    }
}
impl FromStr for MetadataField {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self> {
        Ok(serde_json::from_value(serde_json::Value::String(
            value.into(),
        ))?)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CustomTag {
    pub category: String,
    pub value: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub fields: BTreeMap<MetadataField, Vec<String>>,
    pub custom_tags: Vec<CustomTag>,
}
impl Metadata {
    pub fn first(&self, field: MetadataField) -> Option<&str> {
        self.fields.get(&field)?.first().map(String::as_str)
    }
    pub fn set(&mut self, field: MetadataField, values: Vec<String>) {
        if values.is_empty() {
            self.fields.remove(&field);
        } else {
            self.fields.insert(field, values);
        }
    }
    pub fn validate(&self) -> Result<()> {
        let mut bytes = 0usize;
        for values in self.fields.values() {
            ensure!(values.len() <= 128, "Too many values in one metadata field");
            for value in values {
                bytes = bytes.saturating_add(value.len());
            }
        }
        ensure!(
            bytes <= 1_048_576 && self.custom_tags.len() <= 4096,
            "Cleanup metadata is too large"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldChange {
    pub field: MetadataField,
    pub before: Vec<String>,
    pub after: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupReport {
    pub original: Metadata,
    pub metadata: Metadata,
    pub changes: Vec<FieldChange>,
    pub applied_rules: Vec<String>,
}
impl CleanupReport {
    pub fn changed(&self) -> bool {
        self.original != self.metadata
    }
}
fn title() -> MetadataField {
    MetadataField::Title
}
fn yes() -> bool {
    true
}
fn separator() -> String {
    " - ".into()
}
fn one() -> u16 {
    1
}
fn remix() -> String {
    "remix".into()
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SmartFix {
    TitleCase {
        #[serde(default = "title")]
        field: MetadataField,
        #[serde(default = "yes")]
        uppercase: bool,
        #[serde(default = "yes")]
        lowercase: bool,
    },
    ReplaceCharacters {
        characters: String,
    },
    DecodeEntities,
    ExtractArtist {
        #[serde(default = "separator")]
        separator: String,
        #[serde(default = "one")]
        part: u16,
    },
    ExtractRemixer {
        #[serde(default = "remix")]
        word: String,
        #[serde(default = "yes")]
        remove_title: bool,
    },
    RemoveGarbage {
        #[serde(default = "title")]
        field: MetadataField,
    },
    RemoveNumberPrefix,
    RemoveUrls {
        #[serde(default = "title")]
        field: MetadataField,
        #[serde(default)]
        clear_field: bool,
    },
    MixParentheses,
    RemoveCommonText {
        #[serde(default)]
        pattern: CommonTextPattern,
    },
    RemovePromotionalText {
        #[serde(default = "title")]
        field: MetadataField,
    },
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum CommonTextPattern {
    CamelotKeyFromTitle,
    #[default]
    OriginalMixFromTitle,
    CamelotKeyEnergyFromTitle,
    BpmFromTitle,
}
impl SmartFix {
    pub fn target(&self) -> MetadataField {
        match self {
            Self::TitleCase { field, .. }
            | Self::RemoveGarbage { field }
            | Self::RemoveUrls { field, .. }
            | Self::RemovePromotionalText { field } => *field,
            _ => MetadataField::Title,
        }
    }
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::ExtractArtist { separator, part } => {
                ensure!(
                    !separator.is_empty() && separator.chars().count() <= 64,
                    "Artist separator must contain 1 to 64 characters"
                );
                ensure!(
                    (1..=256).contains(part),
                    "Artist result number must be between 1 and 256"
                );
            }
            Self::ExtractRemixer { word, .. } => ensure!(
                !word.trim().is_empty() && word.chars().count() <= 64,
                "Remixer word must contain 1 to 64 characters"
            ),
            Self::ReplaceCharacters { characters } => ensure!(
                characters.chars().count() <= 64,
                "Too many replacement characters"
            ),
            _ => {}
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredFix {
    pub id: String,
    pub enabled: bool,
    #[serde(default)]
    pub ignored_values: BTreeSet<String>,
    pub rule: SmartFix,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalMapping {
    pub from: String,
    pub to: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenreConversion {
    pub category: String,
    pub genres: BTreeSet<String>,
    pub remove_genre: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GenreRules {
    pub enabled: bool,
    pub mappings: Vec<CanonicalMapping>,
    pub protected_values: BTreeSet<String>,
    pub conversion: Option<GenreConversion>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributorRole {
    Artist,
    Remixer,
    Producer,
    Composer,
    Lyricist,
}
impl ContributorRole {
    pub fn field(self) -> MetadataField {
        match self {
            Self::Artist => MetadataField::Artist,
            Self::Remixer => MetadataField::Remixer,
            Self::Producer => MetadataField::Producer,
            Self::Composer => MetadataField::Composer,
            Self::Lyricist => MetadataField::Lyricist,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ArtistRules {
    pub enabled: bool,
    pub roles: BTreeSet<ContributorRole>,
    pub mappings: Vec<CanonicalMapping>,
    pub protected_values: BTreeSet<String>,
}
impl Default for ArtistRules {
    fn default() -> Self {
        Self {
            enabled: false,
            roles: [ContributorRole::Artist].into(),
            mappings: Vec::new(),
            protected_values: BTreeSet::new(),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CleanupConfig {
    pub fixes: Vec<ConfiguredFix>,
    pub genres: GenreRules,
    pub artists: ArtistRules,
}
impl CleanupConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.fixes.len() <= 64, "Too many cleanup rules");
        let mut ids = BTreeSet::new();
        for fix in &self.fixes {
            ensure!(
                !fix.id.trim().is_empty() && fix.id.len() <= 128 && ids.insert(&fix.id),
                "Invalid or duplicate cleanup rule ID"
            );
            validate_protected(&fix.ignored_values)?;
            if fix.enabled {
                fix.rule.validate()?;
            }
        }
        validate_protected(&self.genres.protected_values)?;
        validate_protected(&self.artists.protected_values)?;
        if self.genres.enabled {
            crate::mappings::validate(&self.genres.mappings)?;
            if let Some(conversion) = &self.genres.conversion {
                ensure!(
                    !conversion.category.trim().is_empty() && conversion.category.len() <= 256,
                    "Invalid genre tag category"
                );
                ensure!(conversion.genres.len() <= 4096, "Too many converted genres");
            }
        }
        if self.artists.enabled {
            ensure!(
                !self.artists.roles.is_empty(),
                "Select at least one contributor role"
            );
            crate::mappings::validate(&self.artists.mappings)?;
        }
        Ok(())
    }
    pub(crate) fn protects_text(&self, field: MetadataField, value: &str) -> bool {
        let rules = if field == MetadataField::Genre {
            Some((
                self.genres.enabled,
                &self.genres.mappings,
                &self.genres.protected_values,
            ))
        } else if field.contributor() {
            Some((
                self.artists.enabled && self.artists.roles.iter().any(|role| role.field() == field),
                &self.artists.mappings,
                &self.artists.protected_values,
            ))
        } else {
            None
        };
        rules.is_some_and(|(enabled, mappings, protected)| {
            protected.contains(value)
                || (enabled
                    && mappings.iter().any(|m| {
                        m.from == value || m.to.as_ref().is_some_and(|s| s.trim() == value)
                    }))
        })
    }
}
fn validate_protected(values: &BTreeSet<String>) -> Result<()> {
    ensure!(values.len() <= 4096, "Too many protected values");
    for value in values {
        ensure!(
            !value.trim().is_empty() && value.len() <= 4096,
            "Invalid protected value"
        );
    }
    Ok(())
}
