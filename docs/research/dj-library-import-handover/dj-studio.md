# DJ.Studio 4.2.9: clean-room library sync handover

Research date: 2026-10-23. Scope: static inspection of separate application copies and read-only inspection of copied local persistence. No DJ.Studio launch, IPC invocation, music-tag modification, or write to a live library was performed. This document contains descriptions and synthetic fixtures, not proprietary implementation source or real library rows.

**Seen** means a shipped artifact, public document, or read-only persistence inspection establishes the claim. Static behavior is not observed execution. **Inferred** means a proposed adapter behavior derived from that evidence. **Unknown** is a release gate, not an invitation to guess.

## Implementer conclusion

**Seen [N:142515–143495, D:130998–131080]** DJ.Studio has two different persistent library surfaces. `Database/studio.db` indexes the native Explorer/local library, playlists, bookmarked folders, and links to external DJ libraries. Imported tracks and their detailed edits also have extensionless JSON records in `Database/audio-library-table` and beat structures in `Database/track-structures-table`. A SQL-only adapter cannot synchronize the full imported library or hotcues.

**Inferred** Implement a read adapter over both surfaces, joined conservatively by normalized local file path, while preserving their distinct IDs. For write-back, stage an offline change set across the SQLite database, selected JSON records, and derived aggregate caches. First enable narrow existing-record metadata changes only after the runtime acceptance gates below pass. New analyzed-track creation, grid replacement and project propagation require additional gates; the research is not runtime acceptance.

**Seen [N:142775–142777, 143435–143437]** Both native DJ.Studio hotcue getters explicitly throw an unimplemented-method error. The generic `library/v2/get-hotcues` route therefore does not establish an internal DJ.Studio cue reader/writer. Detailed cues must be read from imported-track JSON.

## Reproducible evidence identity

The inspected installed app reports 4.2.9 in its bundle metadata and `package.json`. The archive entry point is `bootstrap.js`; native Electron code is `dist/main/main.js`. Its package declares `better-sqlite3-multiple-ciphers` 12.8.0. That dependency alone does not mean DJ.Studio's own database is encrypted: the copied `studio.db` opened using ordinary SQLite and passed `integrity_check`.

| Label | Relative artifact within the copied resources | SHA-256 |
|---|---|---|
| A | `app.asar` | `51980b37cb61fa1afe8e054982678f7773736b7db7c66224affe2ea1a9b47aeb` |
| N | extracted `dist/main/main.js` | `d4a2366f19cb29a8f7ef427f0736a527f360f887c0cfd9fd0f87b78c977ec75f` |
| P | extracted `dist/main/preload.js` | `93986b4e6937019a559c18cfddda762ee5ef3a71a1d9b2322b93316c9fb0080d` |
| W | `public/download/webapp.zip` | `d28fde435da7f7e80746a8826eaf6a33f7b87f5ac0e841a5a5c35f4f64bb5021` |
| D | W entry `assets/FullScreenButton-oJZRjRT5.js` | `a129c34b6ff9a5afc441cfa2c62933dcd0768e17ef687e254b9107c1e7d0d093` |
| H | W entry `assets/Home-CBLgHayF.js` | `7a9ee3a9993195f59033c11b08fbbcbb77f11022cf5f535753dd3a4aa7bfae4b` |
| S | W entry `assets/simple-studio-DWKCd2tG.js` | `c4d7a252e74525de57b47af83e711ebe09dcce63c8e8e7335c35dbf682ae5dd5` |

**Seen:** the bundled web archive contained no `.map` files; analysis used shipped JavaScript and its symbols, not a recovered source tree.

N, D, H and S line anchors refer to local inspection copies formatted with Prettier 3.9.10, `prettier --parser babel INPUT > OUTPUT`. Formatted N SHA-256 is `a418ece688ee337bafc4a88398df4c79bd911906b80768f5d2248bd8011e8e7a`; formatted D is `5ebcacdde5d40428124494f5ec16860f3ce84aaf6c69751db54cc0ae4c6c21a0`. Formatted S SHA-256 is `2c397583b899349ea71c26ca653a4ccbfea56888ea86fb35c06ce5cb04da8f8c`. Symbols and artifact hashes remain the primary lookup keys if formatting differs. Do not commit extracted or formatted code. Public migrations are identified below by their shipped filenames.

**Seen [N:111646–111740, `LocalWebProtocol.verifiedLocalWebAppVersion`]** Native and renderer versions are separate. The bundled renderer deployment is `dbc14cf914187b40bb359ad570ffb4c4a95e83b8`. A local configuration selected another renderer hash and recorded both 4.2.9 and a newer native-version association. Therefore the existing local DB is useful schema/shape evidence, but is not proof that its last writer was exactly the bundled 4.2.9 renderer. **Unknown:** runtime compatibility of that active renderer. Pin both native and renderer identities before enabling writes; reject unrecognized combinations.

## Discovery and ownership boundaries

| Surface | Seen storage/discovery | Meaning and adapter rule |
|---|---|---|
| Root folder | Native `config.json` key `root-folder`; `settings:getRootFolder`; default Music/DJ.Studio [N:114098–114138, 115129–115137, 146802] | User-configurable root. Read only the path setting; do not parse/export authentication settings. |
| SQLite index | `<root>/Database/studio.db` [N:143304–143305, 143478–143495] | Metadata, native playlists/folder bookmarks/external links. |
| Imported-track records | `<root>/Database/audio-library-table/<shard>/<record-key>` [D:131015–131020, 74405–74446] | JSON metadata, key/BPM, cue groups, preferred grid references, analysis provenance. |
| Beat structures | `<root>/Database/track-structures-table/<shard>/<structure-key>` [D:131038–131045, 131319–131345] | JSON beat times and optional BPM automation/phrases/energy information. A structure key can contain `/`, creating further subdirectories. |
| Project arrangement | `projects-table`, `projects-meta-table`, `mix-data-table`, `automation-data-table` [D:131024–131065, 129860–130100] | Mix order, transitions, cuts, per-instance cue overrides, video/effects. Preserve as opaque; not a playlist schema. |
| Derived aggregate cache | `<root>/Cache/Database/<table-folder>.json` [N:113560–113699] | Cached whole-table JSON; stale entries can hide correct edits. |
| Audio/image/stems blobs | `audio-library-*` field stores, image stores, `Cache/Images`, `Cache/Stems` [D:74060–74085; N:114123–114138] | Preserve untouched; metadata edits do not authorize audio/cache regeneration or deletion. |
| Browser state | Electron IndexedDB/Local Storage and web renderer deployment cache [D:130917–131024; N:114114–114121] | Browser fallback/migration/settings, not automatically the authoritative desktop library. Do not edit Chromium LevelDB files. |

**Seen [D:130917–130935, 130998–131024]** Desktop availability chooses filesystem storage (`S7` using `SDe` when the channel helper exists, otherwise `xDe`). Browser fallback uses IndexedDB; a separate file API implementation `vDe` uses `navigator.storage.getDirectory()` and File System Access handles. The desktop path is native filesystem persistence, not merely a browser IndexedDB database.

**Seen, public corroboration:** DJ.Studio documents a configurable local database folder and original-file references for newly added tracks since 4.1. Its guidance describes copying the whole data folder for backup. [File storage and database management](https://help.dj.studio/en/articles/12315405-file-storage-and-database-management-in-dj-studio). Current help pages describe newer library features as well; use the pinned artifacts for the 4.2.9 schema. The current [Music Library help](https://help.dj.studio/en/articles/8609490-music-library) has both original-file and older copy wording, so it cannot resolve version-specific behavior alone.

## SQLite schema and evolution

**Seen:** `fixtures/djstudio-sqlite-schema.json` records schema metadata only: columns, declared types, nullability, defaults, primary keys, unique/index columns and foreign keys. It contains no library values. The copied database had all ten migration names, `user_version=0`, `application_id=0`, and DELETE journal mode. Its `schema_version=26` is diagnostic, not an application-version marker. Read actual PRAGMAs for every target; do not assume DELETE mode universally.

| Table | Schema and semantics established by migrations/native queries |
|---|---|
| `Tracks` | AUTOINCREMENT integer `id`; `filePath TEXT NOT NULL UNIQUE`; required `fileName`, `relativePath`, `title`, `artist`. Nullable `fileSize`, `fileType`, `bitrate`, `bpm`, `duration`, `key`, `genre`, `album`, `isrc`, `releaseDate`, `addedDate`, `rating`, `sampleRate`, `label`; `analyzed INTEGER DEFAULT 0`. |
| `Playlists` | AUTOINCREMENT `id`, required `title`, required `sort INTEGER DEFAULT 0`. No parent/folder/type column. |
| `PlaylistTracks` | AUTOINCREMENT membership `id`, required `playlistId`, `trackId`, nullable `sort INTEGER DEFAULT 0`; FKs to playlists/tracks with default NO ACTION. No uniqueness on `(playlistId,trackId)`: duplicates are representable. |
| `Folders` | AUTOINCREMENT `id`, required `name`, unique required `path`, required `sort DEFAULT 0`. Bookmarks into the filesystem; no playlist-parent relationship. |
| `Libraries` | AUTOINCREMENT `id`, unique required `type TEXT`, nullable `lastSynced TEXT`. |
| `LibraryLocations` | AUTOINCREMENT `id`, required `libraryId` FK, required `location`. |
| `TrackLibraryMap` | AUTOINCREMENT `id`, required `libraryId` and `trackId` FKs, required external `identifier TEXT`; nullable `bpm`, `key`; unique `(libraryId,trackId)`. External IDs are opaque strings. |
| `migrations` | AUTOINCREMENT `id`, required `name`, `executed_at DATETIME DEFAULT CURRENT_TIMESTAMP`. Application migration-name ledger. |

Anchors: `public/migrations/001_create_folders_table.js`, `002_create_tracks_table.js`, `003_create_playlists_tables.js`, `004_add_tracks_samplerate.js`, `005_playlist_tracks_sort.js`, `006_add_tracks_label.js`, `007_add_tracks_relative_path.js`, `008_create_trackmap_libraries.js`, `009_clean_tracks_table.js`, `010_add_folders_path_unique.js`; N `wc`, `_c`, `Vc.initializeRepository` at 142515–143520.

**Seen:** migration 008 rebuilds Tracks with unique paths, creates external library maps, and does not recreate the old Tracks filename/artist/title indexes in that replacement. Migration 009 deletes tracks and external mappings that were not in native playlists at migration time. Migration 010 removes duplicate folder paths, keeps the smallest ID, and recreates path/name indexes. Do not run migration files opportunistically, insert fake migration rows, or rewrite version PRAGMAs. Require a complete recognized migration/column/index/FK fingerprint; future schemas stay read-only.

**Seen [N:142676–142709]** Native metadata update uses COALESCE on most fields: missing/null means retain old value. It always binds analyzed as 1 only when the input value is exactly 1, otherwise 0. A supposedly partial native update can therefore reset analysis status. Independent adapter writes should use an explicit owned-field patch and preserve everything else, including `analyzed`. SQL NULL is not an empty string; explicit clearing needs a separate change operation.

**Seen [S:337–364, `uo` / `saveAudioFileRecordChanges`; D:173327–173350, 183015–183038]** Imported-track details edit JSON tag title/artist/genre/comment/label and record rating. Explorer analysis/rating actions instead invoke native `library.V2.updateTracks` for SQL rows. These are distinct stores with separate UI paths; SQL is not a disposable copy of every imported-track JSON field. No automatic bidirectional JSON-to-SQL metadata mirror is established by these paths. **Inferred:** track the observed values/baselines for each surface independently, join by unique path for a unified view, and write to both only when an explicit plan chooses both and resolves divergences. Do not erase an Explorer-only record because no imported JSON exists, or create a fabricated analyzed record merely to mirror an index row.

**Seen [D:94966–94995, `changeTracksBpm`; S:355–357]** The imported-track details BPM action changes scalar BPM, loads audio, regenerates its beat data and resets the selected timeline map. It is not a pure scalar update. Do not invoke or imitate that action for the shared contract's BPM-only import/update policy: preserve the existing grid unless grid replacement is separately selected. Runtime reconciliation of an offline scalar-only JSON edit remains a gate.

**Seen [N:142812–142900]** External sync upserts by file path, preserves existing non-null BPM/key in Tracks, and stores source BPM/key in TrackLibraryMap. A full source sync first deletes that source's existing maps inside a transaction; partial sync need not. It updates `Libraries.lastSynced` after that transaction. Do not overwrite external maps when modifying only DJ.Studio-owned metadata; do not fabricate external provenance for CueRithm.

**Seen [N:142595–142639, 143225–143267]** Normalized paths and relative paths participate in matching. Backslashes normalize to `/`; remove an initial Windows drive prefix or `/Volumes/<volume>/` for the relative path; trim leading/trailing whitespace on native add/lookup. Track IDs and memberships are integers scoped to this database. SQL ID is not an imported AudioFileRecord key. Relative path and filename fallback can be ambiguous: require uniqueness or report a conflict; do not merge tracks by basename.

### Tags, numeric representations and ordering

**Seen [N:143225–143267, 143675–143719; D:14846–14862, 129522–129536]** SQL metadata maps title/artist/album/genre/label/ISRC/rating/BPM/key/duration to library results. JSON `tag` has string fields genre, artist, album, track, title, year, bpm, key, comment, releaseYear, label and boolean premium. JSON also has separate effective BPM/key/energy fields. A SQL row has no comment, track-number, cue, grid, waveform or color column. Preserve representations separately and report where a field cannot be represented.

**Seen:** SQLite declares BPM and duration INTEGER, but read-only inspection found both REAL and INTEGER storage classes in those columns. Preserve finite fractional BPM/duration; do not truncate based on declared affinity. JSON BPM is a float-backed type; the renderer shows one decimal. **Seen [D:10681–10724, N:142480–142510]** Time/duration uses seconds; added/release dates in SQL are Unix seconds serialized as text, while JSON Date fields serialize dates. Native release-year conversion uses February 1 (`Date(year,1,1)`), not January 1. Do not rewrite that quirk during a metadata-only sync.

**Seen [N:142546–142581]** Folders sort ascending by `sort`; rearrangement writes zero-based array indexes. Native `getPlayLists` and `getTracksByPlayListId` have no ORDER BY; native membership insertion does not set `sort`, despite migration 005 adding it. **Unknown:** actual UI preservation of playlist order and duplicate memberships. Read `sort`, membership ID and observed row order as separate evidence. A deterministic importer can choose `(sort,id)` as an explicitly inferred fallback, but must not claim that it is verified DJ.Studio order. Writing `sort` alone does not prove round-trip order; gate order support on a runtime test. No nested native playlist folders are modeled here. Do not synthesize nested playlists by misusing Folders bookmarks or project `folderKey`.

## Imported-track JSON and beat structures

**Seen [D:73998–74042, 74058–74086, 74398–74448, 128312–128345]** Table records are `JSON.stringify` text, one file per key, no file extension. Table store name is `<binding>-table`; blob fields have distinct stores. Dots in store names become `-`. With `hashDirForKey`, shard is the sum of JavaScript UTF-16 code units in the full key modulo 256, rendered in lowercase hexadecimal without zero padding. Add `<shard>/` before the unmodified key; slashes in a key form directories. The ASCII-only synthetic fixture has path examples. All inspected audio/structure record paths matched this derivation. Reject traversal (`..` components), absolute keys, NUL, symlinks escaping the root and unexpected key syntax before creating paths; preserve existing valid keys without renaming.

**Seen [D:129606–129674]** An AudioFileRecord stores stable key/name/kind/type/fileHash/size/path metadata; string tags; `bpm`, `camelotKey`, `originalCamelotKey`, `noteKey`, `mikKey`; `duration`, `sampleRate`, `rating`, energy fields; `externalRec`; analysis-version fields; `structureKey`; `beatQuantize`, `beatsPerBar`, `bpmLine`, `beatGrids`, `beatDataSource`, `bpmMultiplier`; source/cleaned-audio relationships and blob references. Preserve every unowned field and its JSON type. Do not relabel arbitrary audio as analyzed or invent provenance version values.

**Seen [D:94438–94467, 94620–94662, 68453–68458]** Ordinary file import derives `file_<SHA256>` from the selected decoded/processed audio byte buffer, not reliably the original container bytes or a path hash. Jingles use `jingle_`; streaming/external records have other identities. `externalRec` contains `UUID`, `fileLocationPath`, `lastKnownFilePath`; setup stores the import hash in UUID for ordinary files. Decoder/transcode/MP3 cleaning branches change the hash input. **Inferred:** update existing imported records by retained record key and unique path; new fully analyzed record insertion cannot safely emulate the identity algorithm without reproducing and validating that processing chain. Never derive an imported key from the native SQL integer ID.

**Seen [D:129559–129605, 131319–131345]** Track structure records contain `key`, `beatData`, `bpmLine`, `phraseData`, `energyLevelData`. Beat entries have `nr`, `time`, `originalTime`, `type`, optional/runtime phrase and energy associations. Grid info contains source, madeOn, BPM summary, division counts, anchor indexes, map type, structure key, multiplier and custom BPM. `addOrUpdateBeatGrid` uses `<audio-key>/<lowercase-source>` for the structure record, updates the grid info reference and writes beat data/BPM line. Base `structureKey`, selected `beatDataSource` and `beatGrids[*].trackStructureKey` can refer to different representations; read all before deciding which is effective. Do not replace only scalar BPM when the user requested a different grid.

**Seen [D:130792–130799]** Beat types are normal=0, user=1, anchor=2, end=65536, lead=65537, nosync=65538. Preserve sentinel/tail/anchor beats. **Unknown:** every beatMapType/flags variant and UI constraints on manually authored grids. A reader may preserve unknown enum values; a writer must reject unsupported variants rather than flatten them into a fixed grid.

### Audio codecs and source locations

**Seen [N:142416–142444, `dc` / `pc.isSupportedFileType`]** Native local-file enumeration accepts wav/wave, mp3, m4a/mp4, 3gp, m4b/m4p/m4r/m4v, ogg/ogv/oga/ogx/ogm/spx, opus, webm, flac, aiff/aif, alac, aac and wma extensions. This is an enumeration filter, not proof of successful decoding or exact timing for every codec. SQL `fileType` is an INTEGER-declared field but was null in inspected rows; its enum is **Unknown**. JSON `type` is MIME-like and `kind` identifies file versus integrations; preserve unknown types. DRM/streaming service references do not imply a directly writable local audio file. Read `externalRec.fileLocationPath` and `lastKnownFilePath` as recorded paths, and resolve without changing them. No source music was opened or modified for this research.

**Seen [D:94438–94565]** New file identity uses processed audio selected by native decoder/legacy transcode branches; MP3 can undergo frame cleaning. **Unknown:** exact decoded byte agreement across versions/codecs, gapless/encoder-delay conventions and full analysis compatibility. The adapter can retain existing file/structure IDs without redecoding, but must not claim full new-file analysis import from a synthetic JSON row.

### Keys and cue positions

**Seen [N:109931–110163, D:15501–15570]** Numeric key values are not Camelot wheel order: 0–11 are C, Db, D, Eb, E, F, F#, G, Ab, A, Bb, B major; 12–23 are Am, Bbm, Bm, Cm, Dbm, Dm, Ebm, Em, Fm, F#m, Gm, Abm. `fixtures/djstudio-key-map.json` gives exact Camelot/OpenKey mappings. Native parser returns -1 for an unknown key; renderer enum additionally has X=-2. Use canonical numeric values for writes and explicit unknown handling. Keep original-analysis and effective/overridden key fields separate. Native textual aliases include an anomalous D# Minor alias in the F#-major entry; prefer the numeric table instead of duplicating that parser quirk.

**Seen [D:129505–129521, 15597–15606]** A cue is `{beat,time,length,type,name,cueKey}`. `beat` is a floating position in the beat map, with -1 default/unset; `time` has seconds semantics and -1 default/unset; `length` is floating; `type` is source/provenance, not hotcue-vs-memory. Cue groups are `systemCuePoints`, `hotCuePoints`, `memCuePoints`, plus loopMode. Type values: DJ.Studio=0, rekordbox=1, Mixed In Key=2, Serato=3, jingle=4, phrase=5, removed=999. No cue RGB/color field appears in this model. Do not interpret type as a color, pad number, cue kind, or energy label.

**Seen [D:16125–16136, 89577–89587, 94384–94394]** Imported rekordbox/Serato cue times are converted to the index of the nearest existing beat entry; a leading `{beat:-1}` can be inserted. Matching stops after passing the input time and uses a four-second initial distance bound. Type/source and name are retained; imported `length` is set to zero. Consequently this observed import path quantizes cue placement and does not prove sample-accurate arbitrary-time preservation. Read and retain both beat and time values. **Seen [D:83430–83439, `getTrackBeatTime`]** The inspected track-time getter resolves an integer beat array entry directly; negative indexes extrapolate with the first interval; missing positive entries fall back to track duration. It has no fractional interpolation branch. A canonical decoder can resolve integer beat cues to the selected structure's seconds exactly; unresolved/fractional entries must carry an issue and original raw fields, rather than inventing a smooth time. Resolve beat positions against the active structure; do not interpret `beat` as milliseconds or assume `beat * 60/BPM` handles variable grids, negative lead-in or anchors. Fractional beat interpolation and loops must pass dedicated runtime fixtures before writing them.

**Seen [D:81641–81656, 82464–82503]** Project instances initialize/copy cues from the library, then retain separate mix cue groups; explicit reset functions copy beat/length/type/name back from library cues. Updating an AudioFileRecord does not establish automatic propagation into existing project instances. System cues are transition/arrangement anchors; preserve them during hotcue-only synchronization. Existing cueKey identities may link other project objects [D:13743–13887]; do not regenerate existing keys or touch project links during ordinary library sync.

**Seen [H:19966–19995]** Memory-cue loop edge dragging adjusts `length` by rounded deltas in beat coordinates, establishing beat units for that editor representation. Convert a loop end using its mapped end beat only when the relevant grid/index is valid; do not use `start_seconds + length` or assume a constant BPM. **Unknown:** complete cue pad ordering/capacity, every loop mode/flag and boundary behavior, arbitrary absolute-time precedence, and persistence of disabled/removed slots through reopen. Color is unrepresentable in the inspected CuePointRecord schema: retain imported colors in CueRithm's private source observation and report a loss if asked to write them to this DJ.Studio profile. Do not invent a JSON color property or encode colors as cue types. Static code pads the track timeline's hotcue list through index 11 and creates a default cue when empty; that is not proof that all twelve positions map to external pads identically. Preserve array order, holes/sentinels and unknown types; do not sort the JSON cue arrays by time. Treat new cue insertion as gated until the synthetic cue matrix is opened and inspected.

### Canonical clock and independent field selections

**Inferred:** JSON/SQL seconds and beat-array lookups identify DJ.Studio's native timeline. They do not establish alignment to CueRithm's FFmpeg PCM for every codec or processed-audio variant. Follow `import-and-sync-contract.md`: retain original beat/time/length plus the selected structure and native seconds; use `unresolved_native` until a native/renderer/codec/decoder profile is qualified. Keep library source observations distinct from project-local cue overrides even when they share a path.

BPM-only and grid-only selections must remain independent. Do not invoke a native UI BPM helper that regenerates beat data for a scalar-only operation. A grid write must not alter scalar BPM unless the user independently selected it; if that combination cannot be represented coherently in the target, surface the conflict and preserve the target. Cue RGB has no demonstrated field in CuePointRecord: report the write loss instead of inventing JSON properties.

## Native/browser boundary and internal APIs

**Seen [P, symbol `library.V2`; N:105825–105943, `ae.registerApiHandlers`; N:109892–109924, `AddHandler`]** Preload methods invoke Electron IPC channels, registered with `ipcMain.handle`. Generic library methods route by integration type, with native DJ.Studio using `djstudio-local`. Representative signatures after dropping the IPC event argument:

| Route | Arguments / persistence reached |
|---|---|
| `library/v2/get-all-tracks` | integration plus optional query options; native repository rows |
| `library/v2/get-tracks` | integration, playlist ID, optional parameters |
| `library/v2/update-tracks` | integration, row patch array; native COALESCE updater |
| `library/v2/add-tracks` | integration, local path array; stat files then insert index rows |
| `library/v2/create-playlist` | integration, title, local file paths |
| `library/v2/add-tracks-to-playlist` | integration, playlist ID, local file paths |
| `library/v2/remove-tracks-from-playlist` | integration, playlist ID, row ID objects |
| `library/v2/get-folders`, `add-folder`, `remove-folder`, `rearrange-folders` | integration plus path/order inputs; Folders bookmarks |
| `library/v2/get-hotcues` | generic route; DJ.Studio-specific implementation throws |
| `database-file-store` | relative/absolute database filename and serialized content; native file writer |
| `database/folder/get-all-content` | table directory, allowCache; aggregates JSON records |
| `database/cache/file/delete` | aggregate cache basename; deletes derived table JSON |
| `channel/get-config` | returns root/database/cache paths and environment configuration |

**Seen [D:128312–128345, 128489–128523; N:114575–114590, 115580–115650]** Desktop record stores serialize JSON and invoke preload `database.storeFile`, then invalidate the table aggregate cache. Native storage truncates/opens the destination for writing and may datasync; it does not provide an observed cross-record transaction or multi-file atomic replace. A successful IPC return is not proof of consistent library/project state.

**Unknown:** a supported external automation protocol for invoking these methods. Electron renderer IPC and its MessageChannel optimization are in-process capabilities, not an HTTP endpoint, CLI, SDK or contractual public API. Do not implement guessed localhost requests or load DJ.Studio JS into CueRithm. The researched standalone route is version-gated offline persistence with explicit backup and rollback.

## Proposed offline write protocol (Inferred, not vendor behavior)

1. Identify the user-selected root, native+renderer versions, migration fingerprint and record-profile shapes. Require DJ.Studio and its helpers to be fully closed; verify again immediately before commit. Refuse live writes and network/cloud-synchronized roots. Use an adapter-owned exclusive lock to exclude another CueRithm operation; SQLite's lock alone cannot protect JSON from DJ.Studio's independent writers.
2. Snapshot the complete participating persistence set into a private backup: studio.db plus any journal/WAL/SHM and referenced JSON/affected cache files. A file copy made while a writer is active is not necessarily consistent; an active app means stop. For a read-only online inspection use SQLite backup/snapshot semantics and a coherent JSON inventory, or report a potentially mixed snapshot. Do not checkpoint the live database as an inspection side effect.
3. Build a concrete change manifest: stable IDs, normalized paths, before/after hashes, exact owned-field patches, unresolved conflicts and untouched project hashes. Preserve all unknown columns, JSON properties, blobs, array ordering, source maps and analysis provenance. Recheck on-disk hashes/stat metadata before applying; abort on divergence.
4. Stage a standalone SQLite copy in the same filesystem. Open with foreign keys ON and a bounded busy timeout; `BEGIN IMMEDIATE`; use parameterized updates by stable row ID and expected before-values; allocate new integer IDs through SQLite rather than MAX+1. Create memberships only for existing planned track/playlist IDs, remove memberships before deleting parents, and maintain duplicates/order deliberately. Run `foreign_key_check`, `integrity_check` and adapter invariants before COMMIT. No migrations, PRAGMA version edits, media writes or external-map rewrites.
5. Stage changed JSON files as strict UTF-8 JSON with finite numbers, preserved unknown values and valid stable references. Serialize to sibling temporary files, fsync file data, and atomically rename per file with platform-appropriate replacement APIs. Do not call multi-file rename atomic: it is not. A CueRithm transaction journal, stored outside DJ.Studio's recursively scanned table directories, records prepared/backed-up/replaced/validated states and original hashes. Do not place `.tmp`, manifest or backup JSON files inside a table directory: its scanner reads files recursively, not only known extensions.
6. With the app still closed, fsync the prepared journal, replace staged files and database in the journaled order, fsync parent directories, and mark committed only after read-back validation. For a WAL target, require a coherent standalone staged database and no active connection/sidecars before replacement; never replace only the main file under live WAL state. There is no demonstrated vendor journal spanning SQLite and JSON.
7. Invalidate only affected aggregate files such as `Cache/Database/audio-library-table.json` and `Cache/Database/track-structures-table.json` when they exist; retain a private recoverable copy in the transaction backup. Clear neither the entire Cache directory nor analysis/audio/image stores. Read-back must bypass aggregate caches to verify canonical records, then verify rebuild after next approved app open.
8. On any failure or interrupted-journal recovery, keep DJ.Studio closed, restore every replaced persistence object and original cache bytes from the backup, verify original hashes and SQLite integrity, and persist rollback status. Never claim partial success or launch DJ.Studio over a half-applied set. Idempotency keys and before/after hashes allow safe retry without duplicated playlist membership.
9. After adapter commit, reopen the exact supported DJ.Studio build in a disposable test root for acceptance. Record its read-back state and any unexpected rewrites. User-library deployment remains disabled for unsupported field/profile gates. Preserve private transaction evidence and present a concise sync result; do not publish row data or local paths.

Write scope by operation:

| Requested operation | Planned persistence scope | Gate / limitation |
|---|---|---|
| Existing Explorer metadata | Owned Tracks columns | SQL read-back feasible; app reopen required. Fractional BPM and key table verified statically. |
| Existing imported-track tags/effective key | Matched AudioFileRecord tag/effective fields; optionally matched Tracks metadata when both views must agree; aggregate cache | Preserve original/source analysis and project overrides. Field-precedence UI test required. |
| BPM scalar | Both owned scalar fields and explicit handling of existing grid/automation | Scalar edit alone cannot claim grid synchronization. Require grid-consistency policy. |
| Existing cue edit | AudioFileRecord cue group, existing key/array slots; relevant aggregate cache | Must pass pad/sentinel/beat/time/source test. Native SQL has no cues. |
| Grid edit | Owned structure record plus AudioFileRecord selected references/grid info/BPM line; scalar BPM only when independently selected; aggregates | Unsupported sources/flags, variable/fractional timing and projects need explicit acceptance. Preserve alternate grids. If the app requires a scalar BPM change when grid alone is selected, preserve it and report an unrepresentable combination until the user explicitly selects BPM too. |
| Native playlist create/rename/membership | Playlists/PlaylistTracks; new index Tracks only if required and representable | Order/duplicates need runtime acceptance; nested folders/smart playlists unknown for this profile. |
| Explorer folder bookmark | Folders path/name/sort only | Does not create/move filesystem folders or nest playlists. |
| New fully analyzed imported track | New AudioFileRecord/structures/blobs/identity/provenance as required | Not enabled by this research: processed-byte hashing/decoder behavior and first-open reconciliation not validated. Index insertion alone is not full import. |
| Existing project arrangements | None for library sync | Separate migration feature; do not mutate mix-data/projects/automations/video/effect records. |

## Acceptance gates and synthetic fixtures

The fixtures are clean-room synthetic contracts, not app-generated exports. `djstudio-synthetic-records.json` contains deliberately partial metadata for merge/update tests; never install it as a complete DJ.Studio database. `djstudio-sqlite-schema.json` is a schema manifest and `djstudio-key-map.json` is the numeric key mapping. They include no real audio, private filenames, credentials, library rows or proprietary code.

Research-only validation completed: all supplied JSON fixtures parsed; the 24-key mapping was bijective; an in-memory SQLite database reconstructed from the schema manifest accepted the synthetic records, retained REAL BPM under INTEGER affinity, retained duplicate memberships, passed integrity/FK checks, and rolled back a metadata transaction. Sharding recomputation matched every copied audio/structure JSON path. Additional isolated shipped-code buffer controls executed only the copied `S7` sharding class against an in-memory storage stub and the copied `o6` nearest-beat function in a Node VM. Fixture ASCII paths, nested structure keys, dotted store names and Unicode UTF-16 surrogate sharding matched; exact/nearest/tie cue cases and out-of-range four-second-bound behavior matched. No filesystem writer, IPC, decoder, authentication or app initializer was executed in those controls. The synthetic fixture records the input/output cases. These checks validate fixture consistency and those isolated algorithms, not DJ.Studio execution. The source database copy also passed SQLite integrity checking; live-writer quiescence and runtime reopen were not established, so no snapshot consistency or writer-acceptance claim follows.

Required before claiming support:

- Read-only parity: compare native Explorer rows, imported library rows, keys/BPM/duration/tags, native memberships, bookmarks and all referenced structures with the exact app UI. Count unmatched and ambiguous file identities; do not silently drop streaming/cloud or missing-media rows.
- Metadata precedence: independently edit title/artist/comment/rating/BPM/key in each surface, reopen, then reload/reanalyze only in a disposable root. Record which JSON/SQL values the UI reads and what it overwrites. Confirm scalar updates do not reset analyzed or original key provenance.
- Keys: all 24 numeric mappings, unknown -1, renderer X=-2, Camelot/OpenKey/note display, major/minor aliases; exact numeric round trip.
- Timing: a generated original test WAV and a compressed MP3/AAC with known encoder delay; fixed and variable grids, fractional BPM, nonzero first beat, negative lead-in, every sentinel/anchor type. Compare original-audio seconds and visible/playback cue positions. No sample-accuracy claim from static code.
- Cues: hot/memory/system separately, empty/default/sentinel slots, first/last padded slot, duplicate times, names, loops, removed/source types and stable cueKey links. Verify edit and reopen without modifying any existing project arrangement; separately test that an existing project retains its intentional overrides.
- Playlists: ordered synthetic A/B/A membership with duplicate IDs, empty playlists, rename, membership removal, missing-file members; compare actual UI order. Native retrieval lacks ORDER BY, so disable order write claims until proven. Verify bookmarks independently from playlists/project folders.
- Atomicity: inject failure after every replace/SQLite-commit/journal transition on disposable copies, recover original hashes and FKs; retry with the same operation ID without duplicates. Refuse writes while DJ.Studio holds any relevant root open; handle a concurrent modification between plan and commit.
- Version/profile drift: unknown migration, new field/type, native/renderer mismatch, missing target JSON, duplicate path and malformed/trailing JSON all produce safe read-only/conflict behavior. JSON parsers in the app sometimes recover trailing junk; the adapter should report malformed input and preserve it unchanged.
- Reopen reconciliation: verify canonical records survive opening, browsing, cache regeneration, project load and ordinary app shutdown. Record exact native/renderer identities and before/after hashes. No runtime gate was executed during this handover.
