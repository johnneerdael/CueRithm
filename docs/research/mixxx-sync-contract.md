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
| Installed application | Mixxx 2.5.6 macOS bundle from the project's release (built Mar 25 2026), executable `Contents/MacOS/Mixxx`; run on macOS 15.8 (24H23, arm64) |
| Upstream docs | Mixxx 2.5 manual, chapter *Library*; Serato metadata wiki page |

Paths below are upstream repository paths (`src/...`, `res/...`). "Measured" marks
results produced by running tools on generated files; everything else was read from
source. Source reading does not establish behavior of a running player, so where the
installed Mixxx 2.5.6 could be asked, its own log or entitlements are quoted.

## Platform data locations

Mixxx runs natively on all three desktop systems and keeps one SQLite file,
`mixxxdb.sqlite`, in its settings folder (`src/util/cmdlineargs.cpp`):

| System | Settings folder | Rule |
| --- | --- | --- |
| Linux | `$HOME/.mixxx/` | `MIXXX_SETTINGS_PATH ".mixxx/"` (`CMakeLists.txt`); XDG is deliberately not used |
| Linux, official Flatpak `org.mixxx.Mixxx` | `$HOME/.var/app/org.mixxx.Mixxx/.mixxx/` | `--persist=.mixxx` in `packaging/flatpak/org.mixxx.Mixxx.yaml`; music mounts keep host paths for `~/Music`, `/media`, `/run/media` |
| macOS, official build (App Sandboxed) | `~/Library/Containers/org.mixxx.mixxx/Data/Library/Application Support/Mixxx/` | the same Qt rule, but inside the sandbox container; see below |
| macOS, a build that is not sandboxed | `~/Library/Application Support/Mixxx/` | `QStandardPaths::AppLocalDataLocation`, organization name intentionally unset (`src/main.cpp`) |
| Windows | `%LOCALAPPDATA%\Mixxx\` | same Qt rule |

`--settings-path <dir>` overrides the folder, so manual selection must stay
available.

### The macOS App Sandbox

The released macOS build is an **App Sandbox** application. Evidence from the installed
2.5.6 bundle, not only from source:

- `codesign -d --entitlements -` lists `com.apple.security.app-sandbox`,
  `com.apple.security.assets.music.read-write`,
  `com.apple.security.files.downloads.read-write`,
  `com.apple.security.files.user-selected.read-write` and
  `com.apple.security.files.bookmarks.app-scope`.
- Mixxx's own debug log reports `QStandardPaths::writableLocation(HomeLocation):
  "~/Library/Containers/org.mixxx.mixxx/Data"` and an `AppDataLocation` inside it, and
  looks for a `sandbox.cfg` (its store of security-scoped bookmarks, `src/util/sandbox.cpp`).
- The library of the person whose machine this was measured on is in the container
  (`mixxxdb.sqlite`, `soundconfig.xml`, `analysis`, `controllers`, `effects`), and
  `~/Library/Application Support/Mixxx` does not exist.

What follows for an external tool:

1. **Where the library is.** The container path above comes first; a build that is not
   sandboxed (for example one built from source) uses the plain path.
2. **A settings folder outside the container cannot be used by Mixxx.** Started with
   `--settings-path` pointing outside it, Mixxx 2.5.6 could not create its database
   (`QSqlError("14", "Error opening database", "unable to open database file")`, then
   its dialog "Unable to establish a database connection"), although an ordinary process
   could create a database at the same place. `src/util/sandbox.cpp` on `main`
   (`ensureSettingsPathAccessible`) documents the cause: outside the container, without
   a security-scoped bookmark, the sandbox "acts as a read-only filesystem". Test
   libraries for Mixxx on macOS therefore have to live inside the container.
3. **Which music Mixxx can read.** Only the Music and Downloads folders and what the user
   has chosen in a file dialog (or added to the library, which stores a bookmark).
   Tracks anywhere else are asked for when Mixxx first needs them. CueRithm reports the
   tracks of a sync that fall outside Music, Downloads and Mixxx's own `directories`.
4. **Another app's container.** Reading and writing `mixxxdb.sqlite` there from outside
   Mixxx worked from the command-line tools used for these measurements. macOS also has
   a per-app prompt for access to other apps' data; whether it appears for the CueRithm
   application was **not** tested.

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

What the adapter accepts is decided from the library, not from a list of releases, so
a Mixxx released after this code keeps working as long as it keeps the format.

- **Floor: schema 39.** Anything older predates the beat, key and cue formats written and
  must be opened once in a current Mixxx to migrate.
- **Version.** Read from `settings.name = 'mixxx.schema.version'`
  (`src/database/schemamanager.cpp`).
- **Newer than 40, Mixxx's own rule.** The same table holds
  `mixxx.schema.min_compatible_version`, the oldest software that can still use the
  library, and `mixxx.schema.last_used_version`. `SchemaManager::upgradeToSchemaVersion`
  refuses to open a newer library from software whose schema is below the marker
  (`NewerVersionIncompatible`), and a library that does not store one counts as compatible
  only with itself. The adapter applies that rule with 40, the newest schema it was
  checked against: a library of schema 41 or later whose marker is 40 or lower is accepted
  and the preview says so; a higher marker is refused with a message naming both numbers.
  Every revision from 7 onward has declared `min_compatible="3"`.
- **Shape.** Whatever the version, every column the writer names must exist, and no table
  it inserts into may have a column that is `NOT NULL`, has no default, is not the row id
  and is not one the writer names: a new mandatory column means a row now needs something
  CueRithm does not know how to supply. Optional columns, such as schema 40's
  `library.tuning_frequency_hz`, are written only when present.

What this cannot detect is a release that keeps every column and the marker but changes
what a value means, for instance a new beats format that no longer reads the one written.
That would show only in a run against that release.

A library CueRithm *creates* is always schema 39, which every later Mixxx upgrades itself.

## Tables used

Observed from `res/schema.xml` and the DAOs (`src/library/dao/`):

- `track_locations(id, location UNIQUE, filename, directory, filesize, fs_deleted, needs_verification)`
- `library(...)`, one row per track; `library.location` stores `track_locations.id`
- `cues(id, track_id, type, position, length, hotcue, label, color)`
- `Playlists(id, name, position, hidden, date_created, date_modified, locked)` and
  `PlaylistTracks(id, playlist_id, track_id, position, pl_datetime_added)`
- `crates(id, name UNIQUE, count, show, locked, autodj_source)` and
  `crate_tracks(crate_id, track_id, UNIQUE(crate_id, track_id))`
- `directories(directory UNIQUE)`: folders Mixxx scans. While it is empty Mixxx shows
  "Choose music library directory" at every start (`CoreServices::initialize` in
  `src/coreservices.cpp`: `loadRootDirs().isEmpty()`, which counts rows whether or not
  the folder exists; the `skipInvalidOrMissing` parameter defaults to `false`). A library
  that CueRithm fills would keep asking, so the adapter inserts rows into an **empty**
  list only: the outermost folder of each synced track, because
  `DirectoryDAO::addDirectory` drops a folder that lies inside another one. It skips
  the home folder and drive roots, and never edits a list that has any row.

There are no triggers or persistent views. `crate_summary` is a temporary view that
Mixxx creates at runtime (`src/library/trackset/crate/cratestorage.cpp`).

## Foreign keys

Mixxx opens its database with foreign keys **off** (SQLite's default, and Qt's SQLite
driver does not change it), and its schema could not be written with them on. In a
library created by Mixxx 2.5.6:

- `library.location INTEGER REFERENCES track_locations(location)` names the text column
  `location`, but the column holds `track_locations.id`.
- `cues.track_id`, `PlaylistTracks.track_id` and `crate_tracks.track_id` reference a table
  `"library_old"`, left behind when a migration renamed `library`. No such table exists.

A SQLite build that turns foreign keys on by default, as the one bundled with `rusqlite`
does, fails every one of those inserts (`FOREIGN KEY constraint failed`, or
`no such table: main.library_old`). Writers must run `PRAGMA foreign_keys = OFF`, as Mixxx
does. This was found only by running against a library Mixxx made; a database authored
from the documented contract had hidden it.

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

## Creating a library

A Mixxx that has never been started has no library, and on Windows and Linux no settings
folder. A library CueRithm writes there must be one Mixxx accepts as current, or Mixxx
would start migrating a half-made file.

- **Layout.** The tables, indexes and views Mixxx 2.5.6 creates for a new profile, read from
  `sqlite_master` of a library it made (26 tables; the statements are verbatim, including
  the `library_old` references). Mixxx 2.4 and 2.5 are both schema 39 and create the same
  layout.
- **Rows.** `settings`: `mixxx.schema.version` 39, `mixxx.schema.min_compatible_version` 3,
  `mixxx.schema.last_used_version` 39. `Playlists`: the Auto DJ playlist (`hidden = 1`) and
  `historyPlaceholder` (`hidden = -1`, `locked = 1`), which the schema migrations insert.
  Today's history playlist and `mixxx.db.model.autodj.ColumnsSorting` are written by Mixxx at
  run time and are not created.
- **Version.** Always 39. A newer Mixxx upgrades 39 to 40 itself, as it would any older
  library, so one template serves every supported Mixxx.
- **Licence.** The layout is the structure of a GPL-2.0-or-later program's database, copied
  so that CueRithm can interoperate with it. It is a single file, `schema39.sql`, with its
  provenance in its header.
- **macOS.** The sandboxed download keeps its library in a container that macOS creates
  when the application is first started. Until then there is nothing to write into, and
  CueRithm does not create another application's container.

## Process and database locking

Mixxx sets no SQLite journal pragma, so the file uses the default rollback journal
and a `mixxxdb.sqlite-journal` exists only inside a transaction (or after a crash).
No lock file was found in the source. Publication therefore relies on the existing
process guard: the executable is `Mixxx` (macOS), `mixxx` (Linux, including the
Flatpak command) and `mixxx.exe` (Windows).

## Timing model

Cue and loop positions, and beat positions, are only useful if they land on the same
audio sample in Mixxx as in CueRithm. They do not automatically: CueRithm analyzes the
PCM that FFmpeg decodes (`crates/cuerithm-media/src/decode.rs`), and Mixxx positions
are frames of whatever decoder Mixxx picked for the file, at the file's own sample
rate. The two decoders start the stream in different places for MP3. This section
records which decoder Mixxx uses on each system, what each does at the start of the
file, and the offset the adapter adds to a CueRithm position as a result.

### Which decoder Mixxx uses

Mixxx registers one decoder ("SoundSource provider") per file type and priority, and a
file is opened with the highest-priority provider that supports its type. Read from
`src/sources/soundsourceproxy.cpp` (`registerPlatformAndFallbackSoundSourceProviders`,
`registerProviders`) and each provider's `getPriorityHint`:

| System | MP3 | AAC, M4A, MP4 | WAV, AIFF, FLAC, Ogg, Opus |
| --- | --- | --- | --- |
| macOS | Apple CoreAudio (`Higher`; its list includes `mp3`) | Apple CoreAudio (`Higher`) | libsndfile and the format libraries |
| Windows | MAD / libmad (`Default`) | Media Foundation (`Higher`) | libsndfile and the format libraries |
| Linux | MAD / libmad (`Default`) | FAAD through `SoundSourceM4A` (`Default`) | libsndfile and the format libraries |

On the installed macOS bundle the log confirms the first row: for both `mp3` and
`m4a` Mixxx 2.5.6 lists `4 (higher): "Apple CoreAudio Version 15.8 (Build 24H23)"` and
`1 (lowest): "FFmpeg 6.0"`, and no MAD provider is built in. The Linux and Windows rows
come from the source only.

FFmpeg is registered everywhere it is built, but its hint is `Lowest`
(`SoundSourceProviderFFmpeg::getPriorityHint`), so it only opens a file when nothing
else can. A build without MAD or without CoreAudio changes the row; the adapter
assumes the default build of each system.

CueRithm picks the decoder from the system it runs on, because Mixxx's library is a
file on that same machine. A library copied to another system plays with that system's
decoder and may be misaligned by the differences below.

### macOS: CoreAudio, measured

`SoundSourceCoreAudio::tryOpen` opens the file with `ExtAudioFileOpenURL`, sets a
float client format, and reads the converter's `AudioConverterPrimeInfo`. It then
declares the stream's frames as `IndexRange::forward(leadingFrames, totalFrames)`, so a
sample that `ExtAudioFile` returns at output position `p` is Mixxx frame
`leadingFrames + p`.

`tools/mixxx-acceptance/coreaudio_probe.c` makes those calls on a click track, and
`tools/mixxx-acceptance/decoder_table.py` compares the result with CueRithm's FFmpeg
decode. Measured on macOS 15.8 (24H23, arm64) with `lame` 4.0 and FFmpeg 9.0.2; every
file is one second of silence, a click, then silence. `priming` is the packet-table
value from `AudioFileGetProperty(kAudioFilePropertyPacketTableInfo)`, `-1` when the
file has none; `add` is Mixxx frame minus CueRithm position, in samples:

| File | priming | add |
| --- | ---: | ---: |
| `lame`, constant bitrate, LAME tag | 576 | 0 |
| `lame -V2`, Xing and LAME tag | 576 | 0 |
| `lame`, mono | 576 | 0 |
| FFmpeg `libmp3lame` (tag identifier `Lavc`) | 576 | 0 |
| LAME tag with its CRC broken | 576 | 0 |
| `lame -t`: no Info frame | -1 | **-529** |
| Info frame, identifier replaced with `XXXX`, `GOGO` or `Bolt` | -1 | **-529** |
| Info frame, identifier hand-patched to `Lavf` or `Lavc` | -1 | **+576** |
| AAC in M4A, Apple encoder | 2112 | 0 |
| AAC in M4A, FFmpeg encoder | 1024 | 0 |
| ALAC in M4A | 0 | 0 |
| AAC in ADTS (`.aac`) | -1 | 0 |
| FLAC, WAV | 0 or -1 | 0 |

`leadingFrames` was 0 for every file. What happens is that `ExtAudioFile` discards the
529 samples (528 + 1) of decoder delay that any MP3 decoder produces, plus the
file's priming frames when `AudioFile` found a packet table: the table's valid-frame
count equals the original length exactly (132,300 for the three-second test file).
FFmpeg trims `encoder delay + 529` only when it trusts a `LAME`, `Lavf` or `Lavc` tag,
so the two clocks differ by `FFmpeg trim - CoreAudio trim`. The two rows with
hand-patched identifiers show that Apple does not read tags by the same rule as FFmpeg
(it accepted FFmpeg's own `Lavc63.1` tag and rejected a patched one), and Apple also
reads gapless data from places FFmpeg does not (an `iTunSMPB` comment, for example).
So the adapter does not predict Apple's rule: on macOS it asks `AudioFile` for the
packet table's priming frames, which is what `ExtAudioFile` trims, and adds

`(FFmpeg trim) - (529 + priming)`, where priming is 0 when there is no table.

Every MP3 row above follows from that formula. AAC needs no offset for the Apple and
FFmpeg encoders measured, because both decoders discard the same priming, and the
lossless rows need none because neither decoder trims them.

### Linux and Windows: MAD, read from source and a measured decoder alignment

`SoundSourceMp3` (libmad) drops the first MP3 frame unconditionally, on the assumption
that it is an Info frame, and trims nothing else (`src/sources/soundsourcemp3.cpp`; its
own comment records that files without an Info frame lose real samples, and that the
behavior is kept so existing cues stay valid). FFmpeg drops the Info frame when
present and, when it trusts a tag, trims `encoder delay + 529`.

Measured with `lame` and FFmpeg on a generated 44.1 kHz click track
(`encoder_delay = 576`), the burst onset against its true position. The four files are
synthetic variants of one encode; their names describe the header, and they are **not**
the letters of `cuerithm-media`'s `Mp3TimingCase`, a different heuristic that also
promotes files by channel mode and that the Mixxx rule does not use:

| Synthetic header variant | FFmpeg trim | FFmpeg onset | Mixxx onset (derived) | Mixxx - CueRithm clock |
| --- | ---: | ---: | ---: | ---: |
| No Xing/Info frame | 0 | +1104 | +1104 - 1152 = -48 | -1152 samples (-26.12 ms) |
| Info frame, identifier replaced | 0 | +1104 | +1104 | 0 |
| Info frame, `LAME` identifier, CRC broken | 1105 | -1 | +1104 | +1105 samples (+25.06 ms) |
| Info frame, `LAME` identifier, valid CRC | 1105 | -1 | +1104 | +1105 samples (+25.06 ms) |

FFmpeg does not verify the LAME CRC. A second measurement patched the 4-byte encoder
identifier of a valid tag: `LAME`, `Lavf` and `Lavc` all trimmed 1105 samples, while
`GOGO`, `XING`, four zero bytes and `Bolt` trimmed nothing, and a `Xing` marker trimmed
exactly like `Info`. An untrimmed decode with libmpg123 (`mpg123 --no-gapless`, a
second independent decoder) places the burst at +1104 in all four variants, matching
FFmpeg's untrimmed alignment, so the raw decoder alignment does not depend on the
decoder. The Mixxx column combines that measured alignment with the libmad behavior
read from source; **libmad itself was not run**, and no Windows or Linux system was
used.

The rule, from header facts read from the file:

- no Xing/Info frame: `-F`, where `F` is the first-frame length (1152 samples at
  MPEG-1 rates of 32 kHz and above, 576 below);
- Info/Xing frame without a `LAME`/`Lavf`/`Lavc` identifier: `0`;
- Info/Xing frame with such an identifier: `+(encoder_delay + 529)`, the delay being
  the 12-bit value at frame offset `marker + 141` when the tag carries the usual
  fields (marker at `4 + side_info`; the position follows the Xing flags).

### What the adapter adds

Offsets are in the source file's own frames and apply additively to cue starts, loop
ends and beat positions. A cue that lands before the first Mixxx sample (possible only
for the `-F` and `-529` cases) is moved to the start.

| Format | macOS | Linux and Windows |
| --- | --- | --- |
| MP3 | `FFmpeg trim - (529 + CoreAudio priming)`, asked of CoreAudio | `-F`, `0` or `+(delay + 529)` as above; measured with libmad in Debian's Mixxx 2.5.0 for a LAME file with an Info frame: +1105 |
| AAC, M4A, MP4 | 0, measured for Apple and FFmpeg encoders, and in Mixxx 2.5.6 for AAC and ALAC in M4A | 0 with FFmpeg, measured in Debian's Mixxx 2.5.0 (its only AAC decoder there); **not verified** for Media Foundation (Windows) and FAAD (Linux builds that have it), which were not run; the FAAD path shows no start trim in source (2112 is only a seek prefetch) |
| WAV, FLAC | 0, measured in Mixxx 2.5.6 | 0 |
| AIFF, Ogg Vorbis | 0, measured in Mixxx 2.5.6 (libsndfile, `Xiph.org OggVorbis`) against FFmpeg's decode | 0 |
| Opus | 0, measured in Mixxx 2.5.6 (`Xiph.org libopusfile`) against FFmpeg's decode, counted at 48 kHz: Mixxx always decodes Opus at 48000 Hz and reports that rate for the file | 0; opusfile discards the pre-skip as RFC 7845 requires, as FFmpeg does |
| WMA | not openable: the 2.5.6 download registers no decoder for `wma` (its debug log lists none), so the tracks are added and do not play | 0, plays only if Mixxx was built with FFmpeg (or Media Foundation on Windows); not verified |

The preview warns for each case marked not verified.

### Reproducing

```sh
python3 tools/mixxx-acceptance/decoder_table.py
```

builds the probe, generates every file above and prints the table (macOS with `clang`,
`afconvert`, `ffmpeg` and `lame`). The Linux and Windows rows are not reproducible with
it; a libmad probe, or a run of Mixxx itself, would settle them.

## Representation gaps and chosen conversions

| CueRithm data | Mixxx | Conversion |
| --- | --- | --- |
| Smart playlist | none | export captured members as an ordinary list |
| Playlist folder | none | flatten to `Parent / Child` |
| Record label | no column | not written; map Label into Comment with a field mapping if wanted |
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

## Runtime acceptance

`tools/mixxx-acceptance/run.py` drives the installed Mixxx 2.5.6 (macOS, App Sandboxed
build, CoreAudio) with its own `--settings-path`, inside Mixxx's container, and checks
what is in its database and what its debug log says. It never touches the person's own
library. The last run passed **168 of 168 checks**. Four scenarios, each on its own
library, with three click tracks (WAV, FLAC, MP3):

| Scenario | What was done | What was observed |
| --- | --- | --- |
| fresh | Mixxx made an empty library; CueRithm added the tracks through the real preview and apply path | Mixxx's first start stopped at "Choose music library directory" with an empty `directories`; after the sync the next start did not, the library scan found the same three tracks, and nothing was rewritten |
| existing | Mixxx added the tracks itself; CueRithm replaced grid, key and cues in place | the same row ids; Mixxx deserialized the grids and keys and its analyzer reported `BpmLocked` for all three; rows unchanged after Mixxx's scan |
| overwrite | As existing, then the whole library was replaced | only CueRithm's tracks and playlist remained; the tables, indexes and views, the schema settings and Mixxx's two default playlists equalled what Mixxx itself had made |
| created | No settings folder at all; CueRithm created the library | the same equality with a library Mixxx made; Mixxx started without prompting or upgrading ("Database schema is up-to-date at version 39"), read the grids, and left the rows alone |

In every scenario the written first beat was within 2 frames of CoreAudio's first click
(the probe's threshold is 16), hot cues and the loop were `frames x 2` with the same
shift, and the key was A minor with source *user*. The log lines relied on are
`Successfully deserialized Beats ("BeatGrid-2.0")`, `Successfully deserialized KeyMap`,
`Track is BpmLocked: Beat calculation will not start` and `Database schema is up-to-date`.

Defects the earlier, fixture-only tests had not shown, found by the first runs and fixed:

1. the schema version is stored under `mixxx.schema.version`, not `mixxx.schema_version`,
   so the adapter would have refused every real library;
2. Mixxx's schema declares foreign keys it does not satisfy (see *Foreign keys*), so
   writing with SQLite's foreign keys on failed on every track;
3. a Mixxx started on an empty `directories` waits for a music folder at every start.

What the run does **not** show: Windows, Linux or Flatpak (no such system was available);
Mixxx 2.4, or 2.6 and later, so that Mixxx's own upgrade of a library CueRithm created
from schema 39 to 40 was not run; playback or the waveform inside Mixxx (the grid was
compared with CoreAudio's decoder through the probe, and Mixxx loaded and deserialized
it); a folder CueRithm adds to the scan list outside Music, Downloads and Mixxx's own
container, for which the sandbox is expected to ask for access; the CueRithm application
itself (the sync ran through the same preview and apply code from a test harness, not
from the desktop UI), including any macOS prompt for access to another application's
container. In the *existing* and *overwrite* scenarios the row in `directories` was
inserted by the script, standing in for choosing the folder in Mixxx's dialog.

### Linux: Debian's Mixxx 2.5.0, in a container

`run.py --linux` runs the same scenario against the real Mixxx from Debian 13 (2.5.0,
schema 39, arm64) in a Docker container, on a virtual display (Xvfb) with software OpenGL
and no audio device. Nobody clicks anything: the checks read Mixxx's database and its debug
log, and the folder prompt does not appear because the sync fills the folder list. Mixxx
creates its library at `~/.mixxx` there, as the platform table says. **105 of 105 checks
passed** for MP3, WAV, FLAC, AAC and ALAC in M4A, AIFF, Ogg Vorbis and Opus:

- Mixxx's log lists the decoders: MAD (libmad 0.15.1) for MP3, FFmpeg 7.1 only for AAC and
  M4A (this build has no FAAD decoder), libsndfile for AIFF and WAV, libFLAC, OggVorbis,
  libopusfile. No decoder is registered for WMA.
- **MP3 through libmad.** `mad_probe.c` does what Mixxx's MP3 source does (skip the first
  frame, decode the rest with libmad) and finds the first click at frame 23157 for a LAME
  file whose source click is at 22050. The adapter's Linux timing wrote the grid at 23155, a
  shift of 1105 frames (576 of encoder delay and 529 of decoder delay, read from the LAME
  header), within 2 frames. This is the first measurement of the libmad derivation above.
- AAC, ALAC, AIFF, Ogg Vorbis and Opus land within 2 frames with no shift (the Opus grid
  counted at 48 kHz, as on macOS). Their click positions come from FFmpeg's decode on the
  host, assumed to equal where Mixxx's FFmpeg source and the format libraries start.
- Mixxx started without asking for a folder, found the schema current, deserialized the
  grids and keys, left the BPM-locked grids alone and did not change the rows.

What this does **not** show: the sync itself ran on macOS and only behaved as a Linux system
would (libmad timing, `CUERITHM_MIXXX_HOST=linux`), so CueRithm running on Linux (process
names, folder discovery) is covered by tests and by the repository's Linux checks, not by
this run; Mixxx builds with FAAD (AAC in M4A would not be trimmed, from its source) or other
packaging (AppImage, Flatpak, Ubuntu); Windows, where Media Foundation decodes AAC and no
Windows system was available.

### Formats, measured in Mixxx 2.5.6 on macOS

A fifth scenario, `formats`, runs AAC and Apple Lossless in M4A, AIFF, Ogg Vorbis and Opus
through the installed Mixxx the way the first four run three files: Mixxx adds them,
CueRithm replaces grid, key and cues, and Mixxx starts again. **72 of 72 checks passed.**
Mixxx's debug log gives the decoder it chose for each file type (`Apple CoreAudio` for
M4A, `libsndfile` for AIFF, `Xiph.org OggVorbis`, `Xiph.org libopusfile`); the first click
of each file was found the way that decoder reads it, and every written first beat was
within 2 frames of it (22050 against 22052 at 44.1 kHz, 24000 against 24002 at 48 kHz for
Opus, which Mixxx records as the file's sample rate). The click for M4A comes from the
CoreAudio calls Mixxx makes; for AIFF, Ogg Vorbis and Opus it comes from FFmpeg's decode of
the file, which is assumed to start where libsndfile, libvorbisfile and libopusfile start,
not measured with those libraries. The real application takes each track's rate from its
media probe, which reports 48000 for Opus.

## Not established

- **Linux and Windows.** No Linux or Windows system, and no libmad, Media Foundation
  or FAAD decoder, was run. Their MP3 alignment is a measured raw-decoder alignment
  plus libmad's first-frame drop read from source; their AAC alignment is
  unverified. Path, process and database behavior on those systems was read from
  source and exercised by authored fixtures on macOS only.
- **CoreAudio outside the probe.** The macOS offsets were measured with the CoreAudio
  calls `SoundSourceCoreAudio` makes, on one macOS version (15.8) and one set of
  encoders. Apple can change its tag rules between releases, which is why the adapter
  asks `AudioFile` instead of predicting them. Playback inside the Mixxx application
  is covered separately, in *Runtime acceptance* below.
- **Opus, Ogg Vorbis and AIFF** alignment was measured in Mixxx 2.5.6 on macOS against
  FFmpeg's decode, not against libopusfile, libvorbisfile or libsndfile themselves.
- **WMA** cannot be opened by the macOS download; whether a Linux or Windows build can
  was not run.
- **Mixxx versions** other than schema 39 and 40, and builds without MAD or CoreAudio.
- **Schema 40 against a running Mixxx.** Mixxx 2.5.6 (schema 39) is the only release
  available on the development machine; schema 40 was exercised with a schema-39
  library plus the one column Mixxx's source adds, and the upgrade of a library CueRithm
  created from 39 to 40 by a newer Mixxx was not run.
