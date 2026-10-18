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

The old `energy-svc-v1.json` remains a historical development artifact for the
standalone explicit-model research API. It is not embedded or installed by the
desktop. Existing cached copies are left untouched on upgrade; desktop settings
and unfinished jobs migrate to compiled energy. Previously completed track
ratings change only when those tracks are reanalyzed.

`manifest.json` records active desktop component identities. Synthetic helper
agreement is not a claim of end-to-end browser decode parity or real-music
accuracy. Licensed applications, private captures and music are not bundled.
