# Automatic smart-playlist inputs

Read-only inspection of the local collection found 434 FLAC tracks. All 434 carry `DATE`, `RELEASE_TIME` and `YEAR`; both date fields are full calendar dates. Seven tracks have different `DATE` and `RELEASE_TIME` values, so explicit release tags take priority over generic dates. 233 tracks contain collaboration separators and 45 have a remixer tag. Identifying metadata and exact track values remain under ignored `.analysis/`.

Static inspection of licensed Lexicon 1.11.15 establishes these behaviors (not an observed UI execution):

- **Seen:** renderer `KeyFilter.isSimilar` (character offset 8518099) delegates to `KeyConverter.isCompatibleKey` and the selected harmonic/fuzzy mode. `KeyConverter` starts at 7082308. Harmonic mixing uses the same key, relative major/minor, and neighboring wheel numbers with the same mode. Fuzzy expands compatibility to both modes of neighboring numbers.
- **Seen:** renderer `NumberFilter` (8517033) applies inclusive `NumberBetween` boundaries, rounding BPM to two decimal places. The automatic range UI rejects reversed bounds even though the general Lexicon operator accepts them.
- **Seen:** renderer `SmartlistGenerator` (8531394) builds field-value lists and numeric ranges, persists them on demand, and skips existing smartlists by name in the selected folder. CueRithm automation uses managed identities so manual lists with matching names stay independent.
- **Seen:** a private database-copy inspection found 819 existing smartlists: 768 month/energy lists, 24 key-similarity lists with an additional BPM restriction, 23 artist/remixer lists, and four BPM lists. Monthly energy rules use 1–5, 6–7 and 8+. Existing months are encoded by comment suffixes.
- **Inferred design choice:** automatic release folders can use the collection's actual date metadata rather than reproducing the manual comment convention. Create 8–10 Anthems lists as requested. Key automation uses the existing `KeySimilar` evaluator without the private lists' extra BPM restriction.

No application entry point, license credentials, tag writer or source audio was executed or modified during inspection. Shipped code remains in the private temporary extraction; this document contains behavior and aggregate observations only.
