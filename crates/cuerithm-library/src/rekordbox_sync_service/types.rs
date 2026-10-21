use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RekordboxSyncTarget {
    pub id: String,
    pub installation: TargetInstallation,
    pub database: PathBuf,
    pub options_file: PathBuf,
    pub asset_root: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case", deny_unknown_fields)]
pub enum RekordboxSyncScope {
    Full,
    Playlist { playlist_id: String },
    Changed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RekordboxSyncOptions {
    pub metadata_fields: BTreeSet<String>,
    pub cues: bool,
    pub beat_grid: bool,
    pub artwork: bool,
    pub playlists: bool,
    pub my_tags: bool,
    pub materialize_smart_playlists: bool,
    pub convert_incompatible_audio: bool,
    pub nearest_color: bool,
}
impl Default for RekordboxSyncOptions {
    fn default() -> Self {
        Self {
            metadata_fields: [
                "title",
                "artist",
                "album",
                "album_artist",
                "genre",
                "label",
                "comment",
                "remixer",
                "composer",
                "lyricist",
                "mix_name",
                "key",
                "bpm",
                "rating",
                "release_year",
                "release_date",
                "track_number",
                "disc_number",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            cues: true,
            beat_grid: true,
            artwork: true,
            playlists: true,
            my_tags: true,
            materialize_smart_playlists: false,
            convert_incompatible_audio: false,
            nearest_color: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RekordboxSyncRequest {
    pub target_id: String,
    pub scope: RekordboxSyncScope,
    pub options: RekordboxSyncOptions,
    #[serde(default)]
    pub decisions: BTreeMap<String, BTreeMap<String, NativeFieldDecision>>,
}
/// Runtime resources are deliberately never serialized in a plan or settings.
pub struct RekordboxSyncRuntime {
    pub discovery_paths: DiscoveryPaths,
    pub media_runtime: Option<cuerithm_media::MediaRuntime>,
}
impl RekordboxSyncRuntime {
    pub fn current(media: Option<cuerithm_media::MediaRuntime>) -> Result<Self> {
        Ok(Self {
            discovery_paths: DiscoveryPaths::current()?,
            media_runtime: media,
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RekordboxSyncPreflight {
    pub target: RekordboxSyncTarget,
    pub readable: bool,
    pub writable: bool,
    pub restrictions: Vec<String>,
    pub schema_fingerprint: Option<String>,
    pub identity: Option<DatabaseIdentity>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RekordboxSyncFieldReview {
    pub track_id: String,
    pub title: String,
    pub field: String,
    pub local: NativeFieldObservation,
    pub remote: NativeFieldObservation,
    pub result: NativeReconciliation,
    pub decision: Option<NativeFieldDecision>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RekordboxSyncPreview {
    pub id: String,
    pub target_id: String,
    pub library_revision: u64,
    pub fields: Vec<RekordboxSyncFieldReview>,
    pub tracks: usize,
    pub changed_rows: usize,
    pub skipped_fields: usize,
    pub warnings: Vec<String>,
    pub can_apply: bool,
    pub media_copies: usize,
    pub copied_audio_bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RekordboxSyncOutcome {
    pub id: String,
    pub status: String,
    pub tracks: usize,
    pub changed_rows: usize,
    pub acknowledged_fields: usize,
    pub skipped_fields: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RekordboxSyncHistoryEntry {
    pub id: String,
    pub target_id: String,
    pub status: String,
    pub created: i64,
    pub updated: i64,
    pub outcome: Option<RekordboxSyncOutcome>,
}
