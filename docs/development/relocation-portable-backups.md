# Relocation and portable backups: implementation verification

This records the implemented boundary and measured acceptance scope. It does not
establish real-music DSP accuracy or compatibility with untested vendor schemas.
The source contracts are in `docs/research/relocation-backup-handover/`.

## Storage and lifecycle

Migration 29 separates an immutable portable source locator from an optional
native file binding. Accepted musical revisions retain signed cue times, loops,
memory/slot metadata and multiple tempo anchors independently of optional DSP.
Native publication history remains available after a logical restore.

Relocation changes database bindings under an exclusive operation lease. It
probes selected media and verifies file identity/content before an atomic apply;
reviewed survivor groups retain a complete survivor and support guarded undo.
Traversal uses a disk-backed directory queue, rejects links/mount escapes, and
supports continuation with exact unsearched counts and bounded detailed reports.

Portable and Lexicon imports stream into a private SQLite spool. ZIP preflight,
strict duplicate-key/depth/allocation-limited JSON parsing, checksums and domain
reference validation run before activation. Logical archives contain metadata,
not music or runtime credentials. Restored watch folders, bindings and automation
remain inactive until explicitly configured.

Restore seals a separate generation and retains the current one. Activation uses
the existing exit/draft guard and a controlled application restart, closing all
Host and SQLite handles before switching. Invalid pointers/journal interruption
recover a validated retained generation. Same-installation recovery snapshots
remain a separate supported format and include unresolved logical records.

## Measured workloads

Measurements below ran on macOS 15.8 (24H23), Apple M4 Pro, 48 GiB RAM,
with synthetic data. Peak RSS includes the test
process. Private execution logs remain outside the published tree. Scale tests
are ignored by the routine suite and were explicitly executed.

| Workload | Time | Peak RSS | Longest progress callback gap |
|---|---:|---:|---:|
| Native export/restore: 100,000 tracks with offline native bindings, 800,000 cues, 10,000 playlists, 100,000 memberships | 209.085 s | 127,156,224 bytes | 0.156 s |
| Lexicon streaming import: 100,000 minimal tracks with accepted musical records, no cues/playlists | 67.666 s | 47,038,464 bytes | 0.414 s |
| Relocation: 1,000,000 flat candidates plus a target beyond the cap | 97.334 s initial scan; 67.535 s continuation | 23,363,584 bytes | 0.648 s |

The native archive measured 9,389,490 bytes and preserved 1,000 archived tracks
without creating jobs and grouped all native sources under one mapping root.
The measured native runner SHA-256 was
`4e3ad9b35d65b7b901f0b72736ba42d03520acb147ddc2ee8799b6bae4a78669`.
The Lexicon source ZIP measured 1,241,008 bytes, SHA-256
`c09018ddfa66737a17ef958ec9e8b154eb6e94b7b6f8eb712e64514b1758f51b`. Million
candidate cancellation completed in 0.253 seconds; setup/deletion is excluded
from the scan timings. Subsequent Linux mount-ID and bounded-report changes have
focused tests and Linux cross-compilation evidence; the million-file measurement
itself ran on macOS before those additive changes.

A separate retained-history workload roundtripped sixteen 4 MiB extension records
with a 16 MiB page setting, forcing at least sixteen snapshot pages. Peak RSS was
451,559,424 bytes including fixture creation. Nonselected native cue revisions
and safe provider preferences were also checked for preservation.

## Automated coverage and boundaries

The backend suites cover transactional rollback, restart recovery, operation
races, unresolved locators, source byte/UTF-16 preservation, verbatim Windows
roots, parent-swap binding races, malformed archives, lossless history, survivor
undo and resumable search. The frontend suite covers preview/confirmation,
progress/cancellation, stable paged review selection and restored musical data.

`portable_cross_platform` constructs macOS, Windows and Linux source locators and
roundtrips every required media fixture through the destination runtime and PCM
cache. Running that test in the existing three-platform desktop workflow covers
nine source/destination combinations. Synthetic Windows verbatim drive/UNC tests
run on every host; an actual `std::fs::canonicalize` fixture runs only on Windows.
These fixtures do not replace an interactive three-machine migration exercise.

The referenced private Lexicon ZIP was unavailable in the provided handover.
The implemented claim is bounded to `lexicon-main-db-v26` and authored synthetic
fixtures; no real private sample or broader Lexicon version acceptance is claimed.
UI screenshots were inspected using a controlled synthetic desktop bridge on
macOS. Interactive Windows/Linux GUI inspection remains unverified.

No DSP calculations, music files, installed application, release versions or
external DJ libraries were changed by these validation runs.
