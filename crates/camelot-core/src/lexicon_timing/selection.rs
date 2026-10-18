//! Initial grid anchor selection, before the separate final correction stage.
use super::{
    LexiconTempoOptions, TempoEvidence, analyze_tempo, anchors,
    finish::Candidate,
    intro::{self, IntroEvidence},
    onset::{self, attack_ratio, power},
    phase::{self, CorrectionState},
    walkback,
};
use anyhow::Result;

mod drop;
mod early;
mod immediate;
mod long_intro;

struct Context<'a> {
    pcm: &'a [f32],
    rate: u32,
    bpm: f64,
    period: f64,
    confidence: f64,
    peaks: Vec<f64>,
    intro: IntroEvidence,
}
impl Context<'_> {
    fn index(&self, seconds: f64) -> usize {
        (seconds * self.rate as f64).round().max(0.) as usize
    }
    fn width(&self, milliseconds: f64) -> usize {
        (milliseconds / 1000. * self.rate as f64).round() as usize
    }
    fn rms_index(&self, index: usize, width: usize) -> f64 {
        (power(self.pcm, index, index.saturating_add(width)) / width as f64).sqrt()
    }
    fn rms(&self, seconds: f64, milliseconds: f64) -> f64 {
        self.rms_index(self.index(seconds), self.width(milliseconds))
    }
    fn ar(&self, at: f64, milliseconds: f64) -> f64 {
        attack_ratio(self.pcm, self.rate, at, milliseconds)
    }
    fn dynamic(&self, at: f64, options: Option<(f64, f64)>) -> f64 {
        onset::dynamic_onset(self.pcm, self.rate, at, options)
    }
    fn snap(&self, at: f64) -> f64 {
        onset::snap(at, &self.peaks, self.period, 0.015)
    }
    fn walk(&self, at: f64, strength: Option<f64>) -> walkback::Walkback {
        walkback::walk_back(
            self.pcm,
            self.rate,
            at,
            self.period,
            &self.peaks,
            strength,
            self.intro.intro_type,
        )
    }
    fn correct(&self, at: f64, state: &mut CorrectionState) -> f64 {
        let result = phase::subsequent(
            self.pcm,
            self.rate,
            at,
            self.bpm,
            &phase::SubsequentOptions::default(),
            state,
        );
        if result.applied_correction {
            result.corrected_first_beat
        } else {
            at
        }
    }
    fn candidate(&self, at: f64) -> Candidate {
        Candidate {
            bpm: self.bpm,
            first_beat: at,
            confidence: self.confidence,
            peaks: self.peaks.iter().take(30).copied().collect(),
        }
    }
    fn refine_first_peak(&self) -> f64 {
        let first = self.peaks[0];
        let (mut best, mut ratio) = (first, self.ar(first, 15.));
        let mut at = (first - 0.055).max(0.);
        while at <= first + 0.01 {
            let next = self.ar(at, 15.);
            if next > ratio {
                best = at;
                ratio = next;
            }
            at += 0.001;
        }
        best
    }
    fn initial_max_rms(&self, limit: f64) -> f64 {
        let mut best: f64 = 0.;
        let mut index = 0;
        while (index as f64) < limit * self.rate as f64 {
            best = best.max(self.rms_index(index, self.width(50.)));
            index += self.width(25.);
        }
        best
    }
    fn following_hits(&self, at: f64, count: usize, threshold: f64) -> usize {
        let mut hits = 0;
        for beat in 1..=count {
            let center = at + beat as f64 * self.period;
            let mut best = 0.;
            let mut probe = center - 0.03;
            while probe <= center + 0.03 {
                if probe >= 0. {
                    let ratio = self.ar(probe, 15.);
                    if ratio > best {
                        best = ratio;
                    }
                }
                probe += 0.005;
            }
            if best > threshold {
                hits += 1;
            }
        }
        hits
    }
}

pub(super) fn select(
    pcm: &[f32],
    rate: u32,
    options: &LexiconTempoOptions,
    state: &mut CorrectionState,
) -> Result<Option<Candidate>> {
    state.reset();
    let Some(tempo) = analyze_tempo(pcm, rate, options)? else {
        return Ok(None);
    };
    Ok(from_tempo(pcm, rate, tempo, state))
}

// The caller owns PCM/rate validation and supplies tempo-helper evidence.
pub(super) fn from_tempo(
    pcm: &[f32],
    rate: u32,
    tempo: TempoEvidence,
    state: &mut CorrectionState,
) -> Option<Candidate> {
    state.reset();
    let mut context = Context {
        pcm,
        rate,
        bpm: tempo.bpm,
        period: 60. / tempo.bpm,
        confidence: tempo.confidence,
        peaks: tempo
            .peaks
            .into_iter()
            .map(|index| index as f64 / rate as f64)
            .collect(),
        intro: intro::detect(pcm, rate, &intro::IntroOptions::default()),
    };
    remove_leading_glitches(&mut context);
    if context.peaks.is_empty() {
        return None;
    }
    let anchor = if let Some(at) = long_intro::select(&context, state) {
        at
    } else if let Some(at) = drop::weak_start(&context, state) {
        at
    } else if let Some(at) = early::delayed_transient(&context, state) {
        at
    } else {
        let immediate = anchors::starts_immediately(&context.peaks, context.period, 5.);
        if let Some(at) = early::non_immediate(&context, immediate, state) {
            at
        } else if immediate {
            immediate::select(&context, state)
        } else if let Some(at) = early::near_zero(&context, state) {
            at
        } else if let Some(at) = early::initial_energy(&context, state) {
            at
        } else if let Some(at) = early::late_first_peak(&context, state) {
            at
        } else {
            drop::select(&context, state)
        }
    };
    Some(context.candidate(anchor))
}

fn remove_leading_glitches(c: &mut Context<'_>) {
    if c.peaks.len() < 2 || c.peaks[0] >= 0.005 {
        return;
    }
    let quiet = c.rms(0.1, 30.).max(c.rms(0.2, 30.)) < 0.005;
    let duplicate = c.peaks[1] - c.peaks[0] < 0.001;
    let quarter_distance = |beats: f64| {
        let part = ((beats % 0.25) + 0.25) % 0.25;
        part.min(0.25 - part)
    };
    let off_grid = c.peaks.len() >= 3
        && quarter_distance((c.peaks[1] - c.peaks[0]) / c.period) > 0.11
        && quarter_distance((c.peaks[2] - c.peaks[0]) / c.period) > 0.11
        && quarter_distance((c.peaks[2] - c.peaks[1]) / c.period) <= 0.11;
    if quiet || duplicate || off_grid {
        let count = c.peaks.iter().take_while(|at| **at < 0.005).count();
        c.peaks.drain(..count);
    }
}
