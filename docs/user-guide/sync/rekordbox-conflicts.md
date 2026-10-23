# Resolve rekordbox sync conflicts

<p class="lead">When a track is already in rekordbox, CueRithm will not overwrite it until you say whose value wins. This page shows how to read the review, pick a choice for each field or for many at once, and finish the sync. The first review takes about ten minutes; later ones are short.</p>

**Before you start:** you have chosen **rekordbox 6 / 7** in **Sync to**, rekordbox and rekordbox Agent are closed, and you have chosen **Preview rekordbox sync** ([Sync to rekordbox 6 or 7](rekordbox-direct.md)). The review appears only for tracks that rekordbox already has. Tracks it does not have need no choices ([Add only new tracks to rekordbox](rekordbox-new-tracks.md)).

## Read the review { #read-the-review }

Under the preview summary, the **Review fields** section counts the work: for example `12 fields need a choice · 0 skipped`. It is a table with four columns.

| Column | Shows |
|---|---|
| **Track / field** | The track title, the field name, and a status line: the reason a choice is needed, **Change proposed**, **Choice selected**, **Unchanged** or **Skipped · not acknowledged** |
| **CueRithm** | The value CueRithm would send |
| **rekordbox** | The value rekordbox holds now |
| **Choice** | A menu that starts at **Choose a value** |

Values are written out so you can compare them. **Empty** means the field exists and has no value; **Unavailable** means that side could not be read. Cues are listed with their slot, name and time (for 17 (Extended Mix): `Start · 0:00.000`, `Drop · 1:15.000`, `Breakdown · 2:15.000`, `Second Drop · 3:00.000`, `Last beat · 4:30.000`, each line starting with its slot such as **Hot A**). A beat grid shows its BPM range and anchors. Artwork shows **Artwork available**. The table shows 100 rows at a time; **Show N more fields** reveals the rest.

For example, a first sync of 17 (Extended Mix) when rekordbox already has the track with a different genre and no cues. The rekordbox values here are examples; the CueRithm values are the real ones:

| Track / field | CueRithm | rekordbox | Choice you would make |
|---|---|---|---|
| 17 (Extended Mix) · genre · *First sync: choose explicitly* | Melodic House & Techno | Techno (an example value) | **Use CueRithm** |
| 17 (Extended Mix) · title · *First sync: choose explicitly* | 17 (Extended Mix) | 17 (Extended Mix) | **Accept equal values** |
| 17 (Extended Mix) · cues · *First sync: choose explicitly* | The five cues above | **No cues** | **Replace listed rekordbox cues** |

## What each choice does { #choices }

The menu offers only choices that are safe for that field.

| Choice | Offered when | Effect |
|---|---|---|
| **Use CueRithm** | CueRithm has a value | Writes CueRithm's value to rekordbox |
| **Keep rekordbox** | rekordbox has a value | Keeps rekordbox's value and records the field as reviewed. It never rewrites your music tags |
| **Accept equal values** | Both sides already hold the same value | Records the field as in step; nothing is written |
| **Clear in rekordbox** | The field is one of the 18 metadata fields or artwork, and rekordbox has a value | Removes the value in rekordbox. This is the only way to clear a field |
| **Replace listed rekordbox cues** | The field is cues, and CueRithm has cues | Replaces the cues shown in the review with CueRithm's |
| **Clear listed rekordbox cues** | The field is cues | Removes the cues shown in the review |
| **Skip this field** | Always | Leaves the field alone and unreviewed; it comes back in the next preview |

An **Empty** or **Unavailable** value never means "clear". CueRithm sends a clear only when you choose **Clear in rekordbox** or **Clear listed rekordbox cues**.

## Why a field needs a choice { #why-a-field-needs-a-choice }

| Status line | Meaning |
|---|---|
| **First sync: choose explicitly** | CueRithm has never compared this field for this track. Even equal values need **Accept equal values**, so the first sync is always your decision |
| **Changed in both libraries** | Both sides differ from the last value they agreed on |
| **Choose which library to use** | One side changed since the last agreed value and no side is recorded as the owner |
| **Changed outside its owner** | The side that does not normally own the field changed, or rekordbox holds cues CueRithm did not create |
| **Value unavailable** | One side could not be read. You can **Skip this field**, or pick a value if the other side has one |
| **Change proposed** | CueRithm has a safe suggestion and has already selected it in the menu. Change it if you disagree |

Neither the newer timestamp nor a matching name decides a winner.

## Decide many at once { #bulk-choices }

The buttons above the table apply one choice to every field that still needs one and that the choice fits. Choices you already made stay as they are.

| Button | Applies to |
|---|---|
| **Accept equal values** | Fields where both sides are identical |
| **Use CueRithm for unresolved** | Fields where CueRithm has a value. Shows the number of cue sets it will replace, for example `· 3 cue sets` |
| **Keep rekordbox for unresolved** | Fields where rekordbox has a value |
| **Skip unresolved fields** | Every field that still needs a choice |
| **Reset choices** | Clears every choice you made, so you can start the review again |

A sensible order for a first sync: **Accept equal values**, then scan the remaining rows, then **Use CueRithm for unresolved** (or **Keep rekordbox for unresolved**) for the rest.

You do not have to resolve every field. A field with no choice is counted as skipped, nothing is written for it, and the sync still applies the rest.

## Choices make a new preview necessary { #preview-again }

Each choice discards the prepared preview, and the summary tells you: "Preview again before syncing. Your field choices are retained." Choose **Preview rekordbox sync** again. CueRithm sends your choices with the request, and **Sync rekordbox** becomes available when the new preview is ready. Changing the library, the destination, the scope or an option clears the choices, because the review no longer describes what would happen.

## Decide what happens to cues { #cues }

CueRithm tracks which rekordbox cues it wrote. A cue you or another program created in rekordbox is not CueRithm's, and CueRithm will not take over its slot silently.

- **Replace listed rekordbox cues** replaces the recognised cues shown in the review and, once the sync commits, transfers their management to CueRithm. Cue data CueRithm does not recognise stays protected.
- Accepting equal cues alone does not transfer their management.
- Choosing a value for a different field never turns a suggested cue replacement into a hand-over; you must choose it for the cue row.
- rekordbox has eight hot cue slots. Memory cues are not limited by slot.

## Decide what happens to a beatgrid { #beatgrid }

**Use CueRithm** writes a constant BPM and a first-beat anchor ([Check the beatgrid at beat level](../analysis/check-the-beatgrid.md)). **Keep rekordbox** retains a grid that has tempo changes. A complete grid cannot be cleared, so **Clear in rekordbox** is not offered for it.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| Every row says **First sync: choose explicitly** | This is the first sync of tracks rekordbox already had | Use **Accept equal values**, then the bulk choice you prefer |
| No **Use CueRithm** option on a row | CueRithm's value is empty | Fill it in CueRithm ([Edit tags and artwork](../tagging/edit-tags-by-hand.md)) or choose **Keep rekordbox** or **Clear in rekordbox** |
| **Sync rekordbox** is disabled after a choice | The preview was discarded | Preview again |
| "Both field observations are required for acknowledgement; skip unavailable fields" | **Keep rekordbox** on a field CueRithm could not read | Choose **Skip this field** |
| "Native audio location differs; explicitly use the CueRithm source location before exporting timing" | rekordbox points at a different file location for this track and you chose to replace its cues or grid | Choose **Use CueRithm** for the source path row, or skip the cues and grid |

## Related

- [Sync to rekordbox 6 or 7](rekordbox-direct.md)
- [Add only new tracks to rekordbox](rekordbox-new-tracks.md)
- [Set the sync options](sync-options.md#metadata-fields)
- [Preview, apply and restore a sync](preview-apply-restore.md)

<!-- verified: apps/desktop/src/components/RekordboxConflicts.tsx (labels, reasons, bulk buttons, value text); views/RekordboxSync.tsx (choose, invalidate, Reset choices, signature clears choices); src/test/rekordbox-sync.test.tsx; crates/cuerithm-library/src/native_reconcile.rs (reconcile_field); rekordbox_sync_service/planning.rs (field_choice messages, cue ownership conflict, source path message); assets.rs (eight hot cue slots) -->
<!-- UNVERIFIED: the exact wording of "owner" cases (Changed outside its owner, Choose which library to use) is derived from reconcile_field and the baseline defaults, not exercised in the app; the Hot A to Hot H slot letters for the demo tracks -->
