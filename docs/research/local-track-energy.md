# Local whole-track energy investigation

Inspected 2026-10-18 with leanrea. Originals, music and application libraries
were preserved. Private disassembly and the instrumented reference worker stay
outside published source. These findings distinguish a locally evaluated formula
from local aggregation of remotely classified segments.

## Current Cuerithm baseline

Before this change, `EnergyModel` loaded Cuerithm's bundled RBF SVC (183 acoustic
features, gamma 0.1), classified internal windows locally, then selected the
level with the greatest total window duration, preferring higher levels on ties.
This was offline ML inference, not a request to Mixed In Key. No segment labels
were exposed in the desktop result.

## Mixed In Key 11.2.6.7446: seen

The installed main executable's ARM64 slice SHA-256 is
`051e2805a4b34b3bbd1f4437aae2b537b1b133a1830c5450c27500199666d36b`,
identical to the retained inspected slice. Its fat executable SHA-256 is
`04f10f4cf84938f42dc13c0c984da915b861beaab9675713b61bf82e17c5b905`.

The analysis response consumer `MIKAnalysisOperation::duringOperation:` at
`0x10008dd58` reads server `EnergyLevelSegments` and `OverallEnergy`. Retained
source evidence is `ev_e33d2b8168c28c02f22c58bbe27658488c2e1347b63b3e18f3439210ddc2d90f`.
See [the service contract](analysis-service-contract.md) and the historical
[request/response investigation](request-response-mapping.md).

Fresh local disassembly resolves `recalculateOverallEnergyLevel` through the
compact method entry at `0x100236310` to implementation `0x1000a01e8`:

- `0x1000a021c` / `0x1000a0260`: obtain `energySegments`.
- `0x1000a02bc`: obtain segment `length`; `0x1000a02c8` obtains `energy`.
- `0x1000a02d0..0x1000a0330`: round/clamp each label to 0–10 and sum its length.
- `0x1000a0370..0x1000a0394`: scan 10 down to 0, replacing only on strictly
  greater duration. The highest level wins equal-duration ties.
- `0x1000a03d0`: `setEnergy:`; `0x1000a03e0`: `writeTagIfNeeded`.

**Conclusion:** the tag's aggregation is local and deterministic. That does not
make the complete audio-to-rating path independent of the service: the observed
analysis path first consumes remotely classified segment levels. Server
`OverallEnergy` is also supplied independently; it is not the same formula as
the local tag aggregation. The server's implementation and whether all of its
energy computations use ML remain unknown. No fresh network request was made.

## Lexicon 1.11.15.0: seen

Installed ASAR SHA-256:
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
The renderer's `AudioAnalyzer.analyzeTrackInWorker` renders the entire audio
buffer to mono 22050 Hz using `OfflineAudioContext`, then sends those PCM bytes
to `182.index.worker.js`. It stores the worker's numeric `energy` result.

In the original worker, local helper `Me` (pretty-printed lines 18203–18312):

1. Uses 2048-sample Hann windows, hop 1024, and Float64 FFT magnitudes scaled by
   2/2048. Window samples and per-frame summaries are Float32.
2. Computes unwindowed RMS, spectral flatness, 20–60 Hz sub-bass power fraction
   and 2000–5000 Hz high-mid power fraction. DC is excluded from spectral sums.
3. Retains frames with RMS at least 12% of the maximum frame RMS.
4. Computes peak-to-retained-RMS crest factor in dB and retained-frame feature
   means, then evaluates:

```
score = 9.0124 - 0.49685 * crest_db + 21.436 * flatness
        + 10.183 * high_mid_ratio + 2.9802 * sub_ratio
level = clamp(round(score), 1, 10)
```

This path uses no network request or external learned parameter file. The
historical origin of these fixed coefficients is unknown; the expression could
have been fitted originally. Static code alone cannot establish that no machine
learning was involved in designing it.

## Independent Rust implementation and validation boundary

`camelot-core/src/lexicon_energy.rs` independently implements the observed DSP
using the existing Rust FFT library. The desktop selects it automatically and
stops embedding/installing the 10.27 MB SVC JSON. Key and cue algorithms are
unchanged. The explicit-model research API remains available.

`tests/fixtures/lexicon-energy/reference.json` records 29 original synthetic PCM
cases evaluated by the shipped worker inside Lexicon's Electron 32.3.3 /
Node 20.18.1 / V8 12.8 runtime. The application entry point was not launched;
its library, authentication, license and network APIs were not used. Only the
synthetic PCM and numeric reference outputs are published. No proprietary code
or music is included.

26 usable cases match source levels, with all four feature values within 1e-6.
Three explicit quality exceptions retain raw source outputs in the fixture:
Lexicon returns 9 for empty, sub-frame and silent input; Cuerithm returns no
rating. Nonfinite PCM and the wrong helper sample rate are explicit errors.

This proves the measured helper boundary, not exact browser resampling or
real-music accuracy. Cuerithm uses its own existing PCM downmix/resampler, whose
edge samples may differ from WebAudio. Real-library comparison is still needed
before claiming complete Lexicon energy parity. Completed ratings remain intact
until explicit reanalysis; settings and unfinished jobs migrate automatically.
