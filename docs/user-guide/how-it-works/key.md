# How is the key detected?

<p class="lead">CueRithm predicts the musical key of the whole track with a small neural network that ships inside the app. It runs locally, needs no download or server, and never reads the key tag in your file.</p>

!!! abstract "What this means for you"
    - **The key is a strong suggestion, not a fact.** The network picks one of 24 keys. It gives no confidence score and CueRithm shows none, so a doubtful track looks as certain as a clear one.
    - **Check by ear when a mix matters**, especially for minor versus major and for neighbouring keys. The disagreements measured against another tool were mostly one step round the wheel; see [How accurate is it?](accuracy.md).
    - **A track with silence or under three seconds of audio gets no key** and cannot leave Incoming.
    - New to the notation? Read [Understand Camelot keys](../analysis/camelot-keys.md).

## Which model reads the key { #the-model }

| | |
|---|---|
| **Network** | OpenKeyScan3, an ONNX conversion of the model from the MIT-licensed project `rekordcloud/openkeyscan-analyzer` |
| **Size** | About 19 MB, embedded in the application and checked against its SHA-256 when the app starts |
| **Runtime** | RTen, a Rust ONNX runtime. No Python and no native machine-learning libraries |
| **Output** | 24 scores: 1A to 12A (minor keys), then 1B to 12B (major keys) |

## What happens to the audio { #steps }

1. **Input.** The whole track as 44.1 kHz mono float samples (both channels averaged), with no peak normalisation.
2. **Guards.** Fewer than 3 seconds of audio, or an RMS level at or below 1e-5, gives *unknown*. More than 64 million samples is an error.
3. **Resampling.** The signal is halved twice with the bundled `libsoxr` at high quality, giving 44,100, 22,050 and 11,025 Hz versions.
4. **Constant-Q transform.** A fixed 105-bin spectrum with 24 bins per octave starting at 65 Hz, one column every 0.2 seconds, built from five filter banks. The result is compressed with `ln(1 + x)`, the last two columns are dropped and at least 10 columns are required.
5. **Network.** The spectrum goes through the network and the highest score wins.

The class order is 1A to 12A, then 1B to 12B (`models/openkeyscan/model.json`).

## How the key is written { #notation }

The result is stored as a Camelot code. A is minor and B is major: C major is 8B and A minor is 8A. When the Key field is written to a file and **Write Camelot keys** is off, the standard name is written instead, for example "A minor" or "Eb major". The spellings are fixed: C#, Eb, F#, Ab and Bb. Smart playlist rules also read Open Key and musical names, but Open Key is not an output format.

## How sure is the key? { #confidence }

The engine records the gap between the top two scores and the ranked list of all 24 candidates. These are raw network scores. They rank the keys but are not probabilities, there is no low-confidence threshold, and the app does not display them. A track with no resolvable key does not become ready.

## What is not used { #what-is-not-used }

A classical profile estimator still runs on an 11.025 kHz copy, but only its chroma and tuning diagnostics are kept; its answer is not the saved key. Research models such as S-KEY are in the repository but are not enabled in the desktop app.

## What the key cannot tell you { #limits }

The network predicts the key a reference tool would label, not musical truth. On the 282-track development set it matched 274 stored Mixed In Key 11 labels exactly, but only 8 of those tracks were major keys, so major-key coverage is thin and held-out agreement is unmeasured. The figures and their scope are on [How accurate is it?](accuracy.md).

## Where you see it in the app { #where-you-see-it }

| Place | What shows |
|---|---|
| **Key** column in Tracks | The Camelot code, for example <span class="cr-key">6A</span> for "17 (Extended Mix)" and <span class="cr-key">2A</span> for "Atoma (Extended Mix)" |
| Smart playlists | The **Key** field, and the harmonic or fuzzy "similar key" rule: see [How do smart playlists stay current?](smart-engine.md#keys) |
| File tags | Only if the **Key** field is selected in **Settings → Automatic tagging** |

A key that looks wrong is not a setting you can change; you can correct the file's tag by hand in [Edit tags and artwork](../tagging/edit-tags-by-hand.md), but the measured key in the library stays. See [Read a track's key, BPM and energy](../analysis/read-key-bpm-energy.md) and [My key or BPM differs from another app](../help/troubleshooting/key-or-bpm-differs.md).

## Related

- [Understand Camelot keys](../analysis/camelot-keys.md)
- [How accurate is it?](accuracy.md)
- [Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md)
- [How does CueRithm analyse a track?](pipeline.md)

<!-- verified: crates/camelot-core/src/openkeyscan.rs (guards 3 s, rms 1e-5, 64M samples, 105 bins, ln_1p, two frames cropped, >=10 frames, key_from_logits order and margin), openkeyscan/kernels.rs (five banks, hops 8820/4410/2205 = 0.2 s), native/openkeyscan/half.c (SOXR_HQ), key.rs (name() spellings, from_camelot), crates/cuerithm-smartlists/src/keys.rs (Open Key aliases), apps/desktop/src-tauri/src/builtin.rs (model installed and SHA-256 verified; other key backends cleared), engine.rs (mono mix, features retained), models/openkeyscan/model.json, docs/research/openkeyscan-comparison.md (274/282, 8 major); legacy technical/key.md re-checked -->
