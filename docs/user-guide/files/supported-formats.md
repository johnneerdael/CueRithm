# Check that your files are supported

<p class="lead">CueRithm prepares ten kinds of audio file with a decoder that ships inside the app, so there is nothing to install. This page lists the formats, the limits that can stop a file, and how to check a file before you add it.</p>

## Which formats work { #formats }

All of these are decoded, analysed and played back in the same way, and all of them get the same key, BPM, beatgrid, energy and cue results.

| Format | Usual extensions | Notes |
|---|---|---|
| MP3 | `.mp3` | |
| AAC in ADTS | `.aac` | Raw AAC streams |
| AAC in M4A | `.m4a` | |
| ALAC in M4A | `.m4a`, `.alac` | Apple Lossless |
| FLAC | `.flac` | The demo library's 26 purchases are FLAC |
| WAV | `.wav`, `.wave` | PCM at 8, 16, 24, 32 and 64 bits, floating point, A-law and µ-law |
| AIFF | `.aiff`, `.aif`, `.aifc` | |
| Ogg Vorbis | `.ogg`, `.oga` | |
| Opus | `.opus` | `.ogg` files can also hold Opus |
| WMA | `.wma` | WMA 1, 2, Pro and Lossless |

You do not need FFmpeg, a codec pack or any other runtime. CueRithm checks its bundled decoder every time it starts. [How are audio files decoded?](../how-it-works/decoding.md) explains the details.

Catalog tagging, cleanup and the metadata editor write tags into the same formats; see [Auto tagging](../tagging/index.md).

## How CueRithm decides a file is audio { #how-files-are-recognised }

There are two rules, depending on how the file arrives.

| How you add it | What decides |
|---|---|
| A [watch folder](watch-folders.md), or a **folder** you drop on the window | The extension. Only files ending in `mp3`, `aac`, `m4a`, `alac`, `flac`, `wav`, `wave`, `aif`, `aiff`, `aifc`, `ogg`, `oga`, `opus` or `wma` are picked up, in any letter case. Files whose names start with a dot are skipped, and links are not followed. |
| **Add tracks**, or files you drop **one by one** | The content. The extension is not checked, so a file with the wrong suffix still prepares if it really is audio. A file that is not decodable fails with a message. |

A file that sits in a watch folder with an unlisted extension is ignored, silently. Add it with **Add tracks** instead; see [Add tracks by dropping, picking or watching a folder](add-tracks.md).

## Limits that can stop a file { #limits }

| Limit | Value | What you see |
|---|---|---|
| Length | Decoding stops at about 24 minutes 11 seconds (64 million frames at 44.1 kHz) | *Media scan exceeded its output limit*. Long DJ mixes cannot be prepared. |
| Decoding time | Five minutes per file | *Media scan exceeded its time limit* |
| Sample rate | 8,000 to 384,000 Hz | *Unsupported audio rate/channel layout* |
| Channels | 1 to 32 | The same message |
| Audio streams | The first audio stream is used | Other streams are ignored |
| Protected files | Files locked by digital rights management cannot be decoded | *Audio decoder failed: …* |
| Damaged files | Any decode error fails the file; CueRithm does not skip over damage | *Audio decoder failed: …* |

The longest track in the demo library is *Undercurrent (Original Mix)* at 6:41, well inside the limit.

CueRithm analyses a stereo copy at 44.1 kHz. Your file stays as it is. A file at 96 kHz or in mono gets the same result types, and cue times are positions in the track, so they line up with the audio you hear.

## Check a file before you add it { #check-a-file }

1. Try the extension against the list above.
2. Add a few files first, not a whole library. Rows that fail stop in Incoming with a message you can read in [Fix a track that needs attention](needs-attention.md).
3. For files already in the library, open **Maintenance → Missing & broken**, tick **Inspect audio decoding** and select **Scan library files**. The report separates files that are unsupported, time out or fail to decode. See [Find missing, moved or broken files](library/find-missing-files.md).

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| A folder you watch stays empty | The files use extensions the folder scan does not pick up | Rename them correctly, or add them with **Add tracks** |
| *Audio decoder failed* on one file only | That file is damaged or protected | Replace it with a good copy |
| A long mix never prepares | It is over the length limit | Split it, or keep it out of CueRithm |

## Related

- [Add tracks by dropping, picking or watching a folder](add-tracks.md)
- [Follow a track through Incoming](follow-incoming.md)
- [Fix a track that needs attention in Incoming](needs-attention.md)
- [How are audio files decoded?](../how-it-works/decoding.md)
- [What CueRithm does not do yet](../help/not-available-yet.md)

<!-- verified: crates/cuerithm-library/src/watch.rs (supported_path extension list); apps/desktop/src/api.ts (chooseFiles filters); crates/cuerithm-media/src/decode.rs (44.1 kHz stereo float copy, 64,000,000 frame output cap, 300 s timeout, -err_detect explode, first audio stream); crates/cuerithm-media/src/probe.rs (8000 to 384000 Hz, 1 to 32 channels); crates/cuerithm-media/src/cancel.rs (limit messages); crates/cuerithm-media/tests/decoding.rs (wrong suffix decodes by content; truncated files fail); crates/cuerithm-autotag/src/input.rs (audio_format covers FLAC, MP3, MP4/ALAC, AAC, OGG, WMA, AIFF, WAV); apps/desktop/src/views/MaintenanceBroken.tsx (Inspect audio decoding); docs/release-readiness.md (format list); legacy technical/decoding.md re-checked. UNVERIFIED: the decoder list is taken from the legacy technical page and the seal script, not re-run here.-->
