# Portable research fixtures

These files are documentation artifacts, not importer implementations. JSON schema/reference values are factual layouts; example values are invented. Do not install partial records into a real DJ library or treat them as app-accepted outputs.

| File | Purpose |
| --- | --- |
|`import-observation.schema.json`|Proposed provider-neutral **envelope-shape** schema; semantic validators are separately required in the acceptance document.|
|`normalized-import-example.json`|Invented store-as-imported policy, raw/native/canonical time distinction, selected metadata and repeated ordered playlist membership.|
|`mixxx-schema-reference.json`|Columns, indexes and FK facts derived by running all40 pinned public schema migrations on an empty in-memory DB. Execute those migrations for complete schema creation.|
|`mixxx-wire-examples.json`|Invented protobuf hex and cue rows with expected seconds, end-only range, zero-time cue, sparse slots and A/B/A membership.|
|`mik-coredata-11-schema.json`|Observed SQL schema and compiled Core Data model attributes/defaults/relationships, without source library rows.|
|`mik-synthetic-import.json`|Invented song/collections/ordered links/cues, minimal bookmark-record path data and binary-plist beats. The bookmark is a parser vector, not an OS security credential.|
|`djstudio-sqlite-schema.json`|Columns/indexes/FKs from copied DJ.Studio SQL persistence.|
|`djstudio-key-map.json`|Exact24-key numeric/Camelot/OpenKey map.|
|`djstudio-synthetic-records.json`|Partial invented SQL/JSON structures, sharding and update test data. Never use as complete analyzed records.|
|`lexicon-serato-codec-vectors.json`|Generated Markers2/BeatGrid buffers successfully decoded by copied Lexicon buffer parsers; no music/tag files.|
|`lexicon-engine-codec-vectors.json`|Generated compressed Engine beat/cue buffers and loop records with expected units and mixed endianness.|

Read the owning adapter document before implementing a decoder. Validate all input lengths/counts/versions, preserve unknown fields and raw values, and report unsupported semantics. A fixture passing its own decoder is a control; independently compare target-app-generated cases before claiming support.
