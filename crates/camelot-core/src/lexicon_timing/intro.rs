use super::onset::power;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum IntroKind {
    None,
    Silent,
    Buildup,
    Atmospheric,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IntroEvidence {
    pub has_intro: bool,
    pub intro_end_position: f64,
    pub intro_type: IntroKind,
    pub confidence: f64,
}
impl IntroEvidence {
    fn none(confidence: f64) -> Self {
        Self {
            has_intro: false,
            intro_end_position: 0.,
            intro_type: IntroKind::None,
            confidence,
        }
    }
}
pub(super) struct IntroOptions {
    pub max_seconds: f64,
    pub window_ms: f64,
    pub hop_ms: f64,
}
impl Default for IntroOptions {
    fn default() -> Self {
        Self {
            max_seconds: 90.,
            window_ms: 100.,
            hop_ms: 50.,
        }
    }
}
pub(super) fn detect(pcm: &[f32], rate: u32, options: &IntroOptions) -> IntroEvidence {
    if !(8000..=192000).contains(&rate)
        || !options.max_seconds.is_finite()
        || options.max_seconds < 0.
        || !options.window_ms.is_finite()
        || options.window_ms <= 0.
        || !options.hop_ms.is_finite()
        || options.hop_ms <= 0.
    {
        return IntroEvidence::none(0.);
    }
    let width = (options.window_ms / 1000. * rate as f64).round() as usize;
    let hop = (options.hop_ms / 1000. * rate as f64).round() as usize;
    if width == 0 || hop == 0 {
        return IntroEvidence::none(0.);
    }
    if width >= pcm.len() {
        return IntroEvidence::none(1.);
    }
    let limit = options.max_seconds.min(pcm.len() as f64 / rate as f64 - 1.);
    let mut profile = vec![];
    let mut times = vec![];
    for start in (0..pcm.len() - width).step_by(hop) {
        if start as f64 / rate as f64 >= limit {
            break;
        }
        profile.push((power(pcm, start, start + width) / width as f64).sqrt());
        times.push(start as f64 / rate as f64);
    }
    if profile.len() < 10 {
        return IntroEvidence::none(1.);
    }
    let maximum = profile.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if maximum < 0.001 {
        return IntroEvidence::none(1.);
    }
    let normalized: Vec<f64> = profile.into_iter().map(|v| v / maximum).collect();
    let mut boundary = None;
    let mut jump = 0.;
    let begin = (10. / (options.hop_ms / 1000.)).ceil() as usize;
    for at in begin..normalized.len().saturating_sub(5) {
        let before = normalized[..at].iter().sum::<f64>() / at as f64;
        let count = 20.min(normalized.len() - at);
        let after = normalized[at..at + count].iter().sum::<f64>() / count as f64;
        let ratio = after / (before + 0.001);
        if ratio > 2. && after > 0.3 && ratio > jump {
            jump = ratio;
            boundary = Some(at);
        }
    }
    let Some(boundary) = boundary else {
        let count = 20.min(normalized.len());
        let initial = normalized[..count].iter().sum::<f64>() / count as f64;
        return IntroEvidence::none(if initial > 0.25 { 0.8 } else { 0.5 });
    };
    let before = &normalized[..boundary];
    let mean = before.iter().sum::<f64>() / before.len() as f64;
    let peak = before.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let count = 40.min(boundary);
    if normalized[..count].iter().sum::<f64>() / count as f64 > 0.3 {
        return IntroEvidence::none(0.6);
    }
    let intro_type = if mean < 0.05 && peak < 0.1 {
        IntroKind::Silent
    } else if peak > 2. * mean {
        IntroKind::Buildup
    } else {
        IntroKind::Atmospheric
    };
    IntroEvidence {
        has_intro: true,
        intro_end_position: times[boundary],
        intro_type,
        confidence: (jump / 5.).min(1.),
    }
}
