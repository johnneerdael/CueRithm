# Engine progress

Current product scope: Cuerithm uses Lexicon-compatible structural cues, the existing key/BPM backends and one compiled acoustic track-energy rating. MiK cue inference and per-segment energy output are retired. See `docs/cuerithm-integration.md`; the chronology below is historical research.

Current delivery is release stabilization of the integrated Tauri 2 desktop
application. See [release scope and outstanding gaps](release-readiness.md).
The engine-first/UI-deferred scope and measurements below are historical;
they do not describe the current desktop or its selected analysis backends.
Historical plan: `docs/superpowers/plans/2026-10-16-offline-engine.md`.
Historical measurements: `docs/research/engine-benchmarks.md`.

## Latest integration status, 2026-10-17

The optional `openkeyscan-key` / `--key-openkeyscan-model` backend now runs from
normal Engine/CLI analysis, with direct44100Hz input, fixed native SoXR/CQT,
pinned owned-buffer graph loading and raw-logit rankings. It preserves default
configuration identity/legacy JSON and rejects conflicting key backends. Model,
frontend, native source and build/dependency identities are retained. See
`models/openkeyscan/README.md` and `THIRD_PARTY_NOTICES.md`.

Fresh Mac verification:386 default tests pass (2 ignored),399 all-feature tests
pass (7 ignored), formatting/default+all-feature Clippy and all-feature release
build pass. The actual model and Engine smoke checks pass, including direct-rate
routing, unknown/partial results and path-independent configuration identity.
Independent whole-change review reports no code findings. The final-binary tuning and Linux production checks below also pass.

Private final-CLI tuning run: `openkeyscan-engine-integration-v1/corpus-v1`,
exec49312. All282 tuning groups completed with verified integrity and unchanged originals,
combining OpenKeyScan with source BPM. All282 keys match the upstream candidate;
274/282 match MiK11 (97.16%). Fresh-request BPM/first beat anchors match exactly
on282/282; all199371 beat values match the prior timing path. Reference grading
followed the terminal barrier. A representative actual sandbox denies both a
synthetic evaluator canary and network connection. Both exec49312 and exec12785
are terminal success. Linux ARM64/Rust1.89 passes399 all-feature tests (7 ignored),
actual model/Engine checks and combined CLI. All290 source files are unchanged
and the owned container is removed. A separate x64 MSVC cross-build now compiles the all-feature release and all
test targets from the same290 files; no product-source fix was needed. Its
macOS linker needed an isolated rpath repair. Actual Windows x64 execution now
passes the generated-audio OpenKeyScan smoke test in the user's AWS WorkSpace
(Windows NT10.0.17763, AMD64,64-bit process). The expected6A class,44100Hz input,
model checksum and source/configuration identities are verified. The executable
is unchanged; the reporting harness needed a Windows PowerShell architecture
compatibility fix. This is a runtime smoke test, not a music-accuracy result.
All26 cross-compiled all-feature Windows test targets have now executed on that
VM:399 tests pass,7 remain ignored in the normal suite, and both OpenKeyScan
model-dependent tests pass explicitly. There are no failed tests or timeouts.
The executable hashes and terminal logs were verified. Native Windows compilation,
broader real-music Windows parity and held-out accuracy remain unverified.

The user approved All-In-One as optional deep analysis on2026-10-17. Its fixed
24-track full-ensemble reference run completed successfully, retaining502 raw
sections and24 separate cue-policy outputs. Same-group MiK F1 at50ms is41.36%
for the initial All-In-One cue policy and62.50% for acoustic8/cosine CBM; their
different timing grids prevent an isolated selector comparison. No listening
ratings or musical-quality winner exists. A five-arm anonymous listening packet
is ready. The complete8fold structure network passes a generated-input Rust gate
on Mac and Linux ARM64 (247195 values across7 lengths, max error2.9e-6). Native separation, spectral
frontend/postprocessing and deep-model production integration, energy
classification, broader formats and held-out evaluation
still remain. UI work stays deferred. Earlier sections retain prior stages and
measurements rather than superseding this integration status.

Energy follow-up: fixed whole-group cross-validation on all6182 request windows
now reaches76.88% exact MiK label agreement (95.21% within one level) for a
balanced larger classifier, versus75.32% for the same-fold earlier model.
Macro recall is50.22%; rare levels remain weak and no level10 example exists.
The native Rust model evaluator matches all saved outputs across five
classification arms (30910 window predictions), including logits/probabilities
and sequence decoding. This is a private request-vector component prototype,
not production PCM integration, overall-energy acceptance or held-out accuracy.
See `docs/research/request-energy-classification.md` for negative results,
overall-energy limits and the independent evaluation audit.

## Feasibility and next acceptance gate

Perfect offline reproduction on unseen tracks remains unproven. The client
request calculations are available for source reconstruction, with exact
component comparisons already recorded below. The remote key/energy/cue
selection implementation and weights have not been recovered. Matching its
request establishes matching inputs, not an independent copy of that service.
Finite request/response examples constrain possible implementations without
uniquely specifying their behavior on every new input.

Judge the next phase by complete development tracks: remove supplied original
timing, finish energy/segment producers and compare a complete independently
generated numeric request against the captured request. Every dependency still
supplied must remain explicit. Then assess the offline request-to-result mapping
separately before claiming exact final outputs or generalization. Further isolated
arithmetic matches do not by themselves establish whole-engine progress against
the user's acceptance gate. Preserve the held-out split until engine freeze.

## Boundaries and corpus

- Inline workspace, no Git repository. No commits, publication or UI work.
- Preserve the installed application, music and CSV. Private artifacts stay under `.analysis/`.
- Predictions use PCM and explicit configuration only. No reference-tag, filename
  or per-audio-ID answer lookup. Model training/evaluation remain separate.
- 434 FLAC files, 364 declared PCM groups; frozen tuning282 / holdout82 split.
  All tuning representatives have completed FLAC integrity checks. Holdout remains
  unused for local calibration, model selection or mismatch inspection.
- Tuning has only eight major-key examples. Full corpus covers17/24 keys; a larger
  library is still needed for missing modes/styles/tempi and external evaluation.
- Target macOS/Windows/Linux. Apple Silicon macOS and Linux ARM64 checks pass;
  actual Windows x64 key inference passes a generated-audio smoke test. These
  scopes do not establish full Windows validation or held-out music accuracy.

## Implemented source

- Streaming Symphonia decoding/downmix, direct per-rate Rubato resampling,
  all24 Camelot mappings, DSP key/classical tempo, reusable Engine facade and CLI.
- Grouped reference evaluator, failures/unknowns in denominators, one-to-one cue DP.
- Resume validates cached source metadata; repairs only an interrupted final JSON
  tail after backup, and rejects malformed committed records/config changes.
- Optional Beat This! small model via Rust RTEN; periodic-lattice BPM fitter fixes
  missing/extra-detection index drift. Models explicitly supplied, no runtime downloads.
- Optional shared learned DSP key profiles; all24 transposition/mode tests and
  independent NumPy inference parity on277 saved records pass.
- `audio::decode_at_rates` decodes once and resamples each branch directly from
  source PCM. Learned key/beat frontends share22050 Hz; DSP/structure stays at the
  configured rate (default11025). Direct-vs-separate full/partial outputs match.
- Review reproduced a high/odd-rate resampler flush error. Aligning chunks to a
  reduced-ratio input block fixes384000 and47999 Hz regressions. All7 audio tests pass.
- Optional `neural-key`/`--key-neural-model` uses the pinned S-KEY end-to-end ONNX.
  Fixed SHA/class order, no second softmax, minimum3s/silence/nonfinite policy.
  Learned DSP profiles and neural-key flag conflict explicitly. No final default
  key engine has been selected. Core/default and each individual neural feature build.
- KeyEstimate retains optional flattened TonalFeatures. Legacy DSP JSON round-trips;
  missing neural-only diagnostics are omitted, and partial/broken diagnostics fail.
- Opt-in structure timeline: RMS, six-band power, normalized flux and centroid.
- Opt-in `--cue-suggestions`: novelty ranking on learned model beats, first valid
  beat retained, rounded musical-beat separation, up to8 neutral Candidate cues.
  No accepted energy-name model or Rekordbox export exists yet.
- Model/source/effective-config identity covers chosen model content/rates/options.

## Current verification

- Full all-feature suite:57 passed,5 fixture/model-dependent tests ignored.
- New real S-KEY model smoke explicitly passed; source class order maps all24 keys.
- Current learned-profile NumPy parity rerun:277/277, scores within1e-9.
- Cue selector parity:38/38 tracks match independent Python prototype.
- Formatting, all-feature Clippy, default/key-only/beat-only checks and release build passed.
- Fresh read-only reviewer found no remaining material findings after the decoder
  fix, and no material findings in neural-key/serialization/feature integration.
- Private RTEN-vs-ONNX Runtime cases used identical PCM. Music25s/noise score error
  <1.4e-6; sparse harmonic case3.8e-4, all three classes identical. Strict1e-5 gate
  failed; do not claim universal numerical equivalence. Evidence kept intact.
- Upstream S-KEY checkpoint hash matches export manifest. Public provenance and
  MIT code notices are under models/skey; weights are external. Upstream README
  does not separately state checkpoint licensing; no weights were published.

## Completed music measurements

- baseline-v1:282 tuning groups,5 old decoder failures retained; key99/282,
  BPM<=0.5:181/282, exact displayed BPM38/282,93 half/double errors.
- profile-balanced-tempo-range-v1:282 complete/integrity-verified, key223/282
  (training fit), BPM<=0.5:269/282, exact58/282, zero metrical errors,639.1s.
- structure-classical-v1:all282 complete/verified, same key/BPM predictions,
  structure timeline retained,734.7s with concurrent work.
- Original balanced-profile five-fold tuning CV:221/282 including5 failed-feature
  penalties; major75% (8 examples), minor79.9%. Zero-major-recall model rejected.
- Quadratic profile CV224/282 only a3-track gain; not adopted.
- Corrected saved12-track neural beat fitter:12/12 exact displayed BPM. A fresh
  full-track run verified integration; never treat the original flawed fitter as
  evidence against the beat model.
- neural-native22050-pilot-v1:12 complete/verified, key features/results identical
  to old path,12/12 exact BPM. Cues45/95 at50ms versus44/95 on same older-rate
  groups. Nearest-beat50ms coverage93/95 versus92/95, median19.9ms versus25.7ms;
  100ms coverage93/95 versus94/95.313.0s concurrent, not a controlled speed test.
- skey-full-v1:raw pretrained S-KEY, no local training/calibration:254/282 keys
  (90.1%), major5/8, minor249/274;0 failures, all complete/verified,457.4s.
  Pretrained training-corpus overlap has not been audited; this is local tuning
  agreement, not a clean external/held-out score.
- key-fusion-v1:whole-group CV on the same282 folds: selected fusion253/282,
  major75%, minor90.1%; training fit255/282. Offers a mode tradeoff, no clear
  overall gain. Keep the extra head as a private research artifact.
- stratum-default-pilot-v1:unchanged default upstream on12 groups, key5/12,
  BPM<=0.5:10/12, exact0/12; own untrained DSP3/12, trained profile11/12 on same
  groups. All decodes verified; not adopted.
- Cue prototype38-track partial tuning baseline:129/296 within50ms,42.4% precision,
  43.6% recall,32/38 exact counts. Fresh combined old-key/native-beat smoke on one
  complete373s track:correct key/BPM,8 cues,3/8 timing matches. Not sufficient quality.
- cue-energy-context-v1:ridge on2195 cues at reference times, group CV41.8% exact
  names vs44.3% median/majority baseline; within-one-level87.7% vs81.2%. Unadopted.
- cue-energy-trees-v1:same oracle-position component/folds. Best aggregate variant
  48.7% exact but18.6% macro recall; balanced variant37.9% exact/31.4% macro. Remain
  unadopted: broad loudness/band features do not yet provide useful exact names.

## Live work: poll these exact handles before restarting

- Completed exec **23792**: `.analysis/benchmarks/neural-small-full-v3/`, old11025-upsampled
  beat frontend plus balanced profile, all282 groups. Completed282/282 with0 failures and verified integrity. BPM280/282 within0.5,
  271/282 exact displayed BPM,0 half/double errors; key223/282 (profile training fit).
  Elapsed5939.1s concurrent. This run did not request cue predictions. Immutable binary `.analysis/bin/camelot-engine-v3` SHA
  `7fb6a66d3ce56078941b63cdc3258f8bfa72047fc181b4ca20b1fc3b0e18ad19`.
- Completed exec **4028**: `.analysis/benchmarks/combined-skey-native-v1/`, current raw S-KEY,
  direct22050 beats, structure and experimental cues; all282 tuning groups.
  Binary `.analysis/bin/camelot-engine-v7` SHA
  `a05fe7d188ce4bb364f9caa217db9c188bc7a965eda7e17b63503ca448420433`;
  source `.analysis/source-snapshots/engine-v7/`. Completed282/282 with0 failures and verified integrity. Key254/282;
  BPM275/282 exact displayed,281/282 within0.5,0 metrical errors. Cues1028/2195
  within50ms,2250 predicted,248/282 exact counts; names remain Candidate.
  Elapsed4183.3s concurrent. All282 key labels/top scores need full adapter parity audit. Fresh matching key labels/top scores
  match the reference-free S-KEY adapter on every completed matching group.
- Both runs use RAYON_NUM_THREADS=1. Timeout is not termination. Read only through
  the final newline when inspecting an appending checkpoint. Never resume an
  immutable run with a rebuilt binary or changed settings.
- Exec **23457** is terminal:Linux ARM64 validation passed in Docker,2CPU/4GB, user501:20, read-only
  v7 source, task-specific Cargo cache/target, no library/music/reference mounts.
  Image rust:1.89.0-slim-bookworm digest
  `sha256:d7fc7de78bb8c1469933aeecbf801314d30d7d6e9f0578bba4cfa285bfa37fe6`.
  Logs/results `.analysis/linux-validation-v7/`:50 routine tests passed,5 private
  tests ignored; real key and beat model smoke tests each passed with network
  disabled and cargo offline. This proves Linux ARM64 backend execution, not
  desktop packaging or Windows behavior.
  Docker Desktop was started for this check. Owned containers use --rm.
- Main ARM64 REA import/decompile completed despite an MCP timeout. Complete
  retained evidence recovered and exported through rea-cue-followup-evidence-v4.json.
  CuePointData helper0x1000933b0 packs capped leading/trailing zero counts into
  two header bytes plus little-endian float32 scores, then Base64 encodes.
  Score timebase/meaning still unresolved; no proprietary code copied into engine.
- Functions exec cell **280** completed the DanceabilityAudioEngine ARM64 import.
  Active target `.analysis/mixed-in-key-11.2.6/DanceabilityAudioEngine.arm64`, SHA
  `a2ab71482503b71edfc5e372b9fcf4d7502f19f808db32ffd07b5eec84de7419`.
  DetectLikelyCuePointsV3 found at0x3232c; caller passes literal8 andfalse but
  their meaning is not yet established. No new decompile requested.
- User requested actual mitmweb JSON capture. Private setup under
  `.analysis/mitmweb-analysis-v1/`, addon capture.py and six-check synthetic
  selftest passed. User enabled pending macOS Mitmproxy Redirector extension.
  Exec **16628**, mitmweb PID27923, local:47720 (Mixed In Key PID), host allowlist
  online5.mixedinkey.com:443, web127.0.0.1:18081. Only detached redacted analysis
  request/response copies retained. Existing CA already trusted; no system proxy
  changes. Original request/response never modified. Captured one HTTP200 transaction
  (132847 request bytes,1018 response bytes), ID4d920b00-7ba2-472e-af81-0550121711ca.
  Live interception stopped (exec16628 terminal). Offline viewer exec95295 also stopped. No task-owned proxy/viewer remains.
  Sanitized .mitm flow can be reopened in mitmweb without live interception.
  First imported copy reused tags. Second capture-fresh.flac has analysis tags
  removed and unique title Camelot Capture Test, unchanged declared PCM checksum.
  Only test copies imported; source library files unchanged. Two test entries
  added to live app library; original434 entries preserved.
- All other benchmark/training/build/test handles recorded in prior turns are terminal.

## Next required work

1. Observe current live jobs; verify fresh combined engine key outputs match the
   reference-free adapter on matching groups; inspect broader BPM/cue gaps.
2. Finish Linux checks, resolve actual failures, record platform scope accurately.
   Windows execution and packaging remain unverified.
3. Trace the native cue-preparation helper/CuePointData, retaining complete REA
   Evidence privately. Service contract now records packed Base64 float32 format and one sanitized
   live transaction; score semantics/timebase remain unresolved.
   Parse minified evidence JSON selectively; do not grep entire JSON lines.
   Never dump full server_identity.catalog; select only package/version/status.
4. Improve cue ranking/counts with grouped supervised learning or better documented
   local feature extraction. Old/native beat components and separate structure
   timelines can support exploratory reprocessing with explicit provenance.
5. Improve key and energy errors/rare-label coverage. Keep training-fit, tuning CV,
   composed component scores and fresh end-to-end results distinct.
6. Implement/verify practical Rekordbox cue export on a disposable playlist.
7. Freeze the actual chosen engine/configuration before inspecting holdout once.
   Obtain whole-engine review and actual desktop checks before declaring readiness.
8. Do not call the engine settled from tests, training fit, prototypes or pilots.

## 2026-10-16 follow-up: cue timing and selection

- Live captured response exactly matches all eight saved tuning reference cue
  times and names for the disposable source copy. Returned positions are exact
  integer beat offsets from the request's supplied tempo/downbeat. This is one
  observed fixture, not a universal format inference.
- Native DetectLikelyCuePointsV3 at0x3232c and CalculateScore at0x34138 were
  decompiled; scoring consumes beat-normalized granules and multiple musical
  windows. Complete evidence in rea-danceability-cue-evidence-v2.json; proprietary
  pseudocode remains private. No production algorithm copied.
- Completed cue-grid-v2 diagnostic,282 tuning groups,2195 reference cues. A grid
  fitted from PCM-derived beats has2157 candidate matches within50ms versus2115
  raw detections. These are candidate timing ceilings, not selection accuracy.
  One reference has four equal timestamps; repeated cues remain in denominators
  while one predicted beat can match only one reference. Failed v1 distinct-cue
  assumption retained as failure.json; no source labels changed.
- Complete old-beat cue heuristic:1006/2195 timing matches,44.7% precision,45.8%
  recall,248/282 exact cue counts,2250 predictions. cue-heuristic-full-v2 report's
  inherited scope text says partial; actual track count is282/full tuning.
- cue-selection-trees-v1:294 PCM-only beat-window/phase-context features,64 trees,
  whole-audio-group five-fold CV. Selected depth12/gap32/no phase pool:1246/2195
  timing matches,55.4% precision,56.8% recall,2251 predictions,248/282 exact counts.
  Configuration selected on these folds; not a held-out result and not adopted.
  No model exported or production inference path added yet.
- Ruling: implement the already specified Rekordbox XML export while the frozen
  combined benchmark continues. Use a separate pure-Rust renderer and CLI for
  saved analysis, escape metadata/URI paths, preserve cue precision, require an
  explicitly verified grid rather than inventing metre from BPM, and refuse
  overwriting an existing output. New tests are being written before implementation.

## Latest user steering

Stop focusing on Rekordbox. Prioritize perfecting key detection, energy, cues
and BPM. Export renderer/CLI work passes its seven new tests and host full suite,
Clippy and release build, but is not the active focus. Rekordbox7.2.19 read the
synthetic XML playlist; first11025Hz fixture did not load to deck (unsupported
CDJ/XDJ notice). Second44100Hz fixture prepared, not yet loaded/verified.
Temporary Rekordbox Imported Library originally
/Users/jneerdael/Library/rekordbox/rekordbox/rekordbox.xml was changed to private
fixture path; XML browser originally off was enabled. Must restore those
temporary settings, without further export investigation. Current app Preferences
AX tree lost its controls after switching fixture path; do not claim restoration.
UI observation in .analysis/rekordbox-export-v1/rekordbox-ui-observation.json.

- Core-feature follow-up: immutable key temporal probe completed in exec31705,
  `.analysis/benchmarks/skey-temporal-full-v1/`,282 tuning groups,30/60s windows
  centred at20/50/80% of duration. Binary .analysis/bin/camelot-skey-window-v1
  SHA1ec26e7c142636e8ed4288d3caafe16bfcad7c2b27642f681f3a450bd5b5dd4c.
  Source/Cargo lock snapshot .analysis/source-snapshots/skey-window-v1. Uses
  existing Rust NeuralKeyModel, direct22050 PCM and one verified full decode;
  no reference manifest supplied. One thread. Do not restart while live.
  Need measure whole-tuning aggregation against raw S-KEY254/282; no holdout.

## Acceptance gate clarified by the user

The user is not satisfied without perfectly accurate reproductions. Exact
agreement with Mixed In Key reference key/BPM/energy/cue outputs is the gate,
not an approximate first usable engine. Do not claim settled/complete or move
to the UI while mismatches remain. Treat all improvements as intermediate.
Keep raw fractional cue times and names/counts in evaluation, include failures
and unknowns, freeze before holdout and preserve independent PCM inference.
Do not replace this goal with approximate compatibility or a lower accuracy bar.

## Latest completed core work

- Temporal key probe exec31705 completed282/282,0 failures,451.3s. Evaluation
  skey-temporal-evaluation-v1: raw full-track254 correct remains best.30s/60s
  mean/logmean/vote variants range246–253. Rejected; no production key changes.
- Overall-energy-v1 whole-group CV: best169/282 (59.9%) vs144/282 majority
  baseline (51.1%), only28.6% macro exact accuracy. Not adopted. Classes4..8
  only; class4 has one track. No production overall-energy estimator yet.
- Cue-energy-boosting-v1, same2195 oracle-position cues/282 whole-group folds:
  unbalanced shallow boosting51.4% exact/24.4% macro versus earlier trees48.7%;
  balanced alternative44.3% exact/33.9% macro. Both inadequate; not adopted.
- Core-mismatch-audit-v1 selected4 difficult tuning groups from28 key/7 displayed
  BPM mismatches (33 groups total). Four tag-free copies analyzed by licensed
  Mixed In Key;4 HTTP200 transactions captured, then interception stopped.
  All four reproduce source key, displayed BPM, overall energy, cue counts/names
  and exact original persisted cue milliseconds. The repeated four timestamps
  in one short track are real reproducible vendor output; do not deduplicate or
  discard this case to improve scores. A seconds↔milliseconds round trip causes
  one2.84e-14s difference, while original stored milliseconds match exactly.
  Source originals preserved. Four additional private audit entries now in MIK
  library (440 total versus434 original, including two earlier test copies).
- Strict exact-fidelity-v1 gate runs against completed combined predictions:
  no time tolerance, original persisted milliseconds including negatives,
  duplicate multiplicity and slot order. Missing production energy fails. CSV
  supports displayed BPM precision only; do not claim internal tempo equality.
- Native energy feature work: active Ghidra target MIKAnalysis.arm64 SHA
  7cb8dee9278c4ec8e0f970ecd54d9d1f298ceedba4d079668321971336dbddf8.
  Init0xa71c creates EnergyFeatureExtractor at44100Hz with current maximum
  version. GetSegmentResults0xc3bc computes32-beat segment times using first
  cached segment tempo, and20log10(RMS floored1e-50). OutputToSvm0xe4d8
  constructs weighted spectral/rhythm feature vectors and removes zero-weight
  entries. Whole183-feature interpretation and the server classifier remain
  unresolved; do not substitute the unrelated EnergyProc audio effect.
  Complete evidence: rea-energy-features-evidence-v1.json (26 records).
- Rekordbox cleanup completed: task-launched app exited normally; restored only
  bridgeImportedLibraryFile to original path and showRbXml to0 in its settings.
  No further export investigation. See private UI-observation record restored=true.
  Existing export implementation remains deferred and is not a core accuracy gain.

## Mechanism validation and actual remaining work

- Exact-fidelity-v1 completed: zero complete exact reproductions among282 tuning
  groups. Key254, displayed BPM275, cue count248; exact persisted cue timestamps
  zero of2195; names Candidate and no production energy estimator. This is a
  failed gate, not an approximately completed engine.
- Private native-feature-oracle-v1/energy_oracle.mm compiled successfully with
  Apple clang/Foundation/AVFoundation/CoreAudio and dlopen of the installed
  MIKAnalysis framework. It uses MIKEnergyAnalyzer's existing local ObjC API,
 44100Hz Float32 PCM in4096-frame chunks, no service/license calls or audio writes.
  First captured track: all22x183 features, segment times and volumes exactly
  match. Four audit tracks: one exact, three differ in about112 values/segment
  by up to2.34e-5. Complete parity/difference files retained; do not claim full
  fidelity or dismiss the differences as harmless noise.
- Native-feature-oracle-v2 varies chunk size.512,1024,4096,8192,32768 all return
  the same mismatch on the short case; chunk-size hypothesis rejected.
- Native-feature-oracle-v3 invokes MIKNewKeyAnalyzer in the same decode pass.
  Full/range/raw/processed/harmonicity variants each produce88 values, while the
  wire key segment has352. None of these standalone variants directly equals a
  wire88-value block. Need trace the main app's feature-combination builder.
- MIKNewKeyAnalyzer init0x21030 constructs KeyFeatureExtractor at44100Hz with
  boolfalse; finish/result/raw-range methods preserved in current REA evidence.
  Do not infer stereo policy from the bool without tracing it.
- The original local extractor oracle is a private macOS validation tool, not
  the portable open-source engine. Reconstruct the complete mathematics in Rust
  and validate intermediate features before claiming equivalent inference.
- Next high-value targets: main MIKFileAnalyzer key feature assembly and
  processEnergyData (potential postprocessing of native energy arrays), followed
  by complete original energy183 feature construction and local classifier
  reconstruction. Native BPM/phase/cue fallback behavior also needs tracing; the
  current32-beat suppression cannot reproduce the confirmed short-track duplicate
  cue slots. Preserve those cases rather than changing references.
- Current evidence saved in rea-energy-features-evidence-v2.json. No benchmark,
  training, model probe or mitmweb process from this turn remains live.
- Rekordbox temporary path/browser settings are restored and the app is closed.
  No export work is authorized as the current focus; concentrate on the four
  core analysis outputs and exact agreement.

- Functions exec **406** is now live: main ARM64 target opened successfully and
  Ghidra is importing/analyzing it for MIKFileAnalyzer::processEnergyData at
  0x100073b64. Store keys core_fidelity_main_open/main_energy_postprocess.
  Use functions.wait on406; an MCP timeout does not prove backend termination.
  Current target changed from MIKAnalysis.arm64 to MixedInKey11.arm64 after all
  framework evidence was exported. No other process/model benchmark remains live.

## Original-reader/energy reference contract now reproduced

- Native decoder oracle in native-feature-oracle-v6 calls MIKAudio.framework's
  MIKAudioFileOpen/GetFileStreamFormat/GetLengthFrames/SetReadStreamFormat/Read
  with the actual main app's512-frame interleaved Float32/44100Hz setup. All five
  observed read frame counts exactly match request DurationInSeconds*44100.
  Four are shorter than declared FLAC length; do not assume full source length.
- Native-feature-oracle-v5 controlled tests: shortening only constructor duration
  did not help; feeding the observed audio prefix fixed all energy fields exactly
  in all three previously differing cases. This is a diagnostic, not a predictor.
- Native-feature-oracle-v7 composes the original reader and MIKEnergyAnalyzer.
  It takes only audio/output paths, no captured context. All five captured cases
  match every EnergySegmentData field and analyzed duration exactly.
  client-energy-capture-parity.json records source/binary hashes and limits.
- Full tuning original-energy reference extraction exec96819 is completed:
 282 tracks,0 failures,6182 segments,299.9s,148 shorter decoder outputs. Files
  .analysis/reference-features/energy183-native-v1/. This is original-client
  intermediate reference generation, not independent Rust engine predictions.
  Decoder losses can exceed1024samples (max3851), and simple N%1024 trimming
  does not explain all cases. Need trace MIKAudio's custom libFLAC reader; do not
  invent a per-track trim table. Core full-source integrity checks remain intact.
- CoreData analysis store found: ~/Library/Application Support/Mixedinkey/
  Collection11.mikdb. Read-only schema shows ZSONG/ZBEATS, ZENERGYSEGMENT,
  ZKEYSEGMENT and ZCUEPOINT; dense energy labels and beat grids may already be
  available, avoiding another whole-library service run. Scope queries to tuning
  audio identities, do not read license data, save bookmarks or inspect holdout
  musical outputs. Metadata-only joins built for282 tuning groups, with some
  aliases; exact file identity resolution is being prepared.
- Main import406 completed; no longer live. Main energy postprocess merely
  calls finishAnalysis and assigns energySegments (Evidence bce89142...).
  Main analyzeFile0x100073d88 reads512-frame blocks, creates energy with skip0/0
  and calls +[MIKBeatAnalyzer setAufTAKTVersion:0] before tempo initialization.
  Prior v4 rhythm oracle omitted that setup and used4096-frame reads; it is not
  a faithful measurement of the actual configured tempo pipeline.
- Main key feature builder located via selector+stub+caller trace:
  MIKAnalysisOperation::performSinglePassSingleSegmentOnnxAnalysis0x10008f378,
  Evidence8dd14bbb...; assembly02c77b5c... confirms four-native-beat windows,
  floor/ceil edge inclusion with1e-9, harmonicity=true, phase bank(bar&3)*88.
  It combines four88-value banks into352, using refined beat times.
  Native v6 controlled uniform grid reconstruction gets352 values with total
  sum differing9.24 out of9.66e8 on the first case, max36.38 per-feature error.
  It is NOT exact, uses captured tempo/phase, and is not an independent key
  predictor. Correcting input end did not change this discrepancy. Need actual
  refined grid/full-precision timing rather than guessing from float32 wire BPM.
- Ghidra infers incorrect undefined8 returns for time<->beat helpers0x100090634
  and0x100090798; pseudocode misleadingly says return0. Assembly proves return
  in d0, fractional interpolation and FMADD in beat->time. Use assembly authority.
- Main beat transform and refinement wrappers are retained in functions store.
  Do not infer a client neural model from the method name OnnxAnalysis. Main
  Resources' two encrypted sep01 models do not establish a core beat model.
  MIKNewWaveform/MIKNewWaveformAnalyzer are exported by DanceabilityAudioEngine.

## Full-precision gate and intermediate references (latest)

- Read-only library extraction completed for all282 exact-file-verified tuning
  identities:2818 merged energy spans,199371 beat timestamps. All source key,
  displayed BPM, overall energy, cue counts/names and original cue milliseconds
  agree with frozen references. No held-out musical results or license fields
  were read; opaque bookmarks were resolved transiently and not saved.
  `.analysis/experiments/mik-library-analysis-v1/` contains provenance and data.
- `exact-fidelity-v2` strengthens acceptance to exact internal BPM, explicit cue
  energies and original stored milliseconds, retaining duplicate and negative
  cue slots/order. Frozen combined results remain0/282 complete,254 keys,
  0 internal BPM,248 cue counts,0 exact times/names/energy. Rounded275/282 BPM
  remains a diagnostic. Beat-grid/timeline exact equality also0/282. Five durable
  regression tests exercise exact acceptance, nextafter/rounded/invalid BPM,
  duplicate retention, order/precision, missing values and identity isolation.
- Native rhythm oracle v8 now uses the proper +setAufTAKTVersion:0, zero
  adaptationSpeed and original512-frame44100Hz interleaved reader. All five
  captured cases finish; only one legacy tempo equals final wire tempo, so the
  main waveform refinement remains necessary. Earlier v4 setup is superseded.
- Archived grid geometry:281/282 are uniform within numerical roundoff and
  grid-implied BPM rounds to stored float32 BPM. One source retains a nonuniform
  grid with8.57s deviation from its end-to-end linear fit.236 first grid beats
  equal first cues; never impose a uniform grid on all references.
- Controlled v9 key aggregation uses archived beat grids and original reader/
  analyzer, not independent inference. Four captured cases get much closer but
  still have nonzero feature errors (max~0.000602); the nonuniform-grid case has
  a large mismatch (~8.07million). Archived postresponse grids cannot yet be
  treated as the exact prerequest key-builder input. No tolerance was accepted.
- Native energy component CV completed on6182 native183-feature+RMS vectors,
  labels assigned by unique merged-span midpoint. Five whole-audio-group folds:
  shallow boosting4657/6182 (75.3%) exact,44.8% macro recall,4 tracks all labels
  exact. SVM4460 (72.1%),49.6% macro,6 tracks all labels exact; ExtraTrees4602
  (74.4%),43.9% macro,4 tracks. No production model exported or adopted. These
  use original-component inputs, not the independent Rust extractor. Process
  exec57210 completed and is no longer live.
- Main REA evidence v3 exported:70 records,1280034bytes. Current active target
  changed to MIKAudio.arm64 SHA4168fe8d7b964ceb9dd4c852656eec04624eee78405f02bed77b5632f2995718.
  Original custom FLAC read0x1428 drains its output buffer and calls libFLAC
  process_single; write callback0x10a4 feeds each FLAC block through CoreAudio
  conversion in4096-output-frame batches. Input callback0x17b0 supplies each
  source frame once, then returns 'more' instead of EOF. No final converter
  drain is performed. Reader evidence v1 exported (includes prior session records).
- Private interposer v1 failed withSIGSEGV only in our helper process; no installed
  app modification. Correct standard dyld interposer v2 traces converter calls
  and preserves all five observed read lengths. Traces show final residuals
  stay buffered if the final source frame completes an output batch; a short
  final frame that cannot complete one is returned in full with 'more'.
- A generic4096-frame batching simulation predicts analyzed length exactly on
  all282 tuning sources, all with4608 fixed-block STREAMINFO. No per-track rules
  or predicted-output lookup tables. Still validate on generated block-size and
  boundary cases before adding compatibility behavior to independent decoding;
  retain full-source integrity verification. Synthetic validation exec46240 is
  currently live; poll once before starting anything else. Native original
  helpers remain private validation tools, not product dependencies.
- No UI/export work, classifier adoption or production Rust algorithm change
  occurred in this pass. Exact reproduction remains unmet; goal stays active.

- Synthetic batching validation v1 stopped after92 generated cases because the
  FLAC encoder requires --lax for8192-sample blocks; fixtures/failure retained.
  Fixed encoder configuration in v2:95/95 exact across512/1024/2048/4096/4608/
  8192 blocks, multiple preceding-block phases and final lengths. No musical
  output claim follows. exec46240 and replacement47771 are both terminal.
- Independent Rust boundary probe is being built in private experiment crate
  `.analysis/experiments/decoder-boundary-rust-v1/`. It fully decodes actual
  FLAC packets with Symphonia and verifies source MD5/length before reporting a
  simulated analyzed prefix; no original frameworks, tags or labels used in
  prediction. Validated scope44100Hz stereo16-bit (all282 tuning inputs).
  Six unit cases include same total frames with different packetization, so a
  shortcut based only on total length is explicitly rejected. Test/build jobs
  may still be live; revalidate before starting another run.

- Independent Rust boundary run completed (exec80683 terminal):282/282 tuning
  tracks and95/95 generated cases match original analyzed frame counts,0 failures,
  all full-source lengths and FLAC MD5 verified. Elapsed93.5s, no original framework
  dependency, reference labels absent from binary inputs. Binary SHA
  b62c7e65a66788e68a747fa2df5cd7f597d9b158f6110e4ce80cfe93093d8ed1;
  six unit tests, Clippy -Dwarnings and release build pass. Source/binary identities
  frozen in .analysis/benchmarks/decoder-boundary-rust-v1/run.json. Production
  analyzer still unchanged; this isolated private prototype establishes boundary
  behavior only. No Windows/Linux runtime check performed for this prototype.
- Four fresh reanalysis-copy beat archives are byte-equivalent in timestamp
  arrays to original tuning archives, including the nonuniform grid. Old archive
  staleness is therefore rejected as the v9 key mismatch explanation.
- Reinspection of retained time-to-beat assembly exposed an important v9 oracle
  error: original comparison direction uses edge-interval extrapolation, unlike
  a conventional ascending-grid interval search. Preserve actual behavior for
  exact reconstruction, even where it differs from the expected interpolation.
  New controlled v10 implements that mapping; captured parity run exec33320 is
  live until polled. This still conditions on original archived grids and uses
  original decoder/analyzer, so it must not count toward independent engine gate.

- V10 controlled key-input reconstruction completed (exec33320 terminal): all
  five cases reproduce352/352 wire features exactly (maximum absolute error0),
  including the nonuniform grid case. Every energy field remains exact too.
  `.analysis/experiments/native-feature-oracle-v10/captured-key-parity.json`
  records complete source/binary identities. Original ascending-search assumption
  caused both tiny uniform-grid discrepancies and large nonuniform-grid error;
  assembly-directed edge extrapolation fixes both. Do not repair this observed
  behavior to a conventional lookup when pursuing exact compatibility.
- No process from this pass remains live: native CV57210, synthetic47771,
  Rust test44634/build4051/Clippy10051/boundary benchmark80683, and native key
  parity33320 all completed. REA current target MIKAudio.arm64 remains loaded
  idle. Original app, source music, library database and user proxy settings
  were not modified by these read-only probes. No active interception.
- Next work remains independent44100Hz stereo feature reconstruction and main
  waveform/tempo refinement, then offline classifiers and exact cue behavior.
  Key bank aggregation and decoder boundaries now have exact component targets.
  The full-precision production-engine gate remains0/282 complete outputs; no
  UI work or lower accuracy acceptance is authorized by these component results.

## Latest continuation: exact arithmetic stages and JSON precision

- Previous goal turn classified as progress: independent decoder boundary
  prototype377/377 verified, v10 original key input352/352 on5controlled cases.
  Full objective/strict gate remains unchanged; no UI or export work.
- Production bug fixed: workspace serde_json now enables float_roundtrip.
  Two new real Cue/Analysis JSON roundtrip tests first failed by1binary64 step
  for synthetic230.12125976431182, then passed. Positive/negative/repeated cue
  slots retained. No reference labels enter tests or inference. No algorithm
  agreement gain claimed merely from parsing correctness.
- Current macOS all-feature suite59passed,5ignored; formatting, all-targets/
  all-features Clippy -Dwarnings and release build pass. Linux ARM64 isolated
  offline Rust1.89 snapshot: default53passed2ignored, all-features59passed5ignored,
  release passes. fmt/Clippy unavailable in cached Linux image (not installed).
  Source snapshot unchanged. Evidence .analysis/linux-validation-current-v1/.
  Windows remains unverified; old v7 Linux evidence is superseded for current code.
- REA skill's staged-delegation instruction applied: two workers use only retained
  evidence/static files and isolated private prototypes, never mutate REA target.
  Private key-feature-port-map-v1, energy-feature-port-map-v1 and key-spectral-
  port-map-v1 describe exact known mechanisms and missing functions.
- Key spectral input confirmed sumL+R, N4096/hop2048, paired-real FFT with2x
  scale/phase rotation and a differently scaled final single-frame path.
  Original handwritten radix2 FFT independently implemented in private
  key-fft-port-v1:56/56 synthetic fixtures,74340/74340float32 components exact on
  macOS and Linux ARM64. Does not cover separate waveform vDSP FFT.
- Beat score and2x interpolation private Rust helpers verified exactly on macOS
  and Linux ARM64:72/72 scores and every bucket;13/13 interpolation cases with
 108740/108740binary64 outputs. Original Foundation numeric JSON parsing itself
  perturbed some synthetic decimals, so canonical hexadecimal bit witnesses are
  authoritative. Old mismatch reports retained; no DSP tolerance accepted.
- Original SDK audio-only beat-reference extraction completed282tuning tracks,
  0failures,197691refined beats,210.2s. Refined float32BPM281/282 exact versus1/282
  legacy BPM. One difference is one float32 step,125.9996109008789 versus
 125.99960327148438, not rounded away. Evidence beats-sdk-native-v2; worker gets
  audio paths only, no reference labels. SDKFast waveform differs from main
  BeatAnalysisTransform, which remains required for final fidelity.
- Private feature-conditioned Rust tempo stage reconstructed from source
  mechanisms: normalized legacy beats, >1.23 irregularity bypass, waveform
  onset/autocorrelation, coarse/fine24 search, ratio corrections and six-pass
  period/phase fit. Initial five nativeSDK tempo f64bit patterns match exactly,
  as do initial normalized counts. Parent validation in progress; not independent
  PCM, not production adoption. Final grid trim needs second waveform buffer.
- Native gold key cache v2 generated5cases:88f32 matrices identical to v1,
  per-frame f64 tuning and variable-length frequency/amplitude peak arrays also
  exported/validated finite. No labels or native classes enter production.
- Original waveform amplitudes and melody buffer exported. A native frame has
  f32amplitude,u8validity,3undefined padding bytes; early v4/v5 mistook latter
  field foru32. All15 affected binary files sanitized tozero padding,5tempo
  fixture flag arrays masked0xff. Amplitude/validity unchanged; hashes revised
  explicitly with before/after provenance, no original files modified. Never use
  fullu32 padding as flags or retain uninitialized padding in future exports.
- Clean native-beat-refinement-v6 exporter zeros reserved bytes before writing,
  carries actual f64 bit witnesses, and reproduces sanitized v5 amplitude/melody
  arrays exactly on5cases. stage-fixtures-v2 separates inputs and expected full
  SDK grid/tempo/anchor outputs, ready for next full-grid reconstruction.
- Active REA target SDK DanceabilityAudioEngine.arm64; new retained exports:
  rea-key-front-end-evidence-v4 (tuning helpers), rea-energy-front-end-evidence-v3
  (9missing energy bodies), rea-beat-refinement-sdk-evidence-v3 (coordinate
  helpers), rea-waveform-front-end-evidence-v1 (WaveformAnalyzerFast methods).
  Do not reimport/redecompile these bodies; inspect saved records selectively.
- WaveformFast uses vDSP_fft_zip,N4096/hop1024, frequency-domain Hann-like
  convolution/geometric compression,13frame temporal context and two output
  buffers. Six zero-magnitude frames flush at finish; these are not PCM padding.
  Its native first-buffer sample count2*(floor((N-5120)/2048)+1) forN>=5120
  matched all5. Independent waveform frontend still unimplemented.
- Native jobs25564,33516,83671,95888,12960,80768,29284 and22192 are terminal.
  Host jobs18041/89284/53109/89832 are terminal. REAcell545 completed; target idle.
  Two workers still finalizing sanitized tempo parity and waveform map. No active
  interception, no source audio/library/app/proxy mutation. Full engine agreement
  baseline remains frozen v7:0/282 complete; no fresh full run for parser-only fix.

## User steering: JSON-first cue reconstruction (current priority)

The user challenges spending effort on upstream audio DSP: reproduce the original
sourceJSON, then reconstruct JSON→results; do not require real-file analysis to
investigate the service stage. Explicitly invoked reverse-engineer-anything again
for exact request production. Pause further PCM/frontend work; concentrate on
cached numeric inputs/source tracing and captured JSON→cue outputs.

- JSON-only cue baseline now real: script .analysis/experiments/json_cue_selection_probe.py
  takes request JSON only, no audio and no response in selector. On5captured pairs,
  shared mean9/gap32 baseline matches26/36 integercuebeat locations, one complete
  ordered position list. Not full timestamps/energy/counts/duplicates reproduction;
  no production adoption. Five predefined baselines retained with denominators.
- Protocol audit json-cue-protocol-v1 establishes full decoded score lengths
 768/928/160/384/576; all31nonfirst cues map topositive restoredscores at
  index=beatOffset+CuePointStartBeat. Captured36 offsets lie on4beat lattice,
  repeated beat32slots preserved. Simpletimeformula matches16/36f64timestamps;
  other20 differ1ULP. Bounded operation/FMA/decimal/globalULP hypotheses do not
  close it; no per-track correction fitted. Parent common milliseconds/sample-
  unit variants also max17/36, not adopted.
- Library label geometry is broader:2168/2195 positions within1e-8beats of4beat
  multiples whenfirstcue iscoordinateorigin;27differ (max1beat). Do not force
  all references onto4beat grid ordiscardcases. Sourcegeometry-only report
  json-cue-label-geometry-v1, not independently predicted anchors.
- REA main source trace newexports rea-json-cue-source-evidence-v1/v2/v3.
  Main1000c43ec assembles granules/curve;1000c5744 extracts granules;
 1000c37a8 scores;1000c3d98 selectsONEphaseanchor, not8slots. Defaultpreset
 10005da9c enablespass0+pass1. Pass0 usesmaincustom path/featurearrays2,3 forphase;
  pass1 legacyflag1 calls SDK DetectLikelyCuePointsV3/arrays2,1 and replaces data.
  Thus default final wirecurve likelySDKV3, notmaincustom scorekernel. Runtime
  setting overrides/numerical match still need verification. Parameter/default
  maps in request-cue-recipe-v1; two missingSDKmathhelpers346f4/34f3c pending.
- Original SDKV3 retained tail likewise chooses a scalar32phaseanchor. It does
  not emit the service's8slot list, and int/bool args are unused inoptimizedbody.
  Do not label literal8 asproven slotpickerconfig orassume model-only behavior.
- Cached original waveforms recovered WITHOUTaudio ornative execution:5tuning
  records,324480f32 amplitudes, strictbounds/fullconsumption, byteidentical
  reencoding andf32bitroundtrip pass. Numeric workerJSON has noexpectedcue/path/
  identitylabels; evaluator manifest maps captures. Files cached-waveform-json-
  corpus-v1/waveform0001..0005; originalsinglecaseproof retained separately.
- Cache format2/analyzer2: u32format/u32analyzerversion/f64rate/u32hop/u64frames/
  u32arraycount; eacharray u32count,rawf32amps,u32metadata(representingf32scale);
  finalu32field foranalys>=2. No quantization. FeatureFrameArray SIZE0x50:
  indices1/2 areonset/melody. Earlier maps incorrectly called+0x50/+0xa0 nested
  buffers; addresses/numericparity staycorrect butprovenance labels corrected.
  Nativepadding wasdiscarded asalreadyrecorded; serialized cache has NOvalidity/
  paddingbytes andreader initializesvalid=1.
- Explicit nextaction: usecached arrays andseparate finalstoredbeatcontext to
  reproduce CuePointData values, compareexisting5capturedpayloads, then port.
  This isconditional source-stage validation, not independent PCM/fullengine.
- ActiveREA target switched main→SDK afterexportingcomplete maintrace. Cell562
  currentlylive: decompilingSDK WeightedAverage346f4/DifferenceSum34f3c andthen
  exportingrea-json-cue-sdk-evidence-v1. Do not restart whilehandlelive.
- Earlier privateRusttempo stage sanitizedrerunv2 passes5/5nativetempobits and
 16tests; files/sourcebinarysame. Hostprivate test52356terminal. Noothernative
  audio jobs live. Currentpriority supersedes priorPCMfrontendnextsteps.


## JSON cue-request mechanism verified independently

- REA source tracing and cached-feature validation now agree on the default
  SDKV3 final curve branch. Original-SDK cache-only validation first matched all
  five captured CuePointData payloads and start indices exactly.
- A separate Python math port then matched2816/2816 restored float32 values,
  every packed payload byte/Base64 string, and5/5 CuePointStartBeat values.
  Parent rerun reproduced these totals and both regression tests passed.
  Evidence: request-cue-recipe-v1/captured-curve-parity-parent-v1.json;
  workerSHA8784ffda3a20099b08be79e61e1d68e610498e1c163e1e1fab033e10e5da1350.
- Worker inputs are numeric cached waveform arrays plus explicit stored beat
  context only. Captured request/response labels are confined to evaluation.
  No audio, original SDK, service or REA execution occurs in this worker.
- Important trace correction: vectorA(+0x48) is rescored at zero entries,
  whereas wire vectorB(+0x70) copies the original guarded local scores. The
  wire guard therefore persists. Source-derived onset indices32/32/31/32/32
  explain leading counts48/48/47/48/48 without reading expected headers.
  A separate ceil-based minimum bound leaves every captured start index32.
- Conditional validation only: cached original features/beat context are not
  independent PCM inference. Five development captures do not establish broader
  library or held-out agreement. Server cue selection, counts, duplicate slots,
  energy labels and exact binary64 timestamps remain unresolved.
- Do not assume first stored beat always equals first cue when constructing
  additional contexts: exact on236/282 tuning tracks,46 differ, by up to about
  seven beats. Never silently use a cue label to manufacture an input downbeat.
- Previous pending-helper/live-cell statements above are historical. SDK
  WeightedAverage346f4 and DifferenceSum34f3c evidence is retained; related REA
  cells completed. No additional audio work or interception started.


## Broader cached cue parity, portable Rust, and JSON-only phase

- Previous goal turn: progress (independent five-capture cue payload match).
  This turn: broadened source-stage validation and reconstructed phase.
- Read-only cache recovery282/282 tuning tracks,0failures, byte-identical
  numeric re-encoding. No cue labels in worker inputs, no audio/holdout access.
  Corpus cached-cue-tuning-v1 records explicitly provisional wider context.
- Python versus original SDK under identical context:282/282 payloads/start
  indices/full curves;217408/217408 float32 values.0failures,52.9s.
  Evidence cached-cue-tuning-parity-v1/report.json. Original SDK used only in
  reference processes; independent worker uses no framework or service.
- Separate private Rust port matches all5 real captured payloads and all282
  provisional-context reference outputs, including217408/217408 f32 values.
  Four tests,fmt,Clippy,release build pass. frozen-v1 identity pins source and
  binary. No production adoption; Linux validation is separately in progress.
- Downbeat lifecycle source trace: requestD=first refined beat−codec offset.
  After response, same adjustedDownbeatTime property overwritten byfirstcue,
  then stored grid rephased withCEIL. Therefore savedfirstbeat is not universal
  evidence of original requestD. Retained wire-downbeat-context-v1 covers
  exact getters/setters/calls and assembly. The5 known wire contexts match;
  wider282 contexts remain provisional.
- Parent geometry check: (firstcue−savedfirstbeat)*tempo/60 ≈computedStart−32
  on282/282, including44+1beat and2−7beat cases. This label-independent computed
  start relation is compatibility evidence only; original wireD is not proved
  by it. No expectedfirstcue is used to manufacture any worker input.
- SDK35108 phrase weighting is PRODUCT, not additive triangular sum; prior
  json-cue-phrase-probe-v1 retained as superseded additive hypothesis with
  originalscript and explanatoryREADME. Corrected v2 tested32sharedvariants;
  best still26/36 positions, no production selection rule adopted.
- New JSON-only phase.py implements source tail smoothing/taper,32-phase
  weighting and strict>ties. SDK35200 assembly proves FCVTPSceilhalfwindow,
  so width32 includesoffset16. Redstub258/282 exactphase; final282/282 exact.
  Worker receives CuePointData/Start only, no cached waveform/beatcontext/audio/
  native code/response/labels. Evaluator alone reads native targets/reference
  cues. All1913 nonfirst referencecue modulo4 alignments explained, including
 27 previously offlattice positions across4tracks. This is phase/lattice only,
  not chosen slots or server implementation proof. Evidence json-cue-phase-v1.
- Strict whole-engine baseline remains0/282 complete. Final slot count/ranking,
  duplicates, energy labels, exact timestamp arithmetic and full independent
  audio feature/request production remain open; no UI work started.

- Phase verification tightened and independently reviewed: phase-parity-v2
  requires exact integer inversion of SDK target (all282residual0), uses
  computedStart−32 asorigin rather thanexpectedfirstcue, and gates reference
  integercoordinates within1e−6beat (observedmax1.903e−7). Totals remain
 282/282phaseindices and1913/1913mod4alignments. Priorreports/evaluator retained.
 3boundarytests pass; frozen-v1 identity pins source/evidence/reports. Reviewer
  found no blocking correctness or worker-leakage issue; no position/timestamp
  inference claimed from alignment. Parent independently reran Rust4tests and
 5actual-capture verifier successfully, binary/sourcehashes unchanged.

- LinuxARM64 cached-cue Rust validation completed:5/5captured and282/282
  conditional SDK curves;2816/2816 and217408/217408f32scores, packedbytes,
  Base64 andstartindices exact.2librarytests andlockedoffline releasebuild
  pass. Evidence cached-cue-linux-arm64-v1/parity-report.json; reportSHA
  cdfb91ee3b4117f55b6197ed027b00c600e70a28e9b9e696f499e496f548e3c5.
  LinuxbinarySHA d7c5fb7d6aff976acebe5b9bef420f0041c40b0a777b41c420a386b903a27d2f.
  No pulls/network/packages ormusic/framework/expectedpayload mounts. Source/
  input/publicarchive drift empty. Workerexecsession3553exited0 andcontainer
  a578824e267098a01fcc4295ff4c305ae238405b78e780cd2b8ee840dffcfa8e
  auto-removed andconfirmedabsent. Windows unverified.
- Next concrete mechanism question: SDK procedure-name search onSDK SHA
  a2ab71482503b71edfc5e372b9fcf4d7502f19f808db32ffd07b5eec84de7419
  found legacy full cue-list producers42078/447ac, with inlined ranking and
  transition helper44244. UnlikeV3scalarphase,42078emitsvector<pair<double,int>>
  time/energy entries andusesrequested count(param5) up to24. Requiresenergy
  timeline; emptyenergy returnsnothing. Its relationship tocurrentserviceV3
  isUNPROVEN; sharedcode isonlya candidateuntil numericalcomparison.
  Retainedcomplete evidence rea-legacy-cue-pickers-evidence-v1.json(95records),
  source excerpts legacy-cue-picker-source-v1.42078dossierexceededtransport,
  completedrecordev_e9facb6ebba05b141adc5014212acf99e4216c7828c5eef58fcb0fa7f8d87ec0
  exportedwithoutreanalysis.447acrecordev_db42564451d3765ef15477dcf827318628c42ccc0beb7e35addd8ce05f0da856.
  Worker probingnumericcache+separateCONDITIONALenergytimeline fromcaptured
  response, keepingexpectedCuePointsoutsideworker. ABI mustreturn24byte
  vectorviax8,not8byteV3resultwrapper. Noaudio/service/UI workauthorizedbythis
  experiment; currenttargetSDK remainsopen/idleforfollowups.


## Source-derived cue energy annotation and Rust postprocessing

- Previous/this goal turn remains progress; goal NOTcomplete. Full engine still
  frozen0/282 exact tracks. No UI orproductionalgorithm changes inthisturn.
- Legacy SDK42078/447ac wholepickers failALL5 fullcurrentlists. Nativecalls
 10/10normal, nofailure, fixedcount8; newcounts2/8/8/8/8 old3/8/8/8/8 versus
 8/4/8/8/8. New5/40orderedtimestamps,12/40beatindices; old5/40,7/40. Missing/
  extras retained. Probe frozenlegacy-cue-picker-probe-v1. Source tree exclusion
  andzero-scorestop explainwhyitcannotgeneratecurrentduplicatefallback.
- Separately, sourceenergy tailmedian untilNEXTSTRICTLYGREATERcueindex,
  integeraveragefloor/clamp1..10, matches36/36capturelabels givenoriginal
  selectedTIMES+classifiedtimeline. Savedlibrarytimeline initially2156/2195
  matches becauseenergyboundaries arechangedbeforepersistence.
- Main duringOperation10008dd58(sourceev_e33d2b8168c28c02f22c58bbe27658488c2e1347b63b3e18f3439210ddc2d90f)
  proves nearestINDIVIDUALbeat first, THENnearest16indexrelativetocueanchor.
  MIKAnalysis9514helper exacttiesEARLIER; secondFCVTASpositive tiesUP. UCVTF
  unsignedsubtraction meansnegative wrapnotgenerallysignedrounding; inverse
  V4 rejectsnegativedifferences. Corpusminrelative0,anchors0:238/1:44.
- FirststoredEnergyStart identifiesPOSTRESPONSEcue-anchorclampedgridpoint,
  NOTuniversallywireD. Known5equalwireD butcannotgeneralize. Earlierproposed
  preDrecoveryfromthisfield is superseded. Source saveoperation overwrites
  finalenergyENDwithanalyzer.duration.
- RawStartinversev1single16round recovers279/282, annotation2171/2195 incl24
  failedslots. Source2roundsv2 resolves3 failures withouttolerance. V3 uses
  actualsavedgrid/earlierties/exactmembership; V4 additionallyguardsnegative
  unsigneddomain andindependentlyinvertsinternalEndviaexactstoredlength.
- V4 raw-energy-timeline-recovery-v4:282/282unique recovery,0fail;all2818starts
  exactsavedgridmembers,2536/2536internalEndcandidatesunique. ALL2195/2195
  cueenergies exact GIVENgoldselectedTIMES+classifiedtransitionlevels. Recovery
  itselfreceivesNOcuetimes/labels. ALL46capturedStart/End/Level fields exact;
  Volumeoutside scope. RawFINALendsoutside5 areexplicit native-tailcandidates,
  notinvertible/provenfromduration-overriddenarchive. Noaccuracyexclusions.
- Eight apparentendgaps are1ULP fromnativegetterstart+length. ThinMIKData
  SHA7c90f6bd9a6fb055b999f2dfee42c33c277d2a89d4aa55cde156bfe22b024565
  copiedfrominstalledframework(originalfat99218933b45bcc90bc85e7d45b5160c4c38184b067a977818e58f347c5719f78)
  staticsetEndTimed8a4 storesf64(max(end,start)−start),getterd86cFADD.
  All2536 nextstart−start==storedlengthbits. Noappmutation. Source+review in
  energy-boundary-persistence-source-v1. FinalV4review sourceSHA
  d7f700c964ec357a27a93f69690546640818d65a0be83661330c80345738de93: nofindings;
 6boundarytests pass,282 freshoutputs/hashesequalfrozenresults.
- Private json-cue-postprocess-rust-v1 implements JSONphase andenergyannotation.
  macOSARM64 AND LinuxARM64 exact:5+282phase targets,36+2195energylabels,
 12tests/platform,fmt/ClippyMac,offlinebuildsboth. Sourcefrozenidentity
  e17e8e2080ec3a41197a6f7c8312b1b73bbbf7c7568f8a89a537dac5da290e41.
  Macbinaryb7dc56f5549523f1161c02f63ab4cdaf94b8e9fad3ca2e3d7cbe98ed6b13a4b8;
  Linux55e63c506c570eb529085894f3e4880946dae4d7b0feaf44e3595857fb435c97.
  Completionreport25300ac6bf63aac317cc654ed51b949af53a5a4d60f1755852222b170792849a.
  Parentfresh12tests pass. Source/noise/inputdriftnone; noexpectedlabelmounts,
  networking/pulls/packages;session34814exit0,containerauto-removalverified.
  Windowsunverified. Originaldata/SDK/nativeoracle/livecaptureprocessesquiet.
- REAactive targetNOWMAINMixedInKey11.arm64, analyzedidle. Cell650COMPLETE.
  Complete rea-energy-persistence-evidence-v1.json has97records/18337519bytes.
  LatestSDKexportlegacy-cue-pickers-evidence-v2 has96records13528980bytes.
  Dontswitch/reimportforsamemainbody; source copiedlegacy-cue-picker-source-v1.
- Remaining: exactcueCOUNT/POSITIONS/duplicateconstruction(still26/36 actual
  capturelocationsbestbaseline); energyclassificationfrom183features, key
  classifierfrom352features, exacttimestamps andfullindependentPCM→JSON.
  ComponentGOLDtimelines/context/selectedtimes mustnotbeclaimedasPCM inference.


## Actual short-corner captures, timestamp reconstruction, and selector gap

- Previous goal turn PROGRESS. This turn also PROGRESS:14 actual new query/
  response pairs, exact time mechanism, prospective repeated-slot behavior,
  expanded cache-only Rust parity, broader selector evaluation. GoalACTIVE;
  production/heldout/UI unchanged, engine still notsettled.
- Bounded coarse32/refine4 JSON family32variants max23/36 originalpositions,
  below26/36baseline. No selector adopted. Fixed11 count hypotheses likewise
  do not beat constant8: original4/5, broader249/282. Scalar tupleN896,
  nonzero768,phase0,energy9 states hascounts3and8; collisionscommon while
  duplicatesrare. Frozenreports json-cue-coarse-refine-v1/json-cue-count-probe-v1.
- Recoveredrawenergyclasses produce EXACTSAME6182 per-native-feature labels as
  earlier midpoint classifier study. No need repeatunchangedmodels; boundary
  restoration fixes cueannotation, notexistingclassificationlabels. MLlibs
  unavailabledefaultPython butNOinstall attempted. Modelresources inventory
  onlysep01_medium/high model.mil; no claimaboutclassifierweightavailability.
- Capturedlicensedapp4short8/16/24/32s+64sloop(v1), fullPCM-identicalcontrol
 38.37s+40/48/56/96sloops(v2), then4PROSPECTIVE controls3othertuning excerpts
  +250ms-shiftedloop(v3). AllHTTP200, originalpayloadsunchanged, no replay.
  Audio-only FLACs; stagev1initialartworkdiscardedbyremuxwithdecodedPCMhash
  equality, stagedartworkcopiesNOTimported. Allsource-copy hashes unchanged.
  Sourceapp now454tracks (440before+14disposabletestentries), analysisqueueReady.
  Analysis/tag/exportpreferences unchanged; nooriginalmusic/bundleedits.
- Capture processes: v1PID11002/session71180exit0;v2PID47483/session40743exit0;
  v3PID80855/session9035exit0. Alllisteners48890absent. Existinguser-enabled
  mitmredirector reused ONLYappPID4647+online5host/IP20.50.2.31; localweb127.0.0.1,
  nonewCA/security/systemproxychanges. All28 newrequest/responseJSONredacted;
  metadata/flows sanitized. Freshselftests6guardseach, query/TLSlogsalsotargetonly.
  Audit cue-short-oracle-v3/final-capture-audit-v1.json. No liveinterception.
- Full38control matchesALLcomparedoriginalnumericrequestfields, CuePoints,
  EnergyLevelSegments,OverallEnergy,Segments,KeySummary exactly (Eliteidentity
  excluded). report cue-short-oracle-v2/control-repeatability-v1.json.
- NewallzeroN96/128curves return1cue. 16sCueStart31 firstcueoffset−1 relativeD,
  actualproof cue0doesNOTalwaysD; savedfirstbeat can differfromwireD. 64/56s
  N192positive64scores return[0,32,64];96sN256positive128 returns6positions
 [0,32,64,96,116,136]. Full38/40/48N160positive32all8duplicatepattern
 [0,32x4,36,40,44]. Fullphase32offset DISPROVEN:64loopphase16 butcues0/32/64;
  mod4alignment remains supported. json-cue-short-json-v1 contains10caseaudit.
- BEFOREv3queries frozenprospectiveworkerSHA
 4a79bb7c8f50226ccf5fab7da792dc83e506d914726961b16439d9bed849cfb7,
  plan289ea6bb3ef0660f1251a22ac59656b978a4d6e6c1a0a3437d55255691b9d484.
  Rawzero→[F]; positiveRaw+96<=N−F<160+proposedpositionsinsidepositiveSupport
  →scoreindicesF+[0,32,32,32,32,36,40,44]. Prospective4/4full8slotlists match.
  Geometrysource-backedbutfallbackinferred; validatedcellONLYN160,F32,
  remainder128. WiderpositiveF!=32/rangeedges UNVERIFIED. Noformulaeditsafterdata.
- TIMESTAMPBREAKTHROUGH sourcec6070two-point extrapolation: p60/T,t0D,t1D+p,
  step=t1−t0; b>=1: t1+(b−1)*step; otherwiseD+b*step, separateUNFUSEDf64ops.
  Independentworker108/108bits on19pairs, includingnew32/32after15/76freeze;
  direct35/76 andfused67/76. SDKARM64sourceFMADDsoactualbackend/runtime/Windows
  stillunproven; synthetic2pointconstruction+unfusedorder observationallyexact.
  Integerselectedpositions suppliedcomponentinputs, notinferredbytimehelper.
  WorkerSHAef419aad8da28becff24c66feaf66b94445897d9b451934b170b38e9ea425b0d.
  Originalproof json-two-point-time-v1 froze40files; extension-v1 froze45,
  ALLoriginalfilesunchanged. Sourceev_dc4be32ad3a0cf9d582317d8cd91e1c512510ad2c1152c73857207513210d7d6.
- OfficiallegacyCoreCLR parser hypothesis ruledout: port10013/10013published
  compiledC++ casesexact(226distinguishmodernparser), butactual20D/T+43time
  lexemesallunchanged; directtime21/43stillfailed. No.NETbackendclaim. Frozen
  json-dotnet-number-v1; source-onlyprimaryresearch; nofurtherparserversionsearch.
- RustpostprocessV2 NEWdirectorypreservesV1. Initial15/76times exactMac/Linux,
  originalphase5+282/energy36+2195corporaall589CLIinputsexact. SourceSHA
 89705778c43bd62715cd08626376fca7178524c686a37c2c27e2dd0e119936c1,
  frozenidentity8028acc0fa54136907c017ac59ff91416d2e5a9e9851f437e7bd318f14c6f2f5.
  Macbinary74ef7037494b09b8fcc9bb038743e5861789fbc2a9f09c9ce7bf8e52b3d53a5f,
  Linuxff60e4055c87a87c739e4fab974bf10c3a2cbda463a03efb61187d6de9a653c1.
  Mac19tests/Linux18synthetic;fmt/Clippy/buildfresh. InheriteddecoderV2as_chunks
  lintfixonlyV2,Rust1.89compatible. Noexpectedgoldmounted; Linuxcontainerremoved.
 19/108extension beingvalidated separatelywithUNCHANGEDbinaries; awaitreport.
- FrozenRustSCORERnewactualcachevalidationMac14/14,2208/2208f32payload/start
  exact; cumulative19actualcapturecases5024/5024f32. CachedcontextusesACTUALqD,
  savedpostresponsebeatlist; 16sD!=savedfirst stillpayload/F31exact. Noaudio/SDK/
  servicecallsinvalidators. Report short-cue-cache-validation-v2. Initialnew10
  alreadyLinuxexact1568/1568, cumulative15/4384, frozenLinuxbin d7c5fb...; first
  Linuxattemptschema-onlyrejectedall10(noanalysis), correctedattemptretained.
  Last4newLinuxproof requestedpending; do notclaim19Linuxyet.
- json-cue-hybrid-v1 combinesfrozencorners+oldmean9/gap32candidate8beatgrid with
  phaseMOD4+exacttimehelper. ExpectedcuesNOTworkerinputs. All19actualcases:
 103/108multiset timestamp overlaps,109predictions,5missing+6extra,15exactlists,
 17counts. ORIGINAL5=32/36matches,2/5exactlists (previous26/36,1list).
  Independentreviewconfirmsnogoldleakage; prospectiveevaluatorisROOTexperiment
  evaluate_prospective_corners.py (notinsubfolder); review_selectors.py separate.
 3compositiontests pass(scoreStart31,duplicatespreserved,targetfieldrejected).
- Broadervalidationhybrid:1808/2195integerpositions,2254predictions,249counts,
 51/282completepositionlists,5.34s, allcasesincluded. Provisionalcachedcontext
  meansNOtimestamp/end-to-end/heldoutclaim. SourceSHA
  d3a678207038b8ce44dde3c2a25ce78dc442bb96a9616bf28ad5072fec9ddc20.
  Bettershort/timecoverage doesNOTsolveordinaryranking. Source/outputsprivate;
  noexperimentalpickeradoptedinproduction. Fullgoalremainskey/energy/BPM/cues
  perfect plusportablefullPCM→JSON, thenUI.
- Allprimaryexecjobscompleted. Agentscopekey_feature_port_mapcurrentlyrunning
  unchangedRust19timeextensionandlast4Linuxscoreextension. Energyagentcompleted
  independent19time/selectorreviews. REAactiveMAINanalyzedidleunchanged.

### Independent cached key-request aggregation and completed cue extensions

The previously pending unchanged-binary cue checks are complete. Both macOS
ARM64 and Linux ARM64 reproduce all108 timestamp bits across19 actual captured
cases, conditioned on supplied selected integer positions. Linux also reproduces
the last640 scores, bringing the actual captured-context total to5,024/5,024
float32 scores across19 cases, with exact packed payloads and start indices.
These cases include related controls and are not19 independent music groups.
The separate extension preserves every original proof. Its report SHA-256 is
`e9ff6be84c8544625386401bfb962777b5967201a34d41c09c9fad4a117ab618`;
completion SHA-256 is
`19227c46c8ebea90a70c369e606c26b41de8c8c678458638f3cf99fd0c08e039`.
The network-disabled container exited0 and auto-removal was verified.

New experiment `key-request-aggregation-rust-v1` independently implements the
cached88-frame range and bank-accumulation stage. Five original captured requests
match352/352 values each, **1,760/1,760 binary64 values** on macOS ARM64. Inputs
are original cached spectral frames and archived beat context, supplied as exact
numeric bytes/bits. Expected requests are evaluation-only and opened after each
worker exits. The worker links only libSystem and reads no audio or framework.
This replaces original-component execution for this aggregation stage only.

The implementation preserves the original reversed time-to-beat comparison,
full-bar extrapolation, fractional clipping denominator, three-frame medians,
separate boundary-coefficient arithmetic, fused accumulation and ordinary bank
addition. It uses the full-file segment path; arbitrary multisegment behavior
and signed-zero/nonfinite edge equivalence remain unproved. Three boundary tests
were observed failing with the zero-output stub before implementation; all eight
final synthetic/contract tests pass. Formatting, Clippy and release build pass.
Clippy initially rejected constant `chunks_exact`; the decoder-only `as_chunks`
fix and extra tests preserve every prior numeric output, with the original
five-case report retained unchanged.

The source SHA-256 is
`285a32d4814f20c61b5f38849b4b91077a2cfb427a0fa3618e9c5154cdf95fa9`;
macOS binary SHA-256 is
`b16368ee5a84d4b92ed392699eb623af68ff8086ba1747b1e5f5a2cc9ec7ad09`.
The24-file freeze identity is
`abc8c31aea7a4b0c5b51a4b8221b621b7df8291eb6ef972f77aac4bdb7918c84`.
Linux ARM64 validation against that freeze is also complete:5/5 vectors and
1,760/1,760 binary64 values match, including bar headers and complete output JSON
semantics. All eight tests and the locked offline release build pass. Dependencies
were already cached, networking was disabled, and reference outputs stayed in
the host evaluator. All24 frozen files remain unchanged. Linux binary SHA-256:
`55fd0cf025d79f0431db3e6aa9046690219c4c9982e2a93dafeb1c56d3412206`.
Windows execution remains unverified. Source scheduling review uses retained
REA evidence; no target switch or original music/app modification is required.
The Linux parity report SHA-256 is
`8887b608b2d6154ddf760441b57e7fe9e1e254fb7642b436fe801aecc143155a`;
completion SHA-256 is
`2798316a75dcff16225f14c58bea56b132694610d9bc8396017b3e945e0db38a`.
The container exited0 and auto-removal was verified; no validation jobs remain.
The separate retained-source review found no blocking implementation issue in
the stated finite-cache scope. It confirms the exact main-image interpolation
order, reversed conversion, floor/ceil schedule, negative-bar bank rotation and
range coefficient/accumulation arithmetic. Its14-file evidence set is frozen
separately; the24 implementation-proof files remain unchanged.

Independent PCM-to-key-cache extraction and the cloud key/energy classifiers
remain open. Ordinary cue ranking still gives51/282 exact lists in the broader
cache-conditioned development check. None of these experiments changes the
production benchmark or establishes perfect engine agreement. The goal remains
active, with UI deferred.

The next source-reconstruction boundary is tracked spectral peaks plus finalized
tuning -> cached88 frames, using the five existing private intermediate exports
for component evaluation before undertaking another PCM run. Exact aggregation
does not supply that missing cache-generation stage or classifier weights.

### Spectral-peak-to-cache reconstruction and portable power calculation

This goal turn advances that boundary. The independent default44100/N4096
finalizer matches5/5 original Mac cases,27,044 frames and all2,379,872 float32
cache values from original peaks and finalized tuning. Its two-stage Mac chain
also matches all1,760 outgoing key-feature values. No original cached88 data is
required by the chain's workers. Source review preserves89-cell grids/raw guard
cells, exact suppression order, pitch/tuning precision and startup alignment.

Strict Linux v1 failed1,206 values; four numerical v2 variants also failed the
original reference. All failed proofs and exhaustive differences are retained.
The failure was traced to platform log/power approximations. Decimal100/200-digit
midpoint analysis shows the examined Mac power results are not nearest, so
promoting precision changes the target behavior. No relaxed tolerances or
per-input lookup corrections were adopted.

Handwritten `darwin-powf-port-v1` now matches22,729/22,729 native synthetic power
results on macOS ARM64 and Linux ARM64, including subnormal inputs. Eight tests
pass on both platforms. It calls no host log/pow/exp and reproduces the pinned
positive-finite approximation, not the complete IEEE powf domain. Runtime
code/table witnesses come only from an agent-owned Python diagnostic process,
without app attachment. Raw observations remain private.

`key-cache-finalizer-portable-v3` combines this frozen power module with the
measured double-logarithm-to-f32 path. Mac and Linux cache both match all
2,379,872 original float32 values,5/5 complete cases, with zero differing bits.
All17 tests/build pass on both platforms; Mac formatting and Clippy pass.
Mac and Linux composed requests both match all1,760 binary64 values,5/5 cases.
Linux uses freshly computed V3 caches plus only projected numeric archived
contexts and the unchanged aggregator55fd0c...; no original cache fallback.
Its proof is in key-request-composition-v1/linux-arm64-v1 (the report explicitly
pins V3 source/binary; the older folder name does not indicate v1 math).
The Linux cache worker session28999 and
containerd0426d3ee287 exited0. Binary SHA-256:
0b9b7f10d0388efce1df36362d031c67b58b903bec1bc42bd4099bf143cc1b1f.
Cache report SHA:73984cf456198c5fcad06f53eb7647b229c73c6621cdff5f4d99796d4b381235.
The composition container42cc528e3693 exited0. Source reviews found no blocking
issue in the bounded finite scopes, and all frozen prior versions were retained.
Both cache/composition containers were confirmed removed; no validation process
remains. Composition report SHA:
a4689d626cc9ef95eb33314ed801c121479823e65066bc9032306582e82e4293.
V3 root completion SHA:05e5fc0c92ddb48e36cd951e14b969a76884d36d6682c56ad517eb1610edde26.
No Windows/all-floating-input/PCM/classifier claim is established.

Detailed mechanisms, failures and frozen identities are in
[key-cache reconstruction](research/key-cache-reconstruction.md). Private proof
folders are key-cache-finalizer-rust-v1, key-cache-finalizer-math-v2,
key-cache-finalizer-portable-v3, darwin-powf-port-v1,
key-cache-libm-probe-v1, key-cache-libm-promoted-probe-v1,
darwin-powf-runtime-v1 and key-request-composition-portable-v3. No production
engine or UI changes; held-out music remains untouched. Full goal stays active.

### Independent finalized tuning from retained peaks

The previous goal turn made concrete progress with exact Mac/Linux cache and
request reconstruction. This turn removes original finalized tuning as an input
to the composed macOS request chain. New `key-tuning-rust-v1` matches all432,704
native score values and27,044 original binary64 tuning values across5 cases.
The three-stage Mac chain—peaks ->new tuning ->new cache ->request banks—matches
all1,760 key-feature values, conditioned only on retained peaks and archived beat
context. No original tuning/cache fallback, PCM/SDK/service in the Rust workers.

Separate `native-tuning-from-peaks-v1` proves the same retained peaks suffice for
the original component. All5 curves and27,044 original bits match; the432,704
score values are exported beforeFinish and exactly match Rust. Dlsym does not
expose Tuning methods; retained failed guards led to validated C1 constructor
anchoring (C1 at1aebc, distinct from C2 at1a578), installed-slice hash and entry
byte checks. No audio, app attachment or credentials were involved. All11 native
real/control processes exited. Six zero-peak controlsN0/1/2/16/17/33 match all
12 Rust jobs:69 f64 tuning values and1,104 f32 scores. Nonempty zero-score curves
are nonzero; the source optimizer's first-tie rule is preserved.

Eight tests/Clippy/format/release checks pass on Mac. Three initial tests were
observed red before implementation. The underflow-ratio guard rejects nonfinite
intermediates outside the supported domain; final test-only changes preserve the
release binary. Original5-case proof/source hashes remain alongside final replay.
Rust73-file freeze3e5b064dec656f405b49113bd0e7ff0167fe08bdd7c49919c15b30c237c87bb5;
source31cb7f23914bba734ad492d36f6b263e5dc615ecf2256c4c0e5f0dd1da918e88;
Mac binaryb5e168ed4589a3bc156684840c765cf2c6971014357de98448097d6431dc92cd.
Native73-file freeze686ab7661af3c2d99ba7089f6b25feede5cc877483df64897c106a726e020c35;
native replay reportfc40f5cf5b0cba21be9a058a6461dce502e4651c2da675be536524f49d1357e7.
Three-stage Mac7-file freeze0fa99fd8f0eb772333a469c4d446e908079e588b85df245a749d9390440760e2.

Initial Linux22-job run (session72588/containera7ec1329988a) failed15,007 raw
score values, though all27,044 tuning values and controls matched. Failure report
76f53820fd1299b2985fdb8bee3ecf0bcff09014a8312d143682aa2501211816 and exhaustive
differences are preserved. The container was removed; no chain was accepted.
The isolated wider-log Mac probe also failed11 raw score values, maximum16ULP,
so Linux was deliberately skipped for that diagnostic.

New `darwin-logf-port-v1` reproduces the pinned positive-finite logf path using
65 mathematical pairs/four coefficients and exact normalization/FMA ordering.
Own-process runtime witness code1876541f5271f47b292e875bb5e738082cb95aa881e408f77d8d7fca09e52953;
no app attachment/SDK/audio. Mac/Linux scalar53,492/53,492 exact, all5 tests pass,
no host log/pow/exp/trig/root imports. Freeze22files
c59ff0a984c386d836764cf13006d82ae27021108ee52a6f3e278344e514c204.
Linux helper report739579cdca12c340f331855bfd37cc5ca815d99731f98a0358b177533ab36c50.

`key-tuning-portable-v2` copies that logarithm byte for byte. Both Mac/Linux now
match every real score432,704/432,704 and tuning27,044/27,044, plus all12 controls
(1,104 scores/69 tunings), zero bit differences. All13 tests/build pass; Mac
format/Clippy pass. Freeze73files95f7f3ecbabb0871f948f1f1835f33813dc03f4d31c3e787eeca937f913da8d4;
source d824438b961b855585ae5d3e9b5ed1aaac000951f727a3ce85ac66dd3be84cf1;
Linux binaryfaf7c8e0dfea753f4a57edde626cc65a6a7ef318ad07ba898d35f75340ba3295;
reportdcdf2274380f8b1ee28167feb80a38fd83377b28dab29cc9db8162a6a1a5c5e2.
Session4106/container548257007bb8 exited0 and removal verified; no source/input
drift or gold mounted. A source-faithful cacheV4 using the same logarithm is
being frozen before new Mac/Linux three-stage chain validation. Existing V3
proofs remain unchanged. Double pow/cos remain scoped numerical dependencies.
Full mechanisms and scope: [tuning reconstruction](research/key-tuning-reconstruction.md).
No production engine or UI changes, no new audio/held-out access. The full goal
remains active; independent tracked-peak extraction and service decisions remain.

The final source-faithful cacheV4 is frozen40files at
9a2ea4a959b616cf2143d2733e0f4893712105db2379f564461a6cfd4d54afed. It replaces
the promoted logarithm with the same byte-identical source-derived f32 logarithm
used by tuningV2; the raw power module is unchanged. Underflow regression was
observed red then green. All23 tests/fmt/Clippy/build and5-case Mac/Linux cache
comparisons pass:2,379,872/2,379,872 f32 values. Mac source
8eabe3681f4a5f3322fd49bafda28f8eb88240ea5a9a85ba5a19993c8392371a;
Linux binary117fae2c4bbf7044b3cd2146bbb243202ea02ab3c7957b7ff0aceffdc4f0b0da.
No hostlog/logf/pow/powf imports in cacheV4; cos/exp2f remain. Root reviewed the
diff: only log module/guard/regression additions; no other cache math changes.

Both final three-stage chains now pass5/5 cases at every newly computed stage:
27,044 f64 tuning values ->2,379,872 f32 cache values ->1,760 f64 request-feature
values. Workers need only retained peaks and projected archived beat context.
No original tuning/cache fallback or expected vectors were supplied. Linux
dataflow audit confirms the two numerical bridges and unchanged bank55fd0c... .
New chain folder:key-tuning-request-composition-portable-v2; Mac7-file freeze
f26525e2bad3c00362b7e08683cd85ea908356702935524510788ccbd9adc94b.
All prior failures/source freezes are preserved. Final Linux cacheV4 report:
81601cc54947253c35fb9c3553624363f90c639287eb73a3c91d649351844598;
fresh three-stage chain report:
3ba61551c306cc32eac816c5ebe4ca29916f116ef9753ec4a26024ea3b19ea50.
All four Linux containers were confirmed absent and workers completed. Root
combined completion record key-tuning-portable-v2/completion-v1.json SHA:
e767a54f7e0af7524dc168cd33ed25ef8ac318d7b5657f4ffc1383f6ed252e61.
No active jobs, production edits, audio/held-out reads, or UI work remain in this
milestone. Full goal remains active; next source boundary is tracked-peak and
beat-context inference from audio, with final classifiers/cue selection still open.

## Independent spectral peak tracking and portable exponential

This continuation makes implementation progress toward the original engine goal.
The source gate for harmonicity candidate correction, mutable-magnitude local
suppression and three-frame tracking is now implemented independently in Rust.
No production component or benchmark has been replaced by this experiment.

On macOS ARM64 and Linux ARM64, portable tracker V2 matches all 57 synthetic
sequences / 327 frames: 5,936 newest-candidate scalar values, 14,796 complete-queue
scalar observations and 3,904 retained scalar values. The 25 initial controls
and 32-case independent-FFT/dense-spectrum extension remain separate cohorts.
The original direct/default paths also agree throughout the extension. The
native transient pre-suppression index list was not observed.

Fresh independently generated retained peaks now feed the unchanged tuning V2
worker. Both platforms match all 5,232 float32 raw scores and 327 binary64
finalized tuning values on all 57 sequences. Original peak/tuning arrays never
enter those workers. Expected outputs remain in the external evaluator. This
synthetic proof does not remove supplied original peaks from the earlier
five-real-track request-feature proof.

The first tracker version failed Linux strict parity on five dense cases:
54 amplitude observations differed; counts and frequencies matched. That
499-file proof is frozen unchanged as key-peak-tracker-rust-v1, identity
ba50f743bdfa418a5b31abf832da2bb2c47cf8791cdf8c263ae236ec96333f5f.
A 2,276-case own-process math diagnostic isolated exp2f: 19 helper values differ
by one ULP, while hypotf matches; helper-only recomputation explains every Linux
candidate amplitude. Failed reports and outputs remain retained.

New source-derived finite exp2 uses the pinned runtime's magic-shift reduction,
adjusted mathematical table, double polynomial and FMA sequence. No host exp/log
or fixture lookup is used. All 308,459 native values match on Mac and Linux;
four tests pass on both. Freeze identity:
708bd95e4fd13cf1152f625b6d1ba89854b664ae5ec3bdc36d33e5d3b26cbd13.
The tracker copies this math source/table byte for byte and retains host hypotf.

A tiny 1e-30 spectrum probe produced NonfinitePeak in the native oracle and no
valid reference vector. An inferred drop-candidate policy was rejected. The
prototype explicitly errors before tracker state mutation on nonfinite
correction; exceptional-domain equivalence remains open. Earlier versions and
the failed native edge probe are preserved rather than counted as passing.

All 19 tracker tests pass on each platform. Mac format/Clippy/release and Linux
release pass; five final malformed/overwrite checks pass. The root workspace
passes its 53 tests, with two explicitly opt-in private parity tests ignored,
plus format/Clippy/release. No music, CSV, held-out data or installed bundle was
modified; no UI work or service request occurred. The sole new Linux container
was stopped, removed and confirmed absent after all workers completed.

Proof directory: key-peak-tracker-portable-v2. Its 972-file freeze identity is
26b167fe9c675f0572b1cc8dcf09708213c530873abba6698a673b6787311851;
Mac binary 07e89a1d1886a55c98de69baafd677ff16194eb70fa686709e0ead84b530d2ad,
Linux binary 686aaea6aa38e0b726022058c3c03d2e3b096263be48c98be30734df4e9b5487.
Full mechanisms and scope: [peak reconstruction](research/key-peak-reconstruction.md).
Next is channel-sum
PCM feeding and original dual/single FFT scheduling, then independent real-track
peaks. Decoding/resampling, beat context, classifiers, final cue selection and
full engine agreement remain unfinished. The overall goal remains active.

## Independent real FLAC to outgoing key features

This continuation is implementation progress. The PCM feeding and real-track
peak boundaries from the previous entry are now verified for the stated profile.
No UI or production inference replacement occurred.

The new independent PCM frontend reuses frozen FFT/tracker/tuning/cache sources.
On both macOS ARM64 and Linux ARM64, all 40 synthetic cases match every compared
state: 1,294 feeds, 343 retained frames, 6,860 peak pairs, 5,488 raw scores,
343 tuning values and 30,184 cache values. The 36 original-512-feed cases and four
alternate schedules are separate. Buffer digests, signed zeros, channel sums,
cursor/compaction state and every emitted row match. Finished adds only complete
remaining single windows, preserves existing rows and flushes no tracking tail.
Fourteen component tests and six malformed/overwrite guards pass.

Read-only source mapping found and reused the existing independent packet
boundary component. The original FLAC converter emits 4,096-frame batches and
does not flush pending converter audio at EOF. Its packet-dependent behavior
drops small tails on four development files but preserves the fifth partial
tail. Independent Symphonia decoding verifies the complete 44,100 Hz/stereo/
16-bit sources and derives the analyzed prefix from packet lengths, not labels
or supplied per-track cut lengths. A separate native decoder-only own-process
reference hashes all valid interleaved float32 bytes without an analyzer or PCM
dump; original source content and metadata remain unchanged.

Both new independent real-file workers match all five development copies:
complete PCM digests cover 110,795,692 float32 values / 443,182,768 bytes, with
108,204 read sizes including terminal zeros. All 27,044 retained frames,
749,737 peak pairs, 432,704 raw tuning scores, 27,044 finalized tuning values and
2,379,872 cached feature values match the native references exactly on both
platforms. No original peaks/tuning/cache or expected labels enter these workers.
Two decoder tests, format/Clippy/release and three input/overwrite guards pass.

Fresh audio-derived caches now feed the unchanged independent request bank
aggregator: all 1,760 outgoing binary64 key-feature values match on both
platforms. The final bridge derives frame count and duration from audio as well;
ONLY the original refined beat grid and downbeat remain supplied. Earlier proofs
retain their original broader input conditions. This is not complete independent
request generation or final key classification.

The root workspace passes all 53 tests with its two intentional opt-in private
parity tests skipped, plus format/Clippy/release. A separate read-only reviewer
found no material correctness, leakage or overclaim issue and rechecked retained
comparisons/hashes/source snapshots. The sole owned Linux container and its
temporary tuning audio copies were stopped, removed and confirmed absent after
all workers completed. No holdout, original music/CSV/app modification, service
request, new proxy or UI work occurred.

New frozen identities:

- key-pcm-inputs-v1: 43 files,
  f8d0e05d604c11ad6568f5b3d5c27d4d5f578df74d35b8631031572b4b594c8a.
- native-key-pcm-stream-v1: 67 files,
  cd22f6ba6efbedec1ccfe76c3abd042618e7c1962a6eb205ad2a8f776d4daf6d.
- key-pcm-stream-rust-v1: 178 files,
  b34c574889538c95ea2acfd677d0738e54624cbc69326037021a0ba2ee320f3a.
- key-real-pcm-source-map-v1: 17 files,
  14af8b9ed3b74fe6032bd0741c708b626febad18dd629c36277ff3a3fc71b597.
- native-real-pcm-digest-v1: 17 files,
  5c65291d16f259b649b66f14d4c9b7c6c0efbd3a537bb2f8357c189afbb4693d.
- key-real-stream-rust-v1: 146 files,
  56b19efb673ffa91b1b06775f002e75dd5a34be07b6986f00fc9b2d319a2132a.

Real worker binaries: Mac
0067cb97a3e3f601bb831da506c8fffd9d98d4c017892829db86f93fe009cfff;
Linux 41df5d8e42926de5c47279d2cbad1bd7b235b6bb73d1a41db846214cd21ac598.
Final audio-duration key-request reports: Mac
277648e94d28082bb1835c72586f6ca3cb438c411adfb1a411aa6455aab99257;
Linux c1e2aa907a76448151b2207c58773bb96ab22a0d9ed21e391b4d54e37862776f.

Full mechanism and limits: [real-audio reconstruction](research/key-real-audio-reconstruction.md).
The next independent timing boundary is exact BPM/refined-grid/downbeat
production. Energy/cue frontend reconstruction, final models/selection, broader
format/platform validation, production adoption and full-engine parity remain
unfinished. The original goal remains active.

## Normal waveform source and FFT numerical gate

The normal main-executable waveform route is now mapped separately from the
SDK fast analyzer. Static evidence covers its upstream float64 stereo mean,
initialization, dual-window FFT framing, six populated feature arrays, context
filters and finalization. The private 71-file source freeze identity is
6b0c897c938af8d50dfd212deb4756f756d1731221c294a4d163d09c52e48fa5.
This is source recovery, not independent waveform parity. Legacy classic beat
API and core DSP bodies have also been retained; their independent port and
full timing composition remain unfinished.

A new own-process Accelerate probe tests the raw waveform FFT configuration
against the unchanged independent key FFT on eleven fixed synthetic complex
inputs. Only two cases match all bits: 68,873 of 90,112 scalar components differ.
This establishes that the verified key FFT cannot simply serve as an exact
waveform FFT replacement. No tolerance was substituted for the bitwise gate.
Eight analytic/domain/overwrite controls pass with their documented scope;
an initially invalid constant-spectrum exact-zero assumption is retained as
a failed control, and the corrected control reports non-DC residuals explicitly.

The new private numerical gate has 50 retained files and identity
83394f99fdd0b2352c261453aa175512f7fc4f660522b5c7a13964df40989ba9.
A read-only review independently confirmed source identity, FFT configuration,
output counts/hashes and the bounded interpretation, with no material finding.
No production code, original application, music, CSV or holdout was changed.
No original application/SDK execution, service request or UI work occurred.
This diagnostic is macOS-only and does not claim full waveform, music or
cross-platform parity. Previous production measurements remain unchanged.

Next is the portable waveform FFT profile and independent waveform parity,
alongside the classic beat/refinement/downbeat path. Full request generation,
server classification and final cue selection remain open. Details:
[timing reconstruction](research/beat-waveform-reconstruction.md).

## Portable waveform FFT and geometric power

This continuation resolves the raw aligned FFT numerical boundary identified
above. It does not complete the waveform frontend or independent timing.

Own-process system-library source tracing found two FFT numerical profiles
selected by 64-byte buffer alignment. All sixteen float32 displacements were
measured on eleven synthetic inputs (176 calls); nine inputs distinguish the
profiles. The aligned profile matches the earlier system oracle. No original
application/SDK, real music or service was executed in these probes.

The independent Rust table generator computes all 16,096 required coefficient
words exactly on macOS ARM64 and Linux ARM64. Separate first and middle stage
ports each match eleven cases / 90,112 float32 components bit-for-bit on both platforms.
The complete scalar Rust FFT now matches those same cases and a fresh forty-case
overlapping S16-shaped extension: 51 cases / 417,792 exact float32 words on each
platform. References and captured tables remain outside the workers and Linux
container. The full source-only final-stage proof matches all 8,192 expression
components, including arithmetic grouping and output scatter.

The first complete candidate failed 16,209 component words because its final
real-component accumulation reused the first-stage order. Source tracing showed
the final stage accumulates cosine before sine. Correcting that ordering passes
the strict bitwise gate; failed source/binary/results remain retained and two
numerical regression assertions preserve the distinction. No tolerance was added.

A separate source-derived fixed-exponent double-power helper matches all 9,075
synthetic binary64 and narrowed binary32 results on both platforms, without
host power/log/exp calls. Review found a permissive historical evaluator could
false-pass truncated references. A separately frozen strict audit supersedes
it: twelve guard tests pass, and complete count/identity/hash checks confirm
both 9,075-row results remain exact. The original frozen files are unchanged.

Independent reviews found no remaining material issue in the bounded table,
first/middle/full FFT or scalar-power claims after that audit repair. Component
tests pass: table 3, first 4, middle 3, full FFT 5 and power 10 on each platform;
strict audit 12 on the host. Mac format/Clippy/release and Linux release pass.
Linux Clippy is unavailable in the existing image and is not claimed passing.
The default production workspace separately passes 53 tests (two explicit
private parity tests skipped), format/Clippy/release; its predictions and
published accuracy measurements remain unchanged.

New completed private freezes:

- FFT tables: 65dabbce493ef52fa5e5abfc0c5f998dab5b5e26e51836565c35be138c0d4ad9.
- FFT first stage: c07a5eec7faf1598a28848e1e0472f4bf6c16830fbf2022283011b1903edf65b.
- FFT middle stage: 95bc781e35e0f5a15a13d8d19355aa555ae5b836533a26f179c93c073beee938.
- Complete FFT: ef61a9d6de57e3e9243f6aa03b0b10fef773b0361c5b4a098b4919f62ac38f7a.
- FFT extension: 81e959eca2f14e9902043db28f13396d0d8f1c3545f58e18c32d9f28eb80648a.
- System FFT source: 265e375a7a925d3edb61cc90e166b671a0b5c1af3c417c33a020906d1f3f2c0d.
- AMX source proof: 58b23c8ce0b844a9d8921947b4d3daec9f45aca81460fcac8d2f94f7d7446955.
- Power source: a6cf5b59798ac561465d50cf5f2919325b32e20fb224246f54700f17125a2f28.
- Power Rust/Mac: c4c04590056be173acae8ba70890ea28bffa81afac1de1165ad3bc55d6301b42.
- Power Linux: 509b3d4643fd9d86d6904cb15035483df006f0a3a24f3df467f68c61db2e582a.
- Strict power audit: c643f15f5ef40817287d949876a082652057ce7b32b150a1fcfac2fc12000fcc.

Complete FFT binaries: Mac
3b1199252125874227469a2206876dbf0ba7e76a3b831a98f6c67231af4c82cc,
Linux 0ad02291569c233864f1dad1de345f3c3cf3671dc992d50527d0ce591b3c8add.
Actual app waveform-profile binding, full context filters, original cached-array
parity, legacy beat/refinement/downbeat, server models and final cue selection
remain unfinished. Windows and other FFT sizes/profiles are unverified. The
next step composes the normal waveform frontend and validates its arrays before
using them as independent timing inputs. The overall goal remains active.
The sole owned Linux container was stopped, removed and confirmed absent after
the numerical workers finished; no reference outputs were transferred into it.

The next spectral window/magnitude/rebin Rust stage is now source-reviewed with
14 library and two CLI tests passing on Mac, plus format/Clippy/release,
six synthetic smoke cases and five rejection controls. Its fixed profile maps
768 complex bins to 207 scaled magnitudes. Native numerical parity is untested;
neither a complete waveform nor real-track improvement is claimed from these
checks. Context filters and full cached-waveform comparison remain next.
Magnitude-stage freeze: 45 files,
ce15fbc783ccb48db525b9dfeb4cdf64eb9cd40061db6cf5476926817f21cef7.

## Complete independent waveform and outgoing cue composition

The next composition gate now passes on five development audio copies on both
macOS ARM64 and Linux ARM64. Independent Rust decoding, analyzed-prefix
scheduling, stereo averaging, paired FFTs, magnitudes and context filters
reproduce all 324,480 waveform float32 amplitude words across 54,080 frames per
platform. The eight array shapes, scalar metadata and binary64 duration match
as well. All 11,194,560 intermediate magnitudes agree between platforms.

Encoding those computed values and audio-derived frame counts reproduces every
one of the 1,298,420 original raw V2 numeric bytes on each platform. This excludes
the native outer archive wrapper and in-memory frame validity/padding bytes.
The workers receive audio and fixed configuration only; original waveforms,
reference labels and timing are absent from their inputs.

The outgoing cue-field generator now uses these independently computed
waveforms and duration. All five cases match 2,816 float32 scores, packed
payload bytes, Base64 and start-beat fields on both platforms. These cue and
earlier key-feature comparisons still supply the original refined beat grid,
tempo and downbeat as appropriate. They do not establish final service-selected
cue points, classified energy or key results.

Twelve frontend and eleven context tests pass on both platforms, including
nonzero analytic controls. Mac format/Clippy/release and Linux release pass;
Linux format/Clippy are unavailable in the existing image. An independent
review rechecked source, count/identity/hash guards, real comparisons and raw
numeric encoding without a remaining material finding. Linux workers used
offline containers with no host mounts or expected reference data. Owned
containers were removed and confirmed absent.

The first cue composition stopped after all workers when a captured/reference
identity alias failed its strict join. The corrected join verifies both pinned
identities; predictions and comparison tolerances were unchanged. The failed
attempt remains retained. A separate Linux waveform packaging attempt omitted
an unchanged dependency file; its corrected package passed and both attempts'
cleanup receipts remain retained.

Static source tracing separately establishes that the ordinary single-pass
wrapper creates fresh configuration selecting SDK refinement version 4. The
preview implementation is a different path. Captured JSON does not contain
these configuration flags, and no new runtime branch observation is claimed.

Completed private freezes:

- Waveform context source: 96d211d75c004cceb3676b0c10b063ed2d6abfe9fc032b359b3517dafedcebce.
- Audio waveform frontend: 03bc43c61cedaf1d4e37d1468cf1e67ca88cf85d5c43ba5f57bc400596ee20cd.
- Waveform context/comparison: 27314bc4416dde459273cee57db682b006ab0814b0577d251d186f01bab1215c.
- Linux waveform package: d8ea5455198872cc19e5cc5cf2c8915222a977467379c25489a525dd89d89e34.
- Audio-derived cue composition: f949fe1eb07a683a40033afb8a733268a06f2498e81f91ca704a7dd6162d2d35.
- Linux cue runner: eb014f7a1e1fda29275ef8c4a69cbe6b9bd780fab4918ca3b0ae1cb50b3b785a.
- Beat-job configuration source: 87e11340b3fccbbfc07accdb641534d0464c6922e050e0095fce786b08f5af24.

Combined raw waveform audit:
ba4eba36ce94b99d40271d1e25d8340c8f7abb8bf64379377e2d96fbb675bc03.
The default workspace separately passes 53 tests with two explicit private
parity tests skipped, format/Clippy/release. Production predictions and published
accuracy measurements are unchanged. The installed application, original music,
CSV and held-out split were preserved; no service request or UI work occurred.

The next boundary is independent legacy beats, refinement using the normal
waveform, and pre-request downbeat construction. Independent energy-request
features, final models/selection, broader formats and platforms, and production
integration remain open. The engine-first goal remains active.

## Full SDK refinement under supplied inputs

The independently implemented refinement tail now completes the earlier fit
stage: rebuild, octave normalization, beginning extension, transient snapping,
melody fallback and trim. On five retained numeric fixtures on macOS ARM64,
all 2,468 beat-time binary64 values, overall tempi, SDK downbeats and normalized
counts match. Original legacy marks and SDK-fast waveform features remain
supplied. This is separate from the complete normal-waveform proof above;
joining the two is a subsequent gate. Final outgoing downbeat construction,
independent BPM from audio and portable refinement parity remain unfinished.

Review found an unbounded normalization allocation and a verifier that could
return success on mismatches or incomplete case lists. The next revision adds
finite/projected-count checks before entry and every constructor, plus a pinned
five-case verifier with nonzero failure exits. A further narrow CLI revision
rejects oversized input before parsing rather than accepting a truncated prefix.
Earlier frozen revisions and failed tests remain retained.

The current revision passes 33 Rust tests, 16 verifier tests and seven external
rejection controls, format/Clippy and offline release. The mathematical code is
unchanged by the final CLI repair. Independent re-review closes all three
findings and confirms the conditional parity and unchanged earlier freezes.
No new original SDK/app, audio, service or
container execution occurred in these refinement revisions. No production
integration or held-out evaluation is claimed.

Source-tail freeze: 12a72efd14c7685a6961bcf94b7d60e8c691d30c405185a3db189e6c77e35da6.
Current refinement v3 freeze: c88a84b3cff6171785cd930c1b8fbbf02f9a991b0e1cfc7eb12d6e931b6ff8e2.
Current strict report: 280a02583aad40f5ad86ae524c2afbc03ea950eee064798f776cff9116104a14.

A refreshed retained-source audit also binds all 15 request-builder assignments
to the unchanged Main binary and records request/response boundaries. It runs
no target code, reads no captured envelopes or credential values, and does not
claim identical serialized HTTP body bytes. The private composition checkpoint
records the verified component freezes and owned-container cleanup receipts.

## Normal waveform into refinement: exact SDK boundary, incomplete app grid

The next composition now supplies only original classic legacy marks/tempo to
the refinement component. Onset/melody features and duration come from the
independent normal Main waveform/audio-frame pipeline. Original refined grids,
wire tempo/downbeat and SDK-fast feature arrays are absent from prediction.

Five bounded numeric reference processes call the same SDK refinement function
with these independent waveforms and identical supplied legacy context. All
2,463 timestamp words, tempi, SDK downbeats and normalized counts match on Mac.
This is a new fixture boundary, distinct from the earlier 2,468-word SDK-fast
feature gate. A complete signed framework copy binds to the retained ARM64
source bytes; network and writes outside the new output folder are denied.
The original SDK's numeric code executes in those own processes. No application
process, audio decoder, service or held-out evaluation runs.

The first replay failed before the SDK call because the thin analysis image's
signature is not loadable. A flat full-binary copy then lacked the framework's
bound Info.plist. Both failures remain retained; the complete signed framework
copy verifies and runs. No installed artifact was changed or re-signed.

The complete saved-application-grid comparison still matches zero of five
whole grids. All five tempi round to the wire values as float32, but phase,
endpoints and small timestamp differences remain. This failed broader gate
identifies work that the exact SDK component alone cannot cover: additional
Main-specific phase processing and grid extension. No per-track corrections
or expected anchors were added. Public production accuracy remains unchanged.

Static tracing also maps the classic detector's distinct real 2,048-point
ZRIP FFT, packing and magnitude path. Onset and initial-tempo processing differ
in window precision, channel mixing, mean removal and tail scheduling. The
previous Main 4,096-point complex FFT proof cannot establish this profile.
The private 29-file source identity is
e3ed469b285963fede7fca470ec9480cf6e7b16e5bdd7c6f37e9b75d2bcf0ad4.
Independent legacy detection and complete request timing remain open.

The new eighteen-file ordinary Main timing source gate now recovers the
remaining phase pipeline, effective reset constants and final grid extension.
It identifies an intermediate custom Main cue pass that changes the grid
origin before the SDK pass supplies the final curve. It also proves tempo
float32 narrowing/promotion in the pre-request operation. Its identity is
f63479fa81d7ca642488ddf5b53e1d9bcd2afd975101d01066b944d8e2f92c74.
This source recipe is ready for an independent Main timing worker; no
per-track phase or expected-cue substitution is justified.

The new legacy real-FFT system gate covers sixteen pre-frozen synthetic inputs,
two windows and sixteen shared buffer displacements: 512 jobs / 2,621,440 output
float32 words. Every job completes and all strict shape/hash/finite/alignment
checks pass. Against displacement zero, windows match all 480 comparisons,
packed outputs 60 and magnitudes 90. The measured combined pipeline therefore
has alignment-dependent numeric behavior. Actual app allocation binding,
individual API attribution and a portable Rust implementation remain open.
No original app/SDK or audio executes in that system-only probe.

New composition freeze (71 files):
084c2e6d53794ce14f08a9dbf75264884e1132bfc7f0e89fc5f939c33484da8a.
Its independent review freeze (4 files):
f2445873783e4cde124251dccd3b6964d32b4e3237bc9638010daec2c718255c.
Legacy system gate (1,561 files):
3ae17719c61c3615c6835124bdbee5bfba0b376c8237be9eb599084980cdc8a2.
The engine goal remains active; no production or full-request parity is claimed.
Independent reviews confirm the same-input SDK composition and the complete
system-probe inventory/statistics with no material finding. A private checkpoint
reverified all 2,358 prior component files plus 1,684 new component/review files.

## Computed Main timing and key banks on Mac and Linux

The ordinary Main timing worker now implements the complete source sequence
after supplied legacy detection: SDK refinement, Pass1 half-shift/SDK anchor,
Pass2 timestamp shift/alignment/patterns, Main custom intermediate anchor,
final SDK curve before resampling, full-duration grid construction and wire
float32 tempo conversion. Its waveform arrays and duration are independently
audio-derived. No original refined grid, final BPM/downbeat, response cue or
per-track phase correction enters prediction.

All five development cases match on macOS ARM64 and Linux ARM64: every one of
2,498 binary64 grid timestamps, wire BPM/downbeat scalars, 2,816 float32 cue
scores, payload bytes/Base64 and start-beat fields is exact on each platform.
The key-bank worker now uses these computed timings and independent key caches
as well. All 1,760 binary64 outgoing feature values remain exact on both
platforms. Reference-run original legacy marks/tempo are the remaining supplied
timing producer. These fields are analysis inputs, not final classified keys,
energy labels or the service-selected DJ cue list.

The new SDK raw-curve/anchor port separately matches 2,816 binary64 score words
and selected times on five same-input Mac SDK references. Four numeric controls
cover below/exactly/above100BPM and constructor density normalization, totaling
448 additional raw words. Review corrected the strict half-beat100 boundary,
normalization before geometry, and the SDK/Main short-range empty-result guard.
The older frozen baseline and failed assumptions remain retained. The Main
custom scorer has source/analytic review; no native Main replay is claimed.

Component checks pass: Main steps31, custom scorer13, SDK anchor16 and composed
worker3 tests, plus Mac format/Clippy/release. The Linux package also runs all
33 refinement tests, totaling96 passing tests, then builds and executes five
cases offline without host mounts or expected data. Its owned container and
the separate Linux bank container are removed and confirmed absent before
host reference comparisons. Linux rustfmt/Clippy remain unavailable in the
pinned image; they are not claimed passing.

The strict timing verifier passes16 guard tests; the key bridges pass nine.
Independent review recomputes every saved result, fixes denominators and input
identities, checks the fifth-case alias and source/cache/frame bindings, and
confirms every worker exited before expected numeric reads. An empty-output
stub returns five successful process exits but correctly fails parity with a
five-case denominator. No open material findings remain in these bounded gates.

The exported fields match across the measured platforms, but314 internal
JSON-float diagnostic occurrences differ. This count does not cover every raw
bit-string field. It is not a claim of general internal numerical equivalence;
canonical platform math and broader cases remain necessary validation work.

New completed freezes:

- Main deterministic steps,36 files: eb52b94ecc9d9681c91fc269003c4b7df6112bea6d0826f8a2f902c568a87966.
- Main custom scorer,24 files: f329bc8e558cdaa0478c7a516dadb273194bebc6e44d8db2ccb83edf959ef474.
- SDK raw curve/anchor,322 files: 156d947cc03a932e815b7282692c8c13c8f93b50b1edfe6811a1b518ca283dda.
- Main composition source,14 files: a508b33fc97cabbeac9d7b82d2fff07860f3a1995c99ee097f9b5ac86f14e7f6.
- Strict Mac timing evaluation,30 files: 62809cf88ff0dbcbfbf1e1a7ceea1fee87150bf2120bf4fcde80490add43447d.
- Linux timing package,720 files: a300ca1a13da4628c574868f3b0e266ee66e59d8026499bc546831b6b5c451ce.
- Mac key bridge,63 files: 9dc7f894cd730ed1a71dc6c1cd9f059d501879d5aa0a6c059f726b2c2a5ad003.
- Linux key bridge,71 files: 4f9a279a21012588854199821da48628ea3ab42addd86b20c95af07bf245c182.

Timing workers: Mac899828988d7a7920dfc28b018d2bcc872bcee1220a9b0863602502f85bdf4b49;
Linux0cf5d5b0eb7710778e931307f8a1297072cdf76ca3c55b61d196ceb50ecbf11e.
The installed app, original music/CSV and held-out split were preserved. New
SDK references execute copied numeric functions only; the prediction workers
load no purchased SDK and make no service requests. Production inference and
its published accuracy measurements remain unchanged.

The next producer is the independent classic legacy detector, including its
distinct real2048FFT numerical/allocation profile. Energy-request features,
general segment construction/pitch fields, final offline classification/cue
selection, broader formats/corpus, Windows and production integration remain
unfinished. The full engine-first goal remains active.

## Legacy source closure, portable windows and FFT reference

The classic detector now has a source-backed implementation map covering
candidate PCM buffering, onset extraction, initial tempo, histogram correction
and backward/forward tracking. It resolves a mandatory internal adaptive pass
even when final tracking adaptation is zero, three adaptive-reset floating
arguments, and the 16-byte internal/12-byte exported beat-record distinction.
Independent review corrected two rounding-order descriptions and closed all
findings. Actual ordinary-job adaptation remains unknown; this map is not a
completed detector.

The overall-tempo getter is independently implemented on bounded supplied
local-BPM values. Its 12 tests, formatting, Clippy and release build pass, with
source/analytic review closed. No native getter or full detector parity is
claimed by those tests.

Source-derived Rust cosine/window math now matches 7,211 distinct cosine inputs
and 4,096 coefficient positions on macOS ARM64 and Linux ARM64. The 512 older
system-expression vectors add 1,048,576 repeated comparisons, rather than new
unique coefficients. Eight Rust tests and four evaluator guards pass; Linux
builds/runs offline with no mounts or expected data, and its container is
removed before host comparison. The bounded cosine domain is nonnegative
finite input through the required float32 four-pi value, not a general libm
replacement or Windows validation.

A separate numerical gate executes only copied-framework low-level routines
on pre-frozen synthetic input. It covers real FFT lengths 16 through 16,384 plus
both 2048-point windowed profiles: 832 jobs, 13 terminal child processes and
5,109,760 finite output words. Every native forward/inverse output matches
source-expression system calls replayed on the same native buffers/setup.
The native default allocator produces 16-byte-aligned buffers with varying
modulo 64 addresses. No output varies under the four tested heap layouts;
arbitrary callbacks, live-app allocation state and other machines remain
outside this observation.

All 384 window/packed/magnitude vectors compared to the prior 2048-point
displacement-zero system reference match exactly, covering 655,360 words.
The onset window also matches actual native factory output. The periodic
window comparison uses the source expression installed through native
SetWindow; it does not execute the original CDaLoopa constructor. Network,
original-app, user Library and music-library access are denied in the numeric
reference process. Seven host verifier guards pass and independent review
closes without material findings. This is reference execution, not an
independent portable real-FFT implementation.

The source map now makes the next implementation boundary explicit: variable
real forward/inverse FFTs, autocorrelation vector operations, onset extraction
and the tempo tracker must replace supplied original legacy marks/tempo.
Energy features, general key-segment/pitch fields and the offline mapping from
request features to final key, energy and selected cues remain unfinished.
Production inference, whole-engine accuracy metrics and the held-out split
are unchanged. The engine-first goal remains active.

## Portable real FFT, vector arithmetic and beat records

The real-FFT setup generator now independently computes all required tables.
On Mac, all 16,376 words across eleven sizes match actual system setup bytes;
4,118 cosine32, sine64 and narrowed-sine32 controls also match exactly.
Linux generates the same table bytes. Source review resolves the actual
cosine32/double-sine imports, angle rounding and compensated sine reduction.

The portable forward-post/inverse-pre packing stages now cover every power of
two from 16 through 16,384. All 352 synthetic jobs and 1,048,064 float32 words
match on each of Mac and Linux ARM64. A complete real8192 implementation
also passes all sixteen synthetic cases and 262,144 forward/inverse words on
both platforms. It reuses our independent aligned complex4096 arithmetic,
with source-bound dispatch, forward half-scaling and unnormalized inverse
plane swapping. The inverse comparisons use independently computed forward
spectra; arbitrary inverse inputs and the other complex sizes are not covered
by this complete-transform proof.

Packed multiplication, conjugation, thresholding and magnitude are now
independently implemented. The final bounded gate matches 139 cases and
283,365 words on Mac and Linux ARM64. Magnitude reproduces the source reciprocal
square-root estimate/refinement rather than assuming host square root.
Independent review found an eight-bin tail rounding error that the initial
cases missed. Separate square rounding and a distinguishing regression fix
it; the earlier source, binary and passing-but-incomplete fixtures remain
retained. Review also corrected a CLI read that checked its bound too late.
The saved Linux auditor now binds and rechecks every coefficient word and
identity; six additional guards reject corrupted or malformed table output.

The beat-record dependency now preserves physical 16-byte records, the
12-byte export projection, count/capacity semantics, the source nearest-mark
tie/sentinel rules and snapshot threshold mappings. Its 27 analytic/contract
tests pass on both platforms. Those tests are source/analytic evidence, not
a native detector or real-music parity result.

The two Linux packages run 68 Rust tests, including eight previously closed
window tests. Their owned containers have no network or host mounts and
receive only own Rust source, analytic tests and generated numeric inputs.
Captured numerical gold stays outside prediction workers. Removal is verified
before host comparison. Linux rustfmt/Clippy remain unavailable; Mac checks
pass. Independent source, numeric and saved-artifact reviews close with the
recorded fixes. No original app/music/CSV, holdout or production inference
changes, and no Windows or whole-engine accuracy improvement, are claimed.

The next checkpoint below advances the remaining kernels. Complete onset,
initial-tempo/autocorrelation and seven-hypothesis tracking must still replace
supplied legacy timing. Energy/segment producers and the offline final
key/energy/cue mapping remain required. The engine-first goal remains active.

## Fixed 2048-point spectrum and remaining FFT components

The independent complex1024 composition now matches 48 synthetic forward,
inverse and initial-radix4 cases, covering 98,304 float32 words. Its generated
4,192 coefficient words also match actual system setup. Static source checks
preserve the initial zero-coefficient FMAs and verify all 2,048 final output
expressions, including in-place addressing and frequency scatter.

Composing that transform with the previously checked windows, real packing
and magnitude calculation now matches all 32 prepared-frame cases and 98,304
words for the two 2048-point profiles. Window4 uses an actual original-factory
reference; window7 uses the retained source expression installed through the
native window setter, rather than an original CDaLoopa constructor run. Channel
mixing, mean removal, streaming frame scheduling and the later threshold are
outside this prepared-frame comparison.

The small complex transforms at lengths8/16/32/64/128 match 160 synthetic jobs
and 15,872 words, plus all 450 setup words. The special complex256 path matches
32 forward/inverse jobs and 16,384 words, plus all 736 setup words. Its dense
real accumulation order differs from the ordinary final-stage order. Initial
radix2/radix8 stages separately match 36 cases and 258,048 words. These checks
use generated numeric inputs and configuration; captured answers are kept
outside prediction workers.

All of those new Mac comparisons also pass on Linux ARM64: 314 jobs and
492,290 payload float32 words are byte-identical to the independently validated
Mac workers. The isolated Linux package runs 76 Rust tests across ten crates,
with no network or host mounts. Its container is removed before the host reads
expected numerical outputs. This is a bounded transitive comparison, with no
new Linux execution of the purchased SDK. Windows remains unverified.

A separate two-block middle4096 stage matches 16 Mac cases and 262,144 words,
conditional on its explicit 960-word numeric coefficient input. Source-only
final-stage proofs cover all 21,504 output expressions for complex512/2048/8192;
those complete transforms have not yet been composed or numerically validated.
Current prototype amplitude guards also exclude some ordinary inverse and
autocorrelation inputs. Reviewed domain extensions and larger spectral controls
are required before treating these components as a general FFT provider.

These experiments do not change production inference, real-library agreement,
the supplied-legacy condition on Main/key/cue proofs, or held-out evaluation.
The next steps are the remaining complete transforms, onset/initial-tempo
composition and the adaptive tracker, followed by the remaining request fields
and offline response mapping.

## Complete real FFT size coverage

The next composition implements the remaining complex512/2048/8192 transforms.
On Mac, all 144 forward/inverse synthetic jobs match 1,032,192 float32 words;
three generated setup tables match all 42,336 words. This includes larger
DC/power inputs and subnormal controls. Source dispatcher tracing establishes
the initial/middle stages and the coefficient-base reset between levels.

A new real-FFT composition now covers every required power of two16..16384.
All 176 window0 forward/own-forward-inverse jobs match the retained original
wrapper references, totaling 1,048,064 words. Another 440 own-system-reference
jobs match 1,703,104 words, including raw inverse spectra up to1e20. The new
version retains old arithmetic and records guard-only changes in separate copies;
frozen v1 components remain unchanged. A large-roundtrip regression caught an
inverse guard that was too narrow. Forward and inverse now have explicit
separate bounds, with finite-output checks; exhaustive floating-point-domain
equivalence is not claimed.

Linux ARM64 matches all 763 jobs and 3,825,696 words across both compositions,
including every setup coefficient. The isolated seven-crate package passes49
Rust tests. All workers exit and the owned container is removed before the
host reads reference values. This is a bounded comparison to Mac predictions
that were independently checked against the actual references. Final bundle
review closed with no material findings,49 fresh analytic tests and16 rejection
controls. Windows and production integration remain unverified.

This closes the missing FFT size coverage in the recorded profiles. Onset
streaming, CDaLoopa and adaptive tracking must still replace supplied legacy
timing. Complete audio-to-request and offline final-result agreement remain
the acceptance gates; whole-track accuracy and held-out results are unchanged.

## Complete default onset stream

The reconstructed onset producer now connects PCM framing to final onset marks
for the fixed44100Hz classic mode0/band1 profile. It preserves per-channel
division before summation, positive-only mean removal, per-call index carry,
three-spectrum change scoring, FIR/history updates, peak predicates and the
source's residual-dependent finish padding and cluster normalization.

All sixteen synthetic stream schedules match the copied original component:
392 initialized-state snapshots and33 final onset marks, including their exact
sample coordinates and strengths. Separate sine/exponential batches match
67,316 words. Together the gate covers2,028,139 float32 and19,890 integer words;
all packet bytes match as well. These are onset marks, upstream of tempo and
beat tracking. No real-music or final cue agreement is inferred from this gate.

Linux ARM64 produces identical bytes to the Mac predictions, which were checked
against the actual component/math references. The corrected version passes14
Mac Rust tests and101 tests across its isolated Linux dependency package.
Expected values and reference-derived output shapes stay outside that package;
the container is removed before host comparison. Windows remains unverified.

Independent review corrected an admission-proof gap for an ambiguous negative
peak proposal at a nonpositive sample gap. The earlier source and numerical
proof remain retained. A new contract test failed before the guard and passes
afterward; all recorded numerical outputs remain identical. The initial-hop
reachability argument and later zero-strength reentry are documented separately.
The corrected review closed without unresolved material findings. Native
uninitialized reads, unused storage and floating-point exception flags remain
outside the portable profile.

The next missing producers are CDaLoopa initial tempo and the adaptive
seven-hypothesis tracker, including the mandatory histogram pass. Until those
are composed and checked, existing Main/key/cue proofs still supply original
legacy marks/tempo. Production inference, library accuracy and held-out results
are unchanged; complete independent request and offline final-result matching
remain unfinished.

## Complete default initial tempo

The default one-shot initial-tempo producer now reconstructs spectral band
histories, onset curves, autocorrelation, harmonic suppression, band selection,
phase folding, intersection and raw/normalized BPM directly from numeric PCM.
Its fixed profile is three bands,2048-point spectra,256-sample hops and44100Hz,
with4096..1058400 input samples. This also exercises the original constructor's
custom window, closing the earlier native-window-setter condition for this stage.

All18 synthetic packets match the copied original component byte for byte on
Mac:665,418 float32 words and100,050 integer words. The controls cover every
autocorrelation FFT size16..16384, all three selected bands, silence, residual
input, subnormal values, pulses/noise and the24-second maximum. Linux ARM64
produces identical packets and passes67 Rust tests across nine crates. Its
package contains own source and numeric inputs; reference values and derived
output shapes stay outside it. All workers exit and the container is removed
before host comparison. Windows remains unverified.

The comparison caught a source-map mistake in the negative interval getter:
stored negative periods remain sample-scaled, but the getter returns-1.
The corrected code follows the retained assembly and passes a regression that
failed before the fix; the old12/18 result and source remain preserved. A
separate harness alignment guard was corrected before any reference analysis.
Its terminal alignment label is a literal, so only the checked alignment
classes are established; exact per-buffer remainders are not claimed.

Fresh review closed with no unresolved material findings. This is an initial-tempo component result on
synthetic inputs, not final tempo or cue agreement. Adaptive seven-hypothesis
tracking, its mandatory histogram pass and the complete classic lifecycle still
must replace supplied legacy timing. Production inference, library agreement
and held-out results are unchanged; complete audio-to-request and offline final
result matching remain unfinished.

## Seven-hypothesis tracker composition

The portable tracker now implements construction, seven-hypothesis initialization,
candidate scoring, preprocessing, both reset modes and backward/forward passes.
It preserves the saved state from before histogram tracking and the mutated
backward state carried into forward tracking. Later steps with no new winning
candidate reuse the source's already initialized scratch values; a first use of
unwritten source values returns an explicit error.

All18 synthetic mark/operation sequences match the copied original component
on Mac and Linux ARM64:148 initialized-state snapshots,25,645 float32 words,
8,517 integer words and1,110 final physical marks across the cases. These include
empty/short lists, regular and changing tempo, missing onsets, varying strengths,
explicit starts, both resets and the histogram-to-final saved-state workflow.
The projection masks unwritten fields and excludes pointers, padding and unused
capacity; it is not a raw native-object memory comparison.

The required variable-power scoring, confidence and decimal-exponential helpers
also match112,060 synthetic float32 results plus41,927 integer exponents against
own-process system-math references. All21 combined Linux jobs match137,705 float
and50,444 integer words. The isolated package passes47 Rust tests across three
crates, with no reference values or derived output shapes in its inputs. Workers
and the container exit before host comparison. Windows remains unverified.

Fresh review closed with no material findings. The pure histogram counter has source/analytic
coverage; the original classic histogram entry point has not yet been executed
in this composition. Full classic PCM buffering/finalization, histogram invocation
and integration of the ordinary-job configuration still must connect these pieces before replacing
supplied legacy marks/tempo in the five-track downstream proof. Production,
whole-library agreement and held-out results remain unchanged.

Static tracing now binds the ordinary Main path to explicit classic detector0
and source-derived final adaptation+0.0f from a fresh analyzer. Its fresh segment
has no finite end, so reads request512 frames and pass the actual returned count
unchanged to legacy analysis. Histogram adaptation1.1 remains independent.
These are source findings, not new runtime or decoding-parity observations.
