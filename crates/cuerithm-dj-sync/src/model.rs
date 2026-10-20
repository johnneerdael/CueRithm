use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DjSoftware {
    VirtualDj,
    EngineDj,
    DjayPro,
    Serato4,
    PioneerUsb,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncLibrary {
    pub tracks: Vec<SyncTrack>,
    pub playlists: Vec<SyncPlaylist>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncTrack {
    pub id: String,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub label: String,
    pub comment: String,
    pub key: String,
    pub bpm: f64,
    pub duration_seconds: f64,
    pub sample_rate: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_timing: Option<cuerithm_media::SourceExportTiming>,
    /// Present only on a converted export copy; saved publications stay canonical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_timeline: Option<AppliedTiming>,
    /// Encoded JPEG cover image for targets that store their own artwork.
    pub artwork: Option<Vec<u8>>,
    pub rating: u8,
    pub year: Option<u16>,
    pub color: Option<u32>,
    pub cues: Vec<SyncCue>,
    pub beatgrid: Vec<SyncBeat>,
    /// Saved source metadata used by native rules and their matching track fields.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub attributes: std::collections::BTreeMap<String, cuerithm_smartlists::FieldValue>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncCue {
    pub index: u8,
    pub name: String,
    pub start_seconds: f64,
    pub end_seconds: Option<f64>,
    pub color: Option<u32>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// Sparse tempo segment anchors, each at a downbeat; never a dense list of beats.
pub struct SyncBeat {
    pub seconds: f64,
    pub bpm: f64,
    pub beat: u32,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncPlaylist {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smart: Option<cuerithm_smartlists::SmartDefinition>,
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub folder: bool,
    pub track_ids: Vec<String>,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeatgridLock {
    #[default]
    Default,
    Lock,
    Unlock,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncOptions {
    pub smart_context: Option<SmartExportContext>,
    pub materialize_smartlists: bool,
    pub engine_copy_audio: bool,
    pub key_notation: KeyNotation,
    /// Destination field to source field; all mappings read the original track.
    pub field_mappings: std::collections::BTreeMap<SyncTextField, SyncTextField>,
    pub usb_cue_destination: UsbCueDestination,
    pub usb_cdj_compatibility: UsbCompatibility,
    pub usb_cleanup_unused: bool,
    pub skip_beatgrids: bool,
    pub nearest_color: bool,
    pub beatgrid_lock: BeatgridLock,
    pub add_album_art: bool,
    pub extra_cue_markers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartExportContext {
    pub time_zone: String,
    pub native_time_zone: String,
    pub key_mixing_mode: cuerithm_smartlists::KeyMixingMode,
}

impl SyncTrack {
    pub fn attribute_text(&self, field: &str) -> Option<&str> {
        match self.attributes.get(field) {
            Some(cuerithm_smartlists::FieldValue::Text(value)) => Some(value),
            _ => None,
        }
    }
    pub fn attribute_number(&self, field: &str) -> Option<f64> {
        match self.attributes.get(field) {
            Some(cuerithm_smartlists::FieldValue::Number(value)) if value.is_finite() => {
                Some(*value)
            }
            _ => None,
        }
    }
    pub fn attribute_date(&self, field: &str) -> Option<i64> {
        match self.attributes.get(field) {
            Some(cuerithm_smartlists::FieldValue::Date(value)) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbCueDestination {
    #[default]
    Hot,
    Memory,
    Both,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyNotation {
    #[default]
    Original,
    Musical,
    Camelot,
    OpenKey,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncTextField {
    Title,
    Artist,
    Album,
    Genre,
    Label,
    Comment,
    Key,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsbCompatibility {
    #[default]
    Off,
    Detect,
    AutoFix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedTiming {
    pub target: DjSoftware,
    pub offset_seconds: f64,
    pub profile_sha256: String,
}
