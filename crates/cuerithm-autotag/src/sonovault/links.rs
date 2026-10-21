use super::*;
use crate::{SourceCatalogIdentityKind, SourceCatalogLink};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SonoVaultLookup {
    TrackId(u64),
    Isrc(String),
    BeatportTrackId(u64),
    DiscogsReleaseTrack { release_id: u64, position: String },
    MusicBrainzRecording(crate::Mbid),
    AppleMusicTrackId(u64),
    TidalTrackId(u64),
    YoutubeVideoId(String),
}
impl SonoVaultLookup {
    fn query(&self) -> Result<(String, String)> {
        let pair = match self {
            Self::TrackId(id) => ("id", id.to_string()),
            Self::Isrc(id) => {
                let id = id.replace('-', "").to_ascii_uppercase();
                ensure!(valid_isrc(&id), "Invalid source ISRC");
                return Ok(("isrc".into(), id));
            }
            Self::BeatportTrackId(id) => ("beatport_id", id.to_string()),
            Self::DiscogsReleaseTrack {
                release_id,
                position,
            } => {
                numeric_id(&release_id.to_string())?;
                position_valid(position)?;
                return Ok(("discogs_id".into(), format!("{release_id}-{position}")));
            }
            Self::MusicBrainzRecording(id) => return Ok(("musicbrainz_id".into(), id.to_string())),
            Self::AppleMusicTrackId(id) => ("applemusic_id", id.to_string()),
            Self::TidalTrackId(id) => ("tidal_id", id.to_string()),
            Self::YoutubeVideoId(id) => {
                video_valid(id)?;
                return Ok(("youtube_id".into(), id.clone()));
            }
        };
        numeric_id(&pair.1)?;
        Ok((pair.0.into(), pair.1))
    }
    fn external(&self) -> Result<Option<(&'static str, String)>> {
        Ok(match self {
            Self::BeatportTrackId(id) => Some(("beatport", id.to_string())),
            Self::DiscogsReleaseTrack {
                release_id,
                position,
            } => Some(("discogs", format!("{release_id}-{position}"))),
            Self::MusicBrainzRecording(id) => Some(("musicbrainz", id.to_string())),
            Self::AppleMusicTrackId(id) => Some(("applemusic", id.to_string())),
            Self::TidalTrackId(id) => Some(("tidal", id.to_string())),
            Self::YoutubeVideoId(id) => Some(("youtube", id.clone())),
            _ => None,
        })
    }
}
fn position_valid(value: &str) -> Result<()> {
    ensure!(
        !value.trim().is_empty()
            && value.len() <= 128
            && value.bytes().all(|b| b.is_ascii() && !b.is_ascii_control()),
        "Invalid Discogs release track position"
    );
    Ok(())
}
fn video_valid(value: &str) -> Result<()> {
    ensure!(
        value.len() == 11
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "Invalid YouTube track identity"
    );
    Ok(())
}
fn discogs_parts(value: &str) -> Result<(&str, &str)> {
    let (release, position) = value
        .split_once('-')
        .context("Discogs requires a release and raw track position")?;
    numeric_id(release)?;
    position_valid(position)?;
    Ok((release, position))
}
fn canonical_link(source: &str, id: &str, raw: &str) -> Result<String> {
    let url = reqwest::Url::parse(raw).map_err(|_| anyhow::anyhow!("Invalid platform-link URL"))?;
    ensure!(
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.username().is_empty()
            && url.password().is_none(),
        "Platform link origin is not approved"
    );
    let host = url.host_str().unwrap_or("");
    let parts = url
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let (target, canonical) = match source {
        "spotify" => (
            host == "open.spotify.com" && parts.as_slice() == ["track", id],
            format!("https://open.spotify.com/track/{id}"),
        ),
        "beatport" => (
            matches!(host, "www.beatport.com" | "beatport.com")
                && parts.first() == Some(&"track")
                && parts.last() == Some(&id),
            format!("https://www.beatport.com/track/-/{id}"),
        ),
        "discogs" => {
            let (release, _) = discogs_parts(id)?;
            (
                matches!(host, "www.discogs.com" | "discogs.com")
                    && parts.first() == Some(&"release")
                    && parts
                        .get(1)
                        .is_some_and(|p| p.split('-').next() == Some(release)),
                format!("https://www.discogs.com/release/{release}"),
            )
        }
        "musicbrainz" => (
            host == "musicbrainz.org" && parts.as_slice() == ["recording", id],
            format!("https://musicbrainz.org/recording/{id}"),
        ),
        "applemusic" => {
            let exact = parts.last() == Some(&id) && parts.contains(&"song");
            let album = parts.contains(&"album")
                && url
                    .query_pairs()
                    .any(|(key, value)| key == "i" && value == id);
            (
                host == "music.apple.com" && (exact || album),
                format!("https://music.apple.com/song/{id}"),
            )
        }
        "tidal" => (
            matches!(host, "tidal.com" | "www.tidal.com" | "open.tidal.com")
                && parts.last() == Some(&id)
                && parts.contains(&"track"),
            format!("https://tidal.com/browse/track/{id}"),
        ),
        "youtube" => {
            let watch = matches!(host, "www.youtube.com" | "youtube.com")
                && url.path() == "/watch"
                && url
                    .query_pairs()
                    .any(|(key, value)| key == "v" && value == id);
            let short = host == "youtu.be" && parts.as_slice() == [id];
            (watch || short, format!("https://youtu.be/{id}"))
        }
        _ => anyhow::bail!("Unsupported platform-link source"),
    };
    ensure!(
        target,
        "Platform-link URL does not bind its source identity"
    );
    Ok(canonical)
}
struct ResolvedLinks {
    track_id: u64,
    requested_isrc: Option<String>,
    identities: Vec<SourceCatalogIdentity>,
    links: Vec<SourceCatalogLink>,
}
fn parse_links(value: &Value, lookup: &SonoVaultLookup) -> Result<ResolvedLinks> {
    let query = lookup.query()?;
    let requested_isrc = if matches!(lookup, SonoVaultLookup::Isrc(_)) {
        let actual = value
            .get("isrc")
            .and_then(Value::as_str)
            .context("Platform links did not prove the requested ISRC")?;
        ensure!(
            normalize_isrc(actual)? == query.1,
            "Platform links resolved another ISRC"
        );
        Some(query.1)
    } else {
        None
    };
    let track_id = value
        .get("track_id")
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
        .context("Platform links have no recording identity")?;
    if let SonoVaultLookup::TrackId(expected) = lookup {
        ensure!(
            *expected == track_id,
            "Platform links resolved another recording"
        );
    }
    ensure!(
        text(value, "title").is_some(),
        "Platform links have no recording title"
    );
    let rows = value
        .get("links")
        .and_then(Value::as_array)
        .context("Malformed platform-link collection")?;
    ensure!(
        rows.len() <= 32,
        "Platform-link collection exceeds evidence bound"
    );
    let mut identities = Vec::new();
    let mut links = Vec::new();
    for row in rows {
        let source = text(row, "source").context("Platform link source missing")?;
        let id = text(row, "external_id").context("Platform link identity missing")?;
        match source.as_str() {
            "spotify" => ensure!(
                id.len() == 22 && id.bytes().all(|c| c.is_ascii_alphanumeric()),
                "Invalid Spotify metadata identity"
            ),
            "beatport" | "applemusic" | "tidal" => numeric_id(&id)?,
            "discogs" => {
                discogs_parts(&id)?;
            }
            "musicbrainz" => {
                crate::Mbid::new(id.clone())?;
            }
            "youtube" => video_valid(&id)?,
            _ => anyhow::bail!("Unsupported platform-link source"),
        }
        let identity = SourceCatalogIdentity {
            source: source.clone(),
            kind: SourceCatalogIdentityKind::PlatformTrack,
            id: id.clone(),
        };
        if !identities.contains(&identity) {
            identities.push(identity)
        }
        match row.get("url") {
            Some(Value::Null) => {}
            Some(Value::String(raw)) => {
                let url = canonical_link(&source, &id, raw)?;
                let link = SourceCatalogLink { source, url };
                if !links.contains(&link) {
                    links.push(link)
                }
            }
            _ => anyhow::bail!("Malformed platform-link URL"),
        }
    }
    if let Some((source, id)) = lookup.external()? {
        ensure!(
            identities.iter().any(|v| v.source == source && v.id == id),
            "Platform links do not prove the requested source identity"
        );
    }
    Ok(ResolvedLinks {
        track_id,
        requested_isrc,
        identities,
        links,
    })
}
impl SonoVaultBackend {
    fn fetch_links(
        &self,
        lookup: &SonoVaultLookup,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Option<ResolvedLinks>> {
        let query = lookup.query()?;
        match self.get(
            "https://api.sonovault.now/v1/tracks/links",
            &[query],
            cancel,
        ) {
            Ok(body) => Ok(Some(parse_links(&body, lookup)?)),
            Err(e)
                if e.downcast_ref::<CatalogError>()
                    .is_some_and(|e| e.kind == CatalogErrorKind::NotFound) =>
            {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }
    pub fn resolve_platform_lookup(
        &self,
        lookup: &SonoVaultLookup,
        cancel: &Arc<AtomicBool>,
    ) -> Result<Option<CatalogMatch>> {
        let Some(links) = self.fetch_links(lookup, cancel)? else {
            return Ok(None);
        };
        let body = self.get(
            &format!("https://api.sonovault.now/v1/tracks/{}", links.track_id),
            &[],
            cancel,
        )?;
        let mut candidate = parse_track(&body)?;
        let reason = if links.requested_isrc.is_some() {
            onetagger_tagger::MatchReason::ISRC
        } else {
            onetagger_tagger::MatchReason::ID
        };
        apply_links(&mut candidate, links)?;
        candidate.reason = reason;
        candidate.accuracy = 1.;
        Ok(Some(candidate))
    }
    pub(super) fn enrich_platform_links(
        &self,
        candidate: &mut CatalogMatch,
        cancel: &Arc<AtomicBool>,
    ) -> Result<()> {
        let id = candidate
            .track_id
            .as_deref()
            .context("Missing SonoVault recording identity")?
            .parse::<u64>()?;
        if let Some(links) = self.fetch_links(&SonoVaultLookup::TrackId(id), cancel)? {
            apply_links(candidate, links)?;
        }
        Ok(())
    }
}
fn normalize_isrc(value: &str) -> Result<String> {
    let normalized = value.replace('-', "").to_ascii_uppercase();
    ensure!(valid_isrc(&normalized), "Malformed platform-link ISRC");
    Ok(normalized)
}
fn apply_links(candidate: &mut CatalogMatch, links: ResolvedLinks) -> Result<()> {
    if let Some(expected) = &links.requested_isrc {
        let actual = candidate
            .isrc
            .as_deref()
            .context("Recording detail did not retain the requested ISRC proof")?;
        ensure!(
            normalize_isrc(actual)? == *expected,
            "Recording detail returned another ISRC"
        );
    }

    ensure!(
        candidate.track_id.as_deref() == Some(links.track_id.to_string().as_str()),
        "Platform links changed recording identity"
    );
    let Some(CatalogFacts::Source(facts)) = candidate.facts.as_mut() else {
        anyhow::bail!("Missing source facts")
    };
    ensure!(
        facts.provider == "sonovault",
        "Platform links cannot alter another provider"
    );
    for id in links.identities {
        if !facts.identities.contains(&id) {
            facts.identities.push(id)
        }
    }
    for link in links.links {
        if !facts.links.contains(&link) {
            facts.links.push(link)
        }
    }
    candidate.validate()?;
    Ok(())
}
pub(super) fn known_source_lookup(input: &TagInput) -> Result<Option<SonoVaultLookup>> {
    for (aliases, source) in [
        (&["sonovault_track_id"][..], "sonovault"),
        (
            &["musicbrainz_trackid", "musicbrainz_recordingid"][..],
            "musicbrainz",
        ),
        (&["beatport_track_id"][..], "beatport"),
        (&["discogs_track_id"][..], "discogs"),
        (
            &["itunes_track_id", "applemusic_track_id"][..],
            "applemusic",
        ),
        (&["tidal_track_id"][..], "tidal"),
        (&["youtube_track_id"][..], "youtube"),
    ] {
        let mut values = input
            .media
            .tags
            .iter()
            .filter(|(key, _)| {
                aliases
                    .iter()
                    .any(|alias| crate::candidate::native_identity_key_matches(key, alias))
            })
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>();
        values.sort_unstable();
        values.dedup();
        ensure!(values.len() <= 1, "Conflicting source recording identities");
        let Some(value) = values.first() else {
            continue;
        };
        return Ok(Some(match source {
            "sonovault" => {
                numeric_id(value)?;
                SonoVaultLookup::TrackId(value.parse()?)
            }
            "musicbrainz" => SonoVaultLookup::MusicBrainzRecording(crate::Mbid::new(*value)?),
            "beatport" => {
                numeric_id(value)?;
                SonoVaultLookup::BeatportTrackId(value.parse()?)
            }
            "discogs" => {
                let (release, position) = discogs_parts(value)?;
                SonoVaultLookup::DiscogsReleaseTrack {
                    release_id: release.parse()?,
                    position: position.into(),
                }
            }
            "applemusic" => {
                numeric_id(value)?;
                SonoVaultLookup::AppleMusicTrackId(value.parse()?)
            }
            "tidal" => {
                numeric_id(value)?;
                SonoVaultLookup::TidalTrackId(value.parse()?)
            }
            _ => {
                video_valid(value)?;
                SonoVaultLookup::YoutubeVideoId((*value).into())
            }
        }));
    }
    Ok(None)
}
#[cfg(test)]
mod tests;
