//! Public MusicBrainz metadata adapter; all physical requests use CatalogRuntime.
#[cfg(test)]
mod tests;
mod wire;
use crate::{
    AutotagConfig, BackendFailure, CatalogError, CatalogIdentityScope, CatalogMatch,
    CatalogRuntime, CatalogService, Mbid, ProviderBackend, ProviderFailureKind, ProviderId,
    TagInput,
};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
pub struct MusicBrainzBackend {
    runtime: Arc<CatalogRuntime>,
}
impl MusicBrainzBackend {
    pub fn new(runtime: Arc<CatalogRuntime>) -> Self {
        Self { runtime }
    }
    pub fn recording_candidates(
        &self,
        id: &Mbid,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        let Some(value) = self.get(
            &format!("recording/{id}"),
            vec![("inc".into(), "artist-credits+isrcs+releases+media".into())],
            cancel,
        )?
        else {
            return Ok(vec![]);
        };
        let mut found = self.mapped(wire::recordings(value))?;
        if found.len() == 1
            && found[0]
                .musicbrainz()
                .is_some_and(|f| f.scope == CatalogIdentityScope::RecordingOnly)
        {
            let releases = self.browse(&found[0], cancel)?;
            if !releases.is_empty() {
                found = releases;
            }
        }
        for candidate in &mut found {
            candidate.reason = onetagger_tagger::MatchReason::ID;
            candidate.accuracy = 1.;
        }
        Ok(found)
    }
    fn get(
        &self,
        path: &str,
        mut query: Vec<(String, String)>,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Option<Value>> {
        ensure!(!cancel.load(Ordering::Acquire), "Catalog lookup cancelled");
        query.push(("fmt".into(), "json".into()));
        match self.runtime.get_json(
            CatalogService::MusicBrainz,
            &format!("https://musicbrainz.org/ws/2/{path}"),
            &query,
            None,
            cancel,
        ) {
            Ok(v) => Ok(Some(v)),
            Err(e)
                if e.downcast_ref::<CatalogError>()
                    .is_some_and(|e| e.status == Some(404)) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
    fn mapped<T>(&self, value: Result<T>) -> Result<T> {
        value.map_err(|_| {
            BackendFailure::new(
                ProviderFailureKind::Parse,
                "MusicBrainz returned malformed or inconsistent metadata",
            )
            .into()
        })
    }
    fn browse(
        &self,
        selected: &CatalogMatch,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        let Some(f) = selected.musicbrainz() else {
            return Ok(vec![]);
        };
        let Some(value) = self.get(
            "release",
            vec![
                ("recording".into(), f.recording_id.to_string()),
                (
                    "inc".into(),
                    "recordings+artist-credits+labels+release-groups+isrcs".into(),
                ),
                ("limit".into(), "10".into()),
                ("offset".into(), "0".into()),
            ],
            cancel,
        )?
        else {
            return Ok(vec![]);
        };
        self.mapped(wire::releases_for_recording(value, selected))
    }
}
fn tag(input: &TagInput, keys: &[&str]) -> Option<String> {
    input
        .media
        .tags
        .iter()
        .find(|(key, _)| keys.iter().any(|allowed| key.eq_ignore_ascii_case(allowed)))
        .map(|(_, v)| v.clone())
        .filter(|s| !s.trim().is_empty())
}
fn source_id(input: &TagInput, keys: &[&str]) -> Result<Option<Mbid>> {
    input
        .media
        .tags
        .iter()
        .find(|(key, _)| {
            keys.iter()
                .any(|alias| crate::candidate::native_identity_key_matches(key, alias))
        })
        .map(|(_, value)| Mbid::new(value.clone()))
        .transpose()
}
fn phrase(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars().take(512) {
        if "+-!(){}[]^\"~*?:\\/|&".contains(c) {
            out.push('\\')
        }
        out.push(c)
    }
    format!("\"{out}\"")
}
impl ProviderBackend for MusicBrainzBackend {
    fn id(&self) -> ProviderId {
        ProviderId::Musicbrainz
    }
    fn stage_timeout(&self, _: &AutotagConfig) -> Duration {
        Duration::from_secs(360)
    }
    fn candidates(&self, input: &TagInput, config: &AutotagConfig) -> Result<Vec<CatalogMatch>> {
        self.candidates_cancellable(input, config, &Arc::new(AtomicBool::new(false)))
    }
    fn candidates_cancellable(
        &self,
        input: &TagInput,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Vec<CatalogMatch>> {
        let info = input.audio_info(config)?;
        let mut exact = None;
        let mut found = if config.options.match_by_id {
            let recording = source_id(
                input,
                &[
                    "musicbrainz_trackid",
                    "musicbrainz_recordingid",
                    "musicbrainz_recording_id",
                    "musicbrainz_track_id",
                ],
            )?;
            let release = source_id(
                input,
                &[
                    "musicbrainz_albumid",
                    "musicbrainz_releaseid",
                    "musicbrainz_release_id",
                ],
            )?;
            let release_track = source_id(
                input,
                &["musicbrainz_releasetrackid", "musicbrainz_release_track_id"],
            )?;
            if let Some(release) = release {
                let Some(v) = self.get(
                    &format!("release/{release}"),
                    vec![(
                        "inc".into(),
                        "recordings+artist-credits+labels+release-groups+isrcs".into(),
                    )],
                    cancel,
                )?
                else {
                    return Ok(vec![]);
                };
                exact = recording
                    .as_ref()
                    .or(release_track.as_ref())
                    .map(|_| onetagger_tagger::MatchReason::ID);
                self.mapped(wire::release_candidates(
                    v,
                    recording.as_ref(),
                    release_track.as_ref(),
                ))?
            } else if let Some(recording) = recording {
                exact = Some(onetagger_tagger::MatchReason::ID);
                let Some(v) = self.get(
                    &format!("recording/{recording}"),
                    vec![("inc".into(), "artist-credits+isrcs+releases+media".into())],
                    cancel,
                )?
                else {
                    return Ok(vec![]);
                };
                self.mapped(wire::recordings(v))?
            } else if let Some(isrc) = tag(input, &["isrc"]) {
                let isrc = isrc.replace('-', "").to_ascii_uppercase();
                ensure!(
                    isrc.len() == 12 && isrc.bytes().all(|c| c.is_ascii_alphanumeric()),
                    "Invalid source ISRC"
                );
                exact = Some(onetagger_tagger::MatchReason::ISRC);
                let Some(mut v) = self.get(
                    &format!("isrc/{isrc}"),
                    vec![("inc".into(), "artist-credits+releases+media".into())],
                    cancel,
                )?
                else {
                    return Ok(vec![]);
                };
                if let Some(rows) = v.get_mut("recordings").and_then(Value::as_array_mut) {
                    for r in rows {
                        if r.get("isrcs").is_none() {
                            r["isrcs"] = json!([isrc])
                        }
                    }
                }
                self.mapped(wire::recordings(v))?
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        if found.is_empty() && exact.is_none() {
            let Some(title) = info.title.as_deref().filter(|s| !s.trim().is_empty()) else {
                return Ok(vec![]);
            };
            let mut query = format!("recording:{}", phrase(title));
            if let Some(artist) = info.artists.first() {
                query.push_str(&format!(" AND artist:{}", phrase(artist)))
            }
            let Some(v) = self.get(
                "recording",
                vec![
                    ("query".into(), query),
                    ("limit".into(), "10".into()),
                    ("offset".into(), "0".into()),
                ],
                cancel,
            )?
            else {
                return Ok(vec![]);
            };
            found = self.mapped(wire::recordings(v))?;
        }
        if found.len() == 1
            && found[0]
                .musicbrainz()
                .is_some_and(|f| f.scope == CatalogIdentityScope::RecordingOnly)
        {
            let releases = self.browse(&found[0], cancel)?;
            if !releases.is_empty() {
                found = releases
            }
        }
        if let Some(reason) = exact {
            for candidate in &mut found {
                candidate.reason = reason;
                candidate.accuracy = 1.
            }
            return Ok(found);
        }
        let tracks = found.iter().map(|m| m.track.clone()).collect();
        let scores =
            onetagger_tagger::MatchingUtils::match_track(&info, &tracks, &config.options, true);
        let mut out = Vec::new();
        for score in scores {
            for candidate in found.iter().filter(|c| c.track == score.track) {
                let mut candidate = candidate.clone();
                candidate.accuracy = score.accuracy;
                candidate.reason = score.reason;
                if !out
                    .iter()
                    .any(|m: &CatalogMatch| m.entity_key() == candidate.entity_key())
                {
                    out.push(candidate)
                }
            }
        }
        Ok(out)
    }
    fn extend_candidate_cancellable(
        &self,
        candidate: &mut CatalogMatch,
        _: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        candidate.validate()?;
        let facts = candidate
            .musicbrainz()
            .ok_or_else(|| anyhow::anyhow!("Missing MusicBrainz evidence"))?;
        if let Some(release) = &facts.release_id {
            let value = self
                .get(
                    &format!("release/{release}"),
                    vec![(
                        "inc".into(),
                        "recordings+artist-credits+labels+release-groups+isrcs".into(),
                    )],
                    cancel,
                )?
                .ok_or_else(|| {
                    BackendFailure::new(
                        ProviderFailureKind::Unavailable,
                        "Selected MusicBrainz release is unavailable",
                    )
                })?;
            let mut expanded = self.mapped(wire::release(value, candidate))?;
            expanded.accuracy = candidate.accuracy;
            expanded.reason = candidate.reason;
            expanded.recognition = candidate.recognition.clone();
            *candidate = expanded;
        } else {
            let value = self
                .get(
                    &format!("recording/{}", facts.recording_id),
                    vec![("inc".into(), "artist-credits+isrcs".into())],
                    cancel,
                )?
                .ok_or_else(|| {
                    BackendFailure::new(
                        ProviderFailureKind::Unavailable,
                        "Selected MusicBrainz recording is unavailable",
                    )
                })?;
            let mut expanded = self
                .mapped(wire::recordings(value))?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    BackendFailure::new(
                        ProviderFailureKind::InvalidResponse,
                        "Selected MusicBrainz recording is missing",
                    )
                })?;
            expanded.accuracy = candidate.accuracy;
            expanded.reason = candidate.reason;
            expanded.recognition = candidate.recognition.clone();
            expanded.validate_extension_from(candidate)?;
            *candidate = expanded;
        }
        Ok(())
    }
}
