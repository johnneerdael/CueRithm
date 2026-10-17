use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub sample_rate: f64,
    pub hop: i32,
    pub audio_frames: u64,
    pub arrays: Vec<Vec<f32>>,
    pub legacy_beats: Vec<f64>,
    #[serde(default)]
    pub legacy_beat_bits: Vec<String>,
    pub legacy_tempo: f64,
}
#[derive(Serialize)]
pub struct Output {
    pub scope: &'static str,
    pub wire_tempo: f64,
    pub wire_tempo_bits: String,
    pub wire_downbeat: f64,
    pub wire_downbeat_bits: String,
    pub beat_times: Vec<f64>,
    pub beat_bits: Vec<String>,
    #[serde(rename = "CuePointData")]
    pub cue_point_data: String,
    #[serde(rename = "CuePointStartBeat")]
    pub cue_point_start_beat: usize,
    pub diagnostics: serde_json::Value,
}
fn sdk_wave(input: &Input) -> crate::dsp::sdk_cue_anchor_rust_v1::WaveformInput {
    crate::dsp::sdk_cue_anchor_rust_v1::WaveformInput {
        sample_rate: input.sample_rate,
        hop: input.hop as f64,
        arrays: input
            .arrays
            .iter()
            .map(|v| crate::dsp::sdk_cue_anchor_rust_v1::ArrayInput {
                amplitudes: v.clone(),
                amplitude_bits: None,
            })
            .collect(),
    }
}
fn sdk_beats(
    grid: &crate::dsp::main_timing_steps_rust_v1::Grid,
) -> crate::dsp::sdk_cue_anchor_rust_v1::BeatInput {
    crate::dsp::sdk_cue_anchor_rust_v1::BeatInput {
        beat_times_seconds: grid.times.clone(),
        beat_time_bits: None,
        tempo: grid.tempo,
        tempo_bits: None,
        duration_seconds: grid.duration,
        duration_bits: None,
        downbeat_time: grid.downbeat,
        downbeat_bits: None,
    }
}
pub fn analyze(input: Input) -> Result<Output, String> {
    use crate::dsp::main_timing_steps_rust_v1::{
        Grid, Waveform, align_anchor, final_resample, pass1_half_shift, prepare_pass2,
        special_tempo,
    };
    if input.sample_rate != 44100.0
        || input.hop != 1024
        || input.audio_frames == 0
        || input.audio_frames > 44100 * 86400
        || input.arrays.len() != 8
    {
        return Err("fixed44100/hop1024/eight-array Main audio profile required".into());
    }
    let rows = if input.audio_frames >= 5120 {
        ((input.audio_frames - 5120) / 2048 + 1) * 2
    } else {
        0
    };
    if rows == 0
        || rows > 100000
        || input.arrays[..6].iter().any(|x| x.len() != rows as usize)
        || input.arrays[6..].iter().any(|x| !x.is_empty())
    {
        return Err("audio-derived waveform row geometry differs".into());
    }
    let duration = input.audio_frames as f64 / input.sample_rate;
    let initial = crate::dsp::beat_refinement_full_rust_v3::full::FullInput {
        sample_rate: input.sample_rate,
        hop: input.hop,
        samples: input.arrays[1].clone(),
        flags: vec![1; rows as usize],
        melody_samples: input.arrays[2].clone(),
        melody_flags: vec![1; rows as usize],
        legacy_beats: input.legacy_beats.clone(),
        legacy_beat_bits: input.legacy_beat_bits.clone(),
        legacy_tempo: input.legacy_tempo,
        duration,
        audio_frames: input.audio_frames,
    };
    let refined = crate::dsp::beat_refinement_full_rust_v3::full::refine_full(&initial)?;
    let saved_tempo = refined.tempo;
    let initial_grid = Grid::new(
        refined.beat_times.iter().map(|t| *t + 0.0).collect(),
        saved_tempo,
        duration,
    )?;
    let wave = Waveform::new(input.sample_rate, input.hop, input.arrays.clone())?;
    let (pass1, pass1_diagnostics) = pass1_half_shift(&initial_grid, &wave)?;
    let sdk_wave = sdk_wave(&input);
    let first_anchor =
        crate::dsp::sdk_cue_anchor_rust_v1::predict_anchor(&sdk_wave, &sdk_beats(&pass1))?;
    let (mut prepared, prepared_diagnostics) =
        prepare_pass2(&pass1, &wave, first_anchor.selected_time)?;
    let mut custom = None;
    let mut origin_update = None;
    if !special_tempo(saved_tempo) {
        let custom_wave = crate::dsp::main_custom_cue_rust_v1::WaveformInput {
            sample_rate: input.sample_rate,
            hop: input.hop as f64,
            arrays: input
                .arrays
                .iter()
                .map(|v| crate::dsp::main_custom_cue_rust_v1::ArrayInput {
                    amplitudes: v.clone(),
                    amplitude_bits: None,
                })
                .collect(),
        };
        let custom_beats = crate::dsp::main_custom_cue_rust_v1::BeatInput {
            beat_times_seconds: prepared.times.clone(),
            beat_time_bits: None,
            tempo: prepared.tempo,
            tempo_bits: None,
            duration_seconds: prepared.duration,
            duration_bits: None,
            downbeat_time: prepared.downbeat,
            downbeat_bits: None,
        };
        let result = crate::dsp::main_custom_cue_rust_v1::predict(&custom_wave, &custom_beats)?;
        origin_update = Some(align_anchor(&mut prepared, result.selected_time, false)?);
        custom = Some(result);
    }
    // The source computes its final SDK curve before resampling the stored grid.
    let final_sdk =
        crate::dsp::sdk_cue_anchor_rust_v1::predict_anchor(&sdk_wave, &sdk_beats(&prepared))?;
    let final_grid = final_resample(&prepared)?;
    let wire_downbeat = final_grid.times.first().copied().unwrap_or(0.0);
    let wire_tempo = saved_tempo as f32 as f64;
    let diagnostics = serde_json::json!({"sdk_refined":refined,"pass1_half":pass1_diagnostics,"pass1_sdk_anchor":first_anchor,
        "pass2_prepare":prepared_diagnostics,"main_custom":custom,"origin_update":origin_update,
        "pre_resample_grid":prepared,"final_sdk_curve":final_sdk,"audio_frames":input.audio_frames,"duration":duration});
    Ok(Output {
        scope: "independent normal-waveform Main timing composition; original legacy marks/tempo supplied, zero FLAC waveform offset",
        wire_tempo,
        wire_tempo_bits: format!("{:016x}", wire_tempo.to_bits()),
        wire_downbeat,
        wire_downbeat_bits: format!("{:016x}", wire_downbeat.to_bits()),
        beat_times: final_grid.times.clone(),
        beat_bits: final_grid
            .times
            .iter()
            .map(|x| format!("{:016x}", x.to_bits()))
            .collect(),
        cue_point_data: final_sdk.payload.cue_point_data,
        cue_point_start_beat: final_sdk.payload.cue_point_start_beat,
        diagnostics,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_waveform_uses_source_fallback_tempo_and_complete_duration_grid() {
        let i = Input {
            sample_rate: 44100.0,
            hop: 1024,
            audio_frames: 176400,
            arrays: (0..8)
                .map(|c| if c < 6 { vec![0.0; 168] } else { vec![] })
                .collect(),
            legacy_beats: (0..8).map(|x| x as f64 * 0.5).collect(),
            legacy_beat_bits: vec![],
            legacy_tempo: 120.0,
        };
        let out = analyze(i).unwrap();
        assert_eq!(out.wire_tempo.to_bits(), 128.0_f64.to_bits());
        assert_eq!(out.beat_bits.len(), 9);
        assert!(out.wire_downbeat >= -0.2 && out.wire_downbeat < 0.46875);
    }
}
