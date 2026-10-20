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
- **CueRithm.** `camelot analyze --lexicon-timing --lexicon-tempo 78_155 --lexicon-cues --lexicon-energy`.
- **MIK column.** The base64-JSON tags that Mixed In Key wrote into the original files (`source: mixedinkey`).
  This is not a fresh MIK run, and the MIK version that wrote them is not established.
- Identical audio gives identical results: no disagreement across the 42 songs with more than one copy, in either tool.

## Results (tuning split, n = 282)

| Measure | CueRithm vs Lexicon | CueRithm vs MIK tags | Lexicon vs MIK tags |
| --- | ---: | ---: | ---: |
| BPM within 0.001 | 282/282 (100%) | 239/282 (84.8%) | 239/282 (84.8%) |
| First beat within 1 ms | 275/282 (97.5%) | 43/282 (15.2%) | 44/282 (15.6%) |
| First beat within 5 ms | 277/282 (98.2%) | 153/282 (54.3%) | 154/282 (54.6%) |
| Same cue count | 276/282 (97.9%) | — | — |
| Every cue within 5 ms | 270/282 (95.7%) | — | — |
| Key exact | 116/282 (41.1%) | 101/282 (35.8%) | 213/282 (75.5%) |
| Key adjacent on the wheel | 42/282 (14.9%) | 40/282 (14.2%) | 24/282 (8.5%) |
| Energy exact | 54/282 (19.1%) | 182/282 (64.5%) | 85/282 (30.1%) |
| Energy within one level | 243/282 (86.2%) | 266/282 (94.3%) | 236/282 (83.7%) |

Lexicon stores keys in Open Key notation (`11M` minor, `11D` major); the mapping to Camelot was checked against the
MIK tags (`9M` is `4A`, `1M` is `8A`, `6M` is `1A`). MIK cues are phrase positions named "Energy N", not structural
cues, so cue comparison with MIK is not meaningful.

## What the numbers say

- **Timing and cues.** CueRithm reproduces Lexicon's BPM on every song and its grid and cues on about 96–98%. The 14
  songs that differ fail identically under the default 98–195 preset, so they are timing-engine corner cases (anchors
  a whole number of beats off, or one more cue), not a preset effect. Only the 78–155 preset was compared.
- **Energy.** CueRithm's rating is Lexicon's published local formula. It tracks MIK's tags (64.5% exact) better than
  Lexicon's own rating does (30.1%) but does not reproduce Lexicon's rating on real music. Probed on 12 songs, Lexicon
  feeds the formula the left channel decimated to 22.05 kHz without a low-pass filter, which reproduced its values on
  all 12; CueRithm averages the channels and low-passes first. That makes the formula score one level lower on most
  tracks. The earlier bundled SVC, fit to the captured MIK segment labels and scored on its own training songs, matched
  MIK's overall level on 58.5%, below the formula.
- **MIK energy.** MIK's tag rating is a local aggregation of segment levels that its server returns, so a local
  rating can only imitate it. The earlier replacement of the SVC by the Lexicon formula (2026-10-18) was made for
  independence from a model file and a server and was not compared with MIK.
- **Key.** CueRithm agrees with MIK on 35.8% and Lexicon on 75.5%. Key is the largest gap to MIK.
