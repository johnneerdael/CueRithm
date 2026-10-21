# Curation and preparation preset services

PR4 extends the existing library, metadata writer and Settings snapshots. It does
not add another audio writer or provider runtime. Migrations 021/022 preserve
existing taxonomy IDs and captured jobs.

## Curation

`CurationService` exposes catalog/taxonomy CAS, source-ID snapshots, idempotent
desired-revision updates, explicit re-export, lightweight pending/history pages,
exact publication detail, Retry/Discard and guarded Undo. Library-only categories,
user rating and notes require no audio reads. Existing native rating/comment
semantics remain separate.

The durable head and immutable in-flight publication have distinct revisions.
Mapped children use PR3's metadata publication, native identities and journals.
Link child intent before execution and acknowledge the exact curation revision
with the native metadata transaction. Cancellation retains recoverable evidence;
an older acknowledgment cannot mark a newer desired revision Saved.

The host keeps one shared service and background worker. `curation-change`
events refresh desired/publication state, with scoped library and metadata
notifications. Shutdown requests cancellation before joining preparation and
curation workers. Explicit long Retry/Undo commands use the shared maintenance
lease and cancellation token.

Taxonomy aliases/tombstones protect frozen cleanup decisions. Renaming a label
keeps its identity and permits a captured cleanup decision to finish; changing
the track's category membership invalidates that input and requires a retry.
Shared query
construction covers table pages, full ordered selection and cursor identity.
Smart projections distinguish `user_rating`/`curation_notes` from native fields.

## Preparation presets

Library methods provide revisioned CRUD, typed projection/overlay, bounded import
preview/import/export, native folder registrations and assignments, and
per-source current-rule resolution. Import/export is limited to 1 MiB. Native
pickers return only validated documents; client display paths never authorize
filesystem operations.

Portable rules are an explicit allowlist. Embedded cue/cleanup structures also
reject unknown nested fields. Global transport, credentials, model paths,
ingestion controls and native destination grants never become portable rules.

Global Settings and assignment changes commit with selected-settings and folder
CAS checks in one short transaction. Native validation and model identity work
occur beforehand. Materializing an effective source snapshot inserts its
immutable settings/provenance without changing `selected_settings`.

Low-level `enqueue(paths, revision)` keeps captured-revision semantics. High-level
intake/current-rule retry resolves each source independently; claimed workers
never re-resolve a preset. Source-folder authorities remain private persisted
evidence. Failed discovery admission stays retryable after reconnection.

`preparation-presets-change` updates preset/folder views. Access-only refresh still
uses `catalog-access-change`, preserving the existing review choice contract.
