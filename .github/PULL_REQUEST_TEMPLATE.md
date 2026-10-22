<!--
Merging needs a completed @codex review of the latest commit, resolved review threads and the sections below.
Linux checks start once the gate passes (a completed Codex review of the latest commit), not on every push. Windows and macOS run after the merge, on main.
-->

## Summary

<!-- What changes and why. Link issues here. -->

## Changes

<!--
For extraction, key/BPM/energy analysis, training, evaluation or model promotion:
cite the revision and relevant F-numbers from docs/research/mik-facts-and-modelling-constraints.md.
State the inputs/target, counterexamples checked, split/lineage exposure and separate
captured-feature versus end-to-end results. Explain non-applicability for other changes.
If evidence contradicts a finding, link its explicit supersession rather than silently ignoring it.
-->

-

## Release notes

<!--
Write for the person using the app, not the person reading the diff. Each bullet is one change
they can notice: what they can do now, or what behaves differently, in plain words. The bullets
become this PR's entry on the release page. Start with a short bold benefit, then explain
how it helps in one or two short sentences. Group related fixes into a single clear point.
No crate, module or function names, and no internal terms (coordinator, planner, provenance,
schema, reservation). If a DJ would ask "what does that mean for me?", rewrite it.
  Good: **Review uncertain matches.** Tracks that could match several editions stay in Incoming for you to check.
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
