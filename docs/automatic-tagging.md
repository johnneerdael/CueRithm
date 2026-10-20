# Automatic tagging

Enable **Settings → Automatic tagging** and save once. The same configuration
applies to new imports, watch-folder arrivals and explicit Analyze again jobs.
It does not automatically retag tracks already in the library when Settings
change. Existing offline configurations stay offline until enabled.

Preparation runs full audio analysis, catalog matching, staged metadata writing
and replacement cues before moving the track from Incoming to Tracks. Original
audio stays at its location. Key, BPM, beatgrid, cue and whole-track energy
results in the library remain measured analysis, independently of catalog tags.

## Providers and matching

Select Beatport and/or Traxsource. Move providers earlier or later to set their
order. With Combine providers off, the first successful match supplies metadata.
With it on, every selected provider is consulted and matches apply in order.
Configured provider failures leave the job in Incoming for retry; they are not
reported as successful empty results. Traxsource uses the shared public-catalog
fetcher to detect Cloudflare challenges. Detected restrictions are persisted,
and a marked source stops direct requests until explicitly reset. Configure an
optional FlareSolverr-compatible URL under Automatic tagging; affected sources
use its `/v1` endpoint, with a 120-second default solve timeout and one retry for
timeouts or temporary unavailability.
See [solver setup](user-guide/guides/catalog-solver.md) for FlareSolverr, Byparr
and Trawl, source availability, caching and troubleshooting.

Matching settings include title/artist strictness, optional duration tolerance,
embedded IDs/ISRC, oldest/newest release preference, filename parsing and title
cleanup patterns. Skip previously tagged recognizes OneTagger and Cuerithm date
markers; audio analysis and cue replacement still run. Album matching can reuse
an identified Beatport release for the remaining Incoming files in the same
folder and settings revision. It does not retag older library files in that folder.

Tagging concurrency is shared across analysis workers. Traxsource permits one
active request at a time. Audio workers independently use 75% of process-available
logical cores, rounded down with a minimum of one. Include subfolders controls
both watched folders and folder imports, even when catalog tagging is disabled.

## Fields and output

Select fields to write and choose overwrite-all or individual overwrite exceptions.
Unselected fields remain unchanged. Available fields come from the selected
provider implementations; the editor shows which providers supply each field.
Key/BPM file tags can use measured analysis or catalog metadata. Track energy is
a separate optional file write and remains a single whole-track rating.

Genre/style routing offers eight routes, optional merging/capitalization and
per-format custom style fields. Choose embedded artwork or an external cover file
beside the audio using a relative filename template. External mode retains
existing embedded covers. Enable Album art under
Fields to write to retrieve artwork. Artwork overwrites follow the same field
policy. File-format settings include ID3v2.3/2.4, separators, comment language,
year-only dates and track-number padding. Beatport adds artwork resolution,
search-page count and version matching controls.

If a catalog returns no match, either retain existing catalog metadata and finish
with configured measured tags, or leave the track in Incoming for attention.
A no-match outcome is retained in preparation provenance. Failed writes retain
previous cues and use journaled backups/recovery. Original backups are retained
in application data; Settings shows their count and size. Redundant staged
outputs are cleaned automatically, and no-op writes retain no duplicate audio.
Unchanged provider retries reuse completed full analysis. If a file changes during lookup,
its new bytes are analysed before publication; an older attempt cannot overwrite
a newer job.

## Validation and current access

Deterministic local HTTP fixtures validate both provider parsers, matching,
failures, expiry and concurrency. Generated audio fixtures validate metadata and
artwork changes across the supported formats with exact before/after decoded
PCM and frame checks. These checks establish pipeline behavior, not catalog
coverage or musical-analysis accuracy.

A native Beatport catalog probe returned a public diagnostic match through its
catalog API. Direct Traxsource requests returned HTTP 403 on the validation
machine. A read-only Trawl probe retrieved live search, track and release pages
that the Rust adapter parsed, including key, BPM and release metadata. This
establishes that fetch/parse path, not a file-write or packaged-app acceptance. A blocked provider stays actionable in
Incoming, without silently bypassing a required tagging stage.

The integration excludes Spotify/audio-feature workflows, Shazam, QuickTag,
standalone tag editing/renaming and post-commands. It reuses pinned OneTagger
source under GPLv3; desktop licensing and modifications are documented in
[third-party notices](../THIRD_PARTY_NOTICES.md). The separate analysis engine
remains MIT.
