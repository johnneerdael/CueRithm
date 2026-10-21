# Tracklist matcher implementation plan

Status: implemented and integrated on the feature branch. Final workspace validation is recorded in the pull request; no release or native-player acceptance is implied.

Spec: `docs/superpowers/specs/2026-10-21-tracklist-matcher-design.md`.

- [x] Add a pure bounded parser/matcher in
  `crates/cuerithm-library/src/tracklist.rs` and behavioral tests in
  `crates/cuerithm-library/tests/tracklist.rs`. Define text/CSV options, parsed
  entries, same-tag/similar candidate reasons, explicit unmatched rows and limits.
  Reuse a proper CSV parser and Unicode normalization; do not silently simplify
  non-Latin names to empty strings. Observe RED before implementing.
- [x] Add coherent ready/nonarchived library matching and an atomic guarded
  create-playlist-from-reviewed-selections operation. Preserve row order, verify
  current IDs/readiness and expected playlist revision, and reuse playlist batch
  edit/Undo. Tests must prove failure leaves no empty playlist and source files
  are never changed.
- [x] Add a separate `tracklist-types.ts`/`tracklist-api.ts`,
  `src-tauri/src/tracklist_commands.rs`, `src/views/TracklistMatcher.tsx` and
  `src/test/tracklist-matcher.test.tsx` under `apps/desktop`. Inject bridges for
  tests and use existing player callbacks. Cover stale/late responses, no default
  selections, alternate-version review, selected file validation and save errors.
- [x] Integrate navigation, native command registration and a user-guide page.
  Verify screenshots, Rust tests, frontend tests/type checks/build and strict
  lint on the current branch. Keep native/runtime and source-only evidence clear.

Root owns App/navigation and native handler integration after other independent
workspace deliveries settle. This plan introduces no external network service.
