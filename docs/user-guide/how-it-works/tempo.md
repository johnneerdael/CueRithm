# How are BPM and the beatgrid found?

<p class="lead">CueRithm's timing detector produces exactly two things per track: one constant BPM and the time of the first beat. The beatgrid you see in the player is drawn from those two numbers.</p>

!!! abstract "What this means for you"
    - **The grid is constant.** A track with live drums, a long acceleration or a tempo-change edit will drift away from it. Check such tracks with [Check the beatgrid at beat level](../analysis/check-the-beatgrid.md).
    - **The tempo range is 98 to 195 BPM and cannot be changed.** A 94 BPM track is reported as 188, and a 70 BPM track as 140. Halve the number in your head, or fix the grid.
    - **There is no downbeat detection.** The first beat is the anchor, and the player's bar jumps count four beats from wherever you are.
    - **A wrong grid is fixable.** See [Fix a wrong beatgrid](../analysis/corrections/fix-the-beatgrid.md). Corrections do not change the detector's measured result.

!!! warning "What the grid is not"
    - It is constant. There is no variable-tempo map.
    - It has no downbeat detection. The first beat is the anchor.
    - The detector uses the left channel only and needs at least 10 seconds of audio.
    - Silence gives *no result* rather than a made-up 120 BPM or a zero anchor.

The detector is an independent Rust implementation of behaviour observed in Lexicon (see [How CueRithm was made](../help/about/provenance.md)). It is a local calculation with many explicit correction rules, not a learned model.

## What the detector listens to { #inputs }

The left channel of the whole track at 44.1 kHz. Tags, filenames and other apps' values are never read.

## How the BPM is chosen { #tempo }

1. **Amplitude windows.** The loudness envelope is measured over 10 ms windows. Peaks must be at least `30 / maximum BPM` seconds apart.
2. **Energy-change peaks.** A peak is a point where the squared energy of the next moment greatly exceeds the moment before. The threshold starts slightly under the maximum and steps down by 5% for up to 20 passes. A separate search of the first 5 seconds adds peaks that help the anchor stage.
3. **Interval voting.** Each peak is paired with the next nine. The intervals are folded into the BPM range by doubling or halving, and candidates within 0.5 BPM reinforce each other.
4. **Refinement.** Alternatives (rounded, floor and ceiling values, times 1.5, divided by 1.5, doubled and halved) are scored by normalised autocorrelation at 4, 8 and 16-beat lags. Special rules handle cases such as 130.5 to 174, and doubling for 85 to 90 and 90 to 115 BPM when the double scores better. A fine search in steps of 0.001 BPM uses a 64-beat lag.
5. **Confidence** is the top score divided by the sum of scores. It is an uncalibrated heuristic and is not shown in the app.

## How the first beat is found { #the-anchor-first-beat }

1. A spectral-flux onset envelope: an FFT of 512 samples, a hop of 128 and 24 log-spaced bands.
2. A dynamic-programming beat chain over that envelope.
3. Intro and drop selectors choose a reliable reference beat.
4. A walk-back to the true start, with attack checks over a 15 ms window.
5. Half-beat checks, riser validation, silent and weak-phase rescue, and a final grid validation.

## What is saved { #saved-values }

The BPM is rounded to two decimals, after a 15-digit correction, when it is saved. That rounded value is what the track table shows and what is written to tags: <span class="cr-bpm">128</span> for "17 (Extended Mix)" is stored as 128.00. The cue engine uses the unrounded BPM so it stays exact, and the first-beat time is not rounded.

Beats are generated as `first beat + n × 60 / BPM` to the end of the track. The player draws its grid from the saved BPM and first beat.

For a file's BPM tag, MP4 files get both the rounded integer `tmpo` atom and a two-decimal `BPM` field; other formats get the two-decimal value.

## Edge cases { #edge-cases }

- The timing stage rejects a first beat before 0:00. The cue stage itself tolerates one beat before the start, for a cropped intro, and clips a cue at that point to 0:00.
- Tracks outside 98 to 195 BPM are folded into the range, not rejected.
- Under 10 seconds, or on near-silence, there is no result and the track cannot become ready.

## Where you see it in the app { #where-you-see-it }

<!-- figure:how-beatgrid-zoom -->
<figure markdown>
![The player zoomed to 32 seconds on 17 (Extended Mix) at 128 BPM, with beat lines and bar markers around the drop at 1:15](../assets/screens/how-beatgrid-zoom-light.webp#only-light){ .screen loading=lazy }
![The player zoomed to 32 seconds on 17 (Extended Mix) at 128 BPM, with beat lines and bar markers around the drop at 1:15](../assets/screens/how-beatgrid-zoom-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The player draws its beat lines from one BPM and one first-beat time.</figcaption>
</figure>
<!-- /figure -->

| Place | What shows |
|---|---|
| **BPM** column in Tracks | The saved BPM with two decimals |
| The player | The beat lines and the bar markers drawn from the saved grid. ++left++ and ++right++ jump one bar (four beats) from the current position |
| File tags | The BPM field, if selected in **Settings → Automatic tagging** (on by default) |
| Smart playlists | BPM compares after rounding to two decimals: see [How do smart playlists stay current?](smart-engine.md#missing-values) |

If the BPM or grid is wrong, correct it with [Fix a wrong beatgrid](../analysis/corrections/fix-the-beatgrid.md). If the number differs from another app, read [My key or BPM differs from another app](../help/troubleshooting/key-or-bpm-differs.md). Agreement with a reference worker is described, with its limits, in [How accurate is it?](accuracy.md).

## Related

- [Check the beatgrid at beat level](../analysis/check-the-beatgrid.md)
- [Fix a wrong beatgrid](../analysis/corrections/fix-the-beatgrid.md)
- [How are cues placed?](cues.md)
- [How accurate is it?](accuracy.md)

<!-- verified: crates/camelot-core/src/lexicon_timing/mod.rs (98..195 defaults, 10 s minimum, anchor >= 0 check), tempo.rs (10 ms windows, 5% steps up to 20 passes, nine following peaks, 0.5 BPM, 4/8/16-beat autocorrelation, 130.5 to 174, 85-90 and 90-115 doubling, 0.001 fine search, 64-beat lag), dp.rs (FFT 512, hop 128, 24 bands), onset.rs and selection.rs (15 ms attack window), engine.rs (beats from unrounded BPM, silent audio gives no timing); lexicon_publication.rs round_bpm; publication.rs; structural_cues.rs (anchor tolerance, clipping); apps/desktop/src/published-analysis.ts (grid from saved BPM), App.tsx jumpBar (240/bpm), TrackTable.tsx (toFixed(2)); cuerithm-autotag write.rs apply_key_bpm; legacy technical/tempo.md re-checked. Dropped from legacy: the +/-20 ms in 2 ms steps walk-back detail (not confirmed in the code read) -->
