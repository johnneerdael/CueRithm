# Playlist flow overview

Approved roadmap delivery 5, 2026-10-21.

Extend the existing CueRithm tokens and dense library layout with an ordered,
compact flow list. Show playlist position, track/artist, a single whole-track
energy bar, BPM and its change from the preceding track, and harmonic key
comparison. Preserve original key text. Camelot, Open Key and standard musical
aliases follow the existing smart-list key rules. State unknown measurements
explicitly; never substitute zero energy or classify unknown keys as a mismatch.

Supply canonical playlist membership separately from loaded track metadata so
table sorting cannot alter flow order. Compare only consecutive canonical slots
whose metadata is loaded. Label partial data and missing predecessors. Bound the
rendered list to 40 rows with previous/next controls; retain global positions.

Use ordinary keyboard-accessible buttons to select a track or preview through
the existing player. Reflect selection without starting playback. Use existing
focus/hover styling and subtle color transitions; introduce no decorative motion.
Describe key comparison as a preparation aid rather than a musical verdict.
Do not infer transition timing, per-segment energy, or a recommended ordering.

Implementation: add a pure ordering/measurement/key helper, component and scoped
styles. First write behavior tests for canonical ordering, unknown measurements,
gaps, notation aliases, keyboard selection/preview, selection and bounded pages;
observe failure, implement, then run targeted tests and TypeScript validation.
Integrate with the playlist workspace and verify the rendered screen separately.
