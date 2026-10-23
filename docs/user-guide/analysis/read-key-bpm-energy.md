# Read a track's key, BPM and energy

<p class="lead">In about ten minutes you will find the three numbers CueRithm measures for every track, learn what each one means, and use them to choose a track that mixes with the one you are playing.</p>

**You need:**

- [ ] CueRithm with a few prepared tracks in **Tracks**. If the table is empty, [prepare your first tracks](../get-started/first-tracks.md)
- [ ] Optional: speakers or headphones, for the last step

!!! note "Your numbers will differ"
    The examples use a demo library of 26 FLAC files from one Beatport purchase
    folder. Your tracks have other values; the steps are the same.

## 1. Find the three columns { #columns }

Click **Tracks** in the sidebar. Three of the default columns are the ones you
need: **KEY**, **BPM** and **ENERGY**.

Look at the row for *Come To Life (Original Mix)* by Cassian and AR/CO. It reads
<span class="cr-key">7A</span>, <span class="cr-bpm">130</span> BPM and
<span class="cr-energy">7/10</span>. A dash (—) in any of these columns means
CueRithm has no value for that track. Tracks in **Tracks** have finished
preparation, so a dash is rare.

CueRithm measured all three from the audio. It does not copy them from file tags or
filenames. If you want to see what the tags say, show the **Embedded key** and
**Embedded BPM** columns with [Show the columns you need](choose-columns.md).

## 2. Read the key { #key }

The **KEY** column shows a chip such as <span class="cr-key">7A</span>. This is a
Camelot code. The number says where the key sits on the wheel from 1 to 12. The
letter **A** means a minor key and **B** a major key. So <span class="cr-key">7A</span>
is D minor, and <span class="cr-key">8B</span> would be C major.

The chip is tinted by its position on the wheel. The tint only helps you spot
neighbours; the text is what counts.

You should see one key per track. CueRithm names one key for the whole track and
shows no confidence value. [Understand Camelot keys](camelot-keys.md) explains the
wheel and the rule for which keys mix.

## 3. Read the BPM { #bpm }

The **BPM** column shows the tempo with two decimals, for example `130.00`. CueRithm
finds one constant tempo and one first-beat time for the whole track, and draws the
[beatgrid](check-the-beatgrid.md) from those two numbers.

That makes the BPM reliable for tracks with a steady beat. It does not describe a
track whose tempo changes: CueRithm does not detect tempo changes and does not mark
which beat starts a bar. See
[What CueRithm does not do yet](../help/not-available-yet.md).

If you [correct the grid by hand](corrections/fix-the-beatgrid.md), the table shows
the corrected BPM.

## 4. Read the energy { #energy }

The **ENERGY** column shows a rating from 1 to 10 and a small bar, such as
<span class="cr-energy">7/10</span>. It is one rating for the whole track, worked out
from the track's loudness and spectrum.

CueRithm rates energy once per track. It does not rate sections, and the cues
carry no energy. Use the number to compare tracks with each other, not to judge
one moment of a track.

In the demo library, *Play With Fire (Original Mix)* is the only 8/10, eight tracks
are 7/10, fifteen are 6/10 and two are 5/10. Most club tracks cluster in a narrow
band, so a one-point difference is a small step.

## 5. Sort to compare { #sort-to-compare }

1. Click **BPM** once, then again. The arrow beside it points down, and the highest tempo comes first: *Nivk, Rabteu - PLV (Creeds remix - Extended)* at 160 BPM, then *Brackish (Original Mix)* and *Play With Fire (Original Mix)* at 140 BPM.
2. Click **ENERGY** twice. *Play With Fire (Original Mix)* comes first at 8/10, then the eight 7/10 tracks.
3. Click **TRACK** to return to title order.

Sorting never changes your tracks; it only reorders the table. See
[Find a track](find-tracks.md#sorting) for the other headings.

## 6. Use the three numbers together { #use-them-together }

Suppose *Come To Life (Original Mix)* is playing at <span class="cr-key">7A</span>,
<span class="cr-bpm">130</span> BPM and <span class="cr-energy">7/10</span>. Pick the next track in three questions:

1. **Does the key fit?** The same key, one number up or down with the same letter, or the same number with the other letter. For 7A that means 6A, 7A, 8A or 7B.
2. **Is the tempo close?** A gap of a few BPM is easy to blend. The compatible-tracks tool starts with a window of ±4 BPM.
3. **Which way should the energy go?** Equal holds the floor; higher lifts it.

| Track | Key | BPM | Energy | Why it fits |
|---|---|---|---|---|
| Say Less (Extended Mix) | 8A | 130 | 6/10 | One step up the wheel, same BPM, one point lower energy |
| Dancin (Extended Mix) | 6A | 131 | 6/10 | One step down the wheel, 1 BPM faster |
| 17 (Extended Mix) | 6A | 128 | 7/10 | One step down the wheel, 2 BPM slower, same energy |
| When The Fire Starts To Burn (Extended Mix) | 7A | 126 | 6/10 | Same key, but 4 BPM slower, which is the edge of that ±4 window |

Key compatibility is a guide for preparing and practising; listen to a pair before
you trust it. To have CueRithm list matches for you, use
[Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md),
which starts with a BPM tolerance of ±4. To keep a standing list, build a
[smart playlist](../playlists/build-a-smart-playlist.md).

## 7. Open the full detail { #open-the-full-detail }

Press the play button on the row for *Come To Life (Original Mix)*, or double-click
it. The track loads into the player at the bottom of the window and starts playing.
Press ++space++ to pause.

The player header repeats the numbers, with the BPM to two decimals, the **KEY**
chip and the **ENERGY** rating. Below it you see the waveform and the cue buttons.
The player has its own page: [Preview a track with the player](preview-a-track.md).

## 8. Know where the numbers come from { #where-the-numbers-come-from }

CueRithm analyses each track offline on your computer. The key comes from a neural
network that listens to the whole track, the tempo and first beat from a timing
detector, and the energy from a fixed formula. The library always keeps these
measurements. If you let CueRithm write key, BPM or energy into your files' tags,
the table still shows the measurement. The key and BPM written to a file can come
from the catalog instead, so a file's tag may differ from the table.

CueRithm's key and BPM can differ from another program's, and agreement is measured
rather than guaranteed. See
[My key or BPM differs from another app](../help/troubleshooting/key-or-bpm-differs.md)
and [How accurate is it?](../how-it-works/accuracy.md). For the methods, see
[How is the key detected?](../how-it-works/key.md),
[How are BPM and the beatgrid found?](../how-it-works/tempo.md) and
[How is energy rated?](../how-it-works/energy.md).

## What you learned

- The **KEY**, **BPM** and **ENERGY** columns are CueRithm's own measurements, one value each per track.
- The key is a Camelot code. The BPM is one constant tempo. The energy is one rating from 1 to 10.
- You can sort by BPM and energy, and use the key rule to choose a track that mixes.

## Next steps

- [Understand Camelot keys](camelot-keys.md)
- [Jump to the drop with cues](jump-to-cues.md)
- [Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md)
- [Show the columns you need](choose-columns.md)

<!-- verified: apps/desktop/src/components/TrackTable.tsx (key chip, bpm toFixed(2), energy N/10, sort buttons); components/PlayerPanel.tsx (header values); display.ts (camelot chip tint); crates/camelot-core/src/key.rs (Camelot mapping, D minor = 7A); crates/cuerithm-library/src/jobs.rs (stored key, bpm, energy); apps/desktop/src/views/CompatibleTracks.tsx (initial BPM tolerance 4, preparation-aid wording); apps/desktop/src/views/TaggingSettings.tsx (library keeps measured analysis); library-facts.md values; legacy learn/read-analysis.md and about/roadmap.md re-checked -->
