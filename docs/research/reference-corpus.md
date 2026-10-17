# Reference corpus and cue-point evaluation

The user supplied a Beatport audio folder and a Mixed In Key CSV export for
supervised comparison. This changes the implementation strategy: an unrecovered
vendor classifier is not required to measure agreement with its recorded
outputs or train/tune an independent offline predictor.

## Audit results

The local, read-only audit found:

| Item | Observed count |
| --- | ---: |
| FLAC files | 434 |
| Duration across files, including copies | 40.67 hours |
| Sample rate | 44,100 Hz for every file |
| Distinct declared decoded-audio MD5 groups | 364 |
| CSV rows | 1,285 |
| CSV playlists | 3 |
| Distinct normalized CSV title strings | 362 |
| Files with source-labelled key, energy and cue JSON | 434 |
| Cue points across files, including copies | 3,382 |
| Cue points across the 364 declared-audio groups | 2,835 |

The grouping uses nonzero PCM MD5 values declared in FLAC STREAMINFO, sample
rate and sample count. Metadata fields were cross-checked against `metaflac` for
three files. The candidate engine has now fully decoded and passed FLAC
integrity verification for all 282 tuning representatives. The 82 held-out
groups have not been decoded/evaluated. Treat the 364 figure as declared audio
groups, not proof that every file has been independently verified.

The CSV has title strings under its `File name` column rather than full paths.
Embedded TITLE values match all files. There are 430 files with one consistent
CSV title-label tuple, and four files affected by two colliding titles with
conflicting labels. Matching each candidate against the source-labelled embedded
key and energy resolves a unique CSV BPM tuple for all 434 files. Preserve this
join provenance; do not silently assume title uniqueness.

## Embedded reference labels

FLAC Vorbis comments contain Base64-encoded JSON:

- `KEY`: `source: mixedinkey`, `algorithm: 94`, a Camelot `key`.
- `ENERGY`: `source: mixedinkey`, `algorithm: 13`, `energyLevel`.
- `CUEPOINTS`: `source: mixedinkey`, `algorithm: 14`, ordered `cues` containing
  `name` and `time`.

All 434 files have those source and algorithm values. The exact app version
that produced every label is not recorded, so retain it as unknown rather than
inferring it from the currently installed application version. Plain `BPM` tags
do not carry equivalent provenance; retain them separately from CSV references.

These are vendor-reference labels for agreement testing. They are not claimed
to be independently verified musical truth. In particular, vendor-reference
agreement and independently annotated key/beat accuracy are separate metrics.

### Cue timing units and boundaries

Static ARM64 inspection of `MIKTag.framework` proves the embedded JSON uses
milliseconds, including fractional milliseconds:

REA decompilation corroborates the conversion and serialized fields:
`ev_1d32e35b9de081810d4401139d510de7eede8ed83258b61d29247dbf3418556e`
(JSON reader) and
`ev_8a3e9ad11c2eb2109cf830634dfcfad79680d0d371697f7050db55778a982419`
(JSON writer). Complete records are in
`.analysis/mixed-in-key-11.2.6/rea-tag-evidence.json`.

- `MIKTagCuePoints::updateWithJSONDict:` at `0x232f8` divides the JSON time by
  1000.0 at `0x23400` before constructing an internal cue.
- `MIKTagCuePoints::JSONDict` at `0x2352c` multiplies internal time by 1000.0 at
  `0x2364c` before serialization.
- The constant at `0x5c5b8` has bytes `0000000000408f40`, a little-endian double
  equal to 1000.0.
- `MIKTagFLAC::mikCuePoints` at `0x28edc` reads the `CUEPOINTS` field.

For example, a raw value of 30920.19 means 30.92019 seconds. This illustration
is about units, not a claim of a particular track's result.

383 files have eight cues; the remaining files have three through seven. All
cue arrays are chronological. Seventy-three files have a cue slightly before
zero; the minimum is about -6.639 milliseconds. No cue exceeds its track's end
using a 100 ms end tolerance. Preserve raw reference times. Define an explicit
export boundary policy rather than silently changing reference labels.

The embedded cue format preserves name and time but has no explicit hot-cue
slot, color or beatgrid fields. A separate user export would help validate
those additional fields; it is not required to begin timing/name comparisons.

## Frozen split and leakage prevention

The first manifest assigns all aliases in a declared-audio group to the same
split using a deterministic hash, with no group overlap:

| Split | Audio groups | Files including copies |
| --- | ---: | ---: |
| Tuning | 282 | 328 |
| Held-out | 82 | 106 |

This is a first frozen split; no engines have been selected or tuned against
these held-out results. Only 17 of 24 Camelot labels occur in the full corpus,
and the held-out subset covers 13. The missing full-corpus labels are 1B, 2B,
3B, 6B, 8B, 9B and 12B. Reference tempos span 104–144 BPM. A larger library is
therefore useful both for more tracks and for missing key/tempo/style coverage.

Analysis engines must receive decoded samples, sample rate and explicitly
allowed configuration only. Never use existing KEY, INITIALKEY, BPM, ENERGY or
CUEPOINTS tags to generate predictions. Keep input manifests and reference
labels separate. Do not ship audio-hash-to-answer lookup tables as an estimator.
Keep copied tracks and later identified alternate versions out of opposing
splits. Freeze any selected engine before opening held-out mismatch details;
once tuned against those details, those tracks no longer constitute a fresh
holdout. Reserve a future larger library for an external evaluation.

## Evaluation and intended outcomes

The desired outcome is high agreement with Mixed In Key on unseen audio, not
merely successful reference extraction. Current tuning measurements and their
training/validation limits are in `docs/research/engine-benchmarks.md`.

- Key: exact tonic/mode and exact Camelot agreement; weighted harmonic errors
  and confusion matrices separately. Treat enharmonic names consistently.
- BPM: exact displayed-value agreement plus absolute error and separate
  half/double-time categories. Use the same declared display precision.
- Cues: compare count, chronological order, names, and one-to-one timing
  matches. Report raw time errors, tolerance-based precision/recall and beat/
  bar offsets where independently established. A predicted cue must not match
  multiple references.
- Export: preserve track identity and verify cue positions/slots and any
  supplied beatgrid by importing into a disposable Rekordbox test playlist.
  XML validity alone does not establish successful import behavior.

Target full agreement where achievable, retain a per-track mismatch ledger,
and measure the remaining gap. If a lightweight offline engine is insufficient,
use the vendor-labelled tuning subset for calibration or a trained model while
keeping validation and deployment preprocessing identical.

## Rekordbox interoperability

The [official Rekordbox XML specification](https://cdn.rekordbox.com/files/20200410160904/xml_format_list.pdf)
defines track locations as URIs, `TEMPO` records for beatgrids and
`POSITION_MARK` records for cues/loops. Position times are seconds. Hot-cue
numbers start at zero; memory cues use -1. See the
[official developer entry point](https://rekordbox.com/en/support/developer/?lang=en).
Preserve source names and order, use an explicit slot policy when none was
exported, and review near-zero clipping in the export report.

Start with a newly written XML export, without editing the user's live
Rekordbox database. Round-trip tests must distinguish memory cues, hot cues,
loops and beatgrid alignment. A beginner-focused editor should make useful
cue points visible, editable and previewable before export.

## Local artifacts

Private reference manifests are under `.analysis/reference-corpus/beatport-v1/`:

- `inputs.jsonl`: file identities and input metadata, without reference labels.
- `references.jsonl`: source-labelled key/energy/cues and CSV-derived BPM.
- `summary.json`: counts, coverage, split policy and source CSV digest.

The local repeatable audit is `.analysis/reference-corpus/audit.py`. Do not
publish these manifests, music files, local paths or per-track labels in the
open-source repository by default. This document records aggregate findings.
