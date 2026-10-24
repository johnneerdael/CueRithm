# Database metadata, file tags and “all tags”

Status: proposed import behavior based on current CueRithm main `190f2f549454e5c6e8486e4949042197351ea14f`. Source evidence is `crates/cuerithm-autotag/src/metadata.rs`, `editing/native.rs`, `transfer.rs`, and the native source-adapter documents. No music tags were changed by this research.

## Three distinct sources

A DJ database's current title/key/BPM/cues can disagree with the tags inside its audio file. A cached tag value in MIK can be older than the file and can differ from MIK's analyzed value. “Import all tags” therefore requires three layers, rather than choosing one string and discarding the others:

1. Native database metadata and analysis, including database-only fields and user edits.
2. Cached tag values explicitly stored by the source application (for example MIK analyzed key versus tag key, float tempo versus integer tag tempo).
3. Current file-container tags and embedded artwork, read without changing the file, when the file is available and the user selects file-tag capture.

Default active-value precedence is selected native library value, then current file tag **only when the native field is absent**, then optional enrichment only when enabled. A conflicting file value must not overwrite a database edit on import. Retain both with source/time/hash and let the user choose another precedence in preview. Empty is not absent. For M3U, the playlist only supplies references and optional display hints; file tags are needed for real track metadata. Missing files still import source metadata and memberships.

Importing file tags is a **read**, distinct from retagging (a lookup/change proposal) and writing tags (an explicit mutation). It requires no key/BPM/energy DSP. Current `read_metadata` reads a selected set of human-readable fields; `read_transfer_metadata` adds key/energy/color/extras/BPM/date/rating/track number. Neither should be described as a lossless inventory of every arbitrary frame/atom/chunk. The implementation needs a raw metadata envelope alongside normalized fields when “all tags” preservation is promised.

## Existing read mappings to reuse

**Seen:** `metadata.rs:41-68` and `editing/native.rs:12-43` map the following normalized fields; inspect current main before implementation. Values retain contributor boundaries. Do not split an artist on a comma simply because it contains one.

| Field | ID3 | Vorbis comments | MP4 native / logical binding | ASF |
| --- | --- | --- | --- | --- |
| Title | TIT2 | TITLE | ©nam | Title |
| Artist | TPE1 | ARTIST | ©ART | WM/Artist (Author fallback) |
| Album | TALB | ALBUM | ©alb | WM/AlbumTitle |
| Album artist | TPE2 | ALBUMARTIST | aART | WM/AlbumArtist |
| Genre | TCON | GENRE | ©gen | WM/Genre |
| Comment | COMM | COMMENT | ©cmt | Description |
| Label | TPUB | LABEL / PUBLISHER fallback | LABEL | WM/Publisher |
| Remixer | TPE4 | REMIXER | REMIXER | WM/ModifiedBy |
| Mix/version | TIT3 | SUBTITLE / VERSION fallback | desc | WM/SubTitle |
| Composer | TCOM | COMPOSER | ©wrt | WM/Composer |
| Lyricist | TEXT | LYRICIST | LYRICIST | WM/Writer |
| Grouping | TIT1 | GROUPING | ©grp | WM/ContentGroupDescription |
| Key | TKEY | INITIALKEY | INITIALKEY logical binding | WM/InitialKey |

**Seen:** `editing/native.rs:64-84,132-155` handles release/original dates and track/disc totals; MP4 tuple fields `trkn`/`disk` are different from scalar strings. `transfer.rs:112-119` prefers fractional MP4 freeform `----:com.apple.iTunes:BPM` before generic BPM. Keep that fractional value even if another field is integer. These are CueRithm binding names, not a claim each is an on-disk MP4 four-character atom. The underlying tag library resolves logical/custom names; do not serialize raw MP4 boxes by copying this table.

A raw envelope must include native identifiers (ID3 frame ID plus COMM language/description, TXXX description, UFID/PRIV owner; MP4 item/freeform namespace; Vorbis key + repeated order; ASF typed attribute identity), data type, multiplicity/order, raw bytes or a local binary attachment reference, and normalized interpretation if recognized. Keep binary data out of ordinary text columns. Preserve encoding/version flags when exact re-emission is a requirement. Deduplicate attachment storage by hash, not tag identity. Do not write a source tag merely to normalize it.

For artwork, preserve MIME/type/description/order/bytes separately; external artwork paths are references with availability status. Current `has_embedded_artwork` only answers presence and is not an image extractor. Source artwork caches may be optional and must not be confused with the user's embedded cover.

## DJ-specific binary tags

Serato library rows alone may not contain cue/grid payloads; read the relevant music tag blocks for the source profile. The Lexicon format document specifies `Serato Markers2`, `Serato BeatGrid` and container encodings. Its inspected importer has no legacy `Serato Markers_` fallback; preserve that legacy payload as opaque with an explicit unsupported-performance-data issue rather than claiming cue recovery. Decode on private bytes using bounded parsers. Keep unrelated tag blocks unchanged and store their raw payload/provenance. Container wrappers differ for MP3/AIFF/WAV ID3, FLAC/Vorbis, MP4 and ASF; a base64 payload in one container is not automatically the byte layout in another.

CueRithm's existing outbound Serato4 code already stages audio-tag changes; inbound extraction must call the read/parse side only. Never run its `write_staged` path during import. Source-app format support is independent of CueRithm decode support: when a source never stored that payload for a container, import what exists and report unavailable fields.

## Completion criteria

Test read-only extraction across MP3, AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA. Include multiple comments, languages, repeated artists, unknown binary frames, custom key tokens, fractional BPM, Unicode and artwork; conflicting DB/cache/file values; missing media; malformed payloads and byte limits. Hash the source before/after. The raw layer's preservation test must enumerate every captured native tag and attachment, not merely assert normalized title/key equals the expected text.

When a container parser cannot enumerate some tag kind, show the exact unsupported kind/field in the loss report. Do not claim “all tags” based on a normalized transfer snapshot or silently run a retag lookup to replace unknown source metadata.
