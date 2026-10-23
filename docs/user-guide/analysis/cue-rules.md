# Choose how CueRithm places cues

<p class="lead">Three settings and one input decide which cues a track gets: a minimum breakdown length, a rule for drops at the start, an optional emergency loop, and the track's genre. Changing them takes a minute; applying them to tracks you already prepared takes one more step.</p>

<!-- figure:existing-11-settings-cue-generation-cue-rules -->
<figure markdown>
![Cue generation settings](../assets/screens/11-settings-cue-generation.webp){ .screen loading=lazy }
<figcaption>The cue rules live under Settings → Cue generation.</figcaption>
</figure>
<!-- /figure -->

**Before you start:** these rules apply when a track is prepared. Tracks that are already finished keep their cues until you choose [Analyze again](analyze-again.md) for them.

## 1. Open the cue settings { #open }

Choose **Settings** at the bottom of the sidebar, then **Cue generation**. The page reads *Validated structural cues, applied after full analysis.*

| Control | What it does | Default |
|---|---|---|
| **Existing cue points** | Fixed to *Replace*. After a successful analysis the new cues replace the old ones. If preparation fails, the previous cues are kept. | Replace |
| **Minimum breakdown length** | A breakdown shorter than this many beats is not placed. Accepts 1 to 4096. | 64 |
| **Drop at start** | **Never** or **Automatic**. Whether the Drop cue may sit at the very start of a track. | Never |
| **Emergency loop** | When on, a track with a suitable late section also gets a 16-beat loop cue named **S.O.S.** | Off |

Change a value, then select **Save settings** at the top right. A value outside 1 to 4096 beats is refused.

## 2. Pick a minimum breakdown length { #minimum-breakdown }

The setting counts beats, but you hear seconds. Convert with `beats × 60 ÷ BPM`:

| Minimum | At 128 BPM | At 140 BPM |
|---|---|---|
| 16 beats | 7.5 s | 6.9 s |
| 32 beats | 15.0 s | 13.7 s |
| 64 beats (default) | 30.0 s | 27.4 s |
| 128 beats | 60.0 s | 54.9 s |

Lower the number if you want short breakdowns to get a cue. Raise it to keep only long ones.

Not every track gets every cue, whatever the setting. These three demo tracks, prepared with CueRithm 0.1.13, show how much they vary:

| Track | BPM | Cues |
|---|---|---|
| Man Like Me (Extended Mix) | <span class="cr-bpm">128</span> | <span class="cr-cue cr-cue--start">Start</span> 0:00, <span class="cr-cue cr-cue--drop">Drop</span> 1:00, <span class="cr-cue cr-cue--breakdown">Breakdown</span> 1:53, <span class="cr-cue cr-cue--drop">Second Drop</span> 3:15, <span class="cr-cue cr-cue--end">Last beat</span> 5:15 |
| Desert (Original Mix) | <span class="cr-bpm">128</span> | <span class="cr-cue cr-cue--start">Start</span> 0:00, <span class="cr-cue cr-cue--drop">Drop</span> 2:45, <span class="cr-cue cr-cue--drop">Second Drop</span> 5:15, <span class="cr-cue cr-cue--breakdown">Second Breakdown</span> 6:00, <span class="cr-cue cr-cue--end">Last beat</span> 6:15 |
| Prophecy (Extended Mix) | <span class="cr-bpm">130</span> | <span class="cr-cue cr-cue--start">Start</span> 0:00, <span class="cr-cue cr-cue--drop">Drop</span> 0:59, <span class="cr-cue cr-cue--drop">Second Drop</span> 2:57, <span class="cr-cue cr-cue--end">Last beat</span> 4:26 |

If a cue you expect is missing, lowering the minimum is the first thing to try. If that does not help, add the cue yourself; see [Move or add a cue](corrections/move-or-add-cues.md). Names, letters and colours are listed in [Cue names, letters and colours](../reference/cue-types.md).

## 3. Decide about drops at the start { #drop-at-start }

Some tracks open at full energy. **Drop at start** decides what happens to the Drop cue then.

- **Never** (the default) keeps the Drop cue out of the opening of such a track. CueRithm can use the next drop as the main Drop instead.
- **Automatic** lets the Drop cue sit at the start for a track that opens at high energy.

The setting only matters for tracks with an early drop, so most tracks come out the same either way.

## 4. Switch the emergency loop on or off { #emergency-loop }

Tick **Emergency loop** if you want a loop you can fall back on late in the track. When CueRithm finds a suitable late section, it adds a 16-beat loop cue named **S.O.S.** to the track's cues. Tracks without a suitable section get none, so you will not find one on every track. The loop is a cue like any other; you can [set loops by hand](corrections/set-loops.md) as well.

## How the genre affects cues { #genre }

CueRithm reads the track's genre tag as an input. Techno and hardcore genres get their own handling when the first drop is found; other genres, and tracks with no genre tag, use the standard handling. When automatic tagging or cleanup changes a track's genre, CueRithm recalculates the cues with the final genre before the track finishes. Key, BPM and energy are not recalculated for that.

## 5. Apply the new rules { #apply }

- **New tracks** use the rules as soon as you save.
- **Tracks queued before you saved** keep the rules they were added with.
- **Finished tracks** change only when you choose [Analyze again](analyze-again.md).
- **One folder, different rules:** make a [preparation preset](../files/presets.md) with these settings and assign it with [Use different rules for different folders](../files/folder-presets.md). Cue generation is part of every preset.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| Cues did not change after saving | The track was already finished | Use [Analyze again](analyze-again.md) |
| Fewer cues than expected | Minimum breakdown length is too high for the track, or the track has no clear section | Lower the minimum; add the cue by hand |
| My hand-placed cues disappeared | **Existing cue points** is fixed to *Replace*, and Analyze again recalculates them | Reapply them, or use a [cue template](corrections/cue-templates.md) |
| No S.O.S. loop on a track | **Emergency loop** is off, or no suitable section exists | Switch it on; set a loop by hand |
| Save is refused | Minimum breakdown length is outside 1 to 4096 | Enter a value in range |

## Related

- [Cue generation settings reference](../reference/settings/cue-generation.md)
- [How are cues placed?](../how-it-works/cues.md)
- [Jump to the drop with cues](jump-to-cues.md)
- [Fix a wrong cue, loop or beatgrid](corrections/index.md)

<!-- verified: apps/desktop/src/views/Settings.tsx (Cue generation page: labels, notes, min 1 max 4096, Never/Automatic, Emergency loop); crates/cuerithm-library/src/settings.rs (defaults: min_cue_length_beats 64 from LexiconConfig::default, DropAtStart::Never, add_emergency_loop false); crates/camelot-core/src/structural_cues.rs (1..=4096 validation); crates/camelot-core/src/lexicon_cues.rs (user effects only: breakdown rejected under the minimum, Never moves an opening drop to the next drop, S.O.S. loop only when a late section qualifies, genre techno/hardcore branches; DSP not restated); crates/camelot-core/src/engine.rs (refresh_prepared_cues recomputes cues only when the genre changes, keeps key, BPM, energy); crates/cuerithm-library/src/prepare.rs (cue refresh after tagging); crates/cuerithm-library/src/publication.rs (Never = never, Automatic = highEnergyOnly; S.O.S. is a 16-beat active loop); crates/cuerithm-library/src/jobs.rs (store_analysis_tx inserts a new cue revision); docs/user-guide/_meta/library-facts.md for the three demo tracks; legacy guides/settings.md#cue-generation re-checked -->
