# Lexicon beatgrid investigation

Inspected 2026-10-18 with leanrea. The installed Lexicon application, source
music and application libraries were preserved. Private extraction and probes
remain outside published source. Source observations and original-worker
experiments below do not establish a completed Rust port.

## Current Cuerithm behavior

The desktop's `BuiltinModels::apply` enables `source_profile_timing`. Its BPM,
grid and anchor therefore come from Cuerithm's independent reconstruction of
MIK's local timing pipeline. `Engine::analyze` supplies that BPM and anchor to
the independent Lexicon-compatible cue generator. Lexicon cues and the local
Lexicon acoustic energy meter do not imply Lexicon beatgrid parity.

The recorded 282-track comparison validates MIK BPM agreement; exact full-grid
agreement was checked on five captured cases. See
[source timing](source-timing-engine.md). Neither result proves the grid agrees
with Lexicon or with human musical annotation.

## Seen: installed source path

Lexicon 1.11.15.0 has ASAR SHA-256
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
The worker `renderer/main_window/182.index.worker.js` has SHA-256
`3687c1689cfa8e9e693c69434924e05bd4e5e1b1356da0304a414ef9bf90a98f`.
Anchors below refer to the retained pretty-printed extraction; it is not
published with this repository.

- Renderer `AudioAnalyzer.analyzeTrackInWorker`, lines 238468–238740, passes
  the entire decoded buffer's **first channel**, at its buffer sample rate,
  as the raw PCM argument. The low-pass copy is a different argument used by
  cue analysis; it is not the input to the timing entry point.
- Renderer `re`, lines 238356–238365, reads the configured tempo range and
  clamps the lower bound to at least 50 BPM and upper bound to at most 300 BPM.
  The persisted setting's default is `98_195` (line 396338). The internal
  detector's fallback bounds differ; do not confuse them with renderer defaults.
- Worker message handler, lines 18699–18722, calls module 358 export `Wi`
  with raw PCM, sample rate and the provided bounds. It assigns the returned
  `tempo` and `firstBeat` before calling the cue generator.
- Renderer result publication, lines 238714–238722, creates **one tempo
  marker**: BPM rounded to two decimal places and start time set to
  `firstBeat`. The internal beat chain is not saved as a variable-tempo map.
  Cue generation in the same worker uses its unrounded tempo result.

## Seen: detector components

The entry point is module 358, lines 11311–13124. It combines local DSP with
many explicit correction rules. It does not fetch a model or call a service.

| Module | Observed responsibility | Pretty-print lines |
| --- | --- | --- |
| 745 | Adaptive energy-change peak extraction, inter-peak tempo voting, harmonic candidates, long-lag correlation and fractional/integer refinement | 13126–13423 |
| 974 | Attack ratio, audio onset, precise drop onset and grid snapping primitives | 6548–6702 |
| 522 | Later-beat calibration, half-beat checks, silent/weak phase rescue, riser validation and grid correlation | 6704–7759 |
| 809 | Spectral-flux onset envelope, dynamic-programming beat chain and guarded phase correction | 7761–7984 |
| 473 | Drop, drum-and-bass structural anchor and immediate-start helpers | 7986–8092 |
| 578 | Intro detection | 9934–10032 |
| 40 | Detector defaults and debug-field stripping | 10034–10066 |
| 382 | Walk back from an anchor toward the first beat, with grid/attack checks | 10068–11309 |
| 358 | Tempo-to-anchor decision tree and final correction sequence | 11311–13124 |

For example, module 809 uses a 512-sample FFT, 128-sample hop and 24 logarithmic
bands, then searches a beat chain around the supplied tempo. The chain can
correct the phase when the existing grid is weak. This is local dynamic
programming, not an external beat-model file. The default path also performs
intro/drop-based anchor selection, subsequent-beat corrections, half-beat
correction, quiet-pickup handling and final grid validation. Porting only the
tempo histogram or this DP stage would omit behavior actually used by the app.

**Inference:** matching the entire local timing path should remove one source
of cue differences introduced by the current mixed timing/cue pipeline.
Whether it improves musical correctness or speed requires separate measurement.
`firstBeat` is the app's grid anchor; this trace does not prove a separate
musically annotated downbeat detector or a complete downbeat sequence.

## Observed: isolated original-worker experiments

The shipped code ran inside a fresh isolated VM context per synthetic input,
using the installed application's already-enabled Electron RunAsNode mode.
No bundle, fuse, application data or license was changed. The original worker
message boundary and a privately exposed unchanged tempo helper were measured
separately. The runtime was Electron 32.3.3 / Node 20.18.1 / V8 12.8.

Eight original 32-second, 44100 Hz synthetic recipes now have numeric tempo,
peak and final-anchor references. The independently regenerated PCM hashes
are checked by `tests/fixtures/lexicon-timing/generate.py`. Cases include
120/128/174 BPM, fractional tempo, offbeat hats, a quiet intro, an input below
the default range and a zero anchor. The independent Rust tempo-helper
component now matches all eight best BPMs and peak arrays exactly, with confidence
within 1e-12. Its input generator also matches all eight original PCM hashes.
The complete Rust audio-to-anchor API now passes all 15 full-worker synthetic
references described below. Decoder/engine/desktop integration remains outstanding;
these generated signals do not establish musical correctness.

The next component checks also pass on this host: all 306 original module 974
onset/attack/snapping rows, every one of 88,168 Float32 envelope values from
module 809 bit-for-bit, eight DP beat chains, eight applied phase corrections
and sixteen unchanged/early-anchor decisions. The module 522 phase tests now
also match 790 original functional result/state rows, including changed and
unchanged calibration, subsequent-beat, half-beat, silent/weak rescue, grid
correlation, grid-advancement and riser A/B cases. This remains component
validation. The module 473 drop/DnB/immediate-start and module 578 intro helpers
also match 216 original rows, covering all four intro classifications and a
48000 Hz input. Module 382 walkback matches 1,224 result rows, including optional
drop strength, intro-kind differences and half-beat counts. Module 358 initial
selection and its final correction sequence are now independently implemented. The public Rust `analyze` API matches all 15 original whole-worker
BPM/anchor results within 1e-12; initial candidate/correction-state snapshots also
match. These include 8/48 kHz, wider tempo bounds and a longer quiet intro.
A separate bank of 291 selector component results covers all 20 observed return
sites, including explicit DnB structural selection, sparse starts, quiet beds,
early-drop fallback and missing-drop behavior. Those component inputs supply
tempo/peak evidence explicitly, so they do not count as 291 full audio-to-timing
predictions or every internal predicate combination. The final-correction tests
also retain eight explicit component-only cases. The desktop still selects the
previous timing backend; no larger music-corpus or cross-platform parity claim
follows from these tests.

Additional private diagnostics showed:

- A silent 12-second input did not return before a 45-second VM deadline.
  Static reading of module 745 identifies threshold-reduction loops whose
  decrement remains zero when maximum amplitude is zero. A Rust port must
  explicitly handle silence rather than reproduce this hang.
- Low-amplitude short kick bursts returned a fallback 120 BPM/zero anchor
  even for other authored tempos. This is not evidence of successful analysis.
- On two complete owned FLAC files, raw left-channel PCM was decoded at the
  native 44100 Hz rate with the bundled decoder and supplied to the original
  worker. A fresh run of the current Cuerithm source-profile engine on those
  files agreed with Lexicon's BPM after rounding to two decimals on both.
  Absolute anchor differences were about **1.84 ms and 46.36 ms**. This small
  diagnostic establishes a timing difference, not superiority or corpus parity.

A selection error was found after these probes: one of the two files belongs
to a reserved group in the frozen input manifest. Its source-timing output has
now been observed; it must not be counted as an untouched held-out timing/parity
case in a later evaluation. The exposure is recorded in a separate private
audit without changing the frozen split. No reference-label manifest was opened
and no parameters were fitted from this diagnostic. Future development probes
must verify `split=tuning` before decoding. The numbers above are exploratory,
not held-out accuracy evidence.

The two source files were SHA-256 checked before/after; no tags or library
records were written. Private per-track outputs and hashes are not published.
These experiments do not prove WebAudio decode equivalence. The original
worker and the current Rust CLI also have different execution boundaries, so
their elapsed times are not a controlled performance comparison.

## Integration requirements still outstanding

The complete Rust timing API and measured synthetic component/full-worker
boundaries pass on this host. Preserve the current key, energy and cue
implementations during integration; do not change cue calculations to compensate
for timing discrepancies. The public full-analysis API rejects inputs shorter
than ten seconds as unknown after validating PCM/rate/options. This minimum was
traced through renderer module 57132 to module 45706 `$W` (`$ = 10`);
the tempo-helper API remains a separate, shorter-input component boundary.
The renderer also has a 1800-second application scheduling cap; that UI policy
is not imposed on this Rust DSP API. No source silent-input hang or fabricated
120/zero fallback is reproduced.

Before selecting it for desktop preparation, validate canonical decoding,
first-channel policy, sample-rate handling, complete-file clocks, both fractional
and persisted BPM representations, grid/cue alignment and cache identity.
Exercise all supported common-format layouts on macOS, Windows and Linux.
Compare a larger full-track corpus and record disagreement/failure counts.
Silence and unsupported input must return an explicit unknown/error promptly.
Existing completed analyses stay intact until explicit reanalysis; unfinished
jobs must capture/migrate the selected engine identity consistently.

Do not add an Analysis settings page or a user model download. Keep the legacy
timing backend available to research and regression tests. The desktop default
has **not** changed in this investigation.
