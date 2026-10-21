use anyhow::{Result, ensure};
use cuerithm_media::{CancellationToken, MediaRuntime};
use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, Rgb, RgbImage, codecs::jpeg::JpegEncoder,
    imageops::FilterType,
};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path};

pub struct NativeRekordboxArtwork {
    /// Main JPEG fits within 80 × 80, preserving aspect ratio.
    pub main: Vec<u8>,
    /// Native `_m` JPEG fits within 240 × 240.
    pub medium: Vec<u8>,
    /// Native `_s` JPEG fits within 320 × 320.
    pub small: Vec<u8>,
    /// Compare this hash with the actual native main file's unchanged bytes.
    /// Re-encoding a remote JPEG would change its lossy representation.
    pub observation_sha256: String,
    pub source_extract_sha256: String,
}
/// Normalize JPEG, PNG, GIF or WebP to deterministic quality-90 RGB JPEGs.
/// Select the first frame, apply orientation and composite transparency on white.
pub fn normalize_native_rekordbox_artwork(
    bytes: &[u8],
    cancel: &CancellationToken,
) -> Result<NativeRekordboxArtwork> {
    cancel.check()?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= 8 * 1024 * 1024,
        "Native artwork exceeds the encoded image budget"
    );
    let format = image::guess_format(bytes)?;
    ensure!(
        matches!(
            format,
            ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::Gif | ImageFormat::WebP
        ),
        "Native artwork format is unsupported"
    );
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    cancel.check()?;
    let rgba = image.into_rgba8();
    let mut rgb = RgbImage::new(rgba.width(), rgba.height());
    for (row_index, (source, target)) in rgba.rows().zip(rgb.rows_mut()).enumerate() {
        if row_index.is_multiple_of(64) {
            cancel.check()?;
        }
        for (source, target) in source.zip(target) {
            let alpha = u16::from(source[3]);
            *target = Rgb([0, 1, 2].map(|channel| {
                ((u16::from(source[channel]) * alpha + 255 * (255 - alpha) + 127) / 255) as u8
            }));
        }
    }
    let rgb = DynamicImage::ImageRgb8(rgb);
    let encode = |bounds| -> Result<Vec<u8>> {
        cancel.check()?;
        let resized = rgb.resize(bounds, bounds, FilterType::Lanczos3);
        let mut output = Vec::new();
        JpegEncoder::new_with_quality(&mut output, 90).encode_image(&resized)?;
        cancel.check()?;
        Ok(output)
    };
    let main = encode(80)?;
    let medium = encode(240)?;
    let small = encode(320)?;
    Ok(NativeRekordboxArtwork {
        observation_sha256: format!("{:x}", Sha256::digest(&main)),
        source_extract_sha256: format!("{:x}", Sha256::digest(bytes)),
        main,
        medium,
        small,
    })
}
pub fn native_rekordbox_artwork(
    path: &Path,
    runtime: &MediaRuntime,
    cancel: &CancellationToken,
) -> Result<Option<NativeRekordboxArtwork>> {
    cancel.check()?;
    runtime
        .artwork_cancellable(path, cancel)?
        .map(|artwork| normalize_native_rekordbox_artwork(&artwork.bytes, cancel))
        .transpose()
}
