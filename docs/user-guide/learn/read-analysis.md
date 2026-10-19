# Read key, BPM and energy

**You will:** learn what the three analysis columns mean and how to use them to pick the next track.

## Key

Key appears as a chip such as **7A** or **11B**. CueRithm uses Camelot notation: the number is the position on the wheel, **A** is a minor key and **B** is a major key.

Two tracks mix harmonically when their keys are the same, one step apart on the number with the same letter, or the same number with the other letter (relative major and minor). In a smart playlist you can use the rule *is compatible with key* to let CueRithm do this for you.

!!! note "Where the key comes from"
    The key is predicted by a neural network that runs inside the app, on the whole track. It is never read from file tags. See [Key detection](../technical/key.md).

## BPM

BPM is shown with two decimals, for example `124.00`. CueRithm detects one constant tempo and one first-beat position for the whole track. The beat lines in the player are generated from those two numbers.

!!! warning "One tempo, no downbeats"
    The beatgrid is constant. CueRithm does not detect tempo changes and does not mark which beat is the first beat of a bar. For tracks with a clear constant tempo this is what you want. For live-drummer or rubato material, treat the grid with care. See [Tempo and beatgrid](../technical/tempo.md).

## Energy

Energy is a **single rating from 1 to 10 for the whole track**, shown as `6/10` with a small bar.

It is computed from the track's loudness shape and spectrum, not from a label. It tells you how hard a track hits overall, not how any one moment compares. CueRithm deliberately does not show per-segment energy and does not attach energy labels to cues.

<figure markdown>
  ![Table header and first rows with BPM, key chip and energy bar](../assets/screens/crop-table-head.webp){ .crop loading=lazy }
  <figcaption>BPM, key and energy as they appear in the table.</figcaption>
</figure>

## Use them together

1. Sort by **BPM** or **ENERGY** by clicking the column header. Click again to reverse.
2. Look for a track with a compatible key and a BPM within a few percent.
3. Pick the energy to suit the moment: a similar rating to hold the floor, a higher one to lift it.

## Open the full detail

Double-click a track, or press the play button on its row, to load it into the player. The player shows the same BPM, key and energy, plus the waveform and cues. Continue with [Navigate a track with cues](navigate-with-cues.md).
