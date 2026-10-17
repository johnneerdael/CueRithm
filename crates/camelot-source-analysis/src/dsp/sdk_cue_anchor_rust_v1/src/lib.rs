include!("baseline.rs");

#[derive(Debug, Serialize)]
pub struct AnchorOutput {
    #[serde(flatten)]
    pub payload: CurveOutput,
    pub raw_f64_scores: Vec<f64>,
    pub raw_f64_score_bits: Vec<String>,
    pub anchor_available: bool,
    pub selected_phase: u32,
    pub selected_normalized_coordinate: f64,
    pub selected_normalized_coordinate_bits: String,
    pub selected_time: f64,
    pub selected_time_bits: String,
}

pub fn phrase_weight(beat: i32, weights: [f64; 5]) -> f64 {
    let factors: [f64; 5] = std::array::from_fn(|i| {
        let period = [32, 16, 8, 4, 2][i];
        let remainder = beat & (period - 1);
        let distance = remainder.min(period - remainder) as f64 * (2.0 / period as f64);
        (weights[i] - 1.0).mul_add(distance.clamp(0.0, 1.0), 1.0)
    });
    ((factors[2] * (factors[0] * factors[1])) * factors[3]) * factors[4]
}

fn hanning_sum(values: &[f64], index: usize, width: f64) -> f64 {
    let half = ((width - 1.0) * 0.5).ceil() as usize;
    let mut total = 0.0;
    for (position, value) in values
        .iter()
        .enumerate()
        .take((index + half + 1).min(values.len()))
        .skip(index.saturating_sub(half))
    {
        let u = (position as f64 - index as f64) / (width + 1.0) + 0.5;
        let weight = if u > 0.0 && u < 1.0 {
            (u * (std::f64::consts::PI + std::f64::consts::PI))
                .cos()
                .mul_add(-0.5, 0.5)
        } else {
            0.0
        };
        total = value.mul_add(weight, total);
    }
    total / ((width + 1.0) * 0.5)
}

pub fn select_raw_phase(scores: &[f64], start: usize) -> Result<u32, String> {
    if scores.is_empty()
        || scores.len() > 100_000
        || start >= scores.len()
        || scores.iter().any(|x| !x.is_finite())
    {
        return Err("finite bounded raw f64 curve and valid start required".into());
    }
    let count = scores.len() - start;
    if count < 96 {
        return Ok(0);
    }
    let whole = ((count - 64) / 32) * 32;
    let end = start + whole;
    let mut taper = vec![0.0; scores.len()];
    for index in start..scores.len() {
        let coordinate = 32 + index - start;
        let distance = coordinate
            .abs_diff(64)
            .min(9999)
            .min(coordinate.abs_diff(whole)) as f64;
        let weight = (((distance * 0.03125) * distance) * 0.03125).min(1.0);
        if index >= start + 32 && index <= end - 32 {
            taper[index] = scores[index] * weight;
        }
    }
    let powered: Vec<f64> = (0..scores.len())
        .map(|index| {
            let narrow = hanning_sum(&taper, index, 1.0);
            let middle = hanning_sum(&taper, index, 3.0);
            let wide = hanning_sum(&taper, index, 32.0);
            let sharp = (narrow - middle).mul_add(0.5, narrow) - wide;
            if sharp > 0.0 { sharp.powf(1.1) } else { 0.0 }
        })
        .collect();
    let base = [0.4, 0.3, 0.1, 0.0, 0.0];
    let bias = [
        0x3feccccccccccccd,
        0x3fed916872b020c5,
        0x3feeb851eb851eb8,
        0x3fef22d0e5604189,
        0x3fef126e978d4fdf,
    ]
    .map(|bits| f64::from_bits(bits).powf(1.15));
    let mut best = 0;
    let mut best_value = 0.0;
    for phase in 0..32 {
        let mut value = 0.0;
        for (index, sample) in powered
            .iter()
            .enumerate()
            .take(end.min(scores.len()))
            .skip(start)
        {
            value = sample.mul_add(
                phrase_weight(index as i32 - start as i32 - phase, base),
                value,
            );
        }
        let weighting = phrase_weight(phase, bias);
        value *= weighting * weighting;
        if value > best_value {
            best = phase;
            best_value = value;
        }
    }
    Ok(best as u32)
}

// New SDK API uses the strict source GetHalfBeatsEnabled boundary. The included
// baseline remains unchanged and is not reused for this selection decision.
fn anchor_context(input: &BeatInput) -> Result<BeatContext, String> {
    let mut context = BeatContext::new(input)?;
    let tempo = input
        .tempo_bits
        .as_deref()
        .map(parse_f64)
        .transpose()?
        .unwrap_or(input.tempo);
    let normalized = crate::dsp::beat_refinement_full_rust_v3::full::Beats::new(
        context.times.clone(),
        tempo,
        context.duration,
    )?;
    if normalized.times.len() < 2 || normalized.times.len() > 100_000 {
        return Err("normalized grid requires2..100000 points".into());
    }
    context.times = normalized.times;
    context.offset = context.position_internal(context.downbeat);
    context.scale = if tempo > 0.0 && tempo < 100.0 {
        2.0
    } else {
        1.0
    };
    Ok(context)
}

fn validate_anchor_input(waveform: &WaveformInput, beats: &BeatInput) -> Result<(), String> {
    if waveform.sample_rate != 44100.0
        || waveform.hop != 1024.0
        || waveform.arrays.len() < 3
        || waveform.arrays.len() > 8
    {
        return Err("fixed44100Hz/hop1024 with3..8 waveform arrays required".into());
    }
    for array in &waveform.arrays {
        if let Some(bits) = &array.amplitude_bits
            && (bits.len() != array.amplitudes.len()
                || bits.iter().zip(&array.amplitudes).any(|(text, value)| {
                    text.len() != 8 || u32::from_str_radix(text, 16).ok() != Some(value.to_bits())
                }))
        {
            return Err("waveform bits and numeric amplitudes must agree".into());
        }
        if array.amplitudes.len() > 1_000_000
            || array
                .amplitude_bits
                .as_ref()
                .is_some_and(|x| x.len() > 1_000_000)
        {
            return Err("waveform frame limit exceeded".into());
        }
    }
    if beats.beat_times_seconds.len() > 100_000
        || beats
            .beat_time_bits
            .as_ref()
            .is_some_and(|x| x.len() > 100_000)
    {
        return Err("beat count limit exceeded".into());
    }
    if let Some(bits) = &beats.beat_time_bits
        && (bits.len() != beats.beat_times_seconds.len()
            || bits
                .iter()
                .zip(&beats.beat_times_seconds)
                .any(|(text, value)| {
                    text.len() != 16 || u64::from_str_radix(text, 16).ok() != Some(value.to_bits())
                }))
    {
        return Err("beat bits and numeric times must agree".into());
    }
    for (text, value) in [
        (&beats.tempo_bits, beats.tempo),
        (&beats.duration_bits, beats.duration_seconds),
        (&beats.downbeat_bits, beats.downbeat_time),
    ] {
        if text.as_ref().is_some_and(|text| {
            text.len() != 16 || u64::from_str_radix(text, 16).ok() != Some(value.to_bits())
        }) {
            return Err("scalar bits and numeric values must agree".into());
        }
    }
    let context = anchor_context(beats)?;
    let tempo = beats
        .tempo_bits
        .as_deref()
        .map(parse_f64)
        .transpose()?
        .unwrap_or(beats.tempo);
    if !(30.0..=400.0).contains(&tempo)
        || context.duration <= 0.0
        || context.duration > 86400.0
        || context.downbeat.abs() > 86400.0
        || context.times.iter().any(|x| x.abs() > 86400.0)
        || context.times.windows(2).any(|x| x[1] - x[0] < 1e-6)
    {
        return Err(
            "bounded positive tempo/duration and progressing beat geometry required".into(),
        );
    }
    let projected = context.position(context.duration);
    if !projected.is_finite() || !(-32.0..=99_000.0).contains(&projected) {
        return Err("projected curve limit exceeded".into());
    }
    Ok(())
}

pub fn predict_anchor(waveform: &WaveformInput, input: &BeatInput) -> Result<AnchorOutput, String> {
    validate_anchor_input(waveform, input)?;
    let waveform = Waveform::new(waveform)?;
    let beats = anchor_context(input)?;
    let first = (beats.position(beats.downbeat) + 0.5).floor() as i32;
    let origin = (((first + 32) / 64) * 64).max(0) - 32;
    let last = (beats.position(beats.duration) + 0.5).floor() as i32;
    let end = ((last + 16) / 32) * 32;
    // SDK3232c only enters raw-score/anchor construction when rounded last>15.
    // Empty result retains the native object's zero time and inactive phase.
    // Report the inverse of that default time, not a selected phase coordinate.
    if last < 16 || end <= origin {
        let default_coordinate = beats.position(0.0);
        return Ok(AnchorOutput {
            payload: CurveOutput {
                scope: "SDK source empty-result branch; no selected phase",
                score_bits: Vec::new(),
                cue_point_data: String::new(),
                cue_point_start_beat: 0,
                origin,
                end,
                count: 0,
                onset_index: 0,
                minimum_start_index: 0,
            },
            raw_f64_scores: Vec::new(),
            raw_f64_score_bits: Vec::new(),
            anchor_available: false,
            selected_phase: 0,
            selected_normalized_coordinate: default_coordinate,
            selected_normalized_coordinate_bits: format!("{:016x}", default_coordinate.to_bits()),
            selected_time: 0.0,
            selected_time_bits: "0000000000000000".into(),
        });
    }
    let mut granules: Vec<Granule> = (origin..end)
        .map(|i| waveform.granule(beats.time(i as f64), beats.time(i as f64 + 1.0)))
        .collect();
    granules.extend([[0.0; 5]; 32]);
    let (onset, minimum) = onset_index(&granules, &beats, origin)?;
    let mut raw = vec![0.0; granules.len()];
    if onset + 96 < raw.len() {
        for (index, score) in raw
            .iter_mut()
            .enumerate()
            .take(granules.len() - 80)
            .skip(onset + 16)
        {
            *score = score_at(&granules, index);
        }
    }
    let start = onset.max(minimum);
    let phase = select_raw_phase(&raw, start)?;
    let coordinate = (start + phase as usize) as f64;
    let time = beats.time(coordinate);
    if !time.is_finite() || raw.iter().any(|x| !x.is_finite()) {
        return Err("nonfinite source result outside bounded contract".into());
    }
    let (packed, score_bits) = encode_curve(&raw);
    Ok(AnchorOutput {
        payload: CurveOutput {
            scope: "independent numeric waveform/refined-grid conditioned SDK curve+anchor; not full PCM/legacy inference",
            score_bits,
            cue_point_data: packed,
            cue_point_start_beat: start,
            origin,
            end,
            count: raw.len(),
            onset_index: onset,
            minimum_start_index: minimum,
        },
        raw_f64_score_bits: raw
            .iter()
            .map(|x| format!("{:016x}", x.to_bits()))
            .collect(),
        raw_f64_scores: raw,
        anchor_available: true,
        selected_phase: phase,
        selected_normalized_coordinate: coordinate,
        selected_normalized_coordinate_bits: format!("{:016x}", coordinate.to_bits()),
        selected_time: time,
        selected_time_bits: format!("{:016x}", time.to_bits()),
    })
}
