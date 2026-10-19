# Under the hood

This section explains how CueRithm gets from an audio file to a key, a tempo, an energy rating and a set of cues, and how it protects your files while doing so. Everything here is derived from the source code and the project's own research notes. Where the project has not verified something, the page says so.

!!! abstract "The short version"
    - One **decoder** (a bundled, LGPL-licensed FFmpeg 9.0.2) turns every supported format into the same 44.1 kHz stereo float samples. Analysis, the waveform, tag verification and playback all share that one timeline.
    - **Key** comes from a small neural network (OpenKeyScan3, MIT) running on a constant-Q spectrum of the whole track.
    - **Tempo and the first beat** come from a signal-processing detector with many explicit correction rules, producing one constant BPM and one anchor.
    - **Energy** is one number from 1 to 10 from a fixed acoustic formula.
    - **Cues** come from a section-by-section energy analysis of the track.
    - Results go into a **SQLite** library. Your audio is never moved. Tag writes are staged, verified, journaled and atomic.

| Page | Topic |
| --- | --- |
| [The analysis pipeline](pipeline.md) | From file to ready track, step by step |
| [Decoding and the media runtime](decoding.md) | Bundled FFmpeg, sealing and diagnostics |
| [Key detection](key.md) | CQT and OpenKeyScan3 |
| [Tempo and beatgrid](tempo.md) | Tempo voting, the anchor and the constant grid |
| [Track energy](energy.md) | The acoustic formula |
| [The cue engine](cues.md) | Drops, breakdowns and the last beat |
| [Waveforms](waveform.md) | Three bands and the colour palettes |
| [The library database](library.md) | SQLite, schema, caches and recovery |
| [Safe tag writing](tag-writing.md) | Staging, verification, atomic swap |
| [Catalog matching](matching.md) | Normalising, scoring and thresholds |
| [Cleanup engine](cleanup-engine.md) | A pure function over metadata |
| [Smart playlist engine](smart-engine.md) | Rules, evaluation and refresh |
| [How accurate is it?](accuracy.md) | What the published numbers do and do not mean |

For provenance, licences and credits see [About](../about/index.md).
