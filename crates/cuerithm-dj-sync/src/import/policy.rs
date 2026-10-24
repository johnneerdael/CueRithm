use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldGroup {
    DescriptiveTags,
    Key,
    Bpm,
    Beatgrid,
    HotCues,
    Markers,
    MemoryCues,
    MainCues,
    LoadCues,
    Loops,
    Ratings,
    Color,
    History,
    Energy,
    Artwork,
    CustomTags,
    Collections,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisField {
    Key,
    Bpm,
    Beatgrid,
    Energy,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisAdoption {
    FillMissing,
    ReplaceSelected,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueGeneration {
    #[default]
    Off,
    Generate,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RelocationPolicy {
    #[default]
    Off,
    RenameInPlace {
        rule: String,
    },
    Move {
        root_id: String,
        rule: String,
    },
    Copy {
        root_id: String,
        rule: String,
    },
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EnrichmentPolicy {
    pub enabled: bool,
    pub providers: Vec<String>,
    pub fields: BTreeSet<String>,
    /// Opaque references into the existing credential store, never credentials.
    pub credential_refs: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CleanupPolicy {
    pub enabled: bool,
    pub rules: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportPolicy {
    pub version: u32,
    pub selected_fields: BTreeSet<FieldGroup>,
    /// None means every descriptive field; Some(empty) means none.
    pub descriptive_fields: Option<BTreeSet<String>>,
    pub metadata_enrichment: EnrichmentPolicy,
    pub cleanup: CleanupPolicy,
    pub analysis_fields: BTreeMap<AnalysisField, AnalysisAdoption>,
    pub cue_generation: CueGeneration,
    pub replace_hotcues: bool,
    pub write_audio_tags: BTreeSet<String>,
    pub relocation: RelocationPolicy,
    pub capture_file_tags: bool,
    pub include_history: bool,
    pub include_queue: bool,
    pub allow_content_matching: bool,
}
impl Default for ImportPolicy {
    fn default() -> Self {
        Self {
            version: 1,
            selected_fields: [
                FieldGroup::DescriptiveTags,
                FieldGroup::Key,
                FieldGroup::Bpm,
                FieldGroup::Beatgrid,
                FieldGroup::HotCues,
                FieldGroup::Markers,
                FieldGroup::MemoryCues,
                FieldGroup::MainCues,
                FieldGroup::LoadCues,
                FieldGroup::Loops,
                FieldGroup::Ratings,
                FieldGroup::Color,
                FieldGroup::Energy,
                FieldGroup::Artwork,
                FieldGroup::CustomTags,
                FieldGroup::Collections,
            ]
            .into(),
            descriptive_fields: None,
            metadata_enrichment: Default::default(),
            cleanup: Default::default(),
            analysis_fields: Default::default(),
            cue_generation: Default::default(),
            replace_hotcues: false,
            write_audio_tags: Default::default(),
            relocation: Default::default(),
            capture_file_tags: false,
            include_history: false,
            include_queue: false,
            allow_content_matching: false,
        }
    }
}
impl ImportPolicy {
    pub fn requires_processing(&self) -> bool {
        self.metadata_enrichment.enabled
            || self.cleanup.enabled
            || !self.analysis_fields.is_empty()
            || self.cue_generation != CueGeneration::Off
            || !self.write_audio_tags.is_empty()
            || self.relocation != RelocationPolicy::Off
    }
}
