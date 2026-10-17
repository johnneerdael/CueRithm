use crate::dsp::legacy_onset_rust_v1::{Mark, OnsetStream};
use crate::dsp::legacy_tracker_rust_v2::{Grid, Tracker};
pub const CAP: usize = 24 * 44100;
pub struct Classic {
    pub initial_bpm: f32,
    pub adaptation: f32,
    pub saved_exponent: f32,
    pub ready: bool,
    pub first_onset: bool,
    pub candidate_count: usize,
    pub rolling_offset: i32,
    pub relationship: i32,
    pub export_ready: bool,
    pub candidate: Option<Vec<f32>>,
    pub saved_grid: Option<[Grid; 7]>,
    pub onset: OnsetStream,
    pub tracker: Option<Tracker>,
    channels: usize,
    finished: bool,
    failed: bool,
}
pub struct WireResult {
    pub tempo: f32,
    pub signature: i32,
    pub marks: Vec<Mark>,
    pub times: Vec<f64>,
    pub valid: bool,
}
impl Classic {
    pub fn state_bytes(
        &self,
        status: i32,
        histogram: Option<(f32, i32)>,
    ) -> Result<Vec<u8>, String> {
        fn word(out: &mut Vec<u8>, value: u32) {
            out.extend_from_slice(&value.to_le_bytes());
        }
        let output = self
            .tracker
            .as_ref()
            .map_or(&[][..], |t| t.output.as_slice());
        let overall = overall_bpm(output)?;
        let wire = self.wire_result()?;
        let onset = self.onset.state_bytes();
        let tracker = self.tracker.as_ref().map(|t| t.state_bytes(0));
        let mut out = vec![0u8; 4];
        for value in [
            status,
            i32::from(histogram.is_some()),
            histogram.map_or(0, |h| h.1),
            44100,
            self.channels as i32,
            self.candidate_count as i32,
            self.rolling_offset,
            self.relationship,
            i32::from(self.ready),
            i32::from(self.first_onset),
            i32::from(self.candidate.is_some()),
            i32::from(self.saved_grid.is_some()),
            i32::from(self.tracker.is_some()),
            i32::from(self.export_ready),
            self.onset.marks().len() as i32,
            output.len() as i32,
            i32::from(wire.valid),
            wire.marks.len() as i32,
            wire.signature,
        ] {
            word(&mut out, value as u32);
        }
        for value in [
            self.initial_bpm,
            self.adaptation,
            self.saved_exponent,
            histogram.map_or(0., |h| h.0),
            overall,
            wire.tempo,
        ] {
            word(&mut out, value.to_bits());
        }
        if let Some(candidate) = &self.candidate {
            for &v in candidate {
                word(&mut out, v.to_bits());
            }
        }
        if let Some(grid) = &self.saved_grid {
            for g in grid {
                word(&mut out, g.closest as u32);
                for v in [
                    g.anchor,
                    g.period,
                    g.prediction,
                    g.multiplier,
                    g.reciprocal,
                    g.bpm,
                    g.smooth,
                    g.instant,
                ] {
                    word(&mut out, v.to_bits());
                }
            }
        }
        word(&mut out, onset.len() as u32);
        out.extend_from_slice(&onset);
        if let Some(tracker) = tracker {
            out.extend_from_slice(&tracker);
        } else {
            word(&mut out, 0);
        }
        for m in &wire.marks {
            out.extend_from_slice(&m.export_le_bytes());
        }
        for &t in &wire.times {
            out.extend_from_slice(&t.to_le_bytes());
        }
        let length = (out.len() - 4) as u32;
        out[..4].copy_from_slice(&length.to_le_bytes());
        Ok(out)
    }
}
impl Classic {
    pub fn new(channels: usize) -> Result<Self, String> {
        if !(1..=2).contains(&channels) {
            return Err("default classic profile requires1..2 channels".into());
        }
        Ok(Self {
            initial_bpm: 0.,
            adaptation: 1.,
            saved_exponent: 1.,
            ready: false,
            first_onset: false,
            candidate_count: 0,
            rolling_offset: 0,
            relationship: -1,
            export_ready: false,
            candidate: Some(Vec::with_capacity(CAP)),
            saved_grid: None,
            onset: OnsetStream::new(channels)?,
            tracker: None,
            channels,
            finished: false,
            failed: false,
        })
    }
    fn active(&self) -> Result<(), String> {
        if self.failed {
            Err("poisoned classic lifecycle".into())
        } else {
            Ok(())
        }
    }
    pub fn pre_analysis(&mut self, planes: &[&[f32]]) -> Result<(), String> {
        let result = self.pre_inner(planes);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn pre_inner(&mut self, planes: &[&[f32]]) -> Result<(), String> {
        self.active()?;
        if self.finished {
            return Err("feed after Finish unsupported".into());
        }
        if planes.len() != self.channels {
            return Err("channel shape".into());
        }
        let n = planes[0].len();
        if n > 32768
            || planes
                .iter()
                .any(|p| p.len() != n || p.iter().any(|v| !v.is_finite() || v.abs() > 1.))
        {
            return Err("finite normalized PCM/feed domain".into());
        }
        self.buffer_candidate(planes)?;
        self.onset.process(planes)?;
        if self.onset.marks().is_empty() {
            self.first_onset = false;
        } else if !self.first_onset {
            let sample = self.onset.marks()[0].sample;
            let delta = sample
                .checked_sub(self.rolling_offset)
                .ok_or("onset alignment overflow")?;
            if delta < 0 || delta as usize > self.candidate_count {
                return Err("source candidate alignment outside initialized slice".into());
            }
            let values = self
                .candidate
                .as_mut()
                .ok_or("missing candidate allocation")?;
            values.copy_within(delta as usize..self.candidate_count, 0);
            self.candidate_count -= delta as usize;
            values.truncate(self.candidate_count);
            self.first_onset = true;
        }
        Ok(())
    }
    #[allow(clippy::assign_op_pattern)] // Preserve source incoming + accumulated order.
    fn buffer_candidate(&mut self, planes: &[&[f32]]) -> Result<(), String> {
        let n = planes[0].len();
        let c = self.candidate_count;
        let values = self.candidate.as_mut().ok_or("candidate already freed")?;
        if values.len() != c || c > CAP || n > 32768 {
            return Err("candidate storage admission".into());
        }
        if c + n >= CAP {
            let remaining = CAP - c;
            if remaining == 0 && self.first_onset {
                return Ok(());
            }
            let discarded = n
                .checked_sub(remaining)
                .ok_or("invalid source candidate move")?;
            let kept = c
                .checked_sub(discarded)
                .ok_or("invalid source candidate kept count")?;
            values.copy_within(discarded..c, 0);
            values.truncate(kept);
            self.candidate_count = kept;
            self.rolling_offset = self
                .rolling_offset
                .checked_add(n as i32)
                .ok_or("rolling offset overflow")?;
        }
        let offset = self.candidate_count;
        values.extend_from_slice(planes[0]);
        for plane in &planes[1..] {
            for (value, &incoming) in values[offset..].iter_mut().zip(*plane) {
                *value = incoming + *value;
            }
        }
        self.candidate_count += n;
        Ok(())
    }
    pub fn finish_pre_analysis(&mut self, use_initial: bool) -> Result<(), String> {
        let result = self.finish_inner(use_initial);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn finish_inner(&mut self, use_initial: bool) -> Result<(), String> {
        self.active()?;
        if self.finished {
            return Err("repeated Finish unsupported".into());
        }
        let mut original = 0.;
        if use_initial && self.candidate_count > 4095 {
            let input = self
                .candidate
                .as_ref()
                .ok_or("missing initial-tempo candidate")?;
            let result = crate::dsp::legacy_initial_tempo_rust_v1::analyze(input)?;
            let bpm = result.bpm;
            original = if bpm <= 55. { bpm + bpm } else { bpm };
            self.initial_bpm = original;
            self.candidate = None;
        } else {
            self.initial_bpm = 0.;
            if !use_initial {
                self.candidate = None;
            }
        }
        let marks = self.onset.finish()?;
        let mut tracker = Tracker::new(&marks, self.initial_bpm)?;
        tracker.energy_ratio = 0.;
        tracker.bpm_adapt = true;
        tracker.preprocess()?;
        tracker.preprocess()?;
        self.saved_grid = Some(tracker.grid);
        self.saved_exponent = tracker.exponent;
        self.tracker = Some(tracker);
        self.ready = true;
        if self.initial_bpm > 150. {
            self.initial_bpm *= 0.5;
        }
        let (histogram, _nonempty) = self.calculate_histogram()?;
        let hist = if histogram > 150. {
            histogram * 0.5
        } else {
            histogram
        };
        let initial = self.initial_bpm;
        if initial > 0. {
            let diff = (initial - hist).abs();
            let quotient = diff / initial;
            let agreement = 1. - quotient;
            let agree = f64::from(agreement) >= f64::from_bits(0x3fee147ae147ae14);
            let selected = if agree { initial } else { hist };
            let candidate = if selected > 55. {
                selected
            } else {
                selected + selected
            };
            if !(agree && selected > 55.) {
                self.initial_bpm = candidate;
            }
            if (original - candidate).abs() < f32::from_bits(0x7e967699) {
                self.relationship = 0;
            }
            let p = f64::from(original);
            let q = f64::from(candidate);
            let bound = f64::from_bits(0x47d2ced320000000);
            if (p * 1.25 - q).abs() < bound {
                self.relationship = 1;
            }
            if (p * 0.75 - q).abs() < bound {
                self.relationship = 2;
            }
            if (p * 1.5 - q).abs() < bound {
                self.relationship = 2;
            }
            if ((p + p) - q).abs() < bound {
                self.relationship = 0;
            }
            if (p * 0.5 - q).abs() < bound {
                self.relationship = 0;
            }
        }
        self.tracker
            .as_mut()
            .ok_or("missing finished tracker")?
            .energy_ratio = f32::from_bits(0x3d4ccccd);
        self.finished = true;
        Ok(())
    }
    pub fn process(
        &mut self,
        adaptation: f32,
        start: i32,
        reset_after_backward: bool,
        bpm_adapt: bool,
    ) -> Result<(), String> {
        let result = self.process_inner(adaptation, start, reset_after_backward, bpm_adapt);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn process_inner(
        &mut self,
        adaptation: f32,
        start: i32,
        reset_after_backward: bool,
        bpm_adapt: bool,
    ) -> Result<(), String> {
        self.active()?;
        if !adaptation.is_finite() || !(0.0..=50.).contains(&adaptation) {
            return Err("adaptation domain".into());
        }
        if !self.ready || self.onset.marks().len() < 7 {
            return Ok(());
        }
        if adaptation != 0. {
            self.adaptation = adaptation;
        }
        let saved = self.saved_grid.ok_or("missing saved grid")?;
        let tracker = self.tracker.as_mut().ok_or("missing tracker")?;
        tracker.process(
            saved,
            self.saved_exponent,
            self.initial_bpm,
            adaptation,
            start,
            reset_after_backward,
            bpm_adapt,
        )?;
        self.export_ready = true;
        Ok(())
    }
    pub fn calculate_histogram(&mut self) -> Result<(f32, i32), String> {
        self.active()?;
        self.process(1.1, 0, false, true)?;
        let marks = self
            .tracker
            .as_ref()
            .map_or(&[][..], |t| t.output.as_slice());
        if !marks.is_empty() && !self.export_ready {
            self.failed = true;
            return Err("source histogram export buffer not refreshed".into());
        }
        crate::dsp::legacy_tracker_rust_v2::histogram(marks)
    }
    pub fn wire_result(&self) -> Result<WireResult, String> {
        self.active()?;
        let marks = self
            .tracker
            .as_ref()
            .map_or(&[][..], |t| t.output.as_slice());
        if !marks.is_empty() && !self.export_ready {
            return Err("export buffer not refreshed".into());
        }
        let times: Vec<_> = marks.iter().map(|m| f64::from(m.sample) / 44100.).collect();
        let tempo = if marks.len() >= 4 {
            overall_bpm(marks)?
        } else {
            0.
        };
        let mut valid = marks.len() >= 4 && tempo.is_finite() && (30.0..=400.).contains(&tempo);
        for (i, &t) in times.iter().enumerate() {
            if t.is_nan() || t < -20. {
                valid = false;
            }
            if i > 0 {
                let dt = t - times[i - 1];
                if dt <= 0.001 || dt > 4. {
                    valid = false;
                }
            }
        }
        Ok(if valid {
            WireResult {
                tempo,
                signature: 8,
                marks: marks.to_vec(),
                times,
                valid: true,
            }
        } else {
            WireResult {
                tempo: 0.,
                signature: 8,
                marks: Vec::new(),
                times: Vec::new(),
                valid: false,
            }
        })
    }
}
/// Source39740 reducer, independent from the adaptive histogram mechanism.
pub fn overall_bpm(marks: &[Mark]) -> Result<f32, String> {
    if marks.len() > 1000000
        || marks
            .iter()
            .any(|m| !m.local_bpm.is_finite() || !(0.0..=1000.).contains(&m.local_bpm))
    {
        return Err("overall tempo input domain".into());
    }
    let mut minimum = 500f32;
    let mut maximum = 0f32;
    for m in marks {
        let x = m.local_bpm;
        if x > maximum {
            maximum = x;
        }
        if x < minimum {
            minimum = x;
        }
    }
    let spread = maximum - minimum;
    let quotient = spread / 5.;
    let bins = ((f64::from(quotient) + 1.5) as i32).max(1) as usize;
    if bins > 201 {
        return Err("overall tempo bin domain".into());
    }
    let mut counts = vec![0u32; bins];
    for m in marks {
        let delta = m.local_bpm - minimum;
        let quotient = delta / 5.;
        let bin = (f64::from(quotient) + 0.5) as usize;
        let count = counts.get_mut(bin).ok_or("overall tempo bin bounds")?;
        *count += 1;
    }
    let mut best = None;
    let mut highest = 0;
    for (i, &count) in counts.iter().enumerate() {
        if count > highest {
            highest = count;
            best = Some(i);
        }
    }
    let mut sum = 0.;
    let mut included = 0;
    if let Some(bin) = best {
        let step = (bin * 5) as f32;
        let center = minimum + step;
        let low = center + -2.5;
        let high = low + 10.;
        for m in marks {
            if m.local_bpm >= low && m.local_bpm <= high {
                sum += m.local_bpm;
                included += 1;
            }
        }
    }
    Ok(if included == 0 {
        0.
    } else {
        sum / included as f32
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_instance_matches_initialized_classic_fields() {
        let c = Classic::new(2).unwrap();
        assert_eq!(c.initial_bpm.to_bits(), 0);
        assert_eq!(c.adaptation, 1.);
        assert_eq!(c.saved_exponent, 1.);
        assert!(!c.ready && !c.first_onset && !c.export_ready);
        assert_eq!(
            (c.candidate_count, c.rolling_offset, c.relationship),
            (0, 0, -1)
        );
        assert!(c.saved_grid.is_none() && c.tracker.is_none());
    }
    #[test]
    fn candidate_uses_channel_sum_and_preserves_negative_zero() {
        let mut c = Classic::new(2).unwrap();
        c.pre_analysis(&[&[1., -0.], &[1., -0.]]).unwrap();
        let values = c.candidate.unwrap();
        assert_eq!(values[0], 2.);
        assert_eq!(values[1].to_bits(), 0x80000000);
        assert_eq!(c.candidate_count, 2);
    }
    #[test]
    fn capacity_equality_advances_full_feed_offset_without_discard() {
        let mut c = Classic::new(1).unwrap();
        c.candidate = Some(vec![0.; CAP - 4]);
        c.candidate_count = CAP - 4;
        c.buffer_candidate(&[&[1.; 4]]).unwrap();
        assert_eq!(c.candidate_count, CAP);
        assert_eq!(c.rolling_offset, 4);
        assert_eq!(&c.candidate.as_ref().unwrap()[CAP - 4..], &[1.; 4]);
    }
    #[test]
    fn full_buffer_rolls_before_first_onset_and_stops_after_recognition() {
        let mut c = Classic::new(1).unwrap();
        c.candidate = Some((0..CAP).map(|i| (i % 13) as f32 / 16.).collect());
        c.candidate_count = CAP;
        c.buffer_candidate(&[&[-1.; 4]]).unwrap();
        assert_eq!(c.candidate.as_ref().unwrap()[0], 0.25);
        assert_eq!(c.rolling_offset, 4);
        c.first_onset = true;
        let before = c.candidate.as_ref().unwrap().clone();
        c.buffer_candidate(&[&[1.; 4]]).unwrap();
        assert_eq!(c.candidate.unwrap(), before);
        assert_eq!(c.rolling_offset, 4);
    }
    #[test]
    fn silence_finishes_both_stages_and_exports_empty_fallback() {
        let mut c = Classic::new(1).unwrap();
        for _ in 0..16 {
            c.pre_analysis(&[&[0.; 512]]).unwrap();
        }
        c.finish_pre_analysis(true).unwrap();
        assert!(c.ready);
        assert!(c.candidate.is_none());
        assert_eq!(c.candidate_count, 8192);
        c.process(0., 0, false, false).unwrap();
        let w = c.wire_result().unwrap();
        assert!(!w.valid);
        assert_eq!(w.tempo.to_bits(), 0);
        assert_eq!(w.signature, 8);
        assert!(w.marks.is_empty() && w.times.is_empty());
        assert!(c.pre_analysis(&[&[0.; 512]]).is_err());
        assert!(c.finish_pre_analysis(true).is_err());
    }
    #[test]
    fn short_finish_retains_candidate_allocation() {
        let mut c = Classic::new(1).unwrap();
        c.pre_analysis(&[&[0.; 512]]).unwrap();
        c.finish_pre_analysis(true).unwrap();
        assert!(c.candidate.is_some());
        assert_eq!(c.candidate_count, 512);
    }
    #[test]
    fn overall_getter_uses_modal_inclusive_range_and_sequential_sum() {
        let make = |bpm| Mark {
            sample: 0,
            transition_energy: 0.,
            local_bpm: bpm,
            confidence: 0.,
        };
        assert_eq!(overall_bpm(&[]).unwrap(), 0.);
        let a = [make(100.), make(100.), make(102.5), make(110.)];
        assert_eq!(overall_bpm(&a).unwrap(), 100.833336);
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    #[test]
    fn histogram_rejects_source_export_reads_before_cache_refresh() {
        let mut c = Classic::new(1).unwrap();
        let mut t = Tracker::new(&[], 120.).unwrap();
        t.output.push(Mark {
            sample: 0,
            transition_energy: 1.,
            local_bpm: 120.,
            confidence: 1.,
        });
        c.tracker = Some(t);
        c.ready = true;
        assert!(c.calculate_histogram().is_err());
    }
}
