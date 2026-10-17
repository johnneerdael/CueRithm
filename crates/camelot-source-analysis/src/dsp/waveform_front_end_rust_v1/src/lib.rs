use std::io::Write;

pub const FFT_SIZE: usize = 4096;
pub const HOP: usize = 1024;
pub const ROW_VALUES: usize = 207;
pub type ComplexBins = Vec<[f32; 2]>;

#[derive(Debug, PartialEq, Eq)]
pub struct Metadata {
    pub input_frames: usize,
    pub row_count: usize,
    pub dual_fft_count: usize,
}

pub fn stereo_mean(left: f32, right: f32) -> f32 {
    ((f64::from(left) + f64::from(right)) * 0.5) as f32
}

/// Consume only normalized interleaved signed16 stereo, with no tags or references.
pub fn mono_profile(stereo: &[f32]) -> Result<Vec<f32>, String> {
    if !stereo.len().is_multiple_of(2) || stereo.len() / 2 > 64_000_000 {
        return Err("expected bounded interleaved stereo PCM".into());
    }
    for (index, &value) in stereo.iter().enumerate() {
        if !value.is_finite() || !(-1.0..1.0).contains(&value) || (value * 32768.0).fract() != 0.0 {
            return Err(format!("sample {index} is outside normalized signed16 PCM"));
        }
    }
    Ok(stereo
        .as_chunks::<2>()
        .0
        .iter()
        .map(|v| stereo_mean(v[0], v[1]))
        .collect())
}

pub fn window_pairs(input_frames: usize) -> Vec<(usize, usize)> {
    if input_frames < FFT_SIZE + HOP {
        return Vec::new();
    }
    (0..=(input_frames - FFT_SIZE - HOP) / (2 * HOP))
        .map(|index| {
            let first = index * 2 * HOP;
            (first, first + HOP)
        })
        .collect()
}

pub fn reconstruct_dual(
    spectrum: &[crate::dsp::waveform_fft_portable_v1::Complex],
) -> Result<(ComplexBins, ComplexBins), String> {
    if spectrum.len() != FFT_SIZE
        || spectrum
            .iter()
            .any(|v| !v.re.is_finite() || !v.im.is_finite())
    {
        return Err("expected4096 finite complex FFT values".into());
    }
    let mut first = vec![[0.0; 2]; 768];
    let mut second = vec![[0.0; 2]; 768];
    first[0] = [spectrum[0].re + spectrum[0].re, 0.0];
    second[0] = [0.0, spectrum[0].im * (-2.0)];
    for index in 1..768 {
        let a = spectrum[index];
        let b = spectrum[FFT_SIZE - index];
        first[index] = [a.re + b.re, a.im - b.im];
        second[index] = [b.re - a.re, (-a.im) - b.im];
    }
    if first
        .iter()
        .chain(&second)
        .flatten()
        .any(|x| !x.is_finite())
    {
        return Err("nonfinite reconstructed FFT value".into());
    }
    Ok((first, second))
}

/// Emit raw207-f32 little-endian magnitude rows; no context delay or flushing.
pub fn analyze_pcm(stereo: &[f32], output: &mut impl Write) -> Result<Metadata, String> {
    let mono = mono_profile(stereo)?;
    let windows = window_pairs(mono.len());
    let magnitude_stage = crate::dsp::waveform_magnitudes_rust_v1::DefaultMagnitudes::new();
    for &(first, second) in &windows {
        let packed: Vec<_> = (0..FFT_SIZE)
            .map(|index| crate::dsp::waveform_fft_portable_v1::Complex {
                re: mono[first + index],
                im: mono[second + index],
            })
            .collect();
        let transformed = crate::dsp::waveform_fft_portable_v1::fft4096(&packed)
            .map_err(|e| format!("FFT profile error: {e:?}"))?;
        let (first_bins, second_bins) = reconstruct_dual(&transformed)?;
        for bins in [first_bins, second_bins] {
            let row = magnitude_stage
                .process(&bins)
                .map_err(|e| format!("magnitude profile error: {e:?}"))?;
            if row.len() != ROW_VALUES {
                return Err("magnitude row shape mismatch".into());
            }
            let mut bytes = [0u8; ROW_VALUES * 4];
            for (value, chunk) in row.iter().zip(bytes.as_chunks_mut::<4>().0) {
                *chunk = value.to_le_bytes();
            }
            output.write_all(&bytes).map_err(|e| e.to_string())?;
        }
    }
    Ok(Metadata {
        input_frames: mono.len(),
        row_count: windows.len() * 2,
        dual_fft_count: windows.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mean_is_not_channel_sum() {
        assert_eq!(stereo_mean(-0.5, 0.25), -0.125);
    }

    #[test]
    fn mean_promotes_before_addition() {
        assert_eq!(
            stereo_mean(f32::MAX, f32::MAX).to_bits(),
            f32::MAX.to_bits()
        );
        assert_eq!(stereo_mean(f32::from_bits(1), 0.0).to_bits(), 0);
    }

    #[test]
    fn normalized_s16_mean_has_half_step_precision() {
        let mono = mono_profile(&[-1.0, 32767.0 / 32768.0]).unwrap();
        assert_eq!(mono, [-1.0 / 65536.0]);
    }

    #[test]
    fn pcm_requires_stereo_shape_and_normalized_s16_domain() {
        assert!(mono_profile(&[0.0]).is_err());
        for value in [f32::NAN, f32::INFINITY, 1.0, -1.1, 1.0 / 65536.0] {
            assert!(mono_profile(&[value, 0.0]).is_err());
        }
    }

    #[test]
    fn complete_pair_boundaries_and_no_single_tail() {
        for frames in [0, 4096, 5119] {
            assert!(window_pairs(frames).is_empty());
        }
        for frames in [5120, 6144, 7167] {
            assert_eq!(window_pairs(frames), [(0, 1024)]);
        }
        assert_eq!(window_pairs(7168), [(0, 1024), (2048, 3072)]);
    }

    #[test]
    fn dual_split_keeps_dc_scale_rotation_and_bin767() {
        let mut z = vec![crate::dsp::waveform_fft_portable_v1::Complex::default(); FFT_SIZE];
        z[0] = crate::dsp::waveform_fft_portable_v1::Complex { re: 13.0, im: 17.0 };
        for k in [1, 767] {
            z[k] = crate::dsp::waveform_fft_portable_v1::Complex { re: 3.0, im: 5.0 };
            z[FFT_SIZE - k] = crate::dsp::waveform_fft_portable_v1::Complex { re: 7.0, im: 11.0 };
        }
        let (a, b) = reconstruct_dual(&z).unwrap();
        assert_eq!(a.len(), 768);
        assert_eq!(b.len(), 768);
        assert_eq!(a[0], [26.0, 0.0]);
        assert_eq!(b[0], [0.0, -34.0]);
        for k in [1, 767] {
            assert_eq!(a[k], [10.0, -6.0]);
            assert_eq!(b[k], [4.0, -16.0]);
        }
    }

    #[test]
    fn dual_second_dc_retains_negative_zero() {
        let (_, b) = reconstruct_dual(&vec![
            crate::dsp::waveform_fft_portable_v1::Complex::default(
            );
            FFT_SIZE
        ])
        .unwrap();
        assert_eq!(b[0][1].to_bits(), 0x80000000);
    }

    #[test]
    fn malformed_spectrum_rejected() {
        assert!(reconstruct_dual(&[]).is_err());
    }

    #[test]
    fn zero_pcm_emits_all_scheduled_rows_without_context_delay() {
        let mut bytes = Vec::new();
        let result = analyze_pcm(&vec![0.0; 7168 * 2], &mut bytes).unwrap();
        assert_eq!(
            result,
            Metadata {
                input_frames: 7168,
                row_count: 4,
                dual_fft_count: 2
            }
        );
        assert_eq!(bytes, vec![0; 4 * ROW_VALUES * 4]);
    }

    #[test]
    fn emits_first_window_then_second_window() {
        let mut stereo = vec![0.0; 5120 * 2];
        stereo[4096 * 2] = 0.5;
        stereo[4096 * 2 + 1] = 0.5;
        let mut bytes = Vec::new();
        let result = analyze_pcm(&stereo, &mut bytes).unwrap();
        assert_eq!(result.row_count, 2);
        let rowmax = |row: usize| {
            bytes[row * 828..(row + 1) * 828]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| f32::from_le_bytes(*b))
                .fold(0.0, f32::max)
        };
        assert!(rowmax(1) > rowmax(0));
    }
}
