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

## Strongly wanted: the genre of every song

`EliteData.Genre` is the tag the application sent. The patents say weights may be chosen by genre, and the corpus so far
is 87% "Melodic House & Techno", so the other genres are where a genre effect would show. Keep the genre tag exactly as sent.

## Optional: the finished analysis as written to the files

A `final.jsonl` next to the pairs, one JSON object per line, lets the code cross-check the response against what the
application stored for the user: `{"id": ..., "key": "11A", "energy": 7, "cues": [{"time": 4.115, "energy": 5}, ...],
"comment": "11A - Energy 7"}`. The rekordbox library of the 3,400-track collection holds exactly this: the comment is
`<key> - Energy <n>`, and eight hot cues plus eight memory cues carry `Energy <n>` names at the cue times.

## Splits

`mikdata.split_of(feature_hash)` holds out one song in five by a hash of the song only. Before anything else, drop every
song whose feature hash is in `sealed_corpus_holdout_feature_sha1.txt` (`--deny-hashes`): those are the held-out songs of
the first 434-file corpus, sealed for one final scoring. The first corpus is also available as a tuning list via
`--allow-names`.
