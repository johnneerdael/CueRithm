use camelot_core::{Engine, EngineSettings};
use cuerithm_media::MediaRuntime;
use std::path::{Path, PathBuf};

fn runtime() -> MediaRuntime {
    let dir = std::env::var_os("CUERITHM_MEDIA_RUNTIME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/media-runtime/bin")
        });
    MediaRuntime::from_bundle(&dir).unwrap()
}
fn settings(dir: &Path) -> EngineSettings {
    let path = dir.join("test-energy.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&serde_json::json!({
            "gamma":0.000001,"classes":[3,8],"n_support":[1,1],
            "support_vectors":[vec![0.;183],vec![1.;183]],
            "dual_coef":[[1.,-1.]],"intercept":[0.1]
        }))
        .unwrap(),
    )
    .unwrap();
    EngineSettings {
        source_profile_timing: true,
        lexicon_config: Some(Default::default()),
        energy_model: Some(
            std::env::var_os("CAMELOT_ENERGY_MODEL")
                .map(PathBuf::from)
                .unwrap_or(path),
        ),
        key_openkeyscan_model: std::env::var_os("CAMELOT_OPENKEYSCAN_MODEL").map(PathBuf::from),
        ..Default::default()
    }
}
#[test]
fn prepared_flac_preserves_existing_key_grid_energy_and_cues() {
    let d = tempfile::tempdir().unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let settings = settings(d.path());
    let mut e = Engine::new(&settings).unwrap();
    let old = e.analyze_file(&path).unwrap();
    let audio = runtime().decode(&path, d.path()).unwrap();
    let new = e.analyze_prepared(&audio).unwrap();
    for field in ["key", "tempo", "energy", "lexicon_cues"] {
        assert_eq!(
            serde_json::to_value(&old.analysis).unwrap()[field],
            serde_json::to_value(&new.analysis).unwrap()[field],
            "{field}"
        );
    }
    assert!(new.complete);
}
#[test]
fn common_formats_receive_full_analysis_without_flac_container_restrictions() {
    let d = tempfile::tempdir().unwrap();
    let archive = std::fs::File::open(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/fixtures/media.tar.gz"),
    )
    .unwrap();
    tar::Archive::new(flate2::read::GzDecoder::new(archive))
        .unpack(d.path())
        .unwrap();
    let cases: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.path().join("cases.json")).unwrap()).unwrap();
    let mut engine = Engine::new(&settings(d.path())).unwrap();
    let rt = runtime();
    let mut lossless_key = None;
    for row in cases.as_array().unwrap() {
        let path = d.path().join(row["file"].as_str().unwrap());
        let audio = rt.decode(&path, d.path()).unwrap();
        let result = engine
            .analyze_prepared(&audio)
            .unwrap_or_else(|e| panic!("{}: {e:#}", path.display()));
        assert!(result.complete);
        assert!(result.analysis.key.is_some());
        assert!(result.analysis.tempo.is_some());
        assert!(result.analysis.energy.is_some());
        let cues = result.analysis.lexicon_cues.as_ref().unwrap();
        assert!(!cues.cues.is_empty());
        assert!(
            cues.cues
                .iter()
                .all(|c| c.time_seconds >= 0. && c.time_seconds < audio.clock.duration_seconds())
        );
        assert!((result.analysis.duration_seconds - audio.clock.duration_seconds()).abs() < 0.0001);
        if row["lossless"] == true {
            let key = serde_json::to_value(&result.analysis.key).unwrap();
            if let Some(ref expected) = lossless_key {
                assert_eq!(&key, expected);
            } else {
                lossless_key = Some(key);
            }
        }
    }
}

#[test]
fn prepared_analysis_rejects_changed_sources_and_truncated_settings() {
    let d = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../camelot-source-analysis/tests/fixtures/pulses64-stereo16.flac");
    let path = d.path().join("source.flac");
    std::fs::copy(source, &path).unwrap();
    let audio = runtime().decode(&path, d.path()).unwrap();
    let mut engine = Engine::new(&EngineSettings {
        max_seconds: Some(1.),
        ..Default::default()
    })
    .unwrap();
    assert!(
        engine
            .analyze_prepared(&audio)
            .unwrap_err()
            .to_string()
            .contains("complete")
    );
    std::fs::write(&path, b"changed").unwrap();
    let mut engine = Engine::new(&EngineSettings::default()).unwrap();
    assert!(
        engine
            .analyze_prepared(&audio)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    let bad = d.path().join("bad-model.json");
    std::fs::write(&bad, b"{}").unwrap();
    assert!(
        Engine::new(&EngineSettings {
            energy_model: Some(bad),
            ..Default::default()
        })
        .is_err()
    );
}

#[test]
fn reported_duration_is_the_playable_clock_even_for_fractional_dsp_frames() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("odd-length.wav");
    let mut wav = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .unwrap();
    for _ in 0..44101 {
        wav.write_sample(0.1_f32).unwrap();
        wav.write_sample(0.1_f32).unwrap();
    }
    wav.finalize().unwrap();
    let audio = runtime().decode(&path, d.path()).unwrap();
    let mut engine = Engine::new(&EngineSettings::default()).unwrap();
    assert_eq!(
        engine
            .analyze_prepared(&audio)
            .unwrap()
            .analysis
            .duration_seconds,
        audio.clock.duration_seconds()
    );
}
