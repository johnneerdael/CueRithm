//! Read-only public catalog diagnostic; never opens/writes an audio file.
use cuerithm_autotag::{AutotagConfig, TagInput, Tagger};
use cuerithm_media::MediaInfo;
fn main() -> anyhow::Result<()> {
    let tagger = Tagger::new()?;
    let input = TagInput {
        path: "public-catalog-probe.flac".into(),
        source_sha256: "0".repeat(64),
        duration_seconds: 320.,
        media: MediaInfo {
            codec: "flac".into(),
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: Some(16),
            properties: Default::default(),
            tags: [
                ("artist".into(), "Daft Punk".into()),
                ("title".into(), "One More Time".into()),
            ]
            .into(),
        },
    };
    for provider in ["beatport", "traxsource"] {
        let mut config = AutotagConfig::default();
        config.options.platforms = vec![provider.into()];
        match tagger.match_track(&input, &config) {
            Ok(result) => println!(
                "{provider}: {:?}, {} matches",
                result.status,
                result.matches.len()
            ),
            Err(error) => println!(
                "{provider}: {}",
                error.to_string().chars().take(400).collect::<String>()
            ),
        }
    }
    Ok(())
}
