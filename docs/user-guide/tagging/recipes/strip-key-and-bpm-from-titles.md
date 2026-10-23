# Strip the key and BPM from titles

<div class="recipe-meta" markdown>
**Uses:** Cleanup, Smart Fixes, **Common title text** with **Camelot key**, **Camelot key and energy** or **BPM suffix**.  
**Works on:** The title of Ready tracks.  
**Time:** About 5 minutes.  
**Undo:** **Undo history** in Cleanup restores the batch: [Undo a cleanup](../undo-cleanup.md).
</div>

## The problem

Another program, or a habit of your own, left the key and tempo inside the title: `8A - 7 - Signal 128`. CueRithm measures key, BPM and energy itself and shows them in their own columns, so the copies in the title only get in the way, and they go stale if you [analyse the track again](../../analysis/analyze-again.md). The demo library has no such titles, so the table uses the placeholder title `Signal`.

| Title before | Pattern | Title after |
|---|---|---|
| `8A - Signal` | **Camelot key** | `Signal` |
| `12B Signal` | **Camelot key** | `Signal` |
| `Signal - 9A` | **Camelot key** | `Signal` |
| `8A - 7 - Signal` | **Camelot key and energy** | `Signal` |
| `12B - 128 - Signal` | **Camelot key and energy** | `Signal` |
| `Signal 128` | **BPM suffix** | `Signal` |
| `8A - 7 - Signal 128` | **Camelot key and energy**, then **BPM suffix** | `Signal` |

## The solution

1. In **Cleanup**, on **Smart Fixes**, add **Common title text**.
2. Set **Pattern** to the one that matches your titles:
    - **Camelot key** for a key at the front (`8A - Title`, `8A Title`) or at the end (`Title - 8A`).
    - **Camelot key and energy** for `8A - 7 - Title`, a key and a number of one to three digits at the front.
    - **BPM suffix** for a number at the end (`Title 128`).
3. If your titles mix styles, add one more **Common title text** card per pattern. Put **Camelot key and energy** above **BPM suffix**.
4. Press **Save cleanup rules**, then **Scan tracks**, and read the cards.
5. Press **Apply N changes**.

```text
Smart Fix: Common title text
  Pattern: Camelot key and energy

Smart Fix: Common title text
  Pattern: BPM suffix
```

## How it works

The patterns look only at the start or end of the title and never at the middle. A key is `1A` to `12B`; `13A` and a lower-case `signal 8A` are not removed. A title that starts with a number that is not a key, such as `17 (Extended Mix)`, is left alone.

Use **Camelot key and energy** rather than **Camelot key** for `8A - 7 - Signal`: the plain key pattern would leave `7 - Signal` behind.

!!! warning "Gotcha"
    **BPM suffix** removes any trailing number of two or more digits that does not start with 0, so `Studio 54` becomes `Studio`. Type `Studio 54` on a line of that card's **Ignored values, one per line** to keep it. The scan shows every proposal, so look for real titles that end in a number.

## Variations

- **Keep the key, but move it.** Put the key in the file name instead: [Put key and energy in file names](../../rename/recipes/put-key-and-energy-in-file-names.md). It uses CueRithm's measured key, not the text that was in the title.
- **Remove a leading track number.** Add **Number prefix**, which turns `01. Signal` and `02 - Signal` into `Signal` and leaves `17 (Extended Mix)` as it is.
- **Do it on arrival.** [Run cleanup on every new track](../cleanup-automatically.md).

## Related

- [Write cleanup rules](../cleanup-rules.md#fix-common-title-text)
- [Read a track's key, BPM and energy](../../analysis/read-key-bpm-energy.md)
- [Understand Camelot keys](../../analysis/camelot-keys.md)
- [Tag and clean-up recipes](index.md)

<!-- verified: crates/cuerithm-cleanup/src/fixes/remove.rs (common: CamelotKeyFromTitle, CamelotKeyEnergyFromTitle, BpmFromTitle, number_prefix); crates/cuerithm-cleanup/tests/fixtures/lexicon-v1.json; every table row and the Studio 54 case run through the engine -->
