# Manual musical correction

Approved roadmap delivery 3, 2026-10-21.

Extend the existing charcoal and teal player with a collapsible correction editor. Show constant-grid BPM and first beat, followed by editable cue/saved-loop rows and reusable named templates. Keep the player waveform and cue strip on the saved publication. Preview candidate rows with seek buttons; require an explicit Save preview action. Use existing colors, buttons, focus states and restrained hover transitions.

The detector and raw measured analysis remain unchanged. Save only publication timing, cues and the track BPM summary. Append a cue revision using the existing publication schema so sync and playback agree. No music-file writes occur. First beat is nonnegative because player and sync currently require it. Saved active-loop flags are metadata for compatible targets; the local player only seeks and does not promise sample-accurate looping.

Persist previews, before/after publication, manual slot ownership and history in SQLite. Guard preview/apply with source SHA proof, exact library revision, monotonic track entity epoch and latest cue revision ID; recheck preparation and source under the publication gate and write transaction. Undo requires the exact owned cue revision and post-apply epoch, survives restart, and appends a restoration revision. Reject A/B/A changes through monotonic epochs.

Named templates retain the core CueTemplate rows and settings, validate through core, and preview through core publish using raw measured structural markers. Custom anchors come from explicitly named existing cues; missing anchors produce a clear error. Default overwrite is off and manual positions, edits, deletions and grid corrections remain protected unless preservation is explicitly disabled. Batch templates use explicit track guards and commit atomically after preview. Do not retune or rerun DSP.

Bind manual slot protection to its owning cue revision. Treat it as dormant after a generated or donor publication replaces that revision; restoring the original revision through donor Undo restores its protection. Rebind surviving protection when saving a template or restoring a manual correction.
