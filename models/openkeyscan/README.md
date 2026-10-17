# Optional OpenKeyScan3 key backend

Build the workspace with `cargo build --release --features openkeyscan-key`.
CMake and a C compiler are build requirements for this optional feature. The
resulting engine uses Rust/RTen and the vendored native SoXR library; Python
and network access are not runtime requirements. The default build does not
compile the native backend or require CMake.

Supply the pinned ONNX graph explicitly:

```sh
cargo run --release -p camelot-cli --features openkeyscan-key -- analyze audio.flac --key-openkeyscan-model /path/to/openkeyscan3-logits.onnx
```

The engine verifies the exact graph checksum in `model.json`. It does not
convert checkpoints or download models while analyzing. The graph was exported
from the pinned upstream OpenKeyScan3 checkpoint; no locally trained head or
library-label adjustment is used. Weights are not bundled here. The upstream
repository has an MIT license; this folder preserves that notice, and does not
assert separate checkpoint redistribution terms not stated by the publisher.

The backend decodes/resamples directly to 44100 Hz, computes the fixed CQT and
returns raw logits in the candidate ranking, not calibrated probabilities.
Under three seconds or RMS <= 1e-5 yields an unknown key. Nonfinite PCM, an
incorrect input rate at the PCM API, or more than 64000000 analyzed frames is an
error. Explicit `--max-seconds` excerpts remain marked partial. Profile/S-KEY/
OpenKeyScan choices are mutually exclusive. Source-profile BPM can be combined
with OpenKeyScan on its supported complete FLAC profile; energy and semantic
cues are separate work.

Model-dependent checks are opt-in (the environment variable supplies a path,
never model content or expected track labels):

```sh
CAMELOT_OPENKEYSCAN_MODEL=/path/to/openkeyscan3-logits.onnx cargo test -p camelot-core --features openkeyscan-key --lib openkeyscan -- --include-ignored
CAMELOT_OPENKEYSCAN_MODEL=/path/to/openkeyscan3-logits.onnx cargo test -p camelot-core --features openkeyscan-key --test openkeyscan_settings -- --include-ignored
```

See `docs/research/openkeyscan-rust-backend.md` and `THIRD_PARTY_NOTICES.md` for
frontend details, validation limits and native source notices. Actual Windows
x64 execution passes a generated-audio key smoke using the same source/config
and model identities as Mac. All26 cross-compiled all-feature test targets also
execute successfully on Windows:399 tests pass (7 normally ignored), with both
OpenKeyScan model-dependent tests passing explicitly. Native Windows compilation
was not performed. Broad-format parity, real-music Windows parity,
held-out accuracy and DJ cue/energy quality remain separate unverified gates.
