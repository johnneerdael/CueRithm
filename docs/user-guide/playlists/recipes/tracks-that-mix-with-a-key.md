# Collect the tracks that mix with a key

<p class="lead">A smart playlist called <strong>Mixes with 7A</strong> that always holds the tracks in or next to one key, so when you land on a track you know your safe next picks.</p>

<div class="recipe-meta" markdown>
**Uses:** a smart playlist with the field **Key** and the operator **is compatible with key**.  
**Works on:** prepared tracks that have a measured key.  
**Time:** About two minutes.  
**Undo:** Remove the playlist from its **⋯** menu, or click **Undo last playlist change** straight after creating it.
</div>

## The problem

You are playing in `7A` and want a short list of tracks whose key blends with it. **Key is** `7A` finds only the tracks in exactly that key, which is too few. In the Beatport purchase folder used in this guide it finds two.

| Rule | Tracks found |
|---|---|
| **Key** **is** `7A` | 2 |
| **Key** **is compatible with key** `7A` | 6 |

The six tracks in **Mixes with 7A**:

| Track | Key | BPM | Energy |
|---|---|---|---|
| 17 (Extended Mix) | 6A | 128 | 7/10 |
| Come To Life (Original Mix) | 7A | 130 | 7/10 |
| Dancin (Extended Mix) | 6A | 131 | 6/10 |
| Other Dimension (Extended Mix) | 6A | 128 | 7/10 |
| Say Less (Extended Mix) | 8A | 130 | 6/10 |
| When The Fire Starts To Burn (Extended Mix) | 7A | 126 | 6/10 |

## The solution

1. Click **+** beside **PLAYLISTS**, type `Mixes with 7A` and set **Playlist type** to **Smart playlist**.
2. Set the first condition to the field **Key**, the operator **is compatible with key**, and choose `7A` from the key menu.
3. Click **Preview matches**. It should read *6 matching tracks*.
4. Click **Create**.

The exact rule:

```text
Match: all conditions
Key   is compatible with key   7A
```

## How it works

The rule uses your **Key compatibility** setting in Library preferences, which starts as **Harmonic**. In that mode `7A` accepts:

| Key | Why |
|---|---|
| `7A` | the same key |
| `7B` | the relative key, the same number on the other letter |
| `6A` and `8A` | the neighbouring numbers on the same letter |

The purchase folder has no `7B` track, so the list shows `6A`, `7A` and `8A` tracks. The wheel wraps round, so `12A` accepts `1A` and `11A`. See [Understand Camelot keys](../../analysis/camelot-keys.md).

## Variations

- **Fuzzy mode.** Switch **Key compatibility** to **Fuzzy** in [Library preferences](../../reference/settings/library.md) and the rule also accepts `6B` and `8B`. This changes every key rule and every automatic key list, and CueRithm re-evaluates them. The purchase folder has no `6B` or `8B` track, so this list does not change.
- **Add a tempo window.** Add `BPM is between 128 to 130` and the list shrinks to four tracks, dropping *Dancin* (131 BPM) and *When The Fire Starts To Burn* (126 BPM).
- **Another key.** Duplicate the playlist from its **⋯** menu, choose **Edit**, change the key and rename it.
- **Every key at once.** Tick **By Key** under **Settings**, **Smart Playlists** and CueRithm makes all 24 lists. See [Keep smart playlists up to date automatically](../automatic-smart-playlists.md).
- **From a track you have chosen.** [Find tracks that mix with the one playing](../find-compatible-tracks.md) adds BPM and energy to the search and ranks the answers.

## Related

- [Find tracks that mix with the one playing](../find-compatible-tracks.md)
- [Write smart playlist rules](../smart-rules.md#key-rules)
- [Check the flow of a playlist](../check-playlist-flow.md)
- [Understand Camelot keys](../../analysis/camelot-keys.md)

<!-- verified: apps/desktop/src/smart-rules.ts (KeySimilar label, allowedOperators only for field key), components/SmartRuleEditor.tsx (key select 1A-12B); crates/cuerithm-smartlists/src/keys.rs (harmonic and fuzzy distance rules, wrap), lib.rs (Family::Key), text.rs; crates/cuerithm-library/src/playlists/preferences.rs (mode change marks lists stale), automatic.rs (By Key); counts computed from _meta/library-facts.md -->
