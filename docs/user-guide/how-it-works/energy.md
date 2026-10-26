# How is energy rated?

<p class="lead">Energy is one whole-track rating from 1 to 10, computed from the audio with a fixed formula. It uses no model file, and CueRithm shows no energy for individual sections or cues.</p>

!!! abstract "What this means for you"
    - **Treat it as a coarse description of the track's overall texture**, not a rating of one drop. A dense, loud, constantly pounding track rates higher than a sparse, dynamic one.
    - **There is no energy per cue or per section, on purpose.** A label on a Drop would suggest precision the app does not claim.
    - **Energy works well in smart playlists** because it is a single number: for example "Energy 7 and up" in the demo library matches 9 of the 26 tracks. See [Build a smart playlist](../playlists/build-a-smart-playlist.md).
    - **Real-music agreement with another tool has not been measured**, only synthetic checks. See [How accurate is it?](accuracy.md).

## Why only one number { #why-only-one-number }

Earlier research explored per-section energy labels on cues. CueRithm uses a single whole-track rating because it is easier to trust and to use in a smart playlist. Energy labels on structural cues would imply precision the app does not claim.

## How the rating is calculated { #the-calculation }

Method id `lexicon-acoustic-energy-v1`.

1. **Input.** The whole track, mono, 22,050 Hz, with no loudness normalisation.
2. **Frames.** 2,048 samples with a hop of 1,024, a Hann window, FFT magnitudes scaled by 2/2048, DC excluded.
3. **Per frame**, compute the RMS, the spectral flatness, the fraction of power between 2,000 and 5,000 Hz (*high-mid*) and the fraction between 20 and 60 Hz (*sub-bass*).
4. **Keep** only frames whose RMS is at least 12% of the loudest frame.
5. **Crest factor.** `crest_db = 20·log10(peak) − 20·log10(retained RMS)`.
6. **Score and level:**

```text
score = 9.0124 − 0.49685·crest_db + 21.436·flatness + 10.183·highmid + 2.9802·sub
level = clamp(floor(score + 0.5), 1, 10)
```

A track with a peak at or below 1e-10, or shorter than 2,048 samples, gets no rating and cannot become ready.

## How to read it { #how-to-read-it }

A lower crest factor (a more compressed, constant-loudness track), more spectral flatness (noisier, denser) and more high-mid and sub-bass power all push the number up. A sparse, dynamic track scores lower. In the demo library most tracks sit between <span class="cr-energy">5/10</span> and <span class="cr-energy">7/10</span>, and "Play With Fire (Original Mix)" is the one <span class="cr-energy">8/10</span>. The rating describes the whole track's texture; it does not rate an individual drop.

## Where the formula comes from { #provenance-of-the-coefficients }

CueRithm's Rust code independently implements an acoustic calculation observed in Lexicon. The historical origin of its fixed coefficients is unknown, and the expression may originally have been fitted. A test fixture of 29 synthetic audio cases checks the code: 26 usable cases match the reference levels, and 3 (empty, shorter than one frame, and silent) deliberately return no rating here. Real-music agreement has not been measured.

## Where it goes { #where-it-goes }

The number is stored in the library, shown as `N/10`, available in smart playlists as **Energy**, and written to the audio file's `ENERGY` custom field when **Write track energy to files** is on, which is the default.

<!-- figure:how-energy-7-and-up -->
<figure markdown>
![The Energy 7 and up smart playlist listing 9 tracks, each with its ENERGY value of 7/10 or 8/10](../assets/screens/how-energy-7-and-up-light.webp#only-light){ .screen loading=lazy }
![The Energy 7 and up smart playlist listing 9 tracks, each with its ENERGY value of 7/10 or 8/10](../assets/screens/how-energy-7-and-up-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Because energy is one number, a smart playlist can select on it directly.</figcaption>
</figure>
<!-- /figure -->

| Place | What shows |
|---|---|
| **Energy** column in Tracks | `7/10` with a small bar |
| Smart playlists | The **Energy** field, 0 to 10: see [Smart playlist fields and operators](../reference/smart-playlist-fields.md) |
| Automatic smart playlists | Release-month lists split by energy: Warm-Up (1 to 5), Dancefloor (6 to 7), Anthems (8 to 10) |
| File tags | The `ENERGY` field, controlled by **Write track energy to files** in **Settings → Automatic tagging** |

To read the three analysis columns together, see [Read a track's key, BPM and energy](../analysis/read-key-bpm-energy.md).

## Related

- [Read a track's key, BPM and energy](../analysis/read-key-bpm-energy.md)
- [Write smart playlist rules](../playlists/smart-rules.md)
- [How accurate is it?](accuracy.md)
- [How does CueRithm analyse a track?](pipeline.md)

<!-- verified: crates/camelot-core/src/lexicon_energy.rs (METHOD, frame 2048, hop 1024, 12% threshold, crest, coefficients, level clamp, None for short or silent), engine.rs (22,050 Hz mono input), tests/lexicon_energy.rs + tests/fixtures/lexicon-energy/reference.json (29 cases, 3 with no level), cuerithm-autotag write.rs (ENERGY raw field, write_energy default true), jobs.rs (Energy is unresolved), apps/desktop/src/components/TrackTable.tsx (N/10 + bar), AutomaticPlaylistOptions.tsx (Warm-Up 1-5, Dancefloor 6-7, Anthems 8-10), TaggingSettings.tsx (Write track energy to files); the private demo-library facts sheet (energy values; 9 tracks at 7 or more); legacy technical/energy.md re-checked -->
