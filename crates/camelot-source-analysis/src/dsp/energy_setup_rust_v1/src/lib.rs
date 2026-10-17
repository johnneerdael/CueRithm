pub struct Setup {
    pub weighting: Vec<f32>,
    pub window: Vec<f32>,
    pub bands: Vec<(f64, f64)>,
}
// Preserve the source instruction operand order of repeated products.
#[allow(clippy::assign_op_pattern)]
pub fn default_setup() -> Setup {
    let weighting = (0..=1024)
        .map(|k| {
            let f = (44100f64 * k as f64) / 2048.;
            let c = 148840000.;
            let mut numerator = f * c;
            numerator = f * numerator;
            numerator = f * numerator;
            numerator = f * numerator;
            let first = f.mul_add(f, f64::from_bits(0x407a85c28f5c28f7));
            let low = f.mul_add(f, f64::from_bits(0x40c6a7a51eb851ec));
            let high = f.mul_add(f, f64::from_bits(0x41209de0d1eb851e));
            let root = (low * high).sqrt();
            let middle = first * root;
            let top = f.mul_add(f, c);
            let denominator = top * middle;
            ((numerator / denominator) * f64::from_bits(0x3fe6a09e667f3bcd)) as f32
        })
        .collect();
    let window = (0..2048)
        .map(|k| {
            if k == 0 {
                0.
            } else {
                let fraction = k as f64 / 2048.;
                let angle = fraction * f64::from_bits(0x401921fb54442d18);
                angle.cos().mul_add(-0.5, 0.5) as f32
            }
        })
        .collect();
    let edges = [
        0., 75., 125., 180., 250., 375., 500., 800., 1200., 1800., 2500., 3700., 5000., 7000.,
        13000.,
    ];
    let bands = edges.windows(2).map(|v| (v[0], v[1])).collect();
    Setup {
        weighting,
        window,
        bands,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_source_geometry() {
        let s = default_setup();
        assert_eq!(s.weighting.len(), 1025);
        assert_eq!(s.window.len(), 2048);
        assert_eq!(s.bands.len(), 14);
    }
    #[test]
    fn weighting_is_zero_at_dc_and_positive_elsewhere() {
        let s = default_setup();
        assert_eq!(s.weighting.first().map(|x| x.to_bits()), Some(0));
        assert!(s.weighting[1..].iter().all(|x| x.is_finite() && *x > 0.));
    }
    #[test]
    fn periodic_hann_keeps_source_open_endpoint() {
        let s = default_setup();
        assert_eq!(s.window.first().map(|x| x.to_bits()), Some(0));
        assert_eq!(s.window.get(1024).copied(), Some(1.));
        assert!(s.window.last().is_some_and(|v| *v > 0.));
    }
    #[test]
    fn frequency_boundaries_are_contiguous_source_values() {
        let s = default_setup();
        assert_eq!(s.bands.first().copied(), Some((0., 75.)));
        assert_eq!(s.bands.last().copied(), Some((7000., 13000.)));
        assert!(s.bands.windows(2).all(|w| w[0].1 == w[1].0));
    }
}
