use cuerithm_dj_sync::*;
use cuerithm_media::{CancellationToken, MediaRuntime};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn runtime() -> MediaRuntime {
    let root = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    MediaRuntime::from_bundle(&root)
        .expect("packaged runtime is required for all native source layouts")
}
fn fixtures() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let file = std::fs::File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fixtures/media.tar.gz"),
    )
    .unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(file))
        .unpack(directory.path())
        .unwrap();
    directory
}
#[test]
fn native_compatibility_preserves_supported_sources_and_requires_owned_opt_in_copies() {
    let fixtures = fixtures();
    let workspace = tempfile::tempdir().unwrap();
    let runtime = runtime();
    let cancel = CancellationToken::default();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.path().join("cases.json")).unwrap())
            .unwrap();
    for case in cases.as_array().unwrap() {
        let filename = case["file"].as_str().unwrap();
        let path = fixtures.path().join(filename);
        let audio = runtime.decode(&path, fixtures.path()).unwrap();
        let expected = match filename {
            "pulses.mp3" => Some(1),
            "pulses.m4a" => Some(4),
            "pulses.alac.m4a" => Some(6),
            "pulses.flac" => Some(5),
            "pulses.aiff" => Some(12),
            "pulses.wav" | "pulses.24.wav" | "pulses.mono48.wav" | "pulses.stereo96.wav" => {
                Some(11)
            }
            _ => None,
        };
        assert_eq!(
            native_rekordbox_file_type(&audio.info, &path).ok(),
            expected,
            "{filename}"
        );
        let inspected = inspect_native_rekordbox_source(&path, &runtime, &cancel).unwrap();
        assert_eq!(inspected.compatibility.native_file_type, expected);
        assert_eq!(inspected.original.sha256, audio.source_sha256);
        assert_eq!(inspected.original.pcm_sha256, audio.pcm_sha256().unwrap());
        if let Some(file_type) = expected {
            let prepared =
                prepare_native_rekordbox_source(&path, workspace.path(), false, &runtime, &cancel)
                    .unwrap();
            let descriptor = &prepared.descriptor;
            assert!(!descriptor.converted);
            assert_eq!(descriptor.native_file_type, file_type);
            assert_eq!(descriptor.native_path, path.canonicalize().unwrap());
            assert_eq!(descriptor.sample_rate, audio.info.sample_rate);
            assert_eq!(descriptor.sha256, audio.source_sha256);
            assert_eq!(inspected.estimated_copy_bytes, 0);
        } else {
            assert!(
                inspected
                    .compatibility
                    .reason
                    .as_ref()
                    .is_some_and(|reason| !reason.is_empty())
            );
            assert_eq!(inspected.estimated_copy_bytes, 44 + audio.frames * 4);
            assert!(
                prepare_native_rekordbox_source(&path, workspace.path(), false, &runtime, &cancel)
                    .is_err()
            );
            assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
            let prepared =
                prepare_native_rekordbox_source(&path, workspace.path(), true, &runtime, &cancel)
                    .unwrap();
            let descriptor = &prepared.descriptor;
            assert!(descriptor.converted);
            assert!(descriptor.native_path.starts_with(workspace.path()));
            assert_ne!(descriptor.native_path, path);
            assert_eq!(
                (
                    descriptor.native_file_type,
                    descriptor.sample_rate,
                    descriptor.bits_per_sample
                ),
                (11, 44100, 16)
            );
            assert_eq!(descriptor.size_bytes, inspected.estimated_copy_bytes);
            assert_eq!(descriptor.original.pcm_sha256, audio.pcm_sha256().unwrap());
            assert_eq!(descriptor.original.sha256, audio.source_sha256);
            let derivative = runtime
                .decode(&descriptor.native_path, fixtures.path())
                .unwrap();
            assert_eq!(derivative.frames, audio.frames);
            // Integer PCM has one zero representation; normalize the float
            // quantizer's negative zero before comparing exact decoded bytes.
            let expected_pcm: Vec<u8> = audio
                .stereo_s16()
                .unwrap()
                .into_iter()
                .map(|sample| if sample == 0. { 0. } else { sample })
                .flat_map(f32::to_le_bytes)
                .collect();
            assert_eq!(
                derivative.pcm_sha256().unwrap(),
                format!("{:x}", Sha256::digest(&expected_pcm)),
                "{filename}"
            );
            assert_eq!(descriptor.pcm_sha256, derivative.pcm_sha256().unwrap());
            let timed = timing::export_track(
                &SyncTrack {
                    path: descriptor.native_path.clone(),
                    sample_rate: descriptor.sample_rate,
                    export_timing: Some(descriptor.export_timing.clone()),
                    ..Default::default()
                },
                DjSoftware::Rekordbox,
            )
            .unwrap();
            assert_eq!(timed.native_timeline.unwrap().offset_seconds, 0.);
            assert_eq!(
                descriptor.sha256,
                cuerithm_media::file_sha256(&descriptor.native_path).unwrap()
            );
            let owned_path = descriptor.native_path.clone();
            drop(prepared);
            assert!(!owned_path.exists());
        }
        assert!(audio.source_is_unchanged().unwrap(), "{filename}");
        assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
    }
}
#[test]
fn cancellation_and_renamed_adts_never_create_a_native_playable_entry() {
    let fixtures = fixtures();
    let workspace = tempfile::tempdir().unwrap();
    let runtime = runtime();
    let cancel = CancellationToken::default();
    cancel.cancel();
    assert!(
        prepare_native_rekordbox_source(
            &fixtures.path().join("pulses.opus"),
            workspace.path(),
            true,
            &runtime,
            &cancel
        )
        .is_err()
    );
    assert_eq!(std::fs::read_dir(workspace.path()).unwrap().count(), 0);
    let renamed = fixtures.path().join("renamed.m4a");
    std::fs::copy(fixtures.path().join("pulses.aac"), &renamed).unwrap();
    let info = runtime.probe(&renamed).unwrap();
    assert!(native_rekordbox_file_type(&info, &renamed).is_err());
    let mut alac = runtime
        .probe(&fixtures.path().join("pulses.alac.m4a"))
        .unwrap();
    assert_eq!(
        native_rekordbox_file_type(&alac, &fixtures.path().join("pulses.alac.m4a")).unwrap(),
        6
    );
    alac.codec = "aac".into();
    alac.properties
        .insert("audio.profile".into(), "HE-AAC".into());
    assert!(native_rekordbox_file_type(&alac, &fixtures.path().join("pulses.alac.m4a")).is_err());
}
