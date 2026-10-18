# Pull request and release workflow

Pull requests are gated cheaply; the expensive builds run once, after the merge.

1. Run the checks in the PR template locally (`cargo fmt/clippy/test`, plus the desktop `pnpm` test and build when `apps/desktop` changes) and tick them in the PR body.
2. Fill in `## Release notes` (public bullets, or `Internal: ...`). Merged PRs' notes become the release notes.
3. Comment `@codex review` (or rely on automatic Codex review). Resolve every review thread (resolving fires no event: run `gh workflow run pr-gate.yml -f pr_number=N` to refresh the gate).
4. The `PR gate` status passes once the PR body is valid, no threads or change requests are open and Codex has completed a review of the latest commit. Squash-merge when it is green.
5. A push to `main` runs `Engine checks` and `Desktop checks` (all platforms). When both pass for a commit, `Release` publishes a signed, notarized macOS release `vX.Y.Z` (DMGs for arm64 and x64, plus decoder source) with the collected release notes. Windows and Linux are not published yet. `X.Y` follows the workspace version in `Cargo.toml`; the patch number increments per release.

Escape hatches:
- Add the `full-ci` label to a PR to run the full builds before merging.
- A failed post-merge build is fixed by pushing directly to `main`; repository admins bypass the PR requirement. Docs-only pushes skip the builds.
- Re-run a release for a green commit with the `Release` workflow's manual dispatch.

`.github/scripts/` holds the gate and release-note logic; run its tests with `python3 -m unittest discover -s .github/scripts`.
