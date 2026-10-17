# OpenKeyScan Rust backend continuation

Continue the approved offline Rust analysis design. OpenKeyScan3 is the fixed
key candidate: unchanged upstream Python matches274/282 MiK11 tuning labels;
the same network runs through the existing Rust RTen runtime with14/14 class
agreement. The purpose is to make that candidate usable from audio in the Rust
engine. Keep energy/cue work and the overall quality-first engine goal active; exact
reproduction is a diagnostic rather than the product acceptance rule.

Preserve the pinned checkpoint and exported ONNX graph, class order and full-file
preprocessing. Use full44100Hz mono float32 PCM without peak normalization,
105 CQT bins,24bins/octave,fmin65,hop8820,tuning0, default Hann wavelets and
rectangular STFT, sparse fixed filter banks, recursive HQ SoXR half-rate
resampling with source float32 scaling, float64 length normalization narrowed
to complex64, magnitude/log1p, and removal of the final two time frames.

Use the matching portable libsoxr source bundled in Python-SoXR1.0.0, built
without Python, OpenMP, variable-rate or platform DSP frameworks. Preserve
LGPL/PFFFT source notices and provenance. Runtime inference remains Rust RTen;
there is no Python runtime requirement. Native source/ABI/build packaging and
Windows/Linux behavior must be checked, not assumed from a Mac result.

First construct a private diagnostic file-to-key worker from the fixed math and
network. Compare generated PCM plus five existing full-track CQT tensors,
requiring unchanged predicted classes and the predeclared absolute/relative
1e-4 internal numeric tolerance. Internal bit equality is not promised for
RustFFT versus SciPy. Then run all282 tuning representatives, predicting before
opening labels; require per-track upstream class parity and report MiK11
agreement separately, including failures. Do not change parameters to improve
label agreement. Keep82heldout groups unopened.

The initial file-profile diagnostic accepts complete integrity-verified native
FLAC44100Hz stereo16-bit, at most64000000frames. This is a validation boundary,
not the final engine's format target. Keep original inputs unchanged, give the
worker a private encoded snapshot, and retain code/model/config/input identity.
Generated PCM cases may enter through a separate diagnostic-only mode; final
product APIs must not accept arbitrary golden tensors or per-track answer data.

After the complete diagnostic gate, add an explicit optional backend to the
existing engine/CLI. Keep S-KEY/default JSON compatibility, model mutual
exclusion, direct44100Hz decoding, model/code/config identity and unknown/error
semantics explicit. Use TDD and one independent whole-change review for that
production integration. The private diagnostic alone is not the installed key
backend, and key success does not complete BPM/energy/cue/general-platform gates.


## Production integration checkpoint, 2026-10-17

Task3 now has an explicit optional normal Engine/CLI backend. Core/default and
all-feature tests, formatting, Clippy and release build pass on Mac; the pinned
model and generated-audio Engine integration checks also pass. The native
source and fixed kernels are bundled with provenance; weights remain explicit
external inputs. The implementation uses the same fixed frontend math as the
282-track diagnostic, and its final-binary corpus and Linux production gates have now passed. A fresh whole-change
review found no code findings. This checkpoint does not change the default key
backend or complete the whole engine goal.


The final regular CLI combines this key backend with source-profile BPM on all282
original tuning representatives:282 complete/integrity-verified results, no
unknowns/failures,282/282 unchanged upstream key classes and274/282 MiK11 key
agreement (97.16%). Fresh captured BPM and first beat anchors match exactly in
binary64 on282/282, and199371 beat values match the prior frozen timing path.
This is local tuning agreement, not independent musical ground truth or held-out
accuracy. Only eight tuning examples have major-mode reference keys.

Linux ARM64/Rust1.89 offline validation passes399 all-feature tests (7 ignored),
the real model/Engine checks and a combined CLI case. All290 packaged source
files remain unchanged; the owned container was removed. Mac default tests pass
386 (2 ignored), all-feature tests399 (7 ignored), along with fmt, Clippy and
release build. Broader-format key parity and Windows execution remain separate
work. Production binary SHA256:
`b9fb487268ac54d3964ce2017436c10019ff26dec78ccc1d234cb94ba9ccbd6e`.
