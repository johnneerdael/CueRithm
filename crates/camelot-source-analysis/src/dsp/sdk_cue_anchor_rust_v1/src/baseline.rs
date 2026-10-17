// Private numeric-stage prototype conditioned on original cached features and stored beats.
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ArrayInput {
    pub amplitudes: Vec<f32>,
    pub amplitude_bits: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct WaveformInput {
    pub sample_rate: f64,
    pub hop: f64,
    pub arrays: Vec<ArrayInput>,
}

#[derive(Debug, Deserialize)]
pub struct BeatInput {
    pub beat_times_seconds: Vec<f64>,
    pub beat_time_bits: Option<Vec<String>>,
    pub tempo: f64,
    pub tempo_bits: Option<String>,
    pub duration_seconds: f64,
    pub duration_bits: Option<String>,
    pub downbeat_time: f64,
    pub downbeat_bits: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CurveOutput {
    pub scope: &'static str,
    pub score_bits: Vec<String>,
    #[serde(rename = "CuePointData")]
    pub cue_point_data: String,
    #[serde(rename = "CuePointStartBeat")]
    pub cue_point_start_beat: usize,
    pub origin: i32,
    pub end: i32,
    pub count: usize,
    pub onset_index: usize,
    pub minimum_start_index: usize,
}

const SCOPE: &str =
    "cached-feature-conditioned Rust cue stage; not independent PCM/full-engine inference";
type Granule = [f64; 5];

fn parse_f64(bits: &str) -> Result<f64, String> {
    u64::from_str_radix(bits, 16)
        .map(f64::from_bits)
        .map_err(|e| e.to_string())
}

fn parse_f32(bits: &str) -> Result<f64, String> {
    u32::from_str_radix(bits, 16)
        .map(|x| f32::from_bits(x) as f64)
        .map_err(|e| e.to_string())
}

struct BeatContext {
    times: Vec<f64>,
    duration: f64,
    downbeat: f64,
    offset: f64,
    scale: f64,
}

impl BeatContext {
    fn new(input: &BeatInput) -> Result<Self, String> {
        let times = if let Some(bits) = &input.beat_time_bits {
            bits.iter()
                .map(|x| parse_f64(x))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            input.beat_times_seconds.clone()
        };
        if times.len() < 2
            || times.iter().any(|x| !x.is_finite())
            || times.windows(2).any(|x| x[1] <= x[0])
        {
            return Err("strictly increasing finite stored beat context required".into());
        }
        let tempo = input
            .tempo_bits
            .as_deref()
            .map(parse_f64)
            .transpose()?
            .unwrap_or(input.tempo);
        let duration = input
            .duration_bits
            .as_deref()
            .map(parse_f64)
            .transpose()?
            .unwrap_or(input.duration_seconds);
        let downbeat = input
            .downbeat_bits
            .as_deref()
            .map(parse_f64)
            .transpose()?
            .unwrap_or(input.downbeat_time);
        if !tempo.is_finite() || !duration.is_finite() || !downbeat.is_finite() {
            return Err("finite tempo/duration/downbeat required".into());
        }
        let mut context = Self {
            times,
            duration,
            downbeat,
            offset: 0.0,
            scale: if tempo > 0.0 && tempo <= 100.0 {
                2.0
            } else {
                1.0
            },
        };
        context.offset = context.position_internal(downbeat);
        Ok(context)
    }

    fn position_internal(&self, time: f64) -> f64 {
        let n = self.times.len();
        if time < self.times[0] {
            return (time - self.times[0]) / (self.times[1] - self.times[0]);
        }
        if time > self.times[n - 1] {
            return ((time - self.times[n - 1]) / (self.times[n - 1] - self.times[n - 2])
                + n as f64)
                + -1.0;
        }
        let index = self.times.partition_point(|x| *x < time).saturating_sub(1);
        (time - self.times[index]) / (self.times[index + 1] - self.times[index]) + index as f64
    }

    fn position(&self, time: f64) -> f64 {
        (self.position_internal(time) - self.offset) * self.scale
    }

    fn time(&self, position: f64) -> f64 {
        let position = position / self.scale + self.offset;
        let index = position.trunc() as isize;
        if index < 0 {
            return position.mul_add(self.times[1] - self.times[0], self.times[0]);
        }
        let n = self.times.len();
        if index as usize + 1 >= n {
            let delta = (position - n as f64) + 1.0;
            return delta.mul_add(self.times[n - 1] - self.times[n - 2], self.times[n - 1]);
        }
        let index = index as usize;
        (position - index as f64)
            .mul_add(self.times[index + 1] - self.times[index], self.times[index])
    }
}

struct Waveform {
    rate: f64,
    hop: f64,
    arrays: Vec<Vec<f64>>,
    rate_scale: f64,
}

impl Waveform {
    fn new(input: &WaveformInput) -> Result<Self, String> {
        if !input.sample_rate.is_finite()
            || input.sample_rate <= 0.0
            || !input.hop.is_finite()
            || input.hop <= 0.0
        {
            return Err("positive finite waveform sample rate and hop required".into());
        }
        let arrays = input
            .arrays
            .iter()
            .map(|array| {
                if let Some(bits) = &array.amplitude_bits {
                    bits.iter()
                        .map(|x| parse_f32(x))
                        .collect::<Result<Vec<_>, _>>()
                } else {
                    Ok(array.amplitudes.iter().map(|x| *x as f64).collect())
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        if arrays.len() < 3
            || arrays[1].len() < arrays[0].len()
            || arrays[2].len() < arrays[0].len()
            || arrays.iter().flatten().any(|x| !x.is_finite())
        {
            return Err("finite cached waveform arrays0/1/2 of compatible lengths required".into());
        }
        Ok(Self {
            rate: input.sample_rate,
            hop: input.hop,
            arrays,
            rate_scale: ((input.sample_rate * 0.5) / input.hop).powf(0.8333333333333334),
        })
    }

    fn weighted_average(
        &self,
        channel: usize,
        endpoints: [f64; 4],
        minimum: f64,
        power: f64,
    ) -> f64 {
        let [t1, t2, t3, t4] = endpoints;
        let begin = ((self.rate * t1) / self.hop).floor().max(0.0) as isize;
        let end =
            (((self.rate * t4) / self.hop).ceil() as isize).min(self.arrays[0].len() as isize);
        let mut accumulated = 0.0;
        for index in begin..end {
            let mut value = self.arrays[channel][index as usize].max(0.0);
            if value < minimum {
                value = 0.0;
            }
            let time = ((index as f64 + 0.5) / self.rate) * self.hop;
            let mut weight = 0.0;
            if t1 < time && time < t2 {
                let u = ((time - t1) * 0.5) / (t2 - t1);
                if u > 0.0 && u < 1.0 {
                    weight = (u * (std::f64::consts::PI + std::f64::consts::PI))
                        .cos()
                        .mul_add(-0.5, 0.5);
                }
            }
            if t2 <= time && time <= t3 {
                weight = 1.0;
            }
            if t3 < time && time < t4 {
                let u = ((time - t3) * 0.5) / (t4 - t3) + 0.5;
                weight = 0.0;
                if u > 0.0 && u < 1.0 {
                    weight = (u * (std::f64::consts::PI + std::f64::consts::PI))
                        .cos()
                        .mul_add(-0.5, 0.5);
                }
            }
            accumulated = weight.mul_add(value.powf(power), accumulated);
        }
        let edge = ((t2 - t1) + t4) - t3;
        let mut area = (edge.mul_add(0.5, t3 - t2) * self.rate) / self.hop;
        if area <= 0.0 {
            area = 1.0;
        }
        self.rate_scale * (accumulated / area).powf(1.0 / power)
    }

    fn granule(&self, start: f64, end: f64) -> Granule {
        let duration = end - start;
        let first = [
            duration.mul_add(-0.12, start),
            duration.mul_add(-0.02, start),
            duration.mul_add(-0.12, end),
            duration.mul_add(-0.02, end),
        ];
        let second = [
            duration.mul_add(-0.18, start),
            duration.mul_add(-0.12, start),
            duration.mul_add(-0.18, end),
            duration.mul_add(-0.12, end),
        ];
        let g0 = self.weighted_average(2, first, 0.0, 1.2);
        let g2 = self.weighted_average(2, first, 0.0, 5.0);
        let g1 = self.weighted_average(1, second, 0.3, 1.2);
        let g3 = self.weighted_average(1, second, 0.3, 5.0);
        [g0, g1, g2, g3, g0 + g1]
    }
}

fn difference_sum(
    values: &[f64; 128],
    left: usize,
    spacing: usize,
    width: usize,
    negative_gain: f64,
) -> f64 {
    let count = left / spacing;
    let shifts = width.max(spacing).div_ceil(width);
    let mut result = 0.0;
    for shift in 0..shifts {
        let base = 64 + shift * width;
        let mut before = 0.0;
        let mut after = 0.0;
        let mut sample_count = 0;
        for segment in 0..count {
            let position = base + segment * spacing;
            for sub in 0..width {
                before += values[position + sub - left];
                after += values[position + sub];
            }
            sample_count += width;
        }
        let delta = (after - before) / sample_count as f64;
        let gain = if delta >= 0.0 { 1.0 } else { negative_gain };
        let mut value = (delta * gain).abs();
        if delta < 0.0 && negative_gain < 0.0 {
            value = -value;
        }
        result += value;
    }
    result / shifts as f64
}

fn score_at(granules: &[Granule], index: usize) -> f64 {
    let mut primary = [0.0; 128];
    let mut melody = [0.0; 128];
    let mut secondary = [0.0; 128];
    for local in 0..128 {
        let source = index as isize - 64 + local as isize;
        if source >= 0 && (source as usize) < granules.len() {
            let g = granules[source as usize];
            primary[local] = g[0];
            melody[local] = g[3];
            secondary[local] = g[1];
        }
    }
    let score = difference_sum(&melody, 32, 32, 1, 0.8).mul_add(96.0, 0.0);
    let score = difference_sum(&melody, 16, 16, 1, 0.8).mul_add(96.0, score);
    let score = difference_sum(&melody, 8, 8, 1, 0.9).mul_add(96.0, score);
    let score = difference_sum(&primary, 32, 4, 1, 0.4).mul_add(98.304, score);
    let score = difference_sum(&secondary, 32, 4, 1, 0.4).mul_add(245.76, score);
    let long = difference_sum(&melody, 32, 32, 32, -1.0).mul_add(3.2, 0.0);
    let long = difference_sum(&melody, 16, 16, 16, -1.0).mul_add(1.6, long);
    let mut long = difference_sum(&melody, 8, 8, 8, -1.0).mul_add(0.8, long);
    if long < 0.0 {
        long *= 0.0;
    }
    score + long
}

fn onset_index(
    granules: &[Granule],
    beats: &BeatContext,
    origin: i32,
) -> Result<(usize, usize), String> {
    if granules.len() < 64 {
        return Err("onset stage requires at least64 explicit granules".into());
    }
    let maximum_primary = granules[32..40]
        .iter()
        .map(|g| g[0])
        .fold(f64::NEG_INFINITY, f64::max);
    let maximum_combined = granules[32..40]
        .iter()
        .map(|g| g[4])
        .fold(f64::NEG_INFINITY, f64::max);
    let mut index = 32;
    if maximum_primary < 5.0 && maximum_combined < 7.0 {
        for offset in 0..64.min(granules.len() - 33) {
            let current = 32 + offset;
            if granules[current][0] > maximum_primary.mul_add(1.6, 5.0)
                || granules[current][4] > 60.0
            {
                index = if granules[current + 1][0] > granules[current][0] * 5.0 {
                    current + 1
                } else {
                    current
                };
                break;
            }
        }
    }
    let minimum = ((beats.position(-0.018).ceil() as i32) - origin).max(4) as usize;
    if index + 20 > granules.len() {
        return Err("insufficient onset neighborhood".into());
    }
    let mut dramatic = false;
    for start in [index + 4, index + 12] {
        let window = &granules[start..start + 8];
        let maximum = window[..4]
            .iter()
            .map(|g| g[3])
            .fold(f64::NEG_INFINITY, f64::max);
        dramatic = dramatic || maximum.mul_add(3.0, 40.0) <= window[4][3];
    }
    if beats.time((index as i32 + origin) as f64) >= -0.0001 {
        let current = granules[index];
        let previous = granules[index - 1];
        let previous2 = granules[index - 2];
        let threshold = current[0].mul_add(0.4, 7.0);
        let flag = previous[0] >= threshold
            && current[0] * 0.1 > previous2[0]
            && !(current[3] >= 155.0 && previous[3] < current[3] * 0.5);
        if previous[0] < threshold + threshold || current[0] * 0.1 <= previous2[0] {
            if flag {
                index -= 1;
            }
        } else if current[3] < 165.0 || flag || current[3] * 0.35 <= previous[3] {
            index -= 1;
        }
    }
    let primary_threshold = if dramatic { 70.0 } else { 6.0 };
    while index > minimum {
        let neighborhood = &granules[index - 1..index + 5];
        if neighborhood.iter().any(|g| g[0] < primary_threshold) {
            break;
        }
        let min_melody = neighborhood
            .iter()
            .map(|g| g[3])
            .fold(f64::INFINITY, f64::min);
        let threshold = min_melody.mul_add(1.24, 44.0).max(60.0);
        if granules[index..index + 5].iter().any(|g| g[3] >= threshold) {
            break;
        }
        index -= 1;
    }
    Ok((index, minimum))
}

fn encode_curve(scores: &[f64]) -> (String, Vec<String>) {
    use base64::Engine;
    if scores.is_empty() {
        return (String::new(), Vec::new());
    }
    let values: Vec<f32> = scores.iter().map(|x| *x as f32).collect();
    let leading = values.iter().position(|x| *x != 0.0).unwrap_or(0).min(255);
    let trailing = values
        .iter()
        .rev()
        .position(|x| *x != 0.0)
        .unwrap_or(0)
        .min(255)
        .min(values.len() - leading);
    let mut payload = vec![leading as u8, trailing as u8];
    for value in &values[leading..values.len() - trailing] {
        payload.extend(value.to_le_bytes());
    }
    let bits = values
        .iter()
        .map(|x| format!("{:08x}", x.to_bits()))
        .collect();
    (
        base64::engine::general_purpose::STANDARD.encode(payload),
        bits,
    )
}

pub fn predict(waveform: &WaveformInput, beats: &BeatInput) -> Result<CurveOutput, String> {
    let waveform = Waveform::new(waveform)?;
    let beats = BeatContext::new(beats)?;
    let first = (beats.position(beats.downbeat) + 0.5).floor() as i32;
    let origin = (((first + 32) / 64) * 64).max(0) - 32;
    let last = (beats.position(beats.duration) + 0.5).floor() as i32;
    let end = ((last + 16) / 32) * 32;
    if end <= origin {
        return Ok(CurveOutput {
            scope: SCOPE,
            score_bits: Vec::new(),
            cue_point_data: String::new(),
            cue_point_start_beat: 0,
            origin,
            end,
            count: 0,
            onset_index: 0,
            minimum_start_index: 0,
        });
    }
    if end - origin > 1_000_000 {
        return Err("diagnostic granule limit exceeded".into());
    }
    let mut granules: Vec<Granule> = (origin..end)
        .map(|i| waveform.granule(beats.time(i as f64), beats.time(i as f64 + 1.0)))
        .collect();
    granules.extend([[0.0; 5]; 32]);
    let (onset, minimum) = onset_index(&granules, &beats, origin)?;
    let mut scores = vec![0.0; granules.len()];
    if onset + 96 < scores.len() {
        for (index, score) in scores
            .iter_mut()
            .enumerate()
            .take(granules.len() - 80)
            .skip(onset + 16)
        {
            *score = score_at(&granules, index);
        }
    }
    let (packed, score_bits) = encode_curve(&scores);
    Ok(CurveOutput {
        scope: SCOPE,
        score_bits,
        cue_point_data: packed,
        cue_point_start_beat: onset.max(minimum),
        origin,
        end,
        count: scores.len(),
        onset_index: onset,
        minimum_start_index: minimum,
    })
}

#[cfg(test)]
mod tests {
    use super::encode_curve;
    use base64::Engine;

    #[test]
    fn empty_curve_has_no_header_while_nonempty_all_zero_curve_keeps_values() {
        assert_eq!(encode_curve(&[]), (String::new(), Vec::new()));
        let (packed, bits) = encode_curve(&[0.0, -0.0]);
        let payload = base64::engine::general_purpose::STANDARD
            .decode(packed)
            .unwrap();
        assert_eq!(payload, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 128]);
        assert_eq!(bits, vec!["00000000", "80000000"]);
    }

    #[test]
    fn capped_headers_leave_remaining_zero_values_inside_payload() {
        let mut input = vec![0.0; 300];
        input.push(1.0);
        input.extend([0.0; 300]);
        let (packed, bits) = encode_curve(&input);
        let payload = base64::engine::general_purpose::STANDARD
            .decode(packed)
            .unwrap();
        assert_eq!(&payload[..2], &[255, 255]);
        assert_eq!(payload.len(), 2 + 91 * 4);
        assert_eq!(&payload[2 + 45 * 4..2 + 46 * 4], &1.0_f32.to_le_bytes());
        assert_eq!(bits.len(), 601);
    }
}
