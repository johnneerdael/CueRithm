# Credits and open source

CueRithm stands on a lot of other people's work. This page names the main pieces, what each is used for, and its licence. The authoritative list is `THIRD_PARTY_NOTICES.md`, which ships with every release alongside the source.

## OneTagger { #onetagger }

**[OneTagger](https://github.com/Marekkon5/onetagger)** by Marekkon5 is the foundation of CueRithm's automatic tagging. Selected files from commit `36523f71f2d9a5947912f3cb930f1a31fcb2e3ee` are vendored and used under the **GNU General Public Licence v3**:

- the **Beatport** and **Traxsource** providers,
- the matching and configuration engine,
- the tag readers and writers for ID3, MP4, FLAC, Vorbis and WAV,
- the file-name template parser (used only to name external cover files).

OneTagger's interface, Shazam and Spotify support, post-commands and renamer workflows are **not** included. The Beatport search endpoint wrapper is adapted from an upstream pull request (#526, from the `rosgr100` fork).

CueRithm changed the vendored code to fix bugs, harden network handling and fit its safe-write model. The changes are listed in `vendor/onetagger/README.md`. In summary:

- *Fixes:* inverted artwork detection for Ogg Vorbis and Opus; ID3 v2.3 to v2.4 upgrades now keep slashes in names such as "AC/DC"; independent track and total handling in ID3 and MP4; WAV chunk-size correctness.
- *Hardening:* bounded response sizes, timeouts, HTTP status errors, no crash on malformed Traxsource rows, safe token refresh, cancellation.
- *Behaviour:* explicit AAC and WMA handling, checked ID3 creation, full-precision BPM in MP4, the duration filter enforced for fuzzy matches, "match by ID" respected, a failed lookup distinguished from an empty result, Traxsource limited to one request at a time.

Thank you to the OneTagger author and contributors.

## The components

| Component | Used for | Licence |
| --- | --- | --- |
| **OpenKeyScan analyzer** by Alexander Sommer ([rekordcloud](https://github.com/rekordcloud/openkeyscan-analyzer)) | The key-detection network, converted to ONNX, and the constant-Q frontend specification | MIT |
| **FFmpeg and ffprobe 9.0.2** ([ffmpeg.org](https://ffmpeg.org/legal.html)) | The bundled decoder for every audio format | LGPL-2.1-or-later configuration, no GPL or non-free parts, shipped as separate executables with source |
| **SoX Resampler (libsoxr)**, via the Python-SoXR sdist by dofuuz | 44.1 kHz resampling for key analysis, statically linked | LGPL-2.1-or-later (PFFFT inside has its own notice) |
| **TagLib 2.3.2** ([taglib.org](https://taglib.org)) | WMA and ASF tag writing, statically built, one local patch | LGPL-2.1 / MPL-1.1 |
| **RTen** | Runs the ONNX key model in Rust | MIT or Apache-2.0 |
| **RustFFT**, **rubato**, **Symphonia** | Spectral analysis, resampling and Rust-side decoding | MIT or Apache-2.0; Symphonia is MPL-2.0 |
| **regress** by ridiculous_fish, adapted | The bounded JavaScript-style regular expressions in smart playlists | MIT or Apache-2.0 |
| **rodio** and **cpal** | Audio playback | MIT or Apache-2.0 |
| **SQLite** via **rusqlite** | The library database | Public domain and MIT |
| **chrono** and **chrono-tz** | Calendar calculations in smart playlists | MIT or Apache-2.0 |
| **Tauri 2**, **React 19**, **lucide-react** | The desktop shell, the interface and the icons | MIT, MIT and ISC |
| **reqwest** (rustls), **image**, **scraper**, **lofty**, **id3**, **mp4ameta**, **metaflac**, **notify** and others | Catalog requests, artwork and tag input and output, file watching | Mostly MIT or Apache-2.0, see the lock files |
| **Space Grotesk**, **Inter** and **IBM Plex Mono** | The typefaces used in the app and this guide | SIL Open Font Licence 1.1 |

The Rust crates that read rekordbox databases (SQLCipher and OpenSSL among them) are present in the source as foundations for a future feature and are described in [Not available yet](roadmap.md).

!!! info "Full dependency list"
    This table covers the notable components. A complete list with exact versions is in `Cargo.lock` and `pnpm-lock.yaml`, and the notices file lists the licence texts that must travel with the binaries.

## How the licences fit together

- The **analysis engine** crates (the workspace default) are **MIT**.
- The **desktop application**, the tagging crate that wraps OneTagger and the vendored OneTagger code are **GPL-3.0-only**. The combined desktop application you install is therefore distributed under the GPL-3.0.
- **FFmpeg** is built without GPL or non-free options, so it stays LGPL. It ships as separate executables, together with its source tarball and build scripts.
- **libsoxr and TagLib** are LGPL, with notices kept in the source tree.
- Every release includes the matching **application source** and `THIRD_PARTY_NOTICES.md`, and the FFmpeg source.

The project's notice file is candid that release packaging for some LGPL obligations is still being validated. If you plan to redistribute CueRithm, read it first.

## Trademarks

Mixed In Key, Lexicon, Beatport, Traxsource, rekordbox, OneTagger, Apple, Microsoft, Windows, macOS, FFmpeg, SQLite and other product names are trademarks of their owners. CueRithm is **not affiliated with, endorsed by or sponsored by** any of them. Names are used only to say what CueRithm works with or was inspired by. "Camelot" refers to the widely used key notation.
