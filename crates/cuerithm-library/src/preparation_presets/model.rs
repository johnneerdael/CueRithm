use camelot_core::lexicon_cues::LexiconConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetRules {
    pub cue_generation: LexiconConfig,
    pub autotag_enabled: bool,
    pub tagging: Option<PreparationPresetTagging>,
    pub cleanup_enabled: bool,
    pub cleanup: Option<cuerithm_cleanup::CleanupConfig>,
    pub organization: PreparationPresetOrganization,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetOrganization {
    pub mode: crate::OrganizationSettingMode,
    pub pattern: String,
    pub missing: crate::OrganizationMissingPolicy,
    pub collision: crate::OrganizationCollisionPolicy,
    pub organization_optional: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetConnections {
    pub destination_root_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetTagging {
    pub options: PreparationPresetTaggingOptions,
    pub no_match: String,
    pub analysis_tag_source: String,
    pub write_energy: bool,
    pub required_tags: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PreparationPresetTaggingOptions {
    pub platforms: Vec<String>,
    pub tags: Vec<String>,
    pub separators: PreparationPresetSeparators,
    pub id3v24: bool,
    pub overwrite: bool,
    pub overwrite_tags: Vec<String>,
    pub strictness: f64,
    pub merge_genres: bool,
    pub album_art_file: bool,
    pub camelot: bool,
    pub parse_filename: bool,
    pub filename_template: Option<String>,
    pub short_title: bool,
    pub match_duration: bool,
    pub max_duration_difference: u64,
    pub match_by_id: bool,
    pub multiple_matches: String,
    pub styles_options: String,
    pub styles_custom_tag: Option<PreparationPresetFrameName>,
    pub track_number_leading_zeroes: usize,
    pub skip_tagged: bool,
    pub only_year: bool,
    pub title_regex: Option<String>,
    pub capitalize_genres: bool,
    pub id3_comm_lang: Option<String>,
    pub remove_all_covers: bool,
    pub multiplatform: bool,
    pub fetch_all_results: bool,
    pub album_tagging: bool,
    pub album_tagging_ratio: f64,
    pub cover_filename: Option<String>,
    pub custom: PreparationPresetProviderOptions,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetSeparators {
    pub id3: String,
    pub vorbis: Option<String>,
    pub mp4: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetFrameName {
    pub id3: String,
    pub vorbis: String,
    pub mp4: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetProviderOptions {
    pub beatport: Option<PreparationPresetBeatportOptions>,
    pub traxsource: Option<PreparationPresetTraxsourceOptions>,
    pub musicbrainz: Option<PreparationPresetNoOptions>,
    pub sonovault: Option<PreparationPresetNoOptions>,
    pub discogs: Option<PreparationPresetNoOptions>,
    pub itunes: Option<PreparationPresetNoOptions>,
    pub bpmsupreme: Option<PreparationPresetNoOptions>,
    pub bandcamp: Option<PreparationPresetNoOptions>,
    pub deezer: Option<PreparationPresetNoOptions>,
    pub volumo: Option<PreparationPresetNoOptions>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetBeatportOptions {
    pub max_pages: u64,
    pub art_resolution: u64,
    pub ignore_version: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetTraxsourceOptions {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationPresetIssue {
    pub code: String,
    pub field: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationPreset {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub rules: PreparationPresetRules,
    pub connections: PreparationPresetConnections,
    pub status: String,
    pub issues: Vec<PreparationPresetIssue>,
    pub assigned_folder_count: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationPresetSummary {
    pub id: String,
    pub revision: String,
    pub name: String,
    pub status: String,
    pub issues: Vec<PreparationPresetIssue>,
    pub assigned_folder_count: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetSave {
    pub id: Option<String>,
    pub expected_revision: Option<String>,
    pub name: String,
    pub rules: PreparationPresetRules,
    pub connections: PreparationPresetConnections,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationPresetProjection {
    pub rules: PreparationPresetRules,
    pub connections: PreparationPresetConnections,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationPresetExport {
    pub document: String,
    pub warnings: Vec<PreparationPresetIssue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationPresetImportPreview {
    pub name: String,
    pub rules: PreparationPresetRules,
    pub status: String,
    pub issues: Vec<PreparationPresetIssue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreparationFolderChoice {
    Inherit,
    Global,
    Preset { preset_id: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationFolderChange {
    pub folder_id: String,
    pub expected_revision: Option<String>,
    pub choice: PreparationFolderChoice,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationFolderAssignment {
    pub folder_id: String,
    pub display_path: String,
    pub revision: Option<String>,
    pub choice: PreparationFolderChoice,
    pub effective_preset_id: Option<String>,
    pub effective_preset_revision: Option<String>,
    pub matched_folder_id: Option<String>,
    pub status: String,
    pub issues: Vec<PreparationPresetIssue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationPresetCapture {
    pub preset_id: Option<String>,
    pub preset_revision: Option<String>,
    pub preset_name: Option<String>,
    pub assignment_id: Option<String>,
    pub assignment_revision: Option<String>,
    pub base_settings_revision: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparationPresetNoOptions {}
