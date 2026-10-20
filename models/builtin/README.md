# Built-in desktop analysis

The desktop embeds the pinned OpenKeyScan3 key graph, verifies its SHA-256,
and manages its versioned cache. Users never choose or download model files.
OpenKeyScan3 uses the unchanged ONNX conversion documented in
`../openkeyscan/model.json`. Its MIT notice is preserved in
`../openkeyscan/LICENSE.upstream`.

Track energy now uses compiled Rust acoustic DSP: crest factor, spectral
flatness, high-mid and sub-bass ratios combined by a fixed formula. There is no
energy parameter file, training step or network request in the desktop path.
See `../../docs/research/local-track-energy.md` for source evidence, the
synthetic reference boundary and intentional silence policy.
Structural cue detection and BPM/beatgrid DSP are also compiled Rust code.

`energy-svc-v1.json` is a window classifier fitted to Mixed In Key window levels from the development split. The
desktop embeds it and offers it as the opt-in "MIK-style (beta)" energy rating in Settings; the built-in formula stays
the default and nothing is downloaded. Choosing a rating never changes a completed track until it is analyzed again.

`manifest.json` records active desktop component identities. Synthetic helper
agreement is not a claim of end-to-end browser decode parity or real-music
accuracy. Licensed applications, private captures and music are not bundled.
