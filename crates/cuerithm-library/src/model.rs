use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub type TrackId = String;
pub type JobId = String;
pub type SettingsRevision = String;
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LibraryCounts {
    pub tracks: u64,
    pub incoming: u64,
    pub archive: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackView {
    #[default]
    Tracks,
    Incoming,
    Archive,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackSort {
    #[default]
    Title,
    Artist,
    Bpm,
    Energy,
    Added,
    UserRating,
    PlaylistOrder,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackQuery {
    pub view: TrackView,
    pub search: String,
    pub sort: TrackSort,
    pub descending: bool,
    pub limit: u32,
    pub cursor: Option<String>,
    pub playlist_id: Option<String>,
    pub curation: Option<crate::CurationFilter>,
    pub expected_revision: Option<u64>,
}
impl Default for TrackQuery {
    fn default() -> Self {
        Self {
            view: Default::default(),
            search: String::new(),
            sort: Default::default(),
            descending: false,
            limit: 100,
            cursor: None,
            playlist_id: None,
            curation: None,
            expected_revision: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_state: Option<crate::ImportedItemState>,
    pub id: TrackId,
    #[serde(default)]
    pub source_sha256: String,
    #[serde(default)]
    pub metadata: std::collections::BTreeMap<String, serde_json::Value>,
    pub display_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub state: String,
    #[serde(default)]
    pub archived: bool,
    pub key: Option<String>,
    pub bpm: Option<f64>,
    pub energy: Option<u8>,
    pub duration: Option<f64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackPage {
    pub rows: Vec<TrackSummary>,
    pub total: u64,
    pub revision: u64,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub id: JobId,
    pub track_id: TrackId,
    pub title: String,
    pub state: String,
    pub stage: String,
    pub error: Option<String>,
    pub attempts: u32,
}
#[derive(Clone)]
pub struct JobRun {
    pub attempt: u32,
    pub id: JobId,
    pub track_id: TrackId,
    pub path: PathBuf,
    pub source_sha256: String,
    pub settings_revision: SettingsRevision,
    pub snapshot: crate::SettingsSnapshot,
}

pub(crate) fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let mut out = b"unix:".to_vec();
        out.extend(path.as_os_str().as_bytes());
        out
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let mut out = b"windows:".to_vec();
        for unit in path.as_os_str().encode_wide() {
            out.extend(unit.to_le_bytes());
        }
        out
    }
}
pub(crate) fn bytes_path(bytes: &[u8]) -> Result<PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        ensure!(
            bytes.starts_with(b"unix:"),
            "Library paths need platform mapping"
        );
        Ok(std::ffi::OsString::from_vec(bytes[5..].to_vec()).into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        ensure!(
            bytes.starts_with(b"windows:") && (bytes.len() - 8).is_multiple_of(2),
            "Library paths need platform mapping"
        );
        let units: Vec<u16> = bytes[8..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .collect();
        Ok(std::ffi::OsString::from_wide(&units).into())
    }
}
pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
