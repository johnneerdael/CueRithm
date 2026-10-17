#[rustfmt::skip]
#[allow(dead_code,clippy::chunks_exact_to_as_chunks,clippy::manual_is_multiple_of)]
#[path="../frozen-fft/fft.rs"]
mod fft;
pub struct Frame {
    pub raw_rms: f32,
    pub weighted_rms: f32,
    pub spectrum: Vec<f32>,
}
pub fn analyze(pcm: &[f32]) -> Result<Frame, String> {
    if pcm.len() != 4096 || pcm.iter().any(|v| !v.is_finite() || v.abs() > 1.) {
        return Err("2048 finite normalized stereo frames required".into());
    }
    let setup = crate::dsp::energy_setup_rust_v1::default_setup();
    let mut sum = 0f64;
    for sample in pcm[..1364].as_chunks::<2>().0 {
        sum += f64::from(sample[0] * sample[0]);
        sum += f64::from(sample[1] * sample[1]);
    }
    let raw_rms = (sum / 1364.).sqrt() as f32;
    let mut work: Vec<fft::Complex> = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .zip(setup.window)
        .map(|(v, w)| fft::Complex {
            re: v[0] * w,
            im: v[1] * w,
        })
        .collect();
    fft::fft(&mut work);
    let first = work[0];
    let q = first.re.mul_add(first.re, first.im * first.im);
    let q = f64::from(q);
    let dc = ((q + q).sqrt() * f64::from(setup.weighting[0])) as f32;
    let mut spectrum = Vec::with_capacity(1025);
    spectrum.push(dc);
    let mut weighted_sum = f64::from(dc * dc) + 0.;
    for k in 1..=1024 {
        let a = work[k];
        let b = work[2048 - k];
        let q = a.re.mul_add(a.re, a.im * a.im);
        let q = b.re.mul_add(b.re, q);
        let q = b.im.mul_add(b.im, q);
        let value = setup.weighting[k] * q.sqrt();
        spectrum.push(value);
        weighted_sum += f64::from(value * value);
    }
    let weighted_rms = (weighted_sum / 1025.).sqrt() as f32;
    if !raw_rms.is_finite() || !weighted_rms.is_finite() || spectrum.iter().any(|v| !v.is_finite())
    {
        return Err("nonfinite energy frame intermediate".into());
    }
    Ok(Frame {
        raw_rms,
        weighted_rms,
        spectrum,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zero_frame_produces_initialized_zeros() {
        let f = analyze(&vec![0.; 4096]).unwrap();
        assert_eq!(f.spectrum.len(), 1025);
        assert!(f.spectrum.iter().all(|x| x.to_bits() == 0));
        assert_eq!(f.raw_rms.to_bits(), 0);
        assert_eq!(f.weighted_rms.to_bits(), 0);
    }
    #[test]
    fn raw_rms_uses_first_hop_only_and_separate_channels() {
        let mut pcm = vec![0.; 4096];
        pcm[800] = 0.5;
        pcm[801] = -0.25;
        let f = analyze(&pcm).unwrap();
        assert_eq!(
            f.raw_rms.to_bits(),
            ((0.3125f64 / 1364.).sqrt() as f32).to_bits()
        );
        pcm[800] = 0.;
        pcm[801] = 0.;
        pcm[2000] = 0.5;
        let f = analyze(&pcm).unwrap();
        assert_eq!(f.raw_rms.to_bits(), 0);
        assert!(f.weighted_rms > 0.);
    }
    #[test]
    fn antiphase_stereo_does_not_cancel_energy() {
        let mut pcm = vec![0.; 4096];
        for f in pcm.as_chunks_mut::<2>().0.iter_mut() {
            *f = [0.5, -0.5];
        }
        let f = analyze(&pcm).unwrap();
        assert_eq!(f.raw_rms, 0.5);
        assert!(f.weighted_rms > 0.);
    }
    #[test]
    fn rejects_wrong_frame_or_nonfinite_samples() {
        assert!(analyze(&[0.; 4]).is_err());
        let mut p = vec![0.; 4096];
        p[4] = f32::NAN;
        assert!(analyze(&p).is_err());
        p[4] = 1.1;
        assert!(analyze(&p).is_err());
    }
}
