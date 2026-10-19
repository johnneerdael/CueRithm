use camelot_core::lexicon_energy::analyze;
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[derive(Deserialize)]
struct Reference {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    file: String,
    sha256: String,
    expected: Features,
    policy_level: Option<u8>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Features {
    crest_db: f64,
    flatness: f64,
    high_mid_ratio: f64,
    sub_ratio: f64,
}
#[test]
fn original_local_helper_matches_synthetic_pcm_and_explicit_silence_policy() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lexicon-energy");
    let reference: Reference =
        serde_json::from_slice(&std::fs::read(root.join("reference.json")).unwrap()).unwrap();
    for case in reference.cases {
        let bytes = std::fs::read(root.join(case.file)).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            case.sha256,
            "{} fixture integrity",
            case.name
        );
        let (samples, remainder) = bytes.as_chunks::<4>();
        assert!(remainder.is_empty(), "{} fixture alignment", case.name);
        let pcm: Vec<f32> = samples.iter().map(|b| f32::from_le_bytes(*b)).collect();
        let result = analyze(&pcm, 22050).unwrap();
        assert_eq!(
            result.as_ref().map(|r| r.level),
            case.policy_level,
            "{}",
            case.name
        );
        if let Some(result) = result {
            for (name, actual, expected) in [
                ("crest", result.crest_db, case.expected.crest_db),
                ("flatness", result.flatness, case.expected.flatness),
                (
                    "high-mid",
                    result.high_mid_ratio,
                    case.expected.high_mid_ratio,
                ),
                ("sub", result.sub_ratio, case.expected.sub_ratio),
            ] {
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "{} {name}: {actual} != {expected}",
                    case.name
                );
            }
        }
    }
}
#[test]
fn invalid_pcm_and_unresampled_input_are_explicit_errors() {
    assert!(analyze(&[f32::NAN; 2048], 22050).is_err());
    assert!(analyze(&[f32::INFINITY; 2048], 22050).is_err());
    assert!(analyze(&[0.; 2048], 44100).is_err());
}

#[test]
fn engine_uses_whole_pcm_without_an_energy_model_or_tags() {
    use camelot_core::{Engine, EngineSettings};
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("untrusted-energy-1.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 22050,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&file, spec).unwrap();
    let pcm: Vec<f32> = (0..22050)
        .map(|i| (2. * std::f64::consts::PI * 440. * i as f64 / 22050.).sin() as f32 * 0.5)
        .collect();
    for sample in &pcm {
        writer.write_sample(*sample).unwrap();
    }
    writer.finalize().unwrap();
    let settings = EngineSettings {
        lexicon_energy: true,
        ..Default::default()
    };
    let mut engine = Engine::new(&settings).unwrap();
    let result = engine.analyze_file(&file).unwrap();
    let energy = result.analysis.energy.unwrap();
    assert_eq!(energy.method, camelot_core::lexicon_energy::METHOD);
    assert_eq!(energy.level, analyze(&pcm, 22050).unwrap().unwrap().level);
    assert!(!result.analysis.models.contains_key("track-energy-svc"));
    assert!(result.analysis.models.contains_key("track-energy-dsp"));
    assert!(
        Engine::new(&EngineSettings {
            lexicon_energy: true,
            max_seconds: Some(1.),
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        Engine::new(&EngineSettings {
            lexicon_energy: true,
            energy_model: Some("unused-model.json".into()),
            ..Default::default()
        })
        .is_err()
    );
}
