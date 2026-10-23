# Credits and open source

<p class="lead">CueRithm is built on open-source work by other people. This page names the projects it uses, says under which licence, and explains how those licences fit together. The authoritative list is <code>THIRD_PARTY_NOTICES.md</code>, which ships with every release next to the source.</p>

## OneTagger { #onetagger }

**[OneTagger](https://github.com/Marekkon5/onetagger)** by Marekkon5 is the foundation of CueRithm's automatic tagging. Selected files from commit `36523f71f2d9a5947912f3cb930f1a31fcb2e3ee` are vendored and used under the GNU General Public Licence v3:

- the Beatport and Traxsource providers,
- the matching and configuration engine,
- the tag readers and writers for ID3, MP4, FLAC, Vorbis and WAV,
- the file-name template parser, used only to name external cover files.

OneTagger's interface, Shazam and Spotify support, and post-command behaviour are not included. No OneTagger renaming workflow is imported either; renaming files in CueRithm is its own [File organization](../../rename/organize-files.md). The Beatport search endpoint wrapper is adapted from an upstream pull request (#526, from the `rosgr100` fork). The mappings for iTunes, BPM Supreme, Bandcamp and Deezer are adapted from OneTagger's providers as well, with Beatsource, Juno Download and Spotify excluded by decision (`vendor/onetagger/UPSTREAM.json`).

CueRithm adapts the vendored code for its staged tag writes ([How are tags written safely?](../../how-it-works/tag-writing.md)) and includes bug fixes and network checks. The changes are listed in `vendor/onetagger/README.md`:

- *Fixes:* inverted artwork detection for Ogg Vorbis and Opus; ID3 v2.3 to v2.4 upgrades now keep slashes in names such as "AC/DC"; independent track and total handling in ID3 and MP4; WAV chunk-size correctness.
- *Hardening:* bounded response sizes, timeouts, HTTP status errors, no crash on malformed Traxsource rows, safe token refresh, cancellation.
- *Behaviour:* explicit AAC and WMA handling, checked ID3 creation, full-precision BPM in MP4, the duration filter enforced for fuzzy matches, "match by ID" respected, a failed lookup distinguished from an empty result, Traxsource limited to one request at a time.

Thank you to the OneTagger author and contributors. [Tag new tracks from Beatport and Traxsource](../../tagging/tag-new-tracks-automatically.md) shows the feature built on their work.

## Which components CueRithm uses { #components }

| Component | Used for | Licence |
|---|---|---|
| **OpenKeyScan analyzer** by Alexander Sommer ([rekordcloud](https://github.com/rekordcloud/openkeyscan-analyzer)) | The key-detection network, converted to ONNX, and the constant-Q frontend specification ([How is the key detected?](../../how-it-works/key.md)) | MIT |
| **FFmpeg and ffprobe 9.0.2** ([ffmpeg.org](https://ffmpeg.org/legal.html)) | The bundled decoder for every audio format | LGPL-2.1-or-later configuration, no GPL or non-free parts, shipped as separate executables with source |
| **SoX Resampler (libsoxr)**, from the Python-SoXR 1.0.0 source distribution by dofuuz | 44.1 kHz resampling for key analysis, statically linked | LGPL-2.1-or-later (PFFFT inside has its own notice) |
| **TagLib 2.3.2** ([taglib.org](https://taglib.org)) | WMA and ASF tag writing, statically built | LGPL and MPL, with its bundled utf8cpp notice |
| **RTen**, **RustFFT** | Running the ONNX key model in Rust, and spectral analysis | MIT or Apache-2.0 |
| **rubato**, **Symphonia** and others from the lock files | Resampling and Rust-side decoding | MIT or Apache-2.0; Symphonia is MPL-2.0 |
| **regress** by ridiculous_fish, adapted | The bounded JavaScript-style regular expressions in smart playlists | MIT or Apache-2.0 |
| **Chrono** and **Chrono-TZ** | Calendar calculations in smart playlists | MIT or Apache-2.0 |
| **slugify** 1.6.6 (character map) and **ICU4X Collator** 2.3.0 | Normalising and ordering titles when matching duplicates | MIT; Unicode-3.0 |
| **SQLCipher** 4.10.0 and **OpenSSL** 3.5.5, through rusqlite | Opening the encrypted rekordbox database for direct sync | BSD-style; Apache-2.0 |
| **rusqlite** with bundled SQLite | The CueRithm library database | MIT; SQLite is public domain |
| **Lofty**, **quick-xml**, RustCrypto **SHA-1** | Metadata handling and file formats of the DJ-software destinations | MIT or Apache-2.0; MIT; MIT or Apache-2.0 |
| **rusty-chromaprint** 0.3.0 | Audio fingerprints for optional recording identification | MIT |
| **rodio** | Audio playback | MIT or Apache-2.0 |
| **Tauri 2**, **React 19**, **Lucide** | The desktop shell, the interface and the icons | MIT; MIT; ISC |
| **`tauri-plugin-opener`** | Opening approved sign-up and catalog links in your browser | Apache-2.0 OR MIT |
| **`id3`** 1.17.2, locally patched | ID3 tag reading and writing | MIT |
| **Space Grotesk**, **Inter**, **IBM Plex Mono** | The typefaces used in the app and this guide | SIL Open Font Licence 1.1 |

AcoustID and MusicBrainz, used for optional recording identification, are network services and not embedded databases.

!!! info "Full dependency list"
    For every dependency and its exact version, see `Cargo.lock` and `apps/desktop/pnpm-lock.yaml`. The notices file lists the licence texts that must accompany the binaries.

## How the licences fit together { #licences }

- The analysis engine crates, the workspace default, are MIT.
- The desktop application, the tagging crate that wraps OneTagger and the vendored OneTagger code are GPL-3.0-only. The combined desktop application you install is therefore distributed under the GPL-3.0.
- **FFmpeg** is built without GPL or non-free options, so it stays LGPL. It ships as separate executables, together with its source tarball and build scripts.
- **libsoxr and TagLib** are LGPL, with notices kept in the source tree.
- Every release includes the matching application source and `THIRD_PARTY_NOTICES.md`, and the FFmpeg source.

Release packaging for some LGPL obligations is still being validated, as the notice file explains. Read it before you redistribute CueRithm.

## Trademarks and affiliation { #trademarks }

Mixed In Key, Lexicon, Beatport, Traxsource, rekordbox, OneTagger, Apple, Microsoft, Windows, macOS, FFmpeg, SQLite and other product names are trademarks of their owners. CueRithm is not affiliated with, endorsed by or sponsored by any of them. Names are used only to say what CueRithm works with or was inspired by. "Camelot" refers to the widely used key notation.

## Related

- [How CueRithm was made](provenance.md)
- [About CueRithm](index.md)
- [Check what works with your DJ software](../compatibility.md)
- [How are tags written safely?](../../how-it-works/tag-writing.md)

<!-- verified: THIRD_PARTY_NOTICES.md (OpenKeyScan commit and MIT; libsoxr/PFFFT; FFmpeg 9.0.2 LGPL-2.1-or-later; TagLib 2.3.2 LGPL/MPL + utf8cpp; regress; Chrono; slugify/ICU4X; SQLCipher 4.10.0/OpenSSL 3.5.5; Lofty/quick-xml/SHA-1; rusty-chromaprint; Space Grotesk/Inter/IBM Plex Mono OFL; tauri-plugin-opener; id3 1.17.2; OneTagger commit and GPLv3); vendor/onetagger/README.md and UPSTREAM.json (rosgr100 PR526, iTunes/BPM Supreme/Bandcamp/Deezer adaptations, exclusions, no renaming workflow imported); crates/camelot-core/vendor/soxr-provenance.json (dofuuz/python-soxr); Cargo.toml and crate manifests (MIT workspace, GPL-3.0-only desktop and cuerithm-autotag); apps/desktop/package.json (React 19, lucide-react); README.md Credits and Licence; legacy about/credits.md re-checked. UNVERIFIED: licences of RTen, RustFFT, rubato, rodio, Symphonia, Tauri, React and Lucide beyond what THIRD_PARTY_NOTICES.md and the legacy credits state -->
