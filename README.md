<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/brand/readme-dark.png">
    <img src="docs/assets/brand/readme-light.png" alt="CueRithm. Know your music. Shape your set." width="100%">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/johnneerdael/CueRithm/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/johnneerdael/CueRithm?style=flat-square&color=3658F5&label=release"></a>
  <a href="https://github.com/johnneerdael/CueRithm/actions/workflows/desktop.yml"><img alt="Desktop checks" src="https://img.shields.io/github/actions/workflow/status/johnneerdael/CueRithm/desktop.yml?branch=main&style=flat-square&label=desktop%20checks"></a>
  <a href="https://github.com/johnneerdael/CueRithm/actions/workflows/ci.yml"><img alt="Engine checks" src="https://img.shields.io/github/actions/workflow/status/johnneerdael/CueRithm/ci.yml?branch=main&style=flat-square&label=engine%20checks"></a>
  <a href="https://cuerithm.neerdael.nl/"><img alt="User guide" src="https://img.shields.io/badge/user%20guide-online-FF785E?style=flat-square"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-lightgrey?style=flat-square">
</p>

CueRithm is a desktop library app for DJs. It analyses tracks offline, computing musical key, BPM and beatgrid, one whole-track energy rating, structural cues and a colour waveform. You can tag and clean up the library, create playlists, and set up smart playlists that update automatically.

Analysis runs on your computer without an account or audio uploads. Your music stays where it is unless you enable file organization.

<p align="center">
  <a href="https://github.com/johnneerdael/CueRithm/releases/latest"><b>Download</b></a> &nbsp;·&nbsp;
  <a href="https://cuerithm.neerdael.nl/get-started/"><b>Get started</b></a> &nbsp;·&nbsp;
  <a href="https://cuerithm.neerdael.nl/"><b>User guide</b></a> &nbsp;·&nbsp;
  <a href="https://cuerithm.neerdael.nl/how-it-works/"><b>How it works</b></a>
</p>

<p align="center">
  <img src="docs/user-guide/assets/screens/40-player.webp" alt="CueRithm with a track library above a colour waveform with five labelled cues" width="100%">
</p>

## What you get

| | |
| --- | --- |
| **Key** | A neural network inside the app analyses the whole track and returns a Camelot key such as `7A`. |
| **BPM and beatgrid** | One tempo and one first beat per track, shown to two decimals, with beat lines in the player. |
| **Energy** | One rating from 1 to 10 for the whole track. |
| **Cues** | Start, Drop, Breakdown, a second Drop, a second Breakdown and End / Fade-out, plus an optional emergency loop. Click a cue to jump there. |
| **Waveform** | Three frequency bands in six colour palettes, with a default 32-second zoom for checking the grid. |
| **Incoming** | Watch folders, drag and drop or **Add tracks**. Every track is decoded, analysed and moved into your library automatically. |
| **DJ library import** | Preserve source metadata, playlists and crates, then choose analysis, hotcue replacement, retagging, tag writes and file naming independently. See the [Import guide](docs/user-guide/files/import-library.md). |
| **Tagging** | Optional DJ-store and public-catalog providers, private account setup, match review and staged writes that verify your audio is unchanged. |
| **Cleanup** | Eleven Smart Fixes plus genre and artist mappings: scan, review, apply, and **Undo**. |
| **File organization** | Rename files from metadata, or move or copy them into a folder structure. Preview changes, repair linked DJ-library paths, and recover or undo operations. |
| **Playlists** | Ordinary playlists, folders and smart playlists with rules over BPM, key, energy, genre, dates and cues. |
| **DJ sync** | Send prepared tracks, metadata, cues, grids and playlists to **rekordbox 6 / 7, VirtualDJ, Engine DJ, djay Pro, Serato DJ Pro 4, Pioneer USB and Mixxx**, with a preview and recoverable backups. Windows libraries on Linux are supported through explicit Wine / Proton setups and drive mappings. See the [Sync guide](docs/user-guide/sync/index.md). |
| **Set preparation** | Correct cues, loops and grids, save templates, find compatible tracks, check playlist flow and match text/CSV tracklists to your library. |
| **Library care** | Reviewed relocation for moved files, duplicate/file maintenance, portable `.cuerithm` backups, retained-library restore and Lexicon v26 backup import. |
| **Formats** | MP3, AAC (ADTS and M4A), ALAC, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA. The decoder is bundled, so you do not need to install anything else. |

## See it work

<table>
  <tr>
    <td width="50%"><img src="docs/user-guide/assets/screens/21-incoming-analyzing.webp" alt="Incoming view with tracks being decoded, analysed and queued"><br><sub><b>Incoming.</b> Drop tracks in and watch each one move through decoding and analysis.</sub></td>
    <td width="50%"><img src="docs/user-guide/assets/screens/55-smart-playlist-editor.webp" alt="Smart playlist editor with a key rule and a BPM rule"><br><sub><b>Smart playlists.</b> Rules over key, BPM, energy and more, previewed before you save.</sub></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/user-guide/assets/screens/72-cleanup-preview.webp" alt="Cleanup preview listing proposed metadata changes"><br><sub><b>Cleanup.</b> Review every proposed change before anything is written, and undo it later.</sub></td>
    <td width="50%"><img src="docs/user-guide/assets/screens/92-light-player.webp" alt="CueRithm in the light theme"><br><sub><b>Light and dark.</b> Follows your system, with labelled data that never relies on colour alone.</sub></td>
  </tr>
</table>

## Analysis and file handling

Key, tempo, energy and cues are computed on your computer from the decoded audio. Optional catalog tagging and consented recording identification use the network. Identification sends an audio fingerprint; raw audio recognition uploads are not enabled.

File organization can rename, move or copy audio when enabled for Incoming tracks or applied through a reviewed plan. Tagging and cleanup write to a staged copy, then decode it again to verify that the playable audio is identical. The original is kept, and an atomic swap with a journal allows recovery after a crash.

The bundled FFmpeg decodes every format into the same 44.1 kHz stream, so analysis, the waveform and playback use the same cue positions. Analysis agreement is measured against reference tools on my own library; it does not establish musical accuracy. Read [How accurate is it?](https://cuerithm.neerdael.nl/how-it-works/accuracy/).

## Install

Download the latest build from the [releases page](https://github.com/johnneerdael/CueRithm/releases/latest).

| System | File | Notes |
| --- | --- | --- |
| **macOS** (Apple Silicon) | `.dmg` | Signed with a Developer ID and notarized by Apple. There is no Intel Mac build. |
| **Windows** (x86-64) | `.zip` | Portable app. Keep the `media` folder next to the program. Unsigned, so SmartScreen may warn on first launch. |
| **Linux** (x86-64) | `.deb` | Unsigned. |

Every release includes the matching application source, the FFmpeg source and `THIRD_PARTY_NOTICES.md`. For installation steps, see [Install](https://cuerithm.neerdael.nl/get-started/install/).

You can check an installation at any time:

```sh
/Applications/CueRithm.app/Contents/MacOS/cuerithm-desktop --diagnostics
```

## Compatibility and validation

Direct rekordbox 6 / 7 writes are supported on macOS, Windows and Windows installations under Wine / Proton on Linux when the database and required asset formats pass compatibility checks. Compatible patch builds are eligible; unrecognized formats remain read-only. Native rekordbox playback, cues, grids and loops were validated on macOS builds **6.8.7.0303** and **7.2.19.0342**.

Mixxx sync was functionally checked against **Mixxx 2.5.6 on macOS** and **Debian’s Mixxx 2.5.0 in a Linux container**. The macOS runs passed **240 checks**; Linux passed **105**, covering library loading, grids, keys, cue data and decoder timing. The [Mixxx guide](docs/user-guide/sync/mixxx.md) describes the runs and remaining platform and decoder checks.

CueRithm is early software; check each download’s release notes for its included features. Playback uses the system default output. Windows/Wine DJ-app playback, other Mixxx packages and physical-player acceptance still need broader testing. See [remaining limits and planned work](docs/user-guide/help/not-available-yet.md).

## In development

We are building **DJ-library import into CueRithm**: bring an existing managed library into the app, manage it here, then sync prepared tracks back to your DJ software. The planned workflow covers full-library or selected-playlist import, merging libraries, replacing a library, choosing which fields to import and backing up before import, following [Lexicon’s import options](https://www.lexicondj.com/manual/import-options). Coverage is planned across the DJ software destinations and Mixxx; **DJ.Studio is under consideration**.

Relocate externally moved music, export portable `.cuerithm` library data, map music folders during restore, or import the supported Lexicon v26 database-backup profile. These workflows retain the current library for recovery and keep music separate from the archive. See [back up your library](docs/user-guide/files/library/back-up-library.md), [restore a library](docs/user-guide/files/library/restore-library.md) and [find missing files](docs/user-guide/files/library/find-missing-files.md).

## Documentation

The [user guide](https://cuerithm.neerdael.nl/) is organised by what you want to do:

- [Get started](https://cuerithm.neerdael.nl/get-started/): install, learn the window, prepare your first tracks, and [pick the right guide for your goal](https://cuerithm.neerdael.nl/get-started/choose-your-path/).
- [Manage files](https://cuerithm.neerdael.nl/files/): add tracks, watch a folder such as Beatport, follow Incoming, save presets, archive, and [look after your library](https://cuerithm.neerdael.nl/files/library/) with duplicate checks, missing-file scans and backups.
- [Playlists](https://cuerithm.neerdael.nl/playlists/): playlists, smart playlists, generators, compatible tracks, playlist flow and tracklist matching.
- [Auto tagging](https://cuerithm.neerdael.nl/tagging/): tag from Beatport and Traxsource, review matches, edit tags, and clean up titles, genres and artists.
- [Renaming](https://cuerithm.neerdael.nl/rename/): name files from a pattern and sort them into folders, with Undo.
- [Track analysis](https://cuerithm.neerdael.nl/analysis/): key, BPM, energy, the player, cues and the beatgrid, and how to correct them.
- [Sync](https://cuerithm.neerdael.nl/sync/): rekordbox 6 / 7, VirtualDJ, Engine DJ, djay Pro, Serato DJ Pro 4, Pioneer USB and Mixxx.
- [Customize](https://cuerithm.neerdael.nl/customize/), [How it works](https://cuerithm.neerdael.nl/how-it-works/) and [Reference](https://cuerithm.neerdael.nl/reference/): themes, the analysis pipeline, settings and shortcuts.
- [Help](https://cuerithm.neerdael.nl/help/): fix a problem, FAQ, [what works with which DJ software](https://cuerithm.neerdael.nl/help/compatibility/) and [what is not available yet](https://cuerithm.neerdael.nl/help/not-available-yet/).

## Credits

CueRithm uses [OneTagger](https://github.com/Marekkon5/onetagger) by Marekkon5 for selected metadata providers, matching and tag writing (GPL v3), [OpenKeyScan](https://github.com/rekordcloud/openkeyscan-analyzer) for the key network (MIT), and FFmpeg for decoding (LGPL). Other components include TagLib, libsoxr, Tauri and React. The full list with licences is in [Credits and open source](https://cuerithm.neerdael.nl/help/about/credits/) and [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

Mixed In Key and Lexicon DJ shaped how CueRithm behaves. I studied them using copies I own, and CueRithm is my own independent implementation written in Rust. No original code, models or assets are included. See [How CueRithm was made](https://cuerithm.neerdael.nl/help/about/provenance/).

CueRithm is not affiliated with, endorsed by or sponsored by Mixed In Key, Lexicon, Beatport, Traxsource, rekordbox, OneTagger or any other product named here.

## For developers

The analysis engine is a set of Rust crates (`camelot-core`, `camelot-cli` and others) with a Tauri 2 and React desktop app on top. The engineering reference, with build, test and validation commands, is in [`docs/development/engine-and-cli.md`](docs/development/engine-and-cli.md). Desktop setup and platform prerequisites are in [`docs/development/desktop.md`](docs/development/desktop.md), and the pull request and release workflow is in [`docs/development/pr-workflow.md`](docs/development/pr-workflow.md).

```sh
cargo test --workspace
cargo build -p camelot-cli --release
```

Pull requests need a completed `@codex review` and the checks in the [pull request template](.github/PULL_REQUEST_TEMPLATE.md). Releases are published automatically from `main`.

## Licence

The independent analysis engine is MIT licensed ([`LICENSE`](LICENSE)). The desktop application and the OneTagger integration are GPL-3.0-only ([`apps/desktop/LICENSE`](apps/desktop/LICENSE)). See [third-party notices](THIRD_PARTY_NOTICES.md).

Developer automation: [headless library commands](docs/automation.md). Provider evidence: [PR5 acceptance and remaining limits](docs/research/pr5-provider-acceptance.md).
