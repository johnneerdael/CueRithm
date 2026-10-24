# Mixxx library import

Research profile: source commit `2086fe0e551b9eb9dd083dd706a5848eb06534d0` from `mixxxdj/mixxx`, inspected 2026-10-23. This is a development-source profile, not a blanket promise for every Mixxx release. **Seen** means read directly in this pinned source. **Inferred** means proposed CueRithm behavior. No Mixxx app was run or user library modified. The companion `fixtures/mixxx-schema-reference.json` records exact column facts from all 40 schema revisions; `mixxx-wire-examples.json` supplies invented decoding vectors.

## Feasibility and boundary

**Seen:** Mixxx persists its library as `mixxxdb.sqlite` (`src/database/mixxxdb.cpp:29`). Metadata, paths, playlists, crates and cues are relational; beats and keys use explicitly versioned protobufs. Reading is feasible without the installed application and does not require leanrea or a commercial runtime. Reading does require proper unit conversion and schema/version checks; an existing writer's SQL is not sufficient evidence that an importer preserves all fields.

**Seen:** The working checkout is an older feature branch, but current local `main` at `190f2f549454e5c6e8486e4949042197351ea14f` already contains `crates/cuerithm-dj-sync/src/mixxx.rs` and its `mixxx/` modules. Reuse its database discovery, protobuf writer facts, key mapping, schema inspection, timing probes and fixtures. It is an outbound adapter, not the requested inbound library importer. Its `docs/research/mixxx-sync-contract.md` records prior app acceptance; those are existing records, not new tests performed in this research. See `cuerithm-integration.md`. Preserve the existing exporter.

## Discovery, schema and snapshot

**Seen:** `src/util/cmdlineargs.cpp:73-103` keeps Linux settings under the build's `MIXXX_SETTINGS_PATH` (normally `.mixxx/`, `CMakeLists.txt:2083`) and otherwise uses Qt AppLocalDataLocation. `src/util/sandbox.cpp:451-456` documents macOS sandbox and legacy migration paths. Probe user-selected settings directory first, then macOS `~/Library/Application Support/Mixxx`, sandbox `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx`, Linux `~/.mixxx`, and the Windows Qt app-local directory. These are candidates; portable/custom `--settings-path` installs require explicit selection. Do not conclude an empty default path means an empty user library.

**Seen:** The schema version is stored in `settings`, key `mixxx.schema.version`, with `mixxx.schema.last_used_version` and `mixxx.schema.min_compatible_version` (`src/database/schemamanager.cpp:25-27`). `PRAGMA user_version` alone is not Mixxx's schema discriminator. Source schema has revisions 1…40; its last revision has `min_compatible=3`. That compatibility is Mixxx's migration promise, not proof an external parser understands all versions.

Take a read-only SQLite backup including WAL-visible committed data. Inspect only music-related tables and these allowlisted schema keys. Record a local library-instance identity, version, exact table/column/type/default fingerprint and blob codec versions. A library row ID is stable inside that library, not globally unique. Do not run Mixxx migrations on the source or silently promote a copied database to a newer schema. Reject writes entirely in this read adapter.

## Relational extraction

Use explicit column projection from the detected schema; SQLite type affinity is not a validation rule. Core join (a query specification, not production code):

```sql
SELECT l.id AS source_track_id, l.artist, l.title, l.album,
       l.album_artist, l.year, l.genre, l.composer, l.grouping,
       l.tracknumber, l.tracktotal, l.comment, l.url,
       l.duration, l.bpm, l.bpm_lock, l.key, l.key_id,
       l.samplerate, l.channels, l.rating, l.color,
       l.mixxx_deleted, l.timesplayed, l.last_played_at,
       t.location, t.fs_deleted, t.filesize
FROM library AS l
LEFT JOIN track_locations AS t ON t.id = l.location
ORDER BY l.id;
```

**Seen:** Modern `library.location` references `track_locations.id`; revision 3 replaced the original text location (`res/schema.xml:112-145`). Keep `LEFT JOIN` so broken references become typed issues rather than silently disappearing. Preserve hidden/deleted records in observations; default active import excludes `mixxx_deleted!=0` and records that count. `fs_deleted` is media availability, not a request to delete metadata. `needs_verification` is a scanner state, not a guarantee that audio exists.

**Seen:** Descriptive fields listed above are stored directly in `library`. `year`, track numbers and totals are text; do not force every string into an integer. Keep `replaygain`, `replaygain_peak`, cover art source/type/location/hash/digest/color, `datetime_added`, play count, last played, source synchronization timestamp and tuning frequency as distinct optional fields. Technical duration is seconds in the track model; samplerate is audio frames/second. Waveform/track-analysis blobs are not required to import tags/cues and must not be relabeled as audio or beatgrids.

**Inferred:** Preserve raw rating and color; expose an explicit scale conversion if CueRithm needs another range. No label/remixer/lyricist fields appear in this schema profile: represent them unavailable, not empty values that erase data from another provider. `track_locations.directory` and `directories.directory` describe filesystem roots; they do not form a nested playlist model.

## Cue decoding: the factor-of-two trap

**Seen:** `src/library/dao/cuedao.cpp:39-62,178-182` converts stored `position` with `FramePos::fromEngineSamplePosMaybeInvalid` and divides stored `length` by `kEngineChannelOutputCount`. `src/engine/engine.h:7-8` fixes engine output to stereo. `src/audio/frame.h:39-40,81-85` therefore establishes:

- `position == -1` means missing start; otherwise `start_seconds = position / (2 * library.samplerate)`.
- `length_frames = length / 2`. If length is zero, the end is absent. If start exists and length is nonzero, `end_seconds = start_seconds + length/(2*samplerate)`.
- If start is missing and length is nonzero, end is measured from track origin: `end_seconds = length/(2*samplerate)` (`src/track/cue.cpp:62-67`). This represents an end-only range and must survive.
- The divisor is **2 regardless of source file channel count**. A mono or multichannel source still uses stereo engine units here.
- Read SQLite values as real/double even though the declaration says INTEGER. Fractional positions are valid; zero is valid; negative pre-roll except sentinel `-1` is a real position.

**Seen:** Types from `src/track/cueinfo.h:12-22`: `0 invalid`, `1 hot cue`, `2 main cue`, `3 beat (unused)`, `4 loop`, `5 jump`, `6 intro`, `7 outro`, `8 audible-sound range`. Preserve source kind; types 6/7 are Auto DJ ranges, not arbitrary structural labels. `hotcue=-1` is no slot; numbered slots start at 0 (`cueinfo.h:33`), not 1. Keep sparse indices, label and ID. Do not infer the maximum hotcue count from a skin.

**Seen:** `CueDAO` discards a loop with zero length, and a black hot cue exactly at time zero as known historical corruption; it also drops a prior duplicate slot during loading. For a lossless reader, retain raw rows and issue explicit diagnostics. Do not adopt query-order-dependent duplicate resolution as a new CueRithm rule; require a conflict decision. Preserve a valid colored cue at zero.

**Seen:** Current RGB colors are packed 24-bit `0xRRGGBB` (`src/util/color/rgbcolor.h:12-27`). Schema revisions convert legacy palette codes and mask alpha; the original default is an ARGB integer. Decode by schema revision and preserve raw values; blindly masking old palette indices miscolors old libraries. Cue flags imported transiently from Serato are explicitly **not persisted** in this DB (`cueinfo.h:80-83`); do not claim to recover a loop-lock flag from `cues`.

## Beatgrid and variable tempo codecs

**Seen:** `library.beats`, `beats_version`, `beats_sub_version` are loaded by `TrackDAO::setTrackBeats` (`src/library/dao/trackdao.cpp:1300-1325`). `Beats::fromByteArray` dispatches exact strings `BeatGrid-1.0`, `BeatGrid-2.0`, `BeatMap-1.0` (`src/track/beats.cpp:253-338`, constants `beats.h:15-17`). **The protobuf positions are audio frames, unlike cue DB positions.** Divide them by samplerate only.

Wire reference from `src/proto/beats.proto` (proto2):

| Message | Field number / wire type | Meaning |
| --- | --- | --- |
| Beat | 1 / varint int32 | `frame_position` |
| Beat | 2 / varint bool | enabled, default true |
| Beat | 3 / varint enum | source: analyzer=0, file metadata=1, user=2 |
| Bpm | 1 / fixed64 double | BPM |
| Bpm | 2 / varint enum | source, same values |
| BeatGrid | 1 / length-delimited Bpm | constant tempo |
| BeatGrid | 2 / length-delimited Beat | first beat anchor |
| BeatMap | repeated 1 / length-delimited Beat | explicit ordered beats |

Proto2 optional presence is significant. Standard int32 uses protobuf signed semantics, not zigzag `sint32`. Decode unknown fields without losing the original bytes. `BeatGrid-1.0` also has a fallback raw native struct of two doubles `{bpm, firstBeat}` exactly 16 bytes, attempted after protobuf parsing fails (`beats.cpp:17-20,291-305`). Native endianness is not self-described; x86/arm macOS fixtures are little-endian, but do not guess for unidentified historical platforms.

A valid constant grid gives `t(n)=first_frame/samplerate + n*60/bpm`. It provides a beat anchor, not bar phase. A map needs at least two strictly increasing finite beat positions. Preserve all beats for variable tempo; do not replace it with average BPM. The inspected loader reads every BeatMap position and does not consult each beat's enabled/source flags, but retain those fields in observations.

When blobs fail, Mixxx may build a temporary zero-offset grid from scalar BPM before analysis. **Do not copy that fallback as imported truth**: report `grid_decode_failed`, preserve raw bytes, and import scalar BPM independently. A missing/invalid sample rate blocks timing conversion, not the rest of metadata. Do not assume 44.1kHz.

## Key codec and Camelot mapping

**Seen:** `keys_version=KeyMap-1.0`; `keys_sub_version` is metadata. Blob schema is `src/proto/keys.proto`, dispatched in `src/track/keyfactory.cpp:24-32` and parsed in `src/track/keys.cpp:56-63`.

| KeyMap field | Number / wire | Mapping |
| --- | --- | --- |
| global_key | 1 / varint enum | enum below, default invalid=0 |
| global_key_text | 2 / string | preserve source spelling/notation |
| key_change | repeated 3 / message | frame_position field1 int32, key field2 enum, tuning_frequency_hz field3 double |
| source | 4 / enum | analyzer0 / file metadata1 / user2 |
| global_tuning_frequency_hz | 5 / double | 0 means not detected, not proof of measured A440 |

Chromatic enum mapping (each list is C, Db/C#, D, Eb, E, F, F#/Gb, G, Ab/G#, A, Bb, B): major IDs 1…12 map to Camelot `8B,3B,10B,5B,12B,7B,2B,9B,4B,11B,6B,1B`; minor IDs 13…24 map to `5A,12A,7A,2A,9A,4A,11A,6A,1A,8A,3A,10A`. Invalid0 stays unknown. These enum values are chromatic order, **not Camelot numbers**.

**Seen:** `TrackDAO::setTrackKey` uses a valid versioned blob when present, and falls back to legacy `library.key` text when version is empty (`trackdao.cpp:1328-1347`); `key_id` is a cached global key. **Inferred:** Compare blob/text/cache, preserve all raw values, flag conflicts, and use successfully decoded known-version blob as primary. Unknown versions preserve text/cache as independent evidence, never invent an analysis result. Key-change positions are frames/samplerate; keep them as source extension data, not CueRithm structural cues. Preserve tuning offsets and global tuning separately from pitch-class notation.

## Playlists, crates and roots

**Seen:** Read `Playlists` metadata, then `PlaylistTracks` ordered by `position` (`playlistdao.cpp:154-161`). Membership rows have IDs and allow the same track more than once. Preserve occurrences, including duplicate track IDs; break invalid equal-position ties deterministically by row ID and report ambiguity. Playlist `position` is collection display order; membership `position` is track order. Hidden types are normal0, AutoDJ1, set/history log2 (`playlistdao.h:19-23`); history/AutoDJ are opt-in.

**Seen:** `crates` and `crate_tracks` are distinct from playlists. Crate membership has unique `(crate_id,track_id)` and **no sequence column**; preserve as unordered membership, show deterministic display order without claiming source order. Keep `locked`, `show`, `autodj_source` and source IDs. No folder-parent column exists in this profile's playlist/crate tables; do not parse separators in names as hierarchy unless a future supported profile explicitly defines that encoding. Import filesystem roots from `directories` only as source organization references; do not automatically register them as CueRithm watched Incoming folders.

## Native-clock to CueRithm-clock conversion

The formulas above recover seconds on **Mixxx's native decoded-audio timeline**. They do not prove alignment with CueRithm's FFmpeg PCM timeline. **Seen:** current CueRithm main `crates/cuerithm-dj-sync/src/timing.rs:19-42,168-203` and `mixxx/decoder.rs:55-77` add a codec/decoder offset on export. For a validated matching source profile, import uses `canonical_seconds = native_seconds - offset_seconds` exactly once. Keep both values and the profile identity. Do not call the exporter and add the offset a second time.

For MP3, the exporter computes offset frames as `ffmpeg_trim - dropped_frame_samples` for MAD, where dropped is zero if an Info/Xing frame is present, otherwise the first frame sample count. For CoreAudio it is `ffmpeg_trim - (529 + Apple_reported_priming_or_zero)`. Those are existing implementation facts, not universal constants for every source profile. The application that last edited a portable Mixxx DB may have used a different OS/decoder from the machine importing it. Ask for the originating environment when it cannot be established; store native timing unresolved instead of silently picking the current host. Non-MP3 zero offsets in the exporter have measured and unmeasured boundaries in its research document; do not broaden them into an inbound acceptance claim for WMA, FAAD or Windows AAC.

A store-as-imported metadata scan must not trigger full analysis merely to obtain timing. It may inspect a small media header/probe with a recorded decoder profile; if media is missing or clock identity is unknown, keep cues/grids with `clock_status=unresolved_native` and expose the limitation. Conversion/clamping in an old export is sometimes lossy: a cue clamped to zero cannot be inverted to its original negative canonical value. Preserve observed zero and report that limitation.

## Selectable groups and acceptance

Support independent tags/key/BPM/grid/hotcues/main cues/loops/intro-outro ranges/ratings/colors/history/collections switches. Unavailable fields must be disabled, not fabricated. Store-as-imported creates no analysis/tagging/write jobs. The shared sync contract governs stale source checks, partial scans, conflict baselines and path mapping.

Required tests without Mixxx: instantiate a synthetic schema40 DB by executing the pinned `res/schema.xml` migrations in order (use the schema reference to verify columns/indexes/FKs); join missing location rows; decode all fixture vectors; verify factor-of-two independently at 44.1/48/96kHz and mono/stereo channels; end-only ranges; zero-time cues; sparse/duplicate slots; all 24 keys; unknown versions; 16-byte legacy grid; variable tempo; corrupt/truncated protobuf; duplicate playlist occurrences; unordered crates; deleted/missing rows and partial scans. A new importer may be implemented against these source contracts without installing Mixxx. Runtime release acceptance still requires a disposable library opened/saved by each promised stable Mixxx version, followed by independent CueRithm read-back. No such runtime test was performed here.

## Reproducible primary source

All paths above are relative to [Mixxx at the pinned commit](https://github.com/mixxxdj/mixxx/tree/2086fe0e551b9eb9dd083dd706a5848eb06534d0). Critical files: [schema history](https://github.com/mixxxdj/mixxx/blob/2086fe0e551b9eb9dd083dd706a5848eb06534d0/res/schema.xml), [CueDAO](https://github.com/mixxxdj/mixxx/blob/2086fe0e551b9eb9dd083dd706a5848eb06534d0/src/library/dao/cuedao.cpp), [frame conversion](https://github.com/mixxxdj/mixxx/blob/2086fe0e551b9eb9dd083dd706a5848eb06534d0/src/audio/frame.h), [beat codecs](https://github.com/mixxxdj/mixxx/blob/2086fe0e551b9eb9dd083dd706a5848eb06534d0/src/track/beats.cpp), [beat wire schema](https://github.com/mixxxdj/mixxx/blob/2086fe0e551b9eb9dd083dd706a5848eb06534d0/src/proto/beats.proto), [key wire schema](https://github.com/mixxxdj/mixxx/blob/2086fe0e551b9eb9dd083dd706a5848eb06534d0/src/proto/keys.proto). These are source evidence, not vendor app acceptance records.
