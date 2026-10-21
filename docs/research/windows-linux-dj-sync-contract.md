# Windows and Linux DJ sync contract audit

Date: 2026-10-21. Scope: the existing CueRithm destinations, Windows Lexicon
1.11.14, and the proposed Linux Wine/Proton target flow. This is a contract audit,
not a claim that every target application was run on Windows or Linux.

## Provenance and evidence levels

The supplied Windows executable was identified as an x86-64 PE Electron program.
Its package identifies Lexicon 1.11.14. Executable SHA-256:
`b467e6f21e6ade632b9f0acfc1ada6e1666b0698e6c8efd7a67ece7a1a57dcab`.
ASAR SHA-256:
`36e4d16d65392b1ece849985d7ef1f66dc87cb662e674ec9db310d2499698c5c`.
Inspection used private copies. The earlier macOS artifact was 1.11.15; differences
between those versions cannot all be attributed to the operating system.

**Seen** means shipped code or current CueRithm source establishes the behavior.
**Fixture verified** means authored data exercises our implementation.
**Unknown** means native application or device execution has not established it.
Private source locations are retained in ignored reports. No proprietary code,
source music, identifying library records or credentials are published.

## Existing destinations

| Destination | Seen contract or gap | Implementation status |
| --- | --- | --- |
| VirtualDJ | Windows includes `virtualdj8.exe`, a configurable home folder, per-drive databases, and modern versus legacy playlist layouts. | Process aliases, native Windows HomeFolder discovery and matched POI payload preservation corrected; Wine setup, routing and layout work remain. |
| Engine DJ | Windows relative paths can require cross-drive absolute references. Copying audio remains a distinct option. | Current cross-drive restriction needs target-context handling. |
| djay | Windows file URI encoding differs from a simple POSIX URI. Bitrate and album track number are represented in native records. | Numeric fields and equivalent Windows URI matching corrected; explicit Windows context remains. |
| Serato 4 | Root, external and network library routing is distinct; native play counts have no current local producer. | Retain existing routing and native play history; explicit drive context remains. |
| Pioneer USB | Legacy ANLZ and modern SQL encode loop activation independently. Both catalogs carry bitrate and track number. | Activation and numeric fields corrected; converted PCM copies use output bitrate. |
| Native rekordbox 6/7 | Windows settings use roaming Pioneer directories and the same desktop data/asset families. | Replace exact-build restriction with supported schema/protocol checks and add manual Windows/Wine configuration. |

## Confirmed corrections

Seen Windows process names include `virtualdj8.exe` and `djayApp.exe`; both were
missing from the closed-application guard. Authored exact-name, case and path
fixtures reproduce the omissions and verify the corrections. Linux process
inspection now reads full NUL-separated executable arguments rather than a
truncated kernel process name, while ignoring unrelated filename arguments and
refusing an inconclusive snapshot.

Seen djay metadata transfers bitrate in kilobits/second and album track number.
CueRithm now writes valid available values, preserves existing values for missing
or synthesized Null metadata, and rejects invalid values. Existing native play
counts are preserved because the normal local snapshot does not supply one.

djay now reuses native track identities across equivalent Windows drive and UNC
file URI encodings. If an existing alias represents the selected source, ordinary
sync preserves its complete native URI array. Reference repair uses the same
comparison and preserves unrelated aliases. Ambiguous selected identities refuse
writes. POSIX filenames containing a literal backslash remain distinct from
directory paths. New Windows records retain the existing slash-form URI output;
the observed vendor encoding alone does not establish a native requirement to
change that output. These checks do not yet map Linux host paths to Wine drives.

Seen Device Library Plus has an independent active-loop flag. CueRithm previously
wrote it as zero while its legacy memory-loop ANLZ flag could be active. The new
regression reads both representations and verifies preserved unrelated metadata.
Modern hot-loop activation is independent of the legacy memory-only PCPT flag.

Seen USB catalogs carry bitrate and track number, and the normal preparation
snapshot already supplies those facts. Both catalogs now receive normalized
available values and preserve native values when the local facts are unknown.
For a converted stereo PCM16 copy, bitrate comes from the output sample rate;
the source's compressed bitrate and original bytes remain unchanged.

Matched VirtualDJ cue/loop nodes now preserve native-only attributes and child
payload while controlled fields are updated. Ambiguous matching identities refuse
publication. Existing unmatched or Slot-only replacement behavior is unchanged;
this patch does not claim a new per-cue ownership policy.

These are fixture-verified corrections. The selected native Windows registry
wrapper also cross-typechecks with Rust 1.99 for x86-64 MSVC, without linking or
executing it. Real target application or physical-player acceptance is a separate
evidence level.

## Linux and Wine/Proton

The target data format must be separate from the host operating system. Choosing
a Windows installation on Linux requires a prefix, a library and explicit drive
mappings. Host paths used for file access must not be confused with Windows paths
and URIs stored in the target database. Equivalent mapped references must reuse
native identities. Mapping and installation changes must invalidate previews and
remain captured for recovery and restore.

The existing general Sync screen already accepts a manual library path, but
that alone does not establish Windows/Wine parity. The native rekordbox screen
also needs manual target configuration. The proposed design is recorded in
`../superpowers/specs/2026-10-21-windows-linux-dj-sync-parity-design.md`.

## Independent checks and limits

The inspected local rekordcrate 0.3.0 checkout provides Rust USB PDB and ANLZ
read/write primitives, plus settings and device-export helpers. It is useful as
an independent reader for authored export fixtures. It has no desktop SQLite
master database or Wine integration module. Adopting it does not by itself add
native desktop sync. Preserve its MPL-2.0 notices if integrated.

Modern USB cue-kind semantics remain unresolved by the inspected artifacts:
Lexicon's modern reader/writer and independent implementations disagree. Its
reader also reconstructs hot positions by order. The current CueRithm encoding
is retained pending stronger native evidence; do not copy a conflicting vendor
implementation just to make static comparisons agree.

Traktor and legacy Serato are separate missing targets. General DJ-library import
is also distinct from the existing export/reconciliation workflows. Their absence
must remain visible in any full Lexicon coverage claim. Rekordbox XML transport
remains intentionally excluded by the project requirements.
