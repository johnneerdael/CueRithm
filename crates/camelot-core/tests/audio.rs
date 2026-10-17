use camelot_core::audio::{decode, decode_at_rates};

fn write_wav(path: &std::path::Path, rate: u32, channels: u16, frames: usize) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        let value =
            (0.5 * (std::f64::consts::TAU * 440.0 * i as f64 / rate as f64).sin() * 32767.0) as i16;
        for _ in 0..channels {
            writer.write_sample(value).unwrap();
        }
    }
    writer.finalize().unwrap();
}

#[test]
fn downmix_and_resample_preserve_duration_and_amplitude() {
    let temp = tempfile::tempdir().unwrap();
    for source_rate in [22050, 44100, 48000] {
        let path = temp.path().join(format!("{source_rate}.wav"));
        write_wav(&path, source_rate, 2, source_rate as usize * 2);
        let audio = decode(&path, 11025, None).unwrap();
        assert_eq!(audio.sample_rate, 11025);
        assert_eq!(audio.samples.len(), 22050);
        assert!(audio.complete);
        let rms = (audio
            .samples
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            / audio.samples.len() as f64)
            .sqrt();
        assert!(
            (rms - 0.5 / 2.0_f64.sqrt()).abs() < 0.01,
            "rate={source_rate} rms={rms}"
        );
    }
}

#[test]
fn explicit_analysis_limit_is_reported_as_partial() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("long.wav");
    write_wav(&path, 44100, 1, 44100 * 3);
    let audio = decode(&path, 11025, Some(1.0)).unwrap();
    assert_eq!(audio.samples.len(), 11025);
    assert!(!audio.complete);
}

#[test]
fn invalid_audio_and_invalid_limits_fail() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("corrupt.wav");
    std::fs::write(&path, b"RIFFbroken").unwrap();
    assert!(decode(&path, 11025, None).is_err());
    assert!(decode(&path, 0, None).is_err());
    assert!(decode(&path, 11025, Some(f64::NAN)).is_err());
    assert!(decode(&path, 11025, Some(-1.0)).is_err());
}

#[test]
fn exact_resampler_chunk_boundary_flushes_without_an_empty_channel_error() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("exact-chunks.wav");
    write_wav(&path, 44100, 1, 2048 * 32);
    let audio = decode(&path, 11025, None).unwrap();
    assert_eq!(audio.samples.len(), 2048 * 8);
    assert!(audio.complete);
}

#[test]
fn multiple_rates_match_independent_direct_resampling_including_partial_audio() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("two-rates.wav");
    write_wav(&path, 48000, 2, 48000 * 2 + 17);
    for limit in [None, Some(0.713)] {
        let outputs = decode_at_rates(&path, &[11025, 22050], limit).unwrap();
        assert_eq!(outputs.len(), 2);
        for (audio, rate) in outputs.iter().zip([11025, 22050]) {
            let separate = decode(&path, rate, limit).unwrap();
            assert_eq!(audio.sample_rate, rate);
            assert_eq!(audio.samples, separate.samples);
            assert_eq!(audio.complete, separate.complete);
            assert_eq!(audio.verified_integrity, separate.verified_integrity);
        }
        assert!(
            (outputs[0].duration_seconds() - outputs[1].duration_seconds()).abs() < 1.0 / 11025.0
        );
    }
    assert!(decode_at_rates(&path, &[], None).is_err());
    assert!(decode_at_rates(&path, &[11025, 11025], None).is_err());
    assert!(decode_at_rates(&path, &[11025, 0], None).is_err());
}

#[test]
fn neural_rate_retains_high_frequencies_without_changing_the_key_rate_output() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("high-band.wav");
    let mut writer = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 1,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for i in 0..44100 * 2 {
        writer
            .write_sample(
                (0.5 * (std::f64::consts::TAU * 7000.0 * i as f64 / 44100.0).sin() * 32767.0)
                    as i16,
            )
            .unwrap();
    }
    writer.finalize().unwrap();
    let outputs = decode_at_rates(&path, &[11025, 22050], None).unwrap();
    let rms = |samples: &[f32]| {
        (samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
    };
    assert!(rms(&outputs[0].samples[1024..outputs[0].samples.len() - 1024]) < 0.001);
    assert!(
        (rms(&outputs[1].samples[1024..outputs[1].samples.len() - 1024]) - 0.5 / 2.0_f64.sqrt())
            .abs()
            < 0.005
    );
}

#[test]
fn large_rational_resampler_blocks_flush_valid_audio_without_a_false_stall() {
    let temp = tempfile::tempdir().unwrap();
    for source_rate in [384000, 47999] {
        let path = temp.path().join(format!("high-{source_rate}.wav"));
        write_wav(&path, source_rate, 1, source_rate as usize);
        let audio = decode(&path, 11025, None).unwrap();
        assert_eq!(audio.samples.len(), 11025);
        assert!(audio.complete);
    }
}
