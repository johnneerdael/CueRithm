# Add only new tracks to rekordbox

<p class="lead">After a purchase day, send only the new tracks, with their playlists and My Tags, to rekordbox without answering a single per-field question. The sync takes about two minutes once your first sync is done.</p>

**Before you start:** you have synced once ([Sync to rekordbox 6 or 7](rekordbox-direct.md)), the new tracks are prepared and sit in **Tracks**, and rekordbox and rekordbox Agent are closed.

## How CueRithm decides a track is new { #how-new-tracks-work }

A track is new when rekordbox has no track at the same file location. CueRithm matches by the audio file's location, not by title, so:

- a track you imported into rekordbox yourself from the same file is not new, because rekordbox already points at that file
- the same song stored at a different location in rekordbox counts as new, and the sync adds a second entry for it

New tracks need no review. CueRithm sends the values you ticked under **Include in sync**, leaves out any that are empty in CueRithm, and then records those empty fields as settled, so the next sync does not ask about them. Existing tracks keep the review described in [Resolve rekordbox sync conflicts](rekordbox-conflicts.md).

## Send the new tracks { #add-new-tracks }

1. Open **Sync** and choose **Sync to → rekordbox 6 / 7**. Wait for **Ready for preview**.
2. Open **What to sync** and choose **Modified since last sync**. New tracks are always part of it. If you collected the new tracks in a playlist, **Playlist or folder** with that playlist works too ([Choose what to sync](choose-what-to-sync.md#modified)).
3. Leave **Include in sync** as it is. Keep **Title** ticked under **Metadata fields**; a new track cannot be created without one.
4. Choose **Preview rekordbox sync**.
5. Find the panel headed **N new tracks** and open **Tracks to add**. This is your list of additions. It shows 100 at a time; **Show N more tracks** reveals the rest.
6. Check **Playlists and My Tags**. Playlists and My Tags that hold the new tracks are created in the same sync, so a track lands in its playlists straight away.
7. Choose **Sync rekordbox**.

**What you should see:** **Sync completed** with the number of tracks added. Open rekordbox and the tracks are in the collection with their metadata, cover art, beatgrid and cues.

## What a new track receives { #what-new-tracks-get }

| Data | What rekordbox gets |
|---|---|
| Metadata | Every field ticked under **Metadata fields** that has a value in CueRithm |
| Cues and loops | The track's current cues, as hot or memory cues |
| Beatgrid | One BPM and a first-beat anchor |
| Artwork | The cover, in three sizes |
| Analysis | New analysis files (beat grid, cues and waveforms) made from the decoded audio |
| Audio | Not copied. rekordbox points at your file where it is. A format rekordbox cannot play needs **Create compatible WAV copies for unsupported audio** ([Set the sync options](sync-options.md#audio-copies)) |

## When existing tracks appear too { #existing-tracks-appear }

**Modified since last sync** also lists a track that rekordbox already has when one of its fields needs attention, and it always does so the first time you sync that track. The preview then shows a **Review fields** table next to the **N new tracks** panel.

To add the new tracks and leave those alone, choose **Skip unresolved fields**. Skipped fields stay unsynced, nothing is written for them, and they show up again in the next preview. Or send a playlist that holds only the new tracks. The choices are explained in [Resolve rekordbox sync conflicts](rekordbox-conflicts.md#bulk-choices).

## When a playlist waits for its tracks { #playlists-with-new-tracks }

If a playlist or My Tag names tracks that rekordbox does not have and that are not part of this sync, the preview shows this under **Playlists and My Tags**: "Some member tracks are missing from this preview. Include their tracks and title metadata in the sync scope, then preview again." **Sync rekordbox** stays disabled and no playlist or My Tag change is applied from that preview.

Fix it by widening the scope so the tracks are included (**Full library**, or a **Playlist or folder** that contains them) and by keeping **Title** ticked. The tracks and their lists are then created together.

## Choose rekordbox from the Sync to menu { #destination-menu }

From CueRithm 0.1.13 rekordbox is one entry, **rekordbox 6 / 7**, in the same **Sync to** menu as the other destinations. The separate rekordbox tab and the per-track "Use CueRithm" choice for new titles are gone. CueRithm remembers that rekordbox was your last destination and shows it again when you return to Sync. It does not remember the scope and options, which reset to their defaults each time ([Save a destination for next time](saved-destinations.md#remembered-choices)).

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "New rekordbox tracks need a title. Include title in Metadata fields without skipping it, then preview again." | **Title** is unticked | Tick **Title**, preview again |
| A new track is listed twice in rekordbox | rekordbox already had it at another file location | Remove one entry in rekordbox and keep each file in one place ([My sync does not show up in rekordbox](../help/troubleshooting/rekordbox-sync-problems.md)) |
| "Audio source changed after preparation; prepare the track again before native sync" | The file was edited after CueRithm analysed it | [Analyze the track again](../analysis/analyze-again.md) |
| **Sync rekordbox** is disabled after the preview | A playlist waits for tracks, or the library is **Read only** | Read the red line under the summary and the status block |

## Related

- [Sync to rekordbox 6 or 7](rekordbox-direct.md)
- [Resolve rekordbox sync conflicts](rekordbox-conflicts.md)
- [Choose what to sync](choose-what-to-sync.md)
- [Follow a track through Incoming](../files/follow-incoming.md): getting a purchase ready for its first sync

<!-- verified: apps/desktop/src/components/RekordboxConflicts.tsx (new tracks section, Tracks to add, paging); RekordboxEntities.tsx (pending message); views/RekordboxSync.tsx; sync-preferences.ts (destination remembered, rekordbox options not); src/test/rekordbox-sync.test.tsx (new tracks apply without choices); git show d1d3d52; crates/cuerithm-library/src/rekordbox_sync_service/planning.rs (new track proposals, acknowledgement of omitted fields, title warning, Changed scope); native_sync.rs (matching by path) -->
