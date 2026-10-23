# DJ library import and refresh

Status: proposed design, under handover verification and awaiting user review. No product code has been implemented.

## Intent and agreed direction

Bring playlists, crates, folders and tracks from original local DJ libraries into CueRithm on macOS, Windows and Linux, preserving detailed source metadata and performance data. Users choose what to import and whether processing applies. A later refresh must reconcile source changes with local work rather than overwrite either side silently.

The user requested a clean worktree and independent choices for Camelot key analysis, whole-track energy analysis and replacing existing hotcues. They then explicitly added retagging and renaming to those choices. The shared source-reader/import-and-refresh direction was presented before this document; the user's response added these processing requirements.

Preserve imported data by default is a proposed default, not an explicit user selection. Retagging and renaming must be opt-in even if enabled in global Incoming settings.

## Inputs and evidence

- Implementation base: `446aab645b40dcaad82182c79d96daa7fa27f9cc`, current main when the clean worktree was created, and the latest base cross-reference in the supplied index.
- Worktree: `.worktrees/dj-library-import`, branch `codex/dj-library-import`.
- Supplied index: “DJ library import and synchronization engineering handover”, research date 2026-10-23.
- Full pack location supplied by the user: <https://files.thepi.es/dj-library-sync-handover/>.
- The shared import contract, integration guide, file-tag/provenance contract and validation/acceptance guide have been downloaded and read. Their observations distinguish source facts from proposed behavior and runtime acceptance.
- Access to some remaining files is intermittently rejected by the cloud HTTP CONNECT proxy. `files.thepi.es` has been added to the environment configuration draft, preserving existing custom domains. Successful downloads establish access only for the artifacts actually retrieved.
- Local downloads and per-file hashes are recorded under `/workspace/scratch/dj-library-sync-handover/download-manifest.json`; this local transfer record is not the pack author's integrity manifest. Adapter-specific mappings and fixture integrity must be verified against the remaining pack artifacts before implementation planning.
- The author's `pack-manifest.json` has now been retrieved. Twenty of its 22 entries match both declared size and SHA-256; `mixxx.md` and `fixtures/normalized-import-example.json` still return HTTP 403. Exact results are in the local `transfer-verification.json`. These unavailable artifacts are prerequisites, not passing or skipped validation. Normal connection reuse improved transfer reliability without changing proxy routing, authentication or TLS verification.

Before implementation planning, obtain and read `import-and-sync-contract.md`, `cuerithm-integration.md`, `lexicon-import-formats.md`, `mixed-in-key-11.md`, `dj-studio.md`, `mixxx.md`, `file-tags-and-provenance.md`, `validation-and-acceptance.md`, `verification-report.md` and the referenced fixture manifests. Record downloaded document and fixture hashes locally. Do not commit private libraries, music, credentials, proprietary extracts or identifying reports.

## Current implementation and constraints

`cuerithm-dj-sync` implements native output adapters, including Mixxx on the selected base. Its `SyncLibrary` model represents prepared output metadata, grids, cues and playlists; it is not a lossless source-observation contract.

The existing native rekordbox import stores observations and baselines in `native_sync.rs`, but its new-track path calls `enqueue_one`. `PreparationSettings::validate` requires replacement cues, and the preparation worker runs analysis and optional tagging/organization. Routing a general import through that path would violate independent processing choices and preservation of source analysis.

Reuse the existing guarded tag-publication, organization, reference-repair, cancellation and recovery services where their invariants apply. Extend the existing observation/reconciliation foundations without changing the behavior or identity of existing rekordbox links, sync ownership or journals.

Preserve current DSP calculations. This feature orchestrates existing analysis and distinguishes imported observations from measured results; it is not a key, energy, tempo or cue-model change. Any implementation that needs to modify extraction, DSP or model behavior must first follow the repository's facts-register requirements and explicitly expand this design.

## Approaches

Use source-specific readers feeding a shared import-observation and refresh service. Preserve source records, then project user-selected fields into CueRithm through a previewed operation. Reuse existing writers only for separately authorized write-back and file-reference repairs.

Two tempting alternatives do not meet the request:

- Reusing `SyncLibrary` as the complete import representation loses source-specific metadata, observation provenance and cue/collection semantics that its output projection cannot express.
- Enqueuing every imported track as ordinary Incoming couples import to replacement cues and configured processing. It cannot honor an import with all processing choices off.

The shared observation service is therefore the proposed architecture. Detailed source profiles and field mappings remain governed by the unread full pack.

## Scope and capability boundaries

Cover the index's local source families: direct rekordbox 6/7, Serato legacy 3/Lite, Serato 4/Lite, Engine DJ, Traktor Pro 3/4 NML, VirtualDJ, djay Pro, Apple Music/iTunes exported plist, Pioneer/AlphaTheta device libraries, M3U/M3U8, Mixed In Key 11, DJ.Studio and Mixxx. Preserve profile/version distinctions rather than advertise one universal reader per product name.

Existing project instructions prohibit rekordbox XML transport. Rekordbox 5 XML remains an explicit unsupported policy exception; the Apple Music plist and Traktor NML formats are separate source formats. Do not silently expand direct rekordbox 6/7 acceptance profiles.

Native Mixed In Key Windows storage is uninspected in the pack. Cross-platform reading of supported artifacts and explicit path mapping must be distinguished from discovery/reading of an unqualified native Windows store. Generic MIK cue markers must not be relabeled as numbered hotcues or used to infer downbeat/meter.

DJ.Studio project overrides and its separate proposed write experiment are not a blanket write-back capability. Include the pack's restricted existing-record write experiment against generated, disposable data: explicit profile checks, independent read-back, multi-file journaling and fault recovery. Production write capability remains disabled until its exact D5 native-app acceptance gates pass. New fully analyzed record insertion, project propagation and wider write operations remain independently gated; qualifying scalar metadata does not qualify them. Existing qualified output adapters remain available separately.

Online account ingestion is excluded. Streaming or unavailable tracks encountered in local libraries remain visible references with explicit availability, not successful local audio imports.

## User choices

Every initial import and explicit refresh captures its own selection and processing policy. Do not consult changing global settings to decide whether an already accepted operation may mutate files or replace analysis.

The default mode is **Store as imported**. Field selection and optional processing are separate: selecting source hotcues imports observations; requesting generated cues invokes DSP; enabling overwrite is mutation authorization, not a generation request.

| Choice | Proposed default | Effect |
| --- | --- | --- |
| Analyse harmonic key | Off | Measure key from playable PCM with the existing engine and expose Camelot notation. With it off, retain the chosen source key and provenance, or keep key missing. |
| Analyse energy | Off | Produce the existing single whole-track energy rating from PCM. Do not generate per-cue or per-segment product energy. |
| Generate CueRithm cues | Off | Run the existing validated cue generator explicitly. It does not follow automatically from importing source cues, analysing key/energy or allowing replacement. |
| Replace existing hotcues | Off | Permit the reviewed selected source/generated hotcue changes to replace an active existing hotcue family. Without it, retain active hotcues and surface conflicts; do not repurpose this switch to replace memory cues, generic markers, loops or the grid. |
| Retag / enrich metadata | Off | Propose changes using explicitly selected providers and field policies. This does not itself authorize writing audio tags or changing imported analysis. |
| Apply cleanup | Off | Apply explicitly selected cleanup rules to the metadata proposal, independently from catalog lookup and musical analysis. |
| Write audio tags | Off | Publish the reviewed selected metadata/analysis fields through staged, PCM-verified file writes. It can write accepted imported values without a catalog request; enrichment/cleanup can also remain library-only. |
| Apply renaming | Off | Apply the reviewed naming pattern through the existing organization/reference coordinator. With it off, leave paths and filenames unchanged. |

The saved policy has independent `metadata_enrichment`, `cleanup`, `analysis_fields`, `cue_generation`, `write_audio_tags` and `relocation` components, plus per-family replacement authorization. The user-facing Retagging controls distinguish lookup/cleanup from writing the accepted values into music files. A preset can set several components, but the review lists every enabled effect. Enabling one does not silently enable the others.

For enabled musical analysis, expose **fill missing** versus **replace selected values**. Missing means the selected active field is absent, not empty, zero-default or unknown text that can be overwritten silently. Preserve source observations and generated results separately. BPM and beatgrid analysis are independently selected when offered; choosing either does not implicitly publish the other. Full CueRithm preparation is an explicit disclosed preset, not an import fallback, and still requires separate authorization before replacing existing user cues or writing/moving music.

Renaming means rename in place unless the user separately chooses an existing supported move/copy organization mode. Show the exact proposed mode, destination authority and naming rule before applying it. Do not enable move/copy implicitly through a preset.

Hotcue behavior is defined for both directions of value adoption:

- On a new source-linked item, selected source hotcues are retained as imported observations and can become its active source-cue family without generating new cues. If the hotcue group is unchecked, it remains provenance only.
- On an existing local track, conflicting local and source hotcues are shown side by side. Default to keeping local active cues; selecting the source set requires an explicit conflict decision and replacement authorization. Preserve both sets in provenance/history.
- When generation is explicitly enabled, show the exact proposed generated hotcue family and any discarded/replaced active cues. Without replacement authorization, an occupied active family stays unchanged and the generated proposal is blocked for that family; do not silently renumber, merge or replace it. If the family is empty, a reviewed generated result may fill it without pretending it was imported.

The replacement checkbox does not authorize changing grids, BPM, memory cues, generic markers or loops indiscriminately. Selection, ownership and provenance for those groups remain independent. Missing cues must not cause an unselected generation stage to run. An explicitly requested generation stage may compute prerequisites privately, but only approved result groups are published. Ambiguous/unqualified native clock alignment cannot be hidden by automatically regenerating the source observations.

Choosing key analysis must not enable energy analysis, cue replacement, retagging or renaming, and vice versa. Disabled musical stages must not be computed simply to discard their outputs; the only exception is a documented prerequisite for an explicitly requested stage, such as timing needed for cue generation. An enabled transformation that cannot run becomes an accounted-for blocked/attention outcome rather than silently falling back to another stage.

The initial workflow presents source/profile and location, collection/track and field selection, these independent processing choices, and a review of actual import/update/file-change counts. Users explicitly apply the server-stored preview. Refresh uses retained source bindings and displays the saved policy and all new conflicts/effects before apply. UI layout and component details are left to the builder; these behavior and choice boundaries are not.

## Selectable fields and initial conflicts

Backend and UI must both enforce the pack's independent field groups: descriptive tags; musical key; scalar BPM; beatgrid; hotcues; generic markers; memory/main/load cues; loops; ratings/color/play history; whole-track energy; artwork/custom tags; playlists/folders. Preserve the pack's distinctions within those groups: do not merge rating scales, coerce a load cue into a hotcue, infer downbeats, or treat a history/Auto DJ queue as an ordinary playlist. History and queue imports are separately opt-in.

Offer per-field read/write/lossless capabilities and disabled explanations, qualified by source version/schema/profile and evidence level. Essential identity/location selection is separate. For new items, unchecked source groups remain private provenance and are not published as active values. For existing items, unchecked means leave the active value and its agreed baseline unchanged. Distinguish absent, explicit null, empty and explicit deletion.

For selected metadata on a new item, the default precedence is native library value, then current file tag only when the native field is absent, then optional explicitly enabled enrichment. Cached source tags remain a third observation layer. File-tag capture is a read choice, not a retag or write request. M3U display hints are not a substitute for real file metadata. Preserve arbitrary raw tag identity, multiplicity and binary attachment references; unsupported kinds must appear as exact losses rather than a false “all tags” claim.

Existing-track identity first uses an established source link, then a verified exact canonical path under the source's volume/case rules. Verified content matching requires explicit permission. Equal names, numeric IDs or bytes alone never authorize merging unrelated tracks. A copied database with matching source IDs requires an explicit same-source-replica versus new-library decision.

Before any agreed baseline exists, selected conflicting local/source values require an initial decision; default to keeping local active values while retaining both observations. Support source/local/custom decisions at the selected field or atomic performance-family boundary, and bind them to the immutable preview. Initial equality does not establish historical synchronization. An explicit new measured-result choice is a separately reviewed replacement, not an excuse to overwrite unchecked fields.

## Source observations and local projection

Keep source identity/profile, snapshot identity, native track/collection IDs, source-relative path evidence, availability, typed fields and raw source metadata. Store current file tags separately from source database values and cached tag observations. Retain arbitrary native values without claiming a normalized field catalog represents every tag or embedded artwork.

Represent BPM, grids, keys, energy, generic markers, numbered hotcues, memory cues and loops as distinct optional typed groups with source provenance. Preserve explicit units, source audio clock, decoder/profile identity and unsupported/unknown states. Never create a measured `FileAnalysis` result merely by copying imported values into it.

Preserve hierarchy, collection kind, stable IDs, ordered membership and repeated occurrences. Current manual-playlist helpers deduplicate track IDs; they cannot silently flatten source occurrences. Store source membership occurrences losslessly and add an imported-collection projection capable of showing them without changing the meaning of existing manual playlist operations. Any unsupported UI projection must be visible, not represented as a complete import.

Path mapping uses native path authority and validated roots, not lossy display strings. Separate recorded source paths from resolved paths. Support explicitly selected Windows drive, mounted-volume and macOS bookmark mappings when the qualified source contracts allow them. Do not identify two tracks solely by filename, title/artist, equal file bytes or a database's reused numeric ID.

A source-linked imported item can enter the visible library without decoding audio, a prepared-analysis result or a completed preparation job. Add honest imported/unanalysed/unavailable representations and query projections rather than manufacturing readiness through Incoming. If current playable `tracks` constraints cannot represent missing/unmounted/streaming media, retain that item in a source-item table and expose its metadata and collection occurrences through the library query layer. Playback is available only when playable media is resolved; metadata visibility does not require it. Lazy waveform/technical decoding must preserve imported musical results. A separately disclosed lightweight probe for clock qualification is distinct from full DSP.

## Snapshots, import and refresh

Readers operate on consistent, read-only copies of the complete artifact set, including required SQLite sidecars or multi-file/per-volume libraries. Prefer SQLite online backup from a read-only source connection, including committed WAL content, into a private snapshot. Sequentially copying a changing database and WAL is not a snapshot; filesystem copies require a closed/stable source and its sidecars. Never checkpoint/migrate the live database or use `immutable=1` to hide active WAL. Capture source fingerprints and schema/profile identity before parsing, reject unsupported or inconsistent snapshots, and bound recursion, allocations, blobs and row counts. Observe the full pack's archive/XML/binary parser restrictions, including no external-entity/network expansion and no execution of source SQL/JavaScript.

An import preview freezes the source observation, local revision, selected native IDs, selected field groups, resolved-path/file identities, processing rules, applicable decoder/analysis identity and any file/reference effects. The preview is opaque server-side authority; client display paths, counts and reconstructed requests cannot authorize mutation. Source/local changes invalidate stale work. Discard and cancellation release only unused private work.

Initial import creates durable source-to-local bindings, preserves full raw observations and projects only selected values. Reimporting the same captured operation is idempotent. Collections and records omitted from a selection are not source deletions.

Refresh keeps the last observed source separate from the last agreed/applied field baseline and current local values. Source-only changes can pull selected fields when local still matches that applied baseline. Local-only changes remain. Equivalent changes on both sides can acknowledge equality for that applied field while preserving raw observations; this never acknowledges an unperformed bidirectional write-back. Divergent edits, ID reuse, ambiguous mapping, source deletion and hierarchy/order changes require explicit conflict decisions. A missing field/parser failure is not deletion. No source removal authorizes deleting media. Unselected fields and baselines do not advance.

Refresh inherits and displays the saved immutable policy revision. Unchanged observations create zero new tracks, jobs, cue revisions, collections, file writes or provider requests. A changed item is optionally processed only as disclosed by that policy and the new reviewed preview; updating metadata does not silently reanalyse the whole library or adopt current global settings. Changing processing rules creates a new explicit policy revision. Native field hashes include semantic values, presence, scope/completeness and ordered membership but exclude the scan's read timestamp.

## Optional processing and recovery

Create an import-processing coordinator rather than weakening normal Incoming's existing full-preparation contract. Each run owns a frozen policy and an attempt/checkpoint token. Reuse engine/decoder and accepted-result caches only after the existing source, PCM, settings and implementation identity checks.

Stage tag changes on copies, prove playable PCM unchanged and retain original bytes through existing recovery journals. If renaming is selected, use final accepted metadata, resolve collisions and known external references before retiring any old path, and bind repair receipts to exact target/source identities. Closed-target requirements and supported write profiles still apply. If the source adapter cannot safely repair a known reference, block that row's rename; importing the data alone must remain possible with renaming off.

Retagging and renaming must preserve stable local track IDs, saved analysis/provenance, collection membership and current immutable playback snapshots. Restart, cancellation or failure must not apply an unchecked processing stage or silently mark a partially transformed track complete. Report imported, unchanged, processed, blocked, failed, cancelled and recovery-required outcomes independently. Guard Undo against later local or external edits.

Capture credentials by existing credential-store references only; never place secrets in import snapshots or portable processing policies. Existing fingerprint/network consent requirements remain independent from enabling retagging.

## Integration boundaries

- Add a source-neutral reader/observation API alongside the existing export model, with adapters based on the completed pack rather than guessing schemas from writer code.
- Add source snapshots, bindings, typed observations/provenance, membership occurrences, frozen import plans and processing receipts through additive library migrations.
- Integrate existing native rekordbox observation/reconciliation state with explicit migration/backward compatibility; do not rewrite or acknowledge old baselines as a side effect of import.
- Add bounded, cancellable Tauri commands for discovery, snapshot inspection, collection/track paging, preview/discard/apply, refresh/conflict review and operation history/recovery.
- Extend desktop bridge types and add an import/refresh entry point alongside existing Sync output controls. Existing output sync must retain its qualified capabilities and guarded behavior.
- Reuse existing metadata/tag and organization services behind the independent import policy. Do not route imports wholesale to `enqueuePaths` or ordinary Incoming jobs.

Detailed types, table names, command names, component layout and implementation sequencing are builder decisions after the handover is accessible and this design is approved.

## Validation and acceptance

Use the pack's invented schema/codec fixtures and a separate semantic validator, with provenance for every source profile and unit/clock conversion. JSON Schema success validates only envelope shape. Independently enforce unique namespaced track/collection IDs and per-collection occurrence IDs, referenced records, acyclic/depth-bounded parents, complete record accounting, finite/present numbers, positive canonical BPM/rate, valid key normalization, cue endpoint/clock evidence and bounded decompression/allocation. Preserve unknown native values and valid negative pre-roll; unresolved native timing has no invented canonical endpoints. End-only markers do not acquire a guessed zero start. Test cross-platform path fixtures without claiming native app acceptance from a Linux parser run.

For the shared service, prove store-as-imported with all optional processing off causes no provider requests, decoding/DSP/preparation jobs, replacement of existing cues, tag writes or path changes. Selected source cue observations may be imported without generating replacements. Test each field group on new and existing items, each processing choice independently and representative combinations: key-only/energy-only with existing hotcues, library-only enrichment, writing accepted imported metadata without lookup, renaming without tag writes, and generation with replacement allowed versus blocked. Use generated audio across every supported format for applicable staged publication, clock and PCM invariants. Enumerate captured raw tags and attachments rather than testing only normalized title/key.

Test initial import, repeated apply, refresh, local edits, divergent conflicts, source ID reuse, source deletion, repeated collection occurrences, source/profile changes, stale previews, cancellation, crashes and restart recovery. Include rename collisions, source-app reference repairs, unsupported repair blockers and safe continuation with renaming disabled. Keep mandatory tests and explicitly private/native acceptance tests distinct.

At this clean worktree's base, `cargo fmt --all -- --check` passed before product changes. No functional baseline suite has been run on this base for this feature. Earlier cloud onboarding on an older commit found block-device identity unavailable on overlay/tmpfs; do not reuse those old counts as this worktree's baseline or disable volume checks to claim current sync acceptance.

Run relevant Rust tests, frontend tests/build, formatting and required lint/build checks for the resulting change. Include a 100,000-track/deep-hierarchy bounded-memory, progress and cancellation fixture benchmark. Reuse validated outputs and limit parallelism to the available CPUs. Track the pack's D0 documented workflow, D1 pinned schema/codec design, D2 synthetic controls, D3 read-only sample coverage, D4 source-app-generated inbound acceptance, D5 reopened-app write acceptance and D6 device acceptance separately per field/profile. Synthetic parser success does not enable a production capability requiring an unrun D4/D5 gate. Do not claim every source/profile is complete when its acceptance gate is blocked or unrun.

## Outstanding prerequisites and decisions

1. Apply the saved `files.thepi.es` network-domain addition and obtain the complete handover. Verify the exact field-selection, retagging and source-profile contracts before implementation planning.
2. Review the proposed preserve-by-default processing behavior, including retagging and renaming as independent choices, and resolve any differences from the full pack.
3. Obtain user approval of this design before product implementation, then write the implementation plan.

The required independent builder review has completed. Its initial questions about cue selection versus generation, initial local conflicts, retagging versus file publication, field granularity and acceptance levels were resolved in this document using the shared handover contracts. The second review reported “nothing important left.” This is a design review result, not application validation or approval by the user.
