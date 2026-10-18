# Cue frontend and source regressions

All inputs are independently authored synthetic signals or section-energy
arrays. No music, application code, library data or credentials are included.

`reference.json` records 100,100 Float32 output bit patterns for the renderer's
seven-stage 800/400/400/200/200/200/200 Hz lowpass cascade, measured with the
installed Chrome 155.0.8059.80 in a separate headless profile. The three cases
use 8000/44100/48000 Hz and the same 4096-frame dyadic pulse starting at frame 64.
The omitted Web Audio Q option is measured as 1. The matching Chromium
128.0.6613.186 coefficient/kernel source was also inspected, including its
10^(Q/20) conversion and double-precision normalization. These measurements
are not an original Lexicon 1.11.15 (Chromium 128) renderer or browser file-decode capture.

The independent Rust scalar filter differs by at most one Float32 epsilon
(1.1920928955078125e-7 absolute) on these cases. Tests enforce that bound and
require the old Butterworth fallback to differ by more than 0.01, so a wrong Q
cannot pass under the rounding tolerance. Byte-identical filter output across
browser/platform versions is not claimed.

`cue-reference.json` contains 40 original Lexicon 1.11.15.0 worker cue results on
ten authored recipes, with explicitly supplied BPM/anchor and the measured
Chrome lowpass input. Four configurations vary drop-at-start, minimum breakdown
beats and emergency-loop generation. The 192-second structured input includes
both drops and both breakdowns. Original marker types/colors are normalized to
the existing Rust schema; source rendering templates are not part of this
boundary. Every cue/loop result must match within 1e-12. This verifies cue DSP
given controlled inputs, rather than installed-app decode equivalence or musical
correctness. The pulse recipes reuse `tests/fixtures/lexicon-timing/generate.py`:

```sh
python3 tests/fixtures/lexicon-timing/generate.py --reference tests/fixtures/lexicon-cue-frontend/cue-reference.json
```

`iir-reference.json` contains 12 original worker single-pole IIR outputs at 80/300 Hz
with one/six passes and three rates: 400,400 Float32 values checked bit-for-bit. It
pins Float64 coefficient/feedback calculations with Float32 stores between
passes. The previous Rust port rounded feedback and coefficients to Float32.

`earlier-drop-reference.json` contains seven original helper results on authored
energy arrays. An earlier stable plateau requires a quieter preceding window;
the previous Rust comparison was reversed. Positive/veto, head, short-range,
zero and empty inputs are covered. These regressions accompany the retained
47 cue tests and check source behavior those tests did not cover.

`transient-threshold-reference.json` contains nine original inline drop-helper
results on authored energies and transient counts. The threshold is a fractional
20% of the largest count: 13 transients do not meet a threshold of 13.2. The
previous Rust cast truncated that threshold. Integer thresholds, zero counts,
single-transient and empty cases are also pinned. A separate private 29-track
tuning probe exposed this defect; no music-derived arrays are published here.

Primary filter references:

- [Web Audio 1.0 Q options](https://www.w3.org/TR/webaudio-1.0/#dom-biquadfilteroptions-q)
- [Chromium 128 biquad](https://chromium.googlesource.com/chromium/src/+/128.0.6613.186/third_party/blink/renderer/platform/audio/biquad.cc)
- [Chromium 128 kernel](https://chromium.googlesource.com/chromium/src/+/128.0.6613.186/third_party/blink/renderer/modules/webaudio/biquad_dsp_kernel.cc)
