# Native DJ software synchronization

Implement the user-requested sync targets shown in the supplied Lexicon screenshots:
VirtualDJ, current Engine DJ, current djay Pro, Serato DJ Pro 4 and Pioneer USB.
Keep the existing direct Rekordbox 6/7 contract and do not introduce Rekordbox XML.
Older software versions supported by Lexicon are explicitly out of scope.

Use independent Rust adapters derived from observed format contracts in the installed
Lexicon 1.11.15 bundle. Keep extracted code and identifying artifacts outside tracked
files. Preserve the installed app and source music. Test synthetic library copies.

Each target must support prepared track metadata, whole-track BPM/key, cues/loops,
beatgrids and ordered playlists/folders as supported by that target. Present target
selection and relevant options in the desktop sync workspace. Resolve full, playlist
and modified scopes against current library state. Show discovery/preflight errors,
preview changes, create backups, reject stale previews and recheck target processes
before applying. Report what was actually written and retain restore information.

Target-specific requirements from screenshots: shared field mapping, key conversion
where applicable, nearest supported colors, materialized smartlists, preserve grids;
Engine DJ grid locks, target drive and artwork; Pioneer USB drive selection, cue
destination, unused-file cleanup, CDJ compatibility and extra markers. Device exports
must copy required audio/resources and write native databases/analysis files. A desktop
SQL database alone does not establish a playable Pioneer USB export.

Completion requires Rust/unit integration tests, workspace formatting/clippy/build,
frontend tests/build and rendered verification. Distinguish synthetic format validation
from actual ingestion by installed DJ applications and physical CDJ playback.
