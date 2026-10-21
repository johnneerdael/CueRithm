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
    let domain = match provider {
        ProviderId::Beatport => "beatport.com",
        ProviderId::Traxsource => "traxsource.com",
    };
    let host = url.host_str().context("Missing artwork host")?;
    ensure!(
        host == domain || host.ends_with(&format!(".{domain}")),
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

/// Fetch only an explicitly selected catalog cover. No credentials are sent.
pub fn fetch_catalog_artwork(
    provider: ProviderId,
    value: &str,
    cancel: &CancellationToken,
) -> Result<Vec<u8>> {
    cancel.check()?;
    let url = approved_url(provider, value)?;
    let origin = url.origin();
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .user_agent("CueRithm catalog artwork (https://github.com/johnneerdael/CueRithm)")
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
    let response = client.get(url).send().map_err(|error| {
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
