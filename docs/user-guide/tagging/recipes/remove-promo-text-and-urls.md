# Remove promo text and web addresses

<div class="recipe-meta" markdown>
**Uses:** Cleanup, Smart Fixes, **Promotional text** and **URLs and email**.  
**Works on:** The title, and optionally the comment, of Ready tracks.  
**Time:** About 10 minutes, mostly reading the preview.  
**Undo:** **Undo history** in Cleanup restores the batch: [Undo a cleanup](../undo-cleanup.md).
</div>

## The problem

Tracks from record pools and download sites carry advertising in their tags: `[FREE DOWNLOAD]`, `PREMIERE -`, a shop's web address, or a comment that says where the file came from. The demo library is clean, and both rules leave titles such as `Brackish (Original Mix)` and `Atoma (Extended Mix)` alone, so the examples below are placeholders.

| Field | Before | After |
|---|---|---|
| title | `Signal [FREE DOWNLOAD]` | `Signal` |
| title | `PREMIERE - Signal` | `Signal` |
| title | `Signal purchased from Store` | `Signal` |
| title | `Signal (Free Download) (Extended Mix)` | `Signal (Extended Mix)` |
| title | `Signal https://example.com` | `Signal` |
| title | `Signal beatport.com` | `Signal` |
| comment | `Downloaded from https://example.com/store` | (empty) |

## The solution

1. In **Cleanup**, on **Smart Fixes**, add **Promotional text**. Leave **Field** on `title`.
2. Add **URLs and email**. Leave **Field** on `title` and **Clear entire matching field** off.
3. For the comment, add **URLs and email** again, set **Field** to `comment` and tick **Clear entire matching field**.
4. Press **Save cleanup rules**, set the **Scope** and press **Scan tracks**.
5. Read every card. A proposal that shows **Empty** has lost its whole value. Untick anything that is a real title.
6. Press **Apply N changes**.

```text
Smart Fix: Promotional text
  Field: title
Smart Fix: URLs and email
  Field: title
  Clear entire matching field: off
Smart Fix: URLs and email
  Field: comment
  Clear entire matching field: on
```

## How it works

**Promotional text** removes bracketed promotions, a `PREMIERE` prefix, and tails that start with phrases such as `purchased from`, `available on`, `follow us` or `© 2024`, or with a web address. **URLs and email** removes `https://` and `www.` addresses, email addresses and bare domains such as `beatport.com`, but keeps names that end in an audio extension, like `Signal.flac`. With **Clear entire matching field** on, a value that contains an address is removed completely, so a comment like `Great opener www.example.com` would also be cleared.

!!! warning "Gotcha"
    The promotional word list is broad. A bracket that starts with one of its words is removed even when it is a real mix name: `Signal (Free Spirit Remix)`, `Signal (Exclusive Mix)` and `Signal (Promo Mix)` all become `Signal`, and `Signal Demo` loses `Demo`. Titles such as `Signal - Support the artist` and `Signal free spirit` are left alone.

To protect a real title, type it exactly on a line of that card's **Ignored values, one per line**. With `Signal (Free Spirit Remix)` listed there, that title is skipped and `Signal [FREE DOWNLOAD]` is still cleaned. A fix that strips a whole value, like `Demo Tape` or `www.example.com`, leaves the field empty, which is why the preview matters.

## Variations

- **Only addresses.** Leave out **Promotional text** and keep **URLs and email**.
- **Other fields.** Both fixes have a **Field** menu: label, album, grouping and the rest are available. [Write cleanup rules](../cleanup-rules.md#fix-promotional-text) lists them.
- **On arrival.** Pool downloads are exactly what [Run cleanup on every new track](../cleanup-automatically.md) is for; run the rules by hand on a few files first.

## Related

- [Write cleanup rules](../cleanup-rules.md#fix-urls-and-email)
- [Remove "(Original Mix)" from titles](remove-original-mix-suffix.md)
- [Undo a cleanup](../undo-cleanup.md)
- [Tag and clean-up recipes](index.md)

<!-- verified: crates/cuerithm-cleanup/src/fixes/remove.rs (promotional, urls); crates/cuerithm-cleanup/src/fixes/mod.rs (emptied values are removed); crates/cuerithm-cleanup/tests/fixtures/lexicon-v1.json; every table row, the false positives and the ignored-value case run through the engine -->
