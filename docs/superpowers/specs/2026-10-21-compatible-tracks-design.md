# Find compatible tracks

Approved roadmap delivery 4. Select one prepared, active source track, preview compatible library tracks, audition/select results and save an ordinary playlist. This is a preparation aid; matching metadata does not establish that tracks sound good together.

## Rules and assumptions

Use current published library key, BPM and whole-track energy. Default key compatibility on with the saved library harmonic/fuzzy mode, BPM tolerance ±4 absolute BPM inclusive. Optional controls: key off/mode override, BPM off, half/double comparison off by default, energy tolerance ±1 (enabled explicitly, inclusive), exact genre value (case-insensitive), and custom tag IDs (all required). Energy stays whole-track; no DSP changes or inferred confidence.

Unknown or invalid key/BPM/energy cannot satisfy its enabled check. Invalid required source measurements fail with an actionable message suggesting preparation/correction or disabling that check. Source and candidates must be prepared and nonarchived. Unknown candidates are counted once and excluded when any enabled measurement is unavailable. Missing disabled measurements remain unknown visibly.

Reuse the existing harmonic/fuzzy helper and canonical key parser. Rank exact canonical key before other compatible keys when enabled, then absolute matched BPM difference when enabled, optional energy difference, case-folded artist/title and track ID. For half/double matching, compare candidate BPM ×1, ×0.5, ×2; choose smallest absolute delta with ×1 winning ties. Reasons list actual enabled conditions and the exact applied BPM multiplier. Restrictions are ANDed. Results cover the full active collection, without a hidden first-page limit.

## Interface and persistence

Extend CueRithm's existing charcoal surfaces, teal controls, compact row spacing and persistent player. Order: searchable source picker, compact match controls, preview action, ranked result rows with checkboxes/measurements/reasons/audition, ordinary-playlist name/save. Show loading, errors, empty and unknown states. Editing options invalidates prior results and selection; ignore late requests. Invalidate on library changes. Saving uses a native prepared-playlist transaction that checks the captured library revision, current playlist revision and every selected track’s readiness before creating the playlist and adding members preserving match order, so failure cannot leave a partial playlist and sidebar Undo can revert the result.

## Boundaries

Rust owns validation/matching and reads one consistent SQLite transaction. Tauri forwards typed requests. React owns controls and transient preview/selection only. No schema change, audio-file writes, raw analysis change or automatic playlists. Saved playlists are ordinary snapshots and do not update automatically.

User instructions: `docs/user-guide/guides/compatible-tracks.md`.

Verification: authored-fixture standalone React view rendered in Chrome at 1280×1240; screenshot `.analysis/compatible-tracks-2026-10-21.png`. No native playback or real-music transition acceptance is claimed.

The compatible and pasted-tracklist save APIs share `Library::save_prepared_playlist`; both validate and commit under one SQLite immediate transaction. Client-side prechecks are only early feedback and cannot authorize a stale save. App integration can provide `onPreview` to load the persistent player once, including detail and waveform.
