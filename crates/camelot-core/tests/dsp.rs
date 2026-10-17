use camelot_core::{AnalysisConfig, analyze_pcm};
use std::f64::consts::TAU;

fn progression(tonic: i32, minor: bool, cents: f64, rate: u32) -> Vec<f32> {
    let chords: [[i32; 3]; 4] = if minor {
        [[0, 3, 7], [5, 8, 12], [7, 11, 14], [0, 3, 7]]
    } else {
        [[0, 4, 7], [5, 9, 12], [7, 11, 14], [0, 4, 7]]
    };
    let mut samples = Vec::new();
    for chord in chords.iter().cycle().take(8) {
        for frame in 0..rate as usize {
            let time = frame as f64 / rate as f64;
            let envelope = (time / 0.02).min(1.0) * ((1.0 - time) / 0.03).min(1.0);
            let mut value = 0.0;
            for offset in chord {
                let midi = 48 + tonic + offset;
                let frequency = 440.0 * 2.0_f64.powf((midi as f64 - 69.0 + cents / 100.0) / 12.0);
                value +=
                    (TAU * frequency * time).sin() + 0.18 * (TAU * frequency * 2.0 * time).sin();
            }
            samples.push((value * 0.15 * envelope) as f32);
        }
    }
    samples
}

fn clicks(bpm: f64, rate: u32) -> Vec<f32> {
    let mut samples = vec![0.0; rate as usize * 24];
    let period = 60.0 / bpm;
    for beat in 0..(24.0 / period) as usize {
        let start = ((beat as f64 * period + 0.25) * rate as f64).round() as usize;
        for i in 0..(rate as f64 * 0.03) as usize {
            if let Some(value) = samples.get_mut(start + i) {
                *value = ((-(i as f64) / (0.006 * rate as f64)).exp()
                    * (TAU * 900.0 * i as f64 / rate as f64).cos()) as f32;
            }
        }
    }
    samples
}

#[test]
fn silence_returns_unknown_key_and_tempo() {
    let result = analyze_pcm(&vec![0.0; 11025 * 4], 11025, &AnalysisConfig::default()).unwrap();
    assert!(result.key.is_none());
    assert!(result.tempo.is_none());
}

#[test]
fn cadences_transpose_across_all_tonics_and_modes() {
    for tonic in 0..12 {
        for minor in [false, true] {
            let samples = progression(tonic, minor, 0.0, 11025);
            let result = analyze_pcm(&samples, 11025, &AnalysisConfig::default()).unwrap();
            let expected = camelot_core::MusicalKey::new(
                tonic as u8,
                if minor {
                    camelot_core::Mode::Minor
                } else {
                    camelot_core::Mode::Major
                },
            )
            .unwrap();
            assert_eq!(
                result.key.unwrap().key,
                expected,
                "tonic={tonic} minor={minor}"
            );
        }
    }
}

#[test]
fn modest_detuning_does_not_change_the_detected_key() {
    let result = analyze_pcm(
        &progression(0, false, 23.0, 11025),
        11025,
        &AnalysisConfig::default(),
    )
    .unwrap();
    assert_eq!(result.key.unwrap().key.camelot(), "8B");
}

#[test]
fn click_tempi_are_estimated_in_bpm_across_sample_rates() {
    for (bpm, rate) in [
        (90.0, 11025),
        (120.0, 11025),
        (128.0, 22050),
        (143.0, 11025),
    ] {
        let result = analyze_pcm(&clicks(bpm, rate), rate, &AnalysisConfig::default()).unwrap();
        let estimate = result.tempo.unwrap();
        assert!(
            (estimate.bpm - bpm).abs() < 0.6,
            "expected {bpm}, got {}",
            estimate.bpm
        );
    }
}

#[test]
fn invalid_pcm_and_configuration_are_rejected() {
    assert!(analyze_pcm(&[f32::NAN; 4096], 11025, &AnalysisConfig::default()).is_err());
    assert!(analyze_pcm(&[0.0; 4096], 0, &AnalysisConfig::default()).is_err());
    let config = AnalysisConfig {
        min_bpm: 150.0,
        max_bpm: 100.0,
        ..Default::default()
    };
    assert!(analyze_pcm(&[0.0; 4096], 11025, &config).is_err());
}
