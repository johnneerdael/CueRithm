const WIDTH: usize = 2048;
const LAGS: usize = 1024;
const RATE: f64 = 44100. / 682.;
const BASE: f64 = f64::from_bits(0x3fd3e578b464ceec);
#[derive(Debug)]
pub struct Correlation {
    pub selected: usize,
    pub scores: Vec<f64>,
    pub values: Vec<Vec<f32>>,
}
#[derive(Debug)]
pub struct Windows {
    pub span: f64,
    pub extent: f64,
    pub ranges: Vec<(usize, usize)>,
}
fn valid(v: &[f32]) -> bool {
    v.len() <= 100000 && v.iter().all(|x| x.is_finite() && x.abs() <= 1e9)
}
pub fn read(v: &[f32], p: f64) -> Result<f64, String> {
    if !valid(v) || !p.is_finite() || p.abs() > 1e8 {
        return Err("finite bounded timeline/position required".into());
    }
    Ok(read_unchecked(v, p))
}
fn read_unchecked(v: &[f32], p: f64) -> f64 {
    let i = p.floor() as i32;
    if i < -1 || v.len() < 4 || (i + 2) as usize >= v.len() {
        return 0.;
    }
    let a = f64::from(v[(i - 1).unsigned_abs() as usize]);
    let b = f64::from(v[i.unsigned_abs() as usize]);
    let c = f64::from(v[(i + 1) as usize]);
    let d = f64::from(v[(i + 2) as usize]);
    let t = p - f64::from(i);
    let t2 = t * t;
    let t3 = t * t2;
    let three = t3 * 3.;
    let w0 = ((-2f64).mul_add(t2, t) + t3) * -0.5;
    let w1 = (5f64.mul_add(t2, -2.) - three) * -0.5;
    let w2 = ((-4f64).mul_add(t2, -t) + three) * -0.5;
    let w3 = (t2 - t3) * -0.5;
    w3.mul_add(d, w2.mul_add(c, w0.mul_add(a, w1 * b)))
}
pub fn lookup(v: &[f32], p: f64) -> Result<f64, String> {
    if !valid(v) || !p.is_finite() || !(0.0..=1.).contains(&p) || v.windows(2).any(|w| w[0] > w[1])
    {
        return Err("finite sorted bounded quantile required".into());
    }
    Ok(lookup_unchecked(v, p))
}
fn lookup_unchecked(v: &[f32], p: f64) -> f64 {
    if v.is_empty() {
        return 0.;
    }
    if v.len() == 1 || p <= 0. {
        return f64::from(v[0]);
    }
    let t = p * (v.len() - 1) as f64;
    let i = (t.floor() as usize).min(v.len() - 2);
    let delta = v[i + 1] - v[i];
    f64::from(delta).mul_add(t - i as f64, f64::from(v[i]))
}
fn validate_rows(v: &[Vec<f32>]) -> Result<usize, String> {
    if v.is_empty() || v.len() > 14 {
        return Err("one to14numeric rows required".into());
    }
    let n = v[0].len();
    if n == 0 || v.iter().any(|r| r.len() != n || !valid(r)) {
        return Err("equal finite nonempty bounded timelines required".into());
    }
    Ok(n)
}
fn dot(v: &[f32], start: usize, lag: usize) -> f64 {
    let n = (v.len() - start).saturating_sub(lag).min(WIDTH);
    let mut total = 0.;
    for i in 0..n {
        total = f64::from(v[start + i]).mul_add(f64::from(v[start + i + lag]), total);
    }
    total.max(0.)
}
pub fn correlate(v: &[Vec<f32>]) -> Result<Correlation, String> {
    let n = validate_rows(v)?;
    let mut scores = Vec::new();
    let mut selected = 0;
    let mut best = -1.;
    for (j, start) in (0..n).step_by(WIDTH).enumerate() {
        let mut sum = 0.;
        for row in v {
            for lag in 0..LAGS {
                let z = (dot(row, start, lag).sqrt() / WIDTH as f64) * 50.;
                sum = z.mul_add(z, sum);
            }
        }
        scores.push(sum);
        if sum > best || j == 0 {
            selected = j;
            best = sum;
        }
    }
    let start = selected * WIDTH;
    let used = (n - start).min(WIDTH) as f64;
    let values = v
        .iter()
        .map(|row| {
            (0..LAGS)
                .map(|lag| ((dot(row, start, lag) / used).sqrt() * 50.) as f32)
                .collect()
        })
        .collect();
    Ok(Correlation {
        selected,
        scores,
        values,
    })
}
pub fn windows(n: usize, tempo: f64) -> Result<Windows, String> {
    if n == 0 || n > 100000 || !tempo.is_finite() || !(60.0..=220.).contains(&tempo) {
        return Err("bounded nonempty segment profile required".into());
    }
    let span = ((60. / tempo) * 32.) * RATE;
    let extent = n as f64 / span;
    let count = ((extent + 0.3).floor() as usize).max(1);
    let length = span.ceil() as usize;
    let mut ranges = Vec::with_capacity(count);
    for i in 0..count {
        let start = (span * i as f64).floor() as usize;
        ranges.push((start, length.min(n - start)));
    }
    Ok(Windows {
        span,
        extent,
        ranges,
    })
}
fn filtered_scores(row: &[f32]) -> Vec<f32> {
    let mut raw = Vec::new();
    let mut p = BASE;
    while p < f64::from_bits(0x3feb0ad12073615a) {
        let bpm = 60. / p;
        let step = (60. / bpm) * RATE;
        let mut sums = [0f64; 9];
        let mut x = 0.;
        let mut k = 0usize;
        while x < row.len() as f64 {
            let y = read_unchecked(row, x);
            sums[1] += y;
            for divisor in [2, 4, 8, 3] {
                sums[divisor] += if k.is_multiple_of(divisor) { y } else { -0. };
            }
            k += 1;
            x += step;
        }
        for (i, s) in sums.iter_mut().enumerate().skip(1) {
            *s = (((60. / (i as f64 * bpm)) * RATE) * *s) / 3000.;
        }
        let positive = sums[4].mul_add(2.5, sums[2].mul_add(1.4, sums[1]));
        let negative = sums[3].mul_add(1.3, sums[8] * 3.5);
        let contrast = positive - negative;
        let a =
            (bpm.ln() + f64::from_bits(0xc013687a9f1af2b1)) / f64::from_bits(0x3fe62e42fefa39ef);
        let prior = ((a * a) * -0.7).exp2();
        raw.push((prior * contrast) as f32);
        p += 0.001;
    }
    let mut gate = Vec::with_capacity(raw.len());
    let mut residual = Vec::with_capacity(raw.len());
    for (i, &value) in raw.iter().enumerate() {
        let mut sorted = raw[i.saturating_sub(32)..(i + 33).min(raw.len())].to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let q40 = lookup_unchecked(&sorted, 0.4);
        let q45 = lookup_unchecked(&sorted, 0.45);
        gate.push(if q40 * 1.2 > f64::from(value) {
            0f64
        } else {
            1.
        });
        residual.push((f64::from(value) - q45).max(0.) as f32);
    }
    residual
        .iter()
        .enumerate()
        .map(|(i, &r)| {
            let m = gate[i.saturating_sub(2)..(i + 3).min(gate.len())]
                .iter()
                .copied()
                .fold(0f64, f64::max);
            (m * f64::from(r)) as f32
        })
        .collect()
}
fn peak(scores: &[f32]) -> Option<usize> {
    if scores.len() < 5 {
        return None;
    }
    let mut best = 0f64;
    let mut selected = None;
    for i in 2..scores.len() - 2 {
        let current = scores[i];
        if f64::from(current) <= best {
            continue;
        }
        let mut j = i + 1;
        let mut accepted = true;
        while j < scores.len() && scores[j] >= current {
            if scores[j] > current {
                accepted = false;
                break;
            }
            j += 1;
        }
        if !accepted {
            continue;
        }
        let mut j = i;
        while j > 0 && scores[j - 1] >= current {
            if scores[j - 1] > current {
                accepted = false;
                break;
            }
            j -= 1;
        }
        if accepted {
            best = f64::from(current);
            selected = Some(i);
        }
    }
    selected
}
pub fn tempo(v: &[Vec<f32>]) -> Result<f64, String> {
    let n = validate_rows(v)?;
    if n != 1024 {
        return Err("exact1024correlation bins required".into());
    }
    let filtered: Vec<_> = v.iter().map(|r| filtered_scores(r)).collect();
    let mut combined = vec![0f32; filtered[0].len()];
    for (i, out) in combined.iter_mut().enumerate() {
        let mut sum = 0f64;
        for row in &filtered {
            let x = f64::from(row[i]);
            sum = x.mul_add(x, sum);
        }
        *out = sum.sqrt() as f32;
    }
    let Some(q) = peak(&combined) else {
        return Ok(128.);
    };
    let mut q = q as f64;
    let mut h = 1.;
    for _ in 0..24 {
        let mut count = 0;
        for sign in [-1f64, 1.] {
            let period = (sign.mul_add(h, q)).mul_add(0.001, BASE);
            let c = (1021. / (period * RATE)).floor() as i32 - 1;
            count = count.max(c);
        }
        let mut scores = [0f64; 2];
        for (index, sign) in [-1f64, 1.].into_iter().enumerate() {
            let step = (sign.mul_add(h, q)).mul_add(0.001, BASE) * RATE;
            let mut sum = 0f64;
            for row in v {
                let mut s = 0.;
                for k in 0..count {
                    s += read_unchecked(row, step * k as f64);
                }
                let scaled = s / 3000.;
                sum = scaled.mul_add(scaled, sum);
            }
            scores[index] = sum.sqrt();
        }
        h *= 0.5;
        q += if scores[1] > scores[0] { h } else { -h };
    }
    Ok(60. / q.mul_add(0.001, BASE))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cubic_reflects_only_start() {
        let v = [1., 2., 3., 4.];
        assert_eq!(read(&v, -1.).unwrap(), 2.);
        assert_eq!(read(&v, 0.).unwrap(), 1.);
        assert_eq!(read(&v, 0.5).unwrap(), 1.375);
        assert_eq!(read(&v, 2.).unwrap().to_bits(), 0);
        assert_eq!(read(&v, -1.01).unwrap().to_bits(), 0);
    }
    #[test]
    fn cubic_rejects_nonfinite_configuration() {
        assert!(read(&[0.; 4], f64::NAN).is_err());
        assert!(read(&[f32::NAN; 4], 0.).is_err());
    }
    #[test]
    fn quantile_keeps_signed_f32_subtraction() {
        assert_eq!(lookup(&[-33554432., -1.], 1.).unwrap(), 0.);
        assert_eq!(lookup(&[-2., 2.], 0.5).unwrap(), 0.);
    }
    #[test]
    fn short_direct_correlation_uses_unshifted_denominator() {
        let r = correlate(&[vec![1., 2., 3.]]).unwrap();
        assert_eq!(r.selected, 0);
        assert_eq!(r.values[0][0], ((14f64 / 3.).sqrt() * 50.) as f32);
        assert_eq!(r.values[0][1], ((8f64 / 3.).sqrt() * 50.) as f32);
        assert_eq!(r.values[0][2], 50.);
        assert!(r.values[0][3..].iter().all(|x| x.to_bits() == 0));
    }
    #[test]
    fn coarse_lag_reads_beyond_its_unshifted_window() {
        let mut v = vec![0.; 2050];
        v[2047] = 2.;
        v[2048] = 3.;
        let r = correlate(&[v]).unwrap();
        assert_eq!(r.selected, 0);
        assert_eq!(r.values[0][1], ((6f64 / 2048.).sqrt() * 50.) as f32);
    }
    #[test]
    fn segment_windows_overlap_and_keep_fractional_extent() {
        let w = windows(5000, 128.).unwrap();
        assert_eq!(w.ranges.len(), 5);
        assert_eq!(w.ranges[0], (0, 970));
        assert_eq!(w.ranges[1], (969, 970));
        assert_eq!(w.ranges[4], (3879, 970));
        assert_eq!(w.extent, 5000. / w.span);
    }
    #[test]
    fn silent_correlations_have_fallback_tempo() {
        assert_eq!(tempo(&vec![vec![0.; 1024]; 14]).unwrap(), 128.);
    }
}

#[derive(Debug)]
pub struct Smoothed {
    pub sustained: Vec<Vec<f32>>,
    pub transient: Vec<Vec<f32>>,
    pub x: Vec<Vec<f32>>,
    pub positive: Vec<Vec<f32>>,
}
#[derive(Debug)]
pub struct Description {
    pub tempo: f64,
    pub extent: f64,
    pub rms: f64,
    pub rejected: bool,
    pub aggregate: Vec<f64>,
    pub families: [Vec<Vec<f64>>; 4],
}
#[derive(Debug)]
pub struct Temporal {
    pub smoothed: Smoothed,
    pub descriptions: Vec<Description>,
}
pub fn smooth(a: &[Vec<f32>], b: &[Vec<f32>], weighted: &[f32]) -> Result<Smoothed, String> {
    let n = validate_rows(a)?;
    if a.len() != 14
        || b.len() != 14
        || b.iter().any(|r| r.len() != n || !valid(r))
        || weighted.len() != n
        || !valid(weighted)
        || weighted.iter().any(|x| *x < 0.)
    {
        return Err("fourteen matching band rows and weighted RMS required".into());
    }
    let mut sorted = weighted.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let gain = 1. / lookup_unchecked(&sorted, 0.95).max(0.0001);
    let mut sustained: Vec<Vec<f32>> = a
        .iter()
        .map(|r| r.iter().map(|&x| (gain * f64::from(x)) as f32).collect())
        .collect();
    let transient: Vec<Vec<f32>> = b
        .iter()
        .map(|r| r.iter().map(|&x| (gain * f64::from(x)) as f32).collect())
        .collect();
    for row in &mut sustained {
        let mut output = Vec::with_capacity(n);
        for i in 0..n {
            let width = (i + 1).min(14).min(n - i);
            let half = (width - 1) / 2;
            let mut sorted = row[i - half..i + half + 1].to_vec();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            output.push(lookup_unchecked(&sorted, 0.4) as f32);
        }
        *row = output;
    }
    let mut x = vec![vec![0f32; n]; 14];
    let mut positive = x.clone();
    for band in 0..14 {
        for i in 1..n.saturating_sub(1) {
            let row = &transient[band];
            let middle = f64::from(row[i]) * 0.5;
            let left = f64::from(row[i - 1]).mul_add(0.25, middle);
            let smoothed = f64::from(row[i + 1]).mul_add(0.25, left) as f32;
            x[band][i] = smoothed;
        }
        for i in 0..n {
            let p = if x[band][i] > 0. { x[band][i] } else { 0. };
            positive[band][i] = p;
            let product = f64::from(sustained[band][i]) * -0.2;
            let value = f64::from(p) + product;
            x[band][i] = value.max(0.) as f32;
        }
    }
    Ok(Smoothed {
        sustained,
        transient,
        x,
        positive,
    })
}
fn phase_families(
    x: &[f32],
    p: &[f32],
    a: &[f32],
    start: usize,
    bpm: f64,
    center: f64,
    weights: &[f64; 256],
) -> (f64, [Vec<f64>; 4]) {
    let quarter = (((60. / bpm) * 44100.) / 682.) * 0.25;
    let mut best_score = -1.;
    let mut best_phase = 0.;
    let mut delta = -0.5;
    for _ in 0..257 {
        let phase = center + delta;
        let mut score = 0f64;
        for (k, &w) in weights.iter().enumerate() {
            let pos = (phase + k as f64).mul_add(quarter, start as f64);
            score = read_unchecked(x, pos).mul_add(w, score);
        }
        if score > best_score {
            best_score = score;
            best_phase = phase;
        }
        delta += 1. / 256.;
    }
    let mut families: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::with_capacity(256));
    let origin = quarter * best_phase + start as f64;
    let step = quarter / (256. * 0.0078125);
    let mut pos = origin;
    for _ in 0..256 {
        for (row, family) in [x, p].into_iter().enumerate() {
            let sum = read_unchecked(family, pos) + read_unchecked(family, pos - 1.);
            let sum = sum + read_unchecked(family, pos + 1.);
            families[row].push(sum.max(0.));
        }
        pos += step;
    }
    let mut pos = origin;
    for _ in 0..256 {
        families[2].push(read_unchecked(a, pos + step * 0.5).max(0.));
        let sum = read_unchecked(a, pos) + read_unchecked(a, pos - 1.);
        let sum = sum + read_unchecked(a, pos + 1.);
        let sum = read_unchecked(a, pos - 2.).mul_add(0.5, sum);
        let sum = read_unchecked(a, pos + 2.).mul_add(0.5, sum);
        families[3].push((sum * 0.25).max(0.));
        pos += step;
    }
    (best_phase, families)
}
pub fn analyze(
    a: &[Vec<f32>],
    b: &[Vec<f32>],
    weighted: &[f32],
    raw: &[f32],
    duration: f64,
    skip_start: f64,
    skip_end: f64,
) -> Result<Temporal, String> {
    let n = validate_rows(a)?;
    if !valid(raw)
        || raw.len() != n
        || raw.iter().any(|x| *x < 0.)
        || !duration.is_finite()
        || !(0.0..=2000.).contains(&duration)
        || !skip_start.is_finite()
        || !skip_end.is_finite()
        || skip_start < 0.
        || skip_end < 0.
        || skip_start + skip_end >= 1.
    {
        return Err("finite default temporal profile required".into());
    }
    let smoothed = smooth(a, b, weighted)?;
    let correlation = correlate(&smoothed.x)?;
    let bpm = tempo(&correlation.values)?;
    let geometry = windows(n, bpm)?;
    let start_limit = ((duration * skip_start) * 44100.).trunc() as usize;
    let end_limit = ((duration * (1. - skip_end)) * 44100.).trunc() as usize;
    let weights = std::array::from_fn(|k| {
        let t = k as f64 / 255.;
        if t > 0. && t < 1. {
            (t * std::f64::consts::TAU).cos().mul_add(-0.5, 0.5)
        } else {
            0.
        }
    });
    let mut descriptions = Vec::with_capacity(geometry.ranges.len());
    for (start, used) in geometry.ranges {
        let rejected = (skip_start > 0. && start * 682 < start_limit)
            || (skip_end > 0. && (start + used) * 682 > end_limit);
        let mut d = Description {
            tempo: bpm,
            extent: geometry.extent,
            rms: 0.,
            rejected,
            aggregate: Vec::new(),
            families: std::array::from_fn(|_| Vec::new()),
        };
        if !rejected {
            let mut total = 0.;
            for &v in &raw[start..start + used] {
                total += f64::from(v * v);
            }
            d.rms = (total / used as f64).sqrt();
            let mut center = 0.5;
            for band in 0..14 {
                let mut sum = 0f64;
                for &v in &smoothed.sustained[band][start..start + used] {
                    let x = f64::from(v).max(0.);
                    sum = x.mul_add(x, sum);
                }
                let aggregate = ((sum / geometry.span).sqrt() * 50.) as f32;
                d.aggregate.push(f64::from(aggregate).max(0.));
                let (next, families) = phase_families(
                    &smoothed.x[band],
                    &smoothed.positive[band],
                    &smoothed.sustained[band],
                    start,
                    bpm,
                    center,
                    &weights,
                );
                center = next;
                for (f, row) in d.families.iter_mut().zip(families) {
                    f.push(row);
                }
            }
        }
        descriptions.push(d);
    }
    Ok(Temporal {
        smoothed,
        descriptions,
    })
}
#[cfg(test)]
mod temporal_tests {
    use super::*;
    #[test]
    fn smoothing_keeps_f64_three_tap_before_f32_narrowing() {
        let r = smooth(
            &vec![vec![10.; 4]; 14],
            &vec![vec![0., 4., 8., 0.]; 14],
            &[1.; 4],
        )
        .unwrap();
        assert_eq!(r.positive[0], vec![0., 4., 5., 0.]);
        assert_eq!(r.x[0], vec![0., 2., 3., 0.]);
    }
    #[test]
    fn baseline_uses_thirteen_values_and_forty_percent_quantile() {
        let a: Vec<f32> = (0..25).map(|i| i as f32).collect();
        let r = smooth(&vec![a; 14], &vec![vec![0.; 25]; 14], &[1.; 25]).unwrap();
        assert_eq!(r.sustained[0][12], 10.8f32);
        assert_eq!(r.sustained[0][0], 0.);
        assert_eq!(r.sustained[0][24], 24.);
    }
    #[test]
    fn silence_has_full_zero_families_and_fallback_tempo() {
        let r = analyze(
            &vec![vec![0.; 4]; 14],
            &vec![vec![0.; 4]; 14],
            &[1.; 4],
            &[0.; 4],
            4. * 682. / 44100.,
            0.,
            0.,
        )
        .unwrap();
        assert_eq!(r.descriptions.len(), 1);
        let d = &r.descriptions[0];
        assert_eq!(d.tempo, 128.);
        assert!(!d.rejected);
        assert_eq!(d.aggregate, vec![0.; 14]);
        assert!(d.families.iter().all(|f| {
            f.len() == 14
                && f.iter()
                    .all(|row| row.len() == 256 && row.iter().all(|x| x.to_bits() == 0))
        }));
    }
    #[test]
    fn skip_rejection_retains_empty_description() {
        let r = analyze(
            &vec![vec![0.; 4]; 14],
            &vec![vec![0.; 4]; 14],
            &[1.; 4],
            &[0.; 4],
            1.,
            0.25,
            0.,
        )
        .unwrap();
        assert!(r.descriptions[0].rejected);
        assert!(r.descriptions[0].aggregate.is_empty());
        assert!(r.descriptions[0].families.iter().all(Vec::is_empty));
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
    let t = analyze(&a, &b, &weighted, &raw, config[0], config[1], config[2])?;
    let mut out = b"TDO1".to_vec();
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&14u32.to_le_bytes());
    for row in t.smoothed.sustained.iter().chain(&t.smoothed.transient) {
        for v in row {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out.extend_from_slice(&(t.descriptions.len() as u32).to_le_bytes());
    for d in t.descriptions {
        for v in [d.tempo, d.extent, d.rms] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&u32::from(d.rejected).to_le_bytes());
        out.extend_from_slice(&(d.aggregate.len() as u32).to_le_bytes());
        for v in d.aggregate {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for family in d.families {
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
    Ok(out)
}
#[cfg(test)]
mod packet_tests {
    use super::*;
    #[test]
    fn numeric_packet_emits_temporal_projection() {
        let mut input = b"TDI1".to_vec();
        input.extend_from_slice(&1u32.to_le_bytes());
        for v in [1f64, 0., 0.] {
            input.extend_from_slice(&v.to_le_bytes());
        }
        for _ in 0..28 {
            input.extend_from_slice(&0f32.to_le_bytes());
        }
        input.extend_from_slice(&1f32.to_le_bytes());
        input.extend_from_slice(&0f32.to_le_bytes());
        let output = process_packet(&input).unwrap();
        assert_eq!(&output[..4], b"TDO1");
    }
    #[test]
    fn packet_rejects_missing_numeric_payload() {
        assert!(process_packet(b"TDI1").is_err());
    }
}
