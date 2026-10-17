# Offline analysis options

Research date: 2026-10-16. This is a source review, not a locally reproduced
accuracy or performance benchmark. The target application is Tauri 2 with a
Rust backend on macOS, Windows and Linux.

The user subsequently supplied 434 audio files and a reference CSV. Their
embedded Mixed In Key tags provide key, energy and cue reference labels for
all files. See `docs/research/reference-corpus.md`. The unrecovered server
classifier does not prevent supervised calibration/training or measured
agreement against those outputs. It only limits claims of implementation
recovery and exact behavior beyond what the reference evaluation establishes.

## Recommendation

Separate the application from the analysis engines. Return tonic and mode,
tempo candidates, beat timestamps, engine identity and uncertainty through a
versioned Rust result type. Compute Camelot labels in our own tested mapping.

For keys, compare an independent small Rust DSP baseline, stratum-dsp, and
libkeyfinder on the same annotated audio before selecting a production engine.
For beats and tempo, benchmark the small Beat This! model using beat-this-rs
against the classical baseline. Do not claim Mixed In Key accuracy parity.

The app can operate offline after any selected model files are bundled. Audio,
features and license information do not need to be sent to a service.

## Key detection

| Candidate | How it works | Integration and limits |
| --- | --- | --- |
| Independent Rust DSP | Spectral peaks, tuning estimation, pitch-class profiles, correlations against major/minor profiles | Small and portable; needs measured accuracy on music, not just synthetic chords |
| libkeyfinder | Chromagram extraction and a 24-class profile classifier | Mature C++ library; requires C/C++ bridge and FFTW; GPL-3.0-or-later |
| Essentia KeyExtractor | Spectral peaks and whitening, HPCP, tuning correction and configurable key profiles | No neural model needed for this algorithm; larger C++ build; source AGPLv3 |
| stratum-dsp | Pure Rust chroma/profile key estimation | MIT/Apache-2.0; benchmark before adoption; provide our own Camelot labels |
| MusicalKeyCNN | PyTorch CNN over CQT features | Code MIT; checkpoint provenance/redistribution and export/preprocessing compatibility need further verification |
| InceptionKeyNet | Research CNN and training/augmentation system | MIT code; not an established ready-to-ship Rust/ONNX package |
| madmom key CNN | Log-frequency spectrogram and CNN ensemble | BSD code; pretrained models/data separately CC BY-NC-SA 4.0 |
| Deezer S-KEY | VQT frontend and ChromaNet, with a small community end-to-end ONNX export | Evaluated locally through Rust RTEN; explicit pinned weights, upstream code MIT, checkpoint terms not separately stated |

Sources:

- [libkeyfinder upstream](https://github.com/mixxxdj/libkeyfinder),
  [preprocessing](https://raw.githubusercontent.com/mixxxdj/libkeyfinder/main/src/keyfinder.cpp),
  [classifier](https://raw.githubusercontent.com/mixxxdj/libkeyfinder/main/src/keyclassifier.cpp).
- [Essentia KeyExtractor](https://essentia.upf.edu/reference/std_KeyExtractor.html),
  [tonal tutorial](https://essentia.upf.edu/tutorial_tonal_hpcpkeyscale.html),
  [source/model licensing distinctions](https://essentia.upf.edu/licensing_information.html),
  [source license](https://github.com/MTG/essentia/blob/master/COPYING.txt).
- [stratum-dsp crate](https://docs.rs/crate/stratum-dsp/latest),
  [actual key mapping](https://raw.githubusercontent.com/HLLMR/stratum-dsp/main/src/analysis/result.rs),
  [project evaluation](https://raw.githubusercontent.com/HLLMR/stratum-dsp/main/docs/literature/stratum_2025_key_detection_real_world.md).
- [MusicalKeyCNN](https://github.com/a1ex90/MusicalKeyCNN),
  [preprocessing](https://raw.githubusercontent.com/a1ex90/MusicalKeyCNN/main/predict_keys.py),
  [code license](https://raw.githubusercontent.com/a1ex90/MusicalKeyCNN/main/LICENSE).
- [InceptionKeyNet](https://github.com/stefan-baumann/inceptionkeynet),
  [ISMIR paper](https://archives.ismir.net/ismir2021/paper/000004.pdf).
- [madmom key pipeline](https://raw.githubusercontent.com/CPJKU/madmom/main/madmom/features/key.py),
  [code/model licenses](https://raw.githubusercontent.com/CPJKU/madmom/main/LICENSE).
- [S-KEY upstream](https://github.com/deezer/skey),
  [pinned inference/mapping](https://github.com/deezer/skey/blob/918b83d273568d5041569bb8068843d19a335726/skey/key_detection.py),
  [community ONNX conversion and provenance](https://huggingface.co/aaatmy/skey-onnx).

The evaluated S-KEY export is 324,536 bytes and takes 22050 Hz mono PCM;
peak normalization, VQT/dB/cropping and classification are contained in its graph.
It reports major classes starting at A, and minor classes starting at B. The
source mapping was checked against the pinned upstream implementation, and all
24 Camelot conversions have independent contract tests. Public hashes/notices
are in `models/skey/`; weights remain external and experimental. See the local
measurement report for actual agreement and numerical-parity limitations.

### Candidate cautions

stratum-dsp's `numerical()` maps C major to `1A` and A minor to `1B` in the
reviewed source. Camelot notation requires `8B` and `8A`, respectively. Use
tonic/mode, never trust that numeric label as Camelot. Its published key
evaluation uses only 68 tracks and vendor tags as reference labels; this is
insufficient to establish accuracy parity with the purchased app.

[key-detect](https://github.com/rohan-singhvi/key-detect) is an informative Rust
experiment using harmonic/percussive separation, tuning and multiple chroma
views. No root license file or Cargo package license was found in the reviewed
repository, and its README states that tests are not implemented. Do not copy
or vendor it without resolving its license. Its published dataset experiments
also guide its tuning; they are not a clean held-out evaluation.

## Tempo, beats and downbeats

| Candidate | Outputs | Packaging |
| --- | --- | --- |
| Rust onset/autocorrelation baseline | Tempo candidates; beat phase only with additional tracking | Small, transparent and no model download |
| beat-this-rs | Beat/downbeat timestamps and BPM | Default pure-Rust rten inference; explicit model files; no Python runtime |
| Official Beat This! | Beat/downbeat timestamps | Python/PyTorch; useful reference implementation |
| aubio | Tempo and beat tracking | Portable C library with Rust FFI; GPL-3.0-or-later |
| Essentia RhythmExtractor2013 | BPM, beat ticks, intervals, estimates and confidence | C++; documented input rate 44.1 kHz; AGPL source |
| BeatNet | Beat/downbeat/tempo/meter | Python dependencies; source repo CC BY 4.0; per-checkpoint licensing should be verified |

[beat-this-rs](https://github.com/danigb/beat-this-rs) supplies a roughly 10 MB
small ONNX beat model and 270 KB mel frontend in its repository; the full model
is roughly 83 MB. The crates.io package does not include these files, so an app
must package pinned, checksummed model assets explicitly. The library supports
streaming decoded mono chunks; memory still grows with its retained mel data.

The Rust port and [official Beat This!](https://github.com/CPJKU/beat_this) state
MIT licensing, with the official project explicitly covering published model
weights. The official project separately notes training-audio rights. Preserve
the applicable notices. The Rust port's Python parity test is useful evidence
about the port on a shared fixture, not a general accuracy benchmark. Its
Windows/macOS/Linux packaging needs our own verification.

Additional sources:

- [beat-this-rs manifest](https://raw.githubusercontent.com/danigb/beat-this-rs/main/Cargo.toml).
- [aubio](https://github.com/aubio/aubio/blob/master/README.md).
- [Essentia RhythmExtractor2013](https://essentia.upf.edu/reference/std_RhythmExtractor2013.html).
- [BeatNet](https://github.com/mjhydri/BeatNet).
- [Classical onset/tempo/beat decomposition](https://librosa.org/doc/0.11.0/generated/librosa.beat.beat_track.html).

BPM describes periodicity; beat times add phase; downbeats identify bar starts.
A BPM value alone does not justify drawing a reliable beatgrid or selecting cue
points. Preserve half/double-time alternatives and avoid silently forcing all
tracks into one DJ tempo range.

## Evaluation before engine selection

1. Use one decoder/resampler policy for all comparable engines.
2. Split tracks used to choose parameters from held-out evaluation tracks.
3. Use [GiantSteps key](https://github.com/GiantSteps/giantsteps-key-dataset)
   and [tempo](https://www.cp.jku.at/datasets/giantsteps/) annotations for EDM,
   supplemented by independently annotated, genre-diverse music.
4. Report exact tonic+mode accuracy separately from
   [MIREX weighted key scoring](https://music-ir.org/mirex/wiki/2017%3AAudio_Key_Detection_Results).
5. Report exact tempo and metrical alternatives separately; for beat timing,
   use [mir_eval beat](https://mir-eval.readthedocs.io/stable/api/beat.html)
   F-measure and continuity measures.
6. Respect train/test separation for pretrained models; official Beat This!
   documents fold-specific checkpoints and a GTZAN holdout.
7. Test silence, noise, short clips, corrupt audio, tuning offsets, tempo
   changes, breakdowns, long recordings, sample rates and relative-key ambiguity.
8. Measure elapsed time and peak memory on each desktop architecture.

Correlation margins and softmax outputs are not calibrated correctness
probabilities. Preserve manual corrections independently of cached analysis.

## Relationship to the inspected app

The original app's JSON schema is documented in
`docs/research/analysis-service-contract.md`. Those vendor-specific feature
vectors are not interchangeable with an arbitrary offline model's input.
Decode source audio and compute the selected engine's documented features.
Reuse conceptual output fields, not the vendor's service, authentication or
unrecovered classifier.
