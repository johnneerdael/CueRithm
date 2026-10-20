# Decoding and the media runtime

CueRithm bundles its decoder, so you do not need to install FFmpeg, Python or a codec pack. Every format uses the same playable timeline.

## The bundled FFmpeg

| | |
| --- | --- |
| **Version** | FFmpeg and ffprobe 9.0.2, built from the official source tarball with a pinned SHA-256 |
| **Licence** | LGPL-2.1-or-later. No GPL or non-free components, no external codec libraries. |
| **Linking** | Internal libraries statically linked into two standalone executables, `ffmpeg` and `ffprobe` |
| **Network** | Disabled. Only the `file` and `pipe` protocols exist. |

It is built with `--disable-autodetect --disable-network --disable-everything` and then only the parts CueRithm needs are switched back on.

| Component | Enabled |
| --- | --- |
| **Demuxers** | aac, aiff, asf, flac, mov, mp3, ogg, wav |
| **Parsers** | aac, aac_latm, flac, mpegaudio, opus, vorbis |
| **Decoders** | aac, alac, flac, mp3 (float), opus, vorbis, wmav1, wmav2, wmapro, wmalossless, and PCM in 8, 16, 24, 32 and 64-bit, float, A-law and mu-law |
| **Encoders / muxers** | 32-bit float and 16-bit PCM, `image2pipe` (for artwork) and `md5` (for fingerprints) |
| **Filters** | aresample, aformat, anull |

That covers MP3, AAC (ADTS and M4A), ALAC, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA, including high-bit-depth and float PCM.

## What decoding produces

CueRithm decodes each track with these settings and checks:

- first audio stream only;
- `-ac 2 -ar 44100`, using the software resampler with no dither (`aresample=44100:resampler=swr:dither_method=none`);
- output as 32-bit float little-endian PCM;
- `-err_detect explode -xerror`: any decode error fails the track;
- the file is first copied to a private snapshot and hashed before and after, so a half-written file is never read.

Limits: the probe accepts 8,000 to 384,000 Hz and 1 to 32 channels. Output is capped at 64,000,000 frames (about 24 minutes 11 seconds at 44.1 kHz) and 300 seconds of run time. Each external process has a five-minute timeout and is killed on cancel. Longer tracks, such as long DJ mixes, fail and stay in Incoming.

## One clock for everything

Analysis, the waveform, tag verification and playback all consume that same decoded PCM. A cue at 3:07.3 in the player is the same sample the analyser saw, in every format, with no codec-delay differences between MP3, AAC and FLAC.

## Sealing and verification

At build time a script called `seal.py` checks the runtime:

1. all 14 required decoders exist (`aac`, `alac`, `flac`, `mp3float`, `opus`, `vorbis`, `wmav1`, `wmav2`, `wmapro`, `wmalossless`, `pcm_s16le`, `pcm_s16be`, `pcm_s24le`, `pcm_f32le`);
2. the PCM and artwork muxers exist;
3. the executables only link against operating-system libraries, from an allow-list per platform (system libraries and frameworks on macOS, libc and friends on Linux, core Windows DLLs on Windows);
4. it writes `runtime.json` with the manifest plus the SHA-256 of `ffmpeg` and `ffprobe`.

At every app start `MediaRuntime::from_bundle` recomputes both hashes. A mismatch stops the app with *Decoder hash mismatch*, *Probe hash mismatch* or *Bundled decoder manifest is missing*. The runtime's identity (recipe plus both hashes) is stored with analysis results, waveform caches and tag-write journals, and tag staging requires the same decoder identity as analysis did.

## `--diagnostics`

`cuerithm-desktop --diagnostics` resolves the platform resource folder, verifies the bundled media runtime and the embedded key model and prints one JSON line. See [Install](../get-started/install.md#check-that-the-install-is-healthy). The build pipeline runs it against the real macOS app, the extracted Linux package and the staged Windows folder.

## Playback

Playback uses the system's default output device (no picker), through a small audio library on its own thread. It plays the cached PCM, so seeking is sample-accurate within it and a seek past the end is rejected. The default volume is 0.65. If the output stream fails, playback stops and an error is shown instead of jumping to the end.
