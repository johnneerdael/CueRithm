//! Local whole-track acoustic energy. See docs/research/local-track-energy.md.
//! Accepts mono 22050 Hz PCM only; metadata never enters this calculation.
use anyhow::{Result, ensure};
use rustfft::{FftPlanner, num_complex::Complex};

pub const METHOD: &str = "lexicon-acoustic-energy-v1";
const FRAME: usize = 2048;
const HOP: usize = 1024;

#[derive(Debug)]
pub struct EnergyMeasurement {
    pub level: u8,
    pub crest_db: f64,
    pub flatness: f64,
    pub high_mid_ratio: f64,
    pub sub_ratio: f64,
}

/// A fixed acoustic formula, without an external model or network dependency.
/// Too-short input and digital silence have no meaningful energy rating.
pub fn analyze(samples: &[f32], sample_rate: u32) -> Result<Option<EnergyMeasurement>> {
    ensure!(
        sample_rate == 22050,
        "Energy requires direct 22050 Hz mono PCM"
    );
    ensure!(
        samples.iter().all(|s| s.is_finite()),
        "Nonfinite energy PCM"
    );
    let peak = samples
        .iter()
        .map(|s| f64::from(*s).abs())
        .fold(0., f64::max);
    if samples.len() < FRAME || peak <= 1e-10 {
        return Ok(None);
    }
    // Float32 window and per-frame statistics match the source helper boundary;
    // FFT and reductions use Float64. No loudness normalization is applied.
    let window: Vec<f32> = (0..FRAME)
        .map(|i| {
            (0.5 - 0.5 * (2. * std::f64::consts::PI * i as f64 / (FRAME - 1) as f64).cos()) as f32
        })
        .collect();
    let fft = FftPlanner::<f64>::new().plan_fft_forward(FRAME);
    let mut buffer = vec![Complex::default(); FRAME];
    let mut scratch = vec![Complex::default(); fft.get_inplace_scratch_len()];
    let mut frames = Vec::with_capacity((samples.len() - FRAME) / HOP + 1);
    for start in (0..=samples.len() - FRAME).step_by(HOP) {
        let mut squares = 0.;
        for i in 0..FRAME {
            let value = f64::from(samples[start + i]);
            squares += value * value;
            buffer[i] = Complex::new(f64::from((value * f64::from(window[i])) as f32), 0.);
        }
        fft.process_with_scratch(&mut buffer, &mut scratch);
        let (mut total, mut logs, mut nonzero, mut sub, mut high_mid) = (0., 0., 0, 0., 0.);
        for (bin, value) in buffer.iter().enumerate().take(FRAME / 2).skip(1) {
            let magnitude = value.norm() * (2. / FRAME as f64);
            let power = magnitude * magnitude;
            let frequency = bin as f64 * sample_rate as f64 / FRAME as f64;
            total += power;
            if magnitude > 1e-9 {
                logs += (power + 1e-12).ln();
                nonzero += 1;
            }
            if (20. ..60.).contains(&frequency) {
                sub += power;
            } else if (2000. ..5000.).contains(&frequency) {
                high_mid += power;
            }
        }
        let arithmetic = total / (FRAME / 2 - 1) as f64;
        let geometric = if nonzero > 0 {
            (logs / f64::from(nonzero)).exp()
        } else {
            0.
        };
        frames.push([
            (squares / FRAME as f64).sqrt() as f32,
            if arithmetic > 0. {
                (geometric / arithmetic).min(1.) as f32
            } else {
                0.
            },
            if total > 0. {
                (high_mid / total) as f32
            } else {
                0.
            },
            if total > 0. { (sub / total) as f32 } else { 0. },
        ]);
    }
    let threshold = frames.iter().map(|f| f64::from(f[0])).fold(0., f64::max) * 0.12;
    let mut sums = [0.; 4];
    let mut count = 0;
    for frame in frames.iter().filter(|f| f64::from(f[0]) >= threshold) {
        count += 1;
        sums[0] += f64::from(frame[0]).powi(2);
        for i in 1..4 {
            sums[i] += f64::from(frame[i]);
        }
    }
    let rms = (sums[0] / f64::from(count)).sqrt();
    let crest_db = 20. * peak.max(1e-10).log10() - 20. * rms.max(1e-10).log10();
    let flatness = sums[1] / f64::from(count);
    let high_mid_ratio = sums[2] / f64::from(count);
    let sub_ratio = sums[3] / f64::from(count);
    let score = 9.0124 - 0.49685 * crest_db
        + 21.436 * flatness
        + 10.183 * high_mid_ratio
        + 2.9802 * sub_ratio;
    Ok(Some(EnergyMeasurement {
        level: (score + 0.5).floor().clamp(1., 10.) as u8,
        crest_db,
        flatness,
        high_mid_ratio,
        sub_ratio,
    }))
}
