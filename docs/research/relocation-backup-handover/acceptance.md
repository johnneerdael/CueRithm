# Engineering acceptance matrix

These are required future tests, not results of this research. Create synthetic
fixtures from the documented schema; do not check in user music, archive rows,
extracted vendor code, licensed binaries or private paths. The supplied ZIP is a
private integration sample only. Test data labels below are invented.

## Relocation vectors

| Case | Input/action | Required outcome |
|---|---|---|
| Root move | `D:/Music/House/A.mp3` → mapped `E:/Music/House/A.mp3` | Same track ID, metadata/cues/tags/member order; file untouched. |
| Cross-OS root | `D:/Music/House/A.mp3` mapped to POSIX `/music/House/A.mp3` | Source syntax parsed independently; playable local binding after verification. |
| Reparenting | Old `/old/House/A.mp3`, candidate `/new/House/A.mp3` and unrelated `/new/Other/A.mp3` | Parent+filename tier wins; explain evidence. |
| Suffix compatibility | Old `A_pn.mp3`, candidate `A.mp3` | NFC/suffix signatures match; original paths never renamed. |
| Filename only | Parent changes, exactly one exact filename | Filename match shown; confirm preserves ID. |
| Duplicate names | Two candidates, only one saved-size match | Size-assisted candidate shown and preselected. |
| Unresolved duplicates | Two candidates, both/neither size match | Ambiguous, none selected; no traversal-order choice. |
| Sole altered file | One filename match, size or known hash differs | Explicit replacement selection; DSP stale, saved cues retained. |
| Same-size content mismatch | Same name/size but known SHA-256 differs | Hash disagreement vetoes preselection. |
| Extension/hash overlap | Extension changed but encoded hash agrees | Explicit replacement approval still required. |
| Case | Linux `a.mp3` and `A.mp3`; restore to case-insensitive volume | Distinct source records preserved; destination collision blocks binding. |
| Unicode | NFC/NFD filename equivalents | Match suggestions normalize signatures; binding identity follows filesystem; no lossy rename. |
| Boundary | Map `/music` with `/music2/A.mp3`; map `D:/Music` with `D:/Music2/A.mp3` | No root match. |
| Extension | Source WAV → chosen MP3 extension | Last extension only; verify decoder; explicit timing-inspection notice. |
| Missing target | Root map points to absent target | No “relocated” success; can retain unresolved map explicitly. |
| Source available | Mix missing/present selection; All utility | Selection skips present, All permits reviewed mapping. |
| Archived/Incoming | Explicitly select each | Preserves flags; Incoming does not auto-prepare. |
| Unavailable drive | Parent missing or permission denied | Unavailable, separate from definitely missing; partial search recorded. |
| Source/target survivor | Destination already bound to B; choose A or B | Selected whole record survives; other state retained for undo; playlists rewired. |
| Playlist collapse | Both colliding records occur in same playlist | Survivor position retained, losing occurrence removed and reported. |
| Many-to-one | Two selected rows claim same destination | Group conflict; no unique-path violation or silent data loss. |
| Hardlink/alias | Candidate is a hardlink/case alias of occupied file | Occupied identity conflict, no second automatic claim. |
| Symlink/junction | Directory loop or root escape | Bounded traversal, no following by default, skipped reason. |
| Stale file/entity | Change candidate or cue after preview | Apply rejects selected set atomically. |
| Cancel | Cancel during traversal, hash, precommit, postcommit | No edit before commit; after commit report completed. |
| Undo | Restart then undo; separately mutate survivor or occupy old path | First succeeds atomically; changed state returns conflict. |
| Probe | Each required audio format and invalid renamed audio | Supported files connect; invalid audio rejected without losing entry. |

## Portable backup and restoration

| Case | Required outcome |
|---|---|
| WAL snapshot | Committed uncheckpointed rows included; no partial transaction. |
| Concurrent writers | Consistent snapshot; gate drains writers; compression outside active DB lock. |
| Full domain round trip | Track IDs, archived/Incoming flags, text lists, attributes, all cue/musical history and selected revision, real analysis provenance, tree/favorites/tombstones/tags/preferences preserved. |
| Equal cue timestamps | Selected revision survives serialization; row order cannot change playback selection. |
| Lightweight boundary | No audio/PCM/artwork/waveform, jobs, runnable journals, undo or credentials; report all exclusions. |
| Export without media | Disconnected audio does not block backup. |
| Active filesystem journal | Export rejects with recovery action; cannot create an apparently complete snapshot. |
| Destination exists/full/permission | Existing archive and active library intact; task temp cleanup only. |
| Empty/minimal library | Valid round trip, all required empty streams, exactly one selected settings/preferences record. |
| Native bindings | Historical remote IDs/baselines retained inactive; no external write or false acknowledged sync. |
| Watcher regression | Restored ready/no-job track is not enqueued; explicit Prepare still works. |
| Imported player/smartlists | Accepted cue/timing/key/BPM/energy available with null DSP analysis. |
| Schema migration | Legacy library adoption preserves original; new portable schema independent of raw SQLite rows. |
| Corruption/future version | Bad hash/CRC/magic/count/reference or unsupported required feature fails before activation. |
| Hostile container | Traversal, absolute/UNC name, duplicate/case alias, symlink, overlap, oversized actual data, huge JSON/deep object fail boundedly. |
| Opaque atoms | Base64 POSIX/UTF-16 dot-dot, NUL, separator or absolute atoms | Retained inert/unresolved or rejected; never joined to a destination root. |
| Binding root race | Replace a destination parent with symlink/junction after preview | No-follow containment proof rejects Apply. |
| Activation fault points | Inject failure before/after each journal phase, flush, pointer replace and reopen; restart yields old or validated new library, never mixture. |
| Windows handles | Open handle/antivirus-style replacement denial leaves old active; recoverable diagnostic. |
| Staging seal | WAL-only committed staging rows, then mutate staging after validation | Checkpointed seal includes committed rows; later mutation invalidates activation. |
| Busy services/player | Failure to drain/close prevents switch; no stale Host/worker handle reads old DB after success. |
| Local rollback | Retained generation can reactivate; later edits are not merged implicitly. |

Run the **nine source→destination OS combinations** for macOS, Windows and Linux,
including same-OS another-computer restore. Use Windows drive + UNC, macOS volume,
Linux case-sensitive, Unicode, POSIX opaque-byte and UTF-16 opaque-code-unit cases.
For unrepresentable names, metadata/playlist restoration succeeds with unresolved
paths and an explicit mapping report. Verify playback, not merely DB row counts,
after binding supported fixtures. Use at least one file of every required format.

## Lexicon profile

| Case | Required outcome |
|---|---|
| Selected real sample | Profile 26; 894 tracks, 5,388 cues, 894 markers, 1,143 playlists, 434 ordinary/source links, 4 categories, 76 tags; no private content in logs. |
| Smart sample | 819 smartlists supported by six documented operators; expected membership compared to independent hand-calculated synthetic predicates, not vendor execution claims. |
| Signed timing | Negative cues retained; seek clamps only at playback; no global shift. |
| Fractional duration | REAL values accepted despite INTEGER column declaration. |
| All scalar fields | Every Track column mapped or preserved/reported; no silent attr loss. |
| Multi-tempo/loops | Synthetic multiple anchors and cue kinds 1–5 retained; no first-BPM flattening; invalid loop inert/report. |
| Memory marker | `originalNum=-1` maps to explicit memory cue and source object preserved. |
| Empty/duplicate memberships | Empty tree nodes retained; duplicates collapse first occurrence with warning. |
| Taxonomy | Nonempty assignments, duplicate display labels, ordering/color/shortcut retained by IDs. |
| Key rules | Camelot/Open Key/musical notation and absent harmonic preference; unknown key false/report. |
| Grouping | Any, all, zero rules and multi-OR group exercise exact truth table. |
| Unsupported definition | Whole smartlist disabled, raw JSON preserved, no broadened rule evaluation. |
| Source IDs | Opaque Database.uuid accepted as bounded text; deterministic namespaces; same backup replacement retains IDs. |
| Version/schema | Future/older version, missing tables, wrong storage types, view masquerade, corrupt DB, malicious trigger rejected. |
| Normal source views/FTS | Six known view/trigger fingerprints and known FTS table fingerprints | Accepted as inert; a malicious same-name object is rejected. |
| Secrets/settings | Excluded by key allowlist, never dumped or activated. |
| Streaming/Windows paths | Synthetic references preserved offline, root mapping never host-parses foreign syntax. |
| No media writes | File hashes/mtimes of controlled fixture media unchanged across preview/import/activate. |
| Repeat/cancel/fail | No accumulating duplicate import; current library retained if stage fails or is cancelled. |

## Scale and release evidence

Measure 100,000 tracks, 10,000 playlists and one million candidate paths on a
documented machine. Initial engineering budgets: peak incremental RSS below 512
MiB (excluding existing library/UI), progress gaps under one second, local cancel
acknowledgement under two seconds when OS I/O returns, and no long DB write lock
during scanning/compression. OS-blocked I/O has no promised hard latency; show the
waiting phase and keep the old library active. Record wall time/archive size and
peak temporary disk; do not promise a fixed backup duration from this research.

Before shipping run the workspace checks required by AGENTS.md and desktop
typechecking/tests/build. Record platforms, commit, fixture versions, archive
checksums, actual passes/failures and remaining unsupported profiles. A passing
parser test alone is not cross-machine restoration acceptance.
