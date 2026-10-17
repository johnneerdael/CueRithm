# Third-party source notices

This file records the new optional OpenKeyScan backend components. Other model
provenance is retained in the corresponding `models/` directories.

## OpenKeyScan

The fixed key network/frontend specification follows
[rekordcloud/openkeyscan-analyzer](https://github.com/rekordcloud/openkeyscan-analyzer),
commit `d5cac64bc7195826b6eb10c55b9b0c97fb3287c0`, copyright 2025 Alexander Sommer.
Its MIT notice is preserved at `models/openkeyscan/LICENSE.upstream`.
Model/checkpoint identities and the fact that weights are external are in
`models/openkeyscan/model.json`. CQT filter-bank arrays were generated with
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
