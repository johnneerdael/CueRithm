# File organization API

The desktop bridge delegates to `cuerithm-library` organization services. Incoming and manual organization share the planner, native file coordinator, reference receipts and recovery history. Commands do not accept source or destination display paths as mutation authority.

## Configuration and native roots

`PreparationSettings.organization` defaults to `leave_in_place`. Its mode is `leave_in_place`, `rename_in_place`, `move` or `copy`. Other fields are `pattern`, `missing` (`block`, `omit_optional`, `skip_optional`), `organization_optional`, `destination_root_id` and `collision` (`block`, `skip`). Skip policies require explicitly optional organization.

`choose_organization_root` opens the native folder picker in Rust, retains the native path losslessly, and returns `{id, display_path}`. `organization_roots` lists those projected grants. A display string is never converted back into filesystem authority. Replaced directories and stale grants require revalidation.

`organization_example(settings)` validates the same naming grammar used for actual plans and returns invented examples. It does not read or modify library audio and does not create an apply plan. The default pattern is `{artist} - {title}`; accepted native titles may already contain their mix/version.

## Preview and apply

| Desktop command | Purpose |
| --- | --- |
| `preview_organization(operation_id, request)` | Freeze a complete scope and rule, source/metadata/path evidence, destination occupancy, owned sidecars and known references. |
| `organization_preview_page(id, offset, limit)` | Page display-only rows from the frozen plan, with complete totals independent of the requested page. |
| `discard_organization_preview(id)` | Discard an unused preview without discarding required publication or recovery evidence. |
| `apply_organization(operation_id, preview_id, revision)` | Apply eligible rows from that exact server-side plan. The revision is an opaque string. No client-computed paths, row subset or totals authorize publication. |

Scopes are tagged objects: `tracks` with `track_ids`, `playlist` with `playlist_id`, `folder` with `root_id` and `recursive`, or `all_ready`. A selected scope is the complete selected ID set, including unloaded table pages. Incoming uses its captured preparation settings and its own checkpoint, rather than pretending to be a Ready manual scope.

Preview totals distinguish `total`, `ready`, `unchanged`, `skipped` and `blocked`. Applying eligible rows from a partially blocked plan leaves the other rows explicitly accounted for. Equal bytes at an unrelated existing destination do not authorize adoption or overwrite.

The complete batch conservatively checks staging and retained recovery storage before publication. Native database repair can retain a backup per track; that storage amplification is included rather than assuming one database backup for an entire batch. Available disk space can still change after preflight, so native write failures retain journaled evidence.

## Lifecycle and cancellation

| Desktop command | Purpose |
| --- | --- |
| `organization_history()` | Read durable batch summaries and recovery/Undo eligibility. |
| `organization_batch_page(id, offset, limit)` | Read actual per-track outcomes and reference status. |
| `cancel_organization(operation_id)` | Cooperatively cancel a currently owned desktop worker. |
| `cancel_organization_batch(id)` | Request durable cancellation for an active worker, or synchronously retire safe unpublished pending rows in a stopped batch. Staged/published rows retain recovery evidence. A batch ID is distinct from a desktop worker ID. |
| `recover_organization(operation_id, id)` | Resume a persisted batch from verified native evidence. |
| `undo_organization(operation_id, id)` | Plan and execute the guarded inverse against current file/reference evidence. |

Native desktop work takes a `MaintenanceTasks` lease. Shutdown cancels the active token and waits for its worker. Navigating away cancels unused planning and disposes a late unused preview; an already applying batch remains recoverable in history.

An Incoming-owned batch exposes `resume_job_id` only while its current job needs attention. The UI routes **Retry in Incoming** through `retry_job(use_captured_settings: true)`; generic recovery and Undo cannot take ownership from preparation. Queued or running Incoming work has no manual resume action.

A solver-only settings change is transport configuration. Retrying an existing preparation checkpoint keeps its captured settings, accepted tags and durable organization/reference receipts. New naming rules still require an unpublished organization plan. Before a checkpoint exists, retrying with new solver settings uses that transport configuration and may reuse the prior analysis only after the normal source, engine/model, decoder, clock and PCM checks; changing analysis or tagging semantics is not equivalent.

Batch totals separately report `completed`, `unchanged`, `skipped`, `blocked`, `failed`, `recovery_required` and `cancelled`. An error can follow partial publication, so mutation commands emit library and organization change notifications on both success and failure. Fetch history to establish the outcome; a rejected invocation alone does not prove that no file changed.

## Copy originals

`organization_alternates()` returns projected alternate records with an operation ID, stable track ID, display path, lifecycle state and captured/observed hashes. Only a `conflict` state offers a new user decision.

`resolve_organization_alternate(operation_id, id, resolution)` accepts `ignore_this_revision` (Keep managed copy) or `add_as_new_incoming` (Add changed original as Incoming). A resolution is bound to the observed native identity and byte revision; it is not an instruction to ignore future bytes. The command takes a cancellable maintenance lease, persists the decision, then resumes manual recovery or queues the original Incoming job with captured settings. The choice survives a cancelled or failed continuation. Adding a changed original creates its distinct Incoming job atomically with local path acknowledgement. Native paths remain in the library and are never supplied by the client.

## Evidence boundary

Known native references require path-only updates, closed-target/stale checks and durable receipts before the old path can retire. Independent copied-media exports differ from desktop references. Unknown external libraries cannot be promised repair. Source-derived adapter behavior, synthetic native fixtures, native application reopen checks and physical player acceptance remain separate evidence layers.

Playback continues from immutable private PCM/source snapshots with its stable track ID and position. A path-only change does not require reanalysis or mutating the historical decoder origin recorded in `PreparedAudio`.
