# DJ library import and synchronization engineering handover

Research completed 2026-10-23. This pack specifies onboarding and refresh from every local DJ library/playlist source linked by [Lexicon's import manual](https://www.lexicondj.com/manual/import-options), plus Mixed In Key11, DJ.Studio and Mixxx. DJ.Studio also has a separate write-back design. It contains clean-room format contracts, source evidence, schema manifests, invented parser fixtures, integration guidance and acceptance gates. **Application code and live DJ libraries were not changed.** No dedicated worktree, commit, release or deployment was created.

The main product requirement is **select what to bring over, then choose whether to preserve it, retag it or reanalyse it**. Imported metadata and source analysis must not automatically enter the current Incoming preparation pipeline. BPM, grid, key, generic markers, hot/memory cues, loops and collections have independent selection/provenance. Preserve source IDs, full hierarchy/order and raw metadata even when a target UI is narrower.

## Start here

| Read order | Document | Engineer's outcome |
| --- | --- | --- |
|1|[Import and sync contract](import-and-sync-contract.md)|Field choices, store-as-imported/retag/reanalyse behavior, normalized observations, identity, merge/conflict, snapshots and operation receipts.|
|2|[CueRithm integration](cuerithm-integration.md)|Existing main adapters to reuse, storage/API changes needed, timing conversion and implementation order.|
|3|[Lexicon-supported source formats](lexicon-import-formats.md)|Exact native tables, joins, units, binary codecs, source paths, hierarchy and losses for each local import source.|
|4|[Mixed In Key11](mixed-in-key-11.md)|Core Data schema/model, macOS bookmarks, folders/ordered membership, analyzed vs tagged fields, persisted cues and beat archives.|
|5|[DJ.Studio](dj-studio.md)|SQL versus imported JSON surfaces, processed identities, grids/cues, project boundaries, proposed journaled offline writes and restrictions.|
|6|[Mixxx](mixxx.md)|Pinned open-source DB/wire contracts, cue sample/frame distinction, key enums, playlists/crates and inverse decoder clock.|
|7|[File tags and provenance](file-tags-and-provenance.md)|Database vs cached vs current file values, native tag bindings, artwork and honest “all tags” coverage.|
|8|[Validation and acceptance](validation-and-acceptance.md)|Semantic validator, shared/adaptor test matrices, proof levels and DJ.Studio write qualification.|
|9|[Verification report](verification-report.md)|Checks actually executed on this pack, scope limitations and changed-file audit.|

Use [fixtures/README.md](fixtures/README.md) to find portable schema/codec examples. They contain invented data or schema facts, never a user's music/library. Proprietary app code and binary extracts are not included or required for the documented decoder designs.

## Coverage and capability boundaries

“Read design” below means a researched engineering contract, **not a shipped CueRithm importer**. Availability of fields depends on the exact source artifact/profile. Each adapter document contains detailed mapping and evidence.

| Source | Read design | Main metadata/performance source | Important boundary |
| --- | --- | --- | --- |
|Rekordbox6/7|Direct SQLite/ANLZ|Tags/key/BPM/MyTags, hot/memory cues, grids, playlists|Keep current accepted app profiles; no XML transport or broadened write claims.|
|Serato3 / Lite legacy|Binary DB/crates + media tags|Metadata, Markers2/BeatGrid, crate hierarchy/order|Per-volume libraries and container wrappers matter; DB alone is insufficient.|
|Serato4 / Lite|SQLite + media tags|Root/location assets/containers, performance tag payloads|Use v4 crate store; retain optional-schema distinctions.|
|EngineDJ|SQLite + compressed/mixed-endian performance blobs|Tags/key/BPM, cues/loops/grid, UUID-linked playlists|Read copies without Lexicon's schema-mutation paths.|
|TraktorPro3/4|NML|Tags/key/BPM, typed cues/loops/grids and playlist tree|Not TraktorDJ; cue semantics and units stay explicit.|
|VirtualDJ|XML/MyLists/legacy playlist files|Tags/key, Scan/POI, grids/cues/loops and folders|Scan BPM is seconds/beat; actions/automix are distinct.|
|djayPro|YapDatabase typed stream/bplist|Manual/analyzed metadata, grids/cues/loops, UUID hierarchy|Version/profile and blob branches must be qualified.|
|Rekordbox5|XML format research documented|Collection tags, position marks, tempo and playlist IDs|**Unresolved policy boundary:** project instructions prohibit rekordbox XML transport, so this pack does not authorize an importer for it.|
|AppleMusic / iTunes|Exported plist XML|Metadata and persistent-ID playlist hierarchy|XML route lacks DJ performance data; preserve fields present rather than promise cues/grids.|
|Pioneer / AlphaTheta USB / OneLibrary|Device SQLite or PDB + DAT/EXT|Device metadata, cues/grids/playlists/history|Distinct from desktop rekordbox; cipher/profile and copy/collision handling explicit.|
|M3U/M3U8|Text references + read-only file tags|Ordered membership and file metadata|No embedded grid/hotcue model; relative paths/duplicates need explicit behavior.|
|MixedInKey11 macOS|Core Data SQLite + archive/bookmark codecs|Analyzed and cached tags/key/BPM/energy, generic cue markers, beat arrays, folders/order|No stored hotcue slots/type/color; native Windows store uninspected; portable path mapping documented.|
|DJ.Studio|SQLite plus sharded JSON/structures|Explorer metadata/collections and separate imported-track analysis/cues|Existing project overrides remain separate; offline write-back is qualified per operation, not a blanket DB edit.|
|Mixxx|SQLite + Beats/Keys protobufs|Metadata, grids, typed cues/ranges/loops, playlists/crates|Existing writer on main can be reused; new reader must handle units and source decoder origin.|

Lexicon's installed UI also exposes online service imports. Beatport/Spotify/Tidal account/network ingestion is outside this local DJ-software-library scope and was not investigated through account credentials. Streaming references encountered inside the listed libraries must still be retained as references with explicit availability.

## What was established

The three installed commercial apps were inspected using the requested leanrea workflow on separate copies: Lexicon1.11.15; MIK11.2.6 build11.2.6.7446; DJ.Studio4.2.9 with a separately identified bundled renderer. Mixxx source is pinned at2086fe0e551b9eb9dd083dd706a5848eb06534d0. Core CueRithm integration is pinned to main190f2f549454e5c6e8486e4949042197351ea14f, while this workspace stays on its original older feature branch. Later USB cross-references pin446aab6 explicitly; the difference from190f2f5 was an unrelated research document, not an adapter change.

Executed controls include original Lexicon Serato buffer parsing on synthetic vectors, Engine structural/zlib vectors, MIK446/446 bookmark path comparisons and446 beat-archive decodes, DJ.Studio copied schema/JSON-shard checks and synthetic transaction controls, plus Mixxx schema/wire examples. They establish the specific recorded facts; none is an end-to-end CueRithm importer or DJ.Studio write acceptance result.

## Known limits that must survive the handoff

- Exact runtime acceptance is still required per source-app/schema/OS/decoder profile; static reading cannot prove successful display/playback or safe write-back.
- DJ.Studio metadata edits, grids, marker arrays, playlist order, new analyzed-track insertion and project propagation are different capabilities. New analyzed records require validated processed-audio identity/analysis invariants. This pack does not enable a live writer.
- MIK's beat timestamps do not establish downbeat/meter/subdivision. Its database cache is not a dump of every arbitrary file tag or embedded artwork. The file-tag contract closes that requirement without retagging or modifying music.
- Rekordbox5 remains a documented coverage/policy exception. Resolve that instruction explicitly before adding XML; direct6/7 support does not make5 readable through the same route.

An engineer can implement the documented readers and the restricted DJ.Studio write experiment from this folder and the pinned CueRithm/Mixxx sources without commercial suites. Release qualification needs a separate operator with the exact source apps and disposable fixtures. All unresolved semantics are called out at the field/operation where they matter.
