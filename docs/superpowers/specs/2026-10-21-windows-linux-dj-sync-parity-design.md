---
title: Windows and Linux DJ sync parity
date: 2026-10-21
status: approved
---

# Windows and Linux DJ sync parity

## Intent and scope

Make the existing DJ sync destinations work on Windows and on Linux when the
DJ application runs through Wine or Proton. The user must be able to select the
installation and library, rather than being blocked by the host operating system
or an exact executable build. Preserve the existing preview, conflict, backup,
source-proof, recovery and ownership guarantees.

The user requested a new PR and is handling the already-merged PR #20 build and
release flow separately. This PR must not change release workflows or monitor
that build. Additional targets and general library import are a scope decision:
the Windows audit found missing Traktor and legacy Serato transports, but adding
them is distinct from platform parity for the existing destinations.

## Evidence and current gaps

The local Windows Lexicon 1.11.14 Electron package was copied before inspection.
Its ASAR SHA-256 is
`36e4d16d65392b1ece849985d7ef1f66dc87cb662e674ec9db310d2499698c5c`.
The earlier macOS audit used 1.11.15, so version differences must not automatically
be described as platform differences. Private source locations remain in the
ignored audit reports; no proprietary source or identifying library data is
published.

Confirmed implementation gaps include Windows process aliases, VirtualDJ home
and per-drive routing, djay Windows URI identity, Engine cross-drive references,
optional numeric metadata and the Device Library Plus active-loop flag. Native
rekordbox writes are also blocked by an unnecessarily narrow exact-build gate.
The modern USB cue-kind interpretation disagrees between Lexicon and independent
implementations; retain the current encoding until stronger native evidence is
available. A vendor discrepancy alone is not a CueRithm defect.

The local `rekordcrate` checkout provides portable USB PDB and ANLZ parsing and
writing. Use it as an independent fixture reader where useful. It does not
provide encrypted desktop `master.db` integration or Wine prefix management.
Do not add rekordbox XML transport.

## Target configuration

Add a serialized target environment to each configured sync target:

- Native: preserve current defaults, choosing the native application's path
  conventions on Windows or macOS.
- Windows through Wine/Proton: select a prefix, an installation directory when
  required, the library location, and explicit drive mappings.

A mapping binds a Windows drive or UNC share to an absolute host directory.
For example, `C:\` can map to a selected prefix's `drive_c`; `D:\Music` can map
to a mounted music directory. Prefix discovery may propose known locations and
`dosdevices` links, but the selected mapping is visible before it can affect a
sync. Do not silently guess a replacement path for an unresolved reference.

Keep physical host paths separate from serialized destination paths. Read music
and stage files through host paths; write Windows references using the selected
mapping and each destination's actual URI/path convention. Reuse existing native
identities when differently encoded references resolve to the same verified
source. Preserve existing stored references unless relocation is intended.

Reject ambiguous reverse mappings, unresolved drives, relative traversal,
unrepresentable paths and collisions under the destination's path semantics.
Use the longest unambiguous mapped root. Freeze the complete configuration and
resolved roots in the preview and journal. A changed mapping, installation or
library invalidates a prepared preview. Old serialized targets default to Native.

Put the shared types and pure mapping helpers in
`crates/cuerithm-rekordbox/src/target_context.rs`, alongside the existing portable
native discovery primitives. The general DJ adapter crate already depends on
this SDK, so both services can consume one implementation without a new crate
or dependency cycle. Keep it independent of Tauri and analysis.

The serialized shape is `TargetEnvironment::Native` or
`TargetEnvironment::WindowsCompatibility { prefix, installation, mappings }`.
Each mapping has `windows_root` and `host_root`. Add an environment field with a
Native serde default to `SyncTarget` and `RekordboxSyncTarget`. Keep Serato's
existing `source_root` meaning unchanged. A destination's stable identity follows
the library and selected environment; compatible application updates must not
silently create new ownership baselines. The installation fingerprint remains a
separate preview precondition.

Persist named, user-selected target configurations in one additive library
migration after schema 27. Store only paths, environment, mappings and revision;
never credentials or derived keys. Current discovery stays available, and
manually configured targets participate in the same service APIs. Existing
operations retain their captured target; old operations deserialize as Native.

Alternatives considered: a separate platform crate would add workspace and API
plumbing without removing a dependency that already exists; per-adapter prefix
handling would duplicate path and process rules. The shared SDK module is the
smallest change that keeps both publication services consistent.

## Discovery and process guards

Keep manual selection available on every host. Native Windows discovery reads
only the relevant application settings, including VirtualDJ's HomeFolder value.
Wine/Proton discovery uses the chosen prefix and known application directories;
manual selection remains available for moved libraries and custom prefixes.

Inspect native processes and Windows executable names under Wine/Proton. Include
VirtualDJ 8 and djayApp aliases. Do not mistake a Wine launcher or an unrelated
filename for proof that the DJ application is closed. If inspection fails,
publication remains blocked. Obtain a fresh snapshot at the existing publication
and restore boundaries. Installation discovery never starts the DJ application.

## Native rekordbox compatibility

Authorize supported database and asset protocols, not a per-executable whitelist.
Recognize the two currently supported schema/header profiles and their supported
application family (rekordbox 6 or 7). Exact version and executable hash remain diagnostic and
preview-stability evidence, rather than an arbitrary compatibility restriction.

Allow a Windows installation and its data/settings inside a selected Linux
Wine/Proton prefix. Read version metadata portably or report a bounded, explicit
installation hint. Reuse the existing data-only PE/Mach-O key derivation and
selected-option parsing. Do not execute vendor code or retain account/license
fields. Read the configured native locations without changing the application's
library-path settings.

Keep unknown schemas read-only. Validate the analysis/artwork formats required
by each planned edit, preserve unrelated chunks and rows, and retain all current
exclusive leases, complete logical proofs, immutable assets, database-last
publication and guarded recovery/restore behavior. A known schema does not
permit bypassing failed asset, source, process or identity checks.

## Existing adapter contract corrections

- VirtualDJ: honor native Windows home selection; route tracks to their owning
  drive's database; preserve per-drive identities and foreign records. Select
  modern MyLists or the legacy playlist/filter representation from destination
  capability rather than creating an incompatible layout by assumption.
- djay: serialize and match native Windows file URIs correctly. Retain existing
  native identities across equivalent URI forms. Transfer available bitrate and
  track number, preserving native values when local metadata is absent.
- Engine DJ: distinguish valid cross-drive desktop references from target-owned
  audio copies. Use mapped absolute Windows references when relative references
  cannot represent the source. Keep audio-copy consent and source proofs.
- Pioneer USB: keep legacy ANLZ and modern SQL active-loop state consistent using
  each format's own semantics. Transfer normalized bitrate and track number;
  converted PCM copies use their output media facts. Preserve unknown native
  metadata. Keep the unresolved cue-kind discrepancy explicitly unclaimed.
- Serato 4: retain current root, external and network library routing while
  applying the same explicit Windows drive context. Preserve native play history
  when no local producer exists.

Missing or synthesized Null numeric metadata is not a request to clear a native
value. Validate present values and target ranges; do not silently narrow invalid
numbers. Do not invent local play counts or introduce automatic tag writes.

## Desktop flow

In Sync, choose the software, its execution environment, and the library. When
Windows through Wine/Proton is selected, show the prefix, installation and drive
mapping controls with folder pickers. Display unresolved source paths and the
resolved destination before Preview can produce an applicable operation.

Use the same explicit setup for native rekordbox. Keep the existing first-sync
choices and readable/unsupported/conflict states. Show tested versions as
validation evidence; do not label every other compatible patch build read-only.
Saving a target does not enable automatic publication.

## Validation and acceptance

Use authored files and temporary native-shaped libraries only. Cover native
Windows drive paths, Wine/Proton prefixes, UNC mappings, Unicode, escaped URI
characters, case differences, equivalent identities, cross-drive sources,
changed mappings, symlink/root changes, missing executables and running processes.

Prove preview/apply/repeat no-op/recovery/restore preserve foreign records and
unowned assets. Old journals must deserialize and recover with Native defaults.
Exercise the actual service and desktop selection flow, not just helper codecs.
Use independent PDB/ANLZ readback from `rekordcrate` where it adds evidence.

Run focused RED-to-GREEN regressions, appropriate Rust/frontend checks and a
current-head review before marking the PR ready. Source/static findings,
authored fixture results and real Windows/Wine/DJ-application execution remain
separate. Do not claim native application or hardware acceptance that was not
performed. No proprietary program, source music or private library is uploaded
to public CI.

## Delivery boundaries

Complete the existing-target platform work as one reviewable PR. Record any
remaining native-runtime acceptance gaps explicitly. Traktor, legacy Serato and
general DJ-library import require their own transport contracts and scope
selection; do not claim full Lexicon target coverage while they are absent.

The user approved this architectural design and authorized implementation,
independent decisions, review rounds, merge and successful release verification.
Implementation is tracked by the companion plan. Runtime acceptance evidence
must remain distinct from format and authored-fixture validation.
