# Library import desktop API

The desktop bridge lives in `apps/desktop/src/import-api.ts`; serialized types
live beside it in `import-types.ts`. Commands use the shared library importer.
They never enqueue Incoming preparation. The default policy is Store as imported:
key/energy analysis, cue generation, hotcue replacement, enrichment, cleanup,
audio-tag writes and relocation all start off, independently of Incoming settings.
`fixtures/library-import-ipc.json` is checked against both Rust serialization and
the frontend's policy defaults, including presence, capability, clock and result
spellings. Missing, null, empty and deleted values remain distinct.

## Source authority

`discover_import_sources` returns candidates only. `choose_import_source` opens a
native file/folder chooser for a provider and returns an opaque `selection_id`,
a display location, total artifact count and at most20 display artifact names.
`add_import_artifacts` opens a native multi-file chooser for additional VirtualDJ
or Serato volumes. Every file keeps its actual selected root and original
file/WAL/object/hash guards. There is no synthetic source-tree copy.

For rekordbox, select an existing discovered/configured target, then call
`choose_rekordbox_import_source` with its target ID and an operation ID. Native
key provisioning remains backend-only and runtime-only. Referenced analysis
files are captured under the selected installation's actual share root and
explicit Windows compatibility mappings. Reopening requires selecting local
access again; no key enters requests, snapshots, history or errors. Read access
does not enable native write capability. XML transport is unavailable.

DJ.Studio folder enumeration is restricted to `Database/studio.db` and the six
known music/project record tables. Browser/account storage, aggregate caches,
audio, images and stems are excluded. No experimental writer command exists.

`choose_import_mapping` returns a native folder's opaque `root_id` and display
location. Inspection accepts `{source_prefix,root_id}` mapping choices; the
backend resolves roots. Client strings never become staged or selected paths.
Inspection accepts `selection_id`, binding (`automatic`, explicit `replica` with
`source_id`, or `new_library`), capture-file-tags/history/queue booleans and path
mapping choices. History and queue start off. Inspection does not accept a
client-supplied clock qualification or native root.

## Commands and review lifecycle

Tauri's outer argument names use camelCase; shared request/query bodies use the
snake_case names in their Rust types. Every long command requires a caller-created
`operationId` of1–128 ASCII letters, digits, hyphens or underscores.

| Command | Arguments after the host/app handle | Result |
| --- | --- | --- |
| `discover_import_sources` | none | Candidate list |
| `choose_import_source` | `operationId,provider` | Native selection or null |
| `choose_rekordbox_import_source` | `operationId,targetId` | Native selection |
| `add_import_artifacts` | `operationId,selectionId` | New native selection or null |
| `choose_import_mapping` | none | Mapping root or null |
| `inspect_import` | `operationId,request` | Inspection/profile/counts |
| `import_page` | `inspectionId,query` | Observation page |
| `preview_import` | `operationId,request` | Frozen import summary |
| `import_preview_page` | `previewId,query` | Actual field comparisons |
| `apply_import` | `operationId,previewId` | Durable import receipt |
| `discard_import_preview` | `previewId` | Unit |
| `inspect_import_refresh` | `operationId,sourceId` | Fresh inspection |
| `import_operation_page` | `query` | Redacted history page |
| `run_import_processing` | `operationId` | Proposal-generation counts |
| `preview_import_effects` | `operationId,id` (durable operation ID) | Separate effects review |
| `import_effects_page` | `previewId,query` | Effects page |
| `apply_import_effects` | `operationId,previewId` | Processing outcome counts |
| `recover_import_operation` | `operationId,id` | Recovery counts/errors |
| `undo_import_operation` | `operationId,id` | Undo counts/errors |
| `imported_item` | `id` | Active fields, provenance and observations |
| `imported_collection_page` | `id,query` | Occurrence-preserving collection page |

Preview requests select tracks/collections by native IDs or `all`, include the
explicit full policy, reviewed identity assignments and field conflict decisions.
Apply accepts only the server-held preview ID; displayed paths/counts/field
values are never apply authority. Generated/provider/file proposals require a
separate effects preview after generation completes. Existing library methods
retain their `(cursor,limit)` preview-page signature; the IPC adapter accepts the
common `{cursor,limit}` query envelope.

Pages default to100 and reject limits outside1–1000. Observation `kind` is track,
collection, membership or issue. Cursors are opaque and bound to their inspection,
preview, collection revision or history source/runtime/revision. History is
newest first and omits capture requests, source paths, native identities and
credential references. Any library write invalidates a history cursor. Refresh
the page after a stale-cursor error; do not continue with an old cursor or merge
pages by row position. Selections use native IDs.

Each long command holds the existing exclusive maintenance lease through its
worker's completion. `cancel_organization` with `operationId` is the shared
cancellation transport. The Host owns the import processing service and cancels
it during shutdown before waiting for maintenance work. Cancellation cannot
authorize apply or turn an unfinished stage into success. The
`library-import-progress` event contains operation ID, stage, completed count and
a redacted issue summary; `library-change` requests a fresh library projection.

Errors retain the structured `{code,message}` envelope. `cancelled`,
`maintenance_busy`, `stale_cursor`, `stale_preview`, `unsupported_profile`,
`source_busy` and `recovery_required` are actionable states. Import acquisition
errors use redacted messages. A stale preview requires a new inspection/review;
restart invalidates uncommitted preview authority. Durable operation receipts
remain idempotent. Recover/Undo still verify later local/source/media edits in
the backend; UI action availability never bypasses those guards.
