use camelot_core::lexicon_timing::{LexiconTempoOptions, analyze_tempo};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
struct Reference {
    source: Source,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Source {
    sample_rate: u32,
    min_tempo: f64,
    max_tempo: f64,
}
#[derive(Deserialize)]
struct Recipe {
    frames: usize,
    quiet_until_frames: usize,
    offbeat_hats: bool,
    bpm: f64,
    offset_frames: usize,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    recipe: Recipe,
    pcm_sha256: String,
    expected: Expected,
}
#[derive(Deserialize)]
struct Expected {
    tempo: ExpectedTempo,
}
#[derive(Deserialize)]
struct ExpectedTempo {
    tempo: f64,
    confidence: f64,
    peaks: Vec<usize>,
}
fn signal(recipe: &Recipe, rate: u32) -> Vec<f32> {
    let mut pcm = vec![0.; recipe.frames];
    for beat in 0.. {
        let start = (recipe.offset_frames as f64 + beat as f64 * 60. * rate as f64 / recipe.bpm)
            .round() as usize;
        if start >= pcm.len() {
            break;
        }
        let gain = if start < recipe.quiet_until_frames {
            1. / 16.
        } else {
            1.
        };
        for index in 0..4096.min(pcm.len() - start) {
            let sign = if index % 512 < 256 { 1. } else { -1. };
            pcm[start + index] = ((4096 - index) as f64 / 4096. * sign * gain) as f32;
        }
        if recipe.offbeat_hats {
            let hat = start + (30. * rate as f64 / recipe.bpm).round() as usize;
            for index in 0..256.min(pcm.len().saturating_sub(hat)) {
                let sign = if index % 2 == 1 { 1. } else { -1. };
                pcm[hat + index] =
                    (pcm[hat + index] as f64 + (256 - index) as f64 / 2048. * sign) as f32;
            }
        }
    }
    pcm
}
#[test]
fn original_tempo_helper_references_match_exact_peaks_bpm_and_confidence() {
    let reference: Reference = serde_json::from_str(include_str!(
        "../../../tests/fixtures/lexicon-timing/reference.json"
    ))
    .unwrap();
    let options = LexiconTempoOptions {
        min_tempo: reference.source.min_tempo,
        max_tempo: reference.source.max_tempo,
    };
    let mut consumed = 0;
    for case in reference.cases {
        let pcm = signal(&case.recipe, reference.source.sample_rate);
        let mut hash = Sha256::new();
        for sample in &pcm {
            hash.update(sample.to_le_bytes());
        }
        assert_eq!(
            format!("{:x}", hash.finalize()),
            case.pcm_sha256,
            "{} PCM",
            case.name
        );
        let actual = analyze_tempo(&pcm, reference.source.sample_rate, &options)
            .unwrap()
            .unwrap();
        assert_eq!(actual.bpm, case.expected.tempo.tempo, "{} BPM", case.name);
        assert_eq!(
            actual.peaks, case.expected.tempo.peaks,
            "{} peaks",
            case.name
        );
        assert!(
            (actual.confidence - case.expected.tempo.confidence).abs() <= 1e-12,
            "{} confidence: {} vs {}",
            case.name,
            actual.confidence,
            case.expected.tempo.confidence
        );
        consumed += 1;
    }
    assert_eq!(consumed, 8);
}
#[test]
fn unavailable_rhythm_returns_unknown_promptly_instead_of_hanging_or_fabricating_120_bpm() {
    let options = LexiconTempoOptions::default();
    let started = std::time::Instant::now();
    for pcm in [
        vec![],
        vec![0.5; 16],
        vec![0.; 44100 * 12],
        vec![f32::from_bits(1); 8000],
    ] {
        assert!(analyze_tempo(&pcm, 44100, &options).unwrap().is_none());
    }
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    // Original weak-burst diagnostic: no qualifying peaks. Nonzero input does
    // not justify the source helper's fallback 120 BPM.
    let weak = vec![0.00001; 44100];
    assert!(analyze_tempo(&weak, 44100, &options).unwrap().is_none());
}
#[test]
fn malformed_pcm_rate_or_tempo_bounds_are_explicit_errors() {
    let options = LexiconTempoOptions::default();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(analyze_tempo(&[value], 44100, &options).is_err());
    }
    for rate in [0, 7999, 192001, u32::MAX] {
        assert!(analyze_tempo(&[], rate, &options).is_err());
    }
    for (min_tempo, max_tempo) in [
        (0., 195.),
        (49.9, 195.),
        (98., 300.1),
        (195., 195.),
        (196., 195.),
        (f64::NAN, 195.),
        (98., f64::INFINITY),
    ] {
        assert!(
            analyze_tempo(
                &[],
                44100,
                &LexiconTempoOptions {
                    min_tempo,
                    max_tempo
                }
            )
            .is_err()
        );
    }
}
