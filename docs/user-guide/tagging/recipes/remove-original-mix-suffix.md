# Remove "(Original Mix)" from titles

<div class="recipe-meta" markdown>
**Uses:** Cleanup, Smart Fixes, **Common title text** with the **Original Mix suffix** pattern.  
**Works on:** The title of Ready tracks.  
**Time:** About 5 minutes.  
**Undo:** **Undo history** in Cleanup restores the whole batch: [Undo a cleanup](../undo-cleanup.md).
</div>

## The problem

Beatport writes the mix into the title, so every track that has no remix still ends in `(Original Mix)`. In a table, in rekordbox and in file names that suffix is noise, but you still want `(Extended Mix)` and remix names, because they tell versions apart.

These are the five `(Original Mix)` tracks in the demo library, and two that must stay as they are:

| Before | After |
|---|---|
| `Brackish (Original Mix)` | `Brackish` |
| `Come To Life (Original Mix)` | `Come To Life` |
| `Desert (Original Mix)` | `Desert` |
| `Play With Fire (Original Mix)` | `Play With Fire` |
| `Undercurrent (Original Mix)` | `Undercurrent` |
| `Atoma (Extended Mix)` | unchanged |
| `17 (Extended Mix)` | unchanged |

## The solution

1. Open **Cleanup** in the sidebar. On the **Smart Fixes** tab, choose **Common title text** in **New Smart Fix** and press **Add Smart Fix**.
2. Leave **Pattern** on **Original Mix suffix**. Press **Save cleanup rules**.
3. Set **Scope** to the tracks you want, or to **All ready tracks**, and press **Scan tracks**.
4. Read the cards. Each should show only `title`, with `(Original Mix)` gone. Untick anything unexpected.
5. Press **Apply N changes**.

The rule, in full:

```text
Smart Fix: Common title text
Enabled: on
Pattern: Original Mix suffix
Ignored values: (none)
```

## How it works

The pattern removes `Original Mix` or the bare word `Original` from the very end of a title, with or without brackets or a leading dash, in any capitalisation. It then trims the space that was in front of it.

| Title | Result |
|---|---|
| `Signal (Original Mix)` | `Signal` |
| `Signal - Original Mix` | `Signal` |
| `Signal (original mix)` | `Signal` |
| `Signal original` | `Signal` |
| `Signal (Original Remix)` | unchanged |
| `Signal (Original Mix) (Extended Mix)` | unchanged, because the suffix is not at the end |
| `Original Mix` | unchanged, because nothing would be left |

Nothing else about the track changes: the other tags, the audio, the key, BPM, energy and cues are untouched.

## Variations

- **Remove it from every new track.** Do the steps above once, then follow [Run cleanup on every new track](../cleanup-automatically.md).
- **Remove other suffixes.** No pattern removes `(Extended Mix)`; **Common title text** offers only the four patterns in [Write cleanup rules](../cleanup-rules.md#fix-common-title-text). Leaving `(Extended Mix)` in place is usually what you want.
- **Keep one title as it is.** Type the full title on a line of **Ignored values, one per line**, for example `Original Mix`, to keep a real song title that ends the same way.
- **Tidy stray spaces too.** Add **Garbage** below it: [Write cleanup rules](../cleanup-rules.md#fix-garbage).

## Related

- [Clean up messy titles, genres and artists](../cleanup-messy-titles.md): the same rule as a guided walk-through
- [Write cleanup rules](../cleanup-rules.md#fix-common-title-text)
- [Strip the key and BPM from titles](strip-key-and-bpm-from-titles.md)
- [Tag and clean-up recipes](index.md)

<!-- verified: crates/cuerithm-cleanup/src/fixes/remove.rs (common, OriginalMixFromTitle); examples run through the engine with the demo titles; crates/cuerithm-cleanup/tests/fixtures/lexicon-v1.json remove_common_text cases; apps/desktop/src/views/CleanupSettings.tsx (labels: Common title text, Pattern, Original Mix suffix) -->
