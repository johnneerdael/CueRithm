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
The key-file interface is for research. A separate Rust runtime key provider now
derives access from installed target resources without Node/Python or Lexicon;
it is not yet exposed through desktop Sync. Installed7.2.19.0342 was inspected on a copy. No rekordbox 6 target
or actual target-app write/playback round trip has been accepted yet.

SQLCipher 4.10.0/OpenSSL 3.5.5 are built natively from pinned Cargo sources; no
Node/Python or separately installed crypto runtime is required. Preserve the
licenses in `licenses/` and corresponding dependency sources/build materials.
See the [direct contract](../../docs/research/rekordbox-direct-contract.md) and
[implementation plan](../../docs/superpowers/plans/2026-10-19-rekordbox-direct.md)
for the complete remaining import/export, asset, ownership and recovery work.


## Discovery and typed collection reads

`DiscoveryPaths::current()` and `discover_targets` inspect installed bundle/PE
version metadata and the configured `masterDbDirectory`. The shared 6/7 settings
file is configuration XML, not a collection transport. Discovery never launches
a target, opens its database or enables writes. Missing/unavailable/nonregular
databases and invalid settings remain distinct. A Windows directory-version hint
is explicitly labelled and is not sufficient write acceptance. Linux reports
local-platform unavailability while copied-database inspection remains usable.

`ReadOnlyDatabase::read_collection` returns a coherent typed collection and raw
record fingerprints. It preserves database/content/UUID/master-song identity,
metadata lookup relationships, native BPM/time units, deleted/service states,
cue/loop fields, ordered playlist links, native smart-rule payloads and MyTags.
Unowned target fields remain raw for future baseline/preservation work; excluded
Plays/Last Played/Danceability features are not added to descriptive metadata.
Only `localUpdateCount` is read from the agent registry. Missing references and
unresolved locations are reported rather than replaced with invented values.

The reader retains raw cue colors and binary-asset references; it does not yet
resolve analysis-file grids, native palette configuration or materialize a
Cuerithm library. Row-budget exhaustion/cancellation fails without returning a
partial collection. Original path and explicit Dropbox-root options resolve
supported local storage locations while preserving their raw counterparts.

The inspector's `--discover` mode reports aggregate discovery without a key;
`--collection` with copied database/key arguments adds aggregate collection
counts and a descriptive metadata projection digest. No track-row export is
printed. These remain SDK/research operations. The library now persists imported
observations and initial unacknowledged per-field baselines. Desktop import/apply,
field reconciliation and direct writes are still pending. See the
[release scope and gaps](../../docs/release-readiness.md).


Windows source builds need a native Perl installation for vendored OpenSSL.
The workflows select Strawberry Perl explicitly with `OPENSSL_SRC_PERL`, because
Git Bash may otherwise pick an incomplete MSYS Perl. This is a build-tool
requirement, not an end-user runtime dependency. The shared action verifies the
native interpreter and required configure modules before Cargo starts.
