mod finite_exp2;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f32,
    pub im: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Peak {
    pub frequency: f32,
    pub amplitude: f32,
}

pub struct Tracker {
    queue: std::collections::VecDeque<Vec<Peak>>,
    widths: Vec<f32>,
}

impl Default for Tracker {
    fn default() -> Self {
        let edges = [
            0.0, 100.0, 200.0, 300.0, 400.0, 510.0, 630.0, 770.0, 920.0, 1080.0, 1270.0, 1480.0,
            1720.0, 2000.0, 2320.0, 2700.0, 3150.0, 3700.0, 4400.0, 5300.0, 6400.0, 7700.0, 9500.0,
            12000.0, 15500.0,
        ];
        let bin_hz = 44100.0_f64 / 4096.0;
        let mut widths = vec![0.0; 4096];
        for (i, pair) in edges.windows(2).enumerate() {
            let low = ((pair[0] / bin_hz + 0.5) as usize).min(4096);
            let high = if i == edges.len() - 2 {
                4096
            } else {
                ((pair[1] / bin_hz + 0.5) as usize).min(4096)
            };
            let mut width = (pair[1] - pair[0]) / bin_hz;
            if pair[0] > 440.0 {
                width /= 1.4;
            }
            widths[low..high].fill(width as f32);
        }
        Self {
            queue: Default::default(),
            widths,
        }
    }
}

fn estimate(spectrum: &[Complex], k: usize) -> f32 {
    let center = spectrum[k];
    let real_difference = spectrum[k - 1].re - spectrum[k + 1].re;
    let imaginary_difference = spectrum[k + 1].im - spectrum[k - 1].im;
    let denominator = center.re.mul_add(center.re, center.im * center.im);
    let imaginary_term = center.im * (-imaginary_difference);
    let numerator = real_difference.mul_add(center.re, imaginary_term);
    (numerator / denominator) * f32::from_bits(0x3ee66666)
}

fn correction(value: f32) -> f32 {
    let coefficient = f32::from_bits(0x3f88f5c3);
    let cubic_coefficient = f32::from_bits(0xbf2e147b);
    let bound = ((-coefficient) / cubic_coefficient).sqrt();
    let value = if value.abs() > bound {
        if value > 0.0 { bound } else { -bound }
    } else {
        value
    };
    value.mul_add(coefficient, cubic_coefficient * (value * (value * value)))
}

fn nearest(peaks: &[Peak], middle: Peak) -> Option<Peak> {
    let mut index = 0;
    let mut distance = (peaks.first()?.frequency - middle.frequency).abs();
    while index + 1 < peaks.len() {
        let next_distance = (peaks[index + 1].frequency - middle.frequency).abs();
        if next_distance < distance {
            index += 1;
            distance = next_distance;
        } else {
            break;
        }
    }
    Some(peaks[index])
}

fn pitch(frequency: f32) -> f32 {
    let bin_hz = 44100.0_f32 / 4096.0;
    let ratio = (bin_hz * frequency) / 440.0;
    let logarithm = positive_logarithm::positive_logarithm(ratio).unwrap_or(f32::NAN);
    ((logarithm / std::f32::consts::LN_2) + 4.0) * 12.0
}

impl Tracker {
    pub fn candidates(&self, spectrum: &[Complex]) -> Result<Vec<Peak>, &'static str> {
        if spectrum.len() < 513 || spectrum.len() > 4096 {
            return Err("default path needs 513..4096 complex values");
        }
        if spectrum
            .iter()
            .any(|z| !z.re.is_finite() || !z.im.is_finite())
        {
            return Err("finite spectrum required");
        }
        const USEFUL: usize = 512;
        let mut magnitudes: Vec<f32> = spectrum[..USEFUL]
            .iter()
            .map(|z| z.re.hypot(z.im))
            .collect();
        let mut accepted = Vec::new();
        let mut index = 1;
        while index < USEFUL - 1 {
            if magnitudes[index] >= magnitudes[index - 1]
                && magnitudes[index] > magnitudes[index + 1]
                && magnitudes[index - 1] > 0.0
                && magnitudes[index + 1] > 0.0
            {
                let delta = estimate(spectrum, index);
                let cube = delta * (delta * delta);
                let offset = delta.mul_add(
                    f32::from_bits(0x3f88f5c3),
                    f32::from_bits(0xbf2e147b) * cube,
                );
                let left_delta = if index == 1 {
                    offset + 1.0
                } else {
                    estimate(spectrum, index - 1)
                };
                let left_position = correction(left_delta) + 1.0;
                let a = delta * f32::from_bits(0x3ed1eb85);
                let c = cube * f32::from_bits(0xbf59999a);
                let q = a.mul_add(4.0, c);
                let negative_q = -a.mul_add(4.0, c);
                // The native oracle produced a nonfinite peak on the tiny-
                // magnitude probe. Do not silently invent a finite result.
                let exponent = finite_exp2::finite_exp2(q * negative_q)
                    .map_err(|_| "nonfinite correction outside validated source profile")?;
                let amplitude = magnitudes[index] / exponent;
                let floor = amplitude * 0.001;
                let left_tolerance = (amplitude / magnitudes[index - 1].max(floor)) * 0.6;
                let passes = if (left_position - offset).abs() < left_tolerance {
                    true
                } else {
                    let right_position = correction(estimate(spectrum, index + 1)) - 1.0;
                    let right_tolerance = (amplitude / magnitudes[index + 1].max(floor)) * 0.6;
                    (right_position - offset).abs() < right_tolerance
                };
                if passes {
                    magnitudes[index] = amplitude;
                    accepted.push((index, offset));
                    index += 2;
                    continue;
                }
            }
            index += 1;
        }
        let mut result = Vec::new();
        for (index, offset) in accepted {
            let radius = (self.widths[index] * 0.4 - 1.0) as i32;
            if radius < 1 {
                continue;
            }
            let radius = radius as usize;
            let low = index.saturating_sub(radius);
            let high = (index + radius + 1).min(USEFUL);
            let threshold = magnitudes[index] * 0.96;
            if magnitudes[low..high]
                .iter()
                .enumerate()
                .any(|(i, &amplitude)| i + low != index && amplitude > threshold)
            {
                continue;
            }
            let frequency = index as f32 + offset;
            if frequency >= 1.0 {
                result.push(Peak {
                    frequency,
                    amplitude: magnitudes[index],
                });
            }
        }
        Ok(result)
    }

    pub fn push(&mut self, spectrum: &[Complex]) -> Result<Vec<Peak>, &'static str> {
        let peaks = self.candidates(spectrum)?;
        self.queue.push_back(peaks);
        if self.queue.len() > 3 {
            self.queue.pop_front();
        }
        let mut retained = Vec::new();
        if self.queue.len() == 3 {
            for &middle in &self.queue[1] {
                let previous = nearest(&self.queue[0], middle);
                let next = nearest(&self.queue[2], middle);
                let middle_pitch = pitch(middle.frequency);
                let previous_distance =
                    (pitch(previous.map_or(-1e9, |p| p.frequency)) - middle_pitch).abs();
                let next_distance =
                    (pitch(next.map_or(-1e9, |p| p.frequency)) - middle_pitch).abs();
                // Literal FCMP/FCSEL order preserves the missing-previous NaN asymmetry.
                let distance = if next_distance < previous_distance {
                    next_distance
                } else {
                    previous_distance
                };
                if distance < 4.0 {
                    if previous_distance < 0.4 && next_distance < 0.4 {
                        let previous = previous.unwrap();
                        let next = next.unwrap();
                        retained.push(Peak {
                            frequency: middle
                                .frequency
                                .mul_add(0.4, (previous.frequency + next.frequency) * 0.3),
                            amplitude: middle
                                .amplitude
                                .mul_add(0.4, (previous.amplitude + next.amplitude) * 0.3),
                        });
                    } else {
                        retained.push(middle);
                    }
                }
            }
        }
        Ok(retained)
    }

    pub fn queue(&self) -> &std::collections::VecDeque<Vec<Peak>> {
        &self.queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(center: usize, amplitude: f32) -> Vec<Complex> {
        let mut result = vec![Complex::default(); 513];
        for (i, scale) in [(center - 1, 0.1), (center, 1.0), (center + 1, 0.1)] {
            result[i].re = amplitude * scale;
        }
        result
    }

    #[test]
    fn positive_neighbors_produce_fractional_bin_peak() {
        let peaks = Tracker::default().candidates(&tone(40, 1.0)).unwrap();
        assert_eq!(
            peaks,
            vec![Peak {
                frequency: 40.0,
                amplitude: 1.0
            }]
        );
    }

    #[test]
    fn empty_previous_rejects_middle_even_when_next_matches() {
        let mut tracker = Tracker::default();
        let empty = vec![Complex::default(); 513];
        assert!(tracker.push(&empty).unwrap().is_empty());
        assert!(tracker.push(&tone(40, 1.0)).unwrap().is_empty());
        assert!(tracker.push(&tone(40, 1.0)).unwrap().is_empty());
        assert_eq!(tracker.push(&tone(40, 1.0)).unwrap().len(), 1);
    }

    #[test]
    fn empty_next_preserves_middle_when_previous_matches() {
        let mut tracker = Tracker::default();
        tracker.push(&tone(40, 1.0)).unwrap();
        tracker.push(&tone(40, 1.0)).unwrap();
        let result = tracker.push(&vec![Complex::default(); 513]).unwrap();
        assert_eq!(
            result,
            vec![Peak {
                frequency: 40.0,
                amplitude: 1.0
            }]
        );
    }

    #[test]
    fn smoothing_retains_fused_float32_rounding() {
        let mut tracker = Tracker::default();
        for _ in 0..2 {
            assert!(tracker.push(&tone(100, 0.7)).unwrap().is_empty());
        }
        let result = tracker.push(&tone(100, 0.7)).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].frequency.to_bits(), 0x42c80001);
        assert_eq!(result[0].amplitude.to_bits(), 0x3f333333);
    }

    #[test]
    fn correction_preserves_asymmetric_peak_bits() {
        let mut spectrum = vec![Complex::default(); 513];
        for (i, re) in [(38, 0.02), (39, 0.5), (40, 1.0), (41, 0.1), (42, 0.01)] {
            spectrum[i].re = re;
        }
        let peaks = Tracker::default().candidates(&spectrum).unwrap();
        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0].frequency.to_bits(), 0x4220c129);
        assert_eq!(peaks[0].amplitude.to_bits(), 0x3f87b255);
    }

    #[test]
    fn useful_bin_boundary_excludes_511_and_512() {
        let tracker = Tracker::default();
        assert_eq!(tracker.candidates(&tone(510, 1.0)).unwrap().len(), 1);
        assert!(tracker.candidates(&tone(511, 1.0)).unwrap().is_empty());
    }

    #[test]
    fn zero_adjacent_magnitudes_reject_isolated_center() {
        let mut spectrum = vec![Complex::default(); 513];
        spectrum[40].re = 1.0;
        assert!(Tracker::default().candidates(&spectrum).unwrap().is_empty());
    }

    #[test]
    fn unsupported_nonfinite_correction_does_not_mutate_tracker_state() {
        let mut tracker = Tracker::default();
        assert!(tracker.push(&tone(40, 1e-30)).is_err());
        assert!(tracker.queue().is_empty());
    }

    #[test]
    fn tied_nearby_peaks_suppress_each_other() {
        let mut spectrum = vec![Complex::default(); 513];
        for (i, re) in [(39, 0.1), (40, 1.0), (41, 0.2), (42, 1.0), (43, 0.1)] {
            spectrum[i].re = re;
        }
        assert!(Tracker::default().candidates(&spectrum).unwrap().is_empty());
    }

    #[test]
    fn validates_spectrum_domain_before_mutating_state() {
        let mut tracker = Tracker::default();
        assert!(tracker.push(&vec![Complex::default(); 512]).is_err());
        let mut invalid = tone(40, 1.0);
        invalid[2].re = f32::NAN;
        assert!(tracker.push(&invalid).is_err());
        assert!(tracker.queue().is_empty());
    }
}
mod positive_logarithm;
