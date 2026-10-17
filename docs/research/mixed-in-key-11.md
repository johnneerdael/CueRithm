# Mixed In Key 11 investigation

## Goal and boundaries

Build CamelotWheel as an independent, open-source desktop application for key,
Camelot notation, BPM and a local music library. Use Tauri 2 with a Rust backend
on macOS, Windows and Linux, as requested by the user. Preserve the installed
Mixed In Key application. Keep local binary copies and decompiler records under
`.analysis/`, outside distributable source and release artifacts.

## Artifact identity

- Installed application: `/Applications/Mixed In Key 11.app`.
- Bundle identifier: `com.mixedinkey.application`.
- Version: `11.2.6`; build: `11.2.6.7446`.
- Main executable: `Contents/MacOS/Mixed In Key 11`.
- Original SHA-256: `04f10f4cf84938f42dc13c0c984da915b861beaab9675713b61bf82e17c5b905`.
- ARM64 analysis slice SHA-256: `051e2805a4b34b3bbd1f4437aae2b537b1b133a1830c5450c27500199666d36b`.
- Derivation: `lipo -thin arm64`; original executable unchanged.

REA rejected direct analysis of the universal executable because its Ghidra
provider cannot enforce an architecture for universal Mach-O imports. Opening
the separately thinned ARM64 executable succeeded. Selected main-executable
and analysis-framework methods have now been decompiled. Full records are
retained locally under `.analysis/`; symbol inventory alone is not behavioral
evidence.

## Observed package structure

These facts come from local bundle metadata and native tools, not REA Evidence
records or runtime observation.

| Boundary | Packaged component |
| --- | --- |
| Analysis | `Contents/Frameworks/MIKAnalysis.framework` |
| Audio | `Contents/Frameworks/MIKAudio.framework` |
| Library | `Contents/Frameworks/MIKData.framework` |
| Tags | `Contents/Frameworks/MIKTag.framework` |
| Playback and waveform | `Contents/Frameworks/DanceabilityAudioEngine.framework` |
| Shared UI/toolkit | `Contents/Frameworks/MIKToolkit.framework` |

The main plist registers MP3, MP4, M4P, M4A, WAV, AIF, AIFF and FLAC extensions.
Registration does not establish decoder coverage or DRM support.

## Local-library schema observations

Read the compiled Core Data schema through `NSManagedObjectModel` without
opening a user database. `MIKData.framework/Versions/A/Resources/DataModel.momd/VersionInfo.plist`
selects `DataModel 11`.

| Entity | Relevant stored concepts |
| --- | --- |
| Song | Path, title and descriptive tags; key, second key, tempo, energy, pitch; beats; sample rate, bitrate and file size; loudness; analysis and tag-sync dates |
| Collection | Name, ordering, parent/children, song membership and filters |
| KeySegment | Start, duration, key, confidence, energy, volume and single-note flag |
| EnergySegment | Start, duration, energy and comment |
| CuePoint | Time, name and energy level |
| Waveform | Transformable waveform object linked to a song |

The serialized beats and waveform formats remain unknown. CamelotWheel should
define its own schema and cache formats rather than reuse these opaque values.

## Packaged defaults

`Contents/Resources/defaults.plist` contains settings for key display, Camelot
wheel visibility, key zero-padding, tempo adjustment, key/BPM tagging, cue
exports, waveform caching, stems and playback. Its tempo adjustment defaults
are 79–192 BPM. These are packaged values, not effective user preferences and
not proof of every implementation path.

## Bounded symbol observations

The ARM64 export table for `MIKAnalysis.framework/MIKAnalysis` contains:

- `MIKIndexForCamelotKey` at `0x85ac`.
- `MIKMajorIndexForCamelotKey` at `0x87ac`.
- `MIKMinorIndexForCamelotKey` at `0x87ec`.
- Objective-C class `MIKNewKeyAnalyzer`.
- `KeyFeatureExtractor` methods for FFT processing, harmonicity, loudness and
  feature results.

Addresses here come from the export table of the selected framework slice.
They must be reconciled with the provider's loaded addresses before analysis.
Names establish entry points, not the algorithms behind them.

## Reconstruction decisions

- Keep analysis and persistence in a portable Rust core separate from Tauri.
- Use independent DSP and public algorithm descriptions; do not ship original
  executables, framework code, interface resources or model assets.
- Treat key and tempo results as estimates. Preserve algorithm version and
  ambiguity so future benchmark results can motivate improvements.
- Initially write only CamelotWheel's application data. Importing or analyzing
  audio must not modify source music tags.
- Defer energy, cues, stems, external-DJ-library export and exact algorithm
  parity beyond the user-selected first version.

## Investigation checklist

- [x] Identify installed version and preserve original artifact identity.
- [x] Create a separate ARM64 analysis copy and open it with Ghidra.
- [x] Inventory the relevant framework and local-library boundaries.
- [x] Decompile selected key/Camelot and analysis entry points.
- [x] Retain REA Evidence IDs and link conclusions to complete records.
- [ ] Establish a reviewed design and implementation plan.
- [ ] Implement and test the selected first-version workflows.
- [ ] Verify a macOS desktop build; prepare Windows/Linux build checks.

## Unresolved questions

The inspected analysis client serializes locally computed features and requests
key/energy/cue results from a vendor service. See
`docs/research/analysis-service-contract.md` for the consumed contract and
Evidence IDs. Local feature extraction and the Camelot parser were inspected;
the server-side classifier, complete BPM estimator and confidence calibration
have not been recovered. Two encrypted `sep01_*_16.mlmodelenc` directories are
present; their architecture, inputs, outputs and weights remain unknown and
have not been decrypted. No equivalence to the purchased application's
analysis accuracy or runtime behavior is claimed.
