# Organization coordinator

This is an internal foundation. No Tauri command, preparation stage or UI invokes source-audio organization yet. It never marks a track Ready.

The pure `cuerithm-organize` planner supplies a complete reviewed scope. `Library::plan_organization` independently captures Rust filesystem authority and exact bytes for the audio file and each explicitly owned sidecar. It does not trust a preview's native identities or infer ownership from neighboring filenames.

`Planned → Publishing → AwaitingReferences → Cleanup → Complete`

- Persist the full per-track intent, native paths, ancestor identities and source fingerprint before creating directories or staging bytes.
- Open every path component without following symlinks/reparse points. Unix mutations use retained directory descriptors. Windows retains ancestor handles without delete sharing through each boundary.
- Reserve source/destination paths in SQLite. Publish each fully flushed, byte-verified temporary with a native no-replace rename. A pre-existing unrelated destination is never overwritten, even if its bytes equal the input.
- Pause with both files retained at `AwaitingReferences`. The PR2 caller must complete required native-reference repair before `acknowledge_organization_references`. Its transaction changes the local path, persists copy-source ownership and enters Cleanup. Analysis and Ready publication remain the preparation service's responsibility.
- Moves and renames quarantine the captured original under an exclusive hidden sibling name after the path commit. Recheck the quarantined native object and bytes. A raced replacement is preserved and restored only into an unoccupied original path. Quarantines are recovery backups; this module never deletes them automatically.
- Copy mode leaves the original untouched. Both watcher and manual enqueue consult captured source ownership. Changed/replaced bytes create a durable alternate conflict. Explicitly ignoring one path/hash revision does not suppress a later revision. Adding a changed original creates a new Incoming identity and requires new analysis.
- Cancellation of an unpublished intent changes only database state and preserves partial stages/directories, even when its original input changed. Inspect captured native staging identities before cancellation so an unjournaled publication or case quarantine cannot release ownership. A published operation requires recovery.
- Undo is a new linked operation. It verifies all published objects and original parent identities, uses no-replace publication, and follows the same reference acknowledgement and cleanup gates. Copy Undo requires the original source object and bytes to remain unchanged. Later edits or occupied original names block Undo.

## Restart behavior

| Interrupted boundary | Recovery |
| --- | --- |
| Intent only | Remain Planned; no source mutation occurred |
| New directory created before its identity was journaled | Preserve it, expose the error, and require explicit cancel/re-plan; never adopt an unclaimed directory automatically |
| Stage creation before its identity was journaled | Preserve the unclaimed temporary and require cancel/re-plan |
| Partial copy after staging identity reservation | Preserve the partial temporary and start another exclusive stage |
| Complete flushed stage before final fingerprint journal | Accept only the captured native staging identity and expected exact bytes |
| No-replace rename before publication journal | Accept only a missing stage plus the captured staging object and bytes at the destination |
| Waiting for native/local references | Keep the original; never infer acknowledgement from destination existence |
| Local path transaction committed | Resume guarded source quarantine |
| Quarantine rename before cleanup journal | Accept only the captured original native object/bytes in quarantine; a repopulated original path blocks completion |
| Any changed parent, unexpected file or unavailable volume | Retain all artifacts and publish a discoverable error; do not set Ready |

Regular same-volume operations also stage a complete byte copy. This favors independent recovery originals over a hard-link backup that later metadata edits could mutate. Cross-volume moves use the same verify/publish/acknowledge/quarantine contract. Case-only renames have a separate early-quarantine step because their target aliases the original entry. Copying to a case alias is rejected; hard-link aliases cannot authorize a case-only rename.

`OrganizationRecord::retained_bytes` reports retained original bytes. Staging and abandoned partial artifacts are recorded individually with their captured sizes; a storage UI must include those separately and explain offline/unclaimed assets. Cleanup of recovery storage requires a separate explicit ownership-safe operation.

## Validation boundaries

Unit tests operate only on temporary directories and invented byte fixtures. They check byte identity, native identities, retained originals, source/destination replacement, permission denial, no-replace races, journals, restart, nested directories, sidecars, copy rediscovery/conflicts and Undo. Windows-specific fixtures cover junction rejection and preserved read-only files; the Linux fixture covers non-UTF8 native filenames, which APFS cannot create.

These filesystem tests do not prove audio decoder/timeline behavior, linked DJ-reference repair, provider health, or the future Incoming integration. Byte-for-byte publication preserves existing audio/tag records without rewriting their format. The native tests must run on macOS, Windows and Linux before the coordinator is accepted for the PR2 UI.
