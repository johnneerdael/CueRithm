# Check what works with your DJ software

<p class="lead">Sync can write to seven destinations, but not every destination has been tried in the DJ program itself. This page lists what was actually checked, on which system and with which version, so you know how far to trust a first sync. Reading it takes about three minutes.</p>

!!! tip "New to Sync?"
    Start with [Send your library to DJ software](../sync/index.md) and the tutorial [Sync to rekordbox 6 or 7](../sync/rekordbox-direct.md). This page is the evidence behind them.

## How to read the evidence { #how-to-read }

Three kinds of check appear below, from weakest to strongest:

| Check | What it shows |
|---|---|
| **File formats** | CueRithm's output matches the destination's database and file layouts, using authored test libraries and reference parsers. The DJ program itself was not opened. |
| **Application checks** | The real DJ program opened a library that CueRithm wrote and showed or played the result. |
| **Physical player** | A CDJ, XDJ or similar player loaded the result. No destination has this check yet. |

A check covers only the system, version and test set named next to it. It is not a promise about other versions, other systems or your own library, and agreement with a program is measured, not guaranteed. Take a copy of a DJ library before your first sync, preview before you apply, and use **Restore backup** if the result is wrong ([Preview, apply and restore a sync](../sync/preview-apply-restore.md)).

## At a glance { #at-a-glance }

| Destination | Libraries CueRithm can write | Application checks completed |
|---|---|---|
| **rekordbox 6 / 7** | macOS, Windows, and Windows installations under Wine / Proton on Linux, when the database and asset formats pass CueRithm's compatibility checks | rekordbox 6.8.7.0303 and 7.2.19.0342 on macOS |
| **Mixxx** | Mixxx's own library on macOS, Windows and Linux | Mixxx 2.5.6 on macOS; Debian's Mixxx 2.5.0 on Linux, in Docker |
| **VirtualDJ** | VirtualDJ 8.5 libraries | File formats only |
| **Engine DJ** | Engine DJ schema 3 libraries and drives | File formats only |
| **djay Pro** | djay Media Library folders | File formats only |
| **Serato DJ Pro 4** | Serato Library folders | File formats only |
| **Pioneer USB** | A mounted USB drive | File formats only; no physical player |

Windows library paths, and Wine or Proton setups on Linux with drive mappings and saved configurations, are supported for the destinations whose programs run on Windows ([Use a Windows DJ library on Linux](../sync/windows-libraries-on-linux.md)). Windows and Wine playback inside the DJ programs has not been checked. Mixxx sync arrived in CueRithm 0.1.14, so an older build does not list it under **Sync to**.

## rekordbox 6 and 7 { #rekordbox }

CueRithm decides whether to write from the library's database and asset formats, not from a list of build numbers. A compatible 6 or 7 patch build can be synced after those checks pass. A format CueRithm does not recognise, for example a newer release that changed its database, stays **Read only**: you can preview but not write.

| Checked | Result |
|---|---|
| rekordbox **6.8.7.0303** and **7.2.19.0342** on macOS | Both programs reopened the synced libraries and displayed metadata, artwork, grids, cues and loops. Created FLAC tracks played and looped. Native readback, repeat-sync, restore and conflict checks also passed. |
| Other 6 and 7 builds, and Windows | Eligible when the format checks pass. Not opened in rekordbox by the checks above, so make your own backup first. |
| Windows rekordbox through Wine / Proton | Eligible after a manual setup. Playback has not been checked. |

CueRithm writes to rekordbox's library directly. It does not read or write rekordbox collection XML, which is intentionally excluded. If rekordbox or its Agent is running, CueRithm refuses to sync ([My sync does not show up in rekordbox](troubleshooting/rekordbox-sync-problems.md)).

## Mixxx { #mixxx }

| Checked | Result |
|---|---|
| **Mixxx 2.5.6 on macOS** | 240 checks passed across fresh, existing, replaced, newly created and additional-format libraries. Mixxx loaded the libraries, grids and keys and preserved the synced rows. |
| **Debian Mixxx 2.5.0 on Linux, in Docker** | 157 checks passed for MP3, WAV, FLAC, AAC, ALAC, AIFF, Ogg Vorbis and Opus. The real Mixxx loaded the library, grids and keys and preserved the synced rows. MP3 timing was measured through libmad. |
| **Mixxx `main` (schema 40) on Linux, built with FAAD, in Docker** | 157 checks passed, including AAC from Apple's encoder moved to match FAAD and Mixxx's own upgrade of a CueRithm-created library. |
| **Mixxx 2.5.6 on Windows Server 2019** | 156 checks passed for AAC (moved to match Media Foundation), ALAC, AIFF, Ogg Vorbis, Opus, WAV, FLAC and MP3 (timing measured through libmad), and a library CueRithm created, and Mixxx's analyzer left locked grids alone. |
| **Mixxx 2.5.6 on Windows Server 2022 and 2025** | 156 checks passed on each, with CueRithm's sync built by MSVC as the Windows app is, and the same results as on Server 2019. |

The Linux run used the sync harness on macOS with Linux timing, then opened the result in the real Linux Mixxx. It shows that the library works in that Mixxx. It did not exercise CueRithm's own screens, discovery or process checks on Linux. Not yet checked: Windows 10 and 11 themselves (only Windows Server 2019, 2022 and 2025 were run), Mixxx 2.4 and release builds newer than 2.5, and Linux packages other than Debian's. The tested macOS, Debian and Windows builds cannot play WMA. [Sync to Mixxx](../sync/mixxx.md) gives the exact scope.

## The other destinations { #other-destinations }

VirtualDJ, Engine DJ, djay Pro, Serato DJ Pro 4 and Pioneer USB are implemented against the current file layouts of those programs and checked with authored fixtures and reference parsers. They were not opened in the DJ programs, and no physical CDJ or XDJ loaded a Pioneer USB stick. Try a small playlist first. The guides are [VirtualDJ](../sync/virtualdj.md), [Engine DJ](../sync/engine-dj.md), [djay Pro](../sync/djay-pro.md), [Serato DJ Pro](../sync/serato-dj-pro.md) and [Pioneer USB](../sync/pioneer-usb.md).

## CueRithm on each system { #systems }

| System | Status |
|---|---|
| macOS, Apple Silicon | Signed and notarized. No Intel build. |
| Windows, x86-64 | Unsigned portable app. Native playback and restart acceptance are not complete. |
| Linux, x86-64 | Unsigned `.deb`. Native playback and restart acceptance are not complete. |

[What CueRithm does not do yet](not-available-yet.md) lists these limits with the rest.

## Not offered { #not-offered }

- **Traktor** and **legacy Serato** (before Serato DJ Pro 4) are separate destinations that are not available.
- **Standalone playlist import and export** (M3U or XML) is not available. Playlist export happens inside Sync.
- **Importing an existing DJ library into CueRithm** is in development and not available. Sync goes one way, from CueRithm to your DJ software ([In development](not-available-yet.md#in-development)).

## Related

- [Send your library to DJ software](../sync/index.md)
- [Sync to rekordbox 6 or 7](../sync/rekordbox-direct.md)
- [Sync to Mixxx](../sync/mixxx.md)
- [What CueRithm does not do yet](not-available-yet.md)

<!-- verified: legacy about/roadmap.md (updated: Available now, application-checks table, Not in the app, Known limitations); README.md "Compatibility and validation" and DJ sync row; crates/cuerithm-rekordbox/src/write_profile.rs (writable = recognised 6/7 schema + matching application family, no build list); apps/desktop/src/views/Sync.tsx (seven destinations, Mixxx option); docs/release-readiness.md (VirtualDJ 8.5, Engine DJ schema 3, djay Pro, Serato DJ Pro 4, Pioneer USB; native app/device acceptance separate); docs/research/rekordbox-direct-contract.md (accepted native profiles; eligibility is not a claim of acceptance); legacy guides/sync-mixxx.md "What has been tested"; get-started platforms and release workflows for signing; sync/index.md and sync/rekordbox-direct.md (Sync writer) read for consistency. Mixxx shipped in v0.1.14 (gh release list shows 0.1.14 latest) -->
