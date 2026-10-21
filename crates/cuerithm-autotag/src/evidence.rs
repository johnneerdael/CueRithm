//! Bounded review evidence. Similarity is a ranking score, never a correctness probability.
use crate::{AutotagConfig, CatalogTrack, MatchStatus, TagInput, TagMatchOutcome};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_REVIEW_CANDIDATES: usize = 10;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSnapshot {
    pub provider: String,
    pub track_id: Option<String>,
    pub release_id: Option<String>,
    pub title: String,
    pub version: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_seconds: Option<f64>,
    pub similarity: f64,
    pub reason: onetagger_tagger::MatchReason,
    pub identity_conflicts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<crate::CatalogFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recognition: Option<crate::CatalogRecognition>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateProvenance {
    pub schema_version: u8,
    pub adapter_revision: String,
    pub source_sha256: String,
    pub config_sha256: String,
    pub query_title: Option<String>,
    pub query_artists: Vec<String>,
    pub status: MatchStatus,
    pub candidates: Vec<CandidateSnapshot>,
    pub truncated: bool,
}
fn bounded(value: &str) -> String {
    value.chars().take(256).collect()
}
fn optional(value: &Option<String>) -> Option<String> {
    value.as_deref().map(bounded)
}
impl TagMatchOutcome {
    pub fn provenance(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
    ) -> Result<CandidateProvenance> {
        config.validate()?;
        ensure!(
            self.matches
                .iter()
                .all(|m| m.accuracy.is_finite() && (0.0..=1.0).contains(&m.accuracy)),
            "Invalid candidate similarity"
        );
        let info = input.audio_info(config)?;
        Ok(CandidateProvenance {
            schema_version: 1,
            adapter_revision: format!(
                "cuerithm-autotag/{}/onetagger-36523f7/identity-v1",
                env!("CARGO_PKG_VERSION")
            ),
            source_sha256: input.source_sha256.chars().take(64).collect(),
            config_sha256: format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&canonical(serde_json::to_value(
                    config
                )?))?)
            ),
            query_title: optional(&info.title),
            query_artists: info.artists.iter().take(16).map(|s| bounded(s)).collect(),
            status: self.status,
            candidates: self
                .matches
                .iter()
                .take(MAX_REVIEW_CANDIDATES)
                .map(|m| CandidateSnapshot {
                    provider: bounded(&m.track.platform),
                    track_id: optional(&m.track.track_id),
                    release_id: optional(&m.track.release_id),
                    title: bounded(&m.track.title),
                    version: optional(&m.track.version),
                    artists: m
                        .track
                        .artists
                        .iter()
                        .take(16)
                        .map(|s| bounded(s))
                        .collect(),
                    album: optional(&m.track.album),
                    isrc: optional(&m.track.isrc),
                    duration_seconds: (!m.track.duration.is_zero())
                        .then_some(m.track.duration.as_secs_f64()),
                    similarity: m.accuracy,
                    reason: m.reason,
                    identity_conflicts: match_identity_conflicts(input, m, config),
                    facts: m.facts.clone(),
                    recognition: m.recognition.clone(),
                })
                .collect(),
            truncated: self.matches.len() > MAX_REVIEW_CANDIDATES,
        })
    }
}
fn normalized(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn trailer(value: &str) -> Option<&str> {
    let value = value.trim();
    for (open, close) in [('(', ')'), ('[', ']'), ('{', '}')] {
        if value.ends_with(close)
            && let Some((_, v)) = value.rsplit_once(open)
        {
            let edition = v.trim_end_matches(close).trim();
            let lower = edition.to_lowercase();
            if ["feat.", "feat ", "ft.", "ft ", "featuring "]
                .iter()
                .any(|p| lower.starts_with(p))
            {
                return None;
            }
            let marked = lower.split(|c: char| !c.is_alphanumeric()).any(|word| {
                [
                    "mix",
                    "remix",
                    "edit",
                    "dub",
                    "instrumental",
                    "vocal",
                    "vocals",
                    "extended",
                    "radio",
                    "club",
                    "version",
                    "remaster",
                    "remastered",
                    "acoustic",
                    "live",
                    "original",
                    "demo",
                    "mono",
                    "stereo",
                    "vip",
                    "bootleg",
                    "rework",
                    "reedit",
                ]
                .contains(&word)
            });
            return marked.then_some(edition);
        }
    }
    None
}
fn source_tag<'a>(input: &'a TagInput, key: &str) -> Option<&'a str> {
    input
        .media
        .tags
        .get(key)
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
}
pub(crate) fn identity_conflicts(
    input: &TagInput,
    candidate: &CatalogTrack,
    config: &AutotagConfig,
) -> Vec<String> {
    let mut conflicts = vec![];
    let title = input.audio_info(config).ok().and_then(|i| i.title);
    let source_version = source_tag(input, "version")
        .or_else(|| source_tag(input, "subtitle"))
        .or_else(|| source_tag(input, "mix"))
        .or_else(|| title.as_deref().and_then(trailer));
    let candidate_version = candidate
        .version
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| trailer(&candidate.title));
    if let Some(v) = source_version
        && candidate_version.is_none_or(|c| normalized(v) != normalized(c))
    {
        conflicts.push("edition".into());
    }
    for (name, source, target) in [
        ("isrc", source_tag(input, "isrc"), candidate.isrc.as_deref()),
        (
            "album",
            source_tag(input, "album"),
            candidate.album.as_deref(),
        ),
    ] {
        if let (Some(a), Some(b)) = (source, target)
            && normalized(a) != normalized(b)
        {
            conflicts.push(name.into());
        }
    }
    let release_key = format!("{}_release_id", candidate.platform);
    if let (Some(a), Some(b)) = (
        source_tag(input, &release_key),
        candidate.release_id.as_deref(),
    ) && a.trim() != b.trim()
    {
        conflicts.push("release_id".into());
    }
    if config.options.match_duration
        && !candidate.duration.is_zero()
        && input.duration_seconds > 0.
        && (candidate.duration.as_secs_f64() - input.duration_seconds).abs()
            > config.options.max_duration_difference as f64
    {
        conflicts.push("duration".into());
    }
    conflicts
}
pub(crate) fn stable_key(t: &CatalogTrack) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{}\0{}\0{:?}\0{}\0{}",
        t.platform,
        t.track_id.as_deref().unwrap_or(""),
        t.release_id.as_deref().unwrap_or(""),
        t.full_title(),
        t.album.as_deref().unwrap_or(""),
        t.isrc.as_deref().unwrap_or(""),
        t.artists,
        t.duration.as_nanos(),
        if t.track_id.is_none() {
            format!("{:?}|{:?}", t.disc_number, t.track_number)
        } else {
            String::new()
        }
    )
}

fn canonical(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect(),
        ),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(canonical).collect())
        }
        other => other,
    }
}

pub(crate) fn recording_conflict(
    a: &CatalogTrack,
    b: &CatalogTrack,
    config: &AutotagConfig,
) -> bool {
    let edition = |t: &CatalogTrack| {
        t.version
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| trailer(&t.title))
            .map(normalized)
    };
    let versions = (edition(a), edition(b));
    matches!(versions,(Some(a),Some(b)) if a!=b)
        || matches!((&a.isrc,&b.isrc),(Some(a),Some(b)) if normalized(a)!=normalized(b))
        || (config.options.match_duration
            && !a.duration.is_zero()
            && !b.duration.is_zero()
            && a.duration.abs_diff(b.duration).as_secs_f64()
                > config.options.max_duration_difference as f64)
}

pub(crate) fn candidate_key(candidate: &crate::CatalogMatch) -> String {
    format!(
        "{}\0{}",
        stable_key(&candidate.track),
        candidate.entity_key()
    )
}
pub(crate) fn match_identity_conflicts(
    input: &TagInput,
    candidate: &crate::CatalogMatch,
    config: &AutotagConfig,
) -> Vec<String> {
    let mut conflicts = identity_conflicts(input, &candidate.track, config);
    if let Some(f) = candidate.musicbrainz() {
        if f.scope == crate::CatalogIdentityScope::RecordingOnly {
            conflicts.push("release_edition_unresolved".into());
        }
        for (aliases, target, name) in [
            (
                &[
                    "musicbrainz_trackid",
                    "musicbrainz_recordingid",
                    "musicbrainz_recording_id",
                ][..],
                Some(f.recording_id.as_str()),
                "recording_id",
            ),
            (
                &["musicbrainz_albumid", "musicbrainz_releaseid"][..],
                f.release_id.as_ref().map(crate::Mbid::as_str),
                "release_id",
            ),
            (
                &["musicbrainz_releasetrackid"][..],
                f.release_track_id.as_ref().map(crate::Mbid::as_str),
                "release_track_id",
            ),
        ] {
            if let Some(source) = input
                .media
                .tags
                .iter()
                .find(|(key, _)| {
                    aliases
                        .iter()
                        .any(|a| crate::candidate::native_identity_key_matches(key, a))
                })
                .map(|(_, value)| value.as_str())
                && target.is_none_or(|target| !source.eq_ignore_ascii_case(target))
            {
                conflicts.push(name.into());
            }
        }
    }
    conflicts.sort();
    conflicts.dedup();
    conflicts
}
pub(crate) fn candidate_conflict(
    a: &crate::CatalogMatch,
    b: &crate::CatalogMatch,
    config: &AutotagConfig,
) -> bool {
    recording_conflict(&a.track, &b.track, config)
        || (a.track_id.is_none()
            && b.track_id.is_none()
            && a.platform == b.platform
            && a.release_id == b.release_id
            && a.track_number.is_some()
            && b.track_number.is_some()
            && (a.track_number != b.track_number || a.disc_number != b.disc_number))
        || (config.options.multiple_matches != onetagger_tagger::MultipleMatchesSort::Default
            && a.facts.is_some()
            && b.facts.is_some()
            && candidate_key(a) != candidate_key(b)
            && partial_dates_overlap(a.selected_release_date(), b.selected_release_date()))
        || match (a.musicbrainz(), b.musicbrainz()) {
            (Some(a), Some(b)) => {
                a.recording_id != b.recording_id
                    || (a.release_id.is_some()
                        && a.release_id == b.release_id
                        && a.release_track_id != b.release_track_id)
            }
            _ => false,
        }
}

// Unknown components define intervals, not invented dates. Only disjoint
// intervals can establish an older/newer edition under the explicit policy.
fn partial_dates_overlap(a: Option<&crate::TransferDate>, b: Option<&crate::TransferDate>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) if a.year != b.year => false,
        (Some(a), Some(b)) => match (a.month, b.month) {
            (Some(a), Some(b)) if a != b => false,
            (Some(_), Some(_)) => match (a.day, b.day) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            },
            _ => true,
        },
        _ => true,
    }
}
