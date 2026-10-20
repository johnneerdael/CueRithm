# Track energy

Energy is one whole-track rating from 1 to 10, computed from the audio with a fixed formula. The calculation uses no model file, and CueRithm does not show per-segment energy.

## Why only one number

Earlier research explored per-section energy labels on cues. CueRithm uses a single whole-track rating because it is easier to trust and use in a smart playlist. Energy labels on structural cues would imply precision the app does not claim.

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

A track with a peak at or below 1e-10, or shorter than 2,048 samples, gets no rating and cannot become ready.

## How to read it

Lower crest factor (a more compressed, constant-loudness track), more spectral flatness (noisier, denser) and more high-mid and sub-bass power all push the number up. A sparse, dynamic track scores lower. The rating describes the whole track's texture; it does not rate an individual drop.

## Provenance of the coefficients

CueRithm's Rust code independently implements an acoustic calculation observed in Lexicon. The historical origin of its fixed coefficients is unknown; the expression may originally have been fitted. Agreement was checked on 29 synthetic audio cases: 26 matched, and 3 are deliberate differences because silence and very short input return *no rating* here. Real-music agreement has not been measured.

## Where it goes

The number is stored in the library as `tracks.energy`, shown as `N/10`, usable in smart playlists (Energy, 0 to 10), and written to the audio file's `ENERGY` custom field when *Write track energy to files* is on (the default).
