# Playlist tool reference inputs

`prefix-reference.json` records 104 positive cases from the unchanged
`PlaylistToolPrefix` class in Lexicon 1.11.15.0: 52 number and 52 text prefixes.
The private probe executes only that class in the installed Electron 32.3.3
RunAsNode runtime. Presence/truthy assertions use controlled stubs; every
published input satisfies them. Source and runtime hashes are recorded in the
JSON. No application startup, user library, authentication or network API runs.
Only independently authored inputs and numeric/text outputs are published.

Number prefixes use a minimum two-digit width when padding is enabled, widening
for the final number. Existing-prefix replacement recognizes one to three ASCII
digits and the source's optional dot/spaces/hyphen pattern. Four-digit prefixes,
digit-like Unicode and non-ASCII whitespace retain the source's behavior.
Text prefixes concatenate directly before JavaScript whitespace trimming.

The library tests separately validate ordered union, intersection, the active
library's complement of selected lists, source preservation, complete smart
snapshots, stable hidden slots, undo, stale previews and transactional rollback.
Name sorting uses trimmed/lowercase UTF-16 lexical keys; descending order reverses
ties as in the source. Track-count sorting uses direct membership, so folders
have zero direct links. These are bounded behavior checks, not a claim of full
Lexicon UI equivalence.
