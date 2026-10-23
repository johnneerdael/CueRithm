# Keep a playlist of the tracks you added this week

<p class="lead">A smart playlist called <strong>Added this week</strong> that shows what is new and drops each track when it is a week old. You never touch it after creating it.</p>

<div class="recipe-meta" markdown>
**Uses:** a smart playlist with the field **Date added** and the operator **is in the last**.  
**Works on:** every track in **Tracks**.  
**Time:** About two minutes.  
**Undo:** Remove the playlist from its **⋯** menu, or click **Undo last playlist change** straight after creating it.
</div>

## The problem

You buy a folder of new music every Friday and want a short list of the week's arrivals to play through before you decide what goes into a set. Sorting **Tracks** by date works, but the list never shrinks on its own.

| When | What **Added this week** holds |
|---|---|
| The day the 26-file Beatport purchase folder is prepared | all 26 tracks |
| Six days later | the same 26 tracks |
| Eight days later | none of them, and whatever you added since |

**Date added** is the day a track entered your CueRithm library, not the release date in its tags.

## The solution

1. Click **+** beside **PLAYLISTS**, type `Added this week` and set **Playlist type** to **Smart playlist**.
2. Set the first condition to the field **Date added**, the operator **is in the last**, the number `7` and the unit **days**.
3. Click **Preview matches**, then **Create** within about a minute.
4. Optional: show the **Date added** column to see when each track arrived; see [Show the columns you need](../../analysis/choose-columns.md).

The exact rule:

```text
Match: all conditions
Date added   is in the last   7 days
```

## How it works

**is in the last** counts back from the current moment, in your library time zone, so a track added at 18:00 on a Friday stays in until about 18:00 the next Friday. CueRithm re-checks any rule that depends on the clock at least once a minute, so tracks leave the list without you opening it.

For the same reason a preview of this rule expires after about a minute. If you wait too long, CueRithm says *Preview expired. Preview matches again before saving.* Click **Preview matches** and then **Create** again.

!!! note
    This is a smart playlist you wrote yourself. It is not one of the lists under **Automatic Smart Playlists**, which are created by the options in **Settings**, **Smart Playlists**. See [Keep smart playlists up to date automatically](../automatic-smart-playlists.md).

## Variations

- **Two weeks.** Choose `14` and **days**, or `2` and **weeks**. The units are **days**, **weeks** and **months**.
- **New and strong.** Add `Energy is at least 7` to see only this week's high-energy tracks.
- **Older than a month.** Use **is not in the last** `30` **days** to see what has been in the library a while, for example together with [a rule for tracks not in a set yet](tracks-not-in-a-set-yet.md).
- **One purchase day.** Use **Date added** **is on** and pick the day from the calendar.
- **Wrong day near midnight.** Set the right zone in [Library preferences](../../reference/settings/library.md).

## Related

- [Write smart playlist rules](../smart-rules.md#date-rules)
- [Find tracks that are not in a set yet](tracks-not-in-a-set-yet.md)
- [Follow a track through Incoming](../../files/follow-incoming.md)
- [Build a smart playlist](../build-a-smart-playlist.md)

<!-- verified: apps/desktop/src/components/SmartRuleEditor.tsx (relative amount and unit menu), smart-rules.ts (DateRecent label); crates/cuerithm-smartlists/src/dates.rs (cutoff counted back from now in library zone; missing dates), lib.rs; crates/cuerithm-library/src/playlists/snapshot.rs (dynamic rules re-evaluated every 60 s; Date added is the track's created time), evaluate.rs (preview expiry); src/test/smart-playlist-editor.test.tsx -->
