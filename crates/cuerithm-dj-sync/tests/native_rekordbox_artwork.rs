use cuerithm_dj_sync::{native_rekordbox_artwork, normalize_native_rekordbox_artwork};
use cuerithm_media::{CancellationToken, MediaRuntime};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[test]
fn native_jpeg_variants_preserve_aspect_matte_alpha_white_and_stable_observation_bytes() {
    let image = RgbaImage::from_fn(640, 320, |x, y| {
        if x < 160 {
            Rgba([255, 0, 0, 0])
        } else {
            Rgba([(x % 255) as u8, (y % 255) as u8, 120, 255])
        }
    });
    for format in [
        ImageFormat::Png,
        ImageFormat::Gif,
        ImageFormat::WebP,
        ImageFormat::Jpeg,
    ] {
        let mut encoded = Cursor::new(Vec::new());
        let input = if format == ImageFormat::Jpeg {
            DynamicImage::ImageRgba8(image.clone()).to_rgb8().into()
        } else {
            DynamicImage::ImageRgba8(image.clone())
        };
        input.write_to(&mut encoded, format).unwrap();
        let first =
            normalize_native_rekordbox_artwork(encoded.get_ref(), &CancellationToken::default())
                .unwrap();
        let repeated =
            normalize_native_rekordbox_artwork(encoded.get_ref(), &CancellationToken::default())
                .unwrap();
        assert_eq!(first.main, repeated.main);
        assert_eq!(first.medium, repeated.medium);
        assert_eq!(first.small, repeated.small);
        for (bytes, dimensions) in [
            (&first.main, (80, 40)),
            (&first.medium, (240, 120)),
            (&first.small, (320, 160)),
        ] {
            assert_eq!(image::guess_format(bytes).unwrap(), ImageFormat::Jpeg);
            let decoded = image::load_from_memory(bytes).unwrap().to_rgb8();
            assert_eq!(decoded.dimensions(), dimensions);
            if format != ImageFormat::Jpeg {
                assert!(
                    decoded
                        .get_pixel(0, 0)
                        .0
                        .iter()
                        .all(|channel| *channel >= 250)
                );
            }
        }
        assert_eq!(
            first.observation_sha256,
            format!("{:x}", Sha256::digest(&first.main))
        );
        assert_eq!(
            first.source_extract_sha256,
            format!("{:x}", Sha256::digest(encoded.get_ref()))
        );
        let remote_file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(remote_file.path(), &first.main).unwrap();
        assert_eq!(
            first.observation_sha256,
            cuerithm_media::file_sha256(remote_file.path()).unwrap()
        );
    }
}
#[test]
fn artwork_decode_and_cancellation_budgets_fail_closed() {
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(normalize_native_rekordbox_artwork(b"invalid", &cancel).is_err());
    assert!(normalize_native_rekordbox_artwork(b"invalid", &CancellationToken::default()).is_err());
    assert!(
        normalize_native_rekordbox_artwork(
            &vec![0; 8 * 1024 * 1024 + 1],
            &CancellationToken::default()
        )
        .is_err()
    );
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::new(4097, 1))
        .write_to(&mut encoded, ImageFormat::Png)
        .unwrap();
    assert!(
        normalize_native_rekordbox_artwork(encoded.get_ref(), &CancellationToken::default())
            .is_err()
    );
}
#[test]
fn bundled_cover_extraction_normalizes_without_modifying_source_and_can_cancel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cuerithm-media");
    let runtime_path = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("../../tools/media-runtime/bin"));
    let runtime = MediaRuntime::from_bundle(&runtime_path).unwrap();
    let path = root.join("tests/fixtures/covered.flac");
    let before = cuerithm_media::file_sha256(&path).unwrap();
    let raw = runtime.artwork(&path).unwrap().unwrap();
    let normalized = native_rekordbox_artwork(&path, &runtime, &CancellationToken::default())
        .unwrap()
        .unwrap();
    assert_eq!(
        normalized.source_extract_sha256,
        format!("{:x}", Sha256::digest(&raw.bytes))
    );
    assert_eq!(
        normalized.observation_sha256,
        format!("{:x}", Sha256::digest(&normalized.main))
    );
    assert_eq!(before, cuerithm_media::file_sha256(&path).unwrap());
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(native_rekordbox_artwork(&path, &runtime, &cancel).is_err());
    assert!(
        native_rekordbox_artwork(
            &root.join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac"),
            &runtime,
            &CancellationToken::default()
        )
        .unwrap()
        .is_none()
    );
}
