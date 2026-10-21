<!--
Merging needs a completed @codex review of the latest commit, resolved review threads and the sections below.
Full cross-platform builds start once the gate passes (a completed Codex review of the latest commit), not on every push, and again after the merge.
-->

## Summary

<!-- What changes and why. Link issues here. -->

## Changes

-

## Release notes

<!--
Write for the person using the app, not the person reading the diff. Each bullet is one change
they can notice: what they can do now, or what behaves differently, in plain words. The bullets
become this PR's entry on the release page.
No crate, module or function names, and no internal terms (coordinator, planner, provenance,
schema, reservation). If a DJ would ask "what does that mean for me?", rewrite it.
  Good: Tracks that could match several editions now stay in Incoming instead of being tagged with a guess.
  Not:  Add deterministic candidate evidence and cross-provider identity checks.
If users cannot see the change, write one `Internal:` bullet saying what changed.
Leave out test results and install information.
-->

-

## Validation

<!-- Tick what you ran locally; delete lines that do not apply. Unticked boxes block the PR gate. -->

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `pnpm --dir apps/desktop test && pnpm --dir apps/desktop build`, if `apps/desktop` changed

## Not tested / known limitations

-
