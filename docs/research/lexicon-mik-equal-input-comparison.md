# Equal-input comparison with Lexicon and Mixed In Key tags, 2026-10-20

Aggregate results only. Per-track detail stays under `.analysis/`.

## Method

- **Songs.** The 434 files of the Beatport development folder are 364 distinct songs (70 files are exact
  duplicates by FLAC audio checksum). One file per song is counted. The reference corpus fixes the split by a hash
  of the audio group: 282 songs in the tuning split, 82 held out. **Every number below is the tuning split
  (n = 282).** The held-out split stays sealed until the engine and configuration are frozen.
- **Disclosure.** A first pass over all 364 songs, including the held-out ones, printed only blended totals. Those
  reports are quarantined and not used. The comparison script now refuses to read the held-out split.
- **Inputs.** A clone of the audio with every tag removed except title, artist and remixer: no BPM, key, energy, cue,
  beatgrid, genre, release date or artwork. The audio checksums equal the originals'. Neither tool received a genre.
- **Lexicon 1.11.15.** Own analysis with tempo range 78–155, key and energy analysis on, and the Cue Point Generator
  with the Default template (overwrite on, start at first beat, breakdown minimum 64 beats, drop at start never,
  no custom anchors, no emergency loop). Read from a read-only copy of its database.
- **CueRithm.** `camelot analyze --lexicon-timing --lexicon-tempo 78_155 --lexicon-cues --lexicon-energy` for timing,
  cues and energy, and `camelot analyze --key-openkeyscan-model models/builtin/openkeyscan3-logits.onnx` (built with
  the `openkeyscan-key` feature) for key, which is the model the desktop uses. A first key pass used the CLI's fallback
  estimator by mistake and reported 35.8% against MIK; that figure was wrong and is withdrawn.
- **MIK column.** The base64-JSON tags that Mixed In Key wrote into the original files (`source: mixedinkey`).
  This is not a fresh MIK run, and the MIK version that wrote them is not established.
- Identical audio gives identical results: no disagreement across the 42 songs with more than one copy, in either tool.

## Results (tuning split, n = 282)

| Measure | CueRithm vs Lexicon | CueRithm vs MIK tags | Lexicon vs MIK tags |
| --- | ---: | ---: | ---: |
| BPM within 0.001 | 282/282 (100%) | 239/282 (84.8%) | 239/282 (84.8%) |
| First beat within 1 ms | 275/282 (97.5%) | 43/282 (15.2%) | 44/282 (15.6%) |
| First beat within 5 ms | 277/282 (98.2%) | 153/282 (54.3%) | 154/282 (54.6%) |
| Published cue set within 5 ms | 276/282 (97.9%) | — | — |
| Grid and published cues both agree | 274/282 (97.2%) | — | — |
| Key exact | 213/282 (75.5%) | 274/282 (97.2%) | 213/282 (75.5%) |
| Key adjacent on the wheel | 23/282 (8.2%) | 6/282 (2.1%) | 24/282 (8.5%) |
| Energy exact | 54/282 (19.1%) | 182/282 (64.5%) | 85/282 (30.1%) |
| Energy within one level | 243/282 (86.2%) | 266/282 (94.3%) | 236/282 (83.7%) |

Lexicon stores keys in Open Key notation (`11M` minor, `11D` major); the mapping to Camelot was checked against the
MIK tags (`9M` is `4A`, `1M` is `8A`, `6M` is `1A`). MIK cues are phrase positions named "Energy N", not structural
cues, so cue comparison with MIK is not meaningful.

## What the numbers say

- **Timing and cues.** CueRithm reproduces Lexicon's BPM on every song, its first beat on 97.5% and its published cue
  set on 97.9%; grid and cues agree together on 274/282 (97.2%). Cues here are the published set, after the template
  rule that Lexicon's renderer applies and `lexicon_publication.rs` ports (a Second Drop or Second Breakdown exists
  only if its first one exists and differs in time). An earlier reading of the raw detector output showed 14
  disagreements; seven of those were that rule and are not differences in the app.
- **The remaining seven songs.** They all differ in the first beat, and the cues follow. What is established: Lexicon's
  result for them is reproducible (re-analysing at an explicit 78–155 with overwrite on changed none of the 282 grids);
  Lexicon's original analysis worker, run in Node on our decoded audio at 78–155, returns exactly CueRithm's first beat
  on the five of them tested, so the port is faithful to the shipped worker; no single other preset reproduces them
  (68–135 reproduces three of five); and the app's decoded length equals the file's true length on every song, so
  decoder trimming is excluded. The cause is therefore in an input or runtime detail of the app that is not visible in
  the shipped code, and is unknown. Only the 78–155 preset was compared across the whole split.
- **Energy.** CueRithm's rating is Lexicon's published local formula. It tracks MIK's tags (64.5% exact) better than
  Lexicon's own rating does (30.1%) but does not reproduce Lexicon's rating on real music. Probed on 12 songs, Lexicon
  feeds the formula the left channel decimated to 22.05 kHz without a low-pass filter, which reproduced its values on
  all 12; CueRithm averages the channels and low-passes first. That makes the formula score one level lower on most
  tracks. The earlier bundled SVC, fit to the captured MIK segment labels and scored on its own training songs, matched
  MIK's overall level on 58.5%, below the formula.
- **MIK energy.** MIK's tag rating is a local aggregation of segment levels that its server returns, so a local
  rating can only imitate it. The earlier replacement of the SVC by the Lexicon formula (2026-10-18) was made for
  independence from a model file and a server and was not compared with MIK.
- **Key.** OpenKeyScan agrees with MIK's tags on 97.2% (274/282), matching the earlier development measurement of
  274/282, and agrees with Lexicon's own key on 75.5%. Lexicon's key is itself 75.5% against MIK, so CueRithm is
  much closer to MIK than Lexicon is.
