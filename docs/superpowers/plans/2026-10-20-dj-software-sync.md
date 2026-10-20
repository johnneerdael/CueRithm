# Native DJ sync implementation

Worktree: `.worktrees/dj-software-sync`, branch `dj-software-sync`.

1. Inspect hash-matched Lexicon bundle using leanrea; record independent format contracts.
2. Add shared typed Rust library/track/cue/grid/playlist projections.
3. Implement current VirtualDJ, Engine DJ, djay Pro and Serato 4 adapters with format tests.
4. Implement Pioneer USB database, copied audio and analysis assets; test device export layout.
5. Add target discovery, process checks, validation, durable preview/backup/apply and restore.
6. Build library projections with full/playlist/modified scope and configurable field mapping.
7. Wire Tauri commands and desktop sync target/options/preview/apply UI; preserve settings.
8. Review and verify current changes, test all required workspace/frontend gates.

Do not mark goal complete until every target and screenshot requirement is accounted for.

## Implemented and observed on this worktree

- Current-format adapters for all five new targets, discovery and process checks,
  source/target/sidecar stale guards, full/playlist/Modified projection, preview,
  recoverable publication and desktop target/options/history UI.
- Engine device audio copies, PNG artwork and performance data; Pioneer PDB-only
  merge, encrypted Plus catalog, copied audio/ANLZ/artwork and bundled-runtime AutoFix.
- Versioned source timing capture, exactly-once cue/grid export conversion, MPEG
  decoded-count fields and detailed Pioneer waveform padding. Saved publications and
  playback remain canonical.
- Native smart export for supported whole definitions, with extra prepared metadata,
  explicit IANA timezone and harmonic/fuzzy key-mode context. Unsupported fields,
  groups or semantics materialize the complete captured membership. Engine text,
  djay BPM and Serato DateAfter deliberately materialize for unresolved native
  comparison semantics. Pioneer always writes ordinary playlists.
- Durable native playlist identity and created-versus-matched ownership, owned
  deletion propagation in Full/Modified, external-match retention and guarded
  rename/move/reorder/representation transitions. Unrelated native entries remain.
- Batch staging and typed file/directory journaling, including exact case-only
  rename/restore. Apply and restore acknowledgement recovery settle the same journal
  idempotently before another preview can begin.
- Source serializer/parser probes for VirtualDJ, Engine, djay, Serato and Pioneer.
  All fixtures are synthetic; private probes remain under ignored analysis paths.
- Focused native and library regressions cover case-only subtree apply/restore,
  source protection, ownership and acknowledgement recovery. Current-head workspace,
  frontend, lint and build evidence is recorded in
  [PR 12](https://github.com/johnneerdael/CueRithm/pull/12).
- The pinned decoder was built/sealed with the existing recipe and exercised by the
  explicit Pioneer AutoFix test. The optional runtime-dependent test remains separate
  from the default suite. Test totals are not frozen while fixes continue.
- The guide documents current behavior and contains two real production React Sync
  captures with synthetic data and production branding. The temporary capture harness
  and browser/server resources were removed afterward.

## Required completion gates

1. Finish the current-head artifact/code review and resolve any new findings.
2. Run all required workspace/frontend checks again after the final changes; retain
   exact current-head logs and optional-runtime test evidence.
3. Commit/push the isolated branch, open the PR and complete repeated Codex review
   and CI on its latest head before merging, as required below.
4. Keep native application/restart/playback and physical CDJ acceptance explicitly
   separate. Synthetic/reference checks do not establish those outcomes.

Keep the original goal active through its requested review and integration gates.
Do not treat staging, serializer agreement or a build as device playback or release.

## User-added delivery gates

After implementation is complete, use reverse-engineer-anything and Ghidra to
walk the installed Lexicon artifact again and review the Rust implementation
against authoritative shipped behavior. Resolve findings before opening a PR to
main. Commit/push the isolated branch, open the PR, run repeated Codex reviews and
address findings until approved, then merge to main. Update the user guide,
documentation and images to match final delivered behavior. The goal is not
complete merely because implementation tests or one review pass succeeds.

## Implementation and review scope

Source timing, native smart compilation, ownership/deletion, typed directory and
case-rename journals, batch staging and both apply/restore acknowledgement recovery
are implemented. Final artifact review found and fixed ANLZ collision allocation,
hidden catalog discovery, dotted VirtualDJ names, Serato explicit null clears, djay
filename indexing and Engine raw BPM precision. Focused regressions cover these fixes;
current-head validation and integration evidence are recorded in PR 12.

Ghidra 12.1.4 static review covered only the shipped native SQLite addon (SHA-256
`9e351a1ed003cd743b341d0b0b4accc7d300d8608694fbffedf5371fa05cd8b7`),
including key attach and SQLCipher version-4 table parameters. It did not decompile
JavaScript. REA's hash-matched ASAR analysis has partial semantic coverage because
of heap/result limits; targeted converters were compared manually with synthetic
probes. See [format evidence](../../research/dj-software-formats.md) for scope and
parameters. No whole-app semantic completeness or native DJ/hardware acceptance is
claimed. Older Lexicon target formats and the separate direct rekordbox 6/7 groundwork
remain outside these five current-target adapters.
