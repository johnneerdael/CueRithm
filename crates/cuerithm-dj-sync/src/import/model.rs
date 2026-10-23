use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceProvider {
    Rekordbox,
    Serato3,
    Serato4,
    EngineDj,
    Traktor,
    VirtualDj,
    DjayPro,
    AppleMusicXml,
    PioneerUsb,
    M3u,
    MixedInKey11,
    DjStudio,
    Mixxx,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceKey {
    pub provider: SourceProvider,
    pub library_instance_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RecordKey {
    pub source: SourceKey,
    pub external_id: String,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "presence", content = "value", rename_all = "snake_case")]
pub enum Presence<T> {
    #[default]
    Absent,
    Null,
    Empty(T),
    Value(T),
    Deleted,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldPresence {
    #[default]
    Absent,
    Null,
    Empty,
    Value,
    Deleted,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    WholeLibrary,
    SelectedScope,
    #[default]
    PartialWithErrors,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockStatus {
    ValidatedProfile,
    #[default]
    UnresolvedNative,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordAccounting {
    pub tracks_seen: Option<u64>,
    pub collections_seen: Option<u64>,
    pub memberships_seen: Option<u64>,
    pub unrepresented_records: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationHeader {
    #[serde(default = "contract_version")]
    pub contract_version: u32,
    #[serde(flatten)]
    pub source: SourceKey,
    #[serde(default)]
    pub product_version: String,
    pub schema_fingerprint: String,
    pub snapshot_hash: String,
    #[serde(default)]
    pub adapter_version: String,
    pub completeness: Completeness,
    #[serde(default)]
    pub scope: Value,
    #[serde(default)]
    pub observed_at: String,
    #[serde(default)]
    pub accounting: RecordAccounting,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
fn contract_version() -> u32 {
    1
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BlobRef {
    pub sha256: String,
    pub bytes: u64,
    pub media_type: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LocationEvidence {
    #[serde(with = "crate::path_serde::option")]
    pub native_path: Option<std::path::PathBuf>,
    pub original: Option<String>,
    pub resolved: Option<String>,
    pub availability: String,
    pub platform: Option<String>,
    pub volume_id: Option<String>,
    pub case_sensitive: Option<bool>,
    pub raw: Option<BlobRef>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MusicalKey {
    pub presence: FieldPresence,
    pub raw: Option<String>,
    pub camelot: Option<String>,
    pub pitch_class: Option<u8>,
    pub mode: Option<String>,
    pub provenance: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScalarBpm {
    pub presence: FieldPresence,
    pub value: Option<f64>,
    pub locked: Option<bool>,
    pub provenance: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CueFamily {
    Marker,
    Hot,
    Memory,
    Main,
    Load,
    Loop,
    Intro,
    Outro,
    AudibleRange,
    #[default]
    Unknown,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CueObservation {
    pub external_id: String,
    pub family: CueFamily,
    pub source_slot: Option<i32>,
    pub name: Option<String>,
    pub color_rgb: Option<u32>,
    pub original: Value,
    pub native_start_seconds: Option<f64>,
    pub native_end_seconds: Option<f64>,
    pub canonical_start_seconds: Option<f64>,
    pub canonical_end_seconds: Option<f64>,
    pub clock_status: ClockStatus,
    pub timing_profile_id: Option<String>,
    pub flags: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridPoint {
    pub native_seconds: f64,
    pub canonical_seconds: Option<f64>,
    pub bpm: Option<f64>,
    pub beat_index: Option<i64>,
    pub original: Value,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GridObservation {
    pub representation: String,
    pub original_time_base: Option<String>,
    pub sample_rate: Option<f64>,
    pub native_first_beat_seconds: Option<f64>,
    pub canonical_first_beat_seconds: Option<f64>,
    pub bpm: Option<f64>,
    pub downbeat_known: bool,
    pub clock_status: ClockStatus,
    pub timing_profile_id: Option<String>,
    pub offset_seconds: Option<f64>,
    pub points: Vec<GridPoint>,
    pub raw: Option<BlobRef>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportIssue {
    pub code: String,
    pub record_id: Option<String>,
    pub field: Option<String>,
    pub message: String,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackObservation {
    pub external_id: String,
    pub locations: LocationEvidence,
    pub metadata: BTreeMap<String, Presence<Value>>,
    pub cached_tags: BTreeMap<String, Presence<Value>>,
    pub file_tags: Option<BlobRef>,
    pub key: Option<MusicalKey>,
    pub bpm: Option<ScalarBpm>,
    pub energy: Presence<Value>,
    pub grid: Option<GridObservation>,
    pub cues: Vec<CueObservation>,
    pub extensions: BTreeMap<String, Value>,
    pub issues: Vec<ImportIssue>,
    #[serde(flatten)]
    pub additional: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionKind {
    Folder,
    #[default]
    Playlist,
    Crate,
    Smart,
    History,
    Queue,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MembershipObservation {
    pub external_occurrence_id: String,
    pub track_external_id: String,
    pub position: Option<i64>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CollectionObservation {
    pub external_id: String,
    pub parent_id: Option<String>,
    pub kind: CollectionKind,
    pub name: String,
    pub occurrences: Vec<MembershipObservation>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record_type", content = "record", rename_all = "snake_case")]
pub enum ObservationRecord {
    Track(Box<TrackObservation>),
    Collection(CollectionObservation),
    Membership {
        collection_id: String,
        occurrence: MembershipObservation,
    },
    Issue(ImportIssue),
}
pub trait ObservationSink {
    fn put_blob(&mut self, bytes: &[u8], media_type: Option<&str>) -> Result<BlobRef>;
    fn push(&mut self, record: ObservationRecord) -> Result<()>;
}
pub trait ObservationReader {
    fn next(&mut self) -> Result<Option<ObservationRecord>>;
}
impl<I: Iterator<Item = ObservationRecord>> ObservationReader for I {
    fn next(&mut self) -> Result<Option<ObservationRecord>> {
        Ok(Iterator::next(self))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationEnvelope {
    pub contract_version: u32,
    pub source: ObservationHeader,
    #[serde(default)]
    pub tracks: Vec<TrackObservation>,
    #[serde(default)]
    pub collections: Vec<CollectionObservation>,
    #[serde(default)]
    pub issues: Vec<ImportIssue>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
impl ObservationEnvelope {
    /// Convenience for small interchange/fixture envelopes; production readers stream to disk.
    pub fn reader(&self) -> impl ObservationReader + '_ {
        self.tracks
            .iter()
            .cloned()
            .map(|track| ObservationRecord::Track(Box::new(track)))
            .chain(
                self.collections
                    .iter()
                    .cloned()
                    .map(ObservationRecord::Collection),
            )
            .chain(self.issues.iter().cloned().map(ObservationRecord::Issue))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLimits {
    pub page_size: usize,
    pub hierarchy_depth: usize,
    pub blob_bytes: u64,
    pub text_bytes: u64,
    pub snapshot_bytes: u64,
}
impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            page_size: 1000,
            hierarchy_depth: 128,
            blob_bytes: 64 * 1024 * 1024,
            text_bytes: 16 * 1024 * 1024,
            snapshot_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCapability {
    pub read: bool,
    pub write: bool,
    pub lossless: bool,
    pub evidence: EvidenceLevel,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceProfile {
    pub provider: SourceProvider,
    pub profile_id: String,
    pub schema_fingerprint: String,
    pub origin: BTreeMap<String, String>,
    pub fields: BTreeMap<String, FieldCapability>,
    pub issues: Vec<ImportIssue>,
}
