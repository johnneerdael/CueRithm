# Dataset format

The research code reads a folder of request/response pairs and nothing else. Audio is not needed for the decision-stage
models: every request already contains the features the server saw, and the client side was checked to reproduce them
exactly from audio (see AGENT_TASK.md).

## Required

For every analysed song, two files with the same stem, in one folder (a flat folder of thousands of pairs is fine):

- `<id>.request.json` - the body of the analysis request, sanitized: every value under a key that looks like a credential,
  session, licence or VIP code is replaced by the string `"[REDACTED]"`; numbers are kept exactly as received (do not
  re-serialize or round them). Fields the code reads: `Tempo`, `DownbeatTime`, `DurationInSeconds`, `CuePointStartBeat`,
  `CuePointData` (base64), `Segments[0].Features` (352 numbers), `Segments[0].PitchProbabilities`,
  `EnergySegmentData[]` (`StartTime`, `EndTime`, `VolumeRmsDb`, `Features` with 183 numbers), and
  `EliteData.Genre`, `EliteData.FileName` (the file name, used only to filter and to report).
- `<id>.response.json` - the sanitized response: `Segments[0].Key`, `KeySummary` (`MainKey`, `MainKeyConfidence`,
  `SecondKey`), `EnergyLevelSegments[]` (`EnergyLevel`, `StartTime`, `EndTime`, `Volume`), `OverallEnergy`,
  `CuePoints[]` (`Time`, `EnergyLevel`), `Error`.

Pairs whose response has an `Error`, or no `Segments`, are skipped. Songs with the same `Segments[0].Features` are one song
(the loader keeps the first). Capture only your own licensed copy's traffic, keep the raw captures private and never replay
a request to the vendor's server.

## Making the folder from a capture

If you captured your own licensed copy's traffic with mitmproxy/mitmweb and saved the flows to a file (`mitmweb -w capture.flow`,
or File > Save in the web interface), convert it offline with the addon in this folder:

```sh
mitmdump -q -nr capture.flow -s import_capture.py --set out_dir=DIR
```

It keeps only the analysis calls, redacts credential-looking values, keeps every other number exactly as received and writes
`import-summary.json` (counts and the genre tally only). The flow file itself still holds credentials from the other
requests: keep it private, never commit it, and never replay it. A large capture is better streamed to a file with `-w`
than left in the web interface's memory.

## Genre

The genre tag the application sends (`EliteData.Genre`) does not change the service's response: a controlled test with 24 tracks in 12 tag
variants each gave identical responses (see AGENT_TASK.md). Keep the tag as sent anyway, but there is no need to collect songs per genre for
that reason. Variety of audio (keys, tempo, structure, major keys in particular) is still what makes the fits better.

## Optional: the stored analysis, tags and file inventory (`gather_mik_data.py`)

`gather_mik_data.py` runs on the machine that has Mixed In Key and writes, next to the pairs:

- `final.jsonl`, one object per analysed song: `id`, `file`, `path`, `key`, `second_key`, `energy`, `tempo`, `genre`,
  `cues` (`time`, `name`, `energy`), `energy_segments` (`start`, `length`, `energy`), `key_segments` (`start`, `length`,
  `key`, `confidence`, `energy`, `volume`), `beats` (`tempo` and the decoded `times` of the stored beat grid),
  `tag_comments` (the stored tag record, decoded) and `db` (every other stored column, timestamps as ISO text). The join
  to a captured pair is the file name (`EliteData.FileName` against `file`, extension and Unicode form ignored), with the
  tempo as a tie-break.
- `tags.jsonl`: per audio file the container, length, sample rate, channels, bit depth, bitrate and every tag frame
  (binary frames as base64, artwork dropped and counted).
- `inventory.jsonl`: every file under the scanned folders with its extension and size.
- `Collection11.backup.mikdb`: a consistent copy of the whole database, for anything not decoded above.
- `report.json`: counts only, including how many captured calls matched a database song and whether the stored key, energy,
  tempo, cue count and cue times equal the response (on the first corpus all 434 did).

## Splits

`mikdata.split_of(feature_hash)` holds out one song in five by a hash of the song only. Before anything else, drop every
song whose feature hash is in `sealed_corpus_holdout_feature_sha1.txt` (`--deny-hashes`): those are the held-out songs of
the first 434-file corpus, sealed for one final scoring. The first corpus is also available as a tuning list via
`--allow-names`.
