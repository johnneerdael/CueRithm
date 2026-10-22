# Mixxx native library contract

Status: researched 2026-10-22 from the public Mixxx source and the installed
macOS release. This records independent format observations. No Mixxx code is
copied; the adapter is an independent Rust implementation of the observed
contract. Runtime acceptance in installed Mixxx builds is recorded separately at
the end and is not implied by any source observation below.

## Evidence boundary

| Source | Identity |
| --- | --- |
| Mixxx source, development line | `main` at `2086fe0e55` (`project(mixxx VERSION 2.7.0)`), schema 40 |
| Mixxx released lines | Tags `2.3.0` (schema 32), `2.4.0` and `2.5.0`/`2.5.6` (schema 39) |
| Installed application | Mixxx 2.5.6 macOS bundle, executable `Contents/MacOS/Mixxx` |
| Upstream docs | Mixxx 2.5 manual, chapter *Library*; Serato metadata wiki page |

Paths below are upstream repository paths (`src/...`, `res/...`). "Measured" marks
results produced by running tools on generated files; everything else was read from
source. Source reading does not establish behavior of a running player.

## Platform data locations

Mixxx runs natively on all three desktop systems and keeps one SQLite file,
`mixxxdb.sqlite`, in its settings folder (`src/util/cmdlineargs.cpp`):

| System | Settings folder | Rule |
| --- | --- | --- |
| Linux | `$HOME/.mixxx/` | `MIXXX_SETTINGS_PATH ".mixxx/"` (`CMakeLists.txt`); XDG is deliberately not used |
| Linux, official Flatpak `org.mixxx.Mixxx` | `$HOME/.var/app/org.mixxx.Mixxx/.mixxx/` | `--persist=.mixxx` in `packaging/flatpak/org.mixxx.Mixxx.yaml`; music mounts keep host paths for `~/Music`, `/media`, `/run/media` |
| macOS | `~/Library/Application Support/Mixxx/` | `QStandardPaths::AppLocalDataLocation`, organization name intentionally unset (`src/main.cpp`) |
| Windows | `%LOCALAPPDATA%\Mixxx\` | same Qt rule |

`--settings-path <dir>` overrides the folder, so manual selection must stay
available. The installed macOS application had not been launched on the host
used here, so no per-user library existed to inspect.

## Schema window

`res/schema.xml` is an append-only migration history. Every revision from 7 onward
declares `min_compatible="3"`, so older Mixxx builds open newer databases; this
means an old Mixxx will not refuse a database written with extra columns, but also
that CueRithm cannot assume a newer schema is safe. Released lines use:

| Mixxx | Schema |
| --- | --- |
| 2.3 | 32 |
| 2.4, 2.5 | 39 |
| development (2.6/2.7) | 40 (adds `library.tuning_frequency_hz FLOAT DEFAULT 0.0`) |

Supported write window: **schema 39 and 40**. Schema is read from
`settings.name = 'mixxx.schema_version'` (`src/database/schemamanager.cpp`).
Anything older must be opened once in a current Mixxx to migrate; anything newer
is read-only until reviewed. Column availability is checked with
`PRAGMA table_info`, and the writer names only columns that exist, so the optional
schema-40 column is written only when present.

## Tables used

Observed from `res/schema.xml` and the DAOs (`src/library/dao/`):

- `track_locations(id, location UNIQUE, filename, directory, filesize, fs_deleted, needs_verification)`
- `library(...)`, one row per track; `library.location` stores `track_locations.id`
- `cues(id, track_id, type, position, length, hotcue, label, color)`
- `Playlists(id, name, position, hidden, date_created, date_modified, locked)` and
  `PlaylistTracks(id, playlist_id, track_id, position, pl_datetime_added)`
- `crates(id, name UNIQUE, count, show, locked, autodj_source)` and
  `crate_tracks(crate_id, track_id, UNIQUE(crate_id, track_id))`
- `directories(directory UNIQUE)`: folders Mixxx scans. The adapter never edits it.

There are no triggers or persistent views. `crate_summary` is a temporary view that
Mixxx creates at runtime (`src/library/trackset/crate/cratestorage.cpp`).

## Track identity and insertion

`TrackDAO::addTracksAddTrack` inserts a `track_locations` row, then a `library` row
with `location` set to that id (`src/library/dao/trackdao.cpp`, around 429–730).
`track_locations.location` is `FileInfo::location()`, the canonical absolute path
with forward slashes on Windows (`C:/Users/me/Music/a.mp3`, UNC as
`//server/share/...`). `directory` is its parent path, `filename` the final
component and `filesize` the byte length.

The row key is the exact string. A case-insensitive volume (default macOS and
Windows) can therefore hold two spellings of one file; the adapter matches
case-insensitively on those systems and refuses an ambiguous match instead of
inserting a duplicate.

A new `library` row also needs `duration`, `samplerate`, `channels`, `bitrate`,
`filetype` (schema 39 normalizes `aif` to `aiff`) and `datetime_added`. Defaults
the loader tolerates: `replaygain 0`, `replaygain_peak -1.0`, `timesplayed 0`,
`played 0`, `mixxx_deleted 0`, `coverart_source/type 0`, `tracktotal '//'`
(re-read from the file on first access), `cuepoint -1.0` (no main cue).

A row with `mixxx_deleted = 1` is a track the user removed from the library.
Re-adding it, which Mixxx itself does through "unremove", only clears that flag.

## Tag re-import hazard and the safe column state

When a track is loaded Mixxx may re-read file tags and replace library values
(`TrackDAO::getTrackById`, `SoundSourceProxy::updateTrackFromSource`). The decision
is `TrackRecord::checkSourceSyncStatus` (`src/track/trackrecord.cpp`):

| `header_parsed` | `source_synchronized_ms` | Status | Effect on load |
| --- | --- | --- | --- |
| 0 | any | Void | Initial import: tags replace library values; Serato markers also import |
| 1 | NULL | Unknown | Never re-imported implicitly ("to prevent overwriting precious user data") |
| 1 | set, older than file mtime | Outdated | Re-imported only when the user enabled *Track Metadata Synchronisation* |
| 1 | set, not older | Synchronized | No import |

The adapter therefore writes `header_parsed = 1` and leaves
`source_synchronized_ms` NULL on rows it creates, and never touches the field on
existing rows. The manual states that re-importing Serato metadata "will clear your
existing cuepoints in Mixxx if the file contains Serato Metadata", which is the
case this state avoids.

## Beats

`library` columns `beats` (BLOB), `beats_version`, `beats_sub_version`, `bpm`,
`bpm_lock`. `Beats::fromByteArray` (`src/track/beats.cpp`) accepts:

| `beats_version` | Blob | Use |
| --- | --- | --- |
| `BeatGrid-2.0` | proto `BeatGrid { Bpm bpm = 1; Beat first_beat = 2 }` | constant tempo |
| `BeatMap-1.0` | proto `BeatMap { repeated Beat beat = 1 }`, at least 2 beats | variable tempo |

`Beat.frame_position` is `int32` in frames at the file's own sample rate, not at a
fixed 44.1 kHz. `Beats::toByteArray` writes a grid when the tempo is constant and a
map otherwise, which defines which representation Mixxx would produce itself. The
`bpm` column is the average over the track (`getBpmInRange`). `beats_sub_version`
is analyzer metadata (`rounding=V4` plus plug-in info); an external writer leaves it
empty.

Analyzer protection (`src/analyzer/analyzerbeats.cpp`): a track with `bpm_lock = 1`
is never re-analyzed. Without the lock, existing beats are still kept unless the
user enabled "re-analyze when settings change" or "re-analyze imported". The
CueRithm `BeatgridLock` option therefore has direct meaning for Mixxx.

## Keys

`keys` (BLOB), `keys_version = 'KeyMap-1.0'`, `keys_sub_version`, `key` (text) and
`key_id` (integer `ChromaticKey`). Blob is proto `KeyMap { global_key = 1,
global_key_text = 2, repeated KeyChange = 3, source = 4, global_tuning_frequency_hz = 5 }`
(`src/proto/keys.proto`), with `source` 0 analyzer, 1 file metadata, 2 user.
`ChromaticKey` ids: major `C…B` = 1–12 (`C, Db, D, Eb, E, F, F#, G, Ab, A, Bb, B`),
minor `Cm…Bm` = 13–24 (`Cm, C#m, Dm, Ebm, Em, Fm, F#m, Gm, G#m, Am, Bbm, Bm`).
`global_key_text` and the `key` column use the ID3v2 spelling from
`src/track/keyutils.cpp`. The loader prefers the blob when `keys_version` is set and
falls back to the text only for pre-1.12 data. `AnalyzerKey::shouldAnalyze` skips a
track whose key is valid unless the user enabled key re-analysis.

## Cues

`CueDAO` (`src/library/dao/cuedao.cpp`) is the authority.

| Column | Contract |
| --- | --- |
| `type` | 0 invalid, 1 hotcue, 2 main cue, 3 beat (unused), 4 loop, 5 jump, 6 intro, 7 outro, 8 audible-sound range |
| `position` | REAL **engine sample position**: frames × 2 output channels. `-1.0` means invalid |
| `length` | REAL, frames × 2; loops only |
| `hotcue` | slot index, `0..35` (`kMaxNumberOfHotcues = 36`); `-1` for none |
| `label` | non-null text; empty reads as no label |
| `color` | integer `0xRRGGBB`, NOT NULL |

The loader discards a loop whose length is 0, a hotcue at position 0 with black
colour, and the second of two hotcues sharing a number. The writer must avoid all
three. Saved loops use `type 4` with a hotcue slot. A loop has no persistent
"activate on load" state, so CueRithm's `active_loop` cannot be represented.
`library.cuepoint` mirrors the main cue and is not touched by the adapter.

The default hotcue palette (`src/util/color/predefinedcolorpalettes.cpp`) is red
`C50A08`, green `32BE44`, celeste `42D4F4`, yellow `F8D200`, blue `0044FF`,
purple `AF00CC`, pink `FCA6D7`, white `F2F2FF`. Any stored RGB value is shown.

## Playlists and crates

| Property | Playlists | Crates |
| --- | --- | --- |
| Order | ordered, 1-based `PlaylistTracks.position`, duplicates allowed | unordered, no duplicates |
| Nesting | none | none |
| Smart rules | none | none |
| Name | not unique | `UNIQUE` |
| Lock | `locked` | `locked` |

A user playlist has `hidden = 0`; `1` is the Auto DJ queue and `2` a set-log
(history) playlist, neither of which the adapter may touch. Mixxx creates a
playlist at `max(position) + 1` and appends tracks at `max(position) + 1`
(`src/library/dao/playlistdao.cpp`). The manual confirms there are no folders and no
smart crates (smart crates are mentioned only as unsupported for Serato imports).

Consequences for CueRithm data: folders flatten to a path-like name, and smart
playlists export their captured members as ordinary lists.

## Process and database locking

Mixxx sets no SQLite journal pragma, so the file uses the default rollback journal
and a `mixxxdb.sqlite-journal` exists only inside a transaction (or after a crash).
No lock file was found in the source. Publication therefore relies on the existing
process guard: the executable is `Mixxx` (macOS), `mixxx` (Linux, including the
Flatpak command) and `mixxx.exe` (Windows).

## Timing model

CueRithm's canonical clock is the FFmpeg-decoded stereo 44.1 kHz PCM
(`crates/cuerithm-media/src/decode.rs`); Mixxx positions are frames at the file's own
rate. Seconds convert to frames with the source sample rate. The remaining
difference is how each decoder trims the start.

Mixxx's libmad MP3 source drops the first MP3 frame unconditionally and applies no
other trim (`src/sources/soundsourcemp3.cpp`; its comment records that files without
an Info frame lose real samples). FFmpeg drops the Info frame when present and, when
it finds a LAME tag, also trims `encoder_delay + 529` samples.

Measured with `lame` and FFmpeg 9.0.2 on a generated 44.1 kHz click track
(`encoder_delay = 576`), the burst onset against its true position:

| MP3 header case (existing classifier) | FFmpeg trim | FFmpeg onset | Mixxx onset (derived) | Mixxx − CueRithm clock |
| --- | ---: | ---: | ---: | ---: |
| A: no Xing/Info frame | 0 | +1104 | +1104 − 1152 = −48 | −1152 samples (−26.12 ms) |
| B: Info frame, no LAME identifier | 0 | +1104 | +1104 | 0 |
| C: LAME identifier, CRC mismatch | 1105 | −1 | +1104 | +1105 samples (+25.06 ms) |
| D: LAME identifier, valid CRC | 1105 | −1 | +1104 | +1105 samples (+25.06 ms) |

FFmpeg does not verify the LAME CRC, so C and D trim alike. An untrimmed decode with
libmpg123 (`mpg123 --no-gapless`, a second independent decoder) places the burst at
+1104 in all four cases, matching FFmpeg's untrimmed alignment, so the raw decoder
alignment does not depend on the decoder. The Mixxx column combines that measured
alignment with the libmad first-frame behavior read from source; libmad itself was
not run. The offsets apply additively to cue starts, loop
ends and beat positions, in the source file's own frames. For C and D the trim is
`encoder_delay + 529` read from the LAME header rather than assumed to be 576; the
first-frame length is 1152 samples for MPEG-1 rates (≥ 32 kHz) and 576 below.

Other formats: lossless/PCM and Ogg (libsndfile, Ogg Vorbis) have no start trim;
Opus uses opusfile, which applies the pre-skip as FFmpeg does. Offset 0.

AAC/M4A is **not resolved**. The Linux FAAD backend and the Windows Media Foundation
backend use 2112 only as a seek prefetch and show no start trim, while the macOS
CoreAudio backend uses ExtAudioFile. Alignment therefore may differ per platform;
the adapter writes offset 0 and warns for AAC/M4A/MP4 sources rather than claim an
aligned result.

## Representation gaps and chosen conversions

| CueRithm data | Mixxx | Conversion |
| --- | --- | --- |
| Smart playlist | none | export captured members as an ordinary list |
| Playlist folder | none | flatten to `Parent / Child` |
| Record label | no column | not written; map it to comment or grouping with field mappings |
| Active loop flag | none | not written |
| Cue slots above 35 | none | skipped with a warning |
| Variable tempo | `BeatMap-1.0` | dense beat list built from the sparse anchors |
| Album art | read from tags/folder at display time | not written; `coverart_*` keep defaults |
| Key notation choice | stored as `ChromaticKey`, shown per Mixxx preference | notation setting is ignored |

## Other transfer routes evaluated

| Route | Finding | Decision |
| --- | --- | --- |
| Serato file tags | Mixxx reads `Markers2`, `Markers_` and `BeatGrid` from ID3/FLAC/MP4 tags, not Ogg; applied only on first import of a track, and clears Mixxx cues on re-import. CueRithm's Serato target writes the Serato 4 database, not tags | Rejected as the primary route: it modifies source music and is first-import-only |
| Playlist files | Mixxx imports M3U/PLS as playlists/crates | Possible later fallback; carries no cues, grids or keys |
| Rekordbox USB export | Mixxx's Rekordbox feature reads `export.pdb`/ANLZ and imports cues when a track is played from the drive | Not a library sync: needs a drive and acts per played track |
| Native database | direct, atomic, covers everything | **Chosen** |

## Not established

- No installed Mixxx was run against an adapter-written library at the time of
  this document. See the acceptance record in the implementation notes.
- libmad was not executed; its alignment is the measured raw decoder alignment
  plus the first-frame drop read from source.
- AAC alignment on each platform backend.
- Behavior of Mixxx versions other than schema 39 and 40.
- Windows and Linux hosts: path, process and database behavior was read from
  source and exercised by authored fixtures on macOS only.
