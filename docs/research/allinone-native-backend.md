# Native optional All-In-One backend investigation

The user approved optional deep structural analysis on2026-10-17. Preserve all
sections and derive DJ cues separately; the24-track full original reference run
has completed successfully. This document concerns runtime portability, not model quality.

## Architecture evidence

The original full structure model is eight independent Harmonix folds. Its
`Ensemble.forward` averages each of the four logit heads, then postprocessing
performs sigmoid/softmax and metrical/functional decoding. The source stores
children in a Python list; an export-only wrapper registers the exact same eight
objects as a ModuleList for serialization, preserving the averaging math.
Time is the variable dimension of [1,4,T,81] input. The validated portable
neighborhood-attention operators use gather/einsum rather than a custom compiled
NATTEN extension, making a standard-operator export worth testing. Variable time
must be executed in Rust and compared, not inferred from a dynamic-axis label.

A separate private export experiment uses copies of the original source,
attention adapter and all eight hash-verified checkpoints. It leaves the live
reference environment unchanged. Generated inputs at1025,1100 and2049 frames
have four eager reference heads. The first export targets one complete ensemble
ONNX17 graph; its Rust/RTen worker and1e-4absolute+relative comparison are prepared.
No result is claimed before export and actual Rust execution finish. Python is
an export/reference tool, not the proposed installed runtime.

The model is only one part of the add-on. Native Demucs separation, original
normalization, spectral features and postprocessing are still required. The
[demucs.onnx author](https://github.com/sevagh/demucs.onnx/blob/main/README.md)
describes moving STFT/iSTFT out of the Demucs graph and implementing them beside
ONNX inference. This is a candidate architecture reference, not a validated
replacement for our fixed htdemucs checkpoint or a chosen dependency.

Do not make a new TorchScript-based production path the default: the
[PyTorch2.12 release notes](https://pytorch.org/blog/pytorch-2-12-release-blog/)
state that TorchScript was deprecated in2.10 and direct new development toward
torch.export/ExecuTorch. This does not prove ONNX export succeeds; export/runtime
operator support, varying lengths and memory still need actual checks.

Private evidence: `allinone-native-feasibility-v1`. The full24-track reference
and new native feasibility experiments have separate identities. Neither is a
production backend, held-out evaluation or musical-quality approval.


## Full-eight-fold Rust network gate

The complete structure ensemble now executes in Rust/RTen0.24 without Python,
through eight ONNX17 graphs and a raw-head mean. All247195 values across the
four heads at1025,1100,2049,1279,1280,1281 and11001 frames match original eager
ensemble outputs within the predeclared1e-4absolute+relative tolerance. Maximum
absolute difference is2.86102294921875e-6. These are generated spectrograms,
not audio-to-sections or musical-quality validation.

The failed combined export (600s deadline), an RTen high-dimensional Einsum
panic and the frozen-padding2049-frame failure are retained. Export-side
broadcast multiply/reduction avoids the unsupported contraction while retaining
all folds; the unchanged author/NATTEN golden suite passes5 tests. Shape-tensor
padding preserves max(0,window-current_length) for different lengths. The long
11001-frame case passes beyond the deepest attention window; original weights
and the live24-track reference pipeline were unchanged. A copied-worker execute
permission error was corrected before inference, with that launch retained.

The same seven generated-input cases also pass on Linux ARM64/Rust1.89 in an
isolated, network-disabled container. All28 output files (247195 values) are
byte-identical between Mac and Linux; both match the original eager reference
within the declared tolerance. The owned Linux container was removed before
host reference comparison.

Private native gate: `allinone-native-feasibility-v1/split-v3`. Windows and real
spectrogram checks, native Demucs/frontend/postprocessing, practical resource
measurements and production integration remain open. Only system C/iconv are
linked by the Mac inference worker; no Python/libtorch runtime is loaded.
