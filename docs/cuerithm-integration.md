# Cuerithm integration

The canonical repository is `johnneerdael/Cuerithm`. The integration builds on
the upstream cue engine and its 47-test suite. Existing DSP/OpenKeyScan/S-KEY key
analysis and classical/source-profile/learned tempo backends are retained.

## Cue engine boundary

`lexicon_cues.rs` retains the validated upstream calculations. A separate
checked wrapper validates PCM, tempo, first-beat and configuration domains
before the application invokes them. Normal Engine analysis decodes a direct
44100 Hz branch for cues, independently of the configurable key/DSP rate.
Source-profile timing supplies its computed BPM and first anchor directly;
reference cue positions are never supplied to ordinary analysis.
If that grid begins before a cropped file, the wrapper clips only cues exactly
at that negative anchor to zero after analysis. It preserves the anchor throughout
the DSP, so the beat and section phases remain unchanged.

The rich cue result preserves colors and emergency loops. The general cue
channel is populated from the same result, so existing export and evaluation
code sees the new cues. The old novelty generator and its private parity test
are removed. `--cue-suggestions` is only an alias for `--lexicon-cues`; it no
longer invokes novelty selection or requires a learned beat model. Enabled
legacy `cue_config` objects are rejected with a migration error.

The source hash includes both the cue DSP and its wrapper. Disabled optional
backends preserve the previous default effective configuration identity. New
cue settings/rate and energy model content are included when enabled.

## One energy value

The optional local RBF SVC consumes the existing 183 energy features and
aggregates internal labels by nominal duration. Only one track rating is
serialized; no segment energy or energy-named cues are produced. The rating
uses the retained client tag aggregation and is distinct from the remote
OverallEnergy research target. Numeric model artifacts remain private.

Energy extraction has its own file entry point, avoiding unused key/request-cue
feature production. Energy and timing consumers share the same captured encoded
source as the key/cue decoders. Content/metadata are checked again before return.
Complete-input requirements and unsupported profiles remain explicit errors.

## Historical material

Old research and plans are retained for provenance, with MiK cue work marked
superseded. They do not define the active product scope. The former request-JSON
CLI command is removed. Low-level timing helpers retained for BPM may still have
historical cue-related names; they are not a MiK final cue selection backend.

Private local analysis, licensed applications, original music, captures and old
engine snapshots are preserved outside the Git working tree's published files.
