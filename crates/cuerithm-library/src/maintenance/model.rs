use crate::SmartStamp;
pub use cuerithm_maintenance::Tolerance as DuplicateTolerance;
pub use cuerithm_maintenance::{GroupEdit, Preference, ReviewGroup, ReviewState};
use cuerithm_media::AudioFingerprint;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateReview {
    pub id: String,
    pub scan_id: String,
    pub created: i64,
    pub revision: u64,
    pub stamp: MaintenanceStamp,
    pub groups: Vec<ReviewGroup>,
    pub stale: bool,
    pub stale_reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DuplicateReviewEdit {
    Group {
        group_id: String,
        edit: GroupEdit,
    },
    Prefer {
        preference: Preference,
        override_locked: bool,
        use_best_cues: bool,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgnoredDuplicateTrack {
    pub id: String,
    pub display_path: String,
    pub title: String,
    pub artist: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateIgnores {
    pub stamp: MaintenanceStamp,
    pub tracks: Vec<IgnoredDuplicateTrack>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DuplicatePlaylistChange {
    pub id: String,
    pub before: Vec<String>,
    pub after: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateMergePreview {
    pub id: String,
    pub review_id: String,
    pub review_revision: u64,
    pub created: i64,
    pub stamp: MaintenanceStamp,
    pub plan: cuerithm_maintenance::MergePlan,
    pub playlists: Vec<DuplicatePlaylistChange>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateMergeResult {
    pub id: String,
    pub preview_id: String,
    pub stamp: MaintenanceStamp,
    pub archived_ids: Vec<String>,
    pub restored_ids: Vec<String>,
    pub affected_playlists: Vec<String>,
    pub undo_of: Option<String>,
    #[serde(default)]
    pub updated_ids: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateMergeHistory {
    pub result: DuplicateMergeResult,
    pub created: i64,
    pub undone_by: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateScanScope {
    #[default]
    FullLibrary,
    SelectedPlaylists,
    FullLibraryFiltered,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DuplicateScanRequest {
    pub scope: DuplicateScanScope,
    pub playlist_ids: Vec<String>,
    pub tolerance: DuplicateTolerance,
    pub ignored_paths: Vec<PathBuf>,
}
impl Default for DuplicateScanRequest {
    fn default() -> Self {
        Self {
            scope: DuplicateScanScope::FullLibrary,
            playlist_ids: vec![],
            tolerance: DuplicateTolerance::None,
            ignored_paths: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MaintenanceStamp {
    pub inputs: SmartStamp,
    pub maintenance_revision: u64,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceFileState {
    Present,
    Missing,
    Unavailable,
    Symlink,
    NotRegular,
    Changed,
    Video,
    NoAudio,
    DecodeError,
    Unsupported,
    TimedOut,
    DurationSkipped,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateScanTrack {
    pub id: String,
    pub display_path: String,
    pub title: String,
    pub artist: String,
    pub archived: bool,
    pub state: MaintenanceFileState,
    pub detail: Option<String>,
    pub observed_sha256: Option<String>,
    pub source_changed: bool,
    pub fingerprint: Option<AudioFingerprint>,
    pub cached: bool,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateMatchMethod {
    Fingerprint,
    ArtistTitle,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateScanGroup {
    pub id: String,
    pub method: DuplicateMatchMethod,
    pub track_ids: Vec<String>,
    pub suggested_keeper: Option<String>,
    pub suggested_tags_from: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateScanReport {
    pub id: String,
    pub created: i64,
    pub stamp: MaintenanceStamp,
    pub request: DuplicateScanRequest,
    pub stale: bool,
    pub stale_reason: Option<String>,
    pub tracks: Vec<DuplicateScanTrack>,
    pub groups: Vec<DuplicateScanGroup>,
    pub cache_hits: usize,
    pub fingerprints_computed: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateScanProgress {
    pub completed: usize,
    pub total: usize,
    pub track_id: Option<String>,
    pub cached: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionMode {
    All,
    Include,
    Exclude,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedScanRequest {
    pub root: PathBuf,
    pub extension_mode: ExtensionMode,
    pub extensions: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedFile {
    pub id: String,
    pub display_path: String,
    pub size_bytes: u64,
    pub source_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanIssue {
    pub display_path: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedScanReport {
    pub id: String,
    pub created: i64,
    pub stamp: MaintenanceStamp,
    pub request: UnusedScanRequest,
    pub files: Vec<UnusedFile>,
    pub size_bytes: u64,
    pub complete: bool,
    pub issues: Vec<ScanIssue>,
    pub stale: bool,
    pub stale_reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnusedRemovalRole {
    Selected,
    Sibling,
    FolderMetadata,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnusedRemovalFile {
    pub id: String,
    pub display_path: String,
    pub size_bytes: u64,
    pub source_sha256: String,
    pub role: UnusedRemovalRole,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedRemovalPreview {
    #[serde(default)]
    pub directories: Vec<String>,
    pub id: String,
    pub scan_id: String,
    pub stamp: MaintenanceStamp,
    pub files: Vec<UnusedRemovalFile>,
    pub size_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedRemovalResult {
    #[serde(default)]
    pub directories: Vec<String>,
    pub id: String,
    pub preview_id: String,
    pub stamp: MaintenanceStamp,
    pub files: Vec<UnusedRemovalFile>,
    pub size_bytes: u64,
    pub restored: bool,
    pub undo_of: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedRemovalHistory {
    pub restore_available: bool,
    pub result: UnusedRemovalResult,
    pub state: String,
    pub error: Option<String>,
    pub undone_by: Option<String>,
    pub cleanup_pending: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScanProgress {
    pub completed: usize,
    pub total: Option<usize>,
    pub current_path: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BrokenScanRequest {
    pub inspect_audio: bool,
    /// Archived records are library records too; repairing them needs them listed.
    pub include_archived: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenScanTrack {
    #[serde(default)]
    pub archived: bool,
    pub id: String,
    pub display_path: String,
    pub state: MaintenanceFileState,
    pub detail: Option<String>,
    pub observed_sha256: Option<String>,
    pub source_changed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenScanReport {
    pub id: String,
    pub created: i64,
    pub stamp: MaintenanceStamp,
    pub request: BrokenScanRequest,
    #[serde(default)]
    pub decoder_identity: Option<String>,
    pub tracks: Vec<BrokenScanTrack>,
    pub stale: bool,
    pub stale_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateRecovery {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub created: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedBackupFile {
    pub operation_id: String,
    pub file_id: String,
    pub display_source_path: String,
    pub size_bytes: u64,
    pub source_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedBackupPreview {
    pub id: String,
    pub operation_ids: Vec<String>,
    pub files: Vec<UnusedBackupFile>,
    pub size_bytes: u64,
    pub restores_lost: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnusedBackupDisposal {
    pub id: String,
    pub preview_id: String,
    pub operation_ids: Vec<String>,
    pub file_count: usize,
    pub size_bytes: u64,
    pub state: String,
    pub cleanup_pending: bool,
    pub error: Option<String>,
}
