# rekordbox application acceptance on a swapped library, 2026-10-20

Aggregate findings only. The library, track titles and paths stay under the ignored `.analysis/`.

## What was run

The installed rekordbox 7.2.19.0342 on macOS was pointed at a **copy** of a real library (3,417 tracks, 1,033 playlists,
36,752 cue rows). The packaged CueRithm app was driven through its **rekordbox** screen against that copy, then rekordbox was
opened on the result.

Safeguards, both rounds:

- rekordbox and its background agent were quit first (the agent holds the database open).
- `~/Library/Pioneer/rekordbox` was cloned to a backup and a SHA-256 manifest of all 3,483 files (230.8 MB) was taken.
  The original folder was renamed aside, a clone of the backup took its place, and nothing touched the held original.
- Afterwards the test folder was moved away, the original renamed back, and every file was re-hashed against the manifest:
  3,483 of 3,483 identical, in both rounds. The agent was then started again, as it was running before.
- rekordbox's settings and licence folder (`~/Library/Application Support/Pioneer`) was never read or changed by the test.

## Round 1: add tracks, then open rekordbox

| Step | Result |
| --- | --- |
| Preflight | rekordbox 7.2.19.0342, library present, can be opened, writing verified. |
| Link tracks | 3,417 rekordbox tracks linked. Afterwards CueRithm held six tracks: the three test files plus three of rekordbox's bundled demo loops, the only library files present on this machine (linking brings rekordbox's tracks into Incoming, as its text says). |
| Preview with "also add CueRithm tracks" | 3 tracks to add, 0 playlists, 3,408 blocked, 13 needing a decision. |
| Apply | "Wrote 3 tracks … A verified backup was made first." Database before/after: tracks 3,417 → 3,420, active local 3,409 → 3,412, cue rows +23, playlists and links unchanged, integrity OK, 0 foreign-key issues. |
| rekordbox opens it | No login or cloud prompt. Collection reads 3,412 tracks. The three added tracks show artwork, an analysed waveform preview with hot-cue letters on it, BPM and key. |
| After rekordbox quits | Integrity OK; track, playlist, link and cue counts exactly as written (rekordbox changed none of them). |
| Repeat sync | Link tracks: 3,420 linked, 0 new. Preview: "0 tracks and 0 playlists to write", Apply disabled. |

## Round 2: ordered playlist and a later edit

On a fresh clone, a playlist of six CueRithm tracks was built in a deliberate non-alphabetical order. Three of the six
exist in rekordbox (the demo loops) and three do not.

| Step | Result |
| --- | --- |
| Preview | "Create · 3 tracks · 3 not linked to rekordbox": the playlist is created with the linked members only. |
| Apply | "Wrote 0 tracks and 1 playlists", verified backup first. |
| Later edit | One track moved up one place in CueRithm. Preview: "Update · 3 tracks". Applied. |
| Stored result | Read back with the crate's own reader: three soft-deleted link rows from the first write and three active rows in the new order (what an update leaves, because the writer soft-deletes and re-inserts members). Playlists 1,033 → 1,034, links 13,355 → 13,361 (the three extra are the soft-deleted rows), tracks and cues unchanged. |
| Repeat sync | Preview: "0 tracks and 0 playlists to write". |
| rekordbox opens it | Opens, collection 3,409 tracks, integrity OK after it quit; counts unchanged by rekordbox. |

## Findings

- **Restoring an older rekordbox library.** CueRithm remembers links from an earlier sync. Against a library that no longer
  holds the tracks it added (round 2 started from a fresh clone that lacked round 1's three tracks), preview marks them
  "Blocked: deleted in target" and does not add them again. That is the conservative result. The screen offered no way to
  add them again in that state, so recovering from a restored rekordbox backup is not a verified workflow.
- **The agent rewrites `master.db` within a minute of starting.** A backup taken before it runs is not a backup of the file
  afterwards; the acceptance took a new backup and manifest after each agent run. The preflight's request to close rekordbox
  and its agent is therefore a hard requirement, not a courtesy.
- **A macOS prompt appeared once** ("rekordbox would like to access files on a network volume") when a track was
  double-clicked in the browser. It was dismissed without allowing, and the stored decision for rekordbox was reset
  afterwards. Its cause was not established; it did not come from the CueRithm write.
- **One unexplained UI observation.** After opening Playlist tools and returning to the library with a web-view context menu
  open, the track table stayed on "Loading tracks…" until another view was selected and Tracks reopened. It was not
  reproduced or investigated.

## Not established

- Cue and grid **playback** in the rekordbox player: no track was loaded into a deck (loading raised the prompt above, and
  the player area was not driven further). Cue markers and the waveform preview were seen in the track list only.
- The new playlist was **not** seen in rekordbox's own playlist tree; its content was confirmed through the database.
- Loops, artwork beyond the list thumbnail, MyTags and smart lists in the application.
- Edits to a track's own fields after the first sync (CueRithm has no per-track edit screen; the only later edit tried was
  the playlist reorder), and rekordbox-side edits being left alone on a later sync.
- Interrupted-write recovery in the real application (covered by tests on copies only).
- rekordbox 6, Windows, and any cloud-synchronised library.
