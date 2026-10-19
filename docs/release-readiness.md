# Release scope and outstanding gaps

Updated 2026-10-19. This is the release-stabilization scope requested after the
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
| Maintenance | Desktop Maintenance screens (sidebar → Maintenance) for duplicate review, donor merge/Undo and recovery, missing/broken-file scans, reviewed unused-file removal/Restore with parent cleanup, and explicit backup disposal, over the native APIs. Confirmations gate removal, merge and irreversible disposal. |
| Rekordbox foundation | Native encrypted copied-database reads/snapshots, target discovery, runtime key derivation, lossless analysis-file staging, and durable imported observations/baselines. These are backend foundations; the desktop Sync action remains disabled. |

No collection XML transport is part of the product. Plays, Last Played and
Danceability, the remaining OneTagger workflows, and Mixed In Key segment-energy
or cue imitation are outside the approved scope.

Smart playlists can explicitly include Incoming tracks; ordinary Tracks,
manual playlist tables and automatic playlist generation keep their prepared
track defaults. Release Date rules use complete calendar-date tags under the
library's time zone. Partial dates and timestamp strings remain unknown rather
than acquiring an invented calendar day. See the
[smart-rule adapter policies](research/smart-playlist-parity.md).

The library upgrades to schema 11. Migration preserves existing tracks, jobs,
cue ownership and legacy sync links; older applications refuse this newer
schema. A database downgrade is not implemented. Completed analysis remains
unchanged until explicit reanalysis.

## Outstanding work after this release

| Priority | Gap | Evidence needed to close it |
| --- | --- | --- |
| Next delivery | Direct rekordbox 6/7 sync: field reconciliation/conflicts, verified schema write profiles, native database and asset publication, IDs/update counters, recovery/restore, playlists/MyTags/native smart rules, and desktop preflight/preview/apply/history. | Closed-target/reopen acceptance in actual rekordbox 6 and 7, including metadata, artwork, ordered playlists, cues/loops/grid playback, no-op repeat sync, later edits and interrupted-write recovery. SDK fixtures or a successful SQL commit are insufficient. |
| Scale acceptance of maintenance | The Maintenance screens are implemented and covered by component tests against a faked native bridge. Result lists render in pages of 200, but nothing here measured them on a large real library, and the packaged app's screens have not been walked through live. | Review/apply/Undo/recovery through the packaged screens on a copy of a large library, including explicit removal and backup-disposal confirmations. |
| Analysis parity | The cue template editor, custom-anchor extraction and tempo-range presets are implemented in Settings → Cue generation and the preparation path (see [publication notes](research/lexicon-publication.md)). Anchor fallback, fixed overwrite and detector-owned fields differ from Lexicon on purpose. | Equal-input comparison against Lexicon at each tempo preset and for custom-anchor/template configurations on real tracks, and end-to-end saved results. |
| Provider acceptance | Live Traxsource success has not been established; the validation machine returned HTTP 403. Beatport has a native successful probe, not universal catalog coverage. | A real native Traxsource match/write and representative provider coverage checks without bypassing failed required stages. |
| Energy acceptance | The local formula matches the recorded usable synthetic helper cases. Real-music scalar-energy agreement and exact WebAudio resampling equivalence remain unmeasured. | A frozen real-music comparison with identical inputs/configuration and separately reported missing/unrated results. |
| Analysis acceptance | Development-set timing/cue agreement does not prove unseen-track musical correctness or all cue configurations. | Independent held-out evaluation and broader native GUI comparisons, with raw detection and saved publication evaluated separately. |
| Platform acceptance | Hosted builds/tests and generated fixtures do not establish every native interaction on all desktops. No Intel macOS release artifact is configured. | Final Windows/Linux native playback, import/settings/restart/recovery acceptance and representative common-format checks; a separate Intel build if added later. |
| Scale acceptance | Full user-library maintenance, provider throughput, CPU/memory use and retained recovery-storage growth need larger-library measurement. Workers use 75% of available logical CPUs; that policy is not a proven optimum or utilization cap. | Reproducible large-library benchmarks and operation/recovery checks using copies. |
| Optional future work | All-In-One structural/deep analysis remains research, not an integrated optional desktop add-on. | Native model/frontend/separation/postprocessing integration and a separately accepted optional workflow. |

Preparation and manual cleanup retain original recovery backups. Explicit
disposal currently covers reviewed unused-file-removal backups; general
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
