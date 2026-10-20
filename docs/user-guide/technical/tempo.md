# Tempo and beatgrid

CueRithm's timing detector produces exactly two things per track: one constant BPM and one first-beat time (the anchor). The grid you see is generated from them.

!!! warning "What the grid is not"
    - It is constant. There is no variable-tempo map.
    - There is no downbeat detection. The first beat is the anchor. Player "bar" jumps count four beats from the saved BPM.
    - The detector uses the left channel only, and needs at least 10 seconds of audio.
    - Its tempo range is 98 to 195 BPM, with no setting to change it. A 94 BPM track will be reported as 188.

The detector is an independent Rust implementation of behaviour observed in Lexicon (see [How CueRithm was made](../about/provenance.md)). It is a local calculation with many explicit correction rules, not a learned model.

## Inputs

The left channel at 44.1 kHz, the whole track. Silence gives *no result*: the Rust version deliberately returns nothing instead of a fake 120 BPM or a zero anchor.

## Tempo

1. **Amplitude windows** of 10 ms (rate divided by 100). Peaks must be at least `30 / maxBPM` seconds apart.
2. **Energy-change peaks.** A peak is where the squared energy of the future greatly exceeds the past. The threshold starts at the maximum minus 5% and steps down by 5% for up to 20 passes. Extra peaks come from the first 5 seconds.
3. **Interval voting.** Each peak is paired with the next nine. The intervals are folded into the BPM range, and candidates within 0.5 BPM reinforce each other.
4. **Refinement.** Alternatives (rounded, floor and ceiling values, times 1.5, divided by 1.5, doubled and halved) are scored by normalised autocorrelation at 4, 8 and 16-beat lags. Special rules handle cases such as 130.5 to 174, and doubling for 85 to 90 and 90 to 115 BPM when the double scores better. A fine search in steps of 0.001 BPM uses a 64-beat lag.
5. **Confidence** is the top score divided by the sum of scores. It is an uncalibrated heuristic.

## The anchor (first beat)

1. A spectral-flux onset envelope: FFT of 512, hop of 128, 24 log-spaced bands.
2. A dynamic-programming beat chain over that envelope.
3. **Intro and drop selectors** choose a reliable reference beat.
4. **Walk-back** to the true start, with attack checks (about a 15 ms window, searching plus or minus 20 ms in 2 ms steps).
5. **Half-beat checks, riser validation, silent and weak-phase rescue** and a final grid validation.

## Saved values

The BPM is rounded to two decimals (after a 15-digit correction) when published, which is what you see in the table and what is written to tags. The cue engine uses the unrounded BPM so it stays exact. The anchor is not rounded.

Beats are generated as `first_beat + n × 60 / BPM` to the end of the track.

## Cropped intros

A track that begins mid-beat can have an anchor slightly before zero. CueRithm accepts an anchor down to one beat before the start for the cue engine, keeps it for all internal calculations and clips only cues that sit exactly at it to zero.

## What it cannot do

- Variable tempo: live drumming, long accelerations and tempo-change edits will not follow the grid.
- Reliable results below 10 seconds or on near-silence.
- Tracks outside the tempo range, which are folded into it.

See [How accurate is it?](accuracy.md) for the evidence behind these statements.
