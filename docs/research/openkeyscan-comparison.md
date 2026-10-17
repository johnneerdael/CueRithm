# OpenKeyScan comparison, 2026-10-17

The unchanged upstream OpenKeyScan key pipeline matches **274/282 (97.16%)**
stored Mixed In Key 11 keys on the existing tuning representatives. The prior
Rust S-KEY run matches254/282 on those same identities. OpenKeyScan fixes24 of
its disagreements and introduces4, a net gain of20 exact matches. An independent
regrade from retained raw predictions reproduces274; source/model hashes and
every prediction hash were verified again. No model was trained or selected
using the holdout.

This measures agreement with MiK11 labels. It is not independently annotated
musical accuracy and does not validate the website's benchmark on its dataset.
Only8 of these282 tuning tracks have major-key reference labels: OpenKeyScan
matches7/8 major and267/274 minor labels. Broader major-key coverage is needed.

## Disagreements

| Relationship to MiK11 | Tracks |
| --- | ---: |
| Neighbouring fifth, same mode; one Camelot step | 5 |
| Relative major/minor; same pitch collection | 1 |
| Parallel major/minor; same tonic | 2 |
| Other relationships | 0 |

Exact or standard Camelot-compatible predictions cover280/282 (99.29%).
Compatibility is a separate diagnostic; exact agreement remains274/282. The
two parallel-mode disagreements are not standard adjacent-wheel transitions.
Listening or independent annotation has not established which disputed labels
are correct.

## Reproduction identity and execution

- Upstream: https://github.com/rekordcloud/openkeyscan-analyzer
- Revision: `d5cac64bc7195826b6eb10c55b9b0c97fb3287c0`.
- Default checkpoint: `checkpoints/openkeyscan3.pt`, SHA256
  `5610691be2b3aa21764c5978ca0c2524fcdbf600f09dcbda6c2da937745db600`.
- Upstream MIT notice retained with the source snapshot; source checkout unchanged.
- Python3.12.13, upstream locked production dependencies, CPU, two workers,
  two math threads; Apple M4 macOS ARM64. No source or checkpoint modifications.
- Full-file mono44100Hz librosa decoding,105-bin CQT, log1p, the upstream
  removal of the last two time frames, and default KeyNet inference.
- All282 predictions succeeded. Before/after input hashes equal the prior
  complete integrity-verified FLAC inputs. Runtime had no network or access to
  reference-label contents; grading began after the server exited successfully.
- Startup25.69s, batch124.16s (136.27files/minute); not a controlled comparison
  against MiK11 speed. The initial interpreter sandbox startup failure produced
  no predictions and remains retained. A metadata-only permission correction
  preserved reference-content denial before the successful batch.

Private run: `openkeyscan-baseline-v1/run-v2`; report SHA256
`66baec12fa43f482d8f7e7226121a8e8f39a8d6ca3ed10def253b90abf5396b6`.

This is a research comparison using upstream Python. It has not yet replaced
the Rust runtime key backend. The native frontend diagnostic below is now complete; production backend
integration remains the next key-engine task. OpenKeyScan supplies key only; BPM, energy classification and cues
remain separate engine work.

## Rust network feasibility, 2026-10-17

The fixed checkpoint now has a validated ONNXopset17 export with variable time
length. The existing Rust BeatThis1.1.0/RTen0.24 runtime executes that network
without Python. Nine synthetic spectrograms and five unchanged upstream full-track
CQT tensors produce the same14 predicted classes as PyTorch. All336 logits meet
the predeclared absolute/relative1e-4 tolerance; maximum absolute difference is
1.9073486328125e-6. These are component-parity results, not exact logit-bit parity
or a new whole-engine agreement score.

The upstream model, checkpoint and source checkout remain unchanged. The five
re-extracted music tensors also retain their frozen full-run classes. Runtime
reference-content denial was checked and grading followed worker termination.
At this component gate, the CQT frontend was still Python; the subsequent
native diagnostic below removes that dependency. The production key backend
remains unchanged. Private component run: `openkeyscan-model-runtime-v1`; graph SHA256
`262ac22bce23d371fe045d478e90d200f71b5be16046792a8b55432ae5c9f952`.

## Native full-file Rust diagnostic, 2026-10-17

The fixed frontend now decodes full supported FLAC files, computes CQT with
portable SoXR C plus RustFFT, and runs the pinned network with Rust RTen. No
Python or platform DSP framework is used by the worker. All282 tuning files
succeed, match the unchanged upstream class on282/282, and retain274/282 exact
MiK11 agreement. All source-file hashes remain unchanged and temporary encoded
snapshots are removed. No model/library parameter was adjusted.

On macOS ARM64, three generated signals and five full-track inputs match all
classes, with977445 CQT values and192 logits inside the predeclared1e-4
absolute+relative tolerance. The same eight-case gate passes in an offline Linux
ARM64 container; the container was removed. These are numeric tolerance and
class-parity checks, not internal bit parity or Windows execution. Initial
native archive/toolchain failures are retained separately from successful runs.

Private runs: `openkeyscan-resample-native-v1`, `openkeyscan-cqt-frontend-v1`
(including `linux-v2`), and `openkeyscan-rust-tuning-v1`. Full-file report SHA256
`35a5709e475facec1d3e2e2484606e515a61b8d6dd04745e67b5701cbf52446d`.

This is a diagnostic executable, not yet an engine/CLI backend. Remaining work
includes model/native-source packaging and notices, explicit backend selection,
silent/short/error semantics, compatibility tests, final-binary comparisons,
and Windows verification. It does not settle energy or cue quality.
