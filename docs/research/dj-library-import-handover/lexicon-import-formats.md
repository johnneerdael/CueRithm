# Lexicon 1.11.15 import formats: clean-room engineering reference

Research date: 2026-10-23. Target: installed macOS arm64 Lexicon 1.11.15. Scope: all eleven local DJ-library/playlist import choices listed on the import manual. No CueRithm implementation changes, live-library imports, music-tag writes, sign-in, or credential inspection were performed. Source copies and disassembly remain outside the repository. This document contains behavioral descriptions, factual format layouts and synthetic data; no shipped source or disassembly is reproduced.

**Seen** means directly observed in the copied bundle or explicitly stated on the individually linked manual page. **Inferred** marks a clean-room design implication. **Unknown** marks an unresolved acceptance or compatibility boundary. Static source inspection establishes implementation intent, not successful operation in the corresponding DJ application.

## Evidence identity and reproducible anchors

The app's `package.json` identifies 1.11.15. The source ASAR was copied to a fresh `$TMPDIR/re/lexicon-imports-ay2l7eom/` and extracted there, including a separate copy of its unpacked resources. No user library was copied or read.

| Evidence | SHA-256 |
|---|---|
| `Contents/Resources/app.asar` | `85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860` |
| extracted `main/index.js` | `ded24dd800a42af44b88beff9dc9ff788a16e57bcd00ac59ec4ccb43d35ec676` |
| extracted `renderer/main_window/index.js` | `df240bbd3677166bda1f36d00212c49b03740038c52fc17fbb8b7cf647b2e198` |
| `main.pretty.js` | `21262d6d721783d9dc40eb69d721ffaac83f7eaf1e8c99ad34ab5191f9d4bdf2` |
| `renderer.pretty.js` | `032bb911711c2ed0da24ce67fad2187fa456faf58ec65f0379c03fb9ef1397e3` |
| copied `lib/lexicon-tagger/lexicon-tagger` | `7cec7bc48078591d395f46b605d2108a2fef64b31f052eefb6c4a3d3ee000d6f` |
| extracted tagger `PYZ-00.pyz` | `5ee42fb0111e559b8d32bbc02b095cdd20c05838a60a026114c2f4efd699eec4` |

`R:line` below refers to `renderer.pretty.js`, produced by Prettier's Babel parser with default settings from the exact renderer hash above. `M:line` refers to the similarly formatted main bundle. These generated copies are intentionally not committed. Symbols and webpack module numbers accompany important ranges so an engineer can relocate them if formatting differs. Python anchors refer to original `co_filename`, qualified code-object name and `co_firstlineno` retained in the copied tagger's Python 3.12 archive, not reconstructed source lines. Extraction reads the PyInstaller CArchive cookie, its `PYZ-00.pyz` entry, the PYZ marshal TOC and zlib-compressed code objects. This was sufficient for buffer-codec inspection; no decompiler or app launch was needed.

## Shared import contract

**Seen, R:266578–267099, `BaseImporter`, module 18497:** Full and Playlist imports use the same batch pipeline. Batch size is 1,000. Merge updates an existing track selected by normalized location; streaming tracks can additionally resolve by service and external ID. Duplicate normalized paths within a batch collapse to one library track while retaining mapping aliases for playlist resolution. A source ID is an adapter-local identifier, not a universal track identity.

**Seen:** With Merge off, track and playlist repositories are cleared before import. With Merge on, the `specificFields_` settings determine updates. The selectable fields are Title, Artist, Album, BPM, Key, Genre, Comment, Rating, Remixer, Label, Composer, Grouping, Year, Lyricist, Track Number, Cue Points, Beatgrid/tempo markers, Color, Play Count and My Tags. All default on (`R:395947–396042`). BPM and beatgrid selection are independent. New imports still need source parsing; selecting fewer updates does not turn another format into a cue-only file format. This is Lexicon's observed update policy; CueRithm's proposed policy also gates publication for newly created tracks and keeps unselected parsed fields in provenance.

**Seen:** Playlist merge identity is trimmed name and compatible type within the same parent folder. Membership is saved from the imported content; a playlist moved to a different parent becomes a separate node. Smartlist-to-normal-playlist replacement is deliberately skipped in one merge case. Cue and grid writes have separate gates; a partial-field merge must preserve unselected existing values, including cues imported from another app. History imports use separate source-scoped unique IDs.

**Seen:** Settings default Merge on and backup on. Rekordbox desktop defaults to hot cues, duplicate cues off, original cloud locations off. USB defaults to both cue classes. Engine hidden/missing-playlist troubleshooting defaults off (`R:395855–395946`). Plan restrictions can force Full and Merge off; these are Lexicon commercial rules, not file-format requirements. [Import options](https://www.lexicondj.com/manual/import-options).

**Inferred handover contract (subject to the pack's shared publication policy):** Parse to a staging model before mutation; retain original source IDs, original paths, exact raw fields and an adapter/version identifier. Report unavailable files, unsupported fields and dropped duplicates separately. Do not identify tracks by basename alone except as an explicit, reviewable USB reconciliation strategy. Preserve playlist entries as ordered lists in CueRithm; Lexicon often uses Sets and loses duplicate membership. Retain folder-track membership even if the UI needs a synthetic `_FolderTracks` child.


## Native time units versus CueRithm canonical playback clock

**Inferred integration requirement from current CueRithm timing policy:** Every seconds value in this document is a **native source timeline** after unit conversion. Converting milliseconds/microseconds/samples to seconds does not establish alignment with CueRithm's canonical FFmpeg-decoded PCM timeline. Keep `native_time`, `native_unit`, `native_sample_rate`, codec/container, source application/build, validated timing-profile identity and conversion state. Invert a validated outbound per-target offset exactly once for inbound cues/grids; if no validated inverse exists, retain them as native-clock unresolved performance data and make the limitation visible. Do not apply an arbitrary constant from Lexicon, a filename extension, or a guessed encoder delay. See the handover pack's central timing policy/current CueRithm inventory for the owning `timing.rs` implementation and supported profiles.

Lexicon also runs a beatshift scanner and applies a source/file-specific adjustment in `BaseImporter.importTrackBatch` (`R:266813–266825`, 266836 onward). The static codec controls here do not validate that higher-level adjustment. Keep source decoding, native-unit conversion and canonical alignment as separate recorded steps.

## Import matrix

| Choice | Primary artifact | Stable source join | Main performance-data location | Boundary |
|---|---|---|---|---|
| Rekordbox 6 & 7 | encrypted SQLite `master.db` | internal `ID`, external `MasterSongID` | `djmdCue`, analysis DAT/PQTZ | Direct only in CueRithm; app-version acceptance remains separately gated |
| Serato 3 | `_Serato_/database V2`, `.crate`, `.scrate` | root-relative `pfil` / `ptrk` | music-file tags, streaming metadata XML | Legacy binary chunks; not Serato 4's crate store |
| Serato 4 | Serato `Library/master.sqlite` | `asset.id`, `space.id`, `location_id` | music-file tags plus SQLite metadata | SQLite alone does not supply cues/grids |
| Engine DJ | `Engine Library/Database2/m.db` | origin DB UUID + origin track ID | `PerformanceData` table/view or legacy Track columns | Mixed-endian, some zlib compressed blobs |
| Traktor Pro 3/4 | `collection.nml` | normalized location / playlist primary key | XML `CUE_V2`, nested `GRID` | Not Traktor DJ |
| VirtualDJ | home and per-volume `database.xml` | `Song@FilePath` | `Poi`, `Scan` | My Lists vs older playlist sources |
| djay Pro | `djay Media Library[.djayMediaLibrary]/MediaLibrary.db` | collection + key/UUID | `database2.data` typed stream or bplist | v5 minimum constant; no device-runtime acceptance established |
| Rekordbox 5 | exported collection XML | `TrackID`, playlist `TRACK@Key` | `POSITION_MARK`, `TEMPO` | Research only; no CueRithm rekordbox XML transport |
| Apple Music/iTunes | exported plist XML | numeric Track ID, persistent playlist ID | ordinary metadata only | Does not provide DJ cues/grids through this importer |
| Pioneer USB/OneLibrary | `PIONEER/rekordbox/exportLibrary.db` or `export.pdb` | database content/track IDs | SQLite cue rows and DAT/EXT tags | Prefer SQLite when present; copy/match audio is separate |
| M3U/M3U8 | text playlist(s) | normalized media location | read media tags after resolving path | Flat membership, no embedded DJ performance model |

The installed dropdown additionally contains Beatport, Spotify and Tidal online import entries (`R:235490–235552`). They require service-specific account/network flows and are outside the eleven local-format import pages requested here; no account state or credentials were inspected.

## Rekordbox 6/7 direct database

**Seen, R:270417–270465, `Rekordbox6`:** Settings live under `~/Library/Application Support/Pioneer/rekordbox6/rekordbox3.settings` on macOS and `%APPDATA%/Pioneer/rekordbox6/rekordbox3.settings` on Windows. The database directory can be overridden by the master-database setting. Filename is `master.db` (`R:271027`). The bundle uses `better-sqlite3-multiple-ciphers`; this research intentionally does not publish database keys or license/account material.

**Seen, R:271660–271745, `buildTrackQuery`:** `djmdContent` LEFT JOINs artist aliases for Artist/Remixer/Composer, plus album, genre, label and key lookup rows. Core fields: `Title`, `Commnt`, `Subtitle` (mix), `BPM` (hundredths), `Length`, `BitRate`, `SampleRate`, `FileSize`, `TrackNo`, `DiscNo`, `Rating`, `DJPlayCount`, `StockDate`, `ReleaseYear`, `ReleaseDate`, `FolderPath`, `OrgFolderPath`, `AnalysisDataPath`, `ServiceID`, `ColorID`, `Lyricist`, `updated_at`. Deletion filter is `rb_local_deleted=0`. `MasterSongID` is exposed as the track key in the common import representation; do not confuse it with relational `ID`.

**Seen, R:271265–271300, `buildLocation`:** Local, Dropbox and Google Drive source paths differ. The original-location option can select `OrgFolderPath`. Google Drive also prefers it when present. Dropbox can prepend the locally configured provider root and translate `/contents_…` under `/rekordbox`; absent provider-root discovery can fall back to original location. Unsupported service/path combinations return no location and are skipped. Paths pass through URI encoding then common decoding/normalization; preserve the source form for diagnostics.

**Seen, R:271463–271620 and 271089–271105:** `djmdCue` supplies `InMsec`, `OutMsec`, `Kind`, `Color`, `ColorTableIndex`, `Comment`, `ActiveLoop`, filtered by content ID and deletion flag. Times divide by 1,000. `OutMsec=-1` is a non-loop sentinel. Kinds 0 and 4 are memory cues; 4 forces active-loop state. Kinds 1–3 map to slots 0–2; kinds 5–17 map to slots 3–15. Other kinds require explicit handling. Color comes from either compact index, table index or a palette approximation. Hot vs memory selection and duplicate-position suppression are separate options. Preserve both cue class and original slot in a clean-room model; collapsing them makes later direct synchronization lossy.

**Seen, R:271487–271586:** The analysis file begins `PMAI`. The big-endian header provides header and total lengths. Iterate tagged sections by section length; `PQTZ` beat records begin at section+24 and are 8 bytes: u16 beat number, u16 tempo in hundredths BPM, u32 milliseconds. Lexicon ignores records before a first beat number 1. It compacts constant grids and introduces segments at tempo/phase discontinuities, using a 10 ms tolerance. This is a normalization choice, not the full fidelity of the beat record array. Retain raw beat rows in CueRithm if exact reconstruction matters.

**Seen, R:271302–271428:** Playlist tree uses `djmdPlaylist.ID`, `ParentID`, `Attribute`, `Seq`; root is the string `root`. Attributes 0=playlist, 1=folder, 4=intelligent playlist, -128=auto-upload playlist. Song links are `djmdSongPlaylist` rows ordered by `TrackNo`, with deleted links excluded. Resolve `ContentID` to content's `MasterSongID`. My Tags join `djmdMyTag` and `djmdSongMyTag`; retain category parent and tag name (`R:271430–271460`). Intelligent-rule conversion is a separate mapper (`Rekordbox6.intelligentPlaylistXmlToSmartlist`, R:270466 onward).

**Unknown:** This static inspection does not expand CueRithm's accepted rekordbox builds. Apply the current source-owned acceptance profiles and runtime checks from the pack's CueRithm inventory. Lexicon's broad 6/7 marketing is not evidence that an arbitrary build can safely be written. [Rekordbox direct import](https://www.lexicondj.com/manual/import-rekordbox-6).

## Serato 3: legacy library, crates and file tags

**Seen, R:273488–273504:** Library files are `_Serato_/database V2`, `Subcrates/*.crate`, `SmartCrates/*.scrate`, `neworder.pref`; nested crate paths use `%%`, smartcrate path metadata can use `≫≫`. Search the home music library and per-volume `_Serato_` stores. The importer does not derive the full performance state from `database V2` alone.

**Seen, R:273036–273092, `fromSeratoUnicodeString` / `rawTrackToMap`:** A binary item has a four-byte ASCII name, u32 big-endian payload length, then payload. Names beginning `t` or `p` decode UTF-16BE code units; `s` reads a big-endian short, `b` a byte, other numeric items a big-endian 32-bit value. `otrk` wraps track items. Bound each item to its parent length; reject truncated lengths and retain unknown IDs rather than scanning unlimited input for a magic string.

**Seen, R:274403–274632, `Serato3DatabaseReader.loadAllTrackPaths`:** Source record fields used here include `pfil` media path, `tadd` Unix seconds, `tbpm`, `trmx`, `tlbl`, `tkey`, `bply`, `bbgl`, and `ttyp`. Paths are resolved from the containing library's volume root. This reader scans large binary chunks, deduplicates lowercase full paths, then asks the tag reader for actual file metadata, cues and grids. Streaming metadata uses service-specific XML files in `_Serato_/Metadata/{Beatport,Beatsource,SoundCloud,Tidal,Spotify,AppleMusic}`; selection follows the source app's enabled service, not every cached service.

**Seen, R:274010–274066, `Serato3Crate.parseCrate`:** Crate `otrk` entries contain `ptrk` UTF-16BE path values. Preserve entry order while resolving against the crate's volume root. `neworder.pref` is UTF-16BE text with `[crate]` entries; missing crates are appended and absent intermediate parents can be synthesized (`R:274317–274389`). A parent crate's own tracks become `_FolderTracks`, excluding child memberships in the v4 variant. Lexicon uses Sets, so duplicate crate memberships collapse.

**Seen, R:274076–274097:** Smartcrate rule sections contain `rart`, `brut` (match-all), and repeated `rurt` records; operators include `cond_con_str`, `cond_not_str`, `cond_is_str`, `cond_isn_str`, `cond_bef_str`, `cond_aft_str`, `cond_greq_uint`, `cond_lseq_uint`, `cond_bef_time`, `cond_aft_time`. Rule keys include `urkt`, `trft`, `trpt`, `urpt`, `trtt`. Observed field IDs: location4, title6, artist7, album8, genre9, BPM15, comment17, grouping19, remixer20, label21, composer22, year23, added-date25, key51, play-count79 (`R:273744–273857`). Unknown rules need opaque preservation plus a visible loss report; materialize membership if a live smart-rule equivalent cannot be represented.

**Seen:** iTunes playlists inside Serato are not part of its crate import; they need the separate iTunes XML import. [Serato import](https://www.lexicondj.com/manual/import-serato).

### Serato performance codecs and tag containers

**Seen:** The copied tagger carries Python modules `serato/markers2.py`, `serato/beatgrid.py`, `serato/autotags.py`, `serato/helpers.py`. These code-object names/line anchors are independent of pretty-printing and identify the layouts below.

| Audio container | Markers2 source | Beatgrid source | Envelope |
|---|---|---|---|
| MP3, AIFF, WAV ID3 | `GEOB:Serato Markers2` | `GEOB:Serato BeatGrid` | GEOB binary data; ID3 parsing must handle its own encapsulation |
| FLAC Vorbis comments | `serato_markers_v2` | `serato_beatgrid` | Base64 data with a named payload prefix |
| M4A/MP4 freeform atoms | `----:com.serato.dj:markersv2` | `----:com.serato.dj:beatgrid` | First atom value, base64/named payload wrapper |
| Ogg Vorbis comments | `serato_markers2` | `serato_beatgrid` | Markers2 direct base64; grid string contains `(position,bpm)` and supplies one marker |

Anchors: `serato/markers2.py:get:454`; `serato/beatgrid.py:get:137`; `serato/helpers.py:parse_base64:71`. The named prefix is ASCII `Serato Markers2` or `Serato BeatGrid` followed by NUL; wrapper decoding takes the following bytes. Base64 decoding tolerates non-alphabet bytes and missing padding (`helpers:b64decode_lenient:15`). Strict decoders should diagnose malformed wrappers rather than silently inventing values.

**Seen, `markers2.py:parse:284`:** ID3-style Markers2 data starts version bytes 1,1. After these bytes, read base64 until NUL/end. Decoded payload again begins 1,1. Ogg's decode=false route uses its already-decoded payload directly. Entries are NUL-terminated UTF-8 names, u32 BE payload length, payload. Empty entry name ends parsing. Zero-length entries skip. Unknown entries are retained by the tagger with name and raw bytes. Do not lose the unknown entry header on round trip.

| Entry | Fixed layout before UTF-8 NUL-terminated name | Units / meaning |
|---|---|---|
| `CUE` | byte padding, u8 slot, u32BE position, byte padding, RGB3, reserved2 | Position milliseconds; zero-based slot; fixed portion 12 bytes |
| `LOOP` | byte padding, u8 index, u32BE start, u32BE end, reserved8, byte flag, bool locked | Start/end milliseconds; fixed portion 20 bytes |
| `COLOR` | padding1 + RGB3 | Track color, 4 bytes total |
| `BPMLOCK` | bool byte | Grid lock flag |
| `CUELOOP` | padding1, u8 slot, u32BE start, u32BE end, padding1, RGB3 | Present in this tagger's newer-format support; compatibility caveat below |

Code-object anchors: `CueEntry:107/load:118`, `LoopEntry:173/load:186`, `ColorEntry:79`, `BpmLockEntry:57`, `CueLoopEntry:141`. Reserved fields are not semantic padding guarantees: preserve original bytes when editing an existing entry. Unknown `FLIP` or future entries must remain opaque when the destination cannot represent them.

**Seen, `beatgrid.py:parse:63`:** Version 1,0; u32BE marker count; each record is 8 bytes. First four bytes are float32BE seconds. For all except the final record, second word is u32BE beats to next marker; for the final record it is float32BE BPM. A trailing footer byte follows. BPM of intermediate segments is `60 * beats_to_next / (next_seconds-current_seconds)`. Reject nonfinite positions/BPM and protect count/length arithmetic. This differs from Markers2 milliseconds. `autotags.py:parse:6` uses version1,1 then three NUL-terminated ASCII float values (BPM, auto gain, gain dB), not a float-array struct.

**Seen, R:273535–273666, `TagReaderTrack.toTrack`:** Serato import sets the Serato branch. Autotags BPM can override ordinary BPM and is integer-parsed there. Cue milliseconds divide by 1,000; grids already contain seconds. Intermediate grid BPM uses the next marker span; zero/invalid BPM is skipped. Loops append after existing cue slots and retain original loop index/locked state in source data. Key, rating, energy, lyricist and ordinary tags arrive through the helper's metadata result, with field mapping applied afterward.

**Seen limitation:** The Python helper recognizes `CUELOOP`, but this JS converter filters only its Cue/Loop enum values; original helper recognition does not prove full import of the newer entry. Treat Serato 5 as outside the requested 3/4 compatibility boundary. The legacy empty-cues/loop-only slot expression also deserves an independent regression fixture rather than imitation.

**Executed synthetic control:** `fixtures/lexicon-serato-codec-vectors.json` was decoded successfully by the extracted original Markers2/BeatGrid parsers in a separate Python3.12 interpreter. The exact generated cue1250ms and loop4000–8000ms and grid1.25s/5.25s values matched. Mutagen was an unused empty import stub; no files or app state were fed to these buffer parsers. This verifies those synthetic codec layouts, not real library or audio-container acceptance.


### Legacy Serato Markers_ boundary

**Seen, inspected read path:** `serato/read.py:get_all_tags:68` calls `serato.markers2.get` at original Python line389; the returned entries are assigned to the `markers2` result. `serato/markers2.py:get:454` looks up `GEOB:Serato Markers2` for MP3/AIFF/WAV, `serato_markers_v2` for FLAC, `----:com.serato.dj:markersv2` for MP4 and `serato_markers2` for Ogg. A missing corresponding key returns an empty list through its KeyError branch. This getter does **not** fall back to `GEOB:Serato Markers_` or the MP4 legacy named payload. The JS importer consumes that Markers2 result (`R:273565–273639`).

A code-object constant inventory found legacy `GEOB:Serato Markers_`/`Serato Markers_` names in the copied MP3/AIFF/MP4 **write_tags** paths, not the selected read path. Their presence must not be presented as legacy import support. **Conclusion for this inspected profile:** a file containing only legacy Markers_ has no decoded cue source through this reader. Preserve the raw legacy frame/atom as unselected opaque provenance and emit `unsupported_legacy_serato_markers`; do not let an empty helper result erase existing cues. This handover makes no claim about legacy Markers_ decoding elsewhere in Lexicon or other versions. The Serato synthetic decoder vectors exercise Markers2 only; no legacy codec vector is claimed.

## Serato 4: SQLite metadata plus file performance tags

**Seen, R:274771–274793, `BaseSeratoSqlite.getDatabasePath`:** macOS `~/Library/Application Support/Serato/Library/master.sqlite`; Windows `%LOCALAPPDATA%/Serato/Library/master.sqlite`. Source app must be closed, or acquire a consistent copied SQLite/WAL snapshot. Old `Subcrates` are not the v4 crate source. [Serato 3/4 import choice](https://www.lexicondj.com/manual/import-serato).

**Seen schema, R:275284–275440, module88275:**

| Table | Relevant columns / join |
|---|---|
| `space` | `id,name`; select name `Serato Library` for the main library |
| `space_asset` | `space_id,asset_id`; INNER JOIN asset.id |
| `asset` | `id,location_id,portable_id,file_name,file_size,file_bit_rate,file_sample_rate,format,artist,name,album,genre,bpm,key,year,label,remixer,comments,grouping,composer,rating,emoji,color,time_added,time_modified,dj_play_count,is_beatgrid_locked,length_ms` |
| `connection` | `location_id,database_uri`; derive mounted location root; `root.sqlite` identifies main drive connection |
| `container` | `id,parent_id,name,type,list_order,color,space_id` |
| `location_container` | `id,container_id,location_id` |
| `container_asset` | `asset_id,location_container_id,list_order`; join location_container.id then container_id |
| `smart_crate_rules` | `container_id,version,rules` |
| `alias_container` | optional `container_id,list_order` favorites metadata |
| `container_keep_alive_list_tbl` | optional `container_id,location_id`; orphan filtering |
| `history_session` / `history_entry` | session id/start time; entry asset/session ID, media metadata and start time |

**Seen, R:351559–351819, `SeratoSqliteReader`:** Read crates in main space, identify root container, group by parent_id, sort list_order, walk the tree. Ignore root Apple Music/Streaming Tracks pseudo-containers. Read membership by `container_asset` joined through `location_container`, ordered by list_order. Check optional columns rating/emoji/color and optional tables before selecting them. Keep-alive references filter orphan containers when that table exists; its absence must not result in dropping every crate. Convert only recognized smartcrate rules; preserve originals.

**Seen, R:351401–351471 and R:352142–352220:** Build local paths from mapped `location_id` root plus `portable_id`. If a root is unavailable, try mounted drives; unresolved local locations are skipped. Metadata mapping is direct except `length_ms/1000`, file bit rate `/1000`, Unix timestamps `*1000`, and normalized rating `*5`. The loop intentionally reads media tags again and copies only cuepoints, tempomarkers, mix, energy and lyricist from that result. Main tag metadata remains from SQLite. A SQLite-only backup therefore cannot reproduce cues/grids without the referenced audio or cached streaming XML. Retain asset IDs for membership mapping even when duplicate paths collapse.

**Unknown:** No actual Serato4 database/application was opened. Optional-table compatibility beyond the static branches, path URI edge cases and true source smartcrate predicates require synthetic copied-database tests and an app acceptance sample. Preserve source schema fingerprints; do not assume every v4 build shares identical optional columns.

## Engine DJ

**Seen, R:269873–270059:** Default library is `Music/Engine Library/Database2`; `m.db` is master, `stm.db` streaming, `hm.db` history. Source-drive selection can target desktop/internal, removable drive or Dropbox. Read `Information.uuid` and schema version major/minor/patch. Track identity is origin database UUID plus origin track ID, formatted by Lexicon as `uuid_trackId` (`R:269846`). The local `Track.id` is a different database-local key.

**Seen tables:** `Track`, `PerformanceData`, `Playlist`, `PlaylistEntity`, `Smartlist`, `Historylist`, `HistorylistEntity`. `Track` core mapping includes `artist,title,album,genre,comment,label,composer,remixer,year,fileBytes,length,bitrate,bpmAnalyzed,key,rating,dateAdded,timeLastPlayed,path,originTrackId,originDatabaseUuid,isBeatGridLocked,streamingSource,uri`. `path` is relative to the parent of Database2; non-Dropbox streaming rows select URI. Key is zero-based Lexicon/OpenKey order: add1, odd=>D/even=>M, pair number1..12. Rating0..100 divides by20 and rounds; dates are Unix seconds. `bpmAnalyzed`, not `bpm`, is imported (`R:346391–346505`).

**Seen, R:346306–346338:** LEFT JOIN PerformanceData on track ID and select `beatData`, `quickCues`, `loops`, `thirdPartySourceId`, `activeOnLoadLoops`. `EnginedjDatabaseBase` recognizes a PerformanceData view over legacy Track columns and can flatten it into a table (`R:269428–269475`). Smartlists have an explicit support check: schema major>2 or major2/minor>=21 (`R:269144`).

**Seen source-side effects, R:269284–269534:** The original reader base opens a writable connection, changes journal/synchronous pragmas, normalizes schema, can drop legacy indexes/views/triggers, migrates performance columns from Track, inserts placeholder album art and changes schema2.20.2 to2.20.3. This is why invoking Lexicon's original whole-library importer on a live source was excluded from this research. **Inferred clean-room requirement:** Open a copied snapshot read-only and understand both representations through queries; never reproduce these source mutations as an import prerequisite. No evidence of a separate new performance sidecar codec was found in this path; sqlite WAL/SHM snapshot consistency is still required.

### Engine blob layouts

**Seen, R:269535–269577, `extractBeatData`; R:346001–346098, `extractPerformanceData`:** Compressed beat/cue blobs have a four-byte uncompressed-size prefix, followed by zlib-deflated bytes. Prefix size is u32BE in the writer (`R:269637–269757`). Reader strips those four bytes. Reject decompression bombs by imposing a size and count cap; compare declared/inflated length.

`beatData` inflated structure:

| Offset | Type | Meaning |
|---|---|---|
|0|f64BE|Sample rate, fallback44100 if invalid/nonpositive|
|8|8 bytes|Track sample-length value (writer emits f64BE); reader skips|
|16|u8|Flag; reader skips|
|17|u64BE N|First list count|
|25|N×24 bytes|First list; reader skips|
|25+24N|u64BE M|Second list count|
|33+24N|M×24 bytes|f64LE sample position, i64LE beat index, u32LE beatsTillNext, reserved4|

Divide sample positions by extracted sample rate, not a hardcoded44.1kHz. Reader rejects beat indices outside±100000 and beatsTillNext>100000. The second list is the imported one. Preserve both lists if exact round-trip is needed. Segment BPM is derived from next position and beatsTillNext; a terminal zero-span record is ignored. Negative-index markers undergo a four-beat adjustment in Lexicon (`R:346465–346503`); preserve original indices alongside normalized segments.

`quickCues` inflated structure: u64BE count; repeated u8 UTF-8 name byte length, name bytes, f64BE sample position, alpha/flag byte, R,G,B. Position=-1 is empty; slight negative positions down to -sampleRate/10 are accepted then clamped to0. Slot is record index. The writer reserves eight slots and appends a footer; import reads the declared count. **Inferred caution:** The writer's name sizing uses JS string length; an independent implementation should encode UTF-8 first and validate byte length rather than reproduce that Unicode bug.

`loops` is uncompressed: u64LE count (reader reads low byte then skips7); each record is u8 name byte length + UTF-8 name + f64LE start samples + f64LE end samples + three flag bytes + RGB3. -1 start/end marks empty. Divide by sample rate. Active-on-load state is the bit indexed by original loop slot in `activeOnLoadLoops`. Imported loop positions append after cues; retain source loop index (`R:346447–346463`).

**Seen, R:346100–346292:** Playlists use `parentListId` and reverse `nextListId` chains, with sequence recovery; playlist entities use `nextEntityId` similarly. Do not sort by row ID as the primary order. Membership identity is `(databaseUuid,trackId)`. Entity `membershipReference=0` distinguishes direct folder membership; derived/inherited members must not be reintroduced as new explicit children. Parent membership can be materialized as `_FolderTracks`. Hidden/nonpersisted playlists and explicitly exported IDs affect discovery; troubleshooting can widen this set. Preserve unlinked nodes with an ordering warning instead of silently dropping them.

**Seen:** Public import offers source drive and missing-playlist troubleshooting. [Engine DJ import](https://www.lexicondj.com/manual/import-engine-dj). **Unknown:** No Engine app/device, mixed-database corpus, or new store-layout sample was executed. `fixtures/lexicon-engine-codec-vectors.json` is a 48kHz structural engineering vector with independent struct/zlib checks, not an app-accepted database.

## Traktor Pro 3 and 4

**Seen:** Input is `collection.nml`, auto-detected from versioned Native Instruments configuration or explicitly chosen. XML root `NML`, collection `COLLECTION`, track `ENTRY`, playlists `PLAYLISTS`. The importer sanity check expects a Traktor PROGRAM marker and rejects empty collection; Traktor DJ is excluded. [Traktor import](https://www.lexicondj.com/manual/import-traktor).

**Seen, R:275760–275776:** `LOCATION` combines `VOLUME`, `DIR`, `FILE`, except WEBADDRESS wins for streaming. NML `/:` separators become `/`; double-slash handling differs on macOS for colon filenames. Playlist `PRIMARYKEY@KEY` passes through the same root/volume normalization; nonabsolute macOS paths can be rooted under `/Volumes`. Do not split these keys with a generic URI parser before Traktor's escaping is handled.

**Seen, R:348073–348177:** ENTRY TITLE/ARTIST plus ALBUM TITLE/TRACK, INFO GENRE/COMMENT/LABEL/REMIXER/PRODUCER/KEY_LYRICS/MIX/RATING/PLAYCOUNT/PLAYTIME/FILESIZE, TEMPO BPM, and MUSICAL_KEY VALUE map into track metadata. INFO RANKING0..255 maps floor(value/51) clamped0..5. INFO FILESIZE is KiB (`*1024`), BITRATE is divided by1000, PLAYTIME is seconds. Dates use `YYYY/M/D`; modified time adds seconds since midnight. The implementation assumes sample rate44100 because NML does not provide a normal sample-rate field here. Preserve this as an assumption, not an audio-derived measurement.

**Seen key map:** Traktor VALUE0..23 are chromatic major then minor; the explicit mapper converts to Lexicon/OpenKey internal number. The mapping from Traktor numeric value to internal number1..24 is `[1,15,5,19,9,23,13,3,17,7,21,11,20,10,24,14,4,18,8,22,12,2,16,6]`. Internal odd number=>`((n+1)/2)D`; even=>`(n/2)M`. Keep the original INFO textual KEY separately if present; it is not the preferred key source in this importer. Anchors R:276266–276315 and R:228207–228212.

**Seen, R:348271–348364:** `CUE_V2` START and LEN are milliseconds. HOTCUE is zero-based and -1 denotes unassigned. Beat markers without hotcue are grid-only; numbered beat markers can also appear as cues. Loop requires a nonzero length and end=start+LEN/1000. Cue types Load/Beat/Loop/FadeIn/FadeOut map to distinct colors while original type, display order and repeats are retained. Duplicate cue slots are incremented; unnamed/unassigned cues are appended after maximum slot. Grid BPM comes from nested `GRID@BPM`, falling back to track BPM; invalid/nonpositive segments skip. Do not infer grid from the selected BPM field alone.

**Seen, R:348398–348467:** Recursive NODE/SUBNODES handles folders, playlists and smartlists; playlist entries with Track or Stem primary-key type are eligible. Default internal lists are filtered. Smartlist SEARCH_EXPRESSION QUERY is separately translated. History files are `History/*.nml`, using the same track converter. Unknown XML attributes are collected into source-specific unmapped data (`R:348178–348270`). Preserve that model in CueRithm.

## VirtualDJ

**Seen:** Resolve VirtualDJ home containing `database.xml`; Windows can consult its home setting and users can override `virtualdjHomeFolder`. Search home and external-volume database files, reconcile their volume prefixes, and skip database-internal folder entries or unsupported nonaudio records. [VirtualDJ import](https://www.lexicondj.com/manual/import-virtualdj).

**Seen, R:348755–349051:** `Song@FilePath` identity, `@FileSize` bytes; Tags Author/Title/Album/Genre/Label/Remixer/Composer/TrackNumber/Grouping/Year/Stars/Key/User1/User2/Remix; Comment node text; Infos SongLength seconds, Bitrate, PlayCount, LastPlay/FirstSeen Unix seconds. Key prefers Tags Key over Scan Key. Crucially, `Scan@Bpm` is seconds per beat: normalized BPM=60/value. `Tags@Bpm` is retained as source metadata and is not the primary analysis BPM.

**Seen:** `Poi@Pos` is seconds. Beatgrid POIs use `@Bpm` as actual BPM (not seconds-per-beat), falling back to normalized track BPM. Cue Num is one-based; normalized slot=Num-1. Saved loops with Num=-1 and Slot can be appended after maximum cue. Loop `Size` is beats; end=Pos+Scan.Bpm*Size. Color can come from POI Color; otherwise a slot palette. AutoTrigger=yes marks active loop. Preserve Action, Point, Slot, Phrase, grid Num and original type, plus unknown nodes and attributes. Avoid promoting arbitrary automix/action markers to ordinary cues without the same selection predicates.

**Seen, R:349108–349480 and 349733:** Newer `My Lists` imports its hierarchy plus virtual folders. Older mode chooses Playlists or Virtual Folders; merging two passes can recover both. Physical filesystem browsing is not playlist membership. Legacy `.m3u` playlists, `.vdjfolder` lists/filters and `.subfolders` hierarchy suffixes require separate parsers; filter folders translate only supported rules. Tracks directly inside folder playlists can become `_FolderTracks`. The generic virtual-folder membership parser reads song path attributes; it does not reconstruct full music metadata from the folder file. History M3U uses richer `#EXTVDJ`/`#EXTINF` metadata parsing.

**Unknown:** No live VirtualDJ build/schema migration was exercised. Preserve original POI/Scan blobs and expose loss for dynamic filter predicates that cannot be represented. Relative M3U paths must be resolved deliberately by the adapter; do not assume the legacy helper does that automatically.

## djay Pro

**Seen, R:267637–267644, `Djay`:** Minimum supported version constant5. macOS `Music/djay/djay Media Library.djayMediaLibrary/MediaLibrary.db`; Windows `Music/djay/djay Media Library/MediaLibrary.db`. SQLite table `database2` has `rowid,collection,key,data,metadata`. Key/UUID joins across collections drive the library. [djay import](https://www.lexicondj.com/manual/import-djay).

**Seen collections, R:268360–268374:** `mediaItems`, `localMediaItemLocations`, `globalMediaItemLocations`, `mediaItemAnalyzedData`, `mediaItemUserData`, `mediaItemPlaylists`, `mediaItemPlaylistItems`, `mediaArtists`, `mediaAlbums`, `mediaLabels`, `mediaGenres`, `mediaItemTitleIDs`. Read base media items, then attach same-key user data, local/global URI locations and analyzed data. Playlist-item UUID resolves to its mediaItemUUID; genre/album/label UUIDs resolve via their own collections (`R:267749–268055`). Metadata indexes/views are not the authoritative data records.

### djay data blob codec

**Seen, R:267651–267678 and 268069–268344, `DjayFileReader`:** Two routes: `bplist` Apple binary plist/NSKeyedArchiver, or a custom typed stream with20-byte fixed header followed by class marker0x2B. Numeric payloads are little-endian and aligned relative to the beginning of the blob (u16 to2, u32/float to4, u64/double/date to8). UTF-8 strings are NUL terminated and added to a pointer table. Pointer indexes are single bytes. Type codes:

| Hex | Value |
|---|---|
|08 /05|String / prior string pointer|
|14 /13|float64 / float32, aligned8/4|
|10 /11 /12|u16/u32/u64, aligned2/4/8|
|18|Aligned u32 control/skip value|
|30 /02|Date float64 / prior numeric/date pointer|
|1B,0C,0A,0B,1A|Fixed-size array variants; aligned u32 count|
|21|URI: skip one byte, read string, skip one byte|
|15|Binary block: aligned u32 length then skip bytes and align4|
|0D /0E|true / false|
|2B|Class: typed class-name then value/key pairs until00|
|2E /2D /0F|Number0 / number1 / following u8 literal|
|00|Null/terminator|

For class arrays whose next item is the control0x18 marker, the stream includes an aligned entry count and value/key pairs per object. Ordinary arrays contain typed values. The reader skips arbitrary binary blocks, so not every djay field is semantically decoded. The bplist route resolves `$objects` UID references, class dictionaries, `NS.objects`, `NS.relative`, and combines ADCMediaItemTitleID/UserData/Location objects. Unknown archive classes and binary blocks must be preserved as raw data in an independent implementation, not replaced with the reader's diagnostic placeholder string.

**Seen, R:350855–351143:** Local source URI selection prefers a path that exists, then first candidate; Apple iTunes or streaming identities use service-specific URI mapping. macOS local URI candidates prefer `/Users/` and `/Volumes/`. Cues use seconds directly, `number` one-based=>slot-1, optional endTime creates loop. Saved loops append after cues; pairs within0.01s in both endpoints deduplicate, while original loop number is retained. Metadata key prefers manualKeySignatureIndex2 over analyzed key; BPM incorporates signed tempo exponents.

**Seen grid calculation:** Base BPM is analyzed or manual. Tempo exponent is signed8-bit when stored>100, and multiplier is2^exponent. FirstDownbeatPosition or startBarPosition supplies initial seconds; straight-grid/startPoint and nrOfBeatShift fallback alter this (`0.003*nrOfBeatShift`). Between downbeat markers, BPM=`60*4*numberOfBars/(next_position-position)`; sentinel -1/255 bars uses base BPM. Last segment uses the final tempo exponent. Preserve `useStraightBeats`, both exponents, nrOfBeatShift and fractionalBeatShift. Date fields are seconds from2001-01-01; Lexicon constructs this epoch without an explicit UTC suffix, so timezone correctness needs an independent test (`R:267123`).

**Seen hierarchy:** Playlist root UUID `mediaItemPlaylist-root`; playlist type0, folder1, smartlist2. Follow childUUIDs in order, with parentUUID recovery for an empty root; translate filterSet separately. The original reader merges same-key collections and deduplicates media locations across batches. **Unknown:** Real djay v5/current fixtures, cross-platform UUID encoding, all typed-stream edge cases, and streaming account behavior were not executed.

## Rekordbox 5 collection XML: research-only

**Seen:** Lexicon reads exported collection XML; it does not directly connect to rekordbox5. Public guidance requires BPM change points enabled before export. [Rekordbox5 import](https://www.lexicondj.com/manual/import-rekordbox-5).

**Seen, R:347450–347666 and shared Rekordbox importer:** `DJ_PLAYLISTS/COLLECTION/TRACK` metadata attributes include TrackID, Name, Artist, Album, Genre, Composer, Mix, Rating, Remixer, Label, Tonality, PlayCount, Comments, BitRate, TrackNumber, TotalTime, Year, AverageBpm, Size, DiscNumber, SampleRate, DateAdded, Location, Colour. `POSITION_MARK` contains Name, Num (-1 memory), Start/End seconds, Type; `TEMPO` contains Inizio seconds, Bpm, Metro and Battito. PLAYLISTS NODE tree uses Type0 folder/Type1 playlist and TRACK Key links. The current direct reader creates a common in-memory representation resembling these attributes; that is not XML transport or a reason to add it to CueRithm.

**Boundary:** Keep this as source-format research and migration planning only. Repository instructions prohibit rekordbox XML transport. Do not add a selectable CueRithm XML import/export mode or route rekordbox6/7 through XML under this handover.

## Apple Music/iTunes plist XML

**Seen:** User exports a library XML. Track and playlist data are in that export; native Music database access is not needed by this adapter. [Apple Music/iTunes import](https://www.lexicondj.com/manual/import-apple-music-itunes).

**Seen, R:347039–347253, `ItunesXmlReader` / converter:** Top-level Tracks dictionary keyed by numeric ID; each track includes Track ID, Location, Name, Artist, Album, Size, Total Time, Date Added, Bit Rate, Sample Rate, Comments, Release Date, Composer, Grouping, Genre, Year, Rating, Play Count, Track Type. Track Type=Remote is excluded. Strip file:// and localhost, percent-decode location, normalize platform path; malformed encoding drops the row. Total Time milliseconds divide by1000 and floor to integer seconds; rating divides by20 except the special value1 is ignored. Dates use ISO8601. This reader does not read BPM/key from XML, nor DJ cue/grid structures.

**Seen, R:347146–347193 and 347254–347315:** Playlists use Playlist Persistent ID and Parent Persistent ID, Folder=true, Smart Criteria marker, Playlist Items Track ID. Master and distinguished lists are filtered. Smart membership can materialize from exported Track IDs, but raw Apple smart-rule bytes are not fully translated. The importer calls `importTrackBatch(batch,true)`, disabling cue/grid writes for these metadata-only batches (`R:347418`). Preserve existing DJ cues/grids during Merge. Retain playlist persistent IDs as source mapping even though Lexicon's merge identity uses name/path.

**Inferred:** Implement with a general safe plist parser rather than the exact line-matching helper, which assumes normal Apple pretty-printed XML. Preserve duration milliseconds in CueRithm rather than intentionally truncating them. XML alone cannot recover cloud-only audio or DJ performance metadata that was never exported.

## Pioneer/AlphaTheta USB and OneLibrary

**Seen, R:212628–212647, `USBImporter.detectSource`:** Find PIONEER directory, then `rekordbox/exportLibrary.db`; if present choose SQLite, otherwise `export.pdb`. `PioneerUsbImporter` sanity check accepts either. This is USB device-library parsing, distinct from direct desktop master.db. [USB import](https://www.lexicondj.com/manual/import-from-pioneer-usb).

**Seen SQLite mapping, R:212794–212889:** Tables `content`, `artist`, `album`, `genre`, `key`, `label`, `cue`, `playlist`, `playlist_content`, `history`, `history_content`. Content key=`content_id`; artist references distinguish artist/remixer/composer; `bpmx100/100`, length seconds, path on USB, analysisDataFilePath. Cue rows join by content_id, use inUsec/outUsec divided by1000000, cueComment, kind and isActiveLoop. Kind0 receives sequential hotcue positions in this adapter; others are unnumbered memory cues. Preserve raw kind before applying this mapping. Playlist parent/attribute/sequence fields are playlist_id_parent/attribute/sequenceNo, membership sequenceNo and content_id.

**Seen PDB mapping, R:212891–212960:** Read tables artists/albums/genres/keys/labels/tracks/playlistTree/playlistEntries. Tempo/100; duration seconds; strings include title/filePath/comment/analyzePath/analyzeDate/dateAdded. ParentId/rawIsFolder/id/name/sortOrder describe tree; playlistId/trackId/entryIndex describe ordered membership. History raw table11 has u32LE ID plus encoded name at offset4; table12 has u32LE trackId/historyId/entryIndex (`R:212767–212792`). The page, row and packed-string codecs are detailed in the following subsection. They are static layouts, not a claim of corpus acceptance.

**Seen analysis fallback, R:212642–212700:** Analyze DAT plus sibling EXT for grid segments. If SQLite supplied no cues, try EXT `PCO2` entries first, then DAT `PCOB`. Cue positions are milliseconds/1000; hotCue>0 maps slot=hotCue-1; loop cueType2, loopTimeMs=0xFFFFFFFF is absent. PCO2 comment/RGB can preserve cue names/colors; color mapping can approximate to palette while keeping RGB. If SQLite cue rows exist, this helper does not enrich them from PCO2—report that fidelity difference.

**Seen audio reconciliation, R:350576–350708:** Match existing library files by a filename-based query, passing artist/title hints. Otherwise copy USB audio to `Music/Lexicon/<drive label>/<sanitized artist>/<sanitized album>/<basename>`. Already-existing destination paths are reused. Missing audio still allows metadata import with a warning. This research never invoked that path. An independent CueRithm importer should stage a copy plan and detect basename collisions; do not equate file-copy success with metadata identity.

**Seen, R:350348–350419:** USB cue selector supports hot, memory or both. In both mode, an unnumbered memory cue at an already imported exact start time is dropped. USB history sessions are source/device-label scoped; imported creation time in this wrapper is current time rather than original session time (`R:350710–350810`). Preserve original timestamp separately when available.

**Unknown:** No real USB/PDB/OneLibrary image or CDJ hardware was tested. PDB page/row/string variant acceptance and full ANLZ extension codec acceptance need separate synthetic format controls before claiming a complete portable implementation. No raw PDB fixture from a user's device is included.


### PDB binary page, row and string reference

**Seen, R:199275–199496, modules81002/4545:** PDB header u32LE page size at4, table count at8, next unused page at12, sequence at20; table directory begins28, stride16: type, emptyCandidate, firstPage, lastPage (u32LE each). Follow each table's linked `nextPage`, bounded by file page count and a visited set. Page header is40 bytes: page index at4, type at8, nextPage at12; packed24-bit LE word at24 encodes `numRowOffsets=word&8191`, `numRows=(word>>13)&2047`; flags at27, free/used sizes u16LE at28/30, transaction row count/index at32/34. A data page has flags bit0x40 clear.

Row heap begins page+40. Row groups run backward from page end: u16LE free bitmap, u16LE present bitmap, up to16 u16LE heap offsets. Only offsets with corresponding present bit produce visible rows. Retain all offsets while calculating row lengths: each visible row ends at the next distinct sorted heap offset, or usedSize, bounded to heap end. Offset tables and heaps must never overlap; cap table/row counts before allocation.

**Seen table type IDs, R:198726–198800:** tracks0, genres1, artists2, albums3, labels4, keys5, colors6, playlist tree7, playlist entries8, artwork13, menu items16, category17, sort18, device name19; history11/12 are raw-table paths in this importer. Unknown tables remain raw.

**Seen packed strings, R:199957–199995, module38868:** If first byte low bit is1, payload ASCII length=(byte>>1)-1, beginning+1; total bytes consumed=1+length. Otherwise read u16LE total byte length at+1: tag0x40 means ASCII at+4 until total length, tag0x90 means UTF-16LE at+4 until total length. Other tags return unknown/empty in Lexicon; preserve bytes instead of treating them as an empty source value. Bounds-check tag header and length, including UTF-16 alignment.

**Seen track row, R:199528–199572:** u16LE subtype0/indexShift2; u32LE bitmask4, sampleRate8, composerId12, fileSize16, masterContentId20, masterDbId24, artworkId28, keyId32, originalArtistId36, labelId40, remixerId44, bitrate48, trackNumber52, tempo56, genreId60, albumId64, artistId68, id72; u16LE disc76/playCount78/year80/sampleDepth82/duration84/unknown86; u8 color88/rating89; u16LE fileType90/stringOffsetPrefix92. String-offset array begins94 with21 u16LE row-relative pointers, in order: isrc, texter, unknown2, unknown3, unknown4, message, kuvoPublic, autoloadHotcues, unknown5, unknown6, dateAdded, releaseDate, mixName, unknown7, analyzePath, analyzeDate, comment, title, unknown8, filename, filePath. Retain unknown fields; never infer their semantics from names alone.

**Seen lookup/hierarchy rows, R:199574–199667:** Genre/label/artwork starts u32LE ID then packed string at4. Key has u32LE ID0/ID2 at4 then string8. Artist subtype96 uses u8 name pointer at9, subtype100 uses u16LE at10; ID is u32LE at4. Album subtype128 uses u8 name pointer21, subtype132 uses u16LE22; artist ID8, album ID12 (u32LE). Color ID is byte4 and name starts8. Playlist tree u32LE parent0, unknown4, sortOrder8, id12, folderFlag16, packed name20. Membership u32LE entryIndex0, trackId4, playlistId8.

**Seen SQLite device cipher profile, R:207488–207525, modules1594/26850/33119:** `better-sqlite3-multiple-ciphers`, SQLCipher cipher with legacy4, then application format-key configuration; reader opens read-only and enumerates the known tables. No key value is included in this handover. Resolve key provisioning through the approved existing CueRithm adapter/dependency policy, never from user/account credentials. `property.dbVersion` and optional table presence should drive acceptance rather than a filename alone.


### Existing CueRithm native codecs to reuse

**Seen repository source at commit `446aab645b40dcaad82182c79d96daa7fa27f9cc` (origin/main when inspected):** `crates/cuerithm-dj-sync/src/pioneer_usb/pdb.rs:124` already decodes packed strings; `:155` already reads linked data pages and row bitmaps with bounded sizes/cycle protection; `:220` builds PDB. `database.rs:12` opens the format with SQLCipher compatibility4; `:28` stages a consistent read-only SQLite backup and gates `property.dbVersion` to1000. Existing approved format-key provisioning is owned there; it is an interoperability constant, unrelated to any user account. Reuse this source policy rather than asking for credentials or rediscovering a key from installed apps.

`database.rs:121` imports PDB into the staged relational representation; `:511` maps relational data back to PDB tables, and `:598` constructs complete track rows. `analysis.rs:149` maps cue records; `:210` creates grid records; `:284/:300` build tagged ANLZ buffers. These are existing native implementation paths, not new work requested here. Their published source and current tests provide the clean-room implementation starting point; the Lexicon table/offset notes explain the corresponding incoming layout. Preserve raw records when extending readers.

`crates/cuerithm-dj-sync/src/timing.rs:44` owns target offsets and `:168` exports adjusted tracks at this same commit. Reuse a validated inverse profile for import rather than adding format-level guesses. All source paths in this paragraph are pinned to this commit; they must not be inferred from the older checked-out working branch.


### Existing native tests and fixture generation without DJ applications

The following locators are pinned to the same CueRithm commit `446aab645b40dcaad82182c79d96daa7fa27f9cc` above. They generate original synthetic audio/catalogs or read checked-in synthetic media; an engineer can exercise these controls without installing any DJ suite. These tests were **inspected, not rerun** in this research. They do not supply target-app/device acceptance.

| Source/test locator | Reusable control |
|---|---|
| `crates/cuerithm-dj-sync/src/pioneer_usb.rs:467`, `encrypted_catalog_uses_observed_native_sqlcipher_v4_profile` | Confirms page4096, KDF256000, HMAC-SHA512/PBKDF2-HMAC-SHA512 profile |
| `pioneer_usb.rs:483`, helper `audio`; `:506`, helper `library` | Builds synthetic48kHz WAV and catalog input |
| `pioneer_usb.rs:564`, `active_loops_agree_across_usb_catalogs_and_preserve_native_metadata` | Compares loop/active flags across output databases and analysis assets |
| `pioneer_usb.rs:669`, `optional_numeric_metadata_agrees_across_usb_catalogs_and_survives_unknown_sources` | Optional metadata agreement and preservation |
| `pioneer_usb.rs:791`, `exports_playable_media_encrypted_database_and_complete_analysis_assets` | Writes complete synthetic media/PDB/encrypted database/ANLZ outputs; optional `CUERITHM_USB_FIXTURE_DIR` keeps them in an engineer-selected empty scratch directory |
| `pioneer_usb.rs:897`, `subset_sync_preserves_unrelated_tracks_playlists_and_device_configuration` | Subset preservation |
| `pioneer_usb.rs:951`, `merges_existing_pdb_only_catalog_without_migration` | Legacy PDB-only input route |
| `pioneer_usb.rs:1090`, `pdb_rejects_corrupt_offsets_and_row_overflow` | Malformed offsets and row-size rejection |
| `pioneer_usb.rs:1097`, `real_mp3_profile_moves_cues_grid_and_detail_frames_once_and_preserves_audio` | Native timing offset applied once; uses `tools/fixtures/media.tar.gz` (`pulses.mp3`) plus sealed `tools/media-runtime/bin` |
| `crates/cuerithm-dj-sync/src/pioneer_usb/tests/path_tests.rs:44/63/95/121/148/183` | Hash/analysis-path collisions, subset reservations, orphan assets, hidden library precedence and content mismatch |
| `crates/cuerithm-dj-sync/src/pioneer_usb/analysis.rs:625`, `detail_padding_matches_source_frame_quantization_strides_and_first_frame_contents` | Detail-frame stride/padding for PWV3/PWV5/PWV7 |
| `tools/fixtures/generate_audio.py`, `tools/fixtures/media-cases.json`, `tools/fixtures/media.tar.gz` | Reproducible media-corpus sources and declared format cases |

Representative existing test filter: `cargo test -p cuerithm-dj-sync pioneer_usb::tests::exports_playable_media_encrypted_database_and_complete_analysis_assets -- --exact`. Run it from the pinned source checkout with its required native build dependencies; to keep generated catalogs, set the fixture-directory variable only to a fresh scratch path, never an existing device library. No checkout was changed or created by this research.

## M3U/M3U8

**Seen, R:270148–270247, `M3uImporter`, module40146:** Multiple selected files are stored as NUL-separated paths. Import forces Merge on and backup off, reads text as UTF-8 except detected UTF-16LE, parses non-comment lines, deduplicates normalized locations, adds referenced media through the ordinary track importer, and writes a playlist named from each file's basename under the selected/root parent. Existing same-name playlist membership is replaced. Empty/unmatched lists are skipped. [M3U import](https://www.lexicondj.com/manual/import-m3u).

**Seen, R:328006–328071, module66859:** The import parser splits CR or LF, ignores lines whose trimmed form starts `#`, normalizes path separators and removes empty entries. `#EXTINF` display metadata is ignored in this import branch. A separate richer parser handles `#EXTVDJ` and EXTINF for other paths; do not assume the import branch uses it. No parent-directory join is visible in `M3uImporter`; relative-path semantics are therefore **Unknown** until a control test. A clean-room importer should specify playlist-directory-relative paths, URI handling, BOM, UNC, drive roots, Unicode and duplicates explicitly, then test them.

## Preservation and acceptance checklist for implementation

1. Snapshot inputs read-only with WAL/SHM consistency where relevant; keep original source IDs and schema fingerprints. Reject unsupported encrypted profiles with an actionable message.
2. Separate source metadata, performance data, membership/hierarchy and audio-copy plan. One failed audio path must not masquerade as no library data.
3. Normalize all time into seconds internally while retaining original integer milliseconds/microseconds or samples/sample-rate. Preserve negative/sentinel states separately from valid0s cues.
4. Preserve original cue class, slot/index, loop flags, lock flags and RGB, plus unsupported raw records in adapter-owned metadata. Report palette reduction, duplicate membership removal, smart-rule loss and missing analysis sidecars.
5. Stage full/playlist filters before mutation. **Seen Lexicon behavior:** selected fields gate existing-track updates. **Proposed CueRithm contract:** apply selected groups to publication for new tracks too, retaining unselected parsed data only in provenance; use the pack's shared policy as the authority. Preserve unselected existing cue/grid data and unrelated destination-only content under Merge.
6. Use namespaced IDs for Track/asset/UUID/PDB keys. Distinguish source mapping from normalized path matching. Test equal basenames, case/Unicode differences, moved volumes, orphan links and cyclic hierarchy.
7. Test static and variable grids, non44.1k sample rates, nonzero lead-in, sparse hotcue slots, loop-only tracks, Unicode cue labels, memory/hot coincidence, invalid lengths, optional columns/tables, unknown version/type, and playlist duplicate order.
8. Run the synthetic codec vectors first; then copied synthetic per-app databases/containers; then each source application's own import/reopen and read-back checks. Record exact app build, source schema, importer version and expected losses. Static Lexicon reading is not source-app acceptance.

## Remaining evidence gaps

The described Serato Markers2/BeatGrid buffers have an executed original-decoder synthetic control; Engine vectors have independent struct/zlib controls. The remaining implementation findings above are static bundle inspection or public documentation. No complete end-to-end local-library import/export acceptance, source application write-back, device playback, online-service import, or arbitrary version compatibility is claimed. Engine schema-variant acceptance, low-level PDB corpus acceptance and complete smart-rule operator coverage require further targeted work before an engineer can honestly mark those adapters fully implemented. The reference is sufficient to build the staging model and the documented parser paths; those explicit gaps remain release gates.
