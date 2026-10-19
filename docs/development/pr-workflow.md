# Pull request and release workflow

Pull requests are gated cheaply; the expensive builds run once, after the merge.

1. Run the checks in the PR template locally (`cargo fmt/clippy/test`, plus the desktop `pnpm` test and build when `apps/desktop` changes) and tick them in the PR body.
2. Fill in `## Release notes` (public bullets, or `Internal: ...`). Merged PRs' notes become the release notes.
3. Comment `@codex review` (or rely on automatic Codex review). Resolve every review thread (resolving fires no event: run `gh workflow run pr-gate.yml -f pr_number=N` to refresh the gate).
4. The `PR gate` status passes once the PR body is valid, no threads or change requests are open and Codex has completed a review of the latest commit. Squash-merge when it is green.
5. A push to `main` runs the fast `Engine checks` and `Desktop checks` (tests and lint on all platforms, no packaging). When both pass and the merged PRs since the last release include at least one public release note (not only `Internal:`), `Release` builds the desktop packages (`package.yml`) and publishes `vX.Y.Z`: a signed, notarized Apple Silicon DMG, an unsigned Linux `.deb` and Windows portable zip, and the decoder source. There is no Intel build. `X.Y` follows the workspace version in `Cargo.toml`; the patch number increments per release.

Escape hatches:
- Add the `full-ci` label to a PR to run the full builds before merging.
- A failed post-merge build is fixed by pushing directly to `main`; repository admins bypass the PR requirement. Docs-only pushes skip the builds.
- Force a release (even for internal-only changes) or re-run a failed one with the `Release` workflow's manual dispatch on a green commit.
- Test packaging without releasing by dispatching `Package desktop builds` with a commit SHA; the packages are kept as artifacts for 7 days.

`.github/scripts/` holds the gate and release-note logic; run its tests with `python3 -m unittest discover -s .github/scripts`.
