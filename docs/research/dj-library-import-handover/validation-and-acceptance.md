# Validation and acceptance plan

Research date: 2026-10-23. This distinguishes checks executed on this documentation pack from tests required when an importer/writer is implemented. Research completion is not feature completion. No application build/test result, target-app import result or safe live write is implied by a documentation check.

## Evidence levels and promotion

| Level | Evidence | What it establishes |
| --- | --- | --- |
| D0 | Public documented workflow | Source selection and advertised behavior only |
| D1 | Pinned artifact/source path, schema, byte layout | A reproducible parser/write design |
| D2 | Executed synthetic buffer/DB controls | Internal format agreement for those exact inputs |
| D3 | Read-only sample snapshot decoding with full row accounting | Sample schema/codec coverage; not app write acceptance |
| D4 | Disposable source-app-generated fixture imported by CueRithm; independently compared | Exact profile's inbound acceptance |
| D5 | Candidate write opened, read, edited and saved by exact target app, then independent read-back | Exact profile's write acceptance |
| D6 | DJ device playback/display if promised | Hardware acceptance for that device/firmware |

A field can have a different evidence level from another field in the same adapter. Only promote profiles with recorded inputs, expected values, app/renderer/schema/OS/decoder identities and actual outputs. “Seen” static code is D1, not D4/D5. Documentation with explicit gates can be handed to an engineer without the apps, but it cannot substitute for target-app acceptance before a release claim.

## Checks executed in this research

- Lexicon1.11.15 copied/extracted and importer paths inspected. Synthetic Markers2/BeatGrid vectors decoded through its copied original buffer parsers; exact cue/loop/grid values matched. No real Serato or other DJ source library was imported through Lexicon.
- MIK11.2.6 model/schema and read-only SQLite backup inspected. Portable embedded bookmark extraction matched Foundation resource-value paths for 446/446 songs; all446 beat archives decoded structurally, totaling312,412 timestamps. These are aggregate metadata checks, not analysis-accuracy or playback results. Nested folders/external volumes were not represented sufficiently to accept them universally.
- DJ.Studio4.2.9 native/renderer artifacts and copied persistence inspected. Copied SQL integrity, synthetic in-memory schema/FK checks, REAL BPM, duplicate membership and transaction rollback checked. All copied JSON audio/structure shard paths matched the documented algorithm. Isolated copied sharding and nearest-cue functions also passed synthetic in-memory controls (including UTF-16 surrogate sharding and nearest/tie/out-of-range cases); no app initializer, native decoder, IPC or file writer was executed. No DJ.Studio launch or live write.
- Mixxx pinned source2086fe0 inspected. All schema revisions1…40 executed on an empty in-memory DB to derive schema reference. Invented cue/key/grid byte vectors are supplied. Existing CueRithm main already has outbound Mixxx acceptance records; they were read, not rerun here.
- All JSON files parse; observation-envelope schema and normalized example are checked. JSON Schema only checks envelope shape. Numeric/clock/relationship semantics require the separate validator specified below.
- Documentation link, whitespace, privacy and changed-file checks are recorded in `verification-report.md` after final assembly.

## Semantic validator required by the observation contract

1. Require unique source track and collection IDs within one library namespace and unique occurrence IDs within a collection. Every membership references an observed track or an explicit unavailable external reference; report dangling references. Parent relationships must be acyclic and depth-bounded. Duplicate track IDs in ordered membership are allowed; duplicate occurrence IDs are not.
2. Validate source/schema/snapshot/profile hashes and scope completeness before deriving missing/tombstone state. An input with any unaccounted record is incomplete. The denominator is raw source records, including failures, skipped services and missing media.
3. Numeric values are finite with explicit presence. Published BPM must be positive; zero/default native values are preserved as raw unknown/default states. Sample rates for frame conversion must be positive. No arbitrary 44.1kHz fallback in the canonical layer. Durations are nonnegative when known; negative cue pre-roll is permitted except format-specific missing sentinels.
4. Preserve missing start/end independently. A `validated_profile` cue must carry at least one finite canonical endpoint plus native raw/time-base evidence and timing-profile identity; an end-only range is valid. `unresolved_native` must not acquire guessed canonical endpoints. A loop needs finite ordered endpoints and positive length; a source malformed loop remains raw with an issue.
5. Validate known key tokens against the24-key mapping. Normalized pitch class/mode/Camelot must agree; raw unrecognized text remains available. Reject inconsistent canonical fields instead of changing the original source string.
6. Beat arrays/anchors retain order, raw units and optional flags. Require strict increase where the source format requires it. Unknown beat subdivision is not automatically quarter-note phase. Sparse downbeat anchors, full beat arrays and custom automation are different representations.
7. Enforce byte/count/depth/decompression budgets before allocation; use checked arithmetic on offsets and multiplication. Decode unknown versions/enum fields into retained opaque observations plus issues. A parser error must not appear as “empty cues” or “no playlists.”
8. Enforce field-selection and processing policy against the immutable plan. No unchecked field gets published for a new track or overwritten on an existing one. Saving raw source provenance does not imply selecting it as active metadata. Verify zero file writes and zero preparation jobs for store-as-imported.

## Shared behavior test matrix

| Case | Required assertion |
| --- | --- |
| Whole library / one playlist / nested selected folder | Exact scope and descendants; unlisted library tracks handled according to explicit all/scope choice; partial scan causes no unrelated deletions. |
| Every individual field group | Only selected group changes. BPM/grid and hot/memory/generic markers/loops remain independent. New-track unchecked fields remain unpublished. |
| Store as imported | Tags/key/BPM/cues/grid match selected source values; no DSP/tag/enrichment/move jobs; missing analysis stays unknown. |
| Fill missing / selected reanalysis / retag | Only chosen absent/replacement fields change; source provenance remains; cue replacement is separate; no source writes unless explicitly chosen. |
| Repeat identical import | Zero additions, updates, queued jobs and file writes; same IDs and occurrence order. |
| Source-only/local-only/both changes | Correct three-way decisions per field; ambiguous changes are conflicts; acknowledged baselines only advance on applied fields. |
| Playlist rename/move/deletion | Stable source ID preserved; source removal affects owned projection only after a complete snapshot; local edits preserved/conflicted. |
| A/B/A ordered membership | Three occurrences survive, not a Set of two. Crates remain unordered where that is source semantics. |
| Same basename/different volume, case-fold ambiguity | No automatic merge; exact/source IDs win; issue includes redacted disambiguation. |
| Unicode, `%`, `+`, drive/UNC/file URL | Decode once; exact source form retained; component-safe path remapping. |
| Missing/unmounted/streaming/protected media | Metadata/collections remain visible; no implicit download; no false DSP success. |
| Corrupt row/blob/unknown schema | Per-field typed issue + original retained; remainder accounted for; write disabled on unsupported profile. |
| Cancellation/stale plan/concurrent local edit | Rollback or explicit conflict; no partial cue/library state or baseline acknowledgement. |
| Source database/audio identity before/after | Hashes unchanged for research/import. A read receipt is not a source write. |
| Large library | Bounded memory/streaming decode; progress/cancellation; totals include failures. Benchmark with100k tracks and deep hierarchy, not just synthetic tiny input. |

## Adapter qualification cases

| Source | Must exercise before claiming complete import |
| --- | --- |
| Rekordbox6/7 | Existing accepted profile boundaries; hot/memory/both, same-time markers, sparse extended slots, content ID vs MasterSongID, DAT grid phase/dynamic tempo, MyTags, cloud original/current paths. No widened writes based on Lexicon code. |
| Serato3 | Main/external-volume databaseV2, UTF16BE crates, nesting/parent membership, neworder, smart rules, Markers2/BeatGrid for each supported container, loop-only and unknown entries; legacy Marker variants explicitly reported. |
| Serato4 | Root/location/aggregate DB consistency, optional tables, portable paths, crate aliases/keepalive, source DB vs tag values, cue/grid tags, streaming reference-only cases. |
| EngineDJ | Schema/version branch, master/performance sidecar consistency, zlib size caps, mixed endianness, UUID+origin ID, explicit membership vs inherited links, next-pointer order, active loops, variable grids. |
| Traktor | Pro3/4 NML, VOLUME/DIR/FILE escapes, fractional millisecond cues, Load/Beat/Fade/Loop kinds, -1 hotcue, dynamic grids, stems and histories, smart predicates. |
| VirtualDJ | Main/per-volume DBs, seconds-per-beat Scan BPM vs actual POI BPM, one-based slots, loop beat size, MyLists/legacy folder structures, actions/automix markers and raw extensions. |
| djayPro | Both TSBF and bplist, alignment/pointer tables/date epoch, locations/bookmarks, analyzed vs manual key/tempo exponent, saved-loop dedup losses, root children and parent recovery. |
| Rekordbox5 XML | Researched format only; do not add transport under current project policy. All-source coverage must disclose this exception. |
| Apple/iTunes XML | Library IDs, persistent playlist parents, duplicate membership, smart snapshot, remote/cloud references, fractional duration and BPM field when present. Cues/grids unavailable in this transport. |
| Pioneer/OneLibrary USB | Correct device DB generation/cipher profile, legacy PDB, DAT/EXT tags, cues microseconds vs ANLZ milliseconds, missing audio, collision-safe optional copy plan, history IDs. Hardware only if promised. |
| M3U/M3U8 | UTF8/BOM/legacy encoding policy, CR/LF, relative paths vs URLs, `%`/UNC, A/B/A duplicates, EXTINF hints, all metadata separately read from files; no fabricated DJ performance state. |
| MixedInKey11 | Store/model UUID/version, bookmark variants/moved/external volumes, ordered membership tokens, nested collections, analyzed/tag key and tempo conflicts, keyed archives and generic source markers with no invented slot/type; Windows-native persistence remains separate. |
| DJ.Studio | Both SQL and JSON surfaces as independent observations even at the same path; no automatic mirroring; native+renderer profile, processed identity preservation, effective grid selection, cue beat/time/length, project-local overrides, folder bookmarks vs playlists, sort uncertainty and cache invalidation. RGB loss and sparse marker arrays are explicit; scalar-only BPM changes must not invoke a grid-regenerating helper. |
| Mixxx | Schema39/40 and blob versions, cue factor2 versus frame grids, end-only ranges, all24 keys/tuning, duplicate playlists vs unordered crates, native decoder clock inverse, history/AutoDJ selection. |

## DJ.Studio write qualification

The target writer is not accepted by this pack. It is an implementable experiment with explicit restrictions, not a completed feature. Start with an existing stable record, disposable root and synthetic audio.

1. Record native version, renderer deployment hash, exact schema/migration and JSON shapes. Confirm app/helpers closed; capture all participating files and hashes.
2. Patch one owned scalar field only, inspect every other field/record/file unchanged, independent read-back bypassing aggregate caches, then actual app open/browse/save/reopen. Check precedence against SQL, AudioFileRecord tag/effective values and project overrides.
3. Repeat for all24 keys, fractional BPM, names/Unicode, hot/memory/system marker arrays, removed/default slots and loop lengths. Change one dimension per fixture; keep expected values independent of the writer implementation.
4. Test grid variants and reference consistency without touching alternate grids or existing project arrangements. Compare audio positions using generated clicks; do not infer alignment from JSON numbers alone.
5. Test playlist creation/order/A-B-A duplicates/rename/removal and bookmark sort. Where the app ignores sort or normalizes duplicates, publish that loss and disable a false guarantee.
6. Inject a failure at every journal/database/JSON/cache transition. Recovery restores **all** original bytes; journal remains retryable and cannot be mistaken for a successful acknowledgement. An external edit after preview or before restore must stop the operation.
7. Only qualify new fully analyzed track creation after processed-byte hash inputs, required blobs/structures/analysis flags and first-open reconciliation are proven. SQL index-only addition is a narrower operation and must be labeled accordingly.
8. Do not advance a shared baseline until on-disk read-back and all required app acceptance gates for that capability succeed. Qualifying existing-record tags does not qualify grids, new tracks, playlist ordering or project propagation.

## Definition of done for the implementing engineer

Each source has a reader with a public capability/profile declaration, exact row accounting, field switches, preservation/provenance, loss reports, read-only source proof, repeat-import tests and target-app-generated fixture acceptance. Shared policy supplies preserve/retag/reanalyse choices. DJ.Studio writes additionally have qualified version/renderer profiles, durable multi-file recovery and reopened-app proof. Until those exist, ship a narrower clearly named capability or leave it disabled; do not claim the research itself shipped synchronization.
