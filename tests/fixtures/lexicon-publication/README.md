# Cue publication references

Authored marker/template inputs measured with the unchanged Lexicon 1.11.15
renderer publication function, plus the actual factory defaults, enum values
and BPM-rounding helper, in its installed Electron Node 20.18.1 runtime.
The JSON records source/factory/capture hashes and the isolation boundary.
Proprietary implementation code and the capture harness remain private.

The bank contains 229 publication cases and 203 rounding cases: default marker
subsets, disabled/fixed positions, repeated structural/custom markers, retained
start cues, zero/first/existing start behavior, legacy settings, custom offsets,
loop/memory/active types, null colors and fractional measured-BPM publication.
Measured-grid cases round saved tempo before applying beat-relative offsets;
raw structural marker times remain untouched.

These are publication-layer references. They do not prove audio detection,
custom-anchor extraction, persisted-library integration, native UI delivery,
DJ software export or corpus-wide accuracy. `overwrite` gating is outside the
captured function. Invalid nonfinite timings and a source emergency-only
negative-infinite position are rejected explicitly by the Rust API rather than
serialized as a valid-looking null.
