use super::*;

impl Context<'_> {
    pub(super) fn positive(&self, candidate: f64, phrases: usize) -> Option<Walkback> {
        let beats = phrases as f64 * 16.;
        if let Some(peak) = self.first_peak((candidate - 0.1).max(0.), candidate + 0.1, true) {
            return Some(result(peak, beats));
        }
        if self.strong() {
            let ratios: Vec<f64> = (0..=3)
                .map(|beat| {
                    self.around(candidate + beat as f64 * self.period, 0.03, 0.005)
                        .ratio
                })
                .collect();
            let minimum = ratios.iter().copied().fold(f64::INFINITY, f64::min);
            let maximum = ratios.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if !(maximum > 2. || variation(&ratios) < 0.2 && minimum > 1.3) {
                return None;
            }
            if self.rms(candidate, 0.03, false) > 0.001 || candidate < 1. {
                if candidate > 0.1 && candidate < 8. * self.period {
                    let (position, walked) = self.single_steps(candidate);
                    return Some(result(position, beats + walked));
                }
                return Some(result(candidate, beats));
            }
            return None;
        }
        let found = self.scan(
            (candidate - 0.1).max(0.),
            candidate + 0.1,
            0.005,
            true,
            Peak::at(candidate),
        );
        if found.ratio > 1.3 || self.rms(candidate, 0.03, false) > 0.005 {
            let selected = if found.ratio > 1.3 {
                found.at
            } else {
                candidate
            };
            if selected > 0.1 && selected < 8. * self.period {
                let (position, walked) = self.single_steps(selected);
                return Some(result(position, beats + walked));
            }
            if selected > 3.
                && let Some(found) = self.silence_fallback(selected)
            {
                return Some(found);
            }
            return Some(result(selected, beats));
        }
        None
    }
}
