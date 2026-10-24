# CueRithm integration and implementation sequence

Status: engineering proposal informed by source inspection, 2026-10-23. No production source, branch, dependencies or installed libraries were changed. The handover files are the only intended workspace changes.

## Source identity matters

The working tree was clean at start on `feat/release-gaps`, HEAD `673c01750c2b0d2ce7ae8b7950964acc6807ccaa` (2026-10-20). It is older than local `main` / `origin/main` at `190f2f549454e5c6e8486e4949042197351ea14f` (2026-10-23). Read the latter with `git show` for implementation planning; do not build the new import system against the obsolete feature branch by accident. No branch was switched and no remote update was performed. The exact commit is the baseline, not a claim that a remote can never advance. During parallel research another process advanced `origin/main` to `446aab645b40dcaad82182c79d96daa7fa27f9cc`; comparing the two commits showed only an unrelated research-document change. Native USB references inspected later are pinned to446aab6 explicitly. No fetch, checkout or reset was performed by this task.

The repository knowledge graph contained zero nodes/files and had never been built. A graph search returned no indexed code, so research used direct source reads. No new index was built.

All existing-source paths in this document refer to **main at `190f2f5`**, unless explicitly labeled working-tree. Source symbols/line numbers allow an engineer to reproduce the findings from Git without installed DJ applications. Examples and new interfaces in the pack are proposals, not exported APIs already present.

## Existing work to retain

| Owning source | Seen responsibility | Reuse / extension boundary |
| --- | --- | --- |
| `crates/cuerithm-dj-sync/src/model.rs` | `DjSoftware` includes VirtualDJ, EngineDJ, djay Pro, Serato4, PioneerUSB, rekordbox and Mixxx. `SyncLibrary`, `SyncTrack`, `SyncCue`, `SyncBeat`, `SyncPlaylist` are outbound data. | Reuse key/cue/grid facts, but introduce an inbound observation model with absence, provenance, cue families, native clocks and source IDs. |
| `crates/cuerithm-dj-sync/src/formatting.rs:5-71` | Musical/Camelot/Open Key parsing and formatting. | Reuse tested notation conversions, retain raw source spelling and ambiguous values. Avoid turning an unrecognized key into a whole-import failure. |
| `crates/cuerithm-dj-sync/src/discovery.rs` | Installed app/profile/target discovery. | Add read-source discovery and manual profile selection; do not equate an export target with a complete readable library. |
| `virtual_dj.rs`, `engine_dj.rs`, `djay_pro.rs`, `serato4.rs`, `pioneer_usb.rs`, `mixxx.rs` in that crate | Native outbound file/database adapters. | Factor reusable codecs after testing; these modules are not proof of a full inbound library API. Legacy Serato3, Traktor, Apple XML, M3U, MIK and DJ.Studio require additional readers. |
| `crates/cuerithm-dj-sync/src/transaction.rs` | Preview/stage/apply/restore journals, target/source fingerprints, interruption recovery. | Reuse its proven journal design for DJ.Studio once the adapter's multiple representations and read-back rules are modeled. Import into CueRithm uses a DB transaction rather than writing the source. |
| `crates/cuerithm-dj-sync/src/ownership.rs` and per-target `owned` modules | Durable playlist bindings, external-list protection, source tombstones. | Extend source import ownership without taking over a same-name external playlist. |
| `crates/cuerithm-dj-sync/src/timing.rs:44-166,168-203` | Per-target native-clock conversion on export clones, applied once with profile hash. | Implement validated inverse inbound conversion; retain native observations when not invertible or source environment is unknown. |
| `crates/cuerithm-dj-sync/src/mixxx/{proto,beats,keys,cues,schema,decoder,coreaudio,lame}.rs` | Mixxx wire encoders, schema inspection and codec-dependent timing. | Build the reader alongside these contracts. Do not reuse encoder clamping or slot truncation as lossless input parsing. |
| `crates/cuerithm-library/src/native_sync.rs:19-114,458` | Rekordbox-specific raw observations and local/remote field baselines. Required native table allowlist is hard-coded. New local tracks go through `enqueue_one` (line647). | Retain this existing contract while adding a generic provider/source registry; migration11 limits provider to rekordbox. Do not merely add enum strings around this validation. |
| `crates/cuerithm-library/src/native_reconcile.rs` | Field ownership, three-way decisions, canonical comparison. | Reuse decision semantics; add inbound selected-field apply and cue/grid model without coercing arbitrary source payloads into rekordbox rows. |
| `crates/cuerithm-library/src/native_playlist_sync.rs`, `rekordbox_sync_service/` | Native rekordbox playlist/import/sync integration. | Audit current incoming playlist behavior before extending it. Preserve native source identity, incomplete-scan safety and restore gates. |
| `crates/cuerithm-library/src/dj_sync_service.rs:115,370,502` | Outbound preview/apply/restore and acknowledged baselines. `capture_sources` line897 queries only ready, unarchived tracks. | Store-as-imported records need a deliberate export-readiness policy. Do not manufacture completed analysis jobs solely to pass an outbound filter. |
| `crates/cuerithm-library/src/settings.rs:66-82` | Preparation validates full-track analysis and replacement cues. | Introduce a separate library-import policy. Disabling auto-tag alone does not disable full DSP or replacement cue generation. |
| `apps/desktop/src-tauri/src/sync_commands.rs`, `rekordbox_sync_commands.rs`; `apps/desktop/src/views/Sync.tsx`, `RekordboxSync.tsx` | Current desktop export/native sync UI and bridge. | Add source import preview/apply/status commands and a field selection UI; keep export direction explicit. Read newest files from the pinned main tree. |

**Seen:** The outbound crate README names current VirtualDJ8.5, Engine schema3, current djay Yap/TSBF, Serato4 and Mixxx39+ profiles. This is existing scope, not acceptance of every future source version. Existing tests and research acceptance are useful evidence; this research did not rerun the application suite or commercial apps.

## Why the outbound DTO cannot be the lossless import model

`SyncTrack` uses nonoptional BPM/duration, flattened artist text and a `u8` rating. `SyncCue` uses a `u8` index, start time, optional end and active-loop flag; it lacks separate hot/memory/main/load families and native raw IDs/values. `SyncBeat` is explicitly **sparse tempo anchors, each at a downbeat**, not a dense list of every beat. `SyncPlaylist` stores parent/folder/members but not arbitrary source smart rules or ordered-occurrence IDs.

An inbound MIK list of beat times or Mixxx BeatMap must not be poured into `SyncBeat` and labeled downbeats. A memory cue at the same timestamp as a hot cue must remain distinguishable. Unknown BPM must not become zero BPM; an unavailable duration must not become a successful zero-length analysis. The normalized observation fixture illustrates a separate model. Later derive export DTOs with explicit losses/field policies.

## Storage changes for the implementing engineer

This is a logical design, not a migration already written. Use the project's current migration sequence at implementation time; do not edit old migrations or hard-code the next migration number from this pack.

1. Add provider-neutral registered sources keyed by provider + library-instance identity with schema/profile/snapshot records.
2. Add source track identity links and safe raw observations, field presence and per-field baselines. Store unresolved/missing media metadata independently of a playable track row if current `tracks.path` uniqueness/required hashes cannot represent it honestly.
3. Add imported cue/grid revisions with source provenance and clock status. Imported cues are not an engine-generated `PublishedCue` unless every required semantic is represented. Store native-only extras even when normal UI cannot display them.
4. Extend playlist/source bindings to folder hierarchy, membership occurrences, unordered crates, history/queue kinds and materialized smart snapshots. Keep source-owned projection distinct from user's edited playlists.
5. Add operation receipts and inverse changes for local undo. Keep source snapshot hash and policy/selection immutable across preview/apply.
6. Add separately scheduled optional enrichment/analysis/publication jobs. Store-as-imported must create **zero** preparation, tag-write and relocation jobs.
7. Extend UI queries so imported/unanalysed and missing-media records are visible with useful metadata; lazy playback waveform generation must preserve imported analysis. Define export eligibility by required facts and target profile, not “a fake analysis succeeded.”

## Proposed backend surface

Names are illustrative engineering contracts, not existing Tauri commands:

- Discover sources → profiles with provider/version/schema and supported read groups.
- Snapshot source → immutable private snapshot ID, profile and completeness.
- Preview import(snapshot, scope, field selection, processing policy, local revision) → change plan, conflicts, losses and totals.
- Apply import(plan ID, conflict resolutions) → atomic receipt and optional queued work.
- Refresh source(source ID, saved policy) → a new preview; never automatic destructive reconciliation.
- Undo import(receipt ID, expected revision) → inverse plan with later-edit conflicts.
- Preview/apply DJ.Studio write-back → separate direction, strict qualified profile, managed ownership, file journal, complete read-back and failure recovery.

Every capability must be available to both UI and programmatic callers. Do not implement select-fields logic solely in React. Cancellation/status/progress/receipts should use the same operation ID.

## Native clock policy for all adapters

First decode source integer/binary units to **native seconds**. Then, only with a qualified media/codec/source-decoder profile, convert to canonical CueRithm PCM seconds. The export relation is `native = canonical + offset`; invert with subtraction. A profile is bound to source bytes/codec/rate and source environment, not just extension. Preserve original numeric values and source clock in all cases.

Current export code sometimes clamps negative cues or converts a grid to a target-limited representation. An inverse of that output cannot recover information already lost. Import what was observed and report uncertainty; do not invent earlier positions. Source-generated data may not follow the same rules as CueRithm's own exported data. Validate against source-app-generated fixtures before assuming the outbound offset is universal.

Missing media, unknown source decoder, encoder changes, or unqualified app profiles produce native-clock-only observations, not discarded cues. Show them as imported but not yet aligned on CueRithm's waveform. A lightweight technical probe is allowed without full DSP under a disclosed processing policy. Do not require users to install a decoder runtime or Python.

## Implementation order and completion criteria

1. **Shared import foundation.** Implement observation/field-presence model, source identities, consistent snapshots, immutable previews, field selection, store-as-imported route, transactional receipts and idempotent refresh. Prove no-write/no-analysis behavior before connecting real libraries.
2. **Inspectable readers.** Mixxx39/40, Traktor NML, VirtualDJ XML/MyLists, Apple XML and M3U. Exercise ordering, Unicode, missing media, dynamic grids and duplicates. Reuse parsers/fixtures with strict native unit distinction.
3. **Binary/native readers.** Engine3, Serato4, legacy Serato3 tags/crates, djay and OneLibrary USB. Promote a profile only after codec fixtures, source-generated comparisons and record accounting pass. Rekordbox6/7 uses existing direct contracts with no widened acceptance window.
4. **MIK onboarding.** Core Data/entity version checks, bookmark/path resolver, archive codecs, folders/ordered memberships, persisted cues/grid and raw-vs-normalized key metadata. Native macOS path resolution and portable mapping must both be tested.
5. **DJ.Studio read.** Reconcile authoritative JSON/audio structures with its index and project overrides. Make library markers distinct from project-local markers and timeline automation.
6. **DJ.Studio write experiment.** Begin with an existing track's descriptive/key/BPM/marker edits on generated disposable data, preserve unknown fields, journal all related files, independent read-back, then actual app open/save acceptance. Add new-track/folder/playlist creation only when the multi-file invariants are proven. No source-project arrangement rewrite by default.
7. **Qualification and UX.** Execute `validation-and-acceptance.md`, publish capability matrix and precise losses. Review whether support claims need platform, schema and decoder restrictions. Only then enable production write UI.

The order permits parallel adapter work after step1's contract is stable. It is not permission to omit later sources: the full requested coverage remains the destination. Rekordbox5 XML is an explicit unresolved policy boundary, documented rather than silently omitted.

## Source access without this workstation

CueRithm baseline is [commit190f2f5](https://github.com/johnneerdael/CueRithm/tree/190f2f549454e5c6e8486e4949042197351ea14f). The engineer needs that repository and this handover pack. Mixxx primary code is publicly pinned in `mixxx.md`. Commercial app source/binaries and user databases are not packaged. The clean-room adapter contracts and synthetic examples are the portable implementation inputs; identified unknowns need qualification fixtures or an app-enabled acceptance operator before release, not a speculative implementation hidden as support.
