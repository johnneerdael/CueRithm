# Key detection

CueRithm predicts the musical key of the whole track with a small neural network bundled in the app. It runs locally without a model download or server and does not use file tags.

## The model

| | |
| --- | --- |
| **Network** | OpenKeyScan3, an ONNX conversion of the model from the MIT-licensed project `rekordcloud/openkeyscan-analyzer` |
| **Size** | about 19 MB, embedded in the application and verified by its SHA-256 at start |
| **Runtime** | RTen, a Rust ONNX runtime. No Python, no native ML libraries. |
| **Output** | 24 logits: 1A to 12A (minor keys) then 1B to 12B (major keys) |

## Steps

1. **Input.** The whole track as 44.1 kHz mono float samples. No peak normalisation.
2. **Guards.** Fewer than 3 seconds of audio, or an RMS at or below 1e-5, gives *unknown*. More than 64 million samples is an error.
3. **Resampling.** The signal is halved twice with the bundled `libsoxr` at high quality, giving 44,100, 22,050 and 11,025 Hz versions.
4. **Constant-Q transform.** A fixed 105-bin CQT with 24 bins per octave starting at 65 Hz, hop of 8,820 samples (0.2 s) at the top rate, using five filter banks. The result is compressed with `ln(1 + x)`, the last two frames are dropped, and at least 10 frames are required.
5. **Network.** The `[1, 1, 105, T]` tensor goes through the model. The highest logit wins.

The class order is 1A to 12A then 1B to 12B (`models/openkeyscan/model.json`).

## Notation

The result is stored as a Camelot code. A is minor and B is major: C major is 8B and A minor is 8A. CueRithm can also write the standard name (for example "A minor") into file tags. Open Key is parsed in smart-playlist rules but is not an output format.

## Confidence

The engine records a margin (top logit minus second) and the ranked list of 24 candidates. These are raw network logits. They rank the keys but are not probabilities. There is no low-confidence threshold. A track with no resolvable key does not become ready.

## What else runs

A classical Krumhansl-Kessler profile estimator still exists in the engine and runs on an 11.025 kHz signal, but its result is not the saved key. Only its chroma and tuning diagnostics survive. Research models (S-KEY and others) are in the repository but are not enabled in the desktop app.

## Limits

The network predicts key agreement, not musical truth. See [How accurate is it?](accuracy.md): against my reference labels it agreed on 274 of 282 tracks, and the major-key coverage in that corpus is small, so held-out accuracy is unverified.
