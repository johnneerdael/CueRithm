# Pull request and release workflow

Pull requests are gated cheaply. Before the merge only the Linux checks run, and only once Codex has finished reviewing the latest commit, never on every push. Windows and macOS run after the merge, on `main`, and a failure there is fixed on `main`.

1. Run the checks in the PR template locally (`cargo fmt/clippy/test`, plus the desktop `pnpm` test and build when `apps/desktop` changes) and tick them in the PR body.
2. Fill in `## Release notes` (public bullets, or `Internal: ...`), written for the person using the app (see below). Merged PRs' notes become the release notes.
3. Comment `@codex review` (or rely on automatic Codex review). Resolve every review thread (resolving fires no event: run `gh workflow run pr-gate.yml -f pr_number=N` to refresh the gate).
4. The `PR gate` status passes once the PR body is valid, no threads or change requests are open and Codex has completed a review of the latest commit. At that point the gate dispatches the Linux runner of `Engine checks` and `Desktop checks` for that commit, once per commit. A newer push cancels the old commit's unfinished runs and starts nothing until it has its own completed review. Squash-merge when the gate and the Linux checks are green; do not wait for other platforms.
5. A push to `main` runs the fast `Engine checks` and `Desktop checks` (tests and lint on all platforms, no packaging). When both pass and the merged PRs since the last release include at least one public release note (not only `Internal:`), `Release` builds the desktop packages (`package.yml`) and publishes `vX.Y.Z`: a signed, notarized Apple Silicon DMG, an unsigned Linux `.deb` and Windows portable zip, and the decoder source. There is no Intel build. `X.Y` follows the workspace version in `Cargo.toml`; the patch number increments per release.

## Writing release notes

The release page is read by DJs who use CueRithm, not by people who read the code. A note answers "what can I do now, or what works differently?" in plain words.

- One bullet per change a user can notice. Lead with the effect, then the condition if it matters.
- Use the words the app uses (Incoming, Tracks, History, Sync). Leave out crate, module and function names and internal terms such as coordinator, planner, provenance, schema and reservation.
- Say what happens to their files and libraries when that is the point: what is kept, what is never overwritten, what can be undone.
- Work users cannot see gets a single `Internal:` bullet. It is kept off the visible release page.
- Test it by asking whether a DJ who has never seen the code would know what it means for them.

| Instead of | Write |
| --- | --- |
| Add deterministic candidate evidence and cross-provider identity checks. | When a track could match more than one release or edition, it now stays in Incoming marked as needing attention instead of being tagged with a guess. |
| Preserve protected metadata and native DJ records during automatic tagging. | Automatic tagging can no longer overwrite a track's rating, comments, lyrics or artwork, or the cue points and beatgrids that Serato, rekordbox and Traktor keep in your files. |
| Add recoverable organization services. | Internal: add the file naming and organization services behind the upcoming organize feature. |

The release page shows these bullets under "What's new". Internal bullets and direct commits to `main` are collapsed under "For developers".

Escape hatches:
- Run the full builds earlier, before a review has finished, with `gh workflow run ci.yml --ref <branch>` and `gh workflow run desktop.yml --ref <branch>`. Only same-repository branches can be dispatched.
- Run only a failed runner with `gh workflow run desktop.yml --ref <branch> -f os=windows-2025`. This also works on `main`; omitting `os` runs every configured platform. Targeted runs have separate concurrency groups and do not cancel a full run still finishing.
- Fix a failed post-merge build directly on `main`, using the process below to retain passing checks. Docs-only pushes skip the builds.
- Force a release (even for internal-only changes) or re-run a failed one with the `Release` workflow's manual dispatch on a green commit.
- Test packaging without releasing by dispatching `Package desktop builds` with a commit SHA; the packages are kept as artifacts for 7 days.

## Retaining passing checks after a build fix

For a transient failure with no source change, wait for the workflow to finish and use `gh run rerun RUN_ID --job JOB_ID` for the failed job. That retry uses the original commit.

For a source fix, first review which components the patch invalidates. Push the repair with `[skip ci]` to prevent a fresh full matrix, and record the checked ancestor and the components that must run again in commit trailers. For a Windows-only repair:

```text
fix(ci): correct Windows-only lint [skip ci]

CI-Baseline: <full 40-character checked ancestor commit>
CI-Retry: engine/windows-2025 desktop/windows-2025
```

Then run only those failed platform components against the corrected `main`:

```sh
gh workflow run ci.yml --ref main -f os=windows-2025
gh workflow run desktop.yml --ref main -f os=windows-2025
```

The release gate checks all seven logical components: the Rust floor plus engine and desktop checks for Linux, macOS and Windows. A combined Windows job can satisfy both Windows components. For an explicitly declared repair, the gate verifies the ancestor's retained job results and requires fresh success for every component listed in `CI-Retry`. Missing, failed, cancelled or unfinished checks block publication; a retry cannot inherit an older success. A newer failure also supersedes an older passing attempt.

The trailers are the maintainer's assertion that the other results remain valid. Never retain a component affected by the patch. The release log records the exact checked commits, workflow runs and jobs used. Packaging still builds the selected release source on every platform.

`.github/scripts/` holds the gate and release-note logic; run its tests with `python3 -m unittest discover -s .github/scripts`.

The release gate and note generator both use the latest global `v*` tag as their
previous release. An unrelated tag blocks publication rather than replaying old
notes. For a verified source-history migration, `.github/scripts/release_history.json`
records the exact published tag commit and its equivalent baseline on current
`main`. The resolver requires that unchanged tag target and an existing baseline
ancestor of the checked source. The 2026-10-22 migration preserves `v0.1.11` and
its published assets; its equivalent baseline excludes already released changes
from future notes. Migration records do not bypass any platform checks, source
promotion restrictions or duplicate-release guards.
