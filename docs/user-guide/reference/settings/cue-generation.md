# Set breakdown length, drop at start and the emergency loop

<p class="lead">The Cue generation section has four controls that shape the structural cues CueRithm places on every track it prepares. Reading this page takes two minutes; the defaults suit most club music.</p>

<!-- figure:existing-11-settings-cue-generation-reference-settings-cue-generation -->
<figure markdown>
![Cue generation settings](../../assets/screens/11-settings-cue-generation.webp){ .screen loading=lazy }
<figcaption>The Cue generation section of Settings.</figcaption>
</figure>
<!-- /figure -->

!!! tip "New to this?"
    Start with [Choose how cues are placed](../../analysis/cue-rules.md), which shows what each choice does to a real track. The names and colours of the cues themselves are on [Cue names, letters and colours](../cue-types.md).

## The four controls { #controls }

Open **Settings → Cue generation**. The section is headed "Validated structural cues, applied after full analysis."

| Control | What it does | Default | Valid values | Task page |
|---|---|---|---|---|
| **Existing cue points** | Shows that cues are replaced after a successful analysis. A preparation that fails keeps the track's previous cues. This is a fixed behaviour, shown with a tick, not a choice. | **Replace** | Not editable | [Analyze a track again](../../analysis/analyze-again.md) |
| **Minimum breakdown length** | The shortest breakdown, in beats, that earns a **Breakdown** cue. Shorter dips in energy are ignored. CueRithm measures a breakdown from where it starts to where the energy comes back, rounds that up to a multiple of 16 beats, and compares it with this number. | `64` | A whole number from `1` to `4096` | [Choose how cues are placed](../../analysis/cue-rules.md) |
| **Drop at start** | Whether the **Drop** cue may sit at the very beginning of a track. **Never** moves a drop that arrives almost immediately on to the next one, when the track has a later drop to promote. **Automatic** lets the detector keep a drop at the start. | **Never** | **Never**, **Automatic** | [Choose how cues are placed](../../analysis/cue-rules.md) |
| **Emergency loop** | Adds a 16-beat loop labelled **S.O.S.** when the track has a suitable late, steady section. The loop is saved as an active loop and shown in magenta. When the box is off, no loop is added. | Off | On or off | [Set a loop](../../analysis/corrections/set-loops.md) |

Press **Save settings** to apply a change. A value outside `1` to `4096` is refused when you save, and the number box stops at those limits.

## When a change takes effect { #when }

New settings apply to tracks prepared after you save. Tracks that are already in **Tracks** keep their cues until you choose **Analyze again** on them ([Analyze a track again](../../analysis/analyze-again.md)); that is also the only way finished results change. After a successful analysis the new cues replace the old ones, so a cue you moved by hand is replaced too. To keep a hand-made layout, use a cue template ([Reuse one cue layout on many tracks](../../analysis/corrections/cue-templates.md)), which is saved separately.

Cue generation is part of a [preparation preset](incoming.md#preparation-presets), so a folder can use its own breakdown length.

## What these settings do not control { #not-controls }

- **Which cues exist.** CueRithm always tries for Start, Drop, Breakdown, Drop #2, Breakdown #2 and End / Fade-out, and drops any it cannot find. See [Cue names, letters and colours](../cue-types.md) and [How are cues placed?](../../how-it-works/cues.md).
- **Genre handling.** The detector reads the track's genre tag and treats techno and hardcore a little differently. There is no control for it.
- **Energy.** A track has one whole-track energy rating. Cues carry no energy of their own ([How is energy rated?](../../how-it-works/energy.md)).
- **Tempo range.** There is no setting; tempo and beatgrid are always found over the full track ([How are BPM and the beatgrid found?](../../how-it-works/tempo.md)).

## Related

- [Choose how cues are placed](../../analysis/cue-rules.md)
- [Cue names, letters and colours](../cue-types.md)
- [Fix a wrong cue, loop or beatgrid](../../analysis/corrections/index.md)
- [Look up a setting](index.md)

<!-- verified: apps/desktop/src/views/Settings.tsx (Cue generation section: labels, min 1 max 4096, Never/Auto, checkbox); crates/cuerithm-library/src/settings.rs (default DropAtStart::Never, add_emergency_loop false, min_cue_length_beats 64 via LexiconConfig::default); crates/camelot-core/src/structural_cues.rs (1..=4096); crates/camelot-core/src/lexicon_cues.rs (breakdown 16-beat rounding, drop-at-start Never branch); crates/cuerithm-library/src/publication.rs (S.O.S. row: active loop, magenta, 16 beats; DropAtStart mapping); apps/desktop/src/test/settings-pages.test.tsx; legacy guides/settings.md and technical/cues.md re-checked. publication.rs apply() publishes fresh cues with no existing cues passed in, so Analyze again replaces hand-edited cues. -->
