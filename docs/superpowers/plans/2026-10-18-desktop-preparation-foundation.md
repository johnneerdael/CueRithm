# Desktop Preparation Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a usable Tauri desktop library with automatic full analysis,
replacement cues and Incoming-to-Tracks promotion for all common audio formats.

**Architecture:** Keep the existing analysis engine independent of Tauri. Add a
shared media boundary and a Rust library/job service; React consumes typed
commands/events. Preserve the validated FLAC analysis path while generalizing
the PCM inputs needed by timing and energy.

**Tech Stack:** Rust (existing workspace floor 1.89), Tauri 2, React/TypeScript,
Vite, SQLite through rusqlite, notify for watcher hints, bundled FFmpeg/ffprobe
under Rust orchestration, native Rust playback, frontend component/browser tests.

**Spec:** `docs/superpowers/specs/2026-10-18-cuerithm-library-workspace-design.md`

## Global Constraints

- Tauri 2, React/TypeScript and Rust; macOS, Windows and Linux.
- MP3, raw AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and
  WMA are required in the first usable delivery. No FLAC-only delivery gate.
- Full analysis, key, BPM, beatgrid, structural cues and one energy rating per track.
- Replace existing cues only after a complete validated replacement is ready.
- Apply every enabled required preparation stage before automatic promotion.
- All preparation options persist in Settings; no per-import configuration wizard.
- No XML transport or fallback in the desktop app.
- Preserve the validated Lexicon DSP, existing FLAC results and private research.
- User correction on 2026-10-18: desktop key, energy and cues are built in. Embed
  the pinned key/energy parameters; remove model-path setup; preserve watch/cue
  preferences and resume unfinished setup jobs. This supersedes external-model
  setup instructions below; the independent CLI retains explicit model APIs.
- Users do not install FFmpeg, Python or OneTagger to run the application.
- Existing engine remains usable independently of desktop/system GUI dependencies.

## Review Focus

1. Codec padding, sample-rate conversion and float/high-depth audio must not make
   displayed/playback/cue/grid timelines disagree: Task 1 and Task 2 fixtures.
2. Watcher storms, file copies still in progress, disconnected folders and
   self-generated writes must not produce duplicate or premature jobs: Task 4.
3. A crash during replacement must preserve the previous valid cues and recover
   work without marking an incomplete track ready: Tasks 3 and 4.
4. Missing models, malformed audio, unsupported/protected media and unknown
   analysis results must be actionable without invented key/BPM/energy: Tasks 2/6.
5. Selection, playback, background events and paginated sorting must not steal
   focus, play the wrong file or lose a completed track: Tasks 5 and 6.

## Scope and subsequent deliveries

The approved specification covers several subsystems. This first plan produces
working library/preparation software; it does not claim the complete application
is delivered before tagging, cleanup, playlists and direct sync are implemented.

Subsequent plans, in order, cover: (1) adapting Beatport/Traxsource tagging and
all applicable persistent settings/file writes; (2) the eleven Smart Fixes plus
genre/artist cleanup and mappings; (3) ordinary playlists/tools and all 58 smart
operators with a parity matrix; (4) direct rekordbox 6/7 import/export and actual
round-trip acceptance; (5) final integration, large-library acceptance and release
packaging. These retain their full approved scope. Task 7 starts direct-sync
schema/fixture work early; it does not substitute read-only inspection for sync.

No stage may be marked implemented because a placeholder returns success.
Tagging/cleanup options remain disabled and explicitly unavailable until their
actual adapters ship; if imported settings enable an unavailable required stage,
the job stays in Incoming with an actionable reason.

## File and responsibility map

| Location | Responsibility |
| --- | --- |
| `crates/cuerithm-media/src/{lib,probe,decode,clock,runtime}.rs` | Bundled decoder, bounded PCM assets, source/playable clocks |
| `crates/camelot-source-analysis/src/{lib,timing,pcm}.rs` | Checked PCM entry points for retained timing/energy arithmetic |
| `crates/camelot-core/src/{engine,prepared}.rs` | Complete prepared-media analysis, retaining existing file API |
| `crates/cuerithm-library/src/{lib,model,store,settings,jobs,watch,prepare}.rs` | Library, settings revisions, durable jobs and preparation |
| `crates/cuerithm-library/migrations/001_initial.sql` | First schema, indexes and migration identity |
| `crates/cuerithm-library/src/{playback,waveform}.rs` | Native playback and bounded waveform data |
| `apps/desktop/src-tauri/` | Tauri host, typed commands/events, app-data/resource paths |
| `apps/desktop/src/` | React workspace, table, Incoming, player and Settings |
| `tools/media-runtime/` | Pinned native-decoder build recipe, artifacts and checksums |
| `tools/fixtures/` | Original generated codec/reference fixture recipes |
| `.analysis/rekordbox-fixtures/` | Private copied target databases and analysis files |
| `docs/research/rekordbox-direct-contract.md` | Sanitized schema/asset/ownership findings |

Use exact locked dependency versions selected against these APIs during each
task. Check Rust 1.89 compatibility before adding dependencies; do not silently
raise the workspace floor. Do not copy local Homebrew-linked binaries into
distribution artifacts. Native decoder build/source provenance and redistribution
notices travel with every packaged artifact.

---

### Task 1: Common-format decoding and bundled runtime

**Files:** Create the `cuerithm-media` crate above, `tests/decoding.rs`,
`tools/media-runtime/{manifest.json,build.sh,build-windows.ps1}`,
`tools/fixtures/{generate_audio.py,media-cases.json}`; modify root `Cargo.toml`.

**Interfaces:** Produce `MediaRuntime::from_bundle(dir: &Path) -> Result<Self>`,
`MediaRuntime::probe(path: &Path) -> Result<MediaInfo>`, and
`MediaRuntime::decode(path: &Path, cache: &Path) -> Result<PreparedAudio>`.
`PreparedAudio` owns a private immutable source snapshot and a 44100 Hz stereo
float PCM cache, plus source codec/rate/channels, decoded frame count, decoder
identity and `AudioClock`. `AudioClock` records playable-time mapping and known
padding/edit-list handling; absent gapless metadata is unknown, not zero-padding
evidence. Expose bounded sample reading; do not send entire tracks through IPC.

- [ ] **Step 1 — Write failing codec/clock tests.** Generate original fixtures
  for every required codec/container above, mono/stereo, 44100/48000/96000 Hz,
  16/24-bit and float WAV, filenames containing spaces/Unicode/leading hyphens,
  wrong extensions, truncated streams and unavailable runtimes. Assert lossless
  equal-PCM cases decode identically; lossy cases match independently stored
  decoded-frame/time expectations rather than the source waveform. Include MP3
  VBR/gapless, M4A edit lists, Opus pre-skip and raw ADTS without gapless metadata.
  Commit the small original generated fixtures with hashes and encoder recipes;
  decoder-only application/CI runtimes need not contain those fixture encoders.
- [ ] **Step 2 — Run** `cargo test -p cuerithm-media --test decoding`; verify the
  new functionality fails before implementation, rather than skipping missing codecs.
- [ ] **Step 3 — Implement** the interfaces. Invoke bundled binaries with argument
  arrays, local immutable inputs and explicit first-audio-stream selection.
  Decode the complete playable stream; disable ReplayGain/loudness processing.
  Count actual decoded frames, bound disk/memory and clean interrupted caches.
  Persist the resampling/channel-map recipe in identity. Build pinned FFmpeg/
  ffprobe artifacts for macOS arm64/x86_64, Windows x86_64 and Linux x86_64;
  require every listed decoder and reject non-system dynamic dependencies.
  Verify artifact hashes; never silently download a runtime when processing music.
- [ ] **Step 4 — Rerun** the decoder suite on all three OS families using packaged
  runtimes. Every required format must pass; missing codec/runtime is a failure.
- [ ] **Step 5 — Commit** as `feat(media): decode common formats with a bundled runtime`.

### Task 2: Full analysis across those decoded formats

**Files:** Create `camelot-source-analysis/src/pcm.rs`,
`camelot-core/src/prepared.rs`, `camelot-core/tests/prepared_formats.rs`; modify
the source-analysis exports/timing wrapper, engine and source fingerprint list.

**Interfaces:** Consume Task 1 `PreparedAudio`. Produce
`Engine::analyze_prepared(&mut self, audio: &PreparedAudio) -> Result<FileAnalysis>`.
Add checked source-analysis `analyze_timing_pcm(profile: &PcmProfile) -> Result<TimingAnalysis>`
and `analyze_energy_pcm(profile: &PcmProfile) -> Result<Vec<EnergySegment>>`.
`PcmProfile` explicitly carries normalized signed16 stereo samples, full frame
count and analyzed-prefix boundary; rate is fixed at 44100 Hz. It is an internal
feature input, not a new per-segment product output.

- [ ] **Step 1 — Write failing integration tests.** For matched 44100 stereo16
  FLAC, compare the entire key/tempo/energy/structural-cue result with existing
  `analyze_file`, excluding identities that intentionally record the new decoder.
  Require identical lossless equal-PCM results across containers. Require a
  complete full analysis with finite bounded cues and one energy rating for
  all lossy/common-format fixtures; never assert lossy audio equals its master.
  Include high-depth PCM, negative beat anchors, silence, short files, non-finite
  float input, changed sources and absent/corrupt key/energy models.
- [ ] **Step 2 — Run** `cargo test -p camelot-core --all-features --test prepared_formats --release`;
  confirm the missing prepared-input path fails.
- [ ] **Step 3 — Implement** the new facade without changing Lexicon DSP or the
  old file/CLI contract. Preserve the validated FLAC timing/energy prefix policy
  on matching source profiles. For other formats use a documented canonical
  profile, with explicit deterministic quantization only for signed16 feature
  inputs; key/cue inputs retain float precision. Preserve amplitude and decoder
  clock. Include recipe/decoder/model identities in results. Keep full read,
  algorithmic prefix and checksum verification distinct; do not label a codec
  with no checksum as FLAC-verified. No catalog tags are analysis predictions.
- [ ] **Step 4 — Run** the new suite, existing workspace release tests and the
  explicit local model checks. Use the fixed development probes for real-music
  regressions; preserve reserved holdouts. Report numerical differences honestly.
- [ ] **Step 5 — Commit** as `feat(engine): analyze prepared audio across common formats`.

### Task 3: Persistent library, settings and job contracts

**Files:** Create `cuerithm-library/{Cargo.toml,migrations/001_initial.sql}`,
`src/{lib,model,store,settings,jobs}.rs` and `tests/{storage,jobs}.rs`; modify
workspace members.

**Interfaces:** Produce `Library::open(path: &Path) -> Result<Library>`,
`Library::enqueue(paths: &[PathBuf], revision: SettingsRevision) -> Result<Vec<JobId>>`,
`Library::tracks(query: TrackQuery) -> Result<TrackPage>`,
`Library::jobs() -> Result<Vec<JobSummary>>`,
`Library::save_settings(settings: PreparationSettings) -> Result<SettingsRevision>`.
Use stable opaque `TrackId`/`JobId`; UI commands operate on IDs. Settings hold
watch folders, model paths/identities and stage policies; defaults require key,
timing, energy and replacement cues. Desktop installs built-in models automatically.

- [ ] **Step 1 — Write failing storage tests.** Reopen after writes; validate
  migrations, cursor sorting/search, duplicate path events, settings snapshots,
  source replacement, previous cue revisions and interrupted running jobs.
  Assert success requires every enabled required stage. Invalid settings or a
  pending provider stage cannot become ready. Keep analysis provenance separate
  from provider metadata and sync IDs. Include Unicode paths and future-schema
  rejection without destructive recreation.
- [ ] **Step 2 — Run** `cargo test -p cuerithm-library --test storage --test jobs`;
  verify failures correspond to the missing contracts.
- [ ] **Step 3 — Implement** versioned SQLite storage and typed job transitions.
  Persist source identities, preparation revisions, analysis, cue/grid revisions,
  undo, archive state and future external-ID ownership separately. Store file
  paths losslessly for each platform; return display text separately. Use
  parameterized queries and transactions; keep migrations repeatable and bounded.
  Do not create a fake tagger or a fake direct-sync success path.
- [ ] **Step 4 — Rerun** the storage/job suites, including reopen/recovery cases.
- [ ] **Step 5 — Commit** as `feat(library): persist tracks settings and preparation jobs`.

### Task 4: Watch folders and durable automatic preparation

**Files:** Create `cuerithm-library/src/{watch,prepare}.rs`,
`tests/{watch,preparation}.rs`.

**Interfaces:** Consume Tasks 1–3. Produce
`PreparationService::start(library: Library, runtime: MediaRuntime) -> Result<Self>`,
`PreparationService::retry(job: JobId, revision: Option<SettingsRevision>) -> Result<()>`
and `PreparationEvent { sequence, job_id, track_id, stage, state }`.
Use one analysis worker initially; bounded queues and cancellation checkpoints
prevent UI blocking. Settings revisions are immutable for a running job.

- [ ] **Step 1 — Write failing end-to-end tests.** Copy a file in chunks, emit
  repeated/out-of-order watcher hints, restart between stages and reprepare a
  track with existing cues. Assert one job per unchanged source/revision; old
  cues survive failures; a successful validated replacement updates cues and
  promotes once. Cover all required formats through this path, missing folders,
  interrupted decoders, unavailable required stages and source mutation.
- [ ] **Step 2 — Run** `cargo test -p cuerithm-library --test watch --test preparation --release`.
- [ ] **Step 3 — Implement** stable-file checks, startup reconciliation and
  retryable durable stages. Watcher events are hints, not completion evidence.
  Analyze immutable snapshots with actual configured models, then atomically
  replace the library cue revision and mark ready only when required work
  succeeds. Record unknown/silent results as attention states, not invented
  music metadata. Promotion changes library state without moving the original
  file by default. Suppress the application's own write/reimport loop.
- [ ] **Step 4 — Rerun** those suites and crash/reopen cases. Inspect actual
  generated-file results after all workers terminate; do not substitute fixtures
  or filename tags as predictions.
- [ ] **Step 5 — Commit** as `feat(preparation): process incoming tracks automatically`.

### Task 5: Playback and waveform on the same timeline

**Files:** Create `cuerithm-library/src/{playback,waveform}.rs`,
`tests/{playback,waveform}.rs`.

**Interfaces:** Consume Task 1 PCM/clock and Task 3 IDs. Produce
`Player::load(track: TrackId, audio: PreparedAudio) -> Result<()>`,
`Player::{play,pause}(&mut self) -> Result<()>`,
`Player::seek(&mut self, seconds: f64) -> Result<()>`,
`Player::status(&self) -> PlaybackStatus`, and
`waveform(audio: &PreparedAudio, bins: usize) -> Result<Waveform>`.
Playback runs in Rust with bounded buffers; expose actual position/events and
min/max waveform bins, not whole-file base64 payloads.

- [ ] **Step 1 — Write failing clock/seek tests.** Test play/pause/seek/load
  transitions, EOF, device failure, invalid seeks and selecting a different row
  while the loaded track continues. For each format assert waveform duration,
  play position, cue positions and beat lines use the same playable clock.
- [ ] **Step 2 — Run** `cargo test -p cuerithm-library --test playback --test waveform`.
- [ ] **Step 3 — Implement** native playback and cached waveform summaries.
  Separate output-device interaction from the transport state for meaningful
  headless tests. Do not infer bar numbers from an unverified downbeat anchor.
- [ ] **Step 4 — Rerun** the tests and manually audition generated codec fixtures
  in the native app; record seek/cue alignment, not only sound-device creation.
- [ ] **Step 5 — Commit** as `feat(player): share playback and cue timelines across formats`.

### Task 6: Tauri workspace and persistent Settings

**Files:** Create `apps/desktop/{package.json,pnpm-lock.yaml,index.html,vite.config.ts,tsconfig.json}`,
`src/{App.tsx,api.ts,types.ts,styles.css}`, `src/components/{Sidebar,TrackTable,PlayerPanel}.tsx`,
`src/views/{Tracks,Incoming,Settings}.tsx`, `src-tauri/{Cargo.toml,build.rs,tauri.conf.json}`,
`src-tauri/src/{lib,main,commands}.rs`, `src-tauri/capabilities/main.json` and tests.

**Interfaces:** Rust commands: `list_tracks(query)`, `list_jobs()`,
`enqueue_paths(paths)`, `retry_job(job_id)`, `get_settings()`,
`save_settings(settings)`, `get_waveform(track_id,bins)`,
`player_load(track_id)`, `player_play()`, `player_pause()`, `player_seek(seconds)`.
Use serialized Result DTOs with stable error codes; emit sequenced preparation
and playback events and reconcile against persisted state after reconnect.

- [ ] **Step 1 — Write failing UI/bridge tests.** A real event sequence moves
  an Incoming row to Tracks only on committed completion. Required errors stay
  actionable. Settings survive restart. Sorting/pagination preserve selection;
  row selection does not change the playing track. Test blank/populated/loading/
  failed states, keyboard focus and resizable panels with a large table fixture.
- [ ] **Step 2 — Run** `pnpm --dir apps/desktop test`; confirm the missing views/
  contracts fail. Use native bridge tests for command serialization as well.
- [ ] **Step 3 — Implement** the approved sidebar/table/player shell and Settings
  using `#181818`, `#202020`, `#00D2BF`, `#106c61`. Use React with table
  virtualization; keep durable state in Rust. Resolve app-data and bundled
  decoder paths in the host. Route folder selection/drop to queue enrollment,
  not a tagging wizard. Embed and automatically select key/energy parameters;
  report installation failures without a model-path setup flow. Never expose arbitrary command execution or a generic filesystem
  bridge to the frontend. Do not add OneTagger's unrelated tool pages.
- [ ] **Step 4 — Run** UI tests, TypeScript checks and a frontend production build;
  launch the actual Tauri app with real generated files of every required format.
  Inspect screenshots of all meaningful states and verify complete preparation,
  cue replacement, playback and restart recovery. Browser mocks alone do not
  establish native IPC or playback success.
- [ ] **Step 5 — Commit** as `feat(desktop): add the automatic preparation workspace`.

### Task 7: Early direct-sync fixture and schema contract

**Files:** Create `docs/research/rekordbox-direct-contract.md`;
store extracted/copied evidence under `.analysis/rekordbox-fixtures/` only.
Add original public synthetic schema/asset test recipes under
`tools/fixtures/rekordbox/` without private tracks or proprietary code.

**Interfaces:** Produce a documented versioned mapping from library track/cue/
grid/playlist identities to rekordbox 6/7 records and associated analysis assets.
Record external IDs, change sequence rules, conflict baselines and native-smart
rule limitations. These constrain later storage migrations and the writer plan.

- [ ] **Step 1 — Define failing contract checks** for duplicate external IDs,
  missing analysis assets, unsupported schema revisions and inconsistent
  track/cue/grid timelines on copied fixtures. State expected outcomes before probing.
- [ ] **Step 2 — Run** those checks against synthetic invalid copies; confirm
  each check detects its intended failure.
- [ ] **Step 3 — Inspect** licensed Lexicon behavior and available copied
  rekordbox versions using leanrea. Keep all source databases read-only and
  private; do not publish encryption material or decompiled code. Record which
  requirements are seen, inferred or still unverified. Do not add an XML branch.
- [ ] **Step 4 — Validate** the mapping against actual version identities where
  fixtures exist; otherwise record the precise missing acceptance evidence.
  A later writer task still requires real target-app round trips.
- [ ] **Step 5 — Commit** sanitized findings as `docs(sync): define the direct rekordbox contract`.

### Task 8: Cross-platform application verification and delivery

**Files:** Modify `.github/workflows/ci.yml`, `README.md`, `.gitignore` and
`THIRD_PARTY_NOTICES.md`; create `.github/workflows/desktop.yml` and
`docs/development/desktop.md`.

**Interfaces:** Produce reproducible developer/build commands and unsigned
test artifacts for macOS, Windows and Linux, including the pinned decoder.

- [ ] **Step 1 — Define acceptance failures.** Missing packaged codecs, absent
  runtime files, nonportable dynamic links, frontend type errors, engine
  regressions and failed restart/preparation cases must fail the relevant job.
  Never skip format checks because a CI machine lacks a decoder.
- [ ] **Step 2 — Run** the acceptance commands against deliberately missing
  runtime resources and a clean build directory; verify missing resources fail.
- [ ] **Step 3 — Configure** separate headless engine/library and desktop jobs.
  Keep engine jobs independent of GUI system dependencies. Desktop jobs install
  Tauri platform prerequisites, build the frontend, package pinned runtimes and
  run Rust/UI/format checks. Lock frontend dependencies and pin action revisions.
  Include the Rust playback audio-device development dependencies on Linux.
- [ ] **Step 4 — Run** formatting, Clippy with warnings denied, workspace release
  tests excluding the native host where appropriate, explicit model checks,
  frontend tests/typecheck/build and native desktop builds. Require all three
  OS jobs green. Verify installed-resource resolution in an actual app artifact;
  distinguish unsigned test builds from signed/notarized public releases.
- [ ] **Step 5 — Commit and push** the reviewed delivery to Cuerithm after actual
  validation, preserving unrelated work. Document subsequent approved subsystems
  as pending rather than declaring the complete product delivered.

## Self-review and planning references

This plan covers the first specification delivery plus early direct-sync
contract research. It deliberately leaves subsequent tagging, cleanup,
playlist/parity and direct-writer implementation for their dedicated plans;
it does not reduce those requirements. All five Review Focus items have owning
tests above. New APIs are defined once and consumed by name in dependent tasks.

The local decoder discovery used generated audio, not the private music corpus.
Every requested format decoded locally; raw AAC/Vorbis/WMA demonstrated that
encoded and decoded durations can differ. This is evidence for clock fixtures,
not packaged-runtime, engine-accuracy or rekordbox parity acceptance.

Primary references: [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/),
[Rust commands](https://v2.tauri.app/develop/calling-rust/),
[bundled binaries](https://v2.tauri.app/develop/sidecar/),
[rusqlite](https://docs.rs/rusqlite/latest/rusqlite/),
[FFmpeg decoding](https://ffmpeg.org/ffmpeg.html).
