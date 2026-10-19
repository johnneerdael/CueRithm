# Third-party source notices

This file records analysis and desktop components. Other model
provenance is retained in the corresponding `models/` directories.

## OpenKeyScan

The fixed key network/frontend specification follows
[rekordcloud/openkeyscan-analyzer](https://github.com/rekordcloud/openkeyscan-analyzer),
commit `d5cac64bc7195826b6eb10c55b9b0c97fb3287c0`, copyright 2025 Alexander Sommer.
Its MIT notice is preserved at `models/openkeyscan/LICENSE.upstream`.
Model/checkpoint identities and desktop distribution details are in
`models/openkeyscan/model.json` and `models/builtin/manifest.json`. CQT filter-bank arrays were generated with
librosa 0.11.0/NumPy 1.26.4 using the fixed parameters documented in the research
specification; no music or per-track reference answer is embedded in them.

## SoX Resampler Library and PFFFT

`crates/camelot-core/vendor/soxr` contains the complete portable libsoxr source
included in the Python-SoXR 1.0.0 source distribution, SHA256
`e07ee6c1d659bc6957034f4800c60cb8b98de798823e34d2a2bba1caa85a4509`.
Python-SoXR's Python extension is not compiled or used by the engine.
The source inventory is `crates/camelot-core/vendor/soxr-provenance.json`.
The single upstream shell-script symlink is materialized as identical contents
for Windows-compatible source archives; the C sources are unchanged.

The library is LGPL-2.1-or-later; preserve `vendor/soxr/LICENCE` and
`vendor/soxr/COPYING.LGPL`. PFFFT's additional notices are retained in
`vendor/soxr/src/pffft.c` and associated headers. The adapter and fixed build
recipe are at `native/openkeyscan`. This build statically links the library.
A binary release must include the applicable notices and the source/build or
relinking materials required for that release; release packaging remains to be
validated. All native source/build file contents enter the engine identity.

## Rust dependencies

RTen/rten-tensor 0.24.0 and RustFFT 6.4.1 declare MIT OR Apache-2.0 licensing.
They are resolved by Cargo, with exact dependencies/checksums retained in
`Cargo.lock`. No proprietary application binary, code or asset is part of the
OpenKeyScan backend.

## Desktop media runtime

FFmpeg/ffprobe 9.0.2 are built from the source archive and SHA-256 pinned in
`tools/media-runtime/manifest.json`. The recipe disables network access,
autodetected external libraries and GPL/nonfree components; internal FFmpeg
libraries are statically linked into these separate executables. FFmpeg declares
LGPL-2.1-or-later for this configuration. Runtime artifacts include its
`COPYING.LGPLv2.1`, source archive and complete rebuild scripts. See
[FFmpeg licensing](https://ffmpeg.org/legal.html). Decoder provenance/hashes are
recorded in each runtime's `runtime.json` and verified before audio processing.

The independent engine retains this repository's MIT license. The desktop and
selected OneTagger integration are distributed under GPL-3.0-only (see
`apps/desktop/LICENSE` and `vendor/onetagger/LICENSE`);
unsigned CI bundles include a matching source archive. Preserve the LGPL SoX
source/notices and rebuild materials when distributing binaries. Do not replace
the source offer with a bare binary checksum.

## Desktop UI and playback

The Dosis font is distributed under SIL Open Font License 1.1; its full notice
is `apps/desktop/src/assets/Dosis-OFL.txt`. Cuerithm's icon is original SVG.
React and Tauri declare MIT licensing; Lucide declares ISC; Rodio declares
MIT OR Apache-2.0. Exact dependency identities are in `Cargo.lock` and
`apps/desktop/pnpm-lock.yaml`. Build tools and test fixtures do not require
Python in the shipped application. The desktop embeds the pinned OpenKeyScan
ONNX conversion under the upstream MIT notice. Track energy is independently
implemented Rust acoustic DSP; no proprietary code or energy model is embedded. Training music, captures,
reference labels and licensed reference applications are not bundled.


## Selected OneTagger integration (GPLv3 desktop)

The user approved reuse of Marekkon5/onetagger code at commit
`36523f71f2d9a5947912f3cb930f1a31fcb2e3ee` under GPLv3. Selected provider,
matching and tag-format sources live in `vendor/onetagger`; their exact original
file hashes are recorded in `UPSTREAM.json`. See `vendor/onetagger/README.md` for
local adaptations. The independent engine retains its MIT notices. The combined
desktop distribution uses GPL-3.0-only, with corresponding source and notices.
No Lexicon application code, user credentials or personal music are included.


## Native WMA/ASF metadata writer

The tagging adapter statically builds TagLib 2.3.2 ASF support from a pinned
prepared source archive under `vendor/taglib`. It retains TagLib's LGPL/MPL
notices and bundled utf8cpp notice. The preparation script and manifest record
the official release checksum, selected-source checksum and omitted upstream
test/audio files. Build sources and notices are included in source archives.
No separate TagLib installation is needed by the desktop user.


## regress for bounded smart-list regex predicates

The independent `cuerithm-regex` wrapper uses a documented adaptation of
regress 0.12.0, copyright 2020 ridiculous_fish, under MIT or Apache-2.0.
The pinned upstream archive, original licenses and precise modification record
are under `vendor/regress/`. Adaptations cover per-execution work limits and
source-verified legacy/UnicodeSets compatibility. No Lexicon code/assets are
included in this component.

## Smart-playlist calendar calculations

The pure evaluator uses Chrono 0.4.45 and Chrono-TZ 0.10.4 under their declared
MIT OR Apache-2.0 licenses, with compiled IANA time-zone data. Exact transitive
dependency identities are recorded in `Cargo.lock`; preserve upstream licenses
with corresponding source distribution. No external date/JavaScript runtime is
required by playlist evaluation.

## Duplicate matching normalization

The independent maintenance matcher uses the default character map from
[slugify 1.6.6](https://github.com/simov/slugify/tree/v1.6.6), copyright
Simeon Velichkov, under MIT. The formatted map, complete license and provenance
are in `vendor/slugify/`. The application does not run its JavaScript source.

Pinned ICU4X Collator 2.3.0 supplies deterministic Unicode root collation under
Unicode-3.0. Cargo resolves the exact data/dependency checksums. Preserve its
upstream license with corresponding dependency source. Duplicate matching
includes no proprietary application source or assets.
