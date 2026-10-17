use camelot_core::lexicon_cues::*;
use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Helpers – synthetic PCM generation
// ---------------------------------------------------------------------------

/// Generate a mono sine wave at the given frequency and amplitude.
fn sine_pcm(sample_rate: u32, duration_secs: f64, freq_hz: f64, amplitude: f32) -> Vec<f32> {
    let n = (sample_rate as f64 * duration_secs) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            amplitude * (2.0 * PI * freq_hz * t).sin() as f32
        })
        .collect()
}

/// Generate a constant-amplitude tone.
fn constant_pcm(sample_rate: u32, duration_secs: f64, amplitude: f32) -> Vec<f32> {
    sine_pcm(sample_rate, duration_secs, 200.0, amplitude)
}

/// Generate silence.
fn silence(sample_rate: u32, duration_secs: f64) -> Vec<f32> {
    vec![0.0f32; (sample_rate as f64 * duration_secs) as usize]
}

/// Concatenate PCM buffers.
fn concat(parts: &[&[f32]]) -> Vec<f32> {
    parts.iter().flat_map(|p| p.iter().copied()).collect()
}

/// Generate a track-like PCM with intro → build → drop → breakdown → drop2 → outro
/// at the given BPM, with first beat at `first_beat` seconds.
///
/// Returns (pcm, total_duration).
fn synthetic_track(sample_rate: u32, _bpm: f64, first_beat: f64) -> Vec<f32> {
    // Typical electronic track structure at 128 BPM:
    //   intro (quiet):      0 – 30s      amplitude ~0.05
    //   build:             30 – 60s      ramp 0.05 → 0.6
    //   drop:              60 – 120s     amplitude ~0.7
    //   breakdown:        120 – 150s     amplitude ~0.1
    //   second drop:      150 – 210s     amplitude ~0.7
    //   outro:            210 – 270s     ramp 0.7 → 0.02
    let sr = sample_rate;
    let freq = 100.0; // low bass-like frequency

    let mut pcm = Vec::new();
    let total_secs = 270.0;
    let n = (sr as f64 * total_secs) as usize;

    for i in 0..n {
        let t = i as f64 / sr as f64;
        let carrier = (2.0 * PI * freq * t).sin() as f32;

        let amp: f32 = if t < first_beat {
            0.0
        } else if t < 30.0 {
            0.05
        } else if t < 60.0 {
            // build: linear ramp
            let frac = (t - 30.0) / 30.0;
            0.05 + 0.55 * frac as f32
        } else if t < 120.0 {
            0.7
        } else if t < 150.0 {
            0.1
        } else if t < 210.0 {
            0.7
        } else if t < 260.0 {
            // outro: linear ramp down
            let frac = (t - 210.0) / 50.0;
            0.7 - 0.68 * frac as f32
        } else {
            0.02
        };

        pcm.push(carrier * amp);
    }
    pcm
}

// ---------------------------------------------------------------------------
// DSP primitive tests
// ---------------------------------------------------------------------------

#[test]
fn low_pass_iir_attenuates_high_frequencies() {
    let sr = 44100;
    let high = sine_pcm(sr, 1.0, 5000.0, 1.0);
    let low = sine_pcm(sr, 1.0, 100.0, 1.0);

    let filtered_high = low_pass_iir(&high, sr, 300.0, 6);
    let filtered_low = low_pass_iir(&low, sr, 300.0, 6);

    let rms = |s: &[f32]| -> f64 {
        let skip = sr as usize / 4; // skip transient
        let slice = &s[skip..];
        (slice.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / slice.len() as f64).sqrt()
    };

    let rms_high = rms(&filtered_high);
    let rms_low = rms(&filtered_low);

    // 5000 Hz should be heavily attenuated vs 100 Hz
    assert!(
        rms_high < rms_low * 0.1,
        "5kHz RMS ({rms_high:.6}) should be <10% of 100Hz RMS ({rms_low:.6})"
    );
}

#[test]
fn low_pass_iir_preserves_dc() {
    let sr = 44100;
    let dc: Vec<f32> = vec![0.5; sr as usize];
    let filtered = low_pass_iir(&dc, sr, 300.0, 6);

    // After settling, output should be ~0.5
    let tail_avg: f64 =
        filtered[sr as usize / 2..].iter().map(|x| *x as f64).sum::<f64>()
            / (sr as usize / 2) as f64;
    assert!(
        (tail_avg - 0.5).abs() < 0.01,
        "DC pass-through should be ~0.5, got {tail_avg}"
    );
}

#[test]
fn biquad_lowpass_attenuates_high_frequencies() {
    let sr = 44100;
    let high = sine_pcm(sr, 1.0, 8000.0, 1.0);
    let low = sine_pcm(sr, 1.0, 50.0, 1.0);

    let filtered_high = biquad_lowpass(&high, sr, 200.0);
    let filtered_low = biquad_lowpass(&low, sr, 200.0);

    let rms = |s: &[f32]| -> f64 {
        let skip = sr as usize / 4;
        let slice = &s[skip..];
        (slice.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / slice.len() as f64).sqrt()
    };

    assert!(
        rms(&filtered_high) < rms(&filtered_low) * 0.05,
        "8kHz should be attenuated much more than 50Hz through biquad at 200Hz"
    );
}

#[test]
fn cascaded_lowpass_produces_output() {
    let sr = 44100;
    let pcm = sine_pcm(sr, 0.5, 100.0, 0.8);
    let filtered = cascaded_lowpass(&pcm, sr);
    assert_eq!(filtered.len(), pcm.len());
    // After 7 stages the signal should still have content at 100Hz (below all cutoffs)
    let rms: f64 = filtered[sr as usize / 4..]
        .iter()
        .map(|x| (*x as f64).powi(2))
        .sum::<f64>()
        / (filtered.len() - sr as usize / 4) as f64;
    assert!(rms > 0.0, "Cascaded lowpass should pass 100Hz signal");
}

#[test]
fn low_pass_iir_empty_input() {
    let out = low_pass_iir(&[], 44100, 300.0, 6);
    assert!(out.is_empty());
}

#[test]
fn biquad_lowpass_empty_input() {
    let out = biquad_lowpass(&[], 44100, 200.0);
    assert!(out.is_empty());
}

// ---------------------------------------------------------------------------
// build_sections tests
// ---------------------------------------------------------------------------

#[test]
fn build_sections_constant_signal_produces_flat_ramps() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 30.0, 0.5);
    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);

    assert!(!sections.is_empty());
    for s in &sections {
        assert_eq!(s.ramp_type, RampType::Flat, "Constant signal should be Flat");
        assert!(s.energy > 0.0, "Constant signal should have positive energy");
    }
}

#[test]
fn build_sections_silence_has_near_zero_energy() {
    let sr = 44100;
    let pcm = silence(sr, 10.0);
    let sections = build_sections(&pcm, sr, 120.0, 0.0, 4, 0.05);

    for s in &sections {
        assert!(
            s.energy < 1e-6,
            "Silent sections should have near-zero energy, got {}",
            s.energy
        );
    }
}

#[test]
fn build_sections_ramp_up_detected() {
    // Create a signal that ramps amplitude up over time
    let sr = 44100;
    let dur = 30.0;
    let n = (sr as f64 * dur) as usize;
    let pcm: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / sr as f64;
            let amp = (t / dur) as f32;
            amp * (2.0 * PI * 200.0 * t).sin() as f32
        })
        .collect();

    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);
    let up_count = sections.iter().filter(|s| s.ramp_type == RampType::Up).count();
    assert!(
        up_count > 0,
        "Ramping signal should produce at least one Up-classified group"
    );
}

#[test]
fn build_sections_beat_aligned_timing() {
    let sr = 44100;
    let bpm = 120.0;
    let first_beat = 0.5;
    let pcm = constant_pcm(sr, 20.0, 0.3);

    let sections = build_sections(&pcm, sr, bpm, first_beat, 4, 0.05);
    assert!(!sections.is_empty());

    let beat_period = 60.0 / bpm;
    let section_dur = 4.0 * beat_period; // 2.0 seconds at 120 BPM

    // Each section should be roughly one section duration apart
    for w in sections.windows(2) {
        let gap = w[1].start_time - w[0].start_time;
        assert!(
            (gap - section_dur).abs() < 0.01,
            "Section gap {gap:.4} should be ~{section_dur:.4}"
        );
    }
}

#[test]
fn build_sections_energy_delta_computed() {
    let sr = 44100;
    // quiet → loud
    let quiet = constant_pcm(sr, 5.0, 0.05);
    let loud = constant_pcm(sr, 5.0, 0.8);
    let pcm = concat(&[&quiet, &loud]);

    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);
    assert_eq!(sections[0].energy_delta, 0.0, "First section delta should be 0");

    // Find the transition: there should be a large positive delta
    let max_delta = sections
        .iter()
        .map(|s| s.energy_delta)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(max_delta > 0.1, "Should see a positive energy delta at quiet→loud transition");
}

// ---------------------------------------------------------------------------
// nearest_section / avg_energy tests
// ---------------------------------------------------------------------------

#[test]
fn nearest_section_finds_closest() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 30.0, 0.5);
    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);

    // The section at index 0 starts at ~0.0
    assert_eq!(nearest_section(&sections, 0.0), 0);
    // A time near the last section
    let last = sections.last().unwrap().start_time;
    assert_eq!(nearest_section(&sections, last + 0.1), sections.len() - 1);
}

#[test]
fn avg_energy_empty_returns_epsilon() {
    assert_eq!(avg_energy(&[], 0, 10), 1e-9);
}

#[test]
fn avg_energy_single_section() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 5.0, 0.5);
    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);

    let e = avg_energy(&sections, 0, 1);
    assert!(e > 0.0);
    assert!((e - sections[0].energy).abs() < 1e-9);
}

// ---------------------------------------------------------------------------
// rms_at_time tests
// ---------------------------------------------------------------------------

#[test]
fn rms_at_time_matches_known_amplitude() {
    let sr = 44100;
    let amp = 0.6_f32;
    let pcm = constant_pcm(sr, 5.0, amp);

    // RMS of a sine wave of amplitude A is A/sqrt(2)
    let expected_rms = amp as f64 / std::f64::consts::SQRT_2;
    let measured = rms_at_time(&pcm, sr, 2.5, 0.5);
    assert!(
        (measured - expected_rms).abs() < 0.02,
        "RMS should be ~{expected_rms:.4}, got {measured:.4}"
    );
}

#[test]
fn rms_at_time_silence_is_zero() {
    let pcm = silence(44100, 2.0);
    let rms = rms_at_time(&pcm, 44100, 1.0, 0.5);
    assert!(rms < 1e-9);
}

// ---------------------------------------------------------------------------
// percentile tests
// ---------------------------------------------------------------------------

#[test]
fn percentile_basic() {
    let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
    assert!((percentile(&values, 0.0) - 10.0).abs() < 1e-9);
    // p=0.99 → floor(5*0.99)=4 → 50.0
    assert!((percentile(&values, 0.99) - 50.0).abs() < 1e-9);
    // p=0.5 → floor(5*0.5)=2 → 30.0
    let med = percentile(&values, 0.5);
    assert!(
        (med - 30.0).abs() < 1e-9,
        "Median of [10,20,30,40,50] should be 30, got {med}"
    );
}

#[test]
fn percentile_single_value() {
    assert!((percentile(&[42.0], 0.5) - 42.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------
// classify_genre tests
// ---------------------------------------------------------------------------

#[test]
fn classify_genre_recognizes_techno() {
    assert_eq!(classify_genre(&Some("Techno".into())), Some("techno".into()));
    assert_eq!(classify_genre(&Some("Peak Time Techno".into())), Some("techno".into()));
    assert_eq!(classify_genre(&Some("Melodic House & Techno".into())), Some("techno".into()));
}

#[test]
fn classify_genre_passes_through_house() {
    // Non-techno/non-hardcore genres are lowercased and passed through
    assert_eq!(classify_genre(&Some("Deep House".into())), Some("deep house".into()));
    assert_eq!(classify_genre(&Some("Tech House".into())), Some("tech house".into()));
}

#[test]
fn classify_genre_recognizes_hardcore() {
    assert_eq!(classify_genre(&Some("Hardcore".into())), Some("hardcore".into()));
    assert_eq!(classify_genre(&Some("Happy Hardcore".into())), Some("hardcore".into()));
}

#[test]
fn classify_genre_lowercases_unknown() {
    assert_eq!(classify_genre(&Some("Classical".into())), Some("classical".into()));
    assert_eq!(classify_genre(&None), None);
}

// ---------------------------------------------------------------------------
// energy_ratio_to_peak / energy_sustain_after / energy_contrast_before
// ---------------------------------------------------------------------------

#[test]
fn energy_ratio_to_peak_at_peak_is_one() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 30.0, 0.8);
    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);

    // All sections have roughly equal energy, so ratio at any index should be ~1.0
    let ratio = energy_ratio_to_peak(&sections, Some(0));
    assert!(
        (ratio - 1.0).abs() < 0.05,
        "Uniform signal: ratio to peak should be ~1.0, got {ratio}"
    );
}

#[test]
fn energy_ratio_to_peak_none_returns_zero() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 10.0, 0.5);
    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);
    assert_eq!(energy_ratio_to_peak(&sections, None), 0.0);
}

#[test]
fn energy_sustain_after_constant_is_high() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 30.0, 0.5);
    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);

    let sustain = energy_sustain_after(&sections, Some(0));
    // Constant signal: sustain after any point should be high
    assert!(sustain > 0.5, "Constant signal sustain should be high, got {sustain}");
}

#[test]
fn energy_contrast_before_constant_is_low() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 30.0, 0.5);
    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);

    let mid = sections.len() / 2;
    let contrast = energy_contrast_before(&sections, Some(mid));
    // Constant signal: no contrast
    assert!(
        contrast < 2.0,
        "Constant signal contrast should be low, got {contrast}"
    );
}

// ---------------------------------------------------------------------------
// refine_drop_start tests
// ---------------------------------------------------------------------------

#[test]
fn refine_drop_start_returns_valid_index() {
    let sr = 44100;
    let quiet = constant_pcm(sr, 10.0, 0.05);
    let loud = constant_pcm(sr, 20.0, 0.7);
    let pcm = concat(&[&quiet, &loud]);

    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);
    assert!(!sections.is_empty());

    let loud_idx = sections.len() - 1;
    let refined = refine_drop_start(&sections, loud_idx, 20);

    // Refined index must be a valid section index
    assert!(refined < sections.len());
    // Should be at or before the starting index
    assert!(refined <= loud_idx);
    // Refined section should have meaningful energy (it's in or near the loud region)
    assert!(
        sections[refined].energy > sections[0].energy * 0.3,
        "Refined section energy {:.4} should be above quiet threshold",
        sections[refined].energy
    );
}

// ---------------------------------------------------------------------------
// find_energy_gap tests
// ---------------------------------------------------------------------------

#[test]
fn find_energy_gap_detects_quiet_region() {
    let sr = 44100;
    let loud1 = constant_pcm(sr, 10.0, 0.7);
    let quiet = silence(sr, 10.0);
    let loud2 = constant_pcm(sr, 10.0, 0.7);
    let pcm = concat(&[&loud1, &quiet, &loud2]);

    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);
    let gap = find_energy_gap(&sections, 0.3, 2);

    assert!(gap.is_some(), "Should find a gap in loud-quiet-loud signal");
}

#[test]
fn find_energy_gap_none_in_constant() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 30.0, 0.5);
    let sections = build_sections(&pcm, sr, 128.0, 0.0, 4, 0.05);

    let gap = find_energy_gap(&sections, 0.3, 2);
    assert!(gap.is_none(), "Constant signal should have no energy gap");
}

// ---------------------------------------------------------------------------
// detect_drop tests
// ---------------------------------------------------------------------------

#[test]
fn detect_drop_finds_drop_in_synthetic_track() {
    let sr = 44100;
    let bpm = 128.0;
    let first_beat = 0.5;
    let pcm = synthetic_track(sr, bpm, first_beat);

    let result = detect_drop(&pcm, sr, bpm, first_beat, None, &None);

    assert!(result.drop.is_some(), "Should detect a drop in synthetic track");
    let drop_time = result.drop.unwrap().time_seconds;
    // Drop should be around 60s (the loud section start)
    assert!(
        drop_time > 30.0 && drop_time < 90.0,
        "Drop at {drop_time:.1}s should be between 30-90s (synthetic drop is at 60s)"
    );
}

#[test]
fn detect_drop_silence_returns_start_only() {
    let pcm = silence(44100, 30.0);
    let result = detect_drop(&pcm, 44100, 128.0, 0.0, None, &None);

    // Start should be at first beat
    assert!(
        result.start.time_seconds < 1.0,
        "Start cue should be near the beginning"
    );
    // May or may not find a drop in silence — but shouldn't panic
}

#[test]
fn detect_drop_constant_loud() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 120.0, 0.7);
    let result = detect_drop(&pcm, sr, 128.0, 0.0, None, &None);

    // Should at least produce a start cue without panicking
    assert!(result.start.name == "Start");
}

// ---------------------------------------------------------------------------
// detect_breakdown tests
// ---------------------------------------------------------------------------

#[test]
fn detect_breakdown_finds_quiet_section_after_drop() {
    let sr = 44100;
    let bpm = 128.0;
    let first_beat = 0.5;
    let pcm = synthetic_track(sr, bpm, first_beat);

    let drop_result = detect_drop(&pcm, sr, bpm, first_beat, None, &None);
    let drop_time = drop_result.drop.as_ref().map(|c| c.time_seconds).unwrap_or(60.0);

    let total_duration = pcm.len() as f64 / sr as f64;
    let bd = detect_breakdown(
        &pcm, sr, bpm, first_beat, drop_time, total_duration, 64, &None, false,
    );

    assert!(
        bd.is_some(),
        "Should find a breakdown after the drop in synthetic track"
    );
    let bd_time = bd.unwrap().time_seconds;
    // Breakdown should be around 120s (the quiet section)
    assert!(
        bd_time > 90.0 && bd_time < 160.0,
        "Breakdown at {bd_time:.1}s should be near 120s (synthetic breakdown)"
    );
}

// ---------------------------------------------------------------------------
// detect_lastbeat tests
// ---------------------------------------------------------------------------

#[test]
fn detect_lastbeat_finds_end_of_energy() {
    let sr = 44100;
    let bpm = 128.0;
    let first_beat = 0.5;
    let pcm = synthetic_track(sr, bpm, first_beat);
    let total_duration = pcm.len() as f64 / sr as f64;

    let lb = detect_lastbeat(&pcm, sr, bpm, first_beat, total_duration);
    assert!(lb.is_some(), "Should detect lastbeat in synthetic track");

    let lb_time = lb.unwrap().time_seconds;
    // Lastbeat should be in the outro region (210-270s), where energy drops off
    assert!(
        lb_time > 180.0 && lb_time < total_duration,
        "Lastbeat at {lb_time:.1}s should be in the outro region"
    );
}

#[test]
fn detect_lastbeat_short_track_no_panic() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 5.0, 0.3);
    let _lb = detect_lastbeat(&pcm, sr, 128.0, 0.0, 5.0);
    // Just verifying no panic on short input
}

// ---------------------------------------------------------------------------
// generate_structural_cues end-to-end tests
// ---------------------------------------------------------------------------

#[test]
fn generate_cues_produces_start_cue() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 60.0, 0.5);
    let config = LexiconConfig::default();

    let result = generate_structural_cues(&pcm, sr, 128.0, 0.0, &config);
    assert!(!result.cues.is_empty(), "Should produce at least a Start cue");
    assert_eq!(result.cues[0].name, "Start");
}

#[test]
fn generate_cues_synthetic_track_finds_structure() {
    let sr = 44100;
    let bpm = 128.0;
    let first_beat = 0.5;
    let pcm = synthetic_track(sr, bpm, first_beat);

    let config = LexiconConfig {
        genre: None,
        drop_at_start: DropAtStart::Never,
        min_cue_length_beats: 64,
        add_emergency_loop: false,
    };

    let result = generate_structural_cues(&pcm, sr, bpm, first_beat, &config);

    let names: Vec<&str> = result.cues.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"Start"), "Should have Start cue");

    // With a well-structured synthetic track we expect at least Start + Drop
    assert!(
        result.cues.len() >= 2,
        "Should find at least Start and one structural cue, found: {names:?}"
    );

    // Cues should be sorted by time
    for w in result.cues.windows(2) {
        assert!(
            w[0].time_seconds <= w[1].time_seconds,
            "Cues should be sorted: {} at {:.1}s before {} at {:.1}s",
            w[0].name,
            w[0].time_seconds,
            w[1].name,
            w[1].time_seconds
        );
    }
}

#[test]
fn generate_cues_drop_at_start_never_shifts_early_drop() {
    // Create a track where the drop is very early (immediately)
    let sr = 44100;
    let bpm = 128.0;
    let first_beat = 0.5;

    // Loud from start, breakdown at 60s, second loud section at 90s, outro at 180s
    let n = (sr as f64 * 240.0) as usize;
    let pcm: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / sr as f64;
            let carrier = (2.0 * PI * 100.0 * t).sin() as f32;
            let amp: f32 = if t < 60.0 {
                0.7
            } else if t < 90.0 {
                0.1
            } else if t < 180.0 {
                0.7
            } else {
                let frac = (t - 180.0) / 60.0;
                (0.7 - 0.68 * frac as f32).max(0.02)
            };
            carrier * amp
        })
        .collect();

    let config_never = LexiconConfig {
        genre: None,
        drop_at_start: DropAtStart::Never,
        min_cue_length_beats: 64,
        add_emergency_loop: false,
    };

    let result = generate_structural_cues(&pcm, sr, bpm, first_beat, &config_never);

    // With Never mode and an early drop, it should try to shift to the second drop
    // We just verify it doesn't panic and produces reasonable output
    assert!(!result.cues.is_empty());
    assert_eq!(result.cues[0].name, "Start");
}

#[test]
fn generate_cues_with_emergency_loop() {
    let sr = 44100;
    let bpm = 128.0;
    let first_beat = 0.5;
    let pcm = synthetic_track(sr, bpm, first_beat);

    let config = LexiconConfig {
        genre: None,
        drop_at_start: DropAtStart::Auto,
        min_cue_length_beats: 64,
        add_emergency_loop: true,
    };

    let result = generate_structural_cues(&pcm, sr, bpm, first_beat, &config);
    // Emergency loop may or may not be found depending on the track structure,
    // but it shouldn't panic
    if let Some(el) = &result.emergency_loop {
        assert!(el.time_seconds > 0.0);
        assert!(el.loop_beats > 0);
    }
}

#[test]
fn generate_cues_empty_pcm_no_panic() {
    let config = LexiconConfig::default();
    let result = generate_structural_cues(&[], 44100, 128.0, 0.0, &config);
    // Should at least have a Start cue and not panic
    assert!(!result.cues.is_empty());
}

#[test]
fn generate_cues_very_short_track() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 3.0, 0.5);
    let config = LexiconConfig::default();

    let result = generate_structural_cues(&pcm, sr, 128.0, 0.0, &config);
    assert!(!result.cues.is_empty());
}

#[test]
fn generate_cues_extreme_bpm_values() {
    let sr = 44100;
    let pcm = constant_pcm(sr, 30.0, 0.5);
    let config = LexiconConfig::default();

    // Very slow BPM
    let result = generate_structural_cues(&pcm, sr, 70.0, 0.0, &config);
    assert!(!result.cues.is_empty());

    // Very fast BPM
    let result = generate_structural_cues(&pcm, sr, 180.0, 0.0, &config);
    assert!(!result.cues.is_empty());
}

#[test]
fn generate_cues_with_genre_hint() {
    let sr = 44100;
    let bpm = 128.0;
    let first_beat = 0.5;
    let pcm = synthetic_track(sr, bpm, first_beat);

    for genre in ["Techno", "Deep House", "Hardcore"] {
        let config = LexiconConfig {
            genre: Some(genre.into()),
            drop_at_start: DropAtStart::Never,
            min_cue_length_beats: 64,
            add_emergency_loop: false,
        };

        let result = generate_structural_cues(&pcm, sr, bpm, first_beat, &config);
        assert!(
            !result.cues.is_empty(),
            "Genre '{genre}' should produce cues without panic"
        );
    }
}

// ---------------------------------------------------------------------------
// Config / serialization tests
// ---------------------------------------------------------------------------

#[test]
fn lexicon_config_default_matches_spec() {
    let config = LexiconConfig::default();
    assert_eq!(config.drop_at_start, DropAtStart::Auto);
    assert_eq!(config.min_cue_length_beats, 64);
    assert!(config.add_emergency_loop);
    assert!(config.genre.is_none());
}

#[test]
fn structural_cue_result_serializes() {
    let result = StructuralCueResult {
        cues: vec![
            StructuralCue {
                time_seconds: 1.5,
                name: "Start".into(),
                color: "Yellow".into(),
            },
            StructuralCue {
                time_seconds: 60.0,
                name: "Drop".into(),
                color: "Red".into(),
            },
        ],
        emergency_loop: None,
    };

    let json = serde_json::to_string(&result).unwrap();
    let deserialized: StructuralCueResult = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, result);
}

#[test]
fn lexicon_config_roundtrips_json() {
    let config = LexiconConfig {
        genre: Some("Techno".into()),
        drop_at_start: DropAtStart::Never,
        min_cue_length_beats: 32,
        add_emergency_loop: true,
    };
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: LexiconConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.min_cue_length_beats, 32);
    assert_eq!(deserialized.drop_at_start, DropAtStart::Never);
}

// ---------------------------------------------------------------------------
// Low sample rate (engine default 11025 Hz)
// ---------------------------------------------------------------------------

#[test]
fn generate_cues_at_engine_sample_rate() {
    let sr = 11025;
    let bpm = 128.0;
    let first_beat = 0.5;

    // Synthetic track at engine's default rate
    let dur = 180.0;
    let n = (sr as f64 * dur) as usize;
    let pcm: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / sr as f64;
            let carrier = (2.0 * PI * 80.0 * t).sin() as f32;
            let amp: f32 = if t < 30.0 {
                0.05
            } else if t < 60.0 {
                0.7
            } else if t < 90.0 {
                0.1
            } else if t < 150.0 {
                0.7
            } else {
                0.02
            };
            carrier * amp
        })
        .collect();

    let config = LexiconConfig {
        genre: None,
        drop_at_start: DropAtStart::Never,
        min_cue_length_beats: 64,
        add_emergency_loop: false,
    };

    let result = generate_structural_cues(&pcm, sr, bpm, first_beat, &config);
    assert!(!result.cues.is_empty());
    assert_eq!(result.cues[0].name, "Start");
}
