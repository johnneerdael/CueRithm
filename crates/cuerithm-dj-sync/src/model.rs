use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DjSoftware {
    VirtualDj,
    EngineDj,
    DjayPro,
    Serato4,
    PioneerUsb,
    Rekordbox,
    Mixxx,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncLibrary {
    pub tracks: Vec<SyncTrack>,
    pub playlists: Vec<SyncPlaylist>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncTrack {
    pub id: String,
    #[serde(with = "crate::path_serde")]
    pub path: PathBuf,
    /// Destination reference derived from the selected environment; never used for I/O.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::path_serde::option"
    )]
    pub native_path: Option<PathBuf>,
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
    /// Activate a saved loop when the native target loads it.
    #[serde(default)]
    pub active_loop: bool,
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
    /// Omitted while default so existing baselines and journals keep their hashes.
    #[serde(skip_serializing_if = "MixxxOptions::is_default")]
    pub mixxx: MixxxOptions,
}

/// Whether a Mixxx sync works inside the library that is there or replaces it whole.
/// A folder with no library yet gets a new one either way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixxxLibrary {
    /// Add to and update the library Mixxx has.
    #[default]
    Update,
    /// Build a new library from this sync and replace the existing one. What Mixxx had
    /// is kept in the sync's backup.
    Overwrite,
}

/// What a Mixxx sync does with tracks that are already in the Mixxx library.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixxxExistingTracks {
    /// Add missing tracks and playlists; leave existing Mixxx tracks untouched.
    #[default]
    Keep,
    /// CueRithm replaces the selected data of tracks already in Mixxx.
    Replace,
}

/// Which Mixxx container CueRithm playlists become.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixxxContainer {
    /// Ordered playlists; keeps order and duplicates.
    #[default]
    Playlists,
    /// Unordered, de-duplicated crates.
    Crates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MixxxOptions {
    pub library: MixxxLibrary,
    pub existing_tracks: MixxxExistingTracks,
    pub container: MixxxContainer,
    /// With `Replace`: title, artist, album, genre, comment, year, rating and colour.
    pub replace_metadata: bool,
    /// With `Replace`: the harmonic key.
    pub replace_key: bool,
    /// With `Replace`: hot cues and saved loops.
    pub replace_cues: bool,
}
impl Default for MixxxOptions {
    fn default() -> Self {
        Self {
            library: MixxxLibrary::default(),
            existing_tracks: MixxxExistingTracks::default(),
            container: MixxxContainer::default(),
            replace_metadata: true,
            replace_key: true,
            replace_cues: true,
        }
    }
}
impl MixxxOptions {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartExportContext {
    pub time_zone: String,
    pub native_time_zone: String,
    pub key_mixing_mode: cuerithm_smartlists::KeyMixingMode,
}

impl SyncTrack {
    pub fn reference_path(&self) -> &Path {
        self.native_path.as_deref().unwrap_or(&self.path)
    }

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

#[cfg(test)]
mod mixxx_option_tests {
    use super::*;
    #[test]
    fn mixxx_options_default_is_omitted_from_serialization() {
        let value = serde_json::to_value(SyncOptions::default()).unwrap();
        assert!(
            value.get("mixxx").is_none(),
            "default block must not change existing hashes"
        );
    }
    #[test]
    fn old_options_json_deserializes_with_default_mixxx_block() {
        let options: SyncOptions =
            serde_json::from_str(r#"{"nearest_color":true,"skip_beatgrids":false}"#).unwrap();
        assert!(options.mixxx.is_default());
        assert_eq!(options.mixxx.library, MixxxLibrary::Update);
        assert_eq!(options.mixxx.existing_tracks, MixxxExistingTracks::Keep);
        assert_eq!(options.mixxx.container, MixxxContainer::Playlists);
        assert!(
            options.mixxx.replace_metadata
                && options.mixxx.replace_key
                && options.mixxx.replace_cues
        );
    }
    #[test]
    fn mixxx_options_round_trip_non_default() {
        let options = SyncOptions {
            mixxx: MixxxOptions {
                library: MixxxLibrary::Overwrite,
                existing_tracks: MixxxExistingTracks::Replace,
                container: MixxxContainer::Crates,
                replace_metadata: false,
                replace_key: true,
                replace_cues: false,
            },
            ..SyncOptions::default()
        };
        let json = serde_json::to_value(&options).unwrap();
        assert_eq!(json["mixxx"]["library"], "overwrite");
        assert_eq!(json["mixxx"]["existing_tracks"], "replace");
        assert_eq!(json["mixxx"]["container"], "crates");
        let back: SyncOptions = serde_json::from_value(json).unwrap();
        assert_eq!(back.mixxx, options.mixxx);
    }
    #[test]
    fn partial_mixxx_block_keeps_remaining_defaults() {
        let options: SyncOptions =
            serde_json::from_str(r#"{"mixxx":{"existing_tracks":"replace"}}"#).unwrap();
        assert_eq!(options.mixxx.existing_tracks, MixxxExistingTracks::Replace);
        assert!(options.mixxx.replace_cues, "unspecified toggles stay on");
    }
    #[test]
    fn software_serializes_as_mixxx() {
        assert_eq!(serde_json::to_value(DjSoftware::Mixxx).unwrap(), "mixxx");
    }
}

#[cfg(test)]
mod cue_tests {
    use super::SyncCue;
    #[test]
    fn missing_active_loop_deserializes_as_inactive() {
        let cue: SyncCue = serde_json::from_str(
            r#"{"index":0,"name":"old","start_seconds":1.0,"end_seconds":2.0,"color":null}"#,
        )
        .unwrap();
        assert!(!cue.active_loop);
        assert!(!SyncCue::default().active_loop);
        let active = SyncCue {
            active_loop: true,
            ..cue
        };
        assert!(
            serde_json::from_value::<SyncCue>(serde_json::to_value(active).unwrap())
                .unwrap()
                .active_loop
        );
    }
}
