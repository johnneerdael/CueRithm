# DJ library import implementation record

The user approved the design and all three linked implementation plans on 23 October 2026 and selected Native execution, with a fresh whole-branch review after implementation.

- Base: `446aab645b40dcaad82182c79d96daa7fa27f9cc`.
- Isolated feature branch: `codex/dj-library-import`.
- Author-supplied handover: all 22 entries match the recorded sizes and SHA-256 in `dj-library-import-handover/pack-manifest.json`.
- The separate baseline checkout uses that exact base. Earlier onboarding checks on `2219143` are not this baseline.
- A missing bundled-runtime resource in the new worktrees was resolved by linking the already verified local runtime into each ignored runtime path. The initial baseline build failure is environment setup evidence, not a product-test failure.

Execution details and in-progress logs live in the ignored per-plan workspace. Completed test evidence, decisions, qualification levels and remaining native-app gates will be recorded here as implementation proceeds. No source-app or hardware acceptance has been performed by this feature work so far.

## Execution decisions

1. The installed cloud skill package does not expose its referenced execution helper scripts. Equivalent brief, BASE, ledger and test-log bookkeeping is performed locally. The cost is manual enforcement of their checks.
2. Per-task tests use the release profile to reuse native dependencies and bound resource use. Final checks retain debug-specific coverage, including overflow behavior.
3. The reader registry will advertise an unavailable capability until its reader exists. Shared-service tests can inject a synthetic reader before the adapters land; final acceptance tests exercise the actual registry to catch seam drift.

## Shared observation contract

Initial red tests failed on the missing import module. Eight contract tests then passed, including independent default processing, distinct presence states, ordered repeated membership, duplicate/orphan/cycle rejection, depth 128/129, native versus canonical timing, negative/end-only cues, finite BPM and author-manifest integrity. An additional malformed Unicode key case reproduced a validator panic; an ASCII guard now returns a validation error. The broader dj-sync suite passed 394 tests with three explicitly ignored tests including final verification after the minimal cleanup.

## Baseline result

The current-base run completed with 2072 Rust tests passed, 48 failures all caused by the unavailable block-device source, and 11 ignored tests. The frontend passed all 535 tests in 59 files. Exact failed test names and evidence boundaries are recorded in [qualification](library-import-qualification.md).

## Read-side infrastructure

Task 2 passed the full dj-sync suite: 412 passed, three ignored; formatting and all-target Clippy with warnings denied passed. Red/green controls cover committed SQLite WAL data without source DB/WAL changes, stale multi-artifact sets, cancellation/size/symlink limits, explicit drive/UNC/Unicode/non-UTF8 path handling, exact clock-profile inversion, native tag multiplicity, opaque binary payloads, deduplicated storage, bounded record counts, and artwork MIME/type/description/original bytes. Container controls cover ID3v1/v2/APE, FLAC, MP4, RIFF/AIFF, Ogg/Opus and ASF; malformed/unsupported structured values retain raw records with precise interpretation issues. These generated metadata containers are D2 controls, not source-app or decoder acceptance.

Additional execution decisions:

- Per-task tests may set the actively changed repository package to release opt-level 0; dependencies remain optimized. This bounds compile latency. Default optimized release checks remain required at final qualification.
- The concrete SourceAdapter/ReadOptions interfaces landed with ImportSnapshot in Task 2, avoiding a placeholder authority type in Task 1. Later consumers compile against this interface.
- Track observation records use a Box to reduce the enum from a 960-byte largest inline variant; serialization is unchanged. Rust callers must use the boxed variant.
- Archive/XML/TOC/UID malformed-input controls run with their owning reader tasks (9/14/15), where those formats are implemented. Shared snapshot, allocation/count and native-container bounds are exercised in Task 2. Reader qualification remains contingent on those owner tests.

## Durable observations and identity

Task 3 adds migration 029, disk-spooled inspections, bounded record paging, content-addressed raw blobs, source bindings and runtime-bound snapshot authority. Ten store tests pass, including fresh/reopened storage, copied-library replica/new-library choices, artifact-order independence, existing rekordbox observations/links/field owners/reconciliation decisions/journals preserved, atomic cancellation, backup contents and field/row issue accounting. Two regressions reproduced before fixes: nonfinite numbers could become JSON null, and a byte-identical source replacement with a restored timestamp could retain snapshot authority. Validation now runs before JSON serialization; source guards also retain a digest of the native file-identity key.

Broad library testing initially found 495 passes, 57 failures and two ignored tests. Forty-eight failures match the baseline block-device limitation. Nine were older migration expectations/fixtures that assumed schema 28 or retained newer tables while pretending to be schema 18/23. Their expected version and fixture teardown were updated without relaxing migration or restore checks; all 97 affected integration checks passed afterward. The store tests passed 10/10, shared validator/snapshot regressions 12/12, and the dj-sync suite 413 passed/three ignored. All-target Clippy with warnings denied, formatting and diff checks passed. A final whole-workspace run remains required after all tasks.

Additional execution decisions:

- ObservationSink now transports raw bytes through `put_blob(bytes, media_type) -> BlobRef`. References alone could not preserve payloads. Every sink must implement storage; actual blob read-back is tested.
- Backup schema inspection accepts version 29; strict same-schema restore and integrity guards remain intact. Source hashing runs outside the main library transaction, and row/blob transfer checks cancellation between bounded batches. Preview/apply must revalidate the captured source before authorizing changes.

## Imported item projection

Task 4 exposes selected fields without preparation, retains unchecked raw observations, preserves repeated source occurrences and explicit unselected member references, and blocks unqualified outbound items with a capability reason. Missing items can be archived without media access. Eight projection checks pass, including generated WAV playback/waveform decoding without field adoption or jobs. A failing Unicode search witness led to Rust Unicode indexing for linked native tracks without splitting artist names. Existing playlists/tools/playback checks passed 25 tests with one device-only ignore; frontend typecheck and all-target Clippy passed. Private projection helpers are temporarily annotated until their Task 5 preview-service consumers land.

## Immutable previews and refresh

Task 5 freezes selected records, field decisions, media identities and policy behind a backend preview ID. Apply creates bindings, accepted active values, agreed per-field baselines and an idempotent operation receipt atomically; inspection never acknowledges source changes. Unchanged refresh creates no active revisions. Partial scans and failed fields preserve applied values. Hotcue replacement requires both an explicit family decision and separate permission. Changed source/location identities require review. Legacy metadata edits and cleared scalar values remain local edits in both reconciliation and detail/table views.

Forty-one import checks passed, covering source/local/equal/divergent changes, null/empty/reviewed deletion, rollback/retry, discarded/cancelled/restarted authority, source/media races, reused IDs, custom collection ordering/cycles, three thousand streamed membership rows and large-comment byte paging. Fifty-four storage/backup/legacy-import checks passed. All-target Clippy with warnings denied, formatting and diff checks passed. Exact-path auto-linking reuses the qualified native-volume check; unavailable volume proof yields no automatic link. Opt-in content matches are revalidated candidates for review, never automatic links. File-tag key/BPM fallback applies only to absent native fields and keeps its provenance. No source-app/hardware acceptance is implied.

Execution decision: Add a fifteenth import table, `import_preview_effects`, to the unreleased migration 029 for disk-backed immutable proposals and bounded paging. The cost is one more table in legacy-schema fixture teardown and future schema maintenance. Collection fingerprints preserve metadata and ordered occurrence identity while avoiding whole-playlist JSON in previews.

## Selective analysis orchestration

Facts register revision 20 was read in full before Task 6. Applicable boundaries are F001/F003 (captured versus PCM qualification), F005 (versioned profiles), F006 (container versus PCM identity), F009/F051 (authored/detected/native clocks), F049/F077 (ordered multiplicity and negative values), and F118/F120/F128–F131 (no hidden model/branch replacement or promotion). This work changes stage orchestration only; no feature extraction, key/rhythm/energy/cue algorithm, fitted model, weights, training/evaluation split or product label changes. Private findings are cited as constraints, not claimed reproduced.

AnalysisSelection and a selected constructor remove disabled model settings before loading. The selected path skips tonal/timing computation and unnecessary mixed-rate inputs, retaining the existing stage implementations. Additive optional FileAnalysis selection/grid fields keep scalar BPM and grid adoption independent; cue timing prerequisites remain private unless selected. All-off is refused before model loading, and the import coordinator must return before decoding. Existing FileAnalysis fixture literals explicitly retain the legacy None shape. Six generated-PCM controls pass with desktop OpenKeyScan support enabled, comparing actual values and implementation identities with full analysis and testing invalid disabled model paths. Full core verification passed 161 tests with 2 explicitly ignored tests across 28 test targets, including supported-format and original timing/cue controls. All-target core/library Clippy passed. The broad run included one redundant settings-only unit control subsequently removed; the final six selective runtime/value controls passed again with OpenKeyScan enabled after the selected-entry guard was added. Final whole-workspace optimized validation remains pending.

## Frozen import processing

Task 7 freezes independent policy, selected engine settings, model-file hashes and implementation identities at preview. A host-started processor additionally freezes the verified decoder identity; a headless caller with no runtime yet records its decoder when processing and still requires the later actual-effects review. Shared durable stage claims survive cancellation and database reopen. Completed results are not replayed; incomplete proposal attempts are abandoned and retried without publishing active values. Qualified unknown results and unavailable dependencies block their requested stage. Global Incoming settings and mandatory Incoming validation are unchanged. No import is routed through ordinary enqueue/preparation jobs.

Key, BPM, grid, energy and generation create separate proposals. Private generation timing reaches the existing publication template without becoming active BPM/grid. Default all-off and overwrite-only policies create no processing receipts or decode cache. Present null, empty and unknown values are not fill-missing candidates. Preview and worker share the same stage planner, so no-op fill requests and identical refreshes schedule no processing. Validated stage reuse checks source/PCM/clock/decoder, selected settings, model/engine/coordinator identity, result shape and proposal checksum; a mismatch recomputes. Source hashes advance when changed media is accepted. History and queue adoption remain independently off even if an earlier inspection captured them.

Verification: 66 shared import checks passed, including cancellation after claim, concurrent wakes, a panic between stages followed by database reopen, missing media, source/model/implementation/decoder mismatch, cache reuse/rejection, unknown-result handling and a later local edit preserved alongside the proposal's older guard. Fifty-four storage/backup/legacy-import regressions and the six selective core controls passed. All-target core/library Clippy with warnings denied, formatting and diff checks passed. Provider, cleanup and file-publication slots remain explicitly blocked at this intermediate commit; Task 8 supplies their actual handlers and requires a separate effect preview/apply. These are fixture and generated-media controls, not native source-app/device acceptance.


## Desktop import review

The import route uses a single reducer for source, selection, policy, review and
result states. Native selections and mapping roots stay opaque; candidate values
and previews remain server-owned. Users choose an observed DJ.Studio SQL/JSON
candidate by immutable ID, then resolve any independent local conflict in a new
preview. Unchecked analysis, retagging, tag writes and naming remain off.

Existing prepared cues are local conflict evidence; an empty later cue revision
is an explicit local edit. Unknown native pad occupancy blocks generation and
performance export until separately authorized replacement for the current
snapshot. Saved loops remain independent, while loops occupying hot slots protect
those slots. Removed/placeholder native entries remain raw observations but do
not become active cues. Accepted duration and descriptive aliases do not require
analysis. Outgoing Sync uses accepted measured fields and explicit grid anchors,
preserving saved FileAnalysis and existing DSP calculations.


## Refresh and guarded operation history

Refresh uses the saved source binding and last applied choices, then requires a
new inspection and immutable preview. Local conflicts start at Keep local; custom
values require typed backend validation in a rebuilt preview. Partial scans do
not remove source records or music. Saved choices appear in readable form, with
independent controls available for the next explicit revision.

History exposes durable import counts separately from latest-stage state counts.
Backend flags govern retry, effects review, recovery and Undo. A scoped retry
uses the original processing service and gate, retaining completed proposals;
publication journals keep their own recovery/Undo authority. Recovery failures
persist with each effect, including later-edit Undo refusals. Returned summaries
and history issues are bounded without truncating outcome counts. Applying and
applied preview authority cannot be discarded.


## Public-service acceptance and observation guards

Portable authored native artifacts exercise all13 real readers through public
Library APIs. The all-off control exposed a duplicate `locked` field in Mixxx
ScalarBpm serialization: its typed value now carries the lock, while the original
native value remains under `native_bpm_lock`. The generic SDK fixture helper also
checks observation serialization. No DSP calculation changed.

Source process checks now cover original-media tag preview, pre-apply,
publication/recovery and Undo, using the existing bounded Linux executable and
Wine/Proton parser plus fresh Windows/macOS enumeration. M3U remains a transport
without a native app. Imported copy Undo protects later source links. A
current-snapshot unknown pad replacement control exposed that no-field refreshes
skipped the observation guard update; binding snapshot/occupancy guards now
advance even when accepted fields remain unchanged. This does not advance
unaccepted field baselines or trigger optional processing.

The scale fixture exercises100000 available generated media paths, repeated
A/B/A membership,1000-record import pages,500-record Tracks pages, actual SHA I/O,
bounded cancellation and depth128/129. Native app/device acceptance remains
separate from these D2 parser, publication and media controls. Exact fixture
hashes, profile/field evidence and measured host/results are in qualification.


Acceptance review found that stored imported collections lacked persistent UI
navigation. A bounded read-only collection list and Imported playlists browser
now expose hierarchy and occurrence pages after restart, including repeats and
missing entries, without changing editable manual-playlist deduplication. Cursor
filters/runtime/revision and byte limits stay backend-owned. Browser playback
uses the existing resolved-media route; source smart rules remain observations.

## Final whole-branch review

The fresh reviewer examined base `446aab645b40dcaad82182c79d96daa7fa27f9cc`
through `c02e1ae065f5dc01b4eab7393384ffb62835f0e6` against the approved spec,
all three plans, their complete Review Focus and recorded rulings. Native
execution used one whole-branch reviewer (`gpt-6-astra`, ultra), followed by one
primary-agent fix pass. No second review is implied by the regression results.
The primary accepted seven findings as Important by user impact:

| Trigger | Correction in the fix pass |
| --- | --- |
| Publish or Undo key/energy while another native field has a newer local value | Project only the owned effect fields. File journals retain their reconciled display projection across interrupted native changes; unrelated local values are not republished from stale source storage. |
| Analyse an unchecked field, or clean/enrich a descriptive field excluded by the saved mask | Intersect analysis planning, frozen selection, optional proposals, publication and tag patches with the selected groups/subfields. Artist/genre aliases share the same authorization. |
| First file operation on an imported item with artist/genre metadata | Native projection preserves the existing singular/plural representation. Tag patches explicitly resolve accepted aliases; source reconciliation compares aliases without changing stored fields, including a later scalar/array representation switch after adoption. |
| Retry after cancellation or an unavailable enrichment stage | Reconstruct predecessor proposals before continuing, reuse only unchanged-input results and recompute dependent results when necessary. Review/publication order is canonical per item rather than receipt chronology. |
| Approve tag writes, copies or a long generated cue family | Expose actual native before/after tag snapshots and resolved current/destination paths. Render complete scrollable families instead of a4096-character slice. Child IDs remain backend mutation authority. |
| Select a nested playlist without its parent | List the selected child at browser root while retaining its recorded parent and source observation. |
| Select an Engine library containing streaming/history databases | Inventory the reader's supported `m.db`, `stm.db` and `hm.db` artifact set; exclude unsupported `p.db`. |

Each original issue has a reproducing regression; the library, native selection
and frontend controls were observed failing before their respective fixes. The
additional tag-projection control confirms imported array/provenance and visible
artist/genre preservation. Later native BPM/energy edits remain protected by the
existing monotonic metadata-revision Undo guard; restoring scalar values alone
is not permission to erase intervening work.

The reviewer covered shared source identity/WAL/partial scans/media recovery,
adapter units/cue families/raw surfaces/clocks and the disposable DJ.Studio
boundary, and desktop authority/defaults/selection/paging/history/playback.
No new DSP calculation, model or native timing qualification was introduced by
this fix pass; the facts-register revision20 constraints continue to apply as
recorded above.

### Deferred minor

Pioneer USB OneLibrary SQL can contain two distinct cue IDs with the same
positive `kind`, producing duplicate hotcue slot observations without a
slot-specific conflict diagnostic. Raw preservation is intact. Current native
clock/export restrictions limit downstream harm, so this was retained as Minor
and left for follow-up rather than expanded into the authorized Important-fix
pass. Cost: those ambiguous source slots can appear active without that warning.
The relevant reader is `crates/cuerithm-dj-sync/src/import/readers/pioneer_usb.rs`.

### Remaining acceptance and publication gates

D3 real source-library samples, D4 source-app inbound behavior, D5 reopened-app
writes/reference repair and D6 device acceptance remain unrun. Actual macOS and
Windows runtime/CI results are not established by this Linux run. The source
capability matrix remains D2; original source-bound moves/renames remain blocked
pending qualified reference repair, and the DJ.Studio writer remains a disabled
production capability with a disposable-only experiment.

The100000-track M3U/generated-WAV benchmark does not characterize Serato media-tag
scanning or100000 full-length music files. Source-specific Serato throughput
profiling remains unrun. Headless acceptance combines real public Library/native
command controls with React workflow/route tests; it is not a claim of a launched
native GUI or source DJ application.

The user guide now covers import, independent processing, actual effects review,
collection browsing, refresh/retry/recovery/Undo and the file/reference limits.
README, guide navigation and outgoing Sync cross-links point to that guide.
Existing engine all-features, desktop and combined-Windows jobs already include
the new test targets; no redundant CI jobs were added.

No PR, push, merge, deployment or release is part of this task. When a PR is
requested, follow `docs/development/pr-workflow.md`: local checks and public
release notes, a completed Codex review of the latest PR commit with every thread
resolved, refreshed PR gate and green Linux checks, then squash merge. This local
whole-branch review does not replace that latest-commit PR gate. No manual release.

## Complete execution rulings

These are all recorded per-plan rulings, including their costs, retained before
any plan-owned scratch cleanup. They are historical execution decisions; later
completed tasks can close an initially temporary boundary. Current capability
limits are stated in the qualification record and final acceptance section.

### Shared services

1. Skill script resources are absent from the installed cloud packages (three exact resource reads failed). Use this ignored workspace with manually extracted briefs, BASE records, test logs and completion lines — preserves execution evidence without blocking on unavailable tooling — cost if wrong: missing automation checks require explicit verification.

2. Reader registry is consumed by Task 3 before adapters land; define registry profiles as unavailable until each reader is implemented, and test Task 3 with an injected synthetic reader — prevents premature support claims — cost if wrong: injection seam could diverge from production registry, covered by final end-to-end tests.

3. **Task 1: RED observed via rustc against cached existing crate: unresolved cuerithm_dj_sync::import. Cargo test queued behind isolated baseline; exact diagnostics task-1-rustc-red.log.** — Use release profile for per-task Cargo commands to reuse the verified existing native dependency cache — same tests, bounded resource use — cost if wrong: debug-only overflow behavior additionally covered by final debug checks.

4. Per-task whole-crate tests may use release package opt-level=0 for the actively changed repository crate, retaining optimized dependencies — avoids several-minute recompilation of large native adapters — cost if wrong: optimized-only behavior remains covered by final default release checks.

5. SourceAdapter and ReadOptions contracts are introduced with their concrete ImportSnapshot type in Task 2, rather than a placeholder snapshot in Task 1 — avoids an unusable authority type — cost if wrong: registry consumers must use the final Task 2 interface, checked by compilation.

6. **Task 2** — ObservationRecord::Track carries Box<TrackObservation> after Clippy measured its inline variant at 960 bytes against 128-byte collection records — reduces paged record memory with identical serialized representation — cost if wrong: Rust callers must box track records, enforced by compilation.

7. **Task 2** — Archive/XML/TOC/UID malformed-input controls execute with their owning source-reader tasks 9/14/15; Task 2 implements shared snapshot, byte/count, path and native-container bounds — avoids placeholder parsers — cost if wrong: those readers must not be qualified before their owner tests run.

8. **Task 3: RED observed: 28 missing API/type errors, including missing ObservationSink::put_blob.** — Extend ObservationSink with put_blob(bytes, media_type) -> BlobRef — raw references need an actual bounded transport into the durable blob store — cost if wrong: every sink must implement storage, enforced by compilation and blob read-back tests.

9. **Task 3** — Extend library_backup.rs accepted schema range through 29 and test actual backup contents — migration otherwise breaks existing backup support — cost if wrong: strict same-schema restore validation must remain unchanged. Source-root identity excludes artifact selection/order; exact artifacts remain in each snapshot request for scope and refresh. No source hashing is performed while holding the main DB transaction; snapshots are guarded before storage and will be guarded again at preview/apply.

10. **Task 5: RED observed for missing API, then regression witnesses for byte-unbounded pages, reused source IDs, failed fields, legacy edit/clear detection, local detail projection and typed file-tag fallback. All fixed.** — Add import_preview_effects as the fifteenth import table in unreleased migration029 — immutable proposals and paged review must remain disk-backed — cost if wrong: legacy fixture teardown and future schema maintenance include one more table.

11. **Task 8** — Refresh only native journal technical revision/fingerprint proofs after an already reviewed import metadata change or owned tag/organization write, under exact active-field, path, directory-authority, source-byte and reference equality — independent enrichment, tags and naming must compose through the existing guarded services without minting preparation jobs — cost if wrong: cross-service coupling could admit stale authority; final-value, later-edit, new-source-link, combined Undo and actual reopen regressions cover that boundary.

12. **Task 8** — Reuse existing metadata/organization fault injection at their native journal transitions and add import cross-link/reopen/Undo boundary tests, instead of adding a second native mutation engine — the same production journals own durable file transitions — cost if wrong: import-specific cross-link windows could escape the reused controls; explicit child-journal discovery and restart tests are retained.

13. **Task 8** — Renames/moves with an original imported source binding block until exact native source-reference repair is qualified; explicit copies preserve the original, and detached synthetic rename tests exercise only the generic organization path — D2 evidence cannot authorize D5 source repair — cost if wrong: original-file organization stays unavailable for some imported sources, disclosed in qualification and the later desktop capability UI.


### Source adapters

14. Installed cloud skill helpers are absent, as verified and recorded by the shared-plan ledger. Use manually extracted briefs, BASE records, logs and completion lines in this plan-owned ignored workspace — preserves the approved workflow — cost if wrong: automation checks require explicit command/result verification.

15. **Task 9** — Expose immutable original_artifact_path/source_root context from ImportSnapshot, while all parser bytes still come only from artifact() — relative paths and stable root-scoped identity cannot use generated private snapshot names — cost if wrong: logical context could diverge from captured authority; original source guards are validated before/after every read and relative-path/source-immutability controls cover this seam.

16. **Task 10** — Preserve duplicate native hotcue slots and their typed issue instead of reproducing Lexicon's slot-incrementing behavior — the approved observation contract requires original identities and explicit conflict handling — cost if wrong: later UI/publication must block or resolve ambiguous slots instead of silently assigning a new hotcue; end-to-end acceptance owns this guard.

17. **Task 10** — Native multi-root selection is owned by Task19's backend entry points; these parsers retain each captured database artifact's distinct authority/identity and only claim D2 for coherent copied-source fixtures — prevents inventing cross-volume authority in a parser — cost if wrong: multi-root snapshot support must land before the desktop can advertise live multi-volume import; original guards may never be replaced by an unguarded synthetic copy.

18. **Task11** — Decode the legacy16-byte fallback for both BeatGrid-1.0 and BeatGrid-2.0, only after protobuf parsing fails and only with backend-established source byte order — pinned Mixxx src/track/beats.cpp253–305 dispatches both versions to the same fallback, stronger evidence than the handover's narrower1.0 wording — cost if wrong: historical unsupported variants could be read, so exact16-byte/finite-value/known-profile guards and both endian fixtures apply; no D4 or canonical-clock authority follows. Public source inspection copy stays in ignored ledger directory.

19. **Task11** — Keep inbound protobuf decoding separate from strict outbound writer validation — inbound must preserve unknown wire fields and optional presence without relaxing output byte contracts — cost if wrong: duplicated codec knowledge; separate existing writer regression suite is mandatory. ReadOptions.source_byte_order is backend-only and defaults None in library imports until source architecture is independently established, never copied from host OS or source labels.

20. **Task12** — Native Serato cues/grids are file-owned observations, so these readers capture read-only media tag provenance even when optional general file-tag capture is off — otherwise a database-only import would falsely claim an empty performance source — cost if wrong: additional audio fingerprint I/O for available media; Task20/22 must measure and qualify the metadata-only large-library path. Native database metadata has precedence; all-off processing still creates no DSP/tag-write/Incoming work.

21. **Task13** — Support the witnessed single-byte grid footer, preserving its value instead of treating it as trailing corruption — independent pack vector plus existing native writer establish the omitted structural byte — cost if wrong: an unsupported footer semantic remains opaque, with no canonical timing or native-app acceptance claim.

22. **Task13** — Reject conflicting same-origin Engine replicas until the user selects a narrower coherent source — no native primary/cache precedence is qualified — cost if wrong: some mixed master/streaming selections need manual source selection; preserves all source bytes and prevents snapshot-order field adoption. Identical replicas retain their raw alternate observations.

23. **Task13** — Preserve smart predicates as opaque source rules, report unresolved mixed smart/ordinary ordering, and count unqualified history rows as individually identified PartialWithErrors — the pack gives no complete accepted dynamic/history profile and no live app is available — cost if wrong: these fields cannot yet act as equivalent live rules/history; never claim native acceptance or infer memberships.

24. **Task14** — Use a bounded graph-based binary-plist/NSKeyedArchiver reader, keeping transport refs and NS UIDs separate; the existing plist crate is only an independent test encoder — avoids recursive expansion/shared-reference loss and supports MIK reuse without executing classes — cost if wrong: more parser maintenance; byte/count/ref/depth/payload budgets plus independent fixtures and source-byte retention constrain the supported profile.

25. **Task14** — Preserve duplicate native cue/loop observations rather than Lexicon's0.01-second loop deduplication — the approved lossless contract owns raw identity and occupied-slot decisions — cost if wrong: UI/effect consumers must report/resolve duplicates rather than silently dropping or reindexing them.

26. **Task15** — Qualify DataModel11 structurally, requiring exact known table columns/types/PKs, six entity mappings, store type/UUID/model identifier and six32-byte hash values, while retaining actual hash facts in the schema fingerprint and reporting mik_structural_profile_only — the pack supplies compiled-model SHA but no actual per-entity Core Data hash bytes, and inventing a native hash allowlist would be false — cost if wrong: structural lookalikes can be inspected read-only; no D3/D4/canonical timing/native Windows acceptance claim or write authority follows. Unknown explicit model identifiers/column/entity shapes reject before rows.

27. **Task15** — Historical bookmark paths qualify only the witnessed root-volume join; external/missing volume paths stay unresolved with components/raw bytes retained — pack only proves root-volume path comparison — cost if wrong: some external files need an independently qualified path mapping/variant before resolution. No portable alias or security scope is inferred.

28. **Task15** — Accept TOC length conventions count*12+20 and count*12+24 while bounds follow actual20-byte header+entries — supplied independent180-byte bookmark declares48 at its44-byte final TOC — cost if wrong: an otherwise bounded unknown length convention stays unresolved; no out-of-bounds access or fabricated path authority. Native writer bookmark parser remains unchanged.

29. **Task16** — Reuse the existing encrypted direct collection read boundary for read-only inspection, retaining its known music-table projection and making all import fields write=false; do not require a write capability merely to observe a copied library — existing source read APIs already accept the authored14-table layout, whereas exact native write protocols remain separately gated and unchanged — cost if wrong: structural read-only observations are D2 until independently qualified, never a new native write/profile acceptance claim. Unknown headers, missing/virtual music tables and unauthenticated key access reject. Runtime key material is backend-only, nonserializable and redacted; production acquisition reuses NativeKeySource/resolve_database_key for supported6/7 contexts.

30. **Task16** — Extend SourceAdapter with a default capture method and reuse original source guards through a crate-private SQLite backup callback — authenticated encrypted backup cannot use the unkeyed generic SQLite opener, and copying an unguarded/decrypted synthetic root would weaken authority — cost if wrong: reader-specific acquisition could diverge; the original database/WAL guard, coherent backup and immutable artifact checks remain shared and are exercised with a live committed WAL fixture.

31. **Task16** — Hold locally provisioned readers by source only in ImportRuntime for repeat import; reopen requires fresh native key provisioning — secrets cannot enter the serializable request or durable snapshot state — cost if wrong: refresh after reopen needs an explicit local installation/access selection, which Task19 must support and explain without asking for key material.

32. **Task17** — Use the existing native device SQL cue contract, Kind0 memory and positive KindN hot slotN-1, rather than Lexicon's reversed helper interpretation — current approved repository writer database.rs297–319 and its both-class native test explicitly preserve these kinds and active flags — cost if wrong: an unqualified alternate producer's normalized family could differ; all raw kinds/rows remain provenance, canonical clocks and D4/D6 acceptance stay unqualified, and the source-profile note discloses the mapping policy.

33. **Task18** — Join SQL and JSON only through a unique exact normalized recorded path and retain all native identities; withhold normalized fields on intra-source disagreement and expose both immutable candidates — the handover explicitly lacks qualified metadata precedence between Explorer and imported records — cost if wrong: a disputed value remains unavailable for automatic adoption until the user resolves it; Task20 must expose candidate choice without forcing DSP/retagging or silently choosing a surface.

34. **Task18** — Unknown SQL migration/schema profiles remain raw, individually accounted music-table observations while recognized JSON records can still be inspected; never run migrations or normalize unknown SQL semantics — future schemas have no accepted fingerprint and read-only provenance must not become a write capability — cost if wrong: unsupported native rows require a later adapter update before normalized import; source bytes and row accounting stay intact.

35. **Task18** — Resolve cue native seconds only from a valid integer selected-structure index or unambiguous explicit time; conflicting explicit time/beat, negative/fractional/out-of-range beats and unqualified loop ends stay unresolved with raw values — static code does not qualify arbitrary-time precedence/interpolation/project semantics — cost if wrong: some source positions remain display-only until a matching profile is verified; no guessed canonical endpoints or RGB properties.

36. **Task18b** — Use the shared file_hash/durable_json primitives but a scoped lab publication journal whose temporary files stay under the experiment control directory — generic FileTransaction.publish_copy creates adjacent temporaries inside recursively scanned DJ.Studio record directories, contrary to this protocol — cost if wrong: a small second publication path needs explicit replace/rollback/readback/fault coverage; no production capability or live source path is exposed.

37. **Task18b** — Effective-key patches own only SQL Tracks.key and JSON camelotKey; note/original/mik/tag-cache key fields remain untouched — there is no reopened-app proof authorizing inferred dependent-field changes — cost if wrong: DJ.Studio may require additional display reconciliation; D5 and user-library deployment remain disabled, and the lab proves only the exact persistence patch.

38. **Task18b** — The root capability is runtime-only and minted solely with a fresh internally created temporary fixture; simulated interruption/recovery retains that capability and private journal evidence — an existing-path constructor would violate the approved disposable-only boundary — cost if wrong: this lab does not prove user-root or cross-process capability rehydration; actual native reopen/restart deployment remains a separate unrun gate. Generated evidence is retained rather than recursively deleting a potentially replaced root on Drop.


### Desktop and acceptance

39. **Task19** — Keep the already implemented shared import_preview_page(preview_id,cursor,limit) signature and adapt a typed query envelope at IPC — all shared service callers already use this guarded implementation — cost if wrong: one thin signature adapter must stay aligned; no second paging or mutation implementation is introduced.

40. **Task19** — Additional artifacts carry an optional backend-selected real root on ArtifactRequest, with original per-root file/WAL/object/hash guards — flattening VDJ/Serato volumes or rekordbox share roots into a synthetic tree would lose source authority — cost if wrong: an additive serialized artifact property needs backward/default and native multi-volume checks. Existing main root/library identity and single-root callers remain intact.

41. **Task19** — History cursors bind runtime nonce and the sole library connection total_changes counter — a conservative cursor invalidation observes recovery/policy updates that do not always advance Tracks revision without another migration — cost if wrong: unrelated writes also require refreshing history, never silently skipping updated entries. Credential refs and raw request/identity journals are excluded.

42. **Task20** — Resolve DJ.Studio surface choices through immutable adapter-authored IDs and normalized observations, never client candidate values — surface choice and local conflict are separate decisions — cost if wrong: adapter normalizations require exact profile tests and older snapshots need reinspection for qualified choices.

43. **Task20** — Unknown native pad occupancy and hot-slot loops protect generation/export independently of decoded cue count; saved loops stay independent — unsupported/undecodable native pads are not evidence of emptiness — cost if wrong: conservative blocks can require explicit replacement or later profile qualification.

44. **Task20** — Project accepted measured fields into Sync without fabricating or mutating FileAnalysis; current export grid requires explicit ordered beats and at least two four-beat anchors — scalar BPM and native bar origins are different authority — cost if wrong: measured grids lacking this export shape remain blocked rather than silently inferred.

45. **Task21** — Add a thin scoped retry command through the existing processing service — run_pending only picks pending/running operations, so blocked/failed/cancelled imports had no working authorized retry — cost if wrong: one additive IPC method and shared gate must remain in parity; no duplicate pipeline or apply payload.

46. **Task21** — History reports latest receipt-state counts, distinct from immediate processed/reused run counts, and bounded persisted per-effect errors — a proposed receipt alone does not prove a cache reuse count — cost if wrong: historical run-level reuse totals are unavailable rather than invented. Issues capped20/operation and1024UTF8bytes; returned recover/Undo errors capped100 with exact counts preserved.

47. **Task21** — An operation with any publication journal, including undone, cannot generate another effects preview or retry completed fields — recovery/Undo retain their own idempotent authority — cost if wrong: users must create a new reviewed import/policy revision for deliberate processing after Undo. Prepared previews cannot be discarded after durable apply starts.

48. **Task22** — No-field refreshes advance observation guards and unknown-pad snapshot identity while preserving accepted values and applied field baselines — newly unreadable native pad evidence must protect export even if all values were kept — cost if wrong: conservative current-snapshot replacement requalification on refreshed unknown pads, extra bounded guard update per selected item. Existing exact snapshot consent rule retained, no automatic processing.

49. **Task22** — Treat M3U as a transport without a native source application; other providers use fresh source-app closure checks for original-media writes — there is no single app behind a playlist text file — cost if wrong: an unrelated player cannot be inferred from M3U alone; existing media/object/hash and linked journal guards remain required.

50. **Task22** — Scale reads use1000-record import pages and500-record Tracks pages — existing Tracks API deliberately caps500 — cost if wrong: more finite Track queries in the benchmark; no production bound is relaxed. Cancel can complete its current two-record track/membership emission; acknowledgment is measured, not assumed at exactly one callback.

51. **Task22** — Add a read-only persisted collection-list API and separate browser during whole-system acceptance — spec requires showing imported hierarchy/occurrences, while existing editable playlist paths deduplicate IDs and had no import navigation — cost if wrong: additive IPC/route/types to maintain; imported collections remain distinct from editable manual playlist operations. Lists bound names to4096characters and page bytes, preserving full names in source observations with explicit truncation flag.

52. **Task22** — Keep private occupancy/planning guards outside public accepted values and provenance — imported details must not label internal safety state as a user/local value — cost if wrong: callers needing that interpretation must use source observations/readiness rather than a private active-field key. Internal generation/export guards unchanged.

53. **Task23** — D3 source sample, D4 native inbound, D5 reopened-app write/reference repair, D6 device and actual macOS/Windows runtime checks remain unrun acceptance gates, not inferred successes or code defects. Qualification stays D2; no production writer promotion. Cost: platform/application/device interoperability remains unproven outside authored automated fixtures.

54. **Task23** — The100000-track available-media M3U benchmark does not qualify Serato media-tag scan throughput. Retain that source-specific profiling gap explicitly rather than extrapolate. Cost: large Serato library inspection latency is not characterized by this branch's benchmark.

55. **Task23** — Selected imported children whose parent is absent from the persisted import are surfaced at browser root, retaining their recorded parent ID and complete source observation. This provides an explicit orphan projection without silently importing unselected collections. Cost if wrong: the omitted folder's visual context appears only after importing that ancestor; existing selected hierarchy remains unchanged.

56. **Task23** — Retry reuses a completed proposal only when its complete frozen input-field map still matches the reconstructed predecessor output; otherwise recompute the dependent stage under the same saved policy and review it again. Effects use canonical per-item dependency order, never receipt append time. Cost if wrong: a later unrelated field edit can conservatively cause extra work/disk reads; successful unchanged-input proposals are retained.

57. **Task23** — Preserve the existing child metadata Undo revision guard even when the later edit concerns another native column or its values were subsequently reverted. Equality of scalar values does not erase intervening work. Cost if wrong: Undo remains conservatively unavailable after those later edits; original import and edited values stay visible and intact.

### Verification close-out

The corrected library suite reports444passed/48exact baseline mount failures/
1ignored, with all125 import-module checks passing. Native units report27passed/
0failed/1ignored; public import acceptance4/4passes. The eight original library
regressions, existing legacy artist-array conflict and added adopted scalar/array
refresh control are green after observed failures. The SDK experimental cohort passes536/536 executed tests
(3ignored); cleanup debug passes13. Workspace formatting, all-target Clippy with
`-D warnings`, release build and corrected sequential workspace doctests pass.
The qualification table retains the full/broad cohorts and their exact source
scope, the build-artifact interference and its resolution, and the48 baseline
failure names. It does not relabel those environmental failures as passing.

Frontend570/570 across65files, production build and strict guide build passed.
The post-compiler confirmation also passed570/570, the production build, strict
MkDocs build and formatting.
Rust-floor and actual macOS/Windows/app/device runs remain unrun. The isolated
M3U scale evidence retains its authored-media and host limits. No new DSP model,
source clock profile or production DJ.Studio writer capability was promoted.

The final scalar/array refresh control first reproduced a conflict bypass after
an actual earlier artist adoption, then passed with comparison-only alias
resolution. Stored values and their shapes remain unchanged. Final library
units pass444tests with exactly the48 baseline mount failures and1ignored;
all125 import-module controls pass. Public acceptance passes4/4 and native units
pass27 with1ignored. Fresh workspace Clippy, optimized release build, documentation
targets and formatting pass after this last source correction.

The32GiB workspace filled during the last lint/build retry. Only completed,
regenerable test executables were removed, excluding latest/active tests and
preserving source, fixtures, all rlibs/metadata and production binaries. Fresh
sequential retries closed those resource errors. The earlier documentation
artifact interference is also closed; neither failed check is represented as a
passing run. All57 rulings and the single deferred Minor are retained here.
The feature branch/worktree is kept as requested by the approved final task;
there is no PR, merge or release action.
