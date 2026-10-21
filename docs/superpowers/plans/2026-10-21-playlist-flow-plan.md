# Playlist flow implementation plan

Status: implemented and integrated on the feature branch. Final validation and remaining platform limitations are recorded in the pull request.

Design: `docs/superpowers/specs/2026-10-21-playlist-flow-design.md`.

1. Write failing tests for canonical ordering, measurement unknowns, missing
   predecessors, harmonic key aliases, keyboard selection and preview, selection
   state, bounded rendering and partial/empty labels.
2. Implement pure flow rows from canonical IDs and loaded track metadata. Reuse
   the harmonic key policy already implemented by cuerithm-smartlists; keep
   measurements and comparisons unknown when inputs are missing or invalid.
3. Implement the scoped, token-based React overview. Render at most 40 rows;
   preserve canonical positions and expose track-ID callbacks to the player.
4. Run the focused test suite and TypeScript checks. Verify an authored fixture
   in the browser, then integrate with canonical membership loaded separately
   from the sortable table. Key the component by playlist ID to reset paging
   when changing playlists.
5. Verify integration with selection/playback and actual playlist changes;
   update user documentation and run the workspace's required gates. Keep
   native application acceptance separate from browser and fixture validation.
