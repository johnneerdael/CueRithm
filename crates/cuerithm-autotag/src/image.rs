use anyhow::{Context, Result, ensure};
use std::io::{Cursor, Read};
const MAX: usize = 8 * 1024 * 1024;
#[derive(Clone)]
pub(crate) struct Artwork {
    pub mime: &'static str,
    pub bytes: Vec<u8>,
}
impl Artwork {
    pub fn validate(bytes: Vec<u8>) -> Result<Self> {
        ensure!(bytes.len() <= MAX, "Artwork exceeds 8 MiB");
        let mime = cuerithm_media::artwork_mime(&bytes).context("Unsupported artwork image")?;
        let mut reader = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(4096);
        limits.max_image_height = Some(4096);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        reader
            .decode()
            .context("Invalid or oversized artwork image")?;
        Ok(Self { mime, bytes })
    }
    pub fn fetch(url: &str, platform: &str) -> Result<Self> {
        let url = reqwest::Url::parse(url)?;
        ensure!(
            matches!(url.scheme(), "http" | "https"),
            "Artwork requires HTTP or HTTPS"
        );
        let traxsource = platform == "traxsource";
        let mut request = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            // Preserve Traxsource's required site Referer across artwork redirects.
            .referer(!traxsource)
            .build()?
            .get(url);
        if traxsource {
            request = request.header(reqwest::header::REFERER, "https://www.traxsource.com/");
        }
        let response = request.send()?.error_for_status()?;
        ensure!(
            response.content_length().is_none_or(|n| n <= MAX as u64),
            "Artwork exceeds 8 MiB"
        );
        let mut bytes = Vec::new();
        response.take(MAX as u64 + 1).read_to_end(&mut bytes)?;
        Self::validate(bytes)
    }
}
