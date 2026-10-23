# Match a folder as a release

<p class="lead">When a folder holds one release, CueRithm can recognise the release from the first tracks that match and reuse its track list for the rest. Every file then takes its album, label, catalog number and track position from the same release. Setup takes two minutes.</p>

**Before you start**, make sure normal catalog tagging works. Follow [Tag new tracks from Beatport and Traxsource](tag-new-tracks-automatically.md) if it does not, and tick **Album**, **Track number**, **Track total** and **Catalog number** under **Fields to write** if you want those written (see [Choose providers and fields to write](choose-providers.md#fields-to-write)). Album matching only decides which catalog entry a track is compared with. The field list decides what is written.

## When it helps { #when-it-helps }

Use it for a folder that holds a whole EP or album, added in one go. It does nothing useful for a folder of unrelated singles. A purchase folder such as `~/Music/Beatport`, with 26 tracks from 26 different line-ups, will never have one release that covers half of the files, so album matching stays idle there.

It works on Beatport and Traxsource releases only. A release is identified by provider and number together, so a Beatport release and a Traxsource release that share a number are never combined.

## Turn it on { #match-a-folder-as-a-release }

1. Open **Settings → Automatic tagging** and find **Matching**.
2. Tick **Album matching**.
3. Set **Album matching ratio**. The default is 0.5. It is the fraction of the folder's Incoming files that must match one release before the rest use that release's track list. The range is 0 to 1.
4. Check that Beatport, Traxsource or both are ticked in the provider list.
5. Choose **Save settings**, then add the folder's files together with **Add tracks** or drop the folder into a watch folder.

## What happens { #how-it-plays-out }

Take an eight-track EP folder with the ratio at 0.5.

1. The first tracks are matched one by one, exactly as usual.
2. When four of the eight have matched the same release, CueRithm loads that release's track list once.
3. Each remaining track is compared with the tracks on that list, using your usual **Matching strictness**, duration and conflict checks. A track that fits takes its data from the list, so album, label, catalog number and track number and total come from one release.
4. With **Combine providers** off, the provider whose release was chosen is asked first for the remaining tracks.

A few rules decide the count:

- Only files that sit in Incoming at the same time in the same folder count. Files in a subfolder belong to that subfolder's own group.
- A file that fails or finds no match still counts towards the total, and a retry never counts twice.
- Tracks already in Tracks are never retagged, and changing the setting later does not touch them.

!!! info "Tracks are processed in parallel"
    CueRithm prepares several tracks at once, so a few of the first tracks may finish before the release is known. They are matched on their own, and Review match shows them if their data differs.

## Check the result { #check }

Select the tracks, choose **Edit metadata**, and compare **Album**, **Label**, **Catalog number**, **Track number** and **Track total**. They should agree across the folder. If one track picked a different edition, open [Review match](review-a-match.md) for that track and compare the candidates.

## What can go wrong { #what-can-go-wrong }

| Symptom | Cause | Fix |
|---|---|---|
| Nothing seems different | The ratio was never reached, or the folder was added in pieces | Add the folder in one go, or lower the ratio |
| One track took the wrong version of a release | Two releases share a track, such as an EP and its remix EP | Raise the ratio, or fix the track in [Review and fix a wrong match](review-a-match.md) |
| Tracks wait in Incoming after a release lookup | The release page was missing, malformed or blocked. Traxsource release pages use a solver when blocked. | See [Get past a Cloudflare block](cloudflare-solver.md), then retry the row |
| Fields are empty after matching | Their **Write** box is off | Tick them under Fields to write and run [Analyze again](../analysis/analyze-again.md) |

## Related { #related }

- [Choose providers and fields to write](choose-providers.md#matching): all matching settings in one table.
- [Review and fix a wrong match](review-a-match.md)
- [Keep a folder like Beatport in your library](../files/watch-folders.md): add release folders automatically.
- [How does catalog matching work?](../how-it-works/matching.md)

<!-- verified: apps/desktop/src/views/TaggingSettings.tsx (Album matching, ratio); crates/cuerithm-library/src/album_context.rs (members, ratio, provider identity, tests); crates/cuerithm-library/src/prepare.rs (observe/selected/hint, Beatport and Traxsource only); crates/cuerithm-autotag/src/lib.rs match_track_with_album (album candidates, provider order); vendor/onetagger/providers/beatport.rs get_album; legacy learn/auto-tagging.md#match-a-folder-as-a-release re-checked -->
