# The cue engine

Cues are the structural landmarks you see in the player: Start, Drop, Breakdown, Drop #2, Breakdown #2, End / Fade-out and, optionally, an emergency loop. They are detected from the track's energy shape, section by section.

!!! note "Provenance"
    I describe the cue detector as the "validated Lexicon-compatible DSP". Its source keeps references to functions in the program it was studied from. See [How CueRithm was made](../about/provenance.md) for the origin and limits of this work.

## Inputs

The left channel at 44.1 kHz, the unrounded BPM and the anchor from [Tempo and beatgrid](tempo.md), and the track's genre tag. If timing failed, there are no cues, and the track cannot become ready.

## Front end

1. **Filtered signal.** Seven cascaded second-order low-pass filters at 800, 400, 400, 200, 200, 200 and 200 Hz (resonance of 1 dB, matching the Web Audio default).
2. **Second signal.** A six-pass single-pole low-pass at 300 Hz for the alternative drop path.
3. **Genre signal.** An 80 Hz six-pass low-pass, used only for genre overrides.
4. **Sections.** The track is cut into 4-beat sections. For each: RMS energy, per-beat strength within plus or minus 50 ms, and a "ramp" class.

## Stages

Each stage builds on the previous one.

1. **Drop.** Two candidates, one from each filtered signal. A section qualifies when its rolling four-section mean is at least 85% of the track's maximum rolling mean and its energy is at least 65% of the peak. The 300 Hz path also needs transients at least 20% of the maximum. An earlier plateau after an energy dip can replace a late candidate. The two paths are reconciled with ratio, sustain (at least 0.85) and contrast (at least 1.4) rules, then refined up to 16 sections, snapped to the 4-beat grid and phase-fixed.
2. **Breakdown.** Searched in 64-beat steps (up to 30 steps) after the drop. The score multiplies the energy dip, the contrast, a position preference (a Gaussian centred at 48% of the track or 192 beats from the drop) and the energy fraction. It is zeroed if the breakdown is within 30 seconds of the end or before 25% of the track. It must stay below 85% of the drop level afterwards and be at least your **Minimum breakdown length** (default 64 beats, rounded up to a 16-beat multiple).
3. **Second drop.** Scans from at least 96 beats after the breakdown up to 82% of the track, with three tiers: principled, energy fallback (needs a breakdown) and heuristic fallback.
4. **Drop at start = Never.** May promote the second drop to the real Drop and re-detect, unless a sustained-energy profile says otherwise.
5. **Second breakdown**, with a simpler dip fallback.
6. **Last beat (End / Fade-out).** Per-beat RMS against the 75th percentile, the last run of at least 16 "active" beats, snapped to a 16-beat grid, at least 4 beats.
7. **Emergency loop** (optional). A 16-beat loop at the best late steady four-section block.

The Start cue always exists when timing exists. Genre influences only tracks whose genre tag contains "techno" or "hardcore".

## Published names, colours and types

| Detector name | Published as | Colour | Type |
| --- | --- | --- | --- |
| Start | Start | yellow | hot cue |
| Drop | Drop | red | hot cue |
| Breakdown | Breakdown | blue | hot cue |
| Second drop | Drop #2 | red | hot cue |
| Second breakdown | Breakdown #2 | blue | hot cue |
| Last beat | End / Fade-out | violet | hot cue |
| Emergency loop | S.O.S. | magenta | active loop, 16 beats |

Slots follow list order, and the emergency loop takes the highest free slot. The detector's raw colour names are replaced by these published colours.

## When a cue is missing

- No timing, silence or empty audio: no cues.
- No Drop: no Breakdown, Second drop, Second breakdown or loop either.
- Breakdown can be dropped by the minimum length, the 25% rule or the 30-second end gap.
- Second drop and Last beat may simply not be found.

## Revisions

Each preparation stores a new cue revision. Earlier revisions are kept. Cues are recomputed when the effective genre changes after tagging or cleanup. Key, BPM and energy are preserved when that happens. Cues are never written to file tags.

## Settings that matter

| Setting | Default | Effect |
| --- | --- | --- |
| Minimum breakdown length | 64 beats | Shorter breakdowns are ignored. |
| Drop at start | Never | Whether a drop can sit at the start. |
| Emergency loop | off | Adds the S.O.S. loop. |

There is no cue template editor, no custom anchors and no tempo-range option yet.
