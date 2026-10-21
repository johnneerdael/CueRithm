//! Owned evidence envelopes; the legacy track is a native-field projection only.
use crate::{CatalogTrack, TransferDate};
use anyhow::{Result, ensure};
use onetagger_tagger::{MatchReason, TrackMatch};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogMatch {
    pub accuracy: f64,
    pub track: CatalogTrack,
    pub reason: MatchReason,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<CatalogFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recognition: Option<CatalogRecognition>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogRecognition {
    pub schema_version: u8,
    pub service: CatalogRecognitionService,
    pub source_sha256: String,
    pub recording_id: Mbid,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CatalogRecognitionService {
    #[serde(rename = "acoustid")]
    AcoustId,
}
impl CatalogMatch {
    pub fn new(accuracy: f64, track: CatalogTrack) -> Self {
        Self {
            accuracy,
            track,
            reason: MatchReason::Fuzzy,
            facts: None,
            recognition: None,
        }
    }
    pub fn new_isrc(track: CatalogTrack) -> Self {
        Self {
            reason: MatchReason::ISRC,
            ..Self::new(1., track)
        }
    }
    pub fn new_id(track: CatalogTrack) -> Self {
        Self {
            reason: MatchReason::ID,
            ..Self::new(1., track)
        }
    }
    pub fn musicbrainz(&self) -> Option<&MusicBrainzFacts> {
        self.facts.as_ref().and_then(|f| match f {
            CatalogFacts::MusicBrainz(f) => Some(f.as_ref()),
            CatalogFacts::Source(_) => None,
        })
    }
    pub fn selected_release_date(&self) -> Option<&TransferDate> {
        match self.facts.as_ref()? {
            CatalogFacts::MusicBrainz(f) => f.selected_release_date.as_ref(),
            CatalogFacts::Source(f) => f.release_date.as_ref(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.accuracy.is_finite() && (0.0..=1.0).contains(&self.accuracy),
            "Invalid candidate similarity"
        );
        if let Some(f) = self.musicbrainz() {
            f.validate()?;
            ensure!(
                self.track.platform == "musicbrainz",
                "Candidate facts belong to another provider"
            );
            ensure!(
                self.track.track_id.as_deref() == Some(f.recording_id.as_str())
                    && self.track.release_id.as_deref() == f.release_id.as_ref().map(Mbid::as_str),
                "Candidate projection changed its MusicBrainz identity"
            );
        } else {
            if let Some(CatalogFacts::Source(f)) = &self.facts {
                f.validate()?;
                ensure!(
                    f.provider == self.track.platform && f.release_id == self.track.release_id,
                    "Source facts do not bind the projected provider/release"
                );
            }
            ensure!(
                self.track.platform != "musicbrainz",
                "MusicBrainz candidate requires typed facts"
            );
        }
        if let Some(origin) = &self.recognition {
            ensure!(
                origin.schema_version == 1
                    && origin.source_sha256.len() == 64
                    && origin.source_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid recognition source evidence"
            );
            ensure!(
                self.musicbrainz()
                    .is_some_and(|facts| facts.recording_id == origin.recording_id),
                "Recognition evidence belongs to another recording"
            );
        }
        Ok(())
    }
    pub fn validate_extension_from(&self, before: &Self) -> Result<()> {
        self.validate()?;
        ensure!(
            self.track.platform == before.track.platform,
            "Candidate provider changed during expansion"
        );
        ensure!(
            self.recognition == before.recognition,
            "Recognition source evidence changed during expansion"
        );
        for (old, new) in [
            (&before.track.track_id, &self.track.track_id),
            (&before.track.release_id, &self.track.release_id),
            (&before.track.isrc, &self.track.isrc),
        ] {
            ensure!(
                old.as_ref().is_none_or(|old| new.as_ref() == Some(old)),
                "Candidate identity changed during expansion"
            );
        }
        if let Some(CatalogFacts::Source(old)) = &before.facts {
            let Some(CatalogFacts::Source(new)) = &self.facts else {
                anyhow::bail!("Selected source facts disappeared")
            };
            ensure!(
                old.provider == new.provider
                    && old
                        .release_id
                        .as_ref()
                        .is_none_or(|id| new.release_id.as_ref() == Some(id))
                    && old
                        .raw_position
                        .as_ref()
                        .is_none_or(|p| new.raw_position.as_ref() == Some(p))
                    && old.identities.iter().all(|id| new.identities.contains(id)),
                "Selected source identity changed during expansion"
            );
        }
        if let Some(old) = before.musicbrainz() {
            let new = self.musicbrainz().ok_or_else(|| {
                anyhow::anyhow!("MusicBrainz evidence disappeared during expansion")
            })?;
            ensure!(
                old.recording_id == new.recording_id
                    && old.release_id == new.release_id
                    && old
                        .release_group_id
                        .as_ref()
                        .is_none_or(|id| new.release_group_id.as_ref() == Some(id))
                    && old.release_track_id == new.release_track_id
                    && old.scope == new.scope
                    && old.source_policy == new.source_policy,
                "Selected MusicBrainz identity or source policy changed during expansion"
            );
        }
        Ok(())
    }
    pub fn entity_key(&self) -> String {
        self.musicbrainz()
            .map(|f| {
                format!(
                    "{}|{}|{}|{}|{}",
                    f.recording_id,
                    f.release_id.as_ref().map(Mbid::as_str).unwrap_or(""),
                    f.release_track_id.as_ref().map(Mbid::as_str).unwrap_or(""),
                    f.position.medium_position.unwrap_or(0),
                    f.position.track_position.unwrap_or(0)
                )
            })
            .unwrap_or_else(|| match &self.facts {
                Some(CatalogFacts::Source(f)) => format!(
                    "{}|{}|{}|{:?}",
                    f.provider,
                    f.release_id.as_deref().unwrap_or(""),
                    f.raw_position.as_deref().unwrap_or(""),
                    f.identities
                ),
                _ => String::new(),
            })
    }
}
impl std::ops::Deref for CatalogMatch {
    type Target = CatalogTrack;
    fn deref(&self) -> &CatalogTrack {
        &self.track
    }
}
impl std::ops::DerefMut for CatalogMatch {
    fn deref_mut(&mut self) -> &mut CatalogTrack {
        &mut self.track
    }
}
impl From<TrackMatch> for CatalogMatch {
    fn from(m: TrackMatch) -> Self {
        Self {
            accuracy: m.accuracy,
            track: m.track,
            reason: m.reason,
            facts: None,
            recognition: None,
        }
    }
}
impl PartialOrd for CatalogMatch {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.accuracy.partial_cmp(&other.accuracy)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Mbid(String);
impl Mbid {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let s = value.into();
        ensure!(
            s.len() == 36
                && s.bytes()
                    .enumerate()
                    .all(|(i, c)| if [8, 13, 18, 23].contains(&i) {
                        c == b'-'
                    } else {
                        c.is_ascii_hexdigit()
                    })
                && s.bytes().any(|c| c.is_ascii_hexdigit() && c != b'0'),
            "Invalid MusicBrainz identifier"
        );
        Ok(Self(s.to_ascii_lowercase()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Mbid {
    type Error = anyhow::Error;
    fn try_from(s: String) -> Result<Self> {
        Self::new(s)
    }
}
impl From<Mbid> for String {
    fn from(id: Mbid) -> Self {
        id.0
    }
}
impl std::fmt::Display for Mbid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum CatalogFacts {
    #[serde(rename = "musicbrainz")]
    MusicBrainz(Box<MusicBrainzFacts>),
    Source(SourceCatalogFacts),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogIdentityScope {
    RecordingOnly,
    ReleaseTrack,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogArtistCredit {
    pub artist_id: Mbid,
    pub name: String,
    pub credited_name: String,
    pub join_phrase: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogPosition {
    pub medium_position: Option<u16>,
    pub track_position: Option<u32>,
    pub printed_number: Option<String>,
    pub medium_track_count: Option<u32>,
    pub medium_count: Option<u16>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicBrainzLabel {
    pub id: Option<Mbid>,
    pub name: Option<String>,
    pub catalog_number: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicBrainzReleaseEvent {
    pub date: Option<TransferDate>,
    pub area_id: Option<Mbid>,
    pub country: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicBrainzRelease {
    pub title: String,
    pub status: Option<String>,
    pub country: Option<String>,
    pub barcode: Option<String>,
    pub media_format: Option<String>,
    pub labels: Vec<MusicBrainzLabel>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicBrainzCoverRef {
    pub release_id: Mbid,
    pub image_id: Option<String>,
}
/// Provenance only. Effective permissions are decided by the trusted provider registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSourcePolicyStamp {
    pub adapter_revision: String,
    pub policy_revision: String,
    pub data_class: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MusicBrainzFacts {
    pub schema_version: u8,
    pub recording_id: Mbid,
    pub release_id: Option<Mbid>,
    pub release_group_id: Option<Mbid>,
    pub release_track_id: Option<Mbid>,
    pub scope: CatalogIdentityScope,
    pub recording_title: String,
    pub track_title: Option<String>,
    pub recording_disambiguation: Option<String>,
    pub recording_credit: Vec<CatalogArtistCredit>,
    pub track_credit: Vec<CatalogArtistCredit>,
    pub release_credit: Vec<CatalogArtistCredit>,
    pub isrcs: Vec<String>,
    pub recording_length_ms: Option<u64>,
    pub track_length_ms: Option<u64>,
    pub selected_release_date: Option<TransferDate>,
    pub recording_first_release_date: Option<TransferDate>,
    pub release_group_first_release_date: Option<TransferDate>,
    pub release_events: Vec<MusicBrainzReleaseEvent>,
    pub position: CatalogPosition,
    pub release: Option<MusicBrainzRelease>,
    pub cover: Option<MusicBrainzCoverRef>,
    pub source_policy: CatalogSourcePolicyStamp,
}
impl MusicBrainzFacts {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "Unsupported MusicBrainz evidence revision"
        );
        ensure!(
            self.recording_credit.len() <= 64
                && self.track_credit.len() <= 64
                && self.release_credit.len() <= 64
                && self.isrcs.len() <= 64
                && self.release_events.len() <= 64,
            "MusicBrainz evidence exceeds collection bounds"
        );
        for d in [
            &self.selected_release_date,
            &self.recording_first_release_date,
            &self.release_group_first_release_date,
        ]
        .into_iter()
        .flatten()
        .chain(self.release_events.iter().filter_map(|e| e.date.as_ref()))
        {
            crate::validate_date(d)?;
        }
        let text = serde_json::to_vec(self)?;
        ensure!(
            text.len() <= 256 * 1024,
            "MusicBrainz candidate exceeds evidence budget"
        );
        for credit in self
            .recording_credit
            .iter()
            .chain(&self.track_credit)
            .chain(&self.release_credit)
        {
            ensure!(
                !credit.credited_name.trim().is_empty()
                    && credit.credited_name.len() <= 4096
                    && credit.name.len() <= 4096
                    && credit.join_phrase.len() <= 1024,
                "Invalid MusicBrainz artist credit"
            );
        }
        ensure!(
            self.position.medium_position.is_none_or(|n| n > 0)
                && self.position.track_position.is_none_or(|n| n > 0),
            "Invalid MusicBrainz track position"
        );
        match self.scope {
            CatalogIdentityScope::RecordingOnly => ensure!(
                self.release_id.is_none()
                    && self.release_track_id.is_none()
                    && self.release.is_none()
                    && self.cover.is_none()
                    && self.selected_release_date.is_none(),
                "Recording evidence cannot claim release metadata"
            ),
            CatalogIdentityScope::ReleaseTrack => ensure!(
                self.release_id.is_some()
                    && self.release_track_id.is_some()
                    && self.release.is_some()
                    && self.position.medium_position.is_some()
                    && self.position.track_position.is_some(),
                "Release evidence is incomplete"
            ),
        }
        if let Some(cover) = &self.cover {
            ensure!(
                Some(&cover.release_id) == self.release_id.as_ref(),
                "Artwork belongs to a different release"
            );
            ensure!(
                cover.image_id.as_ref().is_none_or(|s| !s.is_empty()
                    && s.len() <= 32
                    && s.bytes().all(|c| c.is_ascii_digit())),
                "Invalid CAA image identity"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCatalogFacts {
    pub schema_version: u8,
    pub provider: String,
    pub release_date: Option<TransferDate>,
    pub release_id: Option<String>,
    pub raw_position: Option<String>,
    #[serde(default)]
    pub identities: Vec<SourceCatalogIdentity>,
    #[serde(default)]
    pub links: Vec<SourceCatalogLink>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceCatalogIdentityKind {
    Recording,
    Release,
    ReleaseTrack,
    Artist,
    PlatformTrack,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCatalogIdentity {
    pub source: String,
    pub kind: SourceCatalogIdentityKind,
    pub id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCatalogLink {
    pub source: String,
    pub url: String,
}
impl SourceCatalogFacts {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && crate::ProviderId::parse(&self.provider).is_some(),
            "Unsupported source facts"
        );
        ensure!(
            self.identities.len() <= 64 && self.links.len() <= 32,
            "Source evidence exceeds bounds"
        );
        ensure!(
            self.release_id
                .as_ref()
                .is_none_or(|s| s.len() <= 1024 && !s.chars().any(char::is_control))
                && self
                    .raw_position
                    .as_ref()
                    .is_none_or(|s| s.len() <= 128 && !s.chars().any(char::is_control)),
            "Source identity/position exceeds bounds"
        );
        if let Some(date) = &self.release_date {
            crate::validate_date(date)?;
        }
        for id in &self.identities {
            ensure!(
                id.source.len() <= 64
                    && !id.source.chars().any(char::is_control)
                    && !id.id.is_empty()
                    && id.id.len() <= 1024
                    && !id.id.chars().any(char::is_control),
                "Invalid source identity"
            );
        }
        for link in &self.links {
            let url = reqwest::Url::parse(&link.url)?;
            ensure!(
                link.source.len() <= 64
                    && link.url.len() <= 4096
                    && url.scheme() == "https"
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.port_or_known_default() == Some(443)
                    && url.query().is_none(),
                "Invalid source link"
            );
        }
        ensure!(
            serde_json::to_vec(self)?.len() <= 128 * 1024,
            "Source facts exceed evidence budget"
        );
        Ok(())
    }
}

/// Native ID namespaces use spaces (MP4), slashes (ASF), and underscores (Vorbis).
/// Compare these explicit semantic aliases without changing their values.
pub(crate) fn native_identity_key_matches(key: &str, alias: &str) -> bool {
    key.bytes()
        .filter(u8::is_ascii_alphanumeric)
        .map(|b| b.to_ascii_lowercase())
        .eq(alias
            .bytes()
            .filter(u8::is_ascii_alphanumeric)
            .map(|b| b.to_ascii_lowercase()))
}
