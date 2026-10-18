# Cuerithm

An offline Rust engine for DJ track analysis: musical key, BPM, structural cues
and one energy rating per track, with a Tauri 2 desktop preparation workspace.
The canonical repository is
[johnneerdael/Cuerithm](https://github.com/johnneerdael/Cuerithm).

Cues come exclusively from the validated Lexicon-compatible DSP: Start, Drop,
Breakdown, Second Drop, Second Breakdown, Last beat and an optional emergency
loop. Names, colors and loop information are retained in `analysis.lexicon_cues`;
the existing `analysis.cues` channel carries the same cue names/times for exports.
Mixed In Key cue inference and per-cue/per-segment energy labels are retired.

## Build and analyze

```sh
cargo build -p camelot-cli --release
cargo run --release -p camelot-cli -- analyze "/path/to/track.wav" --lexicon-cues
```

The compatibility binary is still named `camelot` (`camelot.exe` on Windows).
Analysis reads PCM and explicit configuration; existing tags and filenames are
not prediction inputs. Original audio is never retagged. `--lexicon-genre` is an
explicit optional hint. CLI defaults use a 64-beat minimum breakdown, Never for
drop-at-start and no emergency loop; `--emergency-loop` enables a 16-beat loop.
The library retains the original `LexiconConfig::default()` values.

The key and BPM backends are preserved. OpenKeyScan is optional and requires
CMake and a C compiler at build time. Supply model files explicitly; analysis
does not download models or call an analysis service:

```sh
cargo run --release -p camelot-cli --features openkeyscan-key -- analyze "/path/to/track.flac" \
  --key-openkeyscan-model "/path/to/openkeyscan3-logits.onnx" \
  --source-profile-timing --lexicon-cues \
  --energy-model "/path/to/energy-svc.json"
```

`--source-profile-timing` and `--energy-model` currently require a complete
44100 Hz stereo signed16-bit FLAC within the supported 64000000-frame limit.
Other enabled codecs remain available through the general key/tempo/cue path.
Unsupported explicit profiles fail rather than silently changing algorithms.
`--max-seconds` marks an excerpt partial and cannot request a whole-track rating.

With a supplied energy model, `analysis.energy` contains only `{level, method}`.
Internal SVC window decisions are aggregated by total duration (highest level
wins an exact tie). This retains the client-side track-tag rule; it does not
claim recovery of the separate server OverallEnergy formula. No per-segment
energy timeline or cue-energy labels are emitted. Without a model, energy is
omitted. See [energy model setup](models/energy/README.md).

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --exclude cuerithm-desktop --exclude cuerithm-library --all-targets --all-features -- -D warnings
cargo test --workspace --exclude cuerithm-desktop --all-features --release
```

The upstream 47 cue tests cover DSP primitives, sections, drops, breakdowns,
outro/loop behavior, configuration and serialization. Integration tests cover
44100 Hz cue routing, source BPM/anchor reuse, the single energy result and
invalid/unknown inputs. Cues requested without usable timing produce an empty
cue result. The original cue calculations remain unchanged behind a checked
application boundary.

Captured traffic, music, reference labels, installers and private
research artifacts stay outside Git. Older MiK cue studies are historical;
they are not an active Cuerithm cue backend. See
[integration notes](docs/cuerithm-integration.md),
[OpenKeyScan setup](models/openkeyscan/README.md) and
[third-party notices](THIRD_PARTY_NOTICES.md).

## Desktop preparation

The desktop workspace adds Tracks, automatic Incoming preparation, Archive,
persistent Settings and native waveform/cue preview. Watch folders run full key,
BPM, beatgrid, scalar energy and cue analysis, then promote successful tracks
automatically. A completed replacement retains the previous cue revision;
failed analysis keeps the last valid cues. Original audio stays in place.

The bundled decoder supports MP3, AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF,
Ogg Vorbis, Opus and WMA, including high-depth/float PCM. All share the decoded
playable timeline. The desktop's prepared-input API generalizes the explicit
CLI FLAC-profile restriction described above; it does not change that CLI API.
Protected media is unsupported. Users need no separate FFmpeg or Python runtime.

OpenKeyScan key detection, track energy and structural cue analysis are built
into the desktop application. No model selection, downloads or extra setup is
required. Pinned parameters are embedded, checked and cached automatically;
existing unfinished jobs migrate to the built-in revision on upgrade. The
standalone CLI retains its explicit-model API. The app never invents results
to mark an incomplete track ready.

Beatport/Traxsource tagging, cleanup, ordinary/smart playlists and direct rekordbox
6/7 sync are subsequent approved deliveries; their desktop controls are not yet
enabled. No desktop XML fallback exists. This foundation is an unsigned test
build, not a signed public release or a complete product-parity claim.

See [desktop development and acceptance](docs/development/desktop.md) for setup,
commands, platform prerequisites and artifact verification.
