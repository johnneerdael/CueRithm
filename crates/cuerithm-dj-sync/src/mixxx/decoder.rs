//! Which decoder this system's Mixxx uses for MP3, and so where its frame timeline
//! starts relative to CueRithm's FFmpeg clock.
//!
//! Mixxx registers decoders per platform (`src/sources/soundsourceproxy.cpp`) and
//! picks the one with the highest priority hint for a file type:
//!
//! | System | MP3 decoder | Priority | Why |
//! | --- | --- | --- | --- |
//! | macOS | Apple CoreAudio | Higher | `SoundSourceProviderCoreAudio`, lists `mp3` |
//! | Linux, Windows | MAD (libmad) | Default | `SoundSourceProviderMp3`; FFmpeg is only a Lowest fallback |
//!
//! Both are measured in `docs/research/mixxx-sync-contract.md`.

use super::lame::Mp3Start;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mp3Decoder {
    /// macOS: `ExtAudioFile`, which discards the decoder delay and any gapless
    /// priming its AudioFile parser found.
    CoreAudio,
    /// Linux and Windows: libmad through `SoundSourceMp3`, which drops the first
    /// frame unconditionally and trims nothing else.
    Mad,
}

#[cfg(test)]
thread_local! {
    static HOST: std::cell::Cell<Option<Mp3Decoder>> = const { std::cell::Cell::new(None) };
}
/// Tests only: behave as if this system's Mixxx used `decoder`.
#[cfg(test)]
pub(crate) fn pretend_host(decoder: Mp3Decoder) {
    HOST.set(Some(decoder));
}

impl Mp3Decoder {
    /// Mixxx's library is local to the machine it runs on, so the host's decoder is
    /// the one that will play these tracks.
    pub(crate) fn host() -> Self {
        #[cfg(test)]
        if let Some(decoder) = HOST.get() {
            return decoder;
        }
        if cfg!(target_os = "macos") {
            Self::CoreAudio
        } else {
            Self::Mad
        }
    }
}

/// The 528 + 1 samples of decoder delay every MP3 decoder produces before the signal.
const DECODER_DELAY: i64 = 529;

/// Samples to add to a position on CueRithm's FFmpeg clock to get the Mixxx frame.
///
/// FFmpeg drops an Info/Xing frame and trims `encoder delay + 529` when it trusts a
/// `LAME`/`Lavf`/`Lavc` tag (`start.trim`). `coreaudio_priming` is what Apple's
/// AudioFile reports for the file (`None` when it found no gapless table); Apple
/// recognises tags by its own rules, so it must be asked, not predicted.
pub(crate) fn mp3_offset_samples(
    decoder: Mp3Decoder,
    start: &Mp3Start,
    coreaudio_priming: Option<u32>,
) -> i64 {
    let ffmpeg = i64::from(start.trim.unwrap_or(0));
    match decoder {
        Mp3Decoder::Mad => {
            let dropped = if start.info_frame {
                0
            } else {
                i64::from(start.first_frame_samples)
            };
            ffmpeg - dropped
        }
        Mp3Decoder::CoreAudio => ffmpeg - (DECODER_DELAY + coreaudio_priming.map_or(0, i64::from)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(info_frame: bool, trim: Option<u32>) -> Mp3Start {
        Mp3Start {
            info_frame,
            first_frame_samples: 1152,
            trim,
        }
    }

    #[test]
    fn mad_drops_the_first_frame_and_trims_nothing_else() {
        // No Info frame: a real frame is lost. Info frame, no tag: nothing differs.
        assert_eq!(
            mp3_offset_samples(Mp3Decoder::Mad, &start(false, None), None),
            -1152
        );
        assert_eq!(
            mp3_offset_samples(Mp3Decoder::Mad, &start(true, None), None),
            0
        );
        // A tag FFmpeg trusts: it trims delay + 529, MAD does not.
        assert_eq!(
            mp3_offset_samples(Mp3Decoder::Mad, &start(true, Some(1105)), None),
            1105
        );
        assert_eq!(
            mp3_offset_samples(Mp3Decoder::Mad, &start(true, Some(1529)), None),
            1529
        );
        // MPEG-2 frames are 576 samples.
        let half = Mp3Start {
            first_frame_samples: 576,
            ..start(false, None)
        };
        assert_eq!(mp3_offset_samples(Mp3Decoder::Mad, &half, None), -576);
        // The CoreAudio priming is ignored on this path.
        assert_eq!(
            mp3_offset_samples(Mp3Decoder::Mad, &start(true, Some(1105)), Some(576)),
            1105
        );
    }

    /// Every row is a measurement: `coreaudio_probe` against FFmpeg on a click track
    /// (see the research notes). `priming` is the AudioFile packet-table value.
    #[test]
    fn coreaudio_matches_the_measured_macos_results() {
        for (what, ffmpeg_trim, priming, measured) in [
            ("lame tag, valid or broken CRC", Some(1105), Some(576), 0),
            ("lame -V2 Xing tag", Some(1105), Some(576), 0),
            ("FFmpeg libmp3lame (Lavc63.1)", Some(1105), Some(576), 0),
            ("no Info frame", None, None, -529),
            ("Info frame, identifier replaced", None, None, -529),
            ("identifier GOGO, XING, Bolt or zeros", None, None, -529),
            // FFmpeg trusts the identifier but Apple read no gapless table.
            (
                "hand-patched Lavf or Lavc identifier",
                Some(1105),
                None,
                576,
            ),
        ] {
            let start = start(true, ffmpeg_trim);
            assert_eq!(
                mp3_offset_samples(Mp3Decoder::CoreAudio, &start, priming),
                measured,
                "{what}"
            );
        }
        // A different encoder delay moves both clocks together.
        assert_eq!(
            mp3_offset_samples(Mp3Decoder::CoreAudio, &start(true, Some(1529)), Some(1000)),
            0
        );
        // The first-frame length plays no part: CoreAudio keeps a real first frame.
        assert_eq!(
            mp3_offset_samples(Mp3Decoder::CoreAudio, &start(false, None), None),
            -529
        );
    }

    #[test]
    fn the_host_decoder_follows_the_platform_priorities() {
        let expected = if cfg!(target_os = "macos") {
            Mp3Decoder::CoreAudio
        } else {
            Mp3Decoder::Mad
        };
        assert_eq!(Mp3Decoder::host(), expected);
    }
}
