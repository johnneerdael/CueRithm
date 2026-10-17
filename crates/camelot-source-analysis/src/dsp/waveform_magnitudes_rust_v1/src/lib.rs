//! Private source-derived default Main spectral-window/magnitude/rebin stage.

pub const INPUT_BINS: usize = 768;
pub const WINDOWED_BINS: usize = 766;
pub const OUTPUT_BINS: usize = 207;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    WrongBinCount { actual: usize },
    NonFiniteInput { bin: usize, component: usize },
    NonFiniteIntermediate { stage: &'static str, index: usize },
}

pub struct DefaultMagnitudes {
    weights: [f32; WINDOWED_BINS],
}

impl DefaultMagnitudes {
    pub fn new() -> Self {
        Self {
            weights: std::array::from_fn(aweight),
        }
    }

    pub fn weights(&self) -> &[f32; WINDOWED_BINS] {
        &self.weights
    }

    /// Apply the default frequency-domain window and A weighting.
    ///
    /// Require exactly768 finite complex f32 bins and finite derived magnitudes.
    /// Result index0 is explicitly zero; indices1..765 use neighbor bins0..766.
    pub fn windowed_magnitudes(&self, bins: &[[f32; 2]]) -> Result<Vec<f32>, Error> {
        if bins.len() != INPUT_BINS {
            return Err(Error::WrongBinCount { actual: bins.len() });
        }
        for (bin, pair) in bins.iter().enumerate() {
            for (component, value) in pair.iter().enumerate() {
                if !value.is_finite() {
                    return Err(Error::NonFiniteInput { bin, component });
                }
            }
        }
        let mut magnitudes = vec![0.0; WINDOWED_BINS];
        for index in 1..WINDOWED_BINS {
            let real = window_component(bins[index - 1][0], bins[index][0], bins[index + 1][0]);
            let imaginary =
                window_component(bins[index - 1][1], bins[index][1], bins[index + 1][1]);
            let imaginary_square = imaginary * imaginary;
            let norm_square = real.mul_add(real, imaginary_square);
            let magnitude = self.weights[index] * norm_square.sqrt();
            if !magnitude.is_finite() {
                return Err(Error::NonFiniteIntermediate {
                    stage: "magnitude",
                    index,
                });
            }
            magnitudes[index] = magnitude;
        }
        Ok(magnitudes)
    }

    /// Produce207 scaled f32 magnitudes for the fixed44100/4096/tilt0/scale1 profile.
    pub fn process(&self, bins: &[[f32; 2]]) -> Result<Vec<f32>, Error> {
        let mut magnitudes = self.windowed_magnitudes(bins)?;
        rebin(&mut magnitudes)?;
        for (index, magnitude) in magnitudes.iter_mut().enumerate() {
            *magnitude *= 49152.0;
            if !magnitude.is_finite() {
                return Err(Error::NonFiniteIntermediate {
                    stage: "scale",
                    index,
                });
            }
        }
        Ok(magnitudes)
    }
}

impl Default for DefaultMagnitudes {
    fn default() -> Self {
        Self::new()
    }
}

fn aweight(bin: usize) -> f32 {
    let frequency = (44100.0 * bin as f64) / 4096.0;
    let c0 = f64::from_bits(0x41a1be3c80000000);
    let c1 = f64::from_bits(0x407a85c28f5c28f7);
    let c2 = f64::from_bits(0x40c6a7a51eb851ec);
    let c3 = f64::from_bits(0x41209de0d1eb851e);
    let numerator = frequency * c0;
    let numerator = frequency * numerator;
    let numerator = frequency * numerator;
    let numerator = frequency * numerator;
    let first = frequency.mul_add(frequency, c1);
    let second = frequency.mul_add(frequency, c2);
    let third = frequency.mul_add(frequency, c3);
    let product = second * third;
    let denominator = first * product.sqrt();
    let fourth = frequency.mul_add(frequency, c0);
    let denominator = fourth * denominator;
    // Main narrows here, then promotes and multiplies pow(max(f/50,1),-tilt).
    // Fixed tilt0 makes that factor exactly1, so the second narrowing is identical.
    (numerator / denominator) as f32
}

fn window_component(left: f32, center: f32, right: f32) -> f32 {
    let neighbors = right + left;
    neighbors.mul_add(-0.5, center)
}

fn rebin(values: &mut Vec<f32>) -> Result<(), Error> {
    let mut threshold = 80;
    while threshold < values.len() {
        let length = values.len();
        let mut read = threshold;
        let mut write = threshold;
        while read + 1 < length {
            let product = values[read] * values[read + 1];
            let magnitude = product.sqrt();
            if !magnitude.is_finite() {
                return Err(Error::NonFiniteIntermediate {
                    stage: "rebin",
                    index: read,
                });
            }
            values[write] = magnitude;
            read += 2;
            write += 1;
        }
        values.truncate(write);
        threshold += 40;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_spectrum_has_fixed_zero_output() {
        let result = DefaultMagnitudes::new()
            .process(&[[0.0; 2]; INPUT_BINS])
            .unwrap();
        assert_eq!(result.len(), OUTPUT_BINS);
        assert!(result.iter().all(|x| x.to_bits() == 0));
    }

    #[test]
    fn constant_spectrum_cancels_in_window() {
        let result = DefaultMagnitudes::new()
            .process(&[[2.0, -4.0]; INPUT_BINS])
            .unwrap();
        assert!(result.iter().all(|x| x.to_bits() == 0));
    }

    #[test]
    fn source_derived_aweight_bits() {
        let stage = DefaultMagnitudes::new();
        for (index, expected) in [
            (0, 0),
            (1, 0x39a33f61),
            (10, 0x3dc9a72c),
            (100, 0x3f506aaf),
            (765, 0x3f2f21ed),
        ] {
            assert_eq!(stage.weights()[index].to_bits(), expected);
        }
    }

    #[test]
    fn sparse_low_peak_preserves_weight_and_scale() {
        let mut bins = [[0.0; 2]; INPUT_BINS];
        bins[10][0] = 1.0;
        let result = DefaultMagnitudes::new().process(&bins).unwrap();
        assert_eq!(result[10].to_bits(), 0x45973d61);
        assert!(result[9] > 0.0 && result[11] > 0.0);
        assert_eq!(result[8], 0.0);
        assert_eq!(result[12], 0.0);
    }

    #[test]
    fn dc_output_zero_but_dc_is_neighbor_input() {
        let mut bins = [[0.0; 2]; INPUT_BINS];
        bins[0][0] = 2.0;
        let result = DefaultMagnitudes::new().windowed_magnitudes(&bins).unwrap();
        assert_eq!(result[0].to_bits(), 0);
        assert_eq!(result[1].to_bits(), 0x39a33f61);
    }

    #[test]
    fn last_used_neighbor_is_bin766() {
        let mut bins = [[0.0; 2]; INPUT_BINS];
        bins[766][0] = 1.0;
        let result = DefaultMagnitudes::new().windowed_magnitudes(&bins).unwrap();
        assert_eq!(result.len(), WINDOWED_BINS);
        assert_eq!(result[765].to_bits(), 0x3eaf21ed);
    }

    #[test]
    fn supplied_bin767_has_no_numeric_effect() {
        let mut bins = [[0.0; 2]; INPUT_BINS];
        bins[767] = [f32::MAX, -f32::MAX];
        assert_eq!(
            DefaultMagnitudes::new().process(&bins).unwrap(),
            vec![0.0; OUTPUT_BINS]
        );
    }

    #[test]
    fn window_retains_fused_subnormal_rounding() {
        let tiny = f32::from_bits(1);
        assert_eq!(window_component(0.0, tiny, tiny).to_bits(), 0);
    }

    #[test]
    fn rebin_boundary_and_geometric_pair() {
        let mut values = vec![1.0; WINDOWED_BINS];
        values[79] = 4.0;
        values[80] = 4.0;
        values[81] = 9.0;
        rebin(&mut values).unwrap();
        assert_eq!(values.len(), OUTPUT_BINS);
        assert_eq!(values[79], 4.0);
        assert_eq!(values[80], 6.0);
        assert_eq!(values[81], 1.0);
    }

    #[test]
    fn rebin_drops_unpaired_tail_at_next_level() {
        let mut values = vec![1.0; WINDOWED_BINS];
        values[764] = 4.0;
        values[765] = 9.0;
        rebin(&mut values).unwrap();
        assert_eq!(values, vec![1.0; OUTPUT_BINS]);
    }

    #[test]
    fn rebin_underflow_happens_before_square_root() {
        let mut values = vec![1.0; WINDOWED_BINS];
        values[80] = f32::MIN_POSITIVE;
        values[81] = f32::MIN_POSITIVE;
        rebin(&mut values).unwrap();
        assert_eq!(values[80].to_bits(), 0);
    }

    #[test]
    fn rejects_wrong_fixed_profile_count() {
        let stage = DefaultMagnitudes::new();
        for count in [0, 767, 769] {
            assert_eq!(
                stage.process(&vec![[0.0; 2]; count]),
                Err(Error::WrongBinCount { actual: count })
            );
        }
    }

    #[test]
    fn rejects_nonfinite_numeric_input() {
        let mut bins = [[0.0; 2]; INPUT_BINS];
        bins[12][1] = f32::NAN;
        assert_eq!(
            DefaultMagnitudes::new().process(&bins),
            Err(Error::NonFiniteInput {
                bin: 12,
                component: 1
            })
        );
    }

    #[test]
    fn rejects_finite_input_with_nonfinite_derived_magnitude() {
        let mut bins = [[0.0; 2]; INPUT_BINS];
        bins[10][0] = f32::MAX;
        assert!(matches!(
            DefaultMagnitudes::new().process(&bins),
            Err(Error::NonFiniteIntermediate { .. })
        ));
    }
}
