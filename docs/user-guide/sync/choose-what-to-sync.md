# Choose what to sync

<p class="lead">Decide which tracks and playlists go to your DJ software: everything, one playlist or folder, or only what changed since last time. The choice takes a minute and applies to every destination.</p>

## Which tracks can be sent { #eligible-tracks }

Only tracks that are prepared and in **Tracks** are eligible. Tracks still in **Incoming** and tracks you have moved to **Archive** are never sent, even when a playlist you are syncing holds them. If a playlist looks short after syncing, check whether some of its tracks are archived ([Archive tracks you no longer play](../files/archive.md)) or still being prepared ([Follow a track through Incoming](../files/follow-incoming.md)).

Two conditions stop the whole preview, with a message that names the track or list:

| Message | Cause | Fix |
|---|---|---|
| "Prepared track 'Title' has no saved analysis" or "...has no saved cue/grid publication; prepare it before sync" | The track never finished analysis | [Analyze the track again](../analysis/analyze-again.md) |
| "Smartlist 'Name' has no materialized result; refresh it before sync" | A smart playlist in the scope has not produced a result yet | Open it so it refreshes ([Keep smart playlists up to date automatically](../playlists/automatic-smart-playlists.md)) |

## The three scopes { #scopes }

Open **What to sync**.

| Choice | Sends | Notes |
|---|---|---|
| **Full library** | All eligible tracks and all playlists and folders | The default |
| **Playlist or folder** | The playlist or folder you pick in the **Playlist** menu, everything inside it, and the tracks they hold | Also sends the folders above it, so the tree keeps its shape; [details](#playlist-scope) |
| **Modified since last sync** | Tracks and playlists that changed since the last successful sync to this destination | Rules differ for rekordbox; see [below](#modified) |

## Send one playlist or folder { #playlist-scope }

Choose **Playlist or folder**, then choose a name in the **Playlist** menu. The menu lists every playlist, folder and smart playlist by name. With **Club nights** selected, CueRithm sends the folder, **Warm-up set**, **Peak hour** and the tracks in both, and nothing else. Sub-folders and their playlists come with the folder. If you pick a playlist inside a folder, the folders above it are sent too, without their other playlists, so it lands in the right place.

A track in two of the selected playlists is sent once.

## What happens to smart playlists { #smart-playlists }

DJ programs differ in whether they can hold a rule. CueRithm sends a rule as a rule only when the destination can represent the whole definition. Otherwise it sends the tracks the playlist holds at that moment, as an ordinary playlist, and the preview says which lists were affected.

| Destination | What happens to a smart playlist |
|---|---|
| rekordbox 6 / 7 | Must be converted. Tick **Convert smart playlists to track lists** or the preview stops with "Complete smart semantics require explicit whole-list materialization". The list is sent with its current members. Smart playlists that rekordbox already holds keep their rules |
| VirtualDJ, Engine DJ, djay Pro, Serato DJ Pro 4 | Sent as a native smart playlist when the whole rule fits; otherwise as an ordinary playlist of its saved tracks. **All smartlists to playlists** sends every one as an ordinary playlist |
| Pioneer USB | Always an ordinary playlist |
| Mixxx | Always an ordinary playlist (or crate). Mixxx has no smart playlists |

A native smart playlist keeps evaluating inside the DJ program, so it also picks up tracks you add there later. An ordinary playlist stays as it was when you synced. If a rule uses a location prefix and the destination copies your audio to new locations, CueRithm sends the saved tracks instead, and the preview explains why. [Smart playlist fields and operators](../reference/smart-playlist-fields.md) lists what a rule can use.

## Modified since last sync { #modified }

**Modified since last sync** sends less, so it is quicker once the first full sync is done. It works differently for rekordbox than for the other destinations.

**VirtualDJ, Engine DJ, djay Pro, Serato, Pioneer USB, Mixxx.** CueRithm remembers what it sent at the last successful sync to this destination. It sends tracks and playlists whose data differs from that record, plus the full membership of any changed playlist and the folders above it. Three things make it send everything instead:

- there is no earlier successful sync to this destination
- you changed an option since then (the record includes the options)
- you restored a backup, which clears the record

An interrupted or failed sync does not count; the record advances only when a sync is completed or recovered.

**rekordbox.** CueRithm compares each selected field of each track with rekordbox. Tracks rekordbox does not have yet are always included. A track that rekordbox already has is included only when one of its fields needs attention, which includes every field the first time, because the first sync of an existing track needs your choices ([Resolve rekordbox sync conflicts](rekordbox-conflicts.md)). Use it to add this week's purchases: [Add only new tracks to rekordbox](rekordbox-new-tracks.md).

## What happens to playlists you removed { #removed-playlists }

**Full library** and **Modified since last sync** can also remove playlists that CueRithm itself created in the destination and that you have since removed in CueRithm. The preview counts them ("owned playlists to remove"; in rekordbox they appear as **Delete owned object**). A playlist that CueRithm matched to one that was already in the destination is kept ("matched playlists retained"). Everything CueRithm did not create is left alone, and removing a playlist never deletes tracks or music files. Mixxx removes a list it created only if you have not changed it in Mixxx since.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| A playlist is missing tracks in the DJ program | Some of its tracks are archived, in Incoming, or were skipped with a warning | Read the preview warnings; restore archived tracks first |
| rekordbox preview: "Some member tracks are missing from this preview" | A playlist or My Tag names tracks that are neither in rekordbox nor part of this sync | Choose **Full library** or a scope that includes them, and keep **Title** ticked; see [Add only new tracks to rekordbox](rekordbox-new-tracks.md#playlists-with-new-tracks) |
| Mixxx: "Replacing the whole Mixxx library needs the full library; choose the full scope" | **Replace the whole library** is limited to the full scope | Choose **Full library** |
| **Modified since last sync** sends everything | The record was cleared or an option changed | Expected; the next one is smaller |

## Related

- [Set the sync options](sync-options.md): what is written for the tracks you chose
- [Preview, apply and restore a sync](preview-apply-restore.md): checking the preview
- [Build a smart playlist](../playlists/build-a-smart-playlist.md): the lists affected by the table above
- [Group playlists in folders and reorder them](../playlists/organize-playlists.md): how the tree you send is built

<!-- verified: apps/desktop/src/views/Sync.tsx and RekordboxSync.tsx (scope labels, Playlist menu); crates/cuerithm-library/src/dj_sync.rs (dj_sync_library, project_playlists, project_track); dj_sync_service.rs (select_changed, baseline, replace_baseline, invalidate_baseline on restore, Mixxx full-scope check); rekordbox_sync_service/planning.rs (Changed scope skips unchanged existing tracks); native_playlist_sync.rs (smart list materialisation messages); crates/cuerithm-dj-sync/src/sync.rs (copied-audio location rules); docs/research/dj-sync-smart-rules.md; legacy guides/sync.md re-checked -->
