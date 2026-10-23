# How are audio files decoded?

<p class="lead">CueRithm bundles its own decoder, so you never install FFmpeg, Python or a codec pack. Every format is turned into the same playable timeline before anything is measured.</p>

!!! abstract "What this means for you"
    - **MP3, AAC (ADTS and M4A), ALAC, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA all take the same path**, including high-bit-depth and float WAV. Nothing extra to install on macOS, Windows or Linux.
    - **A cue at 3:07 in the player is the same sample the analyser measured**, whatever the format.
    - **Tracks longer than about 24 minutes fail to decode** and stay in Incoming. Long DJ mixes do not prepare. The decoder also stops a file that takes more than five minutes to decode.
    - Check a file before you add it with [Check that your files are supported](../files/supported-formats.md).

## The bundled FFmpeg { #the-bundled-ffmpeg }

| | |
|---|---|
| **Version** | FFmpeg and ffprobe 9.0.2, built from the official source archive with a pinned SHA-256 |
| **Licence** | LGPL-2.1-or-later. No GPL or non-free components, no external codec libraries |
| **Linking** | Internal libraries statically linked into two standalone executables, `ffmpeg` and `ffprobe` |
| **Network** | Disabled. Only the `file` and `pipe` protocols exist |

It is built with autodetection, networking and everything else switched off, then only the parts CueRithm needs are switched back on.

| Component | Enabled |
|---|---|
| **Demuxers** | aac, aiff, asf, flac, mov, mp3, ogg, wav |
| **Parsers** | aac, aac_latm, flac, mpegaudio, opus, vorbis |
| **Decoders** | aac, alac, flac, mp3, opus, vorbis, wmav1, wmav2, wmapro, wmalossless, and PCM in 8, 16, 24, 32 and 64-bit, float, A-law and mu-law |
| **Encoders and muxers** | 32-bit float and 16-bit PCM, `image2pipe` (for artwork) and `md5` (for fingerprints) |
| **Filters** | aresample, aformat, anull |

## What decoding produces { #what-decoding-produces }

CueRithm decodes each track with these settings and checks:

- the first audio stream only;
- two channels at 44,100 Hz, using the software resampler with no dither;
- output as 32-bit float little-endian PCM;
- any decode error fails the track, so a damaged file is reported rather than half-analysed;
- the file is copied to a private snapshot and hashed before and after, so a half-written file is never read.

Limits: the probe accepts 8,000 to 384,000 Hz and 1 to 32 channels. Output is capped at 64,000,000 frames (about 24 minutes 11 seconds at 44.1 kHz), and each external process has a five-minute timeout and is stopped on cancel. A longer track fails and stays in Incoming.

The analysis engine then derives its inputs from that PCM: a mono mix for key and energy, the left channel for BPM and cues, and a 22.05 kHz copy for energy. For 16-bit 44.1 kHz stereo FLAC, the engine reads the private FLAC snapshot directly instead of a converted copy (its validated file profile). The player, the waveform and tag verification still use the converted PCM.

## One clock for everything { #one-clock-for-everything }

Analysis, the waveform, tag verification and playback all read the same decoded PCM. A test decodes the same audio stored as MP3, AAC (ADTS and M4A), ALAC, FLAC, WAV (16-bit, 24-bit and float), AIFF, Ogg Vorbis, Opus and WMA, plus mono 48 kHz and stereo 96 kHz WAV. Every file decodes to the expected frame count. The lossless encodings produce byte-identical PCM.

That is the clock inside CueRithm. DJ software may count samples differently for MP3 and M4A because of encoder delay, so CueRithm also measures that delay for each file and keeps it with the analysis. The measurement is used when you sync and never changes the clock in CueRithm. See [Preview, apply and restore a sync](../sync/preview-apply-restore.md).

## Sealing and verification { #sealing-and-verification }

At build time a script called `seal.py` checks the runtime:

1. all 14 required decoders exist (aac, alac, flac, mp3float, opus, vorbis, wmav1, wmav2, wmapro, wmalossless, pcm_s16le, pcm_s16be, pcm_s24le, pcm_f32le);
2. the PCM and artwork muxers exist;
3. the executables link only against operating-system libraries, from an allow-list per platform;
4. it writes `runtime.json` with the manifest and the SHA-256 of `ffmpeg` and `ffprobe`.

Every time the app starts it recomputes both hashes. A mismatch stops the app with *Decoder hash mismatch*, *Probe hash mismatch* or *Bundled decoder manifest is missing*. The decoder's identity (recipe plus both hashes) is stored with analysis results and waveform caches, and tag staging requires the same decoder identity as analysis used. If the app will not start, see [My computer blocks CueRithm at first launch](../help/troubleshooting/first-launch-blocked.md).

`cuerithm-desktop --diagnostics` resolves the bundled resources, verifies the media runtime and the embedded key model, and prints one JSON line with the version and decoder identity. See [Download and install CueRithm](../get-started/install.md).

## How playback uses the decoded audio { #playback }

Playback uses the system's default output device. There is no device picker. The player plays the decoded PCM through a small audio library on its own thread, so seeking is sample-accurate and a seek past the end is rejected. The default volume is 0.65. If the output stream fails, playback stops and an error is shown. If a track shows a dash or will not play, see [A track shows a dash or will not play](../help/troubleshooting/no-waveform-or-playback.md).

## Where you see it in the app { #where-you-see-it }

| What | Where |
|---|---|
| A file that cannot be decoded | A row in **Incoming** with a warning icon and an *Audio decoder failed* message |
| Decoding in progress | The *Decoding* stage under the track title in **Incoming** |
| Playback | The player at the bottom of the window: see [Preview a track with the player](../analysis/preview-a-track.md) |

## Related

- [Check that your files are supported](../files/supported-formats.md)
- [How does CueRithm analyse a track?](pipeline.md)
- [How is the waveform drawn?](waveform.md)
- [Fix a track that needs attention](../files/needs-attention.md)

<!-- verified: tools/media-runtime/manifest.json, build.sh, seal.py; crates/cuerithm-media/src/decode.rs (snapshot hashing, -ac 2 -ar 44100, pcm_f32le, 64_000_000 frames, 300 s), probe.rs (8000..=384000 Hz, 1..=32 channels), runtime.rs (from_bundle hash errors), export_timing.rs (dj-timeline-v1, MP3 and ISO audio delay); crates/camelot-core/src/engine.rs (legacy_profile FLAC path, input rates), prepared.rs; crates/cuerithm-media/tests/decoding.rs + tools/fixtures/media.tar.gz (14 fixture encodings); crates/cuerithm-library/src/playback.rs (default device, volume 0.65, seek outside the track rejected); apps/desktop/src-tauri/src/lib.rs (diagnostics output); THIRD_PARTY_NOTICES.md; legacy technical/decoding.md re-checked. Dropped from legacy: the claim about which CI jobs run --diagnostics (not verifiable here) -->
