# How accurate is it?

Short answer: the project reports **agreement with reference tools on its developers' own test library**, not independently verified musical truth. This page lays out what was measured and what was not, so you can judge it for your music.

!!! warning "Agreement is not accuracy"
    Every figure below compares CueRithm with another program's output (Mixed In Key labels for key, a reimplemented Lexicon timing and cue worker for timing and cues). The project's own notes call these "agreement tests against supplied outputs, not independently annotated musical truth".

## The test library

282 tuning groups of 44.1 kHz stereo 16-bit FLAC tracks, 38 to 557 seconds long, from Beatport electronic music. A separate holdout of 82 groups was reserved, but one reserved file was accidentally exposed in a diagnostic, so the project does not claim an untouched holdout.

## Key

- **274 of 282** exact agreement (97.16%) with stored Mixed In Key 11 keys.
- The 8 disagreements: 5 neighbouring fifths, 1 relative major or minor, 2 parallel major or minor.
- **Camelot-compatible** (same key or a harmonic neighbour): 280 of 282 (99.29%).
- Only **8 tracks were major keys**, with 7 of 8 matched. Broader major-key coverage is needed, and held-out accuracy "remains unverified".

## Tempo and first beat

- On the same corpus the Rust implementation matched the reimplemented Lexicon timing worker exactly on 282 of 282 tracks. This shows the port is faithful. It does not show the grid is musically right.
- An earlier, classical tempo search that the desktop does not use matched exact two-decimal BPM on only 58 of 282 tracks, and its tests covered only a 104 to 144 BPM range.
- Coverage gaps the project names: other tempo ranges and sample rates, stereo channel policy, long intros, irregular rhythm, tempo changes and complete music files.
- A single-track check in the original application's interface does not establish library-wide parity, independent musical accuracy or Windows and Linux behaviour.

## Energy

- The formula matches the observed calculation on 26 of 29 synthetic cases, with 3 deliberate differences (silence and very short input return no rating).
- **Real-music agreement has not been measured.**

## Cues

- **1,261 of 1,261** cues matched the reference worker across the 282 tracks, under one configuration: drop at start *Never*, 64-beat minimum, no loop, no genre hint.
- That configuration did not exercise the genre-dependent path the desktop actually uses, so the result does not validate genre-based cue selection, other settings or unseen music.
- There is no listening study.

## Platforms and scale

Windows and Linux native playback, import and restart acceptance are not complete. There is no Intel Mac build. Large-library performance and memory behaviour are unmeasured.

## What this means for you

- Treat key as a strong suggestion and check unfamiliar tracks by ear, especially minor-versus-major and neighbouring-key cases.
- Check the grid on tracks with live drums or tempo changes.
- Treat energy as a coarse, whole-track rating.
- Treat cues as good starting points. They are read-only today.

If you find a track where an answer is clearly wrong, an issue on the project's [GitHub](https://github.com/johnneerdael/CueRithm/issues) with the file type and what you expected helps. Please do not attach copyrighted audio.
