# id3 native-record preservation patch

Vendored from the locally cached crates.io `id3` 1.17.2 source (upstream rust-id3, MIT license). Original crates.io archive SHA-256: `ef8be496a46468ba3d43690775aa3a79102ce4daf7d99e8072589aae1a3cd955`. No upstream fixtures or media are included.

The three tag decoder frame insertions in `src/stream/tag.rs` append decoded frames directly instead of calling the editing API `add_frame`, which replaces records sharing an identity. This retains duplicate decoded APIC pictures, POPM owners, COMM identities, and other frame objects and flags supported by the upstream decoder. This is decoded-record preservation, not a claim of byte-for-byte preservation of every possible ID3 encoding. Editing API replacement semantics remain unchanged. CueRithm rejects ambiguous selected POPM/COMM records and selects pictures by ordinal plus native identity and SHA-256.

Regression coverage lives in `crates/cuerithm-autotag/tests/editing.rs`.

The v2.3/v2.4 frame decoder also copies the two already-supported tag/file alteration status bits into decoded Frame objects. Upstream encodes these bits but did not populate them on decode. No other flag or encoding support is changed.
