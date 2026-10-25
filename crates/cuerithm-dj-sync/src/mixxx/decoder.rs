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

/// How the Mixxx on this system decodes AAC in an M4A, which decides whether the encoder's
/// priming is skipped (see `mp4.rs` for what was measured).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AacDecoder {
    /// FAAD, which reads from the first block and ignores the file's priming.
    Faad,
    /// FFmpeg (Debian's Mixxx) or CoreAudio (macOS): the priming is dropped, as CueRithm's own
    /// clock drops it.
    TrimsPriming,
    /// Windows (Media Foundation, not run) or a Linux Mixxx that was not found or not readable.
    Unknown,
}

#[cfg(test)]
thread_local! {
    static AAC: std::cell::Cell<Option<AacDecoder>> = const { std::cell::Cell::new(None) };
}
/// Tests only: undoes `pretend_aac` when dropped.
#[cfg(test)]
pub(crate) struct PretendAac;
#[cfg(test)]
impl Drop for PretendAac {
    fn drop(&mut self) {
        AAC.set(None);
    }
}
/// Tests only: behave as if the Mixxx on this system decoded AAC with `decoder`, until the
/// returned guard is dropped.
#[cfg(test)]
pub(crate) fn pretend_aac(decoder: AacDecoder) -> PretendAac {
    AAC.set(Some(decoder));
    PretendAac
}

/// Where a Linux Mixxx's executable may be: on the PATH, in the usual places, and as a Flatpak.
fn linux_candidates() -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        found.extend(std::env::split_paths(&path).map(|dir| dir.join("mixxx")));
    }
    for fixed in [
        "/usr/bin/mixxx",
        "/usr/local/bin/mixxx",
        "/opt/mixxx/bin/mixxx",
        "/var/lib/flatpak/app/org.mixxx.Mixxx/current/active/files/bin/mixxx",
    ] {
        found.push(fixed.into());
    }
    if let Some(home) = std::env::var_os("HOME") {
        found.push(
            std::path::Path::new(&home)
                .join(".local/share/flatpak/app/org.mixxx.Mixxx/current/active/files/bin/mixxx"),
        );
    }
    found
}

/// Whether a Mixxx executable has the FAAD decoder: a build with it links `libmp4v2` and loads
/// `libfaad` by name, and a build without it (Debian's) does neither. `None` when the file is
/// not a readable ELF executable, for example a wrapper script.
fn executable_has_faad(path: &std::path::Path) -> Option<bool> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).ok()?;
    if &magic != b"\x7fELF" {
        return None;
    }
    const MARKERS: [&[u8]; 2] = [b"libfaad.so", b"libmp4v2.so"];
    let longest = MARKERS.iter().map(|m| m.len()).max().unwrap_or(0);
    let mut buffer = vec![0u8; 1 << 20];
    let mut carried = 0usize;
    loop {
        let read = file.read(&mut buffer[carried..]).ok()?;
        if read == 0 {
            return Some(false);
        }
        let filled = carried + read;
        if MARKERS
            .iter()
            .any(|marker| buffer[..filled].windows(marker.len()).any(|w| w == *marker))
        {
            return Some(true);
        }
        // keep the end, in case a marker straddles two reads
        carried = (longest - 1).min(filled);
        buffer.copy_within(filled - carried..filled, 0);
    }
}

/// The AAC decoder of the first Mixxx among `candidates` that exists. A candidate that is not there
/// is passed over; one that is there but cannot be read as an executable (a wrapper script that
/// launches some other Mixxx, say) ends the search with `Unknown`, because the Mixxx a later
/// candidate would be is not necessarily the one it starts.
fn aac_decoder_of(candidates: &[std::path::PathBuf]) -> AacDecoder {
    let Some(first) = candidates.iter().find(|path| path.is_file()) else {
        return AacDecoder::Unknown;
    };
    match executable_has_faad(first) {
        Some(true) => AacDecoder::Faad,
        Some(false) => AacDecoder::TrimsPriming,
        None => AacDecoder::Unknown,
    }
}

/// How this system's Mixxx decodes AAC. macOS is CoreAudio (measured). A Linux Mixxx is looked
/// for on disk, once. Windows is Media Foundation, which was not run.
pub(crate) fn aac_decoder() -> AacDecoder {
    #[cfg(test)]
    if let Some(decoder) = AAC.get() {
        return decoder;
    }
    if cfg!(target_os = "macos") {
        return AacDecoder::TrimsPriming;
    }
    if !cfg!(target_os = "linux") {
        return AacDecoder::Unknown;
    }
    static FOUND: std::sync::OnceLock<AacDecoder> = std::sync::OnceLock::new();
    *FOUND.get_or_init(|| aac_decoder_of(&linux_candidates()))
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

#[cfg(test)]
mod aac_tests {
    use super::*;

    fn elf(dir: &std::path::Path, name: &str, body: &[u8]) -> std::path::PathBuf {
        let path = dir.join(name);
        let mut bytes = b"\x7fELF\x02\x01\x01".to_vec();
        bytes.extend_from_slice(&[0u8; 32]);
        bytes.extend_from_slice(body);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn a_mixxx_that_names_libfaad_or_links_mp4v2_has_faad_and_one_that_names_neither_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let built_in = elf(dir.path(), "main", b"..NeAACDecOpen..libfaad.so.2..");
        let linked = elf(
            dir.path(),
            "linked",
            b"NEEDED libmp4v2.so.2 libQt6Core.so.6",
        );
        let debian = elf(dir.path(), "debian", b"NEEDED libavcodec.so.61 libmad.so.0");
        assert_eq!(executable_has_faad(&built_in), Some(true));
        assert_eq!(executable_has_faad(&linked), Some(true));
        assert_eq!(executable_has_faad(&debian), Some(false));
    }

    #[test]
    fn a_marker_split_across_two_reads_is_still_found() {
        let dir = tempfile::tempdir().unwrap();
        let mut body = vec![b'x'; (1 << 20) - 36 - 4];
        body.extend_from_slice(b"libfaad.so.2");
        let path = elf(dir.path(), "big", &body);
        assert_eq!(executable_has_faad(&path), Some(true));
    }

    #[test]
    fn missing_candidates_are_passed_over_and_the_first_one_that_exists_decides() {
        let dir = tempfile::tempdir().unwrap();
        let faad = elf(dir.path(), "faad", b"libfaad.so.2");
        let ffmpeg = elf(dir.path(), "ffmpeg", b"libavcodec.so.61");
        let missing = dir.path().join("absent");
        assert_eq!(
            aac_decoder_of(&[missing.clone(), faad.clone(), ffmpeg.clone()]),
            AacDecoder::Faad
        );
        assert_eq!(
            aac_decoder_of(&[missing.clone(), ffmpeg, faad]),
            AacDecoder::TrimsPriming
        );
        assert_eq!(aac_decoder_of(&[missing]), AacDecoder::Unknown);
        assert_eq!(aac_decoder_of(&[]), AacDecoder::Unknown);
    }

    #[test]
    fn a_wrapper_script_that_exists_ends_the_search_because_it_may_start_any_mixxx() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("mixxx");
        std::fs::write(&script, b"#!/bin/sh\nexec mixxx.bin \"$@\"\n").unwrap();
        assert_eq!(executable_has_faad(&script), None);
        let faad = elf(dir.path(), "faad", b"libfaad.so.2");
        let ffmpeg = elf(dir.path(), "ffmpeg", b"libavcodec.so.61");
        // the wrapper first on the PATH: whichever real Mixxx comes after it, none is known to run
        assert_eq!(
            aac_decoder_of(&[script.clone(), faad.clone()]),
            AacDecoder::Unknown
        );
        assert_eq!(
            aac_decoder_of(&[script.clone(), ffmpeg.clone(), faad]),
            AacDecoder::Unknown
        );
        // a missing candidate before it changes nothing
        assert_eq!(
            aac_decoder_of(&[dir.path().join("absent"), script, ffmpeg]),
            AacDecoder::Unknown
        );
    }

    /// Run by `tools/mixxx-acceptance/run.py` on the real executables it copies out of the
    /// containers: a Mixxx built with FAAD and/or one built without, whichever it has.
    #[test]
    #[ignore = "needs real Mixxx executables; run through tools/mixxx-acceptance/run.py"]
    fn real_mixxx_executables_are_told_apart() {
        let path = |name: &str| std::env::var_os(name).map(std::path::PathBuf::from);
        let with = path("CUERITHM_MIXXX_WITH_FAAD");
        let without = path("CUERITHM_MIXXX_WITHOUT_FAAD");
        assert!(
            with.is_some() || without.is_some(),
            "name a Mixxx executable"
        );
        if let Some(with) = with {
            assert_eq!(executable_has_faad(&with), Some(true), "{with:?}");
            assert_eq!(aac_decoder_of(&[with]), AacDecoder::Faad);
        }
        if let Some(without) = without {
            assert_eq!(executable_has_faad(&without), Some(false), "{without:?}");
            assert_eq!(aac_decoder_of(&[without]), AacDecoder::TrimsPriming);
        }
    }
}
