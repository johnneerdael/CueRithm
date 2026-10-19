# Native rekordbox adapter

This crate implements the encrypted read/snapshot foundation for direct 6/7
integration. It exposes no collection XML transport and currently enables no
target writes or desktop Sync action.

`ReadOnlyDatabase` opens an existing regular encrypted file using a runtime-only
`DatabaseKey`, returns schema/integrity diagnostics and captures an owned
standalone encrypted snapshot. SQLite's backup API includes committed WAL data.
Capture is cancellable between native page batches; dropping a result removes
its private temporary directory. Input keys are zeroized/redacted and never
serialized or placed in SQL strings.

The diagnostic example accepts copied data and a private key file:

```sh
cargo run -p cuerithm-rekordbox --example inspect --release -- \
  --database /private/test/master.db --key-file /private/test/key-file \
  --snapshot-cache /private/test/cache
```

Output contains aggregate schema/integrity information, not track rows or keys.
The key-file interface is for research; automatic runtime key discovery for the
desktop remains required. Installed7.2.19.0342 was inspected on a copy. No rekordbox 6 target
or actual target-app write/playback round trip has been accepted yet.

SQLCipher 4.10.0/OpenSSL 3.5.5 are built natively from pinned Cargo sources; no
Node/Python or separately installed crypto runtime is required. Preserve the
licenses in `licenses/` and corresponding dependency sources/build materials.
See the [direct contract](../../docs/research/rekordbox-direct-contract.md) and
[implementation plan](../../docs/superpowers/plans/2026-10-19-rekordbox-direct.md)
for the complete remaining import/export, asset, ownership and recovery work.
