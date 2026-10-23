# Synthetic Mixxx schema fixtures

The empty schema39/schema40 fixtures contain no user library, music, account or
credential data. Tests insert invented records and independent literal protobuf
vectors. Native Mixxx acceptance remains unrun (D2 parser qualification only).

Source: Mixxx commit `2086fe0e551b9eb9dd083dd706a5848eb06534d0`,
[`res/schema.xml`](https://github.com/mixxxdj/mixxx/blob/2086fe0e551b9eb9dd083dd706a5848eb06534d0/res/schema.xml).
SHA-256: `75b87df7d3b726dddd110bd8850565486f2679000f1d86684d15288a92b302c2`.

Reproduce with `python3 tools/fixtures/generate_mixxx_import_schema.py PATH_TO_SCHEMA_XML crates/cuerithm-dj-sync/tests/fixtures/import/mixxx`.
The developer-only generator verifies that hash, applies the upstream migrations
only to a new in-memory database, and exports allowlisted empty music tables and
indexes. It checks the schema40 column/index/foreign-key facts against the handover
manifest, then reconstructs each emitted fixture and compares its facts again.
`profiles.json` records both canonical fingerprints and complete facts. The
installed product and Rust tests do not need Python or download source files.

Historical foreign-key targets and defaults are preserved exactly, including
obsolete target names in upstream migrations; tests disable foreign-key enforcement
when constructing intentionally broken native references. Production readers use
read-only snapshots, never migrate or repair the source.
