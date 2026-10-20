# DJ sync timing contract

Status: researched and implemented on the DJ sync worktree, 2026-10-20; final full verification and integration remain pending. This document records independent format observations and aggregate synthetic results. It contains no extracted implementation or music.

## Finding

Cue and grid serialization requires a target-specific timeline conversion before converting seconds to milliseconds, samples or native floating-point fields. The sync projection keeps saved `Publication` seconds canonical in `SyncTrack`; the native export boundary applies the measured conversion exactly once to an export copy before adapter serialization. Correct binary containers and an original-reader round trip therefore do not establish audio alignment.

The observed reference applies the same additive offset to cue starts, loop ends and tempo-marker starts before native serialization. For newly analyzed local MP3 files, the offset is nonzero for Serato and Pioneer USB. For primed AAC/M4A files it can also be nonzero for Serato. Applying those offsets to stored publications would be incorrect: publications, waveform positions and playback must retain their shared playable PCM clock. Apply the conversion only to a fresh export copy.

These are the reference application's calibrated export rules. They are not universal statements about an MP3 encoder's delay, and the fixed millisecond constants must not be replaced with an assumed sample count.

## Evidence boundary

The authority is the hash-matched shipped Lexicon 1.11.15 renderer, SHA-256 `032bb911711c2ed0da24ce67fad2187fa456faf58ec65f0379c03fb9ef1397e3`.

| Observation | Renderer evidence |
| --- | --- |
| Shared timeline conversion and import normalization | Module 91079, lines 325938–326259 |
| MP3 classifier and M4A/MP4 measurement | Module 55526, lines 325634–325906 |
| Generated/local cue classification | `Track.hasLexiconCuepoints`, lines 311382–311389 |
| Import sequencing | Lines 266808–266837 |
| Serato conversion, also used by the Serato 4 writer | Lines 251667–251689 |
| Engine DJ conversion | Lines 359843–359852 |
| VirtualDJ conversion | Lines 363015–363020 |
| djay conversion | Lines 365275–365291 |
| Pioneer USB conversion | Lines 246087–246093 |
| Serato M4A timeline migration, confirming the priming distinction | Lines 532220–532252 |

Private excerpts and probes are retained in ignored analysis storage. The actual shipped timing functions and scanner were executed with generated silence and synthetic header variants. Ninety-four contract assertions passed. Only unrelated module dependencies, logging and assertions were supplied by the probe harness; the timing functions and scanner were not replaced with no-ops.

This establishes the observed transformation contract. It does not establish waveform/cue alignment in a running native DJ application, nor equivalence between every reference decoder and Cuerithm's decoder. Controlled native-application playback remains a separate acceptance layer.

**Unknown:** whether the reference decoder's calibration transfers to Cuerithm's FFmpeg playable clock with no residual decoder-specific difference. The measured export stage is now implemented; the probes do not independently establish the final audible offset in a DJ application. Do not count serializer agreement as resolving that unknown.

## Scope and canonical input

Cuerithm prepares new analysis from decoded PCM and publishes replacement cues. Treat this as the reference's locally generated cue case (`hasLexiconCuepoints = true`), not as raw cues imported from another DJ database.

The current implementation has these relevant boundaries:

- `cuerithm-media` decodes the first audio stream to stereo, 44.1 kHz playable floating-point PCM. `AudioClock` explicitly defines analysis, waveforms and playback on that PCM clock.
- `Engine::analyze_prepared_inner` records the original source sample rate separately and analyzes a PCM wave copy for common-format input.
- Library publication derives cue/grid times from that analysis and leaves them on the playable clock.
- `cuerithm-library/src/dj_sync.rs` projects cue starts/ends and the saved grid start without changing the publication, and loads the saved prepared-media timing identity.
- `SyncTrack.export_timing` carries measured MP3 or M4A/MP4 timing. `native_timeline` records the target/profile applied to an export copy, preventing accumulated or cross-target shifts.

Do not apply the reference's import normalization to these new publications. Native-database import would need a separately identified origin and its own normalization before reaching the canonical clock. Import normalization is not part of the current export-only feature.

## Locally generated MP3 exports

Offsets below are added to canonical seconds. They apply to cue starts, loop ends and grid anchors alike.

| MP3 header case | Serato DJ Pro 4 | Pioneer USB | VirtualDJ | Engine DJ | djay Pro |
| --- | ---: | ---: | ---: | ---: | ---: |
| A | −5 ms | 0 ms | 0 ms | 0 ms | 0 ms |
| B | +46 ms | +25 ms | 0 ms | 0 ms | 0 ms |
| C | +46 ms | +25 ms | 0 ms | 0 ms | 0 ms |
| D | +46 ms | +51 ms | 0 ms | 0 ms | 0 ms |

The Serato 4 exporter deliberately calls the shared **Serato** timing rule. Passing the separate Serato 4 product identifier directly to the shared reference function would incorrectly yield zero.

The reference's unknown-case fallback produces +46 ms for Serato and +51 ms for Pioneer USB. Unknown information is not evidence of case D or zero encoder delay. Prefer a measured, source-hash-bound profile before declaring an export aligned.

### Case derivation

The observed classifier is implemented with bounded native Rust file reads and CRC arithmetic. It requires neither Python nor an external decoder:

1. Read the leading ID3v2 header, if present, and skip its synchsafe tag length plus the ten-byte header.
2. Inspect the following 2,200 bytes for the first MPEG sync header.
3. Look for `Xing` or `Info` after the four-byte MPEG header and the channel-mode-dependent side information: 17 bytes for mono, 32 otherwise. Missing sync or marker yields A.
4. With a marker present, distinguish a `LAME` encoder identifier and its stored CRC-16/ARC. A missing/non-LAME identifier initially yields B; absent, zero or mismatched LAME CRC yields C; a matching CRC yields D.
5. When the next MPEG frame can be located using the observed MPEG-1 bitrate/rate/padding calculation, compare channel modes. Matching modes promote B to D; differing modes demote D to C.

The classifier uses the encoder field at frame offset 156 and the CRC field at offset 190; CRC covers the preceding 190 frame bytes. This is a specific compatibility heuristic, not a complete MPEG decoder. Record its version with the profile. In particular, do not equate a valid LAME string alone with case D.

Synthetic checks classified a generated no-Info MP3 as A, a generated stereo Info MP3 as B, and isolated header variants as B, C and D. The manipulated variants exercise header classification only and are not playback fixtures.

## M4A/MP4 priming and video

The reference measures audio priming by inspecting the original source file:

- Select its first audio stream and read the source sample rate and stream `start_time`.
- Read the first packet's `pts_time` over the initial packet interval.
- Use the absolute first packet PTS when it is negative; otherwise use stream start time.
- Round the resulting delay to the nearest 0.1 millisecond.
- Inspect video-stream codec names separately. JPEG, PNG and BMP artwork does not count as video. With other video present, use a video delay of `2048 / source_sample_rate` seconds, rounded to the nearest 0.1 millisecond.

The existing bundled FFprobe can perform these bounded probes through `MediaRuntime`; no user-installed runtime, Python process or new user configuration is needed. A pure MP4 atom implementation would need independently validated equivalence to the packet-PTS/edit-list interpretation and is not necessary for this integration.

Use the bundled executable for these two independent queries, with `SOURCE` replaced by the private snapshot path:

```sh
ffprobe -v quiet -print_format json -select_streams a:0 -show_entries stream=start_time,sample_rate -show_entries packet=pts_time -read_intervals '%+0.01' SOURCE
ffprobe -v quiet -print_format json -select_streams v -show_entries stream=codec_name SOURCE
```

Let `d` be measured audio priming in seconds, `v` the rounded video delay, and `r` the **original source rate**, not the 44.1 kHz analysis rate. For a known measured profile:

| Target | `.m4a`, with `d > 0` | `.mp4`, with `d > 0` | Additional video adjustment |
| --- | --- | --- | --- |
| Serato DJ Pro 4 | `d − 2048/r` | `+d` | `−v` |
| Pioneer USB | 0 | 0 | 0 |
| VirtualDJ | 0 | 0 | 0 |
| Engine DJ | 0 | 0 | 0 |
| djay Pro | 0 | 0 | `−v` |

When measured `d` is zero, the Serato audio-priming component is zero: do **not** subtract `2048/r` unconditionally. The video adjustment remains separate. The distinction between M4A and MP4 is extension-based in the observed implementation, so codec identity alone does not describe the contract.

Examples from generated files:

| Generated source | First packet/stream behavior | Reference measurement | Serato offset |
| --- | --- | ---: | ---: |
| AAC M4A, 44.1 kHz | Packet PTS −0.023220 s, stream start 0 | 23.2 ms | −23.239909297 ms |
| AAC M4A, 48 kHz | Negative priming packet, stream start 0 | 21.3 ms | −21.366666667 ms |
| ALAC M4A, 44.1 kHz | No measured leading delay | 0 ms | 0 ms |

The same zero stream start therefore does not distinguish the AAC and ALAC cases. Source sample rate changes the Serato correction even though Cuerithm analyzes both through a 44.1 kHz playable buffer.

Lossless/PCM and Ogg audio without this measured M4A/MP4 profile have zero offset in the observed local-generated export rules. Existing target compatibility limits still apply; a zero timeline adjustment does not establish that a target supports a container.

## Pioneer waveform timing

The Pioneer conversion also adjusts detailed waveform assets. Shifting only cue/grid fields would omit part of the observed native timeline conversion. The local `$`/`Z` helpers in module 43516 (renderer lines 245921–245955) use the same **positive Pioneer export offset** recorded at lines 247869–247871; both the lazy and precomputed waveform paths apply it (247749–247751 and 248195–248208).

| Waveform tags | Observed transformation |
| --- | --- |
| `PWV3`, `PWV5`, `PWV7` | Prepend `round(150 × offset_seconds)` copies of the original first frame. Preserve the remaining frames, grow the count and recompute serialized lengths. |
| `PWAV`, `PWV2`, `PWV4`, `PWV6` | Leave unchanged. These preview arrays do not receive this shift. |

The helper checks tags in all DAT, EXT and EX2 groups. For parsed object entries it duplicates the first entry; for raw data it duplicates exactly `lenEntryBytes` bytes per frame, defaulting to one byte only when no stride is available. It pads with the **first frame's content**, not zeroes, and does not trim the tail. Zero, negative or positive sub-half-frame offsets produce no padding. Consequently, the local MP3 B/C Pioneer offset adds four detailed frames; case D adds eight; case A adds none. Padding uses 150 Hz quantization, while cue/grid fields retain their own time quantization.

Do not apply the padding twice to reused waveform assets already on the device timeline. The observed reuse check recognizes an existing Rekordbox timeline from a positive decoded-sample-count value in `PVBR` (lines 247071–247086) and bypasses waveform padding in that case. Fresh waveform output from canonical PCM still needs the conversion.

Executing the original helpers on synthetic object-entry, byte-buffer and byte-array tags passed 49 assertions: +25 ms added four frames, +51 ms added eight, and all four preview types remained unchanged. This adds waveform-contract evidence, not native playback validation.

Synthetic frame-array regressions decode detailed waveform tags and check four/eight-frame padding for the MP3 B/C and D offsets, count/length updates, nonzero first-frame data, each frame stride, zero offset and unchanged preview tags. Cue/grid conversion has a separate exactly-once marker. Controlled impulse playback through an actual target DJ application remains a separate acceptance step.

## Persisted metadata and implemented boundary

`MediaInfo.properties` retains general stream/container properties, but these alone
are insufficient for the negative first packet PTS distinction. The dedicated timing
probe now measures it on the private source snapshot. `PreparedMediaIdentity` persists
`export_timing` with backward-compatible optional deserialization, and the saved-analysis
projection copies it into `SyncTrack`. It is distinct from `FileAnalysis.source_timing`
and `AudioClock` padding fields; those retain their analysis/playback meanings.

The implemented `SourceExportTiming` stores a versioned profile, source hash, original
sample rate, codec measurement and scanner/bundled-probe identity. For example:

```json
{
  "profile": "dj-timeline-v1",
  "source_sha256": "<snapshot hash>",
  "source_sample_rate": 48000,
  "codec_profile": {
    "type": "iso_audio",
    "extension": "m4a",
    "audio_delay_ms": 21.3,
    "video_delay_ms": 0.0
  },
  "measurement_identity": "<scanner and bundled probe identity>"
}
```

Other variants are `mp3` with case A/B/C/D, `direct` for a measured codec without
reference correction, and `unmeasured` with an explicit reason. An optional
`decoded_sample_count` supplies the native MP3 frame clock for Pioneer PVBR; it is
separate from gapless playable PCM frame count. Missing measurements never imply zero.

The current implementation keeps conversion outside DSP:

1. `cuerithm-media/src/export_timing.rs` captures hash-bound profiles on the private
   snapshot using bounded Rust MP3 reads and the bundled FFprobe. Cancellation,
   bounded output and measurement failures retain explicit state.
2. `PreparedMediaIdentity.export_timing` persists the measurement. Guarded preview
   capture can refresh missing/stale profiles on a verified source copy without
   rerunning analysis or changing the saved publication.
3. `cuerithm-dj-sync/src/timing.rs` converts a fresh export clone once. Its applied
   target/profile marker prevents repeated shifts. Native database fields, cue/grid
   blobs and media tags use that same adjusted timeline.
4. Serato marker starts are clamped before native quantization; a collapsed saved
   loop is rejected. Other targets keep their own native signed-time policy.
5. Skip-grid preserves existing native grids while exported cues are converted.
   AutoFix assigns the converted PCM copy's direct profile rather than reapplying
   compressed-source correction. Pioneer detailed waveforms use the corresponding
   padding once; preview waveforms remain unchanged.
6. Preview guards validate source hash, sample rate and measurement identity.
   Semantic Changed hashes include timing measurements and implementation identity;
   acknowledging Serato's own tag-file hash change does not create endless reexports.

Imported DJ timing should receive a distinct future origin only when native import exists. The reference first normalizes imported baselines and then applies measured case/priming correction; replaying those import steps on a freshly analyzed publication would double-correct it. Streaming source/target offsets are also separate and are outside the current local-audio export scope.

## Regression and acceptance contract

- Table-test the five active targets across MP3 A/B/C/D, including Serato 4 mapping to the shared Serato rule. Compare against the executed reference results within floating-point tolerance.
- Test bounded MP3 classification with generated Info/no-Info files and header variants for absent/malformed LAME CRC and matching/differing next-frame channel modes.
- Test AAC priming at 44.1 and 48 kHz, ALAC with zero delay, unknown measurement, and actual video versus embedded artwork. Verify the first packet PTS path when stream start is zero.
- Verify M4A and MP4 extension rules separately, positive and zero priming, and the source-rate-dependent Serato formula. Preserve the reference's 0.1 ms measurement boundary.
- Export a canonical cue, saved loop and first grid anchor to each native target and decode the actual emitted fields. Expect the target delta on all three, with millisecond/sample quantization applied afterward. Include a near-zero cue for negative-offset clamping.
- Assert saved publication JSON, waveform/playback positions and source PCM are unchanged by target conversion. Repeated exports and exporting multiple targets must not accumulate shifts.
- Test skip-grid preservation and an AutoFix/transcoded PCM copy, ensuring cue timing follows the actual output clock.
- Test old saved analysis/profile absence and source-hash changes. Unknown required timing must not silently become zero or reuse an unrelated prior profile.
- Keep controlled synthetic impulse playback in each actual DJ application as a separate, still-pending acceptance step. Original-reader agreement and the 94 reference-function assertions establish serialization/contract agreement only.

Focused Rust regressions cover timing formulas, source/profile identity, old analysis
handling, native cue/grid fields, MP3 counts, detailed waveform frame padding and
converted copies. These checks and the private reference probes establish measured
contracts and serialization behavior; no native DJ playback or CDJ hardware acceptance
has been recorded. Final full workspace/frontend gates remain pending after review fixes.
