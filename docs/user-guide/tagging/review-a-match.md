# Review and fix a wrong match

<p class="lead">Use Review match when a track is waiting in Incoming for a decision, or when the catalog gave a finished track the wrong mix, label or artwork. You compare the candidates, choose one and decide field by field what changes. Plan five minutes per track.</p>

**Before you start**, tagging should be on with at least one provider. See [Tag new tracks from Beatport and Traxsource](tag-new-tracks-automatically.md). Review match works on one track at a time. Nothing is written until you press **Use selected match**.

## Open the review { #open-review }

Any of these opens the **Review match** panel for a track:

- Select the warning triangle in the narrow status column of a row that needs attention.
- Select exactly one track, then choose **Review match** in the toolbar above the table. With several tracks selected the button is disabled.
- On a finished track, open the row's actions menu (the three dots) and choose **Review match**.

In a window narrower than 1250 pixels the panel replaces the table. Choose **Back to tracks** to return; your selection stays.

A track in the **Archive** cannot be reviewed. Restore it first.

## Read the evidence { #review-a-catalog-match }

The panel shows, from top to bottom:

| Section | What it tells you |
|---|---|
| A line under the heading | Why you are here, for example *Confirm the recording and edition.* or *No retained catalog query; explicitly Requery to search configured providers* |
| **Catalog access** | For each provider you ticked: *Direct access*, *Configured solver* or *Unavailable* with the reason, plus any error from the last lookup |
| **Current file** | The artists, version, album and duration CueRithm read from your file |
| **Candidate recordings** | Up to ten retained candidates. Each shows the provider, **Name similarity** as a percentage, title, version, artists, album, duration, and ISRC, track ID and release ID when known. **Source page** opens the catalog's own page in your browser. |

Name similarity ranks candidates. It is not the chance that a candidate is right, so a 96% candidate can be the wrong mix. Compare the duration and version with your file.

A candidate can carry a list of **conflicts**. Each means the candidate disagrees with something already in your file:

| Conflict | Meaning |
|---|---|
| `edition` | The mix or version differs from the one in your file. A file titled `Here In My Arms (Enjoy The Silence) (Extended Mix)` would show it on a Radio Edit candidate. |
| `isrc`, `album` | Both have the value and the values differ |
| `release_id` | The file stores a release ID from this provider and it differs |
| `duration` | **Match duration** is on and the lengths differ by more than the allowance |
| `release_edition_unresolved` | MusicBrainz identified the recording but not which release you own |
| `recording_id`, `release_track_id` | The file already stores a different MusicBrainz identifier |

## Search again with Requery { #requery }

A finished track has no saved candidates, and an old candidate list can be out of date. Choose **Requery** beside **Candidate recordings** to run a fresh search with your saved provider list.

Requery reads the tags in the file as they are now, so to search with a corrected title or artist, fix it first in [Edit tags and artwork](edit-tags-by-hand.md). It ignores **Skip previously tagged catalog lookups**. It replaces the candidate list, and any choice you made against the old list is dropped.

**Requery** is disabled, with an explanation, when no provider is configured, when none is reachable (for instance Traxsource without a solver; see [Get past a Cloudflare block](cloudflare-solver.md)), or while the track is still being prepared.

## Choose a candidate and decide each field { #choose-fields }

1. Select a candidate. CueRithm builds a preview for it.
2. If the candidate lists conflicts, tick **I reviewed the recording/version conflicts and want to use this candidate**. Without it **Use selected match** stays disabled.
3. Under **Fields from selected match**, look at each row. **Current** is what the file has, **Proposed** is what the candidate offers, **After choice** is the result.
4. Every field starts on **Keep current**. Change a field to **Replace**, or to **Append** where several values are allowed (artists, album artists, genres, styles, remixers), only for the values you want.
5. For artwork, choose **Preview candidate artwork** to load a checked copy, then set **Artwork choice** to **Replace selected cover**.
6. If a row says the values need ID3v2.4, tick **Allow conversion to ID3v2.4**. Older players may not read an ID3v2.4 tag.

The fields offered are Title, Artists, Album, Album artists, Genres, Styles, Label, Mix / version, Remixers, Catalog number, ISRC, File key, Release date, Original release date, Track number, Track total, Disc number and File BPM. MusicBrainz identifiers are shown as read-only evidence in **Recording and release evidence**; they cannot be written here.

A row stays on **Keep current** when the candidate has no value, when the source does not allow writing that field, or when you already saved an edit on an Incoming track. In that last case the saved edit takes priority and the row says so.

If anything blocks the choice, a list of reasons appears under the fields, for example a conflict you have not acknowledged or artwork you have not previewed. **Use selected match** stays disabled until the list is empty.

## Apply it { #apply }

Choose **Use selected match**.

!!! warning "This writes into your file"
    On a finished track you can take the change back with **Undo saved changes** in [Undo a tag change](tag-history.md). A match accepted on a track still in Incoming has no Undo button, so check the field list first.

- **Track in Incoming:** your decision is saved and the same preparation resumes. The panel reads *Preparation was queued*. Only the fields you changed to Replace or Append, and any artwork you chose, are written from the catalog. The **Fields to write** list, the measured key and BPM file tags and the `ENERGY` tag are not applied to this track. Cleanup and file organization still run if you turned them on.
- **Track in Tracks:** the change is written to the file at once through the metadata editor. The result lists each field as changed, unchanged or failed. If you chose Keep current everywhere, the result is *unchanged* and nothing is written.

Choose **Review again** to start over, or **Metadata history** to find the change in [Undo a tag change](tag-history.md).

**Keep existing once** appears for a track waiting in Incoming. It lets the track continue as if there were no match, without changing your **No catalog match** setting. Key, BPM and energy file tags are written as your settings say, and your own tags stay as they are.

## What can go wrong { #what-can-go-wrong }

| Symptom | Cause | Fix |
|---|---|---|
| **Use selected match** is disabled | A blocker is listed, or the preview is still loading | Read the list under the fields and resolve each item |
| *Preparation is running or queued* | The track is still in Incoming and not yet waiting | Wait for it to stop on the warning triangle |
| No candidates and **Requery** disabled | No provider is available | Set one up with [Connect provider accounts](provider-accounts.md), or see [Catalog tagging finds nothing](../help/troubleshooting/no-catalog-match.md) |
| Candidates changed while you were choosing | A newer lookup finished | Choose again; an old preview can never authorise a write |
| *More candidates were found than could be retained* | Only ten candidates are kept | Press **Requery** after correcting the title |
| The best candidate is not listed | The catalog does not have it | Edit the tags yourself |

## Related { #related }

- [Edit tags and artwork](edit-tags-by-hand.md): change any field without a catalog.
- [Undo a tag change](tag-history.md): take back a review you applied to a finished track.
- [Choose providers and fields to write](choose-providers.md#matching): make future matches stricter.
- [Fix a track that needs attention](../files/needs-attention.md): the Incoming side of the same problem.

<!-- verified: apps/desktop/src/components/MatchReview.tsx; apps/desktop/src/components/CatalogEvidence.tsx; apps/desktop/src/App.tsx (toolbar, compactInspector 1250px); apps/desktop/src/components/TrackTable.tsx (status triangle, row menu); apps/desktop/src/test/match-review.test.tsx; crates/cuerithm-library/src/match_review/store.rs (retained candidates, can_requery reasons, archived); match_review/fields.rs (field list, Keep default, Append for multi-value, saved edit priority); match_review/workflow.rs (blockers, Ready vs Incoming apply, Requery ignores skip_tagged); match_review/worker.rs (tags cleared for UseSelected, Keep existing once); crates/cuerithm-autotag/src/evidence.rs (conflict codes); legacy guides/metadata-editing.md#review-a-catalog-match re-checked -->
