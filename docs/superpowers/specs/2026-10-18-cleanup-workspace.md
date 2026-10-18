# Cuerithm cleanup workspace

This is the next part of the approved library-workspace design. It implements
Smart Fixes, Genre Cleanup and Artist Cleanup in Rust, with the same rules used
by automatic Incoming and the manual cleanup workspace. The user's purpose is
useful, consistent metadata without random genre accumulation. Preserve key,
BPM, beatgrid, whole-track energy, cues and audio. Plays, Last Played and
Danceability remain excluded.

## Behavior and source evidence

The inspected licensed Lexicon 1.11.15 artifact has eleven fixes: casing,
character replacement, encoded characters, artist extraction, remixer
extraction, garbage cleanup, number prefixes, URL/email removal, mix
parentheses, common title text and promotional text. Implement all explicit
options. Genre and artist changes use configured mappings and protected values;
similar spelling alone never merges identities.

A local reference harness executes selected pure helper functions without the
application entry point, filesystem APIs or network APIs. The artifact SHA is
`85fdbba4af97886ff1a626e6a4c96b4f9866dca3624fb2348a0332d7d6b63860`.
131 synthetic metadata cases cover all eleven families. Keep source outputs and
intentional policy expectations separately in fixtures. The harness is a source
helper probe, not full application/UI parity. Distribute no extracted code,
application assets, music or identifying library data.

Preserve metadata where stock helpers lose it: retain `vs`, decode numeric and
nested entities with bounded work, keep Unicode words when stripping number
prefixes, replace Unicode scalar characters intact, preserve bare audio filenames
when removing URLs, and remove complete email addresses. Garbage cleanup must be
idempotent. Document every fixture whose policy expectation differs from the
source result; do not advertise these differences as exact parity.

## Shared rule engine

Use original, independent Rust code with explicit typed options and validated
limits. Smart Fix order is user-controlled and deterministic; canonical genre
and contributor mappings follow. Active alias sources/targets are protected from
text fixes in their selected roles so explicit mappings remain applicable and
chosen canonical spelling survives repeated cleanup. Preserve multiple
native contributors and genre values; do not split an artist name on punctuation
to guess separate identities. Preserve field ordering where it has meaning.
Empty/malformed extraction separators, invalid part indices, cyclic alias rules
and empty protected tokens fail validation. An empty/no-op configuration changes
nothing. No filesystem paths, musical-analysis outputs, cue data or binary tags
are cleanup targets.

Genre cleanup lists distinct values/counts, supports protected values, canonical
mappings and conversion of selected source genres into categorized custom tags.
Keep original/provider genre provenance so mappings can be revised. Artist
cleanup supports artist, remixer, producer, composer and lyricist roles,
protected names, explicit aliases, counted previews and stable sorting/search.
Do not infer that spelling similarity establishes identity.

## Manual workflow

Select Smart Fixes, Genres or Artists; choose all Ready tracks or selected track
IDs; scan; review before/after fields and counts; apply selected proposals or
ignore values. Playlist scope will supply track IDs when playlists are delivered.
Do not mutate during scanning. Unchecked proposals are not applied. Configuration
and ignored/protected values persist separately from one-off selections.

Default manual Apply updates file tags and library metadata together, with
recoverable originals and durable Undo. This default was presented in the
optional preference question; a library-only preference can change it before
manual publication is implemented. All research/testing uses generated fixtures.
No existing private music is modified by development acceptance.

Bind proposals to per-track source and metadata/custom-tag fingerprints. If a
file, relevant metadata, archive state or preparation ownership changes, reject
the stale proposal and ask for a rescan. Keep unrelated tracks usable. Preserve
previous committed cues and analysis throughout manual cleanup. Undo must also
check the current post-apply identity and preserve newer edits. Batch results
report applied, unchanged and conflicted tracks accurately.

Reuse verified tag staging, exact PCM/clock checks, publication gate, durable
backups and atomic filesystem primitives. A metadata-only operation updates
metadata/custom tags without creating cue revisions or running DSP. Recover
interrupted operations before normal workers start. Share destination
reservations and owned-watch hashes with preparation journals. Show backup
storage across both workflows; keep originals until explicit retention cleanup.

## Automatic Incoming

Add saved cleanup rules to immutable preparation settings, disabled by default.
After configured catalog field/overwrite selection, run enabled cleanup on the
effective metadata, then stage and publish once. This also works with catalog
tagging disabled. Ready requires configured cleanup to succeed. Preserve raw
provider values and cleanup provenance independently of normalized values.
Settings changes do not silently rewrite previously completed tracks.

## UI and platform acceptance

Follow the existing charcoal/teal workspace and persistent player. Cleanup has
scan/progress, empty, error, stale/conflict, preview, partial-batch result and undo
states; settings and long previews remain scrollable. Expose every operation
through typed Tauri commands. Keep native format coverage: MP3, AAC/ADTS,
AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA.

Prove scalar behavior against source-derived cases and explicit safety cases;
prove multi-value/protected mappings; prove all-format tag changes with unchanged
PCM; prove source/settings/archive races, failed publication/restart and Undo.
Package/test on Windows, Linux and both Mac architectures. Native UI acceptance
and source-helper fixtures are separate evidence.
