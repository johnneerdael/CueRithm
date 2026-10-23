# Collect only the extended mixes

<p class="lead">A smart playlist that keeps the tracks whose title ends in <em>(Extended Mix)</em> and leaves out radio edits, originals and remixes, so a set browser shows the versions you actually play out.</p>

<div class="recipe-meta" markdown>
**Uses:** a smart playlist with the field **Title** and the operator **ends with**, plus a regular-expression variation.  
**Works on:** prepared tracks whose version name is part of the title tag.  
**Time:** About two minutes.  
**Undo:** Remove the playlist from its **⋯** menu, or click **Undo last playlist change** straight after creating it.
</div>

## The problem

A purchase folder mixes versions. Beatport titles carry the mix name in brackets, and you want the long club versions in one place.

| Before | After |
|---|---|
| 26 tracks with several version names | **Extended mixes**: 20 tracks |

The six tracks the rule leaves out:

| Track | Why it is out |
|---|---|
| Brackish (Original Mix) | ends in *(Original Mix)* |
| Come To Life (Original Mix) | ends in *(Original Mix)* |
| Desert (Original Mix) | ends in *(Original Mix)* |
| Play With Fire (Original Mix) | ends in *(Original Mix)* |
| Undercurrent (Original Mix) | ends in *(Original Mix)* |
| Nivk, Rabteu - PLV (Creeds remix - Extended) | ends in *(Creeds remix - Extended)* |

## The solution

1. Click **+** beside **PLAYLISTS**, type `Extended mixes` and set **Playlist type** to **Smart playlist** ([the editor step by step](../build-a-smart-playlist.md#start)).
2. Keep the first condition on the field **Title** and change the operator to **ends with**.
3. Type `(Extended Mix)` in the value box, brackets included.
4. Click **Preview matches**. It should read *20 matching tracks*.
5. Click **Create**.

The exact rule:

```text
Match: all conditions
Title   ends with   (Extended Mix)
```

## How it works

[Text rules](../smart-rules.md#text-rules) ignore capitals and trim spaces from the track's title, so `(extended mix)` works as well. **ends with** compares the end of the whole title, so *Atoma (Extended Mix)* is in and a title ending *(Creeds remix - Extended)* is out. The rule reads the title tag, so it is only as good as your tags. [Clean up messy titles, genres and artists](../../tagging/cleanup-messy-titles.md) keeps version names consistent.

## Variations

- **Everything except extended mixes.** Use **does not end with** and the same value. It lists the six tracks above.
- **Extended or original.** Use **Title** **matches [regular expression](../smart-rules.md#regular-expressions)** and enter the pattern below. It finds 25 tracks.

    ```text
    /\((extended|original) mix\)$/i
    ```

- **Extended mixes in one genre.** Add `genre contains House` to keep the house-family extended mixes.
- **Search more than the title.** The field **Any text field** with **contains** `Extended` checks title, artist, album and other text tags, so it also catches *Nivk, Rabteu - PLV (Creeds remix - Extended)*.

## Related

- [Write smart playlist rules](../smart-rules.md#regular-expressions)
- [Build a smart playlist](../build-a-smart-playlist.md)
- [Clean up messy titles, genres and artists](../../tagging/cleanup-messy-titles.md)
- [Smart playlist fields and operators](../../reference/smart-playlist-fields.md)

<!-- verified: apps/desktop/src/smart-rules.ts (operator labels ends with, does not end with, matches regular expression), components/SmartRuleEditor.tsx (regex placeholder, 100 character limit); crates/cuerithm-smartlists/src/text.rs (lower case, trim, ends_with), crates/cuerithm-regex/src/lib.rs (/pattern/flags); counts computed from _meta/the private demo-library facts sheet -->
