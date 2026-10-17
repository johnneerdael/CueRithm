mod power;
mod scales;
mod tables;
use crate::dsp::energy_temporal_rust_v1::Description;
#[derive(Debug)]
pub struct Record {
    pub tempo: f64,
    pub features: Vec<f64>,
}
fn valid(v: &[f64]) -> bool {
    v.len() <= 100000 && v.iter().all(|x| x.is_finite() && x.abs() <= 1e100)
}
pub fn lookup(v: &[f64], p: f64) -> Result<f64, String> {
    if !valid(v) || !p.is_finite() || !(0.0..=1.).contains(&p) || v.windows(2).any(|r| r[0] > r[1])
    {
        return Err("finite sorted double quantile profile required".into());
    }
    if v.is_empty() {
        return Ok(0.);
    }
    if v.len() == 1 || p <= 0. {
        return Ok(v[0]);
    }
    let t = p * (v.len() - 1) as f64;
    let i = (t.floor() as usize).min(v.len() - 2);
    let difference = v[i + 1] - v[i];
    Ok(difference.mul_add(t - i as f64, v[i]))
}
pub fn count_beats(v: &[f64], offset: usize, lag: usize) -> Result<(f64, f64), String> {
    if v.len() != 256 || !valid(v) || offset > 255 || lag > 255 {
        return Err("finite256-bin count profile required".into());
    }
    let mut values: Vec<f64> = (0..256)
        .step_by(2)
        .map(|i| {
            (v[(i + offset) & 255] * v[(i + offset + lag) & 255])
                .abs()
                .sqrt()
        })
        .collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = lookup(&values, scales::BEAT_PERCENTILE)?;
    let threshold = q * 0.5;
    let count = values.iter().filter(|&&x| x >= threshold).count() as f64;
    Ok((count, q))
}
fn amplitude(q: f64, band: usize) -> Result<f64, String> {
    let base =
        (q * f64::from_bits(scales::TRANSIENT_QUANTILE[band])) * scales::AMPLITUDE_INPUT_FACTOR;
    let value = if base == 0. {
        0.
    } else {
        power::source_power(base, scales::POW_EXPONENT_AND_OFFBEAT_COUNT_FACTOR)?
    };
    Ok(value * scales::POW_OUTPUT_FACTOR)
}
pub fn reduce(d: &Description) -> Result<Record, String> {
    if !d.tempo.is_finite() {
        return Err("finite tempo required".into());
    }
    if d.rejected {
        return Ok(Record {
            tempo: d.tempo,
            features: Vec::new(),
        });
    }
    if d.aggregate.len() != 14
        || !valid(&d.aggregate)
        || d.aggregate.iter().any(|x| *x < 0.)
        || d.families.iter().any(|f| {
            f.len() != 14
                || f.iter()
                    .any(|r| r.len() != 256 || !valid(r) || r.iter().any(|x| *x < 0.))
        })
    {
        return Err("accepted finite fourteen-band descriptions required".into());
    }
    let mut norms = Vec::with_capacity(256);
    for i in 0..256 {
        let mut sum = 0f64;
        for b in 0..14 {
            let t = d.families[1][b][i];
            let s = d.families[3][b][i];
            let pair = s.mul_add(s, t * t);
            sum += pair;
        }
        norms.push(sum.sqrt());
    }
    norms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = lookup(&norms, scales::NORM_PERCENTILE)?;
    let gain = (if q > 0. { scales::NORM_TARGET / q } else { 1. })
        .sqrt()
        .abs();
    let mut features = vec![d.tempo * scales::TEMPO_SCALE];
    for b in 0..14 {
        let t: Vec<f64> = d.families[1][b].iter().map(|x| x * gain).collect();
        let s: Vec<f64> = d.families[3][b].iter().map(|x| x * gain).collect();
        let count_scale = f64::from_bits(scales::TRANSIENT_COUNT[b]);
        features.push((d.aggregate[b] * gain) * f64::from_bits(scales::AGGREGATE[b]));
        let (c, q) = count_beats(&t, 0, 0)?;
        features.push(amplitude(q, b)?);
        features.push(c * count_scale);
        for lag in [2, 4, 8] {
            let (c, q) = count_beats(&t, 0, lag)?;
            features.push(amplitude(q, b)? * scales::LAG_AMPLITUDE_FACTOR);
            features.push(
                (c * count_scale)
                    * if lag == 8 {
                        scales::LAG8_COUNT_FACTOR
                    } else {
                        0.5
                    },
            );
        }
        let (c, q) = count_beats(&t, 1, 0)?;
        features.push(amplitude(q, b)? * scales::OFFBEAT_FACTOR);
        features.push(
            ((c * count_scale) * scales::POW_EXPONENT_AND_OFFBEAT_COUNT_FACTOR)
                * scales::OFFBEAT_FACTOR,
        );
        let (_, q) = count_beats(&s, 1, 0)?;
        features.push(q * f64::from_bits(scales::SUSTAINED_ODD_QUANTILE[b]));
        let (_, q) = count_beats(&s, 0, 0)?;
        features.push(q * f64::from_bits(scales::SUSTAINED_EVEN_QUANTILE[b]));
    }
    for (value, scale) in features.iter_mut().zip(scales::FINAL_FEATURE) {
        let scaled = f64::from_bits(scale) * *value;
        *value = scaled;
    }
    if !valid(&features) {
        return Err("finite reduced feature profile exceeded".into());
    }
    Ok(Record {
        tempo: d.tempo,
        features,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn double_quantile_keeps_rounded_difference() {
        assert_eq!(lookup(&[-18014398509481984., -1.], 1.).unwrap(), 0.);
    }
    #[test]
    fn silent_beat_count_is128() {
        assert_eq!(count_beats(&[0.; 256], 0, 0).unwrap(), (128., 0.));
    }
    #[test]
    fn lag_zero_preserves_product_underflow() {
        assert_eq!(count_beats(&[1e-200; 256], 0, 0).unwrap(), (128., 0.));
    }
    #[test]
    fn constant_beats_retain_quantile() {
        assert_eq!(count_beats(&[4.; 256], 1, 8).unwrap(), (128., 4.));
    }
    #[test]
    fn rejected_description_emits_empty_record() {
        let d = Description {
            tempo: 128.,
            extent: 1.,
            rms: 0.,
            rejected: true,
            aggregate: vec![],
            families: std::array::from_fn(|_| vec![]),
        };
        let r = reduce(&d).unwrap();
        assert_eq!(r.tempo, 128.);
        assert!(r.features.is_empty());
    }
    #[test]
    fn silence_feature_count183_includes_nonzero_counts() {
        let d = Description {
            tempo: 128.,
            extent: 1.,
            rms: 0.,
            rejected: false,
            aggregate: vec![0.; 14],
            families: std::array::from_fn(|_| vec![vec![0.; 256]; 14]),
        };
        let r = reduce(&d).unwrap();
        assert_eq!(r.features.len(), 183);
        assert_eq!(r.features[0], 128. * 0.01332);
        assert_eq!(r.features[1], 0.);
        assert_eq!(r.features[2], 0.);
        assert!(r.features[3] > 0.);
        assert!(r.features.iter().all(|x| x.is_finite()));
    }
}

pub fn process_packet(input: &[u8]) -> Result<Vec<u8>, String> {
    if input.len() < 32 || &input[..4] != b"TDI1" {
        return Err("TDI1header required".into());
    }
    let n = u32::from_le_bytes(input[4..8].try_into().unwrap()) as usize;
    if n == 0 || n > 8192 || input.len() != 32 + n * 30 * 4 {
        return Err("bounded temporal packet shape".into());
    }
    let config: [f64; 3] = std::array::from_fn(|i| {
        f64::from_le_bytes(input[8 + 8 * i..16 + 8 * i].try_into().unwrap())
    });
    let mut words = input[32..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| f32::from_le_bytes(*v));
    let a: Vec<Vec<f32>> = (0..14).map(|_| words.by_ref().take(n).collect()).collect();
    let b: Vec<Vec<f32>> = (0..14).map(|_| words.by_ref().take(n).collect()).collect();
    let weighted: Vec<f32> = words.by_ref().take(n).collect();
    let raw: Vec<f32> = words.take(n).collect();
    let t = crate::dsp::energy_temporal_rust_v1::analyze(
        &a, &b, &weighted, &raw, config[0], config[1], config[2],
    )?;
    let mut out = b"TDO1".to_vec();
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&14u32.to_le_bytes());
    for row in t.smoothed.sustained.iter().chain(&t.smoothed.transient) {
        for v in row {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend_from_slice(&(t.descriptions.len() as u32).to_le_bytes());
    for d in &t.descriptions {
        for v in [d.tempo, d.extent, d.rms] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&u32::from(d.rejected).to_le_bytes());
        out.extend_from_slice(&(d.aggregate.len() as u32).to_le_bytes());
        for v in &d.aggregate {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for family in &d.families {
            out.extend_from_slice(&(family.len() as u32).to_le_bytes());
            for row in family {
                out.extend_from_slice(&(row.len() as u32).to_le_bytes());
                for v in row {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
    }
    if out.len() > 268435456 {
        return Err("temporal output bound".into());
    }
    let mut full = b"EFO1".to_vec();
    full.extend_from_slice(&(out.len() as u32).to_le_bytes());
    full.extend_from_slice(&out);
    full.extend_from_slice(&(t.descriptions.len() as u32).to_le_bytes());
    for d in &t.descriptions {
        let r = reduce(d)?;
        full.extend_from_slice(&r.tempo.to_le_bytes());
        full.extend_from_slice(&(r.features.len() as u32).to_le_bytes());
        for v in r.features {
            full.extend_from_slice(&v.to_le_bytes());
        }
    }
    Ok(full)
}

#[cfg(test)]
mod packet_tests {
    use super::*;
    #[test]
    fn packet_emits_full_energy_projection() {
        let mut data = b"TDI1".to_vec();
        data.extend_from_slice(&1u32.to_le_bytes());
        for v in [1f64, 0., 0.] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        for _ in 0..28 {
            data.extend_from_slice(&0f32.to_le_bytes());
        }
        data.extend_from_slice(&1f32.to_le_bytes());
        data.extend_from_slice(&0f32.to_le_bytes());
        let out = process_packet(&data).unwrap();
        assert_eq!(&out[..4], b"EFO1");
    }
}
