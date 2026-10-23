# Coming from Mixed In Key, Lexicon or rekordbox

<p class="lead">If you already analyse, tag or organise music with Mixed In Key, Lexicon or rekordbox, this page says what carries over to CueRithm, what is different, and how to keep using the tools you like next to it. Reading it takes about ten minutes.</p>

!!! info "Independent software"
    CueRithm is not affiliated with Mixed In Key, Lexicon, Beatport, Traxsource or rekordbox, and it does not claim to match any of them. Any agreement with another tool was measured on one library; it is not a guarantee for yours. [How CueRithm was made](../help/about/provenance.md) says what the owner studied and what is not taken from either product.

## The short version { #short-version }

| If you rely on… | In CueRithm | Read |
|---|---|---|
| A Camelot key such as 8A | The same codes, 1A to 12B, and nothing else in the table or player | [Understand Camelot keys](../analysis/camelot-keys.md) |
| An energy rating | One rating from 1 to 10 for the whole track, never per section and never on a cue | [How is energy rated?](../how-it-works/energy.md) |
| Cue points on drops and breakdowns | Up to six named, coloured cues, kept in the library and sent to your DJ software | [Cue names, letters and colours](../reference/cue-types.md) |
| A beatgrid | One constant BPM and a first beat | [How are BPM and the beatgrid found?](../how-it-works/tempo.md) |
| Cleaning titles, genres and artists | Rules you preview and can undo | [Clean up messy titles, genres and artists](../tagging/cleanup-messy-titles.md) |
| Playlists and smart playlists | Both, plus a key-aware "what mixes with this" search | [Build playlists and sets](../playlists/index.md) |
| Your DJ library | Written to directly by Sync, one way, with a backup | [Send your library to DJ software](../sync/index.md) |

CueRithm can [replace the active library from a supported Lexicon backup ZIP](../files/library/restore-library.md#lexicon), retaining imported cues and musical values without automatically analysing the audio. The current profile is `lexicon-main-db-v26`; mismatched schemas are refused. Generic Mixed In Key or rekordbox library import, selected-playlist import and merging independent libraries remain [separate planned work](../help/not-available-yet.md#dj-library-import). Starting from audio through [Add tracks](../files/add-tracks.md) still performs CueRithm’s own analysis.

## Keys: Camelot codes, measured on your computer { #keys }

CueRithm shows every key as a Camelot code. The number is the key's place on the wheel and the letter is minor (**A**) or major (**B**), so <span class="cr-key">6A</span> is G minor, <span class="cr-key">7A</span> is D minor and <span class="cr-key">8A</span> is A minor. Tracks one number apart on the same letter, or the same number on the other letter, mix harmonically; [Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md) uses that rule.

A small neural network that ships inside the app predicts the key from the whole decoded track, offline and without an account. It never reads a key written into the file by another program. On 282 development tracks it agreed exactly with stored Mixed In Key 11 key labels on 274, and was one Camelot step or a relative or parallel key away on the other 8. That is agreement with another tool's labels on the owner's own library, not proof that either is right, so a track whose key differs is a reason to listen, not a fault. [How accurate is it?](../how-it-works/accuracy.md) lists what was and was not measured, and [My key or BPM differs from another app](../help/troubleshooting/key-or-bpm-differs.md) helps you decide who is right.

## Energy: one number, and it will not match yours { #energy }

CueRithm's energy rating comes from a fixed formula over the whole track. It does not use a model, a catalog or another tool's tags, and it does not label cues or sections with energy. Agreement with other tools' energy numbers on real music has not been measured, so treat it as a coarse sort key and expect different values from the program you used before. If you set up your sets around another tool's energy ratings, keep using that tool for them.

## Cues: structural, named and kept in the library { #cues }

CueRithm places a **Start**, **Drop**, **Breakdown**, **Drop #2**, **Breakdown #2** and **End / Fade-out** cue where the track has them, in yellow, red, blue, red, blue and violet, plus an optional **S.O.S.** emergency loop. The player labels them with the letters A, B, C and so on in time order. The cue code is the part of CueRithm closest to Lexicon's behaviour as the owner observed it; it does not imitate Mixed In Key's cue placement. Cues are never written into the audio file's tags. They live in the library and reach your DJ software when you sync.

You can move, rename or add cues, save loops and reuse a layout on many tracks without disturbing the measured result: [Fix a wrong cue, loop or beatgrid](../analysis/corrections/index.md).

## Beatgrids and BPM { #grids }

The grid is one constant BPM and a first beat. Tracks with a live drummer or a tempo change drift away from the beat lines, and there is no variable grid or downbeat detection ([What CueRithm does not do yet](../help/not-available-yet.md#variable-tempo)). The detector searches 98 to 195 BPM, so a 94 BPM track can show as 188. The table shows BPM with two decimals. You can halve, double or retype the BPM and move the first beat: [Fix a wrong beatgrid](../analysis/corrections/fix-the-beatgrid.md).

## Keep both tools in your workflow { #both-tools }

CueRithm changes your audio files only through features you switch on or start yourself: automatic tagging, cleanup, **Edit metadata** and the optional file renaming and moving. With tagging and cleanup off, preparation never writes to a file ([Analyze tracks without tagging or cleanup](../files/recipes/analyze-only.md)). Its measurements live in its own library, which is why it can sit beside another tool.

| You want to keep… | Do this in CueRithm |
|---|---|
| Key and energy tags another tool wrote | Leave **Automatic tagging** off. If you use it, untick **Write** for Key and BPM in **Fields to write** and switch off **Write track energy to files** ([Choose providers and fields to write](../tagging/choose-providers.md)). |
| The key notation you are used to in file tags | Tick **Write Camelot keys** to write `8A`. It is off by default, which writes musical key names. |
| rekordbox's own beatgrid | In **Include in sync**, untick **Beatgrids**, or choose **Keep rekordbox** on the beatgrid row ([Resolve rekordbox sync conflicts](../sync/rekordbox-conflicts.md)). |
| Cues you made in rekordbox | CueRithm does not take over cues it did not create. Untick **Cues and loops**, or leave the cue row on **Keep rekordbox**. |
| rekordbox's key or BPM values | Under **Metadata fields**, untick **Key** and **Bpm** ([Sync to rekordbox 6 or 7](../sync/rekordbox-direct.md)). |

A workable split is to let CueRithm prepare and tag new purchases, then sync playlists to rekordbox and fine-tune cues there. Resync later with **What to sync** set to **Modified since last sync** ([Choose what to sync](../sync/choose-what-to-sync.md)), and rely on the preview and **Restore backup** ([Preview, apply and restore a sync](../sync/preview-apply-restore.md)). Sync goes one way, from CueRithm to your DJ software, and uses rekordbox's database directly; CueRithm does not read or write collection XML. Direct writes were checked on rekordbox 6.8.7.0303 and 7.2.19.0342 on macOS ([Check what works with your DJ software](../help/compatibility.md)).

If two tools both write tags to the same files, the last one to run wins for fields they share. Preview in each, and keep the file originals that CueRithm retains for **Undo** ([Undo a tag change](../tagging/tag-history.md)).

## What CueRithm does not do { #limits }

- No import of another program's library, and no collection XML.
- No variable-tempo grids, no per-section energy and no energy labels on cues.
- No plays, last played or danceability, and no sync to Traktor.

[What CueRithm does not do yet](../help/not-available-yet.md) keeps the full list.

## What this means in CueRithm { #what-this-means }

Start with a copy of a small folder, [prepare it](first-tracks.md), and compare a few tracks with what your current tool says before you trust the rest. Decide field by field what CueRithm may write, and let [Send your library to DJ software](../sync/index.md) preview before it changes anything. [Pick the right guide for your goal](choose-your-path.md) lists where each of these lives.

## Related

- [How CueRithm was made](../help/about/provenance.md) explains what was studied and what was not taken.
- [How accurate is it?](../how-it-works/accuracy.md) lists every figure and its limits.
- [Prepare your first tracks](first-tracks.md) is the quickest way to compare results on your own music.

<!-- verified: README.md (Credits, provenance, analysis and file-handling sections); AGENTS.md (one whole-track energy; no Mixed In Key cue inference; no energy on cues; no parity claims); docs/research/dj-structure-interoperability.md (read; it predates the product and documents other programs' limits, so no claim here relies on it); docs/release-readiness.md (no parity claim; rekordbox direct; no collection XML); crates/camelot-core/src/key.rs (Camelot mapping and compatible_with); crates/camelot-core/src/lexicon_publication.rs and reference/cue-types.md (published cue names and colours); crates/cuerithm-autotag/src/config.rs and write.rs (analysis_tag_source Measured, write_energy default true, Write Camelot keys), views/TaggingSettings.tsx (Fields to write, Key and BPM written to files, Write Camelot keys, Write track energy to files); views/RekordboxSync.tsx (Include in sync, Metadata fields, What to sync, Modified since last sync); crates/cuerithm-library/src/jobs.rs and how-it-works/pipeline.md (key, BPM and energy computed from audio only); how-it-works/accuracy.md figures (274 of 282); help/not-available-yet.md and help/compatibility.md (no library import, no Traktor). UNVERIFIED: nothing is claimed about how Mixed In Key, Lexicon or rekordbox behave beyond the user-facing facts above; "last writer wins" is the general consequence of writing the same tag field twice, not tested against those programs. -->
