//! Explicit candidate expansion shares the automatic provider limits and cooldown.
use crate::{AutotagConfig, CatalogMatch, ProviderId, TagInput, Tagger, retry};
use anyhow::{Result, ensure};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
const MAX_EXPANDED_BYTES: usize = 2 * 1024 * 1024;
impl Tagger {
    /// Captured provider permission; recognition capability remains tied to its original source.
    pub fn review_candidate_provider_enabled(
        input: &TagInput,
        candidate: &CatalogMatch,
        config: &AutotagConfig,
    ) -> Result<bool> {
        candidate.validate()?;
        if let Some(origin) = &candidate.recognition {
            return Ok(config.recognition.enabled
                && config.recognition.fingerprint_consent
                && origin.service == crate::CatalogRecognitionService::AcoustId
                && origin.source_sha256 == input.source_sha256
                && candidate
                    .musicbrainz()
                    .is_some_and(|facts| facts.recording_id == origin.recording_id));
        }
        Ok(config
            .options
            .platforms
            .iter()
            .any(|id| id == &candidate.track.platform))
    }
    pub fn review_candidate_conflicts(
        input: &TagInput,
        track: &crate::CatalogTrack,
        config: &AutotagConfig,
    ) -> Result<Vec<String>> {
        config.validate()?;
        input.audio_info(config)?;
        Ok(crate::evidence::identity_conflicts(input, track, config))
    }

    pub fn review_match_conflicts(
        input: &TagInput,
        candidate: &CatalogMatch,
        config: &AutotagConfig,
    ) -> Result<Vec<String>> {
        candidate.validate()?;
        config.validate()?;
        Ok(crate::evidence::match_identity_conflicts(
            input, candidate, config,
        ))
    }

    pub fn expand_review_candidate(
        &self,
        input: &TagInput,
        candidate: &CatalogMatch,
        config: &AutotagConfig,
        cancel: &Arc<AtomicBool>,
    ) -> Result<CatalogMatch> {
        config.validate()?;
        input.audio_info(config)?;
        ensure!(
            !cancel.load(Ordering::Acquire),
            "Catalog review was cancelled"
        );
        ensure!(
            candidate.accuracy.is_finite() && (0.0..=1.0).contains(&candidate.accuracy),
            "Invalid candidate similarity"
        );
        let backend = self
            .backends
            .iter()
            .find(|b| b.source.id().as_str() == candidate.track.platform)
            .ok_or_else(|| anyhow::anyhow!("Candidate provider is unavailable"))?;
        ensure!(
            Self::review_candidate_provider_enabled(input, candidate, config)?,
            "Candidate provider is not enabled in this captured configuration"
        );
        let limit = if backend.source.id() == ProviderId::Traxsource {
            1
        } else {
            usize::from(config.options.threads)
        };
        retry::bounded(cancel, backend.source.stage_timeout(config), |cancel| {
            let _permit = backend.gate.acquire(limit, cancel)?;
            backend
                .cooldown
                .run(cancel, backend.source.stage_timeout(config), || {
                    let mut expanded = candidate.clone();
                    backend
                        .source
                        .extend_candidate_cancellable(&mut expanded, config, cancel)?;
                    expanded.validate_extension_from(candidate)?;
                    ensure!(
                        !cancel.load(Ordering::Acquire),
                        "Catalog review was cancelled"
                    );
                    ensure!(
                        expanded.track.platform == candidate.track.platform,
                        "Provider changed during selected-candidate expansion"
                    );
                    for (before, after) in [
                        (&candidate.track.track_id, &expanded.track.track_id),
                        (&candidate.track.release_id, &expanded.track.release_id),
                        (&candidate.track.isrc, &expanded.track.isrc),
                    ] {
                        ensure!(
                            before
                                .as_ref()
                                .is_none_or(|before| after.as_ref() == Some(before)),
                            "Provider substituted a selected recording/release identity"
                        );
                    }
                    ensure!(
                        normalize(&candidate.track.title) == normalize(&expanded.track.title),
                        "Provider substituted the selected title"
                    );
                    ensure!(
                        candidate.track.artists.iter().all(|before| expanded
                            .track
                            .artists
                            .iter()
                            .any(|after| normalize(before) == normalize(after))),
                        "Provider substituted the selected artists"
                    );
                    ensure!(
                        serde_json::to_vec(&expanded)?.len() <= MAX_EXPANDED_BYTES,
                        "Expanded candidate exceeds the bounded review budget; requery explicitly"
                    );
                    Ok(expanded)
                })
        })
    }
}
fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// UI links are informational; image bytes always use the separately bounded native fetcher.
pub fn review_catalog_url(provider: ProviderId, value: &str, artwork: bool) -> Option<String> {
    let url = reqwest::Url::parse(value).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return None;
    }
    let host = url.host_str()?;
    let under = |domain: &str| host == domain || host.ends_with(&format!(".{domain}"));
    let allowed = if artwork {
        match provider {
            ProviderId::Beatport => under("beatport.com"),
            ProviderId::Traxsource => under("traxsource.com"),
            ProviderId::Musicbrainz => host == "coverartarchive.org",
            ProviderId::Sonovault => false,
            ProviderId::Discogs => matches!(host, "img.discogs.com" | "i.discogs.com"),
            ProviderId::Itunes => under("mzstatic.com"),
            ProviderId::Bpmsupreme => false,
            ProviderId::Bandcamp => host == "f4.bcbits.com",
            ProviderId::Deezer => under("dzcdn.net"),
        }
    } else {
        match provider {
            ProviderId::Beatport => matches!(host, "beatport.com" | "www.beatport.com"),
            ProviderId::Traxsource => matches!(host, "traxsource.com" | "www.traxsource.com"),
            ProviderId::Musicbrainz => host == "musicbrainz.org",
            ProviderId::Sonovault => false,
            ProviderId::Discogs => matches!(host, "discogs.com" | "www.discogs.com"),
            ProviderId::Itunes => matches!(host, "music.apple.com" | "itunes.apple.com"),
            ProviderId::Bpmsupreme => host == "app.bpmsupreme.com",
            ProviderId::Bandcamp => under("bandcamp.com"),
            ProviderId::Deezer => matches!(host, "deezer.com" | "www.deezer.com"),
        }
    };
    if !allowed {
        return None;
    }
    if provider == ProviderId::Musicbrainz {
        let segments: Vec<_> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
        if artwork {
            if segments.len() < 2
                || segments[0] != "release"
                || crate::Mbid::new(segments[1]).is_err()
            {
                return None;
            }
        } else if segments.len() != 2
            || !matches!(
                segments[0],
                "recording" | "release" | "release-group" | "artist"
            )
            || crate::Mbid::new(segments[1]).is_err()
        {
            return None;
        }
    }
    Some(url.to_string())
}

/// Validate the original required-field policy after explicit selection edits.
/// This reads the actual staged native values, independent of catalog write selection.
pub fn validate_review_required_fields(
    path: &std::path::Path,
    info: &cuerithm_media::MediaInfo,
    config: &AutotagConfig,
) -> Result<()> {
    let values = crate::fields::values(path, info, &config.required_tags, &[], None)?;
    for (field, value) in config.required_tags.iter().zip(values) {
        ensure!(
            value.is_some(),
            "Required selected field unavailable after explicit review: {field:?}"
        );
    }
    Ok(())
}
