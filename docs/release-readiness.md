# Release scope and outstanding gaps

Updated 2026-10-19; gap rows revised 2026-10-20. This is the release-stabilization scope requested after the
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
| Rekordbox foundation | Native encrypted copied-database reads/snapshots, target discovery, runtime key derivation, lossless analysis-file staging, and durable imported observations/baselines. A native writer for track metadata, cues, playlists/folders and MyTags on copied databases, with conflict guards and a verified publish-with-backup step, is verified on generated databases and on a copy of a real 7.2.19 library ([contract](research/rekordbox-direct-contract.md)). The desktop **rekordbox** screen drives it: preflight, linking tracks, a field-by-field preview, an explicit apply that publishes with a verified backup, and history; the whole flow is verified on a copy of a real library. It has not been run against the real rekordbox application, only writes to a schema allowlisted for writing (7.2.19.0342), and mirrors playlists, folders and smart lists by name path (smart lists as native rekordbox rules when every rule maps exactly, otherwise as ordinary playlists of their current members), touching only playlists CueRithm itself wrote, and writes MyTag changes as a merge into rekordbox's own tags. It can also add CueRithm tracks that rekordbox lacks (only when asked, and only for formats rekordbox plays): each is written with its tags, grid and cues, a placeholder analysis file that rekordbox replaces when it analyses the track, and its place in mirrored playlists; verified on a copy of a real library. rekordbox 6 is not supported until an installed 6 can be inspected. |

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
| Scale acceptance of maintenance | The Maintenance screens are implemented and covered by component tests against a faked native bridge. Result lists render in pages of 200. The library layer under them was run on generated libraries of 10,000 and 50,000 tracks and on 383 real FLAC files (scan, review, merge preview/apply, undo; see [scale measurements](research/large-library-scale-2026-10-20.md)); on real files a review action takes 0.34 s; the packaged app's screens have not been walked through on a large library. | Review/apply/Undo/recovery through the packaged screens on a copy of a large library, including explicit removal and backup-disposal confirmations. |
| Analysis parity | The cue template editor, custom-anchor extraction and tempo-range presets are implemented in Settings → Cue generation and the preparation path (see [publication notes](research/lexicon-publication.md)). Anchor fallback, fixed overwrite and detector-owned fields differ from Lexicon on purpose. On the 282 tuning-split songs of the development folder at Lexicon's 78–155 preset, BPM and first beat match Lexicon on every song and the published cue set within 5 ms on 99.6% (281/282); the one difference is a Last beat one grid step earlier in Lexicon on a song whose length is exactly a whole number of beats, not yet explained. The seven first-beat differences found earlier came from Chromium's uneven 16-bit to float scaling, which the timing and cue input now reproduces ([comparison](research/lexicon-mik-equal-input-comparison.md)). All eight tempo presets were checked against Lexicon's shipped worker on 20 songs (160/160 runs agree); the app's own results at other presets and custom templates are unmeasured. | Equal-input comparison against the Lexicon app at each tempo preset (not only its worker) and for custom-anchor/template configurations on real tracks, and end-to-end saved results. |
| Provider acceptance | Live Traxsource success has not been established; the validation machine returned HTTP 403, and on 2026-10-20 every Traxsource endpoint tried, including the `/search/tracks` request the vendored OneTagger provider makes, answered 403 with `cf-mitigated: challenge` (a Cloudflare managed challenge, "Just a moment…"). A browser passes it by running Cloudflare's JavaScript, which is why the site loads normally in a browser on the same machine; a plain HTTP client such as the provider's `reqwest` client cannot. Whether and how the app should fetch Traxsource through a real browser context, or obtain access from Traxsource, is an open product decision; the client does not attempt to defeat the challenge. Beatport has a native successful probe, not universal catalog coverage. | A real native Traxsource match/write and representative provider coverage checks without bypassing failed required stages. |
| Energy acceptance | The local formula matches the recorded usable synthetic helper cases. On the 282 tuning-split songs, the rating equals the current Mixed In Key tag on 45.4% (within one level on 91.5%) and Lexicon's own rating on 19.1%; the Mixed In Key energy tags in the development folder changed on 54 songs after an earlier measurement of 64.5%, which was against the old tags. Lexicon's input conversion differs from CueRithm's, and MIK's rating comes from server-supplied segment levels, so a local rating can only imitate it. A window classifier with an 80th-percentile rule matches the server value on 74.8% out of fold (99.6% within one level; 79.8% and 99.3% against the current tags, partly in-sample), but it is not in the app; our energy request windows match a fresh Mixed In Key capture bit for bit on all 272 tuning-split songs ([comparison](research/lexicon-mik-equal-input-comparison.md)). | A frozen real-music comparison with identical inputs/configuration and separately reported missing/unrated results. |
| Analysis acceptance | Development-set timing/cue agreement does not prove unseen-track musical correctness or all cue configurations. | Independent held-out evaluation and broader native GUI comparisons, with raw detection and saved publication evaluated separately. |
| Platform acceptance | Hosted builds/tests and generated fixtures do not establish every native interaction on all desktops. `tools/release/linux-container-check.sh` ran on Linux (aarch64, Debian 13, Rust 1.90) in a container: the engine and library tests (111 test binaries, 924 tests) pass, the sealed ffmpeg runtime builds, and the desktop app starts under a virtual display and stays up; no audio was played and no window was driven. Nothing was run on Windows. No Intel macOS release artifact is configured. | Final Windows/Linux native playback, import/settings/restart/recovery acceptance and representative common-format checks; a separate Intel build if added later. |
| Scale acceptance | A reproducible benchmark exists (`scale_benchmark.rs`): at 50,000 generated tracks the duplicate scan takes 286 s cold, review actions take under a second, and peak memory is about 830 MB; on 383 real FLAC files (15 GB) the cold scan takes 41 s and each review action 0.34 s. The scan was serial and re-hashed every file on every review action; both now use the worker pool and reviews prove only their duplicate groups' files (a review action on real files went from 29 s to 0.34 s). Not measured: a real user library, the other maintenance scans, provider throughput and retained recovery-storage growth. The saved review is one row read and written on every edit (82 MB at 50,000). Workers use 75% of available logical CPUs; that policy is not a proven optimum or utilization cap. |
| Optional future work | All-In-One structural/deep analysis remains research, not an integrated optional desktop add-on. | Native model/frontend/separation/postprocessing integration and a separately accepted optional workflow. |

Preparation and manual cleanup retain original recovery backups until the user disposes of
them from Maintenance → Retained backups. Disposal is explicit and previewed, refuses changed
files, and offers originals that still back a cleanup Undo only when the user chooses to give
that Undo up ([details](development/maintenance-api.md)). Disk-space claims must distinguish
logical backup deletion from filesystem snapshot storage.

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
