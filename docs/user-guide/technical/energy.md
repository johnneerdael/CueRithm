# Track energy

Energy is **one whole-track rating from 1 to 10**. It is computed from the audio by a fixed formula. There is no model file, and no per-segment energy anywhere in the app.

## Why only one number

Earlier research explored per-section energy labels on cues. The product deliberately does not offer them: a single whole-track rating is easier to trust and to use in a smart playlist, and attaching labels to structural cues would imply precision that CueRithm does not claim.

## The calculation

Method id `lexicon-acoustic-energy-v1`.

1. **Input.** The whole track, mono, 22,050 Hz. No loudness normalisation.
2. **Frames.** 2,048 samples with a hop of 1,024, Hann window, FFT magnitude scaled by 2/2048, DC excluded.
3. **Per frame**, compute: RMS, spectral flatness, the fraction of power between 2,000 and 5,000 Hz (*high-mid*), and the fraction between 20 and 60 Hz (*sub-bass*).
4. **Keep** only frames whose RMS is at least 12% of the loudest frame.
5. **Crest factor.** `crest_db = 20·log10(peak) − 20·log10(retained RMS)`.
6. **Score and level:**

```text
score = 9.0124 − 0.49685·crest_db + 21.436·flatness + 10.183·highmid + 2.9802·sub
level = clamp(floor(score + 0.5), 1, 10)
```

A track with a peak at or below 1e-10, or shorter than 2,048 samples, gets **no rating** and cannot become ready.

## How to read it

Lower crest factor (a more compressed, constant-loudness track), more spectral flatness (noisier, denser) and more high-mid and sub-bass power all push the number up. A sparse, dynamic track scores lower. It is a statement about the whole track's texture, not about how an individual drop feels.

## Provenance of the coefficients

The coefficients are an independent Rust reimplementation of a calculation observed in Lexicon. The project's research notes say plainly that the historical origin of the fixed coefficients is unknown, and that the expression could have been fitted originally. They also record that agreement with the observed calculation was checked on 29 synthetic audio cases (26 matched, 3 are deliberate differences: silence and very short input return *no rating* here), and that **real-music agreement has not been measured**.

## Where it goes

The number is stored in the library as `tracks.energy`, shown as `N/10`, usable in smart playlists (Energy, 0 to 10), and written to the audio file's `ENERGY` custom field when *Write track energy to files* is on (the default).
