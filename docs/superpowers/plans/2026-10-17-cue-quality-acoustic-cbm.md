> Superseded for cue generation by `docs/cuerithm-integration.md`. Do not
> continue MiK cue work from this historical plan.

# Cue-quality acoustic CBM experiment plan

> Execute inline using executing-plans, within the approved engine research scope. No implementation subagents.

**Goal:** Produce a frozen, offline, quality-first comparison of three cue policies on identical existing PCM-derived features and candidate bar grids.

**Architecture:** Private Rust CBM math plus numeric JSON worker linked to the existing Rust novelty selector. Python only orchestrates verification/measurement; it is not an engine runtime dependency. Production code/defaults remain unchanged until quality evidence supports a candidate.

**Spec:** `.analysis/experiments/cue-quality-acoustic-cbm-v1/design.md`. Official BSD CBM source/license are retained in its upstream folder; no unlicensed wrapper code is used. No Git root exists; preserve source hashes and failed attempts instead of commits.

- [x] Math tests RED: stationary/zero features yield no fabricated transitions; abrupt repeated sections produce their exact boundaries; invalid/nonfinite features are rejected. `rustc --test cbm.rs` followed by the binary must fail for the stub's missing boundary and validation behavior.
- [x] Implement normalized cosine SSM, banded-prefix DP4..32 and novelty gate; run these synthetic behavior tests GREEN. These establish math behavior, not music quality.
- [x] Add numeric worker with exact input-key validation, unchanged novelty arm, bar-only novelty arm, CBM arm, <=8 ordered finite in-range cues, no arbitrary fill or semantic claims. Record core/binary/source/policy identity and invalid-domain tests.
- [x] Export282 tuning-only numeric cases from already retained source predictions, with corpus/source/group identity checked. Freeze arm parameters and24 listening IDs by deterministic SHA256 rank before comparison. Deny worker network and source/reference/music access; retain every result/failure and observe terminal barrier before grading.
- [x] Score one-to-one time and beat tolerance curves, counts and candidate coverage against MiK as diagnostics. Save anonymous24-group listening cue sets. No quality winner until listening evidence.
- [x] Update aggregate research docs with observed results and limitations, freeze experiment artifacts, keep full engine goal active (key backend integration, energy, quality/format/platform/holdout gates remain).

Observed:282/282 native jobs completed,10Rust synthetic tests and4matching checks pass. The24listening inputs are prepared but unreviewed; musical quality remains unproven and no default is promoted.
