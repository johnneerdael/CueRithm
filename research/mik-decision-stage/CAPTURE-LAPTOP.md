# On the laptop that has Mixed In Key and the music

Everything below runs on that laptop. Nothing is written to the Mixed In Key database, the music or the flow file, and nothing
is sent anywhere. Everything produced is private (file names and paths): do not commit or publish it.

## 1. Once, before the scan

- Python 3 (macOS has `/usr/bin/python3`) and the tag reader: `python3 -m pip install --user mutagen`.
- mitmproxy, which you already use for mitmweb. It also provides `mitmdump`, which the tool uses to read the flow file.
- Free disk space of about the size of the Mixed In Key database for the backup (the first 434 songs took 0.8 GB, so 4,000+
  songs will need roughly 7 GB). `--out` can point at an external drive.
- Start mitmweb so that it writes the flows to a file instead of keeping them only in memory:
  `mitmweb -w ~/capture.flow`. A scan of 4,000+ songs makes a large file. Keep it private: it still holds the licence
  code and other requests. You do not need to send it anywhere; the tool turns it into sanitized pairs on this laptop.

## 2. Trial run (any time, even while the scan is running)

```sh
python3 gather_mik_data.py --out ~/mik-gather-trial --limit 10 --skip backup --allow-running
```

It reads ten songs and prints what it found. Check `~/mik-gather-trial/report.json`.

## 3. The real run (after the scan has finished)

Quit Mixed In Key and let mitmweb finish writing, then:

```sh
python3 gather_mik_data.py --out ~/mik-gather --flow ~/capture.flow
```

Add `--music-dir "/path/to/scanned/folder"` if the scanned folder is not simply where the analysed files live, and one
`--flow` for each flow file if there are several. Steps can be skipped with `--skip backup,export,inventory,tags,pairs`.

| Step | Output | What it holds |
| --- | --- | --- |
| backup | `Collection11.backup.mikdb` | a consistent copy of the whole Mixed In Key database (beat grids, cues, segments, waveforms) |
| export | `final.jsonl` | per song: key, energy, tempo, genre, cues, energy and key segments, decoded beat times, the stored tag record, and every other stored value |
| inventory | `inventory.jsonl` | every file under the scanned folders: name, extension, size, whether Mixed In Key analysed it |
| tags | `tags.jsonl` | every tag frame written to each audio file (key, energy, cue and beat-grid records, Serato markers), no artwork, no audio |
| pairs | `mik-pairs.zip` | the sanitized request/response pairs (offline), checked against the database |
| setup | `setup.json`, `report.json` | app build, system, analysis-related settings, and aggregate counts |

## 4. Bring back

`mik-metadata.zip`, `mik-pairs.zip` and `Collection11.backup.mikdb` from the output folder. Do not bring the flow file or
`com.mixedinkey.application.plist` (the preferences hold a licence code). `gather.log` explains anything that failed.

`report.json` also says how many captured calls matched a database song, how many database songs were not captured
(`missing_from_capture.txt` lists them: re-analyse those and run the tool again), and whether the stored result equals the
server response.
