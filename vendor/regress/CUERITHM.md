# Cuerithm adaptation of regress 0.12.0

Source and archive digest are pinned in UPSTREAM.json. Retain LICENSE-MIT and
LICENSE-APACHE. The original API remains unlimited; this is not an official
upstream release.

The additional bounded Boolean API accepts UTF-16 input, uses UCS-2 for legacy
patterns, supports sticky offset-zero matching and returns BudgetExceeded
separately from no-match. A local optional fuel value meters instructions,
backtracking, candidate positions, optimized character repeats and captured
backreference lengths. Nested assertions share that value and cannot absorb an
exhaustion as successful negation. Existing unlimited constructors have no fuel.

Source-helper differential cases required legacy case canonicalization repairs
and correct case-fold-before-algebra behavior for UnicodeSets, including string
disjunctions and complements. These intentionally correct legacy/UnicodeSets
results beyond the budget addition. The opt-in legacy_code_units flag prevents
surrogate-pair combination in legacy body escapes while identifier names remain
Unicode. The wrapper uses checked indexing/index positions. Generated Unicode
tables are retained from the pinned archive.

The empty workspace table permits isolated upstream verification from a nested
project worktree. Public fixtures contain original synthetic metadata and scalar
outputs, with the actual shipped V8 runtime recorded. Proprietary helper code
remains outside the repository. No full ECMAScript-conformance or full Lexicon
application-parity claim follows from the finite differential corpus.
