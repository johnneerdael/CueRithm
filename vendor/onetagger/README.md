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
