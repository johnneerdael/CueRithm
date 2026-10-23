# Fix capital letters without breaking artist names

<div class="recipe-meta" markdown>
**Uses:** Cleanup, Smart Fixes, **Casing** with **Ignored values**.  
**Works on:** Title, artist or any other text field of Ready tracks.  
**Time:** About 10 minutes.  
**Undo:** **Undo history** in Cleanup restores the batch: [Undo a cleanup](../undo-cleanup.md).
</div>

## The problem

Some sources send titles in capitals or in all lower case. **Casing** turns those into capitalised words, but it cannot tell shouting from a stylised name. Run on the artist field, it also rewrites artists whose real spelling is all capitals. The demo library has two: `SOLANCE` and `SLVR`.

| Field | Before | Casing alone | Casing with Ignored values |
|---|---|---|---|
| title | `SIGNAL AND SOUND` | `Signal And Sound` | `Signal And Sound` |
| title | `signal and sound (extended mix)` | `Signal And Sound (Extended Mix)` | same |
| title | `WITH ME (Extended Mix)` | unchanged: mixed case | unchanged |
| artist | `SOLANCE` | `Solance` | `SOLANCE` |
| artist | `SLVR` | `Slvr` | `SLVR` |
| artist | `Adriatique` | unchanged | unchanged |

The title examples `SIGNAL AND SOUND` and `signal and sound (extended mix)` are placeholders; the demo library has no title in capitals.

## The solution

Use two **Casing** cards, so that the artist card can carry the exceptions.

1. In **Cleanup**, on **Smart Fixes**, add **Casing**. Leave **Field** on `title`, and keep **Uppercase values** and **Lowercase values** ticked.
2. Add a second **Casing**. Set **Field** to `artist`. In **Ignored values, one per line**, enter `SOLANCE` and `SLVR`, one per line.
3. Press **Save cleanup rules**, set the **Scope**, and press **Scan tracks**.
4. Read every proposal. Look for names that lost their capitals, and add them to the second card's ignored list.
5. Press **Apply N changes**.

```text
Smart Fix: Casing
  Field: title
  Uppercase values: on
  Lowercase values: on

Smart Fix: Casing
  Field: artist
  Uppercase values: on
  Lowercase values: on
  Ignored values, one per line:
    SOLANCE
    SLVR
```

## How it works

**Casing** looks at each value as a whole. It rewrites a value only if every letter is a capital (**Uppercase values**) or every letter is lower case (**Lowercase values**). A value in mixed case, like `WITH ME (Extended Mix)` or `Adriatique`, is never touched. An artist field with several values is checked value by value.

When it does rewrite, every word starts with a capital, including `and` and `the`. `dj`, `mc` and `va` become `DJ`, `MC` and `VA`, `vs` stays lower case, and a word after a dash gets its own capital. So `SIGNAL VS SOUND` becomes `Signal vs Sound` and `DJ KOZE` becomes `DJ Koze`.

A value on a rule's **Ignored values** list is skipped by that rule. So is a value on the **Protected** list of the Genres or Artists tab, and a value that a mapping uses as its source or target. That is why a genre mapping beats a Casing rule on the genre field.

Some names do not survive title-casing even though they look ordinary: `AC/DC` becomes `Ac/Dc` and `K.ONE` becomes `k.one`. Put them on the ignored list too.

## Variations

- **Only fix lower-case imports.** Untick **Uppercase values**; `SIGNAL AND SOUND` is then left alone and `signal and sound` becomes `Signal And Sound`.
- **Fix capitals in other fields.** Choose `album`, `label`, `genre` or any other field in **Field**. [Write cleanup rules](../cleanup-rules.md#fix-casing) lists the choices.
- **Protect a name for every rule.** Add it to **Protected artists, one per line** on the **Artists** tab instead of to each rule's ignored list; that also stops artist mappings from changing it.

## Related

- [Write cleanup rules](../cleanup-rules.md#fix-casing)
- [Rename an artist everywhere](rename-an-artist-everywhere.md)
- [Clean up messy titles, genres and artists](../cleanup-messy-titles.md)
- [Tag and clean-up recipes](index.md)

<!-- verified: crates/cuerithm-cleanup/src/fixes/text.rs (casing, word); crates/cuerithm-cleanup/src/fixes/mod.rs (ignored values, protects_text); crates/cuerithm-cleanup/src/model.rs; every table row run through the engine; apps/desktop/src/views/CleanupSettings.tsx (labels). -->
