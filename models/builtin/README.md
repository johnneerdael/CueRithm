# Built-in desktop analysis parameters

The desktop executable embeds both files, verifies their pinned SHA-256 values,
and manages its own versioned cache. Users never choose or download model files.
Structural cue detection and BPM/beatgrid DSP are compiled Rust code.

OpenKeyScan3 uses the unchanged pinned ONNX conversion documented in
`../openkeyscan/model.json`. The upstream project distributes its trained model
and documents bundling it in applications. Its MIT notice is preserved in
`../openkeyscan/LICENSE.upstream`; include it with redistribution.

The energy file is Cuerithm's locally trained RBF SVC export: C=10, gamma=0.1,
183 acoustic features, 2760 support vectors and classes 1–9. Inference uses the
existing Rust one-vs-one implementation and exposes one aggregated track rating.
These are learned numeric parameters, not vendor model weights. They are
distributed under this repository's MIT license. Training music, request/response
captures and per-track reference labels are excluded.

`manifest.json` records identities and provenance. Bundling changes setup and
distribution; it does not establish new held-out accuracy or exact reproduction
of a server's independent OverallEnergy formula. The standalone engine/CLI still
supports explicit model files.
