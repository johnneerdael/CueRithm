# Synthetic OpenKeyScan frontend fixture

Generated noise PCM and an independently computed upstream CQT, retained from
the fixed frontend feasibility experiment. No music, tags or library data.
PCM: mono float32 little-endian, 44100 Hz, 220517 frames (odd length).
CQT: float32 little-endian, [1,1,105,24], upstream librosa 0.11.0 and
Python-SoXR 1.0.0; two final time frames removed. Parameters and kernel
provenance are in docs/research/openkeyscan-rust-backend.md.
Comparison tolerance: 1e-4 absolute + 1e-4 relative.

PCM SHA256: ad05dcae0e61d033b068c024f4b85b8a710e9658f91071607201a3104b4cbf07
CQT SHA256: b4b1881cd70d7fc937869600bd2f5157fe86a3180d45649f1d83cb00d5238fbf
