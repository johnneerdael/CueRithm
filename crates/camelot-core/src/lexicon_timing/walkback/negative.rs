use super::*;

impl Context<'_> {
    pub(super) fn negative(&self, candidate: f64, phrases: usize) -> Option<Walkback> {
        if self.strong() && candidate > -16. * self.period {
            self.negative_strong(candidate, phrases)
        } else {
            self.negative_weak(candidate, phrases)
        }
    }
    fn negative_strong(&self, candidate: f64, phrases: usize) -> Option<Walkback> {
        let beats = phrases as f64 * 16.;
        if let Some(peak) = self.first_peak(0., 0.015, false) {
            return Some(result(peak, beats));
        }
        let sparse = self.peaks.first().is_some_and(|p| *p > 10.);
        let relaxed = sparse || candidate.abs() > self.period;
        let threshold = if relaxed { 1.5 } else { 2.5 };
        let consistent_threshold = if relaxed { 1.2 } else { 1.5 };
        let limit = self.period.min(0.5);
        let best = self.scan(0., limit, 0.002, true, Peak::at(0.));
        if best.at > 0.8 * self.period {
            if let Some(peak) = self.first_peak(0., 0.15 * self.period, false)
                && self
                    .following(peak)
                    .iter()
                    .filter(|r| **r > consistent_threshold)
                    .count()
                    >= 2
            {
                return Some(result(peak, beats));
            }
            let early = self.scan(0., 0.05, 0.002, true, Peak::at(-1.));
            if early.at >= 0.
                && early.ratio > threshold
                && self
                    .following(early.at)
                    .iter()
                    .filter(|r| **r > consistent_threshold)
                    .count()
                    >= 2
            {
                return Some(result(early.at, beats));
            }
        }
        if best.ratio > threshold {
            let ratios = self.following(best.at);
            let consistent = ratios.iter().filter(|r| **r > consistent_threshold).count();
            if consistent >= 2 {
                if self
                    .strength
                    .is_some_and(|s| s > 100. && best.ratio < 0.01 * s)
                {
                    return self.peaks.first().map(|p| result(*p, 0.));
                }
                let maximum = ratios.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let minimum = ratios.iter().copied().fold(f64::INFINITY, f64::min);
                if maximum > 5. * minimum && minimum < 2. && maximum > 5. {
                    let &first = self.peaks.first()?;
                    let phase = first - (first / self.period).floor() * self.period;
                    let mut aligned = self.around(phase, 0.03, 0.002);
                    if aligned.at < self.period {
                        let energy = self.rms(aligned.at + 0.1, 0.03, false).max(self.rms(
                            aligned.at + 0.2,
                            0.03,
                            false,
                        ));
                        if energy < 0.005 && aligned.at + self.period < first {
                            aligned = self.around(aligned.at + self.period, 0.03, 0.002);
                        }
                    }
                    return Some(result(aligned.at, beats));
                }
                let average = mean(&ratios);
                if best.ratio > 3. * average
                    && average < 3.
                    && best.at > 0.5 * self.period
                    && self
                        .peaks
                        .first()
                        .is_some_and(|p| best.at < *p - 0.5 * self.period)
                {
                    if maximum > 2.5 {
                        let index = ratios.iter().position(|r| *r == maximum).unwrap();
                        let at = best.at + (index + 1) as f64 * self.period;
                        return Some(result(self.around(at, 0.03, 0.002).at, beats));
                    }
                    return Some(result(self.peaks[0], beats));
                }
                return Some(result(best.at, beats));
            }
            if consistent > 0 && best.ratio > 5. {
                if !(self.peaks.first().is_some_and(|p| *p > 1.) && consistent < 2) {
                    return Some(result(best.at, beats));
                }
                if let Some(peak) = self.first_peak(0., 8. * self.period, false) {
                    return Some(result(peak, beats));
                }
            }
        } else if relaxed
            && best.ratio
                > if self.kind == IntroKind::Buildup {
                    1.5
                } else {
                    1.
                }
        {
            if let Some(found) = self.relaxed_earliest(limit, consistent_threshold, beats) {
                return Some(found);
            }
            // This branch always returns an explicit early-peak or transient
            // fallback after its more selective consistency checks.
            return Some(result(
                self.first_peak(0., 8. * self.period, false)
                    .unwrap_or(best.at),
                beats,
            ));
        }
        let early_limit = if self.kind == IntroKind::Buildup {
            8.
        } else {
            2.
        };
        if let Some(peak) = self.first_peak(0., early_limit * self.period, false) {
            return Some(self.negative_peak_boundary(peak, beats));
        }
        let next = self.drop - (phrases - 1) as f64 * (16. * self.period);
        if next > 0. && next < 16. * self.period {
            return self.overshoot_forward(next, beats);
        }
        None
    }
    fn relaxed_earliest(&self, limit: f64, threshold: f64, beats: f64) -> Option<Walkback> {
        let mut earliest = None;
        let mut at = 0.;
        while at <= limit {
            let ratio = attack_ratio(self.pcm, self.rate, at, 15.);
            if ratio > 1. {
                earliest = Some(self.offsets(at, -0.01, 0.01, 0.002, Peak { at, ratio }));
                break;
            }
            at += 0.002;
        }
        let earliest = earliest?;
        let ratios = self.following(earliest.at);
        let consistent = ratios.iter().filter(|r| **r > threshold).count();
        if consistent >= 2 {
            if earliest.at < 0.1
                && self
                    .peaks
                    .first()
                    .is_some_and(|p| *p > earliest.at + self.period)
            {
                let first = self.peaks[0];
                let middle = (earliest.at + first) / 2.;
                let mut maximum = 0_f64;
                for time in [earliest.at + 0.1, earliest.at + 0.2, middle] {
                    if time < first {
                        maximum = maximum.max(self.rms(time, 0.03, false));
                    }
                }
                if maximum < 0.005 {
                    return Some(result(first, beats));
                }
            }
            let maximum = ratios.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if maximum > 5. * earliest.ratio
                && earliest.ratio < 2.
                && maximum > 5.
                && let Some(&first) = self.peaks.first()
            {
                let phase = first - (first / self.period).floor() * self.period;
                let aligned = self.around(phase, 0.03, 0.002);
                if aligned.ratio < 1.5 && aligned.at + self.period < first {
                    let next = self.around(aligned.at + self.period, 0.03, 0.002);
                    if next.ratio > 2. * aligned.ratio && next.ratio > 1.5 {
                        return Some(result(next.at, beats));
                    }
                }
                return Some(result(aligned.at, beats));
            }
            return Some(result(earliest.at, beats));
        }
        self.first_peak(0., 8. * self.period, false)
            .map(|peak| result(peak, beats))
    }
    fn negative_peak_boundary(&self, peak: f64, beats: f64) -> Walkback {
        if peak > 1.5 * self.period {
            for back in (1..=6.min((peak / self.period).floor() as usize)).rev() {
                let at = peak - back as f64 * self.period;
                if at <= 0. {
                    continue;
                }
                let energy = self.rms(at, 0.015, true);
                let previous = at - self.period;
                let earlier = if previous >= 0. {
                    self.rms(previous, 0.015, true)
                } else {
                    0.
                };
                if energy > 0.03 && (previous < 0. || earlier < 0.001) {
                    return result(at, beats);
                }
            }
        }
        let step = (0.001 * self.rate as f64).round() as usize;
        let width = (0.003 * self.rate as f64).round() as usize;
        let end = (peak * self.rate as f64).round() as usize;
        let onset = (0..end).step_by(step).find(|at| {
            let stop = (at + width).min(self.pcm.len());
            (power(self.pcm, *at, stop) / (stop as f64 - *at as f64)).sqrt() > 0.02
        });
        if let Some(start) = onset {
            let seconds = start as f64 / self.rate as f64;
            if peak - seconds > 0.3 * self.period {
                let pre = (0.02 * self.rate as f64).round() as usize;
                let begin = start.saturating_sub(pre);
                let silence =
                    (power(self.pcm, begin, start) / (start - begin).max(1) as f64).sqrt();
                if silence < 0.005 {
                    let found = self.scan(
                        (seconds - 0.01).max(0.),
                        (seconds + 0.03).min(peak - 0.05),
                        0.002,
                        true,
                        Peak::at(seconds),
                    );
                    return result(found.at, beats);
                }
            }
        }
        result(peak, beats)
    }
    fn overshoot_forward(&self, limit: f64, beats: f64) -> Option<Walkback> {
        let end = limit.min(4. * self.period);
        let mut at = 0.;
        let mut found = None;
        while at <= end {
            let ratio = attack_ratio(self.pcm, self.rate, at, 15.);
            if ratio > 1.5 {
                let refined = self.offsets(at, -0.015, 0.015, 0.002, Peak { at, ratio });
                if (refined.at < 0.2 || self.kind == IntroKind::Buildup)
                    && refined.ratio <= 10.
                    && self
                        .following(refined.at)
                        .iter()
                        .filter(|r| **r > 1.5)
                        .count()
                        < 2
                {
                    if let Some(peak) = self.first_peak(0., 8. * self.period, false) {
                        return Some(result(peak, beats));
                    }
                    at += 0.01;
                    continue;
                }
                found = Some(refined.at);
                break;
            }
            at += 0.01;
        }
        let found = found?;
        if found < self.drop - 2. * self.period {
            let mut before = 0_f64;
            for back in [1, 2] {
                let at = self.drop - back as f64 * self.period;
                if at > found {
                    before = before.max(self.rms(at, 0.03, false));
                }
            }
            let energy = self.rms(found, 0.03, false);
            let drop = self.rms(self.drop, 0.03, false);
            let ratio = if drop > 0. { energy / drop } else { 1. };
            let earlier = if drop > 0. { before / drop } else { 1. };
            let has_between = self
                .peaks
                .iter()
                .any(|p| *p > found + self.period && *p < self.drop - self.period);
            if before < 0.005 || (ratio < 0.25 && earlier < 0.25 && !has_between) {
                return Some(result(self.drop, 0.));
            }
        }
        Some(result(found, beats))
    }
    fn negative_weak(&self, candidate: f64, phrases: usize) -> Option<Walkback> {
        let beats = phrases as f64 * 16.;
        let audio = first_audio(self.pcm, self.rate, candidate.abs() + 2.);
        if audio.has_silence {
            if let Some(peak) = self.first_peak(
                audio.audio_start,
                audio.audio_start + 2. * self.period,
                true,
            ) {
                return Some(result(peak, beats));
            }
            let found = self.scan(
                (audio.audio_start - 0.1).max(0.),
                audio.audio_start + self.period,
                0.003,
                false,
                Peak::at(audio.audio_start),
            );
            if found.ratio > 2.5 {
                return Some(result(found.at, beats));
            }
            if self.peaks.first().is_some_and(|p| {
                *p > audio.audio_start && (*p - audio.audio_start) / self.period <= 8.
            }) {
                return Some(result(self.peaks[0], beats));
            }
            return Some(result(audio.audio_start, beats));
        }
        let limit = (0.5 * self.period).min(0.5);
        if let Some(peak) = self.first_peak(0., limit, false) {
            return Some(result(peak, beats));
        }
        let found = self.scan(0., limit, 0.003, false, Peak::at(0.));
        if found.ratio > 1.5 {
            if self.kind == IntroKind::Buildup
                && self.peaks.first().is_some_and(|p| *p > 10. * self.period)
                && self.rms(found.at, 0.02, false) < 0.005
                && let Some(aligned) = self.buildup_silence_grid(found.at)
            {
                return Some(result(aligned, beats));
            }
            return Some(result(found.at, beats));
        }
        None
    }
    fn buildup_silence_grid(&self, first: f64) -> Option<f64> {
        let step = (0.001 * self.rate as f64).round() as usize;
        let width = (0.003 * self.rate as f64).round() as usize;
        let start = (first * self.rate as f64).round() as usize;
        let end = (self.peaks[0].min(self.pcm.len() as f64 / self.rate as f64) * self.rate as f64)
            .round() as usize;
        let onset = (start..end).step_by(step).find(|at| {
            let stop = (at + width).min(self.pcm.len());
            (power(self.pcm, *at, stop) / (stop as f64 - *at as f64)).sqrt() > 0.02
        })?;
        let wide = (0.02 * self.rate as f64).round() as usize;
        let hop = (0.01 * self.rate as f64).round() as usize;
        let stop = onset.saturating_sub(wide);
        for at in (start..stop).step_by(hop) {
            if (power(self.pcm, at, at.saturating_add(wide)) / wide as f64).sqrt() >= 0.02 {
                return None;
            }
        }
        let seconds = onset as f64 / self.rate as f64;
        let nearest = js_round((self.drop - seconds) / self.period);
        let target = self.drop - nearest * self.period;
        Some(
            self.scan(
                (target - 0.05).max(0.),
                target + 0.05,
                0.002,
                true,
                Peak::at(target),
            )
            .at,
        )
    }
}
