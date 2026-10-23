# Understand Camelot keys

<p class="lead">Why does CueRithm show 7A instead of D minor, and which keys can you mix with it? This page explains the code, the wheel and the rule CueRithm uses.</p>

## The short answer

CueRithm names every track's key as a Camelot code: a number from 1 to 12 and a
letter. The number is the key's place on a wheel of twelve. The letter **A** means
a minor key and **B** a major key. CueRithm shows no other notation in the track
table or the player.

| In CueRithm | Means |
|---|---|
| <span class="cr-key">7A</span> | D minor |
| <span class="cr-key">8A</span> | A minor |
| <span class="cr-key">8B</span> | C major |
| <span class="cr-key">12B</span> | E major |

## How the wheel is laid out { #the-wheel }

Picture two rings of twelve keys. The outer ring (**B**) holds the major keys, the
inner ring (**A**) the minor keys. Moving one number clockwise moves up a fifth, so
neighbouring numbers share six of their seven notes. A major key and the minor key
with the same number, called relative keys, share all seven.

| Number | A (minor) | B (major) |
|---|---|---|
| 1 | Ab minor | B major |
| 2 | Eb minor | F# major |
| 3 | Bb minor | C# major |
| 4 | F minor | Ab major |
| 5 | C minor | Eb major |
| 6 | G minor | Bb major |
| 7 | D minor | F major |
| 8 | A minor | C major |
| 9 | E minor | G major |
| 10 | B minor | D major |
| 11 | F# minor | A major |
| 12 | C# minor | E major |

The names are spelled the way CueRithm spells them: flats for Eb, Ab and Bb, sharps
for C# and F#. The wheel wraps round, so 12 sits next to 1.

In the demo library, *Come To Life (Original Mix)* is <span class="cr-key">7A</span>
(D minor), *17 (Extended Mix)* is <span class="cr-key">6A</span> (G minor) and
*Brackish (Original Mix)* is <span class="cr-key">12B</span> (E major).

## Which keys mix { #which-keys-mix }

CueRithm's harmonic rule accepts three kinds of neighbour:

1. **The same key.** 7A with 7A.
2. **One number up or down, same letter.** 7A with 6A or 8A. The wheel wraps, so 12A goes with 11A and 1A.
3. **The same number, other letter.** 7A with 7B, the relative major.

| Playing | Mixes with (harmonic) |
|---|---|
| <span class="cr-key">7A</span> | 7A, 6A, 8A, 7B |
| <span class="cr-key">8A</span> | 8A, 7A, 9A, 8B |
| <span class="cr-key">12B</span> | 12B, 11B, 1B, 12A |

This is the rule behind the demo smart playlist *Mixes with 7A*, which holds the
keys 6A, 7A, 8A and 7B. In the demo library it finds *Come To Life*, *When The Fire
Starts To Burn*, *17*, *Dancin*, *Other Dimension* and *Say Less*.

CueRithm also offers a looser rule called **Fuzzy**. It adds one number up or down
with either letter, so 7A also goes with 6B and 8B. Harmonic is the stricter of
the two. Both are a guide for preparing and practising; compatible keys can still
clash on a particular pair, so listen before you commit.

## Where CueRithm uses the rule { #where-it-is-used }

- **Compatible tracks** lists tracks that fit the one you pick, with a **Key mode** of **Harmonic** or **Fuzzy**. See [Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md).
- **Smart playlists** have a rule, *is compatible with key*. See [Write smart playlist rules](../playlists/smart-rules.md) and [Smart playlist fields and operators](../reference/smart-playlist-fields.md).
- **Playlist flow** shows how neighbouring tracks in a playlist relate. See [Check the flow of a playlist](../playlists/check-playlist-flow.md).

Smart playlist rules also understand Open Key values and note names such as
*D minor*, but CueRithm always displays the Camelot code.

## Keys in your files and your DJ software { #keys-elsewhere }

- **In file tags.** CueRithm can write the measured key into a file's tags as a Camelot code or as a note name such as *D minor*. The setting is **Write Camelot keys** under automatic tagging. See [Choose providers and fields to write](../tagging/choose-providers.md).
- **In DJ software.** When you sync to Serato DJ Pro or to a Pioneer USB stick, the **Convert keys** option can write Original keys, Musical notation, Camelot or Open Key. See [Set the sync options](../sync/sync-options.md).
- **In the table.** The **Embedded key** column shows the tag stored in a file, which can differ from **KEY**. See [Show the columns you need](choose-columns.md).

## What this means in CueRithm

- A track has one key. CueRithm does not follow key changes within a track.
- The key is a measurement from the audio, not a tag. If another program names the key differently, see [My key or BPM differs from another app](../help/troubleshooting/key-or-bpm-differs.md).
- To read key, BPM and energy together, follow [Read a track's key, BPM and energy](read-key-bpm-energy.md). For how the key is detected, see [How is the key detected?](../how-it-works/key.md).

## Related

- [Read a track's key, BPM and energy](read-key-bpm-energy.md)
- [Find tracks that mix with the one playing](../playlists/find-compatible-tracks.md)
- [Build a smart playlist](../playlists/build-a-smart-playlist.md)
- [How is the key detected?](../how-it-works/key.md)

<!-- verified: crates/camelot-core/src/key.rs (MAJOR/MINOR tables, name(), compatible_with); crates/cuerithm-smartlists/src/keys.rs (Harmonic and Fuzzy modes, Open Key parsing); apps/desktop/src/views/CompatibleTracks.tsx (Key mode Harmonic/Fuzzy); apps/desktop/src/smart-rules.ts (is compatible with key); apps/desktop/src/views/TaggingSettings.tsx (Write Camelot keys); apps/desktop/src/views/Sync.tsx (Convert keys, Serato DJ Pro and Pioneer USB only); crates/cuerithm-autotag/src/write.rs (camelot vs standard key); apps/desktop/src/display.ts; legacy technical/key.md and ia.md demo playlists re-checked -->
