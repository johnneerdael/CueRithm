# Evidence register

`Seen/static` means a symbol, format or branch was read in a copied artifact.
`Seen/data` means a read-only query or archive inventory was performed.
`Inferred` means a consequence deduced from those findings. `Unknown` is not a
claim of absence. `Decision` in the handover is proposed CueRithm behavior.
No Lexicon GUI behavior or Windows runtime was exercised in this research.

## Inputs and reproducibility

Only the user-selected backup `21082925.zip` informs the importer profile. The
installed Mac app and the subsequently supplied Windows package were copied before
extraction. No live Lexicon DB, music tags, signing manifest or license credentials
were changed/read for this work. The Windows installer was inventoried but not run.

| Input | Identity |
|---|---|
| Mac Lexicon | Version 1.11.15, build 1.11.15.0 |
| Mac `app.asar` | SHA-256 `85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860` |
| Windows `Lexicon.exe` | PE32+ GUI x86-64; SHA-256 `b467e6f21e6ade632b9f0acfc1ada6e1666b0698e6c8efd7a67ece7a1a57dcab` |
| Windows adjacent `resources/app.asar` | Package version 1.11.14; SHA-256 `36e4d16d65392b1ece849985d7ef1f66dc87cb662e674ec9db310d2499698c5c` |
| Selected ZIP | SHA-256 `3700383645a5ed2de110f8651afe2059632f9375bc7d8091ce38c4c4dc84232f`; database version 26 |
| Windows installer | 744,072-byte downloader stub; SHA-256 `33c75e303a43f14bce8c60358a94c25b5ac2e9e390293349cd4a52378c5c38cc` |
| CueRithm source | Checkout `673c017`; remote `johnneerdael/CueRithm`; this is a source baseline, not a claim about the latest release. |

Private artifact manifests, schema inventory and original symbol offsets are in
`.analysis/relocation-backup-2026-10-23/`. Copied binaries and extracted vendor
bundles remain in the task-owned temporary research directory recorded there.
They are excluded from publication and unnecessary for implementation. Public
documents contain aggregate data and behavioral/schema descriptions, not extracted
vendor implementation. No per-track music paths, titles or labels are included.

The existing code-review graph was queried and was empty, so direct source
research was used. No graph was built or application source modified.

## Vendor behavior evidence

Offsets are zero-based UTF-8 byte positions in extracted
`renderer/main_window/index.js`. Pretty-line ranges identify the local readable
derivative, not vendor-stable source line numbers. Module IDs plus artifact hashes
make the claims auditable without publishing code.

| ID | Finding | Status and source |
|---|---|---|
| E1 | Selection action refreshes existence and chooses file picker for one missing record, folder search for several; present records skipped. | Seen/static: module 81648 wrapper, Mac pretty 371715–371815; Windows 367337–367618. |
| E2 | Two matching passes: parent basename+filename, then filename. NFC, `_pn` stem suffix removal, case-sensitive signatures. | Seen/static: `AutomaticTrackRelocator`, module 81648; Mac class offset 10,023,341; pretty 371559–371701. Windows class offset 9,938,353. |
| E3 | A single same-signature candidate is used even if size differs; among several choose first size match, otherwise first remaining candidate. No audio hash/fingerprint comparison in that method. | Seen/static: same class, Mac 371559–371635. This is not acoustic identity verification. |
| E4 | Occupied paths skipped normally; bulk force deletes destination row while keeping source ID. No explicit playlist survivor rewrite in those bulk methods. | Seen/static: automatic 371588–371613; manual 314155–314193. |
| E5 | One-file collision offers source or target metadata; keeps chosen whole entry and rewires playlist IDs. Duplicate membership collapses. | Seen/static: `ManualTrackRelocator.relocateOne`, module 4121, Mac 314240–314330; playlist module 27380, 320164–320235. |
| E6 | Manual utility has missing/all, extension change, common folders, default backup, default force off; blocks while local path mappings enabled. | Seen/static: Mac 258287–258531 and 262850–262952. |
| E7 | Manual path transformation replaces first case-insensitive literal occurrence anywhere, not a directory-prefix boundary. Destination folder checked; each target file need not exist. Includes archived rows, skips streaming. | Seen/static: `ManualTrackRelocator`, Mac class offset 8,876,824, pretty 314030–314217; Windows offset 8,811,043. |
| E8 | Global location keys are sanitized/NFC/lowercased while automatic filename signatures are case-sensitive. Host-native Node path parsing is used. | Seen/static: helper module 85917, Mac 327390–327406, 327666–327681; Windows 323923–323941, 324199–324215. |
| E9 | Track loops poll cancellation; manual pending changes flush after loop exit. Traversal is synchronous, without abort/deadline passed by this caller. | Seen/static: base module 96225, offset 8,875,355; manual 314126–314217; automatic 371662–371701. Poor traversal cancellation responsiveness is inferred, not observed execution. |
| E10 | Directory helper uses stat before checking symlinks; no visited-directory protection visible in this path. | Seen/static helper 85917, 327278–327365. Ineffective symlink exclusion is inferred; actual cycle behavior unknown. |
| E11 | DatabaseBackup uses connection backup to a temporary database, then ZIP; restore recognizes DB name, closes DB, replaces DB/WAL/SHM and restarts. No path conversion is present in that restore method. | Seen/static: module 38552, Mac class offset 7,425,936, pretty 242280–242443; Windows offset 7,382,516, pretty 240131–240319; underlying backup 318194–318208. |
| E12 | Cue kinds `1..5`, local/streaming kinds `0/1`, playlist kinds `1/2/3`; timing seconds. | Seen/static: cue module 93814 offset 7,158,081; tempo module 22055 offset 7,219,773; pretty 228091–228172, 231705–231716, 235755–235781; seconds conversion 208304–208328 and 196971. |
| E13 | Smart grouping, six sample operators, string/number behavior and key compatibility dispatch. | Seen/static: `SmartlistFilter` offset 8,601,775, 301299–301530; `StringFilter` offset 8,595,176, 300924–301074; key compatibility 228321–228347. |
| E14 | The supplied backup has SQLite magic, version 26, required schema, expected relation counts, zero orphan track links, successful quick check and foreign-key check. | Seen/data: ZIP inventory, `sqlite_master`, `PRAGMA table_info`, aggregate SELECTs, `quick_check`, `foreign_key_check` on copied sample. Schema and identity recorded privately. |
| E15 | Fractional Track.duration values despite INTEGER declaration; negative cue starts; no sample loops/streaming/custom-tag assignments. | Seen/data: storage-class and enum aggregates on copied sample. The importer must not coerce declared integers or treat this sample as coverage of other variants. |

Mac and Windows `DatabaseBackup`, `FileBackup`, `BaseTrackRelocator`, constants
and sanitization modules compare identically as formatted module text. Reviewed
matching/collision paths agree. Mac 1.11.15 adds sample-rate refresh after manual,
single and automatic relocation; Windows 1.11.14 lacks those calls. This is a
version difference between the supplied artifacts. No cross-version runtime or
full-library compatibility result follows from it.

## User-facing documentation cross-check

The [Find Lost Tracks manual](https://www.lexicondj.com/manual/find-lost-tracks)
describes file, folder-search and source/target relocation, extension changes and
an optional pre-operation backup. The [Database Backup manual](https://www.lexicondj.com/manual/database-backup)
describes database-only ZIP backups and destructive restoration. The
[Local Path Mappings manual](https://www.lexicondj.com/manual/local-path-mappings)
describes machine-local mappings used with manual backup/restore across computers.
These pages were read on 2026-10-23. They support product behavior, not the exact
matching implementation or successful runtime reproduction.

## Deliberate CueRithm differences

The goal is equivalent DJ capability. Copying incidental vendor weaknesses is
not required for compatibility with its backup format.

| Vendor finding | CueRithm decision and reason |
|---|---|
| First traversal candidate can win an ambiguity | Review ambiguity with no implicit winner; avoids reconnecting to a different mix. |
| Unanchored manual substring replacement | Component-aware leading root replacement; avoids changing similarly named folders. |
| Global case-folded path identity | Use actual destination filesystem semantics; supports Linux case-sensitive names. |
| Bulk force deletes destination without explicit membership merge | One survivor policy across all flows, reversible playlist rewires. |
| Per-track destination may remain nonexistent | Verify/probe before successful relocation; unresolved mapping has a distinct outcome. |
| Partial changes may survive cancellation | Atomic selected-set DB publication; search cancellation never edits library paths. |
| Host-native parsing of paths | Explicit source path flavor and portable roots. |
| Raw database backup | Versioned logical `.cuerithm`; source SQLite is parsed only for Lexicon import. |
| Vendor cloud/account features | Local backup/import works offline without vendor credentials. |

## Remaining unknowns and implementation gates

- No vendor relocation or restore was executed. Exact GUI cancellation interaction,
  runtime permissions, file races, rollback and indirect tag-writer effects remain
  unknown. CueRithm's explicit no-tag-write decision requires its own tests.
- No Linux Lexicon artifact was supplied or analysed. CueRithm Linux support is an
  independent requirement, not an inferred Lexicon capability.
- The sample does not prove nonempty marker-data semantics, loops, streaming,
  Windows locators or other database versions. Preserve unknown data inert and
  add profiles deliberately; do not invent undocumented transforms.
- No archive writer/parser or cross-platform round trip was built. Format size,
  speed, UI layout and reliability targets remain implementation acceptance work.
- Current checkout owns this integration plan. If an engineer starts from a newer
  branch, recheck the named CueRithm seams/schema and adjust migrations, not the
  independently specified file-format semantics.
