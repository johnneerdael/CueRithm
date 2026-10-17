use camelot_core::structure::extract;

fn tone(rate: u32, seconds: f64, frequency: f64, amplitude: f64) -> Vec<f32> {
    (0..(rate as f64 * seconds) as usize)
        .map(|i| {
            (amplitude * (std::f64::consts::TAU * frequency * i as f64 / rate as f64).sin()) as f32
        })
        .collect()
}

#[test]
fn silence_and_short_clips_have_finite_features_and_valid_times() {
    for samples in [vec![], vec![0.0; 10], vec![0.0; 11025]] {
        let features = extract(&samples, 11025).unwrap();
        for frame in &features.frames {
            assert!(frame.time_seconds >= 0.0 && frame.time_seconds < features.duration_seconds);
            assert_eq!(frame.rms_db, -120.0);
            assert_eq!(frame.band_energy_fraction, [0.0; 6]);
            assert_eq!(frame.spectral_flux, 0.0);
            assert_eq!(frame.centroid_hz, 0.0);
        }
        assert_eq!(features.frames.is_empty(), samples.is_empty());
        assert!(serde_json::to_string(&features).is_ok());
    }
    assert!(extract(&[f32::NAN], 11025).is_err());
    assert!(extract(&[0.0], 0).is_err());
}

#[test]
fn loudness_and_band_features_distinguish_quiet_bass_from_loud_high_tones() {
    for rate in [11025, 22050, 44100] {
        let mut samples = tone(rate, 2.0, 80.0, 0.1);
        samples.extend(tone(rate, 2.0, 1600.0, 0.5));
        let features = extract(&samples, rate).unwrap();
        let bass = features
            .frames
            .iter()
            .find(|f| f.time_seconds >= 1.0)
            .unwrap();
        let treble = features
            .frames
            .iter()
            .find(|f| f.time_seconds >= 3.0)
            .unwrap();
        assert!(bass.band_energy_fraction[0] > 0.98);
        assert!(treble.band_energy_fraction[4] > 0.98);
        assert!((bass.centroid_hz - 80.0).abs() < 3.0);
        assert!((treble.centroid_hz - 1600.0).abs() < 3.0);
        assert!((treble.rms_db - bass.rms_db - 20.0 * 5.0_f64.log10()).abs() < 0.1);
        assert!(
            features
                .frames
                .iter()
                .filter(|f| (f.time_seconds - 2.0).abs() < 0.2)
                .any(|f| f.spectral_flux > 0.5)
        );
        assert!(
            features
                .frames
                .windows(2)
                .all(|w| w[1].time_seconds > w[0].time_seconds)
        );
    }
}

#[test]
fn gain_changes_do_not_change_normalized_spectral_features() {
    let loud = tone(11025, 2.0, 440.0, 0.8);
    let quiet: Vec<f32> = loud.iter().map(|x| x * 0.1).collect();
    let a = extract(&loud, 11025).unwrap();
    let b = extract(&quiet, 11025).unwrap();
    for (loud, quiet) in a.frames.iter().zip(&b.frames) {
        assert!((loud.rms_db - quiet.rms_db - 20.0).abs() < 0.001);
        for (x, y) in loud
            .band_energy_fraction
            .iter()
            .zip(quiet.band_energy_fraction)
        {
            assert!((x - y).abs() < 1e-6);
        }
        assert!((loud.centroid_hz - quiet.centroid_hz).abs() < 0.01);
    }
}

#[test]
fn large_finite_samples_do_not_overflow_the_feature_fft() {
    let features = extract(&vec![f32::MAX; 11025], 11025).unwrap();
    for frame in features.frames {
        assert!(frame.rms_db.is_finite());
        assert!(frame.centroid_hz.is_finite());
        assert!(frame.spectral_flux.is_finite());
        assert!(frame.band_energy_fraction.iter().all(|x| x.is_finite()));
        assert!((frame.band_energy_fraction.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }
}
