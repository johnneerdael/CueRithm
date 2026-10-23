# Import a DJ library

Use **Import library** in the sidebar to bring tracks, metadata, playlists and
crates into CueRithm while keeping the source library intact. Imported tracks
appear in **Tracks**. Optional analysis, retagging, audio-tag writes and file
naming start off, independently of your Incoming settings.

## Choose and inspect a source

1. Choose **Source app**, then **Choose library**. Sources include M3U playlists,
   Apple Music/iTunes XML, Traktor, VirtualDJ, Mixxx, Serato, Engine DJ, djay,
   Mixed In Key 11, rekordbox, Pioneer USB and DJ.Studio.
2. For VirtualDJ or Serato libraries spread across drives, use **Add files from
   another source volume** to include the additional native files.
3. If the source refers to a Windows drive, UNC share or moved volume, choose a
   local mapping folder and enter its **Recorded source prefix**. For example,
   map `D:\Music` to the folder containing those tracks on this computer.
   CueRithm does not join tracks merely because their filenames match.
4. Optionally capture raw audio tags, source history or the source queue. These
   are separate observations; capturing them does not authorize processing.
5. Choose **Inspect library**. Review the detected format, available fields and
   limitations, then select the tracks and collections to import.

For **rekordbox**, choose a configured local installation. Additional local
installations and Wine/Proton locations are configured in **Sync**. Import uses
direct library access; rekordbox XML is not an import source. For **Mixed In Key
11**, choose a macOS-origin `Collection11.mikdb`; native Windows storage has not
been inspected.

Source identity is remembered for refreshes. Leave **Source binding** on automatic
matching for ordinary use. Treat a source as a separate library when it should
have independent history; bind an explicit replica only when it is a copy of an
already imported source.

Missing music stays visible with its imported metadata. Playback and audio
processing require resolved media. A recognized imported key can display as
Camelot while its original notation remains available in imported details.

## Choose values and optional processing

Under **Processing choices**, select the values to import. The detected source
profile explains unavailable or partially supported fields. These choices also
limit which fields later processing may adopt.

Each processing choice is independent:

| Choice | Result |
| --- | --- |
| **Analyse key**, **Analyse energy**, **Analyse BPM**, **Analyse beatgrid** | Compute the selected values from audio. Choose **Fill missing only** or **Replace selected values** for each enabled analysis. |
| **Generate cues** | Propose cues using audio analysis. Internal prerequisites do not automatically replace your imported key, energy or grid. |
| **Replace existing hotcues** | Allow replacement when generating cues. Enabling this alone generates nothing. Unknown native pad occupancy can block adoption. |
| **Retag / enrich metadata** | Look up selected fields using configured providers. Missing account access can block this stage while leaving the import available. |
| **Apply cleanup rules** | Propose the selected cleanup rules. |
| **Write audio tags** | Write only the selected tags after a separate effects review. Accepted imported values can be written without a provider lookup. |
| **Rename files** | Choose a filename pattern and rename, move or copy mode. Move and copy modes also require a destination folder. |

Library metadata changes and audio-tag writes have separate switches. Retagging
does not by itself authorize changes to your music files.

Renaming or moving an original file referenced by a source library is blocked
until reference repair is qualified for that source. Copying preserves the
original file and its source reference. Close applications using the source
library before applying tag writes to original music files.

## Review and apply

Choose **Review import** to inspect the proposed records and field changes.
Conflicts keep local values by default. Choose a source value or enter a custom
value when appropriate, then rebuild the preview before applying. Where a source
contains conflicting candidate values, select the observed candidate to use.

Apply the reviewed import to store its records. If you enabled optional
processing, generate its proposals, choose **Review processing effects**, and
review the changes and blocked items before **Apply processing effects**.
Changing a choice or editing the library can require a fresh preview.

An imported track is not automatically a prepared track. Source cue and grid
timing can remain unresolved, and reading a format does not certify exporting
every native detail back to it.

## Browse imported playlists

Open **Imported playlists** in the sidebar to browse imported folders, playlists
and crates. Repeated tracks keep their separate positions; unavailable or
unresolved members remain visible. **Preview** is available for resolved music,
and **Details** shows a track's imported values and provenance.

If you import a nested playlist without its folder, the playlist appears at the
top level. Its original parent is retained; importing that folder later restores
the folder context.

Imported collections are read-only snapshots. Imported smart rules remain source
observations and do not become live CueRithm smart playlists. Use the ordinary
[playlist tools](../playlists/index.md) for playlists you create and edit in CueRithm.

## Refresh, retry and recover

Open **Import history** to see saved processing choices and stage results.

- **Refresh source** inspects the source again with the saved policy. Review any
  changes before applying; a partial scan does not authorize deleting omitted
  tracks or collections.
- **Retry processing** retries incomplete stages while retaining completed
  proposals with unchanged inputs. Dependent stages are recomputed if their
  inputs changed. Retry is available before publication begins.
- **Review effects** opens completed proposals for approval.
- **Recover effects** resumes interrupted changes with checks on the current
  library and files.
- **Undo effects** reverses eligible processing changes when later edits allow
  it. It does not remove the imported source records.

Blocked processing leaves the imported metadata visible. Recovery and Undo can
also be blocked by later file or library edits; review the explanation in history
before proceeding.

Portable `.cuerithm` export cannot preserve DJ-library import records yet. Export
is blocked once a source inspection has been saved, including when its tracks
have not been imported or are linked to existing tracks. To keep inspected
sources, imported tracks, playlists and provenance, use **Library backup →
Same-installation recovery snapshots**. These snapshots are for recovery on this
installation; see [Back up your library](library/back-up-library.md#installation-snapshots).

The new source readers have automated fixture and acceptance coverage. Native
application and device validation remains a separate gate; see the
[qualification record](https://github.com/johnneerdael/CueRithm/blob/main/docs/research/library-import-qualification.md) for
the tested profiles and remaining limits. Source DJ databases are read-only
during import. The experimental DJ.Studio writer is not available in the desktop
import flow.
