# Lexicon beatgrid investigation

Inspected 2026-10-18 with leanrea. The installed Lexicon application, source
music and application libraries were preserved. Private extraction and probes
remain outside published source. The independent Rust timing implementation now
has component, synthetic worker and owned-corpus agreement evidence. Native
desktop delivery and cross-platform validation remain separate gates.

## Current Cuerithm behavior

The desktop's `BuiltinModels::apply` now selects `lexicon_timing`, clearing the
previous source-profile and learned timing configuration. Its BPM and anchor
come from the complete local reconstruction described below. Unrounded timing
feeds the constant grid and Lexicon-compatible cue generator. Key analysis and
the local scalar energy meter retain their own downmixed input.

The earlier 282-track comparison validates MIK BPM agreement; exact full-grid
agreement was checked on five captured cases. See
[source timing](source-timing-engine.md). Neither result proves the grid agrees
with Lexicon or with human musical annotation.

## Desktop cue input regression, 2026-10-19

A user-reported missing breakdown exposed an application input bug: background
preparation did not pass the track genre into the cue generator, although the
original renderer does. The earlier 282-group cue benchmark supplied `genre:
null` to both worker implementations. Its agreement therefore did not validate
the desktop metadata path or genre-dependent cue selection.

Prepared analysis now uses each track's native genre unless an explicit engine
override is present. After configured catalog tagging or cleanup, it refreshes
cues with the final genre using verified prepared PCM and the existing timing;
key, BPM and scalar energy are preserved. Effective genre enters analysis
provenance and cache identity. Regression tests cover worker reuse, failure
recovery, explicit overrides and background cleanup publication.

For the reported track, fresh analysis through the installed Lexicon GUI,
followed by its default Cue Point Generator, agrees exactly with Cuerithm's
background preparation on BPM, first-beat anchor and all five cue timestamps.
The user reports that the earlier screenshot also followed beatgrid and cue
reanalysis. Its saved grid differs from the fresh GUI result in this diagnostic;
the cause of that earlier difference remains unproven. This single-track
diagnostic does not establish library-wide desktop parity, independent musical
accuracy, template appearance parity or Windows/Linux runtime parity. Identifying data and the original worker remain private.
Existing completed Cuerithm results require explicit reanalysis to replace their
old cues; upgrading does not rewrite the user's completed collection.

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
The complete Rust audio-to-anchor API now passes all 62 full-worker synthetic
references described below. Decoder/engine integration also passes; hosted desktop delivery remains outstanding;
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
selection and its final correction sequence are now independently implemented. The public Rust `analyze` API matches all 62 original whole-worker
BPM/anchor results within 1e-12; initial candidate/correction-state snapshots also
match. These include 8/48 kHz, wider tempo bounds and a longer quiet intro.
The additional 47 whole-worker cases use 64-second inputs, 120/174/220 BPM
authored pulses, offsets from zero to 31 seconds and quiet intros of 8/22
seconds under 50..300 BPM bounds. This also checks source half/third-tempo
choices rather than forcing agreement with the authored pulse rate.
A separate bank of 291 selector component results covers all 20 observed return
sites, including explicit DnB structural selection, sparse starts, quiet beds,
early-drop fallback and missing-drop behavior. Those component inputs supply
tempo/peak evidence explicitly, so they do not count as 291 full audio-to-timing
predictions or every internal predicate combination. The final-correction tests
also retain eight explicit component-only cases. These synthetic tests alone do
not establish music-corpus or cross-platform agreement.

## Owned Beatport timing comparison

All **282 distinct tuning audio groups**, representing 328 files in the frozen
434-file manifest, match the original Lexicon worker's BPM and first-beat anchor
exactly: maximum absolute differences are zero. The inputs span 38.37–557.08
seconds and total 26.59 hours. Every input is native 44100 Hz stereo 16-bit FLAC;
both predictors use its first channel. No reference-label manifest or reserved
audio was read in this run. This is development-corpus agreement with the
original algorithm, not independent musical ground truth or a cue/energy score.

Each prediction uses a private immutable encoded snapshot. Decoded S16 PCM MD5
must equal the frozen manifest before either prediction, and encoded hashes
are checked before and after. All timestamps had drifted, so timestamp equality
was deliberately not used as proof of audio identity. No source tags or files
were written. One original-worker attempt exceeded 240 seconds under concurrent
load; a separate 600-second retry completed after about 504 seconds and matched
exactly. The initial timeout is retained in the private evidence.

The pinned Rust predictor is commit `73435d7`, before subsequent cue-only
corrections; its timing DSP is unchanged. Ten disjoint shards ran concurrently,
followed by that single retry. Rust full-analysis median was 1.86 seconds and
95th percentile 2.99 seconds. This includes native decoding and default DSP key
analysis; original-worker timing excludes those stages. These different
boundaries and load conditions do not support an application speed ratio.
Aggregate identities and timings are recorded in
[the benchmark summary](lexicon-timing-beatport-2026-10-18.json). Per-track data
and the proprietary worker remain private. The earlier reserved-group exposure
above still applies; this run does not restore an untouched holdout claim.

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

## Engine integration and remaining delivery

The complete Rust timing API and measured synthetic component/full-worker
boundaries pass on this host. The opt-in engine backend is available as
`EngineSettings.lexicon_timing` or the research CLI `--lexicon-timing`, with
fixed 98..195 BPM candidate-search bounds. The source refinement may round
slightly outside custom noninteger bounds; the public helper preserves this
behavior rather than adding an output clamp. It is mutually exclusive with source-profile and
learned timing and requires complete input. Direct decoding retains native-rate
first-channel samples from the same packet stream as the unchanged downmix;
prepared analysis uses canonical 44100 Hz first-channel PCM on the playable
clock. Native timing rejects rates above 192000 Hz; canonical preparation can
still resample supported high-rate sources before timing.

The new mode also routes that retained first channel, at its actual native or
canonical rate, to the existing cue DSP. The worker trace at 18699–18735 uses
that same raw channel for timing and cues. Legacy timing modes retain their
existing cue routing; key and energy remain downmixed. An asymmetric quiet-left/
loud-right case now distinguishes and verifies this routing correction.

Engine tests preserve more-than-two-decimal BPM through the constant grid and
cue generation, report the exact native clock despite key-rate frame rounding,
and leave key/scalar-energy outputs unchanged. All 14 canonical media layouts
pass first-channel timing/cue/source+PCM identity checks and reach Ready through
the real background worker. Completed analyses survive a default-setting
change, while a timing-backend change invalidates the preparation cache;
unchanged cached analysis survives restart. Timing source and input/frontend
code hashes, channel policy, rate and fixed bounds enter identity. The disabled
flag is omitted from serialized settings to preserve legacy settings identities.
The 2026-10-19 broad local core/library/CLI/desktop gate passes 265 tests with
nine optional tests ignored, including the second-drop continuation regression
below. All-target/all-feature Clippy and formatting checks pass. The preceding
`97c3d2b` checkpoint also passed 29 UI tests and hosted engine/packaged desktop
checks on Linux, Windows, macOS ARM and Intel ([engine](https://github.com/johnneerdael/CueRithm/actions/runs/39157474405),
[desktop](https://github.com/johnneerdael/CueRithm/actions/runs/39157474437)). A whole-branch
review at `2830ab1` found no confirmed blockers; subsequent source-derived cue
corrections have focused authored regressions and corpus reruns. Hosted checks
must run again for the new correction's delivery commit.

Preserve the key and energy implementations during delivery. Cue corrections
below have their own source-derived regressions. The public full-analysis API rejects inputs shorter
than ten seconds as unknown after validating PCM/rate/options. This minimum was
traced through renderer module 57132 to module 45706 `$W` (`$ = 10`);
the tempo-helper API remains a separate, shorter-input component boundary.
The renderer also has a 1800-second application scheduling cap; that UI policy
is not imposed on this Rust DSP API. No source silent-input hang or fabricated
120/zero fallback is reproduced.

The owned full-track comparison above is complete. A packaged 84.40 MiB macOS
ARM application also prepared an isolated authored 64-second FLAC automatically:
120 BPM, anchor zero, 128 grid beats, key 2A, scalar energy 10 and replacement
cues. Native playback, full-track/16-second waveform switching, cue seeking and
two-second forward/backward bar jumps were observed. Explicit reanalysis
returned the same key/timing/energy/cues, incremented attempts to two and saved
two cue revisions; encoded source SHA-256 remained unchanged. No real music or
user library was used for this native test. Fractional timing and all common
formats are covered by engine/library tests, rather than this one native case.
Exercise the decoder/clock/common-format gates on macOS Intel, Windows and Linux.
Silence and unsupported input must return an explicit unknown/error promptly.
Existing completed analyses stay intact until explicit reanalysis; unfinished
jobs must capture/migrate the selected engine identity consistently.

Do not add an Analysis settings page or a user model download. Keep the legacy
timing backend available to research and regression tests. The desktop default
now selects this implementation, while completed results remain preserved.


## Cue preprocessing and corrected source arithmetic

The renderer supplies first-channel raw PCM plus seven Web Audio lowpass stages
at 800/400/400/200/200/200/200 Hz. Its omitted Q option defaults to 1 dB for
lowpass filters; the original Rust fallback used Butterworth Q = 1/sqrt(2).
The new timing mode now supplies a separate renderer-compatible frontend to the
existing cue decisions. Legacy standalone cue entry points remain callable.
[Web Audio filter options](https://www.w3.org/TR/webaudio-1.0/#dom-biquadfilteroptions-q)
and the pinned Chromium 128.0.6613.186 coefficient/kernel source were checked.

An isolated headless Chrome 155.0.8059.80 probe measured three authored pulse
inputs at 8/44.1/48 kHz. The independent scalar implementation agrees within one
Float32 epsilon on all 100,100 output samples; the old Q differs by more than
0.01. This is measured conformance plus a pinned-source trace, not an original
Chromium 128 renderer or browser file-decode capture. The earlier copied-runtime
GUI launcher produced no reference output and is not used as evidence.

Forty original worker cue results now cover ten authored signals, including a
192-second structured track, with supplied timing and measured filtered input.
All match within 1e-12 after two demonstrated corrections to the existing port:
Float64 single-pole IIR feedback/coefficients with Float32 stores, and the
previously reversed quieter-before-window condition for earlier-drop selection.
Twelve IIR cases pin 400,400 Float32 values bit-for-bit; seven authored energy
arrays pin the earlier-drop condition and empty/zero behavior. The original
47 cue tests still pass. These checks preserve cue semantics rather than fitting
positions to songs; source agreement alone is not musical ground truth.

Cue source/wrapper/frontend fingerprints now enter identity for configured cues,
and the renderer route records its input method and rate. Existing completed
analyses stay intact; unfinished/explicitly reanalyzed work uses the corrected
implementation. See the [fixture provenance](../../tests/fixtures/lexicon-cue-frontend/README.md).

A separate cue pilot selected every tenth tuning group before prediction:
29 complete tracks and 126 resulting cues. It initially matched 28/29 tracks.
The outlier exposed a third existing port defect: the source compares integer
transient counts against a fractional threshold, while Rust truncated 13.2 to
13. Identical filtered samples still produced the discrepancy, isolating the
decision arithmetic. Nine authored original-helper cases pin fractional,
integer, zero and empty thresholds. Keeping the threshold as Float64 corrects
the outlier; the fresh run matches **29/29 complete cue results exactly**.

The original worker receives its previously verified timing, raw first-channel
PCM and an independently implemented scalar lowpass cascade; Rust runs fresh
complete timing/frontend/cue analysis. Configuration is Never, 64 beats, no
emergency loop and no genre hint. The exact comparison covers cue positions,
types, colors and loop result. This boundary is not an installed Chromium 128
renderer/file-decode capture, other cue configurations or musical ground truth.
All source/PCM identity guards passed. Per-track data stays private; aggregate
identities are included in the benchmark summary above. Real-music scalar
energy agreement remains unmeasured by these timing and cue runs.


The subsequent full comparison covered all 282 distinct tuning groups. Its
[unchanged baseline](lexicon-cue-beatport-2026-10-18.json) matched 281 complete
cue results and 1,260 of 1,261 individual cues. One first Drop was 29.7674 seconds
early. Original and Rust section energies were identical, as were the initial
drop and breakdown. The mismatch came from second-drop refinement: a transition
guard returned from the entire Rust chain, whereas the source returned only
from the peak-region pass and continued later passes. A labeled block confines
that exit to its pass. Eight authored original-helper cases reproduce the
control-flow error and pin its correction; no music-derived arrays are published.

The [fresh corrected run](lexicon-cue-beatport-2026-10-19.json) matches all
**282 complete cue results and all 1,261 cues exactly**, with no missing/extra
cues or unresolved cases. BPM and first-beat positions remain exact on all
282 groups, and all 29 pilot cases repeat unchanged. Source/decoded-PCM identity
guards pass. This remains development-set agreement under the single stated
configuration and worker boundary; it does not establish unseen-music accuracy,
other cue settings, browser decoding equivalence, or whole-track energy parity.
