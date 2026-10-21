//! Explicit catalog-cover reads, confined to the selected provider's image origin.
use crate::{ProviderId, image::Artwork};
use anyhow::{Context, Result, ensure};
use cuerithm_media::CancellationToken;
use reqwest::Url;
use std::{io::Read, time::Duration};

const MAX_BYTES: usize = 8 * 1024 * 1024;

fn approved_url(provider: ProviderId, value: &str) -> Result<Url> {
    let url = Url::parse(value).context("Invalid catalog artwork URL")?;
    ensure!(url.scheme() == "https", "Catalog artwork requires HTTPS");
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Artwork URL cannot contain credentials"
    );
    ensure!(
        url.port_or_known_default() == Some(443),
        "Artwork requires the standard HTTPS port"
    );
    ensure!(
        crate::review_catalog_url(provider, value, true).is_some(),
        "Artwork origin is not approved for this provider"
    );
    Ok(url)
}

fn read_bounded(mut reader: impl Read, cancel: &CancellationToken) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut block = [0; 64 * 1024];
    loop {
        cancel.check()?;
        let remaining = MAX_BYTES + 1 - bytes.len();
        let count = match reader.read(&mut block[..remaining.min(64 * 1024)]) {
            Ok(count) => count,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&block[..count]);
        ensure!(bytes.len() <= MAX_BYTES, "Artwork exceeds 8 MiB");
    }
    cancel.check()?;
    Ok(bytes)
}

fn artwork_request(provider: ProviderId, url: Url) -> Result<reqwest::blocking::RequestBuilder> {
    let origin = url.origin();
    let traxsource = provider == ProviderId::Traxsource;
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .user_agent("CueRithm catalog artwork (https://github.com/johnneerdael/CueRithm)")
        // Keep Traxsource's site Referer fixed across permitted image redirects.
        .referer(!traxsource)
        .redirect(reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 3 {
                attempt.error("Too many artwork redirects")
            } else if attempt.url().origin() != origin
                || approved_url(provider, attempt.url().as_str()).is_err()
            {
                attempt.error("Artwork redirected outside its approved origin")
            } else {
                attempt.follow()
            }
        }))
        .build()?;
    let request = client.get(url);
    Ok(if traxsource {
        request.header(reqwest::header::REFERER, "https://www.traxsource.com/")
    } else {
        request
    })
}

/// Fetch only an explicitly selected catalog cover. No credentials are sent.
pub fn fetch_catalog_artwork(
    provider: ProviderId,
    value: &str,
    cancel: &CancellationToken,
) -> Result<Vec<u8>> {
    cancel.check()?;
    ensure!(
        matches!(provider, ProviderId::Beatport | ProviderId::Traxsource),
        "This provider requires the shared catalog runtime for artwork"
    );
    let url = approved_url(provider, value)?;
    let response = artwork_request(provider, url)?.send().map_err(|error| {
        // Reqwest errors can embed signed image URLs. Report only classification.
        if error.is_timeout() {
            anyhow::anyhow!("Catalog artwork request timed out")
        } else {
            anyhow::anyhow!("Catalog artwork request failed")
        }
    })?;
    ensure!(
        response.status().is_success(),
        "Catalog artwork returned HTTP {}",
        response.status().as_u16()
    );
    ensure!(
        response
            .content_length()
            .is_none_or(|n| n <= MAX_BYTES as u64),
        "Artwork exceeds 8 MiB"
    );
    let image = Artwork::validate(read_bounded(response, cancel)?)?;
    Ok(image.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_traxsource_cover_uses_its_fixed_site_referer_only() {
        let beatport = artwork_request(
            ProviderId::Beatport,
            approved_url(
                ProviderId::Beatport,
                "https://geo-media.beatport.com/cover.jpg",
            )
            .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
        assert!(!beatport.headers().contains_key(reqwest::header::REFERER));
        let traxsource = artwork_request(
            ProviderId::Traxsource,
            approved_url(
                ProviderId::Traxsource,
                "https://geo-static.traxsource.com/cover.jpg",
            )
            .unwrap(),
        )
        .unwrap()
        .build()
        .unwrap();
        assert_eq!(
            traxsource
                .headers()
                .get(reqwest::header::REFERER)
                .and_then(|value| value.to_str().ok()),
            Some("https://www.traxsource.com/")
        );
    }

    #[test]
    fn selected_catalog_origin_excludes_insecure_private_and_credential_urls() {
        for value in [
            "http://geo-media.beatport.com/cover.jpg",
            "https://user:secret@geo-media.beatport.com/cover.jpg",
            "https://geo-media.beatport.com:8443/cover.jpg",
            "https://beatport.com.example.org/cover.jpg",
            "https://notbeatport.com/cover.jpg",
            "https://127.0.0.1/cover.jpg",
            "https://[::1]/cover.jpg",
            "file:///private/cover.jpg",
            "https://geo-static.traxsource.com/cover.jpg",
        ] {
            assert!(
                approved_url(ProviderId::Beatport, value).is_err(),
                "accepted {value}"
            );
        }
        assert!(
            approved_url(
                ProviderId::Beatport,
                "https://geo-media.beatport.com/cover.jpg"
            )
            .is_ok()
        );
        assert!(
            approved_url(
                ProviderId::Traxsource,
                "https://geo-static.traxsource.com/cover.jpg"
            )
            .is_ok()
        );
        assert!(
            approved_url(
                ProviderId::Traxsource,
                "https://geo-media.beatport.com/cover.jpg"
            )
            .is_err()
        );
    }
    #[test]
    fn unknown_length_bodies_are_bounded_and_cancellation_owns_the_read() {
        let cancel = CancellationToken::default();
        assert!(
            read_bounded(std::io::repeat(0), &cancel)
                .unwrap_err()
                .to_string()
                .contains("8 MiB")
        );
        cancel.cancel();
        assert!(read_bounded(&b"unused"[..], &cancel).is_err());
    }
}

/// A selected image's stable release/image identity, separate from its bytes.
pub struct FetchedMusicBrainzArtwork {
    pub reference: crate::MusicBrainzCoverRef,
    pub provenance: CatalogArtworkProvenance,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatalogArtworkProvenance {
    pub release_id: crate::Mbid,
    pub image_id: String,
    pub source_url: String,
    pub approved: bool,
    pub front: bool,
    pub types: Vec<String>,
    pub rights: CatalogArtworkRights,
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogArtworkRights {
    Unspecified,
}
pub fn fetch_musicbrainz_artwork(
    runtime: &crate::CatalogRuntime,
    reference: &crate::MusicBrainzCoverRef,
    cancel: &CancellationToken,
) -> Result<Option<FetchedMusicBrainzArtwork>> {
    cancel.check()?;
    let flag = cancel.shared_flag();
    let root = format!(
        "https://coverartarchive.org/release/{}",
        reference.release_id
    );
    let value = match runtime.get_json(
        crate::CatalogService::CoverArtArchive,
        &format!("{root}/"),
        &[],
        None,
        &flag,
    ) {
        Ok(v) => v,
        Err(error)
            if error
                .downcast_ref::<crate::CatalogError>()
                .is_some_and(|e| e.status == Some(404)) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    if let Some(release) = value.get("release").and_then(serde_json::Value::as_str) {
        let release = Url::parse(release)?;
        ensure!(
            release.host_str() == Some("musicbrainz.org")
                && release.path().trim_end_matches('/')
                    == format!("/release/{}", reference.release_id),
            "CAA index belongs to another release"
        );
    }
    let images = value
        .get("images")
        .and_then(serde_json::Value::as_array)
        .context("Malformed CAA index")?;
    ensure!(images.len() <= 256, "CAA image index exceeds bounds");
    let selected = images.iter().find_map(|image| {
        let id = image.get("id").and_then(|id| {
            id.as_str()
                .map(str::to_owned)
                .or_else(|| id.as_u64().map(|id| id.to_string()))
        })?;
        if id.len() > 32
            || !id.bytes().all(|c| c.is_ascii_digit())
            || image.get("approved").and_then(serde_json::Value::as_bool) != Some(true)
        {
            return None;
        }
        if reference.image_id.as_ref().map_or_else(
            || image.get("front").and_then(serde_json::Value::as_bool) == Some(true),
            |expected| expected == &id,
        ) {
            Some((id, image))
        } else {
            None
        }
    });
    let Some((id, image)) = selected else {
        return Ok(None);
    };
    let provenance = CatalogArtworkProvenance {
        release_id: reference.release_id.clone(),
        image_id: id.clone(),
        source_url: format!("{root}/{id}-500"),
        approved: true,
        front: image.get("front").and_then(serde_json::Value::as_bool) == Some(true),
        types: image
            .get("types")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str)
            .take(32)
            .map(|s| s.chars().take(128).collect())
            .collect(),
        rights: CatalogArtworkRights::Unspecified,
    };
    let bytes = match runtime.get_bytes(
        crate::CatalogService::CoverArtArchive,
        &format!("{root}/{id}-500"),
        &[],
        None,
        &flag,
    ) {
        Ok(bytes) => bytes,
        Err(error)
            if error
                .downcast_ref::<crate::CatalogError>()
                .is_some_and(|e| e.status == Some(404)) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(error),
    };
    cancel.check()?;
    let bytes = Artwork::validate(bytes)?.bytes;
    Ok(Some(FetchedMusicBrainzArtwork {
        reference: crate::MusicBrainzCoverRef {
            release_id: reference.release_id.clone(),
            image_id: Some(id),
        },
        provenance,
        bytes,
    }))
}
/// Shared official transport; source URLs are never arbitrary HTTP destinations.
pub fn fetch_catalog_artwork_with_runtime(
    runtime: &crate::CatalogRuntime,
    provider: ProviderId,
    value: &str,
    cancel: &CancellationToken,
) -> Result<Vec<u8>> {
    cancel.check()?;
    ensure!(
        crate::provider_native_field_allowed(provider.as_str(), crate::SupportedTag::AlbumArt),
        "Source policy does not permit artwork"
    );
    let url = approved_url(provider, value)?;
    if provider == ProviderId::Musicbrainz {
        let parts: Vec<_> = url
            .path_segments()
            .context("Missing CAA path")?
            .filter(|s| !s.is_empty())
            .collect();
        ensure!(
            parts.len() == 3 && parts[0] == "release",
            "CAA artwork requires a selected release reference"
        );
        let release_id = crate::Mbid::new(parts[1])?;
        let image_id = if parts[2] == "front-500" {
            None
        } else {
            Some(
                parts[2]
                    .strip_suffix("-500")
                    .context("Unsupported CAA image size")?
                    .to_owned(),
            )
        };
        if let Some(id) = &image_id {
            ensure!(
                !id.is_empty() && id.len() <= 32 && id.bytes().all(|c| c.is_ascii_digit()),
                "Invalid CAA image identity"
            );
        }
        return Ok(fetch_musicbrainz_artwork(
            runtime,
            &crate::MusicBrainzCoverRef {
                release_id,
                image_id,
            },
            cancel,
        )?
        .context("No approved cover exists for the selected release")?
        .bytes);
    }
    let service = match provider {
        ProviderId::Beatport => crate::CatalogService::Beatport,
        ProviderId::Traxsource => crate::CatalogService::Traxsource,
        ProviderId::Discogs => crate::CatalogService::Discogs,
        ProviderId::Volumo => crate::CatalogService::Volumo,
        ProviderId::Itunes => crate::CatalogService::ITunes,
        ProviderId::Bandcamp => crate::CatalogService::Bandcamp,
        ProviderId::Deezer => crate::CatalogService::Deezer,
        ProviderId::Bpmsupreme | ProviderId::Sonovault | ProviderId::Musicbrainz => {
            anyhow::bail!("Artwork is unavailable for this source")
        }
    };
    let bytes = runtime.get_bytes(service, url.as_str(), &[], None, &cancel.shared_flag())?;
    cancel.check()?;
    Ok(Artwork::validate(bytes)?.bytes)
}
