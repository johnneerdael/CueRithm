use serde::Deserialize;

#[derive(Deserialize)]
pub struct Recipe {
    #[serde(default)]
    pub variant: String,
    pub frames: usize,
    pub quiet_until_frames: usize,
    pub offbeat_hats: bool,
    pub bpm: f64,
    pub offset_frames: usize,
}
pub fn signal(recipe: &Recipe, rate: u32) -> Vec<f32> {
    let mut pcm = vec![0.; recipe.frames];
    if recipe.variant.starts_with("intro-") {
        for (i, value) in pcm.iter_mut().enumerate() {
            *value = if i >= recipe.quiet_until_frames {
                0.5
            } else if recipe.variant == "intro-atmospheric" {
                1. / 16.
            } else if recipe.variant == "intro-buildup" {
                ((i / 4096) as f64).powi(2) as f32 / 65536.
            } else {
                0.
            };
        }
        return pcm;
    }

    if recipe.variant == "quiet-bed" {
        pcm.fill(1. / 256.);
    }
    if recipe.variant == "late-bed" {
        pcm[..recipe.offset_frames.min(recipe.frames)].fill(1. / 256.);
    }
    if recipe.variant == "soft-riser" {
        pcm[..(4 * rate as usize).min(recipe.frames)].fill(1. / 32.);
    }
    for beat in 0.. {
        let start = (recipe.offset_frames as f64 + beat as f64 * 60. * rate as f64 / recipe.bpm)
            .round() as usize;
        if start >= pcm.len() {
            break;
        }
        if recipe.variant == "soft-riser" && (2..=8).contains(&beat) {
            continue;
        }
        let gain = if recipe.variant == "structured-cue" {
            let seconds = start as f64 / rate as f64;
            if seconds < 16. {
                1. / 8.
            } else if seconds < 64. {
                1.
            } else if seconds < 96. {
                1. / 16.
            } else if seconds < 144. {
                1.
            } else if seconds < 176. {
                1. / 16.
            } else {
                1.
            }
        } else if recipe.variant == "quiet-bed" {
            1. / 128.
        } else if start < recipe.quiet_until_frames {
            1. / 16.
        } else {
            1.
        };
        for index in 0..4096.min(pcm.len() - start) {
            let sign = if index % 512 < 256 { 1. } else { -1. };
            pcm[start + index] = ((4096 - index) as f64 / 4096. * sign * gain) as f32;
        }
        if recipe.offbeat_hats && (recipe.variant != "sparse-half" || beat % 3 == 0) {
            let hat = start + (30. * rate as f64 / recipe.bpm).round() as usize;
            for index in 0..256.min(pcm.len().saturating_sub(hat)) {
                let sign = if index % 2 == 1 { 1. } else { -1. };
                pcm[hat + index] =
                    (pcm[hat + index] as f64 + (256 - index) as f64 / 2048. * sign) as f32;
            }
        }
    }
    pcm
}
