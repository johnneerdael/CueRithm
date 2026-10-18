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

The desktop uses the compiled local acoustic meter in `lexicon_energy.rs`.
Full-track mono 22050 Hz PCM supplies crest factor, spectral flatness, high-mid
and sub-bass ratios; a fixed formula produces one rating from 1 to 10. No energy
model file, server response, source tag or cue classification enters the result.
Silence and input shorter than one analysis frame remain unrated. See
[local track energy](research/local-track-energy.md) for evidence and limitations.

The standalone explicit-model SVC API remains available for historical research;
it is not selected or embedded by the desktop. Backend/configuration identity
changes invalidate preparation caches. Upgrade migrates settings and unfinished
jobs without rewriting completed tracks; explicit reanalysis updates old ratings.

## Historical material

Old research and plans are retained for provenance, with MiK cue work marked
superseded. They do not define the active product scope. The former request-JSON
CLI command is removed. Low-level timing helpers retained for BPM may still have
historical cue-related names; they are not a MiK final cue selection backend.

Private local analysis, licensed applications, original music, captures and old
engine snapshots are preserved outside the Git working tree's published files.
