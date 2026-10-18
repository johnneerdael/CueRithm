# Optional single track-energy rating

Pass a local numeric RBF SVC JSON with `--energy-model /path/to/energy-svc.json`.
The desktop embeds the pinned, locally trained parameters in
`models/builtin/energy-svc-v1.json`; its user never selects a model file. This
explicit path remains available for standalone engine/CLI use. Training captures
and labels remain private. Python/sklearn is a training/export tool only;
inference is implemented in Rust.

The model uses the 183 feature values as supplied by the energy extractor, with
no StandardScaler and no appended RMS value. Its array layout is sklearn/libsvm:

- `gamma`: positive finite RBF coefficient.
- `classes`: sorted unique energy labels in 1..10.
- `n_support`: positive support counts, one per class.
- `support_vectors`: finite vectors of length 183, grouped by class.
- `dual_coef`: `(classes - 1)` rows by total support count.
- `intercept`: one value per class pair, in one-vs-one order.

The loader validates dimensions and numeric domains, hashes and parses the same
owned bytes, and records the model identity in normal analysis metadata.
Classification uses one-vs-one votes with libsvm tie ordering. No calibrated
confidence is invented.

Only one rating is exposed: the energy level with the longest total nominal
duration, with the higher level winning an exact tie. The source feature profile
currently supports complete 44100 Hz stereo signed16-bit FLAC files. An empty
feature sequence yields no estimate; unsupported files or invalid models fail.
Window labels are internal and do not affect structural cue placement or names.

For an explicit private numerical parity check, supply the model and an array of
`{"features":[183 numeric values],"level":expected_integer}` records:

```sh
CAMELOT_ENERGY_MODEL=/path/model.json CAMELOT_ENERGY_GOLDEN=/path/golden.json \
  cargo test -p camelot-core --test energy --release trained_svc_matches_explicit_numeric_golden -- --ignored
```
