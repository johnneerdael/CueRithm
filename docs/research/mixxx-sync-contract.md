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
  library, so one template serves every supported Mixxx. Mixxx `main` (schema 40) did exactly
  that to a library CueRithm created, and the result equalled one Mixxx created itself
  (*Linux: Mixxx `main`, built from source*, below).
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
| Linux | MAD / libmad (`Default`) | FAAD through `SoundSourceM4A` (`Default`) when Mixxx is built with it, otherwise FFmpeg | libsndfile and the format libraries |

On the installed macOS bundle the log confirms the first row: for both `mp3` and
`m4a` Mixxx 2.5.6 lists `4 (higher): "Apple CoreAudio Version 15.8 (Build 24H23)"` and
`1 (lowest): "FFmpeg 6.0"`, and no MAD provider is built in. The Linux row was read from
two real builds' logs: Debian's Mixxx 2.5.0 lists MAD for `mp3` and only FFmpeg for `m4a`
(its build has no FAAD), and a Mixxx `main` built from source with FAAD lists
`"Nero FAAD2"` for `m4a`. `SoundSourceM4A` supports `m4a` and `mp4` only, so AAC in ADTS
never reaches FAAD, and FAAD does not open Apple Lossless (Mixxx falls back to FFmpeg).
The Windows row comes from the source only.

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
read from source at this point; libmad itself was run later, in the Linux and Windows runs
under *Runtime acceptance* (the +1105 frames above, on libmad 0.15.1 and 0.16.4).

The rule, from header facts read from the file:

- no Xing/Info frame: `-F`, where `F` is the first-frame length (1152 samples at
  MPEG-1 rates of 32 kHz and above, 576 below);
- Info/Xing frame without a `LAME`/`Lavf`/`Lavc` identifier: `0`;
- Info/Xing frame with such an identifier: `+(encoder_delay + 529)`, the delay being
  the 12-bit value at frame offset `marker + 141` when the tag carries the usual
  fields (marker at `4 + side_info`; the position follows the Xing flags).

### AAC under FAAD, measured

An earlier reading of `SoundSourceM4A` found no trim at the start of the stream (2112 is
only the number of frames it decodes ahead when it seeks) and concluded that AAC needs no
offset anywhere. The measurement disproved that for one encoder. `faad_probe.c` does what
`SoundSourceM4A` does (libmp4v2 to read the samples, FAAD to decode each from the first)
and finds the first click of the same one-second click files. It is the same kind of
evidence as `mad_probe.c`: the probe follows the source's steps with the same libraries
and was not compared with the position Mixxx itself reports. Frames are at 44.1 kHz, the
click is at 22050 in every source (the detector reads every file 2 frames after that,
the same in every row), and `FAAD shift` is FAAD's click minus FFmpeg's:

| M4A | Priming | Written beat | Click read | FAAD shift |
| --- | ---: | ---: | ---: | ---: |
| AAC from FFmpeg's encoder (edit list: media time 1024) | 1024 | 22050 | 22052 (FAAD) | 0 |
| AAC from Apple's encoder (`afconvert`, `iTunSMPB` 2112, no edit list) | 2112 | 23138 | 23140 (FAAD) | **1088** (24.7 ms) |
| Apple Lossless | n/a | 22050 | 22052 (FFmpeg: FAAD does not open it) | 0 |

What it shows: FAAD returns the music `priming - 1024` frames later than FFmpeg and
CoreAudio do, which both drop the whole priming. Two encoders fit that line (1024 and
2112); no other priming value was measured, so it is a two-point model, not a derived
rule. It is consistent with FAAD keeping every decoded block except the first, but that
reading of the decoder is not proven by this run. The priming is what FFmpeg trims: the
first edit of the track's edit list (`elst`) whose media time is not negative, converted
from the track's own timescale (`mdhd`) to frames at the sample rate its audio entry
declares (the same number in the usual case), or, when the file has no edit list, the
second number of Apple's `iTunSMPB` tag (`0x840` = 2112), which already counts samples.
HE-AAC (with spectral band replication) decodes differently in FAAD and was not measured.

The adapter adds `priming - 1024` for AAC in M4A or MP4 only when the system is Linux and
Mixxx decodes AAC with FAAD. CoreAudio and FFmpeg already drop the priming, so they need
nothing. Linux builds differ (Debian's has no FAAD, Mixxx's own has), so the adapter reads
the installed `mixxx` executable: it looks in `PATH`, `/usr/bin`, `/usr/local/bin`,
`/opt/mixxx/bin` and the Flatpak export folders, and a `libfaad` or `libmp4v2` name in an
ELF file means FAAD. Every installation found is read, and the result is a decoder only
when they all have it: the timing step is not told which installation's library is being
synced, and a native Mixxx and a Flatpak one can each have a library, so installations that
differ give unknown. A missing candidate is passed over, but a wrapper script that exists,
or any file that cannot be read as an ELF executable, also gives unknown, because the Mixxx
it starts is not known. No executable found is the same. Unknown means no shift is applied
and the preview says how late the music would play if the build did use FAAD. When
a shift is applied, the preview says how many tracks were moved and by how many
milliseconds. AppImage and Flatpak packages were not run, so their detection is by those
paths only.

### AAC under Media Foundation, measured

Mixxx on Windows registers Media Foundation (`10.0.17763.9121` in its log) for `aac`, `m4a`
and `mp4`. `mf_probe.cpp` reads a file the way `SoundSourceMediaFoundation` does (the
source reader asked for 32-bit float PCM, frame N being the Nth frame it delivers) and
finds the first click of the same one-second click files; like the other probes it follows
the source's steps and was not compared with a position Mixxx itself reports. Frames are
at 44.1 kHz, the click is at 22050 in every source and the detector reads each file 2
frames after it (22052 in the FFmpeg row), so `MF shift` is Media Foundation's click minus
FFmpeg's:

| M4A | Priming | Click read (Media Foundation) | MF shift |
| --- | ---: | ---: | ---: |
| AAC from FFmpeg's encoder (edit list: media time 1024) | 1024 | 23076 | **1024** (23.2 ms) |
| AAC from Apple's encoder (`afconvert`, `iTunSMPB` 2112, no edit list) | 2112 | 24164 | **2112** (47.9 ms) |
| Apple Lossless | n/a | 22052 | 0 |

What it shows: Media Foundation returns the music exactly `priming` frames later than
FFmpeg and CoreAudio do. It ignores both the edit list and `iTunSMPB`, and, unlike FAAD,
does not lose the first block. Two encoders fit that line, no other priming value was
measured, and it is a two-point model, not a derived rule. HE-AAC and a raw `.aac` (ADTS)
stream, which Mixxx also hands to Media Foundation, were not measured, so the preview
says so for `.aac` files on a Windows host. The priming is read from the file as in the
FAAD section.

The adapter adds `priming` frames for AAC in M4A or MP4 on a Windows host, and the preview
says how many tracks it moved and by how many milliseconds. The acceptance run with the
adapter's shift written (below) found each file's first beat within 2 frames of the click.

### What the adapter adds

Offsets are in the source file's own frames and apply additively to cue starts, loop
ends and beat positions. A cue that lands before the first Mixxx sample (possible only
for the `-F` and `-529` cases) is moved to the start.

| Format | macOS | Linux and Windows |
| --- | --- | --- |
| MP3 | `FFmpeg trim - (529 + CoreAudio priming)`, asked of CoreAudio | `-F`, `0` or `+(delay + 529)` as above; measured with libmad for a LAME file with an Info frame: +1105, in Debian's Mixxx 2.5.0 (libmad 0.15.1) and with libmad 0.16.4 on Windows |
| AAC, M4A, MP4 | 0, measured for Apple and FFmpeg encoders, and in Mixxx 2.5.6 for AAC and ALAC in M4A | Linux with FFmpeg: 0, measured in Debian's Mixxx 2.5.0. Linux with FAAD: `priming - 1024` for AAC in M4A or MP4, measured in Mixxx `main` with two encoders (FFmpeg's 0, Apple's +1088); which decoder applies is read from the installed `mixxx`, and unknown means 0 with a warning. Windows (Media Foundation): the whole priming, `+1024` for FFmpeg's encoder and `+2112` for Apple's, 0 for Apple Lossless, measured on Windows Server 2019 with a probe that reads like Mixxx's source |
| WAV, FLAC | 0, measured in Mixxx 2.5.6 | 0 |
| AIFF, Ogg Vorbis | 0, measured in Mixxx 2.5.6 (libsndfile, `Xiph.org OggVorbis`) against FFmpeg's decode | 0 |
| Opus | 0, measured in Mixxx 2.5.6 (`Xiph.org libopusfile`) against FFmpeg's decode, counted at 48 kHz: Mixxx always decodes Opus at 48000 Hz and reports that rate for the file | 0; opusfile discards the pre-skip as RFC 7845 requires, as FFmpeg does |
| WMA | not openable: the 2.5.6 download registers no decoder for `wma` (its debug log lists none), so the tracks are added and do not play | Linux: 0, plays only if Mixxx was built with FFmpeg; not verified. Windows: not openable, the 2.5.6 download registers no decoder for `wma` (Media Foundation is registered for `aac`, `m4a` and `mp4` only), so the tracks are added and do not play |

The preview warns for each case marked not verified, for AAC tracks that FAAD or Media
Foundation play late (saying how many it moved), and for WMA tracks the download cannot play.

### Reproducing

```sh
python3 tools/mixxx-acceptance/decoder_table.py
```

builds the probe, generates every file above and prints the table (macOS with `clang`,
`afconvert`, `ffmpeg` and `lame`). The Linux rows come from `mad_probe.c` and
`faad_probe.c` inside the Docker runs described under *Runtime acceptance*, and the Windows
rows from `mf_probe.cpp` inside the Windows run.

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

What the run does **not** show: Windows, Linux or Flatpak (this run is macOS only; the next
sections ran those systems);
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
creates its library at `~/.mixxx` there, as the platform table says. **157 of 157 checks
passed** (the formats scenario, a library CueRithm created, and the detection check below) for MP3, WAV, FLAC, AAC
from FFmpeg's and from Apple's encoder, ALAC in M4A, AIFF, Ogg Vorbis and Opus:

- Mixxx's log lists the decoders: MAD (libmad 0.15.1) for MP3, FFmpeg 7.1 only for AAC and
  M4A (this build has no FAAD decoder), libsndfile for AIFF and WAV, libFLAC, OggVorbis,
  libopusfile. No decoder is registered for WMA.
- **MP3 through libmad.** `mad_probe.c` does what Mixxx's MP3 source does (skip the first
  frame, decode the rest with libmad) and finds the first click at frame 23157 for a LAME
  file whose source click is at 22050. The adapter's Linux timing wrote the grid at 23155, a
  shift of 1105 frames (576 of encoder delay and 529 of decoder delay, read from the LAME
  header), within 2 frames. This is the first measurement of the libmad derivation above.
- AAC (both encoders), ALAC, AIFF, Ogg Vorbis and Opus land within 2 frames with no shift
  (the Opus grid counted at 48 kHz, as on macOS). Their click positions come from FFmpeg's
  decode on the host, assumed to equal where Mixxx's FFmpeg source and the format libraries
  start. The Apple-encoded AAC file is the case FAAD would move; FFmpeg trims its 2112
  priming itself, so it needs no shift here. The sync runs on the host, so the scenario
  tells the adapter that this Mixxx decodes AAC with FFmpeg (`CUERITHM_MIXXX_AAC`) and
  none was written. Separately, the run copies Debian's `mixxx` executable out of the
  container and CueRithm's own detection reads it as having no FAAD.
- Mixxx started without asking for a folder, found the schema current, deserialized the
  grids and keys, left the BPM-locked grids alone and did not change the rows.

What this does **not** show: the sync itself ran on macOS and only behaved as a Linux system
would (libmad timing, `CUERITHM_MIXXX_HOST=linux`), so CueRithm running on Linux (process
names, folder discovery) is covered by tests and by the repository's Linux checks, not by
this run; other packaging (AppImage, Flatpak, Ubuntu). A Mixxx with FAAD is the next
section, and Windows, where Media Foundation decodes AAC, has its own section after it.

### Linux: Mixxx `main`, built from source, schema 40 and FAAD

Debian's package has neither FAAD nor schema 40, so `run.py --linux-main VOLUME` runs the
same scenarios against Mixxx built from source (`main` at `2086fe0e55`, version 2.7.0,
schema 40, Qt 6, with FAAD, MAD and FFmpeg) in an Ubuntu 24.04 container. The image and the
build are `tools/mixxx-acceptance/linux-main/Dockerfile` and `build.sh`. libmp4v2, which
FAAD needs, has no package in current Debian or Ubuntu, so `build.sh` builds it from the
maintained fork (`enzo1982/mp4v2`, commit `f4c2f1b8`). Engine DJ export (libdjinterop) is
switched off because it does not build with GCC 13 here and has nothing to do with the
test. Same method as the Debian run, on a virtual display with no audio device. **157 of
157 checks passed:**

- Mixxx's log lists `"Nero FAAD2"` for M4A and MAD for MP3. The M4A files that FAAD opens
  (AAC from both encoders) were checked against `faad_probe.c`, ALAC against FFmpeg, since
  FAAD cannot open it and Mixxx falls back.
- **AAC from Apple's encoder.** The scenario told the adapter that this Mixxx decodes AAC
  with FAAD (`CUERITHM_MIXXX_AAC`, because the sync runs on the host); it moved that file's
  grid and cues 1088 frames (24.7 ms) and the preview said so: "1 AAC
  track(s) come from an encoder with more priming than 1024 samples (Apple's, for one).
  Your Mixxx decodes AAC with FAAD, which does not skip it, so CueRithm moved their grid
  and cues up to 25 ms later to match." The written first beat was 23138 against FAAD's
  click at 23140. FFmpeg's encoder (no shift, 22050 against 22052) and ALAC (22050 against
  22052) were within 2 frames.
- **Detection.** The run copies the `mixxx` executable out of the container and CueRithm's
  own reader (`executable_has_faad`) finds `libfaad` and `libmp4v2` named in this build and
  neither in Debian's, so the two builds are told apart the way the adapter does on a
  user's Linux machine. It reads files; it does not run Mixxx.
- MP3 through libmad again showed the 1105-frame shift (23155 against 23157).
- **Schema 40.** A library CueRithm created at schema 39 was upgraded by Mixxx on its next
  start ("Upgraded database schema to version 40"), and afterwards its tables, indexes and
  views (30 objects) equalled those of a library Mixxx created itself. A schema 40
  library Mixxx created was then synced by CueRithm and read back by the running Mixxx
  (grids and keys deserialized, BPM-locked grids untouched, rows unchanged after its scan).

Mixxx `main` has a start-up race that the harness works around and does not hide: a track
loaded onto a deck from the command line while the library scanner is importing the same
folder fails its insert with "database is locked". The harness lets the scanner add the
tracks on this build and loads nothing from the command line.

What this does **not** show: Mixxx 2.4 and 2.6 release builds (this is development `main`,
not a release); AppImage and Flatpak packages, so their FAAD detection by file paths is
untested; and the FAAD model beyond the two encoders measured.

To reproduce (needs Docker, a Mixxx source checkout and a Rust toolchain; the build takes
a long time and the volume holds several gigabytes):

```sh
docker build -t cuerithm-mixxx-main-deps tools/mixxx-acceptance/linux-main
docker run --rm -v /path/to/mixxx:/src:ro -v mixxx-main-out:/out \
    -v "$PWD/tools/mixxx-acceptance/linux-main:/ctx:ro" cuerithm-mixxx-main-deps sh /ctx/build.sh
python3 tools/mixxx-acceptance/run.py --linux-main mixxx-main-out
```

Remove the volume and image afterwards (`docker volume rm mixxx-main-out`,
`docker image rm cuerithm-mixxx-main-deps`).

### Windows: Mixxx 2.5.6, on Windows Server 2019

`run.py --windows` runs the `formats` scenario (AAC from two encoders, Apple Lossless, AIFF,
Ogg Vorbis, Opus, WAV, FLAC, MP3, and a WMA file) and the `created` scenario (a library
CueRithm makes) against the real Mixxx 2.5.6 for Windows, on a Windows Server 2019 workspace
(build 17763, PowerShell 5.1) reached through a remote desktop session. The account is not
an administrator, so Mixxx is the project's installer package unpacked with an
administrative install (checksum checked) rather than installed, and CueRithm's engine
crate was built with the GNU toolchain (MinGW from WinLibs), not MSVC; the setup is
`tools/mixxx-acceptance/windows/bootstrap.ps1`. Nobody clicks anything; the checks read
Mixxx's database and debug log, as in the Linux runs. **156 checks passed and none failed.**
The run also printed one note, which is not a pass: the decoder list.

- Mixxx's log lists the decoders: Media Foundation for `aac`, `m4a` and `mp4`, MAD
  (libmad 0.16.4) for MP3, libsndfile for AIFF and WAV, libFLAC, OggVorbis and libopusfile.
  **No decoder is registered for WMA**, and Mixxx adds no row for the WMA file, so the harness
  expects it left out (and the adapter warns that such tracks will not play).
- **AAC.** Media Foundation was read with `mf_probe.cpp` (section above). With the shift
  written for a Windows host the first beat was within 2 frames of the click for FFmpeg's
  encoder (1024 frames), Apple's encoder (2112 frames) and Apple Lossless (none), and the
  cue positions carried the same shift as the grid. Before the shift was added the same run
  failed those two AAC files by 1026 and 2114 frames, so the run does tell the cases apart.
- AIFF, Ogg Vorbis, Opus, WAV and FLAC passed the same alignment check (the first beat within
  16 frames of the click) with no shift, against FFmpeg's decode on the host as on the other
  systems.
- Mixxx started without asking for a folder, found the schema current, deserialized the
  BeatGrid-2.0 grids and keys, reported no deserialization failure, did not re-read tags,
  found the same tracks again rather than duplicates, kept the folder list and left the
  synced rows unchanged after loading and scanning them. Mixxx stores `/` in its paths on
  Windows (`D:/Users/...`), and the adapter's references matched.
- **A library CueRithm created** equals the one Mixxx makes (30 tables, indexes and views,
  the schema settings and the two default playlists). The first run found a real defect:
  Git for Windows checks `schema39.sql` out with CRLF, `include_str!` embeds it as it is
  and SQLite stores each statement's text as written, so several objects (the first six
  named were `itunes_playlist_tracks`, `Playlists`, `crates`, `track_locations`,
  `PlaylistTracks` and `rhythmbox_playlists`) differed from Mixxx's own by line endings
  alone, which `schema_diff.py` showed. The adapter now normalises the layout to LF and
  `.gitattributes` pins the file; a unit test checks a CRLF layout makes the same library.
- **Mixxx's analyzer left the BPM-locked grids alone** (`Track is BpmLocked` in its log for
  each loaded track, and the rows unchanged). The first Windows run could not observe this: the
  analyzer thread started but no track ever reached it. The cause was the workspace, not
  Mixxx's analysis: with no GPU and no software OpenGL (`opengl32sw` is not in the unpacked
  package) Mixxx shows a modal "OpenGL Direct Rendering" warning right after its skin loads
  (`checkDirectRendering` in the source), and its main thread waits for a click before it
  sets up the sound engine. In the source, the workers that finish loading a track onto a deck
  are scheduled from the engine's audio callback. Nobody clicks in the run, so no deck finished
  loading; the log stopped after the skin and showed no sound-manager line.
  `run.py` now writes the setting a click leaves behind (`[Direct Rendering] Warned yes` in
  the scratch profile's `mixxx.cfg`) before it starts Mixxx on Windows, and the check passed in
  both scenarios.

- **MP3 through libmad.** The log names "MAD: MPEG Audio Decoder 0.16.4 NDEBUG FPM_64BIT
  OPT_ACCURACY". `bootstrap.ps1` builds that version from the maintained source (CMake,
  MinGW, `OPTIMIZE=ACCURACY`, a Release build) and the run builds `mad_probe.c` against it. The
  probe finds the first click of the LAME-encoded file at frame **23157**, the same frame
  Debian's libmad 0.15.1 gave, and the adapter's Windows timing wrote 23155 (the +1105 frames
  of the libmad rule), within 2 frames, in both scenarios. The probe's libmad is that
  version built from source, not the binary Mixxx ships, and like the other probes it was not
  compared with a position Mixxx itself reports.

What this does **not** show: Windows 10 or 11 (Server 2019 has the same Media Foundation
generation, `10.0.17763`, but another build was not run); an MSVC-built CueRithm; a Mixxx
built from the Windows installer rather than unpacked from it; playback or the waveform
inside Mixxx; HE-AAC and raw `.aac` streams; and CueRithm's own window, discovery and process
checks on Windows, which the repository's Windows checks cover. The unit tests of the
adapter crate (`cargo test -p cuerithm-dj-sync --lib`) also ran on the workspace; the two
`pioneer_usb` tests that need the sealed FFmpeg decoder runtime fail there because the
runtime was not built, and the rest pass.

To reproduce, on Windows with the per-user toolchain from `bootstrap.ps1`:

```powershell
powershell -ExecutionPolicy Bypass -File tools\mixxx-acceptance\windows\run.ps1 -Task all
```

which writes `tests.log` and `acceptance.log` next to the checkout.

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

- **Native CueRithm on Linux, other Linux packages, and Windows.** Debian's Mixxx
  2.5.0 with libmad, and Mixxx `main` with libmad and FAAD, were run in Linux containers,
  as recorded in *Runtime acceptance*. The sync itself ran on macOS using Linux timing, so
  those runs do not establish CueRithm's native Linux discovery or process checks. AppImage
  and Flatpak packages were not run. Windows ran Mixxx 2.5.6 on Windows Server 2019 in a
  workspace, with the sync run natively there by the adapter's unit tests and the
  acceptance harness; CueRithm's own window, discovery and process checks on Windows, other
  Windows versions and an MSVC build were not.
- **The FAAD shift beyond two encoders.** `priming - 1024` was measured for priming 1024
  (FFmpeg) and 2112 (Apple) only. An encoder with another priming value, or an M4A with an
  edit list that disagrees with `iTunSMPB`, has not been measured.
- **CoreAudio outside the probe.** The macOS offsets were measured with the CoreAudio
  calls `SoundSourceCoreAudio` makes, on one macOS version (15.8) and one set of
  encoders. Apple can change its tag rules between releases, which is why the adapter
  asks `AudioFile` instead of predicting them. Playback inside the Mixxx application
  is covered separately, in *Runtime acceptance* above.
- **Opus, Ogg Vorbis and AIFF** alignment was measured in Mixxx 2.5.6 on macOS against
  FFmpeg's decode, not against libopusfile, libvorbisfile or libsndfile themselves.
- **The Media Foundation shift beyond two encoders.** `priming` frames was measured for
  priming 1024 (FFmpeg) and 2112 (Apple) only, on one Windows build (`10.0.17763.9121`).
  A raw `.aac` (ADTS) stream, HE-AAC and other encoders have not been measured, and the
  preview says so for `.aac` files.
- **WMA** cannot be opened by the macOS and Windows downloads (their logs register no WMA
  decoder); whether a Linux build with FFmpeg plays it, and how it would align, was not
  run.
- **Mixxx versions** other than schema 39 and 40, and builds without MAD or CoreAudio.
  Schema 40 ran against Mixxx `main` built from source (and its upgrade of a CueRithm-made
  schema 39 library), not against a 2.6 or later release build; Mixxx 2.4 was not run.
