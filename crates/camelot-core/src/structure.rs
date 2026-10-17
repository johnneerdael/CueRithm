//! PCM-only time-local features for experimental cue/energy learning.
//! Times denote window centres. These features do not select or name cues.
use crate::spectral::Spectrum;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureFrame {
    pub time_seconds: f64,
    /// Unweighted RMS over available samples in the centred window; floor -120 dBFS.
    pub rms_db: f64,
    /// Power fractions in [0,100), [100,250), [250,500), [500,1000),
    /// [1000,2500), [2500,Nyquist] Hz. Silence has six zero values.
    pub band_energy_fraction: [f64; 6],
    /// Positive change in L1-normalized magnitude spectrum, in 0..=1.
    pub spectral_flux: f64,
    /// Power-weighted spectral centroid in Hz; zero for silence.
    pub centroid_hz: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureFeatures {
    pub schema: String,
    pub sample_rate: u32,
    pub frame_size: usize,
    pub hop_samples: usize,
    pub duration_seconds: f64,
    pub frames: Vec<StructureFrame>,
}

pub fn extract(samples: &[f32], sample_rate: u32) -> Result<StructureFeatures> {
    ensure!(
        (8000..=192000).contains(&sample_rate),
        "Invalid structure sample rate"
    );
    ensure!(
        samples.iter().all(|x| x.is_finite()),
        "Non-finite structure PCM input"
    );
    let size = (sample_rate as usize * 18 / 100).next_power_of_two();
    let hop = sample_rate as usize / 8;
    let mut fft = Spectrum::new(size);
    let mut window = vec![0.0; size];
    let mut magnitudes = vec![0.0; size / 2 + 1];
    let mut previous = vec![0.0; magnitudes.len()];
    let mut frames = Vec::new();
    for center in (0..samples.len()).step_by(hop) {
        window.fill(0.0);
        let start = center.saturating_sub(size / 2);
        let end = (center + size / 2).min(samples.len());
        let padding = (size / 2).saturating_sub(center);
        window[padding..padding + end - start].copy_from_slice(&samples[start..end]);
        let rms = (samples[start..end]
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            / (end - start) as f64)
            .sqrt();
        let mut bands = [0.0; 6];
        let mut centroid = 0.0;
        let mut total_power = 0.0;
        let mut flux = 0.0;
        if rms > 1e-6 {
            // The spectral features are scale-invariant. Bound FFT input even
            // for finite, out-of-range float PCM; retain the original RMS.
            let peak = window.iter().map(|x| x.abs()).fold(1.0_f32, f32::max);
            if peak > 1.0 {
                for sample in &mut window {
                    *sample /= peak;
                }
            }
            fft.magnitudes(&window, &mut magnitudes);
            let magnitude_sum = magnitudes.iter().sum::<f64>();
            for (bin, magnitude) in magnitudes.iter().enumerate() {
                let hz = bin as f64 * sample_rate as f64 / size as f64;
                let band =
                    [100.0, 250.0, 500.0, 1000.0, 2500.0].partition_point(|edge| hz >= *edge);
                let power = magnitude * magnitude;
                bands[band] += power;
                total_power += power;
                centroid += hz * power;
                let normalized = magnitude / magnitude_sum.max(f64::MIN_POSITIVE);
                if !frames.is_empty() {
                    flux += (normalized - previous[bin]).max(0.0);
                }
                previous[bin] = normalized;
            }
            if total_power > 0.0 {
                for band in &mut bands {
                    *band /= total_power;
                }
                centroid /= total_power;
            }
        } else {
            previous.fill(0.0);
        }
        frames.push(StructureFrame {
            time_seconds: center as f64 / sample_rate as f64,
            rms_db: 20.0 * rms.max(1e-6).log10(),
            band_energy_fraction: bands,
            spectral_flux: flux.clamp(0.0, 1.0),
            centroid_hz: centroid,
        });
    }
    Ok(StructureFeatures {
        schema: "centered-rms-six-band-flux-v1".into(),
        sample_rate,
        frame_size: size,
        hop_samples: hop,
        duration_seconds: samples.len() as f64 / sample_rate as f64,
        frames,
    })
}
