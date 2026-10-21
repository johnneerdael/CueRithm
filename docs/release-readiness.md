# Release scope and outstanding gaps

Updated 2026-10-21. This is the release-stabilization scope requested after the
integration work. Finish and review the existing implementation before adding
features. The broader product plans remain open; merging this release does not
claim complete Lexicon or OneTagger parity.

## Included behavior

| Area | Current delivery |
| --- | --- |
| Incoming | Watched folders and imports run configured analysis, tagging and cleanup, replace cues, then move successful tracks into Tracks. Failed required stages remain actionable in Incoming. |
| Audio | Bundled native decoding and preview for MP3, AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA. |
| Analysis | Built-in OpenKeyScan key detection, local Lexicon-compatible timing and structural cues, saved grid/cue publication, and one compiled acoustic energy rating. Completed results change on explicit reanalysis. |
| Tagging | Reused OneTagger Beatport/Traxsource integrations with persistent provider, matching, field, artwork and format settings. Provider availability is subject to the limits below. |
| Cleanup | Eleven ordered Smart Fixes, genre/artist mappings, protected values, preview/apply and guarded persistent Undo for files and library together. |
| Playlists | Ordinary lists/folders, local smart rules, automatic membership refresh, tools/generation, preview and guarded Undo, with native desktop controls. |
| Maintenance | Native APIs for duplicate review, donor merge/Undo, missing/unused-file scans, reviewed removal/restore, parent cleanup and explicit backup disposal. Dedicated maintenance screens now expose scanning, review, apply, Undo/Restore and recovery; final packaged/native acceptance remains separate. |
| DJ software Sync | Current VirtualDJ 8.5, Engine DJ schema 3, djay Pro, Serato DJ Pro 4 and Pioneer USB adapters and desktop Sync. Native smart export with whole-list fallbacks, timing conversion, owned playlist identity/deletion, typed directory/case-rename journals and recoverable apply/restore acknowledgement are present. Native DJ/player acceptance is separate. |
| Rekordbox foundation | Native encrypted copied-database reads/snapshots, target discovery, runtime key derivation, lossless analysis-file staging, and durable imported observations/baselines. These are backend foundations; the desktop Rekordbox Sync action remains unavailable. The five other DJ targets use the native Sync workspace. |

No collection XML transport is part of the product. Plays, Last Played and
Danceability, the remaining OneTagger workflows, and Mixed In Key segment-energy
or cue imitation are outside the approved scope.

Smart playlists can explicitly include Incoming tracks; ordinary Tracks,
manual playlist tables and automatic playlist generation keep their prepared
track defaults. Release Date rules use complete calendar-date tags under the
library's time zone. Partial dates and timestamp strings remain unknown rather
than acquiring an invented calendar day. See the
[smart-rule adapter policies](research/smart-playlist-parity.md).

The library upgrades to schema 15. Migration preserves existing tracks, jobs,
cue ownership and legacy sync links; older applications refuse this newer
schema. A database downgrade is not implemented. Completed analysis remains
unchanged until explicit reanalysis.

## Home-DJ workflow additions

Current source adds manual correction/template previews with durable Undo, compatible-track filtering, an ordered playlist flow view, text/CSV matching with explicit recording choices and atomic playlist creation, and same-installation library backup/guarded startup restoration. These additions preserve raw DSP results and source music. Local loops retain starts/ends and active-loop metadata, but local preview seeks rather than looping and current sync projection does not transfer the active-loop flag. Library restoration is separate from reversing external file-tag or DJ-software changes.

Direct rekordbox reconciliation reviews and encrypted copied-row staging are internal progress only. No production schema/application profile is write-accepted; the desktop target stays unavailable.

## Outstanding work after this release

| Priority | Gap | Evidence needed to close it |
| --- | --- | --- |
| Next delivery | Direct rekordbox 6/7 sync: field reconciliation/conflicts, verified schema write profiles, native database and asset publication, IDs/update counters, recovery/restore, playlists/MyTags/native smart rules, and desktop preflight/preview/apply/history. | Closed-target/reopen acceptance in actual rekordbox 6 and 7, including metadata, artwork, ordered playlists, cues/loops/grid playback, no-op repeat sync, later edits and interrupted-write recovery. SDK fixtures or a successful SQL commit are insufficient. |
| Native acceptance | Dedicated maintenance screens and confirmations are implemented with React workflow tests and authored screenshots. | Final packaged review/apply/Undo/recovery, removal/disposal confirmations and larger copied-library acceptance. |
| Analysis parity | Manual cue/loop/constant-grid corrections, named templates and custom anchors are implemented. Automatic preparation still uses factory publication rows; equal tempo-bound options remain unavailable. | Equal-input comparisons of remaining automatic settings paths, final packaged manual editing/Undo and saved target playback. |
| Provider acceptance | Direct Traxsource requests returned HTTP 403. A read-only native probe with the configured Trawl transport fetched and matched a live track including release metadata, and correctly returned no-match for an empty search. No end-to-end file write or packaged-app acceptance is established. Beatport has a native successful probe, not universal catalog coverage. | A real native Traxsource match/write and representative provider coverage checks without bypassing failed required stages. |
| Energy acceptance | The local formula matches the recorded usable synthetic helper cases. Real-music scalar-energy agreement and exact WebAudio resampling equivalence remain unmeasured. | A frozen real-music comparison with identical inputs/configuration and separately reported missing/unrated results. |
| Analysis acceptance | Development-set timing/cue agreement does not prove unseen-track musical correctness or all cue configurations. | Independent held-out evaluation and broader native GUI comparisons, with raw detection and saved publication evaluated separately. |
| Platform acceptance | Hosted builds/tests and generated fixtures do not establish every native interaction on all desktops. No Intel macOS release artifact is configured. | Final Windows/Linux native playback, import/settings/restart/recovery acceptance and representative common-format checks; a separate Intel build if added later. |
| Scale acceptance | Full user-library maintenance, provider throughput, CPU/memory use and retained recovery-storage growth need larger-library measurement. Workers use 75% of available logical CPUs; that policy is not a proven optimum or utilization cap. | Reproducible large-library benchmarks and operation/recovery checks using copies. |
| Optional future work | All-In-One structural/deep analysis remains research, not an integrated optional desktop add-on. | Native model/frontend/separation/postprocessing integration and a separately accepted optional workflow. |

Preparation, manual cleanup and DJ Sync retain recovery backups. Sync history
exposes guarded restore and interrupted-operation recovery; Serato sync backups
include affected source audio, while Pioneer conversion protects the affected USB
copies. Explicit disposal currently covers reviewed unused-file-removal backups; general
preparation/cleanup backup retention management remains outstanding. Disk-space
claims must distinguish logical backup deletion from filesystem snapshot storage.

The separate brand-kit UI worktree remains user-owned. This integration does not
replace that work or claim to include its unmerged branding.

## Acceptance evidence and deliberate differences

- [Automatic tagging](automatic-tagging.md) distinguishes deterministic provider
  fixtures from live-service results and documents the supported settings.
- [Cleanup](cleanup.md) and [behavior evidence](research/cleanup-behavior.md)
  describe preservation policies and source-helper differences.
- [Timing/cue comparisons](research/lexicon-beatgrid.md) record exact agreement
  on 282 development groups under one stated worker configuration. They do not
  establish a universal accuracy percentage.
- [Saved grid/cue publication](research/lexicon-publication.md) records the
  separately tested rounding/template boundary and the reported-track native
  acceptance check. Raw markers and published cues are different evidence layers.
- [Local energy](research/local-track-energy.md) documents the formula,
  synthetic checks and intentional unrated silence/short-input behavior.
- [Smart rules](research/smart-playlist-parity.md) and
  [playlist tools](research/playlist-tools-and-generation.md) document finite
  helper comparisons and explicit rule/rounding policy differences.
- [Maintenance API](development/maintenance-api.md) documents publication,
  recovery, disposal and the remaining UI handoff.
- [Direct Rekordbox contract](research/rekordbox-direct-contract.md) records the
  copied-data boundary. Initial per-field observations remain unacknowledged;
  storing an import baseline is not proof that either library was synchronized.

## Merge and publication gates

The release PR must include current-head local verification, the outstanding
limits above, resolved review findings and a completed Codex review of its
latest commit. Require the repository PR gate and full cross-platform checks
before squash-merging. Do not use an older approval or an earlier green run as
approval of a changed head.

After merge, [the release workflow](development/pr-workflow.md) builds and
publishes automatically only after both main-commit build workflows succeed.
Check the resulting signing/notarization, publishing job and downloadable assets
before reporting a release as available. A merged PR or unsigned local app is
not publication evidence. No manual version bump or release tag is part of this
stabilization change.

## Native DJ sync

`crates/cuerithm-dj-sync` adds independent current-format adapters for VirtualDJ,
Engine DJ, djay Pro, Serato DJ Pro 4 and Pioneer USB, with a desktop Sync workspace.
Implemented behavior includes measured export timing, supported native smart-rule
compilation with exact whole-list fallbacks, durable playlist ownership and owned
removal propagation, batch staging, typed file/directory/case-rename journals and
recoverable apply/restore acknowledgement. Extra prepared metadata and explicit
calendar/key-mode context reach the compiler. Older Lexicon formats are excluded;
existing direct rekordbox foundations remain separate and unavailable as a desktop
sync target.

Targeted final artifact review has resolved ANLZ path collisions, hidden catalog
discovery, dotted VirtualDJ names, Serato explicit null clears, djay filename indexes
and raw Engine BPM precision. Current-head validation and integration evidence are
recorded in [PR 12](https://github.com/johnneerdael/CueRithm/pull/12).
See [format evidence](research/dj-software-formats.md),
[timing](research/dj-sync-timing.md), [ownership](research/dj-sync-playlist-ownership.md)
and [smart-rule policies](research/dj-sync-smart-rules.md). Native application/device
acceptance and release publication are separate evidence layers; this implementation is
not a declaration of shipped cross-application parity.
