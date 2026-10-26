# Build a smart playlist

<p class="lead">In about ten minutes you build <strong>Melodic house 128+</strong>, a playlist that fills itself with every melodic house track at 128 BPM or faster, and you see how to change its rules safely.</p>

A smart playlist holds no tracks of its own. You describe the tracks you want with conditions, and CueRithm keeps the list in step with your library.

**You need:**

- [ ] CueRithm with prepared tracks that carry a genre tag and a measured BPM ([Read a track's key, BPM and energy](../analysis/read-key-bpm-energy.md) shows where BPM comes from)
- [ ] A rough idea of the genre names in your library. A genre rule is only as good as the tag it reads; [Clean up messy titles, genres and artists](../tagging/cleanup-messy-titles.md) tidies them.

## 1. Start a smart playlist { #start }

1. Click the **+** button beside **PLAYLISTS** (**Create playlist**).
2. Type `Melodic house 128+` in **Playlist name**.
3. Set **Playlist type** to **Smart playlist**.

**You should see** *Loading rule fields…* for a moment, then a **Match** line that reads **all conditions** with a counter (`1/128`) and one starter condition: the field **Title**, the operator **contains** and an empty value. The page subtitle reads *Rules keep membership up to date automatically.* and a note under the rules says *Calendar dates use* followed by your time zone.

**Why this form:** the same editor handles playlists, folders and smart playlists. Choosing the type decides which parts it shows.

## 2. Set the first condition { #first-condition }

<!-- figure:existing-55-smart-playlist-editor-build-a-smart-playlist -->
<figure markdown>
![The smart playlist rule editor](../assets/screens/55-smart-playlist-editor.webp){ .screen loading=lazy }
<figcaption>Each condition is a field, an operator and a value.</figcaption>
</figure>
<!-- /figure -->

1. In the field menu of condition 1, choose `genre`. The list shows friendly names for common fields, such as Title, Artist, BPM, Key and Energy, and raw names for the rest, so `genre` is written in lower case.
2. Leave the operator on **contains**.
3. Type `Melodic House` in the value box.

**Why `contains`:** Beatport writes the genre as *Melodic House & Techno*. The text operators ignore upper and lower case, and **contains** matches any part of the value, so one rule catches the whole family.

## 3. Add a second condition { #second-condition }

1. Click **Add condition**. A new starter condition appears.
2. Change its field to **BPM**. The operator becomes **equals**, the first choice for a number field.
3. Change the operator to **is at least** and type `128` in the number box.
4. Leave **Match** on **all conditions**, so a track must satisfy both.

**You should see** `2/128` beside **Match**. Each condition also has arrows to move it and an ✕ to remove it.

## 4. Preview the matches { #preview }

Click **Preview matches**.

<!-- figure:playlists-smart-editor-melodic -->
<figure markdown>
![The smart playlist editor named Melodic house 128+ with the conditions genre contains Melodic House and BPM is at least 128, Match all conditions, and the preview reading 10 matching tracks](../assets/screens/playlists-smart-editor-melodic-light.webp#only-light){ .screen loading=lazy }
![The smart playlist editor named Melodic house 128+ with the conditions genre contains Melodic House and BPM is at least 128, Match all conditions, and the preview reading 10 matching tracks](../assets/screens/playlists-smart-editor-melodic-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Preview matches counts the tracks before you create the playlist.</figcaption>
</figure>
<!-- /figure -->

**You should see** *10 matching tracks* beside the button. With the Beatport purchase folder used in this guide, they are:

| Track | Genre | BPM |
|---|---|---|
| 17 (Extended Mix) | Melodic House & Techno | 128 |
| Atoma (Extended Mix) | Melodic House & Techno | 130 |
| Come To Life (Original Mix) | Melodic House & Techno | 130 |
| Here In My Arms (Enjoy The Silence) (Extended Mix) | Melodic House & Techno | 132 |
| Knockout (Extended Mix) | Melodic House & Techno | 130 |
| Murda (Extended Mix) | Melodic House & Techno | 128 |
| No Good (Extended Mix) | Melodic House & Techno | 130 |
| Other Dimension (Extended Mix) | Melodic House & Techno | 128 |
| Prophecy (Extended Mix) | Melodic House & Techno | 130 |
| Say Less (Extended Mix) | Melodic House & Techno | 130 |

*Palm Of Your Hands (Extended Mix)* is also melodic house, at 127 BPM, so the BPM condition leaves it out.

**Why you must preview:** **Create** stays greyed out until a preview of exactly these rules has succeeded. Change anything and the preview clears, so what you save is always what you checked.

## 5. Create the playlist { #create }

Click **Create**.

<!-- figure:existing-56-smart-playlist-result-build-a-smart-playlist -->
<figure markdown>
![A smart playlist showing its matching tracks](../assets/screens/56-smart-playlist-result.webp){ .screen loading=lazy }
<figcaption>The playlist fills itself with the tracks that match.</figcaption>
</figure>
<!-- /figure -->

<!-- figure:playlists-smart-result -->
<figure markdown>
![Melodic house 128+ open with 10 tracks from 17 (Extended Mix) at 128 BPM to Say Less at 130 BPM, and its lightning-bolt icon selected in the sidebar](../assets/screens/playlists-smart-result-light.webp#only-light){ .screen loading=lazy }
![Melodic house 128+ open with 10 tracks from 17 (Extended Mix) at 128 BPM to Say Less at 130 BPM, and its lightning-bolt icon selected in the sidebar](../assets/screens/playlists-smart-result-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>The smart playlist opens sorted by title, marked with a lightning bolt.</figcaption>
</figure>
<!-- /figure -->

**You should see** the playlist open with 10 tracks, and a lightning bolt beside **Melodic house 128+** in the sidebar. The table is sorted by title. A smart playlist has no manual order, so sort by **BPM** or any other column as you like.

## 6. Try a rule change without saving { #change-a-rule }

1. Open **⋯** beside the playlist and choose **Edit**.
2. Change the BPM value from `128` to `130`.
3. Click **Preview matches**. The count drops to *7 matching tracks*, because *17*, *Murda* and *Other Dimension* are 128 BPM.
4. Click **Cancel**.

**You should see** the playlist unchanged. Nothing is saved until you click **Save changes**, and that button, like **Create**, needs a fresh preview first.

## 7. See how it stays current { #it-stays-current }

You do nothing to refresh a smart playlist. A background check runs about once a second. It re-evaluates the list when a track is added or prepared, when a track's tags, key, BPM, energy, cues, custom tags or archive state change, and when you change a playlist or the Library preferences. Rules that depend on the clock, such as *date added in the last 7 days*, are re-evaluated at least once a minute.

If a refresh cannot finish, the playlist keeps its last good result and tells you:

| Sidebar dot | Banner above the table | Meaning |
|---|---|---|
| none | none | Up to date |
| amber | *Evaluating playlist…* or *Updating playlist…* | A refresh is queued or running |
| red | the error text, then *Showing the last successfully evaluated results.* (or *No saved results are available yet.*) | The last refresh failed; the old members stay |

You cannot add or remove tracks by hand in a smart playlist. To freeze today's matches into a playlist you can edit, use a [snapshot](combine-and-tidy-playlists.md#snapshot). For the technical detail, read [How do smart playlists stay current?](../how-it-works/smart-engine.md).

## What you learned

- A smart playlist is a set of conditions joined by **all conditions** or **any condition**.
- **Preview matches** is required before **Create** or **Save changes**.
- A rule reads CueRithm's own measurements (BPM, key, energy) and your tags.
- The list refreshes itself, and keeps its last good result if a refresh fails.

## Next steps

- [Write smart playlist rules](smart-rules.md) for OR groups, dates, key matching and regular expressions.
- [Playlist recipes](recipes/index.md) for ready-made rule sets such as a warm-up and a peak-time playlist.
- [Keep smart playlists up to date automatically](automatic-smart-playlists.md) to have CueRithm create lists by artist, BPM, genre, key or release month.
- [Smart playlist fields and operators](../reference/smart-playlist-fields.md) for the complete catalogue.

<!-- verified: apps/desktop/src/views/PlaylistEditor.tsx (starter condition Title/contains, Preview matches, Create gating, subtitle text); components/SmartRuleEditor.tsx (Match select, n/128 counter, operator reset on field change); smart-rules.ts; App.tsx (activePlaylistStatus banner texts, smart playlists open sorted by title); components/Sidebar.tsx (status dot); crates/cuerithm-library/src/playlists/snapshot.rs (field labels: genre has no friendly label; dynamic rules refresh every 60 s), service.rs (1 s loop); crates/cuerithm-smartlists/src/text.rs (case-insensitive contains), numeric.rs; src/test/smart-playlist-editor.test.tsx, playlist-status.test.tsx; counts computed from _meta/the private demo-library facts sheet; legacy learn/smart-playlist.md re-checked (its Key/contains/123 example was replaced) -->
