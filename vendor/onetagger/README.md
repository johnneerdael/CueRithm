# Selected OneTagger source

Upstream: https://github.com/Marekkon5/onetagger
Base commit: `36523f71f2d9a5947912f3cb930f1a31fcb2e3ee`.
License: GPLv3 (`LICENSE`). Original selected-file hashes: `UPSTREAM.json`.

Only the shared tag types/format helpers, matching/configuration code and
Beatport/Traxsource providers are selected. The Cuerithm adapter does not import
the OneTagger UI, global batch-stop runtime, Shazam, Spotify or executable
post-command behavior. Provider icons are omitted; Cuerithm supplies its own UI.

Local adaptations:

- Standalone workspace/package metadata and Rust formatting.
- Explicit AAC and WMA input types, with distinct ID3/ASF field identities;
  this alone is not a claim of completed physical format writing.
- Injectable loopback HTTP endpoints for deterministic tests; fixed production
  endpoints remain separate from app settings.
- Bounded response bodies, request timeouts and HTTP-status errors; malformed
  Traxsource rows return errors instead of panicking. Missing optional thumbnail
  art remains optional.
- Beatport token expiry is checked before refresh without recursive mutex
  acquisition; token values are not logged.
- Beatport search uses the catalog API and `tracks` response wrapper, adapted
  from [upstream PR526](https://github.com/Marekkon5/onetagger/pull/526), pinned at
  `rosgr100/onetagger@f7adb26a2e0790b0ff751bee04a05c1c20dbd009`.
  The unrelated alternative/fallback matching logic is not imported. Existing
  matching arithmetic remains the pinned base's implementation.
- Native adapter enforces the configured duration filter for fuzzy candidates,
  honors whether matching by embedded ID is enabled, distinguishes failed
  catalogs from genuine empty results and limits Traxsource concurrency to one.
- Original live-network unit check is explicitly ignored; Cuerithm's HTTP
  fixtures and separately invoked live diagnostics identify what was validated.

ASF names follow Microsoft's [music attribute reference](https://learn.microsoft.com/en-us/windows/win32/wmformat/attributes-for-music-files).
The independent analysis engine retains its MIT notices; the combined desktop
and this selected integration are distributed under GPL-3.0-only.


Staged-writing adaptations also include ADTS AAC ID3 support, checked creation
of missing ID3 tags, a valid containing RIFF size for WAV metadata, ID3v2.3
month/day reading, plain ISO MP4 dates, freeform MP4 publish dates, independent
track-total writes and full-precision MP4 BPM text alongside its integer atom.
The pure filename-template parser is selected for cover filenames only; no
file-renaming workflow is imported. Padding and generated cover paths are
bounded. Cuerithm's marker uses its own name and respects the MetaTags selection.

Final integration corrections preserve independent track-number/total policies
for combined ID3 and MP4 representations. ID3 duration is supplied once in native
milliseconds. Beatport advances through configured pages after nonmatches and
collects configured pages for oldest/newest selection. Its shared token refresh
releases the mutex during network I/O, shares failures briefly, and receives the
same cancellation flag as the preparation workers. Existing matching arithmetic
and public catalog endpoints remain unchanged.

ID3v2.3-to-v2.4 upgrades preserve literal slashes in known human-readable text values,
including contributor names such as AC/DC. rust-id3 internal list separators are
normalized before configured catalog writes, retaining frame flags and encoding.

Structured cleanup access (2026-10-18) adds container-directed loading and native
text-value accessors beside the existing configurable separator APIs. ID3 cleanup
manages only the unqualified comment, retaining other comment identities. Ogg
reads inspect the header even when the filename suffix is misleading. Existing
catalog matching and separator settings retain their original path.

ID3v2.3-to-v2.4 upgrades preserve literal slashes in known human-readable text values,
including contributor names such as AC/DC. rust-id3 internal list separators are
normalized before configured catalog writes, retaining frame flags and encoding.
