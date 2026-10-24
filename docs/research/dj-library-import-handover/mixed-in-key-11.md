# Mixed In Key 11 library import handover

This is an implementation reference for **read-only library import and repeat import** from Mixed In Key (MIK), preserving the user's persisted results. It does not implement an importer, establish runtime acceptance, or reproduce analysis/cue generation. Research date: 2026-10-23. Scope: installed macOS 11.2.6, build **11.2.6.7446**. Windows persistence has not been inspected.

**Seen** means direct observation of a snapshot, compiled model, or a bounded native instruction path. **Inferred** means a proposed interpretation or engineering decision. **Unknown** means unverified; it must not silently become a default that loses data. The evidence register below gives stable anchors and native virtual addresses. No vendor code, music, identifying track rows, license material, or usable security bookmarks are included here.

## Decision and boundaries

**Seen:** A complete persisted collection can be imported from `Collection11.mikdb`, including descriptive tags, analyzed and tagged key/BPM/energy, folders/playlists, ordered membership, persisted cue points, key/energy segments, and an explicit beat-time array. Paths require decoding macOS bookmarks; there is no persisted plain path column in this model. [MIK-E02](#mik-e02-schema-and-model), [MIK-E03](#mik-e03-bookmarks), [MIK-E04](#mik-e04-beats)

**Inferred implementation contract:** Import all songs independently of membership, then collections, then links. Default to preserving imported results and source metadata exactly. Reanalysis and retagging are separate, opt-in operations with a preview of replacements. An imported song must not enter CueRithm's automatic Incoming analysis/tagging pipeline merely because it was imported. Retain MIK provenance even when a user later chooses another analyzed result. Writing back to MIK or music files is outside the proven contract.

## Discovery and consistent acquisition

**Seen:** The populated store was at `~/Library/Application Support/Mixedinkey/Collection11.mikdb`; `Collection11.mikdb-wal` and `Collection11.mikdb-shm` also existed. Directories named `Mixed In Key 11` and `com.mixedinkey.application` existed but did not contain files in this inspection. A `backups/` directory contained a ZIP backup, whose format was not opened. Do not assume bundle identifier equals the database directory. [MIK-E01](#mik-e01-artifact-identity)

**Inferred acquisition algorithm:**

1. Let the user select a database or discover the known legacy directory. Do not recursively inspect unrelated application preferences/accounts.
2. Open the original with SQLite URI `mode=ro`; use SQLite's backup API to create a private snapshot in the importer workspace. This incorporates committed WAL transactions consistently. Perform all queries against the snapshot. Do not use `immutable=1` on a live WAL store or copy only the main file.
3. For a forensic preservation copy, copy the database and any WAL/SHM sidecars together; this is distinct from the coherent SQLite backup. Never open the source through Core Data, invoke migrations, checkpoint its WAL, or call MIK data-object setters.
4. Read `sqlite_master`, required columns, `Z_METADATA.Z_PLIST`, and `Z_PRIMARYKEY` before accepting this profile. A future schema must be diagnosed rather than interpreted by table numbers alone.
5. Treat the snapshot, bookmark blobs, decoded paths, fingerprints and source IDs as private local data. Keep logs to aggregate counts and redacted diagnostics.

**Seen:** The research used Python `sqlite3.connect('file:…?mode=ro', uri=True)` followed by `Connection.backup()`. The research snapshot contained 446 songs, 2 collections, 446 memberships, 10,370 cue rows, 4,473 energy segments, 446 key segments and 1,338 waveform rows. These counts demonstrate coverage of this snapshot, not general capacity limits. All source inspection was read-only; no MIK UI or source audio was modified.

## Store identity and relationships

The complete observed SQL column inventory is in [mik-coredata-11-schema.json](fixtures/mik-coredata-11-schema.json). It contains schema only. [MIK-E02](#mik-e02-schema-and-model)

| Table | Identity / relationship | Import behavior |
| --- | --- | --- |
| `ZSONG` | `Z_PK`, `Z_ENT=5` in this model | Import every row, including songs without a collection link. |
| `ZCOLLECTION` | `Z_PK`, `Z_ENT=1`; `ZPARENTCOLLECTION → ZCOLLECTION.Z_PK` | Preserve parent hierarchy, name, emoji, flags, index and pinned index. |
| `Z_1SONGS` | `Z_1COLLECTIONS → ZCOLLECTION.Z_PK`; `Z_5SONGS → ZSONG.Z_PK` | Membership is unique per pair; order by `Z_FOK_5SONGS` ascending within each collection. |
| `ZCUEPOINT` | `Z_PK`, `Z_ENT=2`; `ZSONG → ZSONG.Z_PK` | Preserve all cue rows. Source relationship is unordered. |
| `ZENERGYSEGMENT` | `Z_PK`, `Z_ENT=3`; `ZSONG → ZSONG.Z_PK` | Preserve source segments as provenance, separate from ordinary whole-track energy UI. |
| `ZKEYSEGMENT` | `Z_PK`, `Z_ENT=4`; `ZSONG → ZSONG.Z_PK` | Preserve segmented source results independently of the whole-track key. |
| `ZWAVEFORM` | `Z_PK`, `Z_ENT=6`; `ZSONG`; `ZSONG.ZWAVEFORM` is the song's selected to-one link | Optional cache; do not turn every waveform row into a track or assume a unique `ZSONG`. |
| `Z_METADATA` | SQLite store version/UUID and `Z_PLIST` | Namespace imported identities by the selected store. |
| `Z_PRIMARYKEY` | Entity names and allocation state | Validate names/IDs; never allocate or update IDs. |

**Seen:** `Collection.songs` is ordered, its inverse `Song.collections` unordered. `childCollections` is unordered; `index` is a persisted signed 16-bit attribute. `isFolder`, `isLibrary`, `pinnedIndex` and `emoji` are persisted. `isPlaylist` returns the inverse of `isFolder`, so a library row can also satisfy the low-level playlist predicate. Classify a library specially before assigning user-playlist semantics. [MIK-E05](#mik-e05-order-and-collection-kinds)

**Inferred:** Preserve the numeric ordering token; do not require dense `0..N-1` positions. Observed membership order tokens started at 2048 and contained gaps. Sort siblings by `(ZINDEX, Z_PK)` for deterministic display; `Z_PK` tie-breaking is CueRithm policy, not demonstrated MIK behavior. Handle parent cycles, orphan references and duplicate names without dropping rows. `ZPINNEDINDEX=-1` is the observed/model-default unpinned sentinel; preserve positive/nonnegative values as source ordering metadata pending richer acceptance tests.

**Seen:** `Collection.filter` is **transient** in DataModel 11, with no SQL column. Native classes `MIKDataSmartCollection`, `MIKDataRecentsCollection` and `MIKDataPinnedCollection` exist; smart collection membership is derived by filtering a library ordered set. **Unknown:** persistence of user-visible smart filters outside this store. Do not fabricate rules from the playlist name or promise all virtual sidebar views have persisted membership. Only two top-level non-folder collections existed in the observed snapshot; nested folder/runtime ordering acceptance remains untested. [MIK-E05](#mik-e05-order-and-collection-kinds)

## Paths, security bookmarks and remapping

**Seen:** `Song.path` is a **transient** string; `Song.bookmarkData` is persisted binary data (`ZBOOKMARKDATA`). Every inspected song had a bookmark beginning `book`. `MIKDataSong.fileURL` first asks Foundation for `NSURLPathKey` from the bookmark, then resolves it using options `0x300` or `0x700` depending on `resolvesBookmarksUsingSecurityScope`; it can compare alias/symlink-resolved paths and update its managed object. Do not instantiate that class just to read a source database. [MIK-E03](#mik-e03-bookmarks)

**Inferred macOS route:** Use Foundation directly on the copied bytes. `NSURL.resourceValues(forKeys: [.pathKey], fromBookmarkData: blob)` recovers the embedded path without invoking the MIK object. Optionally resolve the bookmark with no UI/no mounting to detect moved files, retaining original and resolved paths separately; do not rewrite the source bookmark. A bookmark does not automatically grant CueRithm usable security access. Request CueRithm's own file/folder access where required, retain any scope locally, and pair scope start/stop correctly.

**Seen portable embedded-path subset:** All 446 bookmarks' paths were also recovered by the following structural interpretation, with exact string equality to Foundation's resource-value extraction for all 446. This is an embedded historical path decoder, not a portable alias resolver or security-scope implementation.

| Position / value | Observed interpretation |
| --- | --- |
| bytes `0..4` | ASCII `book` magic |
| LE u32 at `4` | total byte length |
| LE u32 at `12` | data-section base offset; observed `64` |
| LE u32 at data base | first TOC offset relative to data base |
| TOC `+0,+4,+8,+12,+16` | length, magic `0xfffffffe`, TOC identifier, next TOC relative offset, entry count |
| TOC entries from `+20`, stride 12 | LE u32 key, record offset relative to data base, reserved u32 |
| record header | LE u32 payload length, LE u32 type, then payload |
| key `0x1004`, type `0x0601` | ordered array of LE u32 record offsets; each points to a path-component string |
| string type `0x0101` | UTF-8 byte string of the declared payload length |
| key `0x2002`, type `0x0101` | volume path; observed root `/` in the path comparison |

**Inferred decoder rules:** Bounds-check every addition, header, offset, length, TOC entry count and array size; cap traversal and detect cycles. Follow valid chained TOCs conservatively, retaining unknown records. Join the volume path and path components using path-component semantics. Test external-volume bookmarks before treating that join as universal; all verified path matches here were for the observed store. Unsupported variants must yield an unresolved track with preserved source metadata, not removal. Never print arbitrary bookmark strings: other record types can contain identifying machine, volume and security information. The synthetic fixture includes only invented path records and is not claimed to be an OS-valid bookmark.

**Inferred remapping:** Keep `source_embedded_path`, any native resolved path, and the user's current CueRithm path distinct. Apply explicit longest-prefix mappings on component boundaries (for example `/DemoMusic` → `D:\DemoMusic`), with a preview of unresolved/ambiguous files. Never basename-match silently. Do not alter case or Unicode of preserved source strings. Treat path comparison rules as platform-specific. Resolve duplicates using store/song identity first; path collisions and fingerprints are evidence for user-reviewed consolidation, not permission to merge source songs.

## Full song metadata and tag provenance

**Seen:** The following persisted attributes exist; names below are SQL columns, not guesses based on UI labels. Numeric SQL `FLOAT` does not imply every source attribute is the same precision. The compiled model distinguishes Float (32-bit; type 600), Double (64-bit; type 500), integer widths, Boolean, Date, Binary and Transformable. [MIK-E02](#mik-e02-schema-and-model)

| Group | Columns / model facts |
| --- | --- |
| Descriptive strings | `ZNAME`, `ZARTIST`, `ZALBUM`, `ZCOMPOSER`, `ZGENRE`, `ZGROUPING`, `ZCOMMENT`, `ZLABEL`, `ZREMIXER`; `ZYEAR` and `ZRATING` signed 16-bit integers |
| Analyzed results | `ZKEY` required string; `ZSECONDKEY` optional string; `ZTEMPO`, `ZENERGY`, `ZPITCH` Float; `ZBEATS` Transformable |
| Cached tag results | `ZTAGKEY` optional string; `ZTAGTEMPO`, `ZTAGENERGY` signed 16-bit integers; `ZTAGCOMMENTS` Binary |
| File facts | `ZBITRATE`, `ZSAMPLERATE` signed 32-bit integers; `ZFILESIZE` signed 64-bit; `ZMODIFICATIONDATE` Date; `ZBOOKMARKDATA` Binary |
| Source identity | `ZFINGERPRINT` (model spelling `fingerPrint`), `ZCLOUDSYNCTRACKID` strings; keep private |
| Source dates | `ZANALYSISDATE`, `ZSYNCDATE`, `ZTAGUPDATEDATE` Date |
| Presence flags | `ZHASID3V1`, `ZHASID3V2`, `ZHASPNTAG`, `ZHASCHECKEDPNTAG`, `ZHASSERATO`, `ZHASTRAKTOR`, `ZISPERSONALIZED` Boolean |
| Loudness / processing provenance | `ZVOLUME`, `ZVOLUMELUFS`, `ZVOLUMERMS1`, `ZVOLUMERMS2` Float; `ZCLIPPEDPEAKCOUNT` Int32; `ZPNTAGISPROCESSED`, `ZPNTAGAPPLIEDCLIPREPAIR` Boolean; `ZPNTAGVOLUMEANALYSISVERSION` Int32; `ZPNTAGOUTPUTVOLUME` Double; `ZPNTAGVOLUMEUNITS` string |

**Inferred:** Retain every source field, null and original numeric value in a private provenance record even if CueRithm has no current UI for it. Retain model-default `key="0"`, zero tempo/energy, zero year/rating and null distinctly; do not present defaults as successful analysis. Core Data Date values use the Apple reference epoch (2001-01-01 UTC); convert to Unix seconds by adding `978307200`, while retaining the original numeric value. Do not interpret `ZSYNCDATE` as a complete database change cursor or `Z_OPT` as a cross-store revision identifier.

**Seen:** `ZTAGCOMMENTS` decoded as a binary plist NSKeyedArchiver whose root was `NSArray`, containing `NSDictionary` entries with keys `isPrimary`, `v1CommentMatchesv2Comment`, `tagVersion`, `comment`. Preserve order, primary/version flags and all comments rather than flattening into only `ZCOMMENT`. The observed dictionaries used Boolean flags, integer tag version and a string comment. [MIK-E06](#mik-e06-tag-comments-and-caches)

**Unknown:** This database is not a lossless dump of every music-file tag. Artwork is a transient `hasArtwork` property, with no artwork payload in this model; arbitrary custom file tags are not enumerated. A requirement to copy **all file tags** needs a separate read-only tag reader for the file formats, preserving unknown frames/atoms and explicitly distinguishing file tags from the MIK cache. Do not call retagging to fill that gap. Rating scale, bitrate unit, `pitch` interpretation, and loudness/RMS units beyond their stored names have not been accepted against UI/audio. Preserve those values without converting them on a guess.

## Keys and notation

**Seen:** `ZKEY`, segment `ZKEY` and `ZSECONDKEY` are strings, not numeric key IDs. This snapshot's analyzed keys were Camelot strings such as `8A`; `ZTAGKEY` is a separate string. Model defaults include `Song.key="0"` and `KeySegment.key="7A"`; absence/failed analysis must not be inferred from an arbitrary default segment row. [MIK-E02](#mik-e02-schema-and-model)

**Seen:** MIKAnalysis contains a 24-row, seven-string notation table at ARM64 VM `0x440d0`, accessed with 56-byte row stride. Column 0 is Camelot; remaining columns contain long/short flat/sharp spellings. Its numeric row index is an internal chromatic ordering, **not** what the SQLite key string stores. [MIK-E07](#mik-e07-key-table)

| Internal index | Camelot | Traditional key (one equivalent spelling) |
| --- | --- | --- |
| 0 | 11B | A major |
| 1 | 6B | B-flat major / A-sharp major |
| 2 | 1B | B major |
| 3 | 8B | C major |
| 4 | 3B | D-flat major / C-sharp major |
| 5 | 10B | D major |
| 6 | 5B | E-flat major / D-sharp major |
| 7 | 12B | E major |
| 8 | 7B | F major |
| 9 | 2B | F-sharp major |
| 10 | 9B | G major |
| 11 | 4B | A-flat major / G-sharp major |
| 12 | 8A | A minor |
| 13 | 3A | B-flat minor / A-sharp minor |
| 14 | 10A | B minor |
| 15 | 5A | C minor |
| 16 | 12A | D-flat minor / C-sharp minor |
| 17 | 7A | D minor |
| 18 | 2A | E-flat minor / D-sharp minor |
| 19 | 9A | E minor |
| 20 | 4A | F minor |
| 21 | 11A | F-sharp minor |
| 22 | 6A | G minor |
| 23 | 1A | A-flat minor / G-sharp minor |

**Inferred:** Preserve original analyzed/tagged strings first, then independently normalize recognized notation for CueRithm display. `A` means minor and `B` major in Camelot. Recognize leading-zero Camelot forms without rewriting raw provenance; the inspected MIK parser removes a leading zero from three-character input. Keep unknown spellings and multiple-key text as raw text with normalization unavailable. Switching display to traditional notation must not change tags or rerun analysis. **Unknown:** the exact user-preference enum naming all seven notation columns; no user settings were read.

## Persisted cue points and time precision

**Seen:** `ZCUEPOINT` stores `ZTIME` as model Double, nullable `ZNAME`, nullable signed 16-bit `ZENERGYLEVEL` and a song link. There is no slot, loop end, cue color, cue type, hot/memory flag, or explicit persisted cue ordinal in this entity. The relationship is unordered. MIK sorts it ascending by `time` for display/export. More than eight cue rows per song existed; do not truncate to eight or relabel all stored rows as target-platform hotcues. A negative near-zero time occurred in the snapshot. [MIK-E08](#mik-e08-cue-order-and-units)

**Seen static unit chain:** Main executable VM `0x100132334..0x100132390` passes a data cue's `time` unchanged as a double to `MIKTagCuePoint.initWithTime:name:`. `MIKTagID3.setSeratoCuePointsNew:` multiplies that value by the double literal `1000.0` and rounds to an integer position. **Inferred unit interpretation:** persisted cue times are seconds on the file timeline, converting to milliseconds for Serato positions; this is strongly supported static evidence, not a measured device/playback round trip. Preserve fractional and negative times; clamp only a playback seek if required, never the stored imported cue. [MIK-E08](#mik-e08-cue-order-and-units)

**Inferred mapping:** Keep source cue identity `(store, Z_PK)`, raw nullable name, energy field and f64 seconds. Sort by `(time, Z_PK)` for deterministic ties; tie-break behavior is CueRithm policy. Do not replace a null name with MIK's computed short label as though it were a stored custom name. MIK can synthesize a short label from position, and export paths can synthesize energy text; preserve the source fields rather than importing that decoration as a user edit. Source cue energy remains source provenance and must not become a new structural-cue energy label in normal CueRithm analysis.

## Explicit beatgrid serialization

**Seen:** All 446 `ZBEATS` blobs were `bplist00` NSKeyedArchiver archives. The root custom class is `MIKBeats`, with keys:

- `MIKBeats_beatTimes`: UID reference to an `NSArray`; its `NS.objects` array is an **ordered list of UIDs** resolving to numeric objects.
- `MIKBeats_tempo`: a floating scalar encoded/decoded with `encodeFloat:forKey:` / `decodeFloatForKey:` (32-bit source precision).
- `$class`: UID to class descriptor `MIKBeats`, class chain `[MIKBeats, NSObject]`.

The archive envelope contains `$version`, `$archiver`, `$top` (`root` UID), and `$objects`; object index zero is `$null`. The `NSArray` class descriptor is `[NSArray, NSObject]`. There were 312,412 numeric beat entries across the inspected blobs; all were floating scalars, and every sequence was strictly increasing. [MIK-E04](#mik-e04-beats)

**Inferred parser:** Use a binary-plist parser and explicitly walk the UID object table. Allow only the known container/class shapes and finite numeric beat values; do not load vendor classes or deserialize arbitrary Objective-C object graphs. Preserve unfamiliar blobs and emit a diagnostic instead of aborting the full song import. Numeric plist values may be direct scalar objects, so do not require every number to be an `NSNumber` dictionary. Keep archive tempo, `ZTEMPO`, and integer `ZTAGTEMPO` separate.

**Inferred timeline interpretation:** Beat-time values use the same seconds timeline as cues. Preserve the explicit array exactly. Do not regenerate a grid from BPM and one offset: the archive holds individual timestamps and the observed endpoint-derived rate divided by archive tempo ranged roughly 0.967–2.000. Native `MIKBeats.beatTempo` even computes `30*(N-1)/(last-first)`; it must not be substituted for stored tempo or treated as a proof of quarter-note metering. **Unknown:** downbeat/bar numbering, time signature and universal subdivision relative to displayed BPM. No such field exists in the decoded root shape. [MIK-E04](#mik-e04-beats)

## CueRithm clock and marker classification

**Inferred:** The seconds interpretation above identifies MIK's native persisted timeline. It does not prove identity with CueRithm's canonical FFmpeg PCM origin for every codec/encoder. Follow `import-and-sync-contract.md` native-clock policy: preserve native seconds and raw archive values, use `unresolved_native` until a matching timing profile is qualified, and subtract a verified source offset exactly once when producing canonical positions. A lightweight timing probe is distinct from key/BPM/cue reanalysis.

Use the selectable generic source cue/marker group for MIK CuePoint rows. They have no stored hotcue slot/class, so represent `family=marker` with source identity and no fabricated pad number. Target-specific conversion into hotcues is a later explicit export operation with its own capacity/loss report.

## Other source results and caches

**Seen:** `KeySegment` stores start/length (Double), key, confidence/energy/volume (Float) and single-note (Boolean); `EnergySegment` stores start/length (Double), energy (Float) and comment. Native `endTime` is `startTime + length`. Preserve segments in source provenance, with inferred seconds units; avoid exposing segment energy as ordinary new analysis. [MIK-E02](#mik-e02-schema-and-model)

**Seen:** `ZWAVEFORM.ZOBJECT` is another keyed archive with class `MIKNewWaveform` and root keys `data` and `duration`. Its proprietary sample/cache payload was not decoded. Waveform is an optional cache, not needed for library/tags/cues/grid import. Rebuilding CueRithm's own waveform from audio is distinct from opting into key/BPM/cue reanalysis. There were three waveform rows per song on average, although the model's song relationship is to-one; follow `ZSONG.ZWAVEFORM` if copying a selected cache. [MIK-E06](#mik-e06-tag-comments-and-caches)

## Repeat import, conflict handling and acceptance

**Inferred design:** Namespace source rows by database UUID plus entity/primary key, with a user-visible source binding when UUID is unavailable. Keep a private snapshot manifest and a content hash of the raw source fields/child rows per song, collection and ordered membership. Re-snapshot on refresh, compare source state, then apply additions/updates transactionally to CueRithm. Do not use paths as primary identities. A copied database may share its original UUID; require an explicit source rebind if two selected stores with the same UUID diverge.

Preserve user edits in CueRithm and source imports separately. A refresh may update the imported layer while keeping the user's chosen active values. Report source deletions/removal from playlists separately; never delete music and do not delete a CueRithm track merely because a refreshed source no longer lists it. Schema `Z_OPT` and per-track dates are not sufficient for tombstones, hierarchy changes or membership order changes. A full snapshot comparison is the safe initial refresh strategy for this profile.

The [synthetic fixture](fixtures/mik-synthetic-import.json) includes invented collections, sparse membership order, analyzed/tagged key/BPM differences, two cues including a negative time, a minimal bookmark-record shape and a real binary-plist archive with invented `MIKBeats` values. It contains no user track data and is **not** a vendor-accepted database or native bookmark round trip.

Before enabling an importer, require:

1. Offline parser tests for the fixture, malformed/truncated offsets and UIDs, nulls, unknown strings, duplicate cue times, unresolved paths, sparse order, orphan/cyclic folders and rollback.
2. User-authorized acceptance on a copied MIK library containing nested folders, deliberately reordered playlists, Unicode/renamed/missing/external-volume files, fractional and negative cue positions, custom cue names, tempo overrides, multiple key spellings and unknown tags. Compare exact imported raw values and memberships, not just visible totals.
3. Repeat-import checks with source changes and CueRithm edits, proving conflict handling and no repeated rows.
4. Source database/WAL and music hashes unchanged by import; no migration, tag write, license/account access or automatic reanalysis.
5. Windows/Linux path-remapping acceptance against a macOS-origin snapshot. Native Windows MIK profile remains unsupported until separately inspected.

**Current status:** Static/model/schema inspection and read-only snapshot parsing are complete for the observed profile. Foundation embedded-path recovery matched the portable subset for 446/446 bookmarks; all 446 beats archives were structurally decoded. There is no CueRithm importer, UI/runtime import test, target-platform cue export test, or successful MIK write-back proof in this handover. Version gates should remain explicit.

## Evidence register

### MIK-E01: artifact identity

Installed bundle: `/Applications/Mixed In Key 11.app`; identifier `com.mixedinkey.application`; version/build from `Contents/Info.plist`: `11.2.6` / `11.2.6.7446`. Main universal executable SHA-256 `04f10f4cf84938f42dc13c0c984da915b861beaab9675713b61bf82e17c5b905`. Research used separate copies under a unique temporary `re/mik11-*` directory; no signing or app files were changed. Snapshot SHA-256 `3a30e6be532bfc8ce1475b421047a462dd5ed85dd348a6062e0ccee3c6a05df5` identifies the private snapshot only, not an importer compatibility signature.

### MIK-E02: schema and model

Source: snapshot `sqlite_master`, `PRAGMA table_info`, `Z_PRIMARYKEY`; packaged `MIKData.framework/Versions/A/Resources/DataModel.momd/DataModel 11.mom` inspected through `NSManagedObjectModel` without opening the database via Core Data. Model SHA-256 `bf8fda51ba9638bc542f085934b678b61dcbc3978b70f30806fdad1e6122c5c4`. MIKData universal framework SHA-256 `99218933b45bcc90bc85e7d45b5160c4c38184b067a977818e58f347c5719f78`. Segment end-time methods: MIKData ARM64 VM `0xc494` / `0xd86c`.

### MIK-E03: bookmarks

Source: all 446 snapshot `ZSONG.ZBOOKMARKDATA` records; direct structural parser, independently compared to Foundation `NSURL.resourceValues(forKeys:fromBookmarkData:)`, 446 exact path matches. Bounded MIKData ARM64 `-[MIKDataSong fileURL]` VM `0xbacc..0xbdd4`, specifically resource-value request `0xbb24..0xbb7c`, resolve options/call `0xbba4..0xbbd4`, and managed-object update branch `0xbcfc..0xbd64`. The helper read copied blobs and did not call MIK or open source music.

### MIK-E04: beats

Source: 446 snapshot `ZSONG.ZBEATS` keyed archives parsed structurally. MIKAnalysis universal SHA-256 `cf06666d4ae1b0d214ec990523a792fe96556508b0a571207a66985a3dc654fd`. ARM64 `-[MIKBeats initWithCoder:]` VM `0x98b4..0x9944`, `encodeWithCoder:` `0x9948..0x99c0`, archive keys via exported `_kMIKBeats_beatTimesKey` / `_kMIKBeats_tempoKey`; `beatTempo` `0x9a38..0x9b20`, with literal 30 at `0x9af8`. `beatTimes` is an NSArray property; `tempo` stored float.

### MIK-E05: order and collection kinds

Source: compiled model relationships and snapshot join keys/order. MIKData ARM64 `-[MIKDataCollection isPlaylist]` VM `0x94b0..0x94c4`; `MIKDataSmartCollection.songs` VM `0xc5bc` calls `filteredOrderedSetUsingPredicate:` on library membership. Smart class symbols establish derived views; they do not establish persistence of every application filter.

### MIK-E06: tag comments and caches

Source: snapshot archived `ZTAGCOMMENTS` dictionaries and `ZWAVEFORM.ZOBJECT` root/class shapes; model marks `hasArtwork` transient. No tag values or waveform sample payloads are reproduced. No music file tags were read or written in this investigation.

### MIK-E07: key table

Source: copied MIKAnalysis ARM64 notation table at VM `0x440d0`, 24 rows × 7 pointers; `_MIKKeyString` VM `0x8834`, `_MIKKeyNames` `0x851c`, `_MIKIndexForCamelotKey` `0x85ac`. The table was read as data; no vendor routine or analysis service was invoked. Semantic mappings here are factual notation relationships, not reproduced proprietary implementation.

### MIK-E08: cue order and units

Source: snapshot `ZCUEPOINT`; compiled model; MIKData ARM64 `sortedCuePoints` VM `0xb3d8..0xb4fc`, ascending descriptor using CFString at VM `0x18ea0` (`time`). Main ARM64 VM `0x100132334..0x100132390` copies cue time unchanged into the tag cue constructor. Copied MIKTag universal SHA-256 `01d6c510fb1491b9fc2bb1e5c1d8cb70a741f78158c2b66c2dde339676f69c5d`; ARM64 `-[MIKTagID3 setSeratoCuePointsNew:]` VM `0xfce0`, conversion `0xfd9c..0xfdcc`, double `1000.0` at VM `0x5c5b8`. These are bounded static paths, not end-to-end playback validation.
