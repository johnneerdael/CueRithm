<!--
Merging needs a completed @codex review of the latest commit, resolved review threads and the sections below.
Full cross-platform builds run after the merge, not on the PR (add the `full-ci` label to run them early).
-->

## Summary

<!-- What changes and why. Link issues here. -->

## Changes

-

## Release notes

<!--
Public, user-facing, one bullet per change; it becomes this PR's entry in the next release.
If nothing changes for users, write `Internal:` followed by what changed.
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
