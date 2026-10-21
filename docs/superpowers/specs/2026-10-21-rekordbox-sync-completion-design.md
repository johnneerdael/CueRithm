# Complete direct rekordbox synchronization

Date: 2026-10-21

Status: approved in conversation on 2026-10-21; implementation in progress.

## Purpose and baseline

Let a home DJ send prepared CueRithm tracks, playlists, tags, artwork, cues,
loops and beatgrids to a supported rekordbox 6 or 7 library, understand conflicts
before writing, and recover an interrupted or unwanted sync.

The existing five external-player integrations are implemented and remain the
baseline. This delivery adds the separate direct rekordbox target; it does not
reimplement VirtualDJ, Engine DJ, djay Pro, Serato or Pioneer USB.

Reuse the requirements in `docs/superpowers/plans/2026-10-19-rekordbox-direct.md`.
The encrypted reader, coherent copied snapshots, target discovery and durable
import observations already exist. Initial field baselines are explicitly
unacknowledged, and an import does not establish synchronized ownership.

## Approach

Extend the current native rekordbox adapter and library orchestration, using the
existing desktop Sync workspace and publication/recovery concepts. Keep native
schema interpretation in Rust and separate from React and Tauri commands.

Two alternatives were considered. A standalone second Sync workspace would
duplicate previews and recovery interactions. Replacing all existing adapters
with a new universal sync engine would expand the task and risk functioning
targets. Extend shared contracts only where rekordbox actually requires it.

## User workflow

1. Select rekordbox, discover/select its local library and choose Full,
   playlist/folder or modified scope using existing Sync conventions.
2. Run preflight. Report unavailable locations, open target processes/background
   agents, unresolved recovery and unsupported schema profiles as actionable
   states. Discovery or read support alone never enables writing.
3. Preview track fields, cue/grid/artwork changes, playlist hierarchy/order,
   MyTags and rule fallbacks. Show concrete source and destination values and
   identify CueRithm-owned versus externally owned objects.
4. Resolve conflicting values explicitly. Retain a keep-target choice and a
   use-CueRithm choice; neither timestamp nor matching names selects a winner.
   Skipped conflicts remain visible and must not be acknowledged as synchronized.
5. Apply the reviewed preview after rechecking source, target, chosen decisions
   and process state. Publish a durable backup and recovery journal before the
   first target exchange.
6. Show committed changes and retained conflicts. Offer guarded Restore and
   interrupted-operation recovery through history after restart.

Preparation can make tracks eligible for a later sync; it does not continuously
write into an open rekordbox library. No collection XML transport is introduced.

## Identity and reconciliation

Scope native IDs by persistent target-database identity and retain native UUIDs,
master-song identities and source/asset relationships. Preserve mappings across
renames. Native ID reuse with conflicting identity requires explicit resolution.

Reconcile current local value, current remote observation and acknowledged
baseline per field. Keep missing, null and explicit-clear states distinct. Treat
first synchronization as an unacknowledged decision, including equal values;
acknowledgement follows committed publication or an explicitly accepted remote
value. Observing a newer remote snapshot cannot erase the earlier baseline.

Keep track fields, manual cue/grid publication and playlist ownership separately
versioned. A field-mapping or key-conversion change invalidates a prior preview.
Never overwrite unrelated native records or infer ownership from display names.
Target-side removals do not implicitly delete local music.

Existing imported raw playlist/tag/rule records must become typed local entities
through an explicit import/reconciliation transaction before being presented as
editable local equivalents. Preserve opaque unsupported payloads and original
native identities; do not silently turn an import into preparation or overwrite
existing ready-track metadata and manual edits.

## Native publication

Enable writes only for explicitly identified and verified schema profiles. Keep
read-only recognition separate from write capability. Unsupported versions fail
preflight with a specific reason. No rekordbox 6 acceptance currently exists on
this host; fixtures alone cannot close that requirement.

Stage encrypted database updates and required analysis/artwork assets together.
Preserve unknown binary chunks, foreign relationships and unowned assets. Use
verified native ID allocation, update counters and local-update semantics. Cue,
loop and grid conversion must use the same playable clock as CueRithm preview,
and support variable as well as constant grids where required by the contract.

Export ordinary playlist hierarchy/order and MyTags with stable owned identities.
Export smart definitions natively only where the complete rule semantics are
supported. Otherwise offer an explicit whole-list materialization using captured
membership while retaining the original local smart definition.

Resolve no-op syncs without rewriting destination files. Propagate deletion only
for objects proven to be owned under the existing synchronization policy;
retain externally matched objects. Preserve later destination edits during
restore and report a conflict rather than overwriting an unknown file version.

## Recovery and process safety

Reuse the existing recoverable-publication boundary where compatible. Native DB
and asset changes require a coordinated journal: a database commit alone cannot
be reported as a successful sync. Persist the intent, exact before/after proofs,
backup locations and acknowledgement state so restart can settle the operation
idempotently before another preview/apply begins.

Recheck foreground and background target processes before publication. Never
kill rekordbox or its agent, delete WAL/SHM files, or bypass a busy target. A
backup must include the coherent native state and affected resources needed for
the promised restore. Redact runtime key material from settings, errors and
diagnostics. Do not ship a Python or Node runtime dependency.

Process preflight alone is insufficient. The eventual publisher must retain
native exclusive database ownership across protected writes and must reject an
agent starting during publication. Design database/assets publication around
SQLite's actual sidecar and locking lifecycle; do not assume a file exchange of
an open SQLite database also transfers its locks or coherent WAL state. Native
acceptance must exercise that boundary before production writes are enabled.

Observed schema profiles may support read-only inspection and owned staging.
They never authorize production writes. Production acceptance additionally binds
the installed app/build and native database/asset protocol, not just SQL schema.
The production write allowlist remains empty until isolated native reopen,
playback and recovery acceptance passes for a specific profile.

## Architecture and interaction

Keep schema profiles, native record/asset conversion and staging in the existing
rekordbox Rust layer. Keep per-field reconciliation, scoped identities, revision
checks and coordinated library acknowledgement in `cuerithm-library`. Expose
typed preflight, preview, decisions, apply and recovery results through Tauri.

Extend the current Sync view with rekordbox-specific discovery, per-field
conflicts and read-only/unsupported states. Preserve current controls and
preferences for the other five targets. Ignore late results after destination
or scope changes, discard abandoned previews and show recoverable versus
unrecoverable failures explicitly. Keep platform-native availability accurate;
Linux must not advertise a locally installed rekordbox target.

## Validation and definition of done

- Authored fixtures cover supported schema profiles, identities, explicit clears,
  first-sync decisions, concurrent edits, rename/removal ownership and rules.
- Binary fixtures cover cue slots, loops, grids, opaque-chunk preservation,
  artwork/analysis references and clock conversion.
- Integration tests cover stale previews, later target edits, process guards,
  no-op repeats, coordinated rollback, injected failures between exchanges,
  restart and guarded restore. Keep keys and proprietary/library data private.
- Desktop tests cover conflict resolution, unsupported/read-only destinations,
  scope changes, cancellation, late results and recovery after restart. Verify
  rendered screens and preserve existing Sync workflows.
- Run required Rust workspace tests, formatting, strict Clippy and release build,
  plus desktop tests/type checks/build and user-guide validation.
- Separately exercise isolated test libraries in actual supported rekordbox 6
  and 7: close target, preview/apply, reopen, inspect metadata/artwork/MyTags,
  ordered playlists/native or materialized rules, cue/loop/grid playback, no-op
  repeat, edits made in each app, interruption recovery and restore.

Report implementation, fixture tests, actual application acceptance and release
publication as separate outcomes. Do not describe a profile as accepted because
its generated SQL transaction succeeded. Keep unsupported profiles unavailable
until their own gates pass; do not mark the complete 6/7 requirement finished
while either remains unaccepted.

## Exclusions and next delivery

Do not add new player formats, mobile/cloud services, collection XML, automatic
library deletion or changes to validated cue detection. Manual cue/template
editing is a later home-DJ delivery; this work must preserve any existing manual
publications and leave the publication boundary usable by that future editor.

After this delivery, finish dedicated maintenance screens using the existing
native APIs. The remaining home-DJ deliveries are recorded separately in
`2026-10-21-home-dj-roadmap.md`.
