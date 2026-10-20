# Task: package a Mixed In Key capture for hand-back

You are on the laptop that has Mixed In Key 11 and the music. The owner's prompt gives the path of a mitmproxy flow file
(call it `$FLOW`) holding the analysis calls of a clean scan of 4,000+ tracks. Your job is to **gather, check and package**
the data so the researchers on another machine can continue from it alone.

**Do not analyse anything on this device.** No fitting, no scoring, no experiments, no interpreting of the music data, no
clone of the CueRithm repository. All analysis happens on the other machine, from your package.

## Rules

- Everything is read-only: the Mixed In Key database (the tool copies it through sqlite's backup interface), the music and
  the flow file. Do not launch, modify or script Mixed In Key. Do not write music tags.
- Never replay a request or contact Mixed In Key's service. Never print, paste, upload or commit the flow file, the database,
  file names, titles or paths. Nothing leaves this machine except the package folder that the owner copies by hand.
- Do not read the Mixed In Key preferences file or any licence or login data yourself. The tool reads an allow-list of
  analysis settings and nothing else.
- Leave the three scripts alone. If one fails on this machine, make the smallest fix, say so in `NOTES.md` and keep the diff.
- If a step fails, read `gather.log`, fix the environment (for example install `mutagen`) and rerun. If the step needs the
  owner (Mixed In Key still open, disk full, flow file missing), stop and say exactly what is needed.

## Files in this folder

| File | Use |
| --- | --- |
| `gather_mik_data.py` | database backup, per-song export with beat grids, file inventory, tag dump, flow import, capture check |
| `import_capture.py` | used by the gatherer: flow file to sanitized request/response pairs (offline, credentials redacted) |
| `build_handback.py` | builds the hand-back package and verifies it |
| `requirements-gather.txt` | `mutagen` |

## Steps

Keep `~/NOTES.md` as you go (see the end).

1. **Preflight.**
   ```sh
   python3 --version                # 3.8 or newer
   python3 -m pip install --user mutagen
   mitmdump --version               # comes with mitmproxy, next to mitmweb
   pgrep -fl "Mixed In Key"         # must print nothing; ask the owner to quit it otherwise
   ls -lh "$FLOW" && shasum -a 256 "$FLOW"
   df -h ~                          # about 1.3 times the size of ~/Library/Application Support/Mixedinkey/Collection*.mikdb, plus 3 GB
   ```
2. **Trial** (about a minute): `python3 gather_mik_data.py --out ~/mik-gather-trial --limit 10 --skip backup`. It should name
   the Mixed In Key build and read ten songs. The owner expects build 11.2.6 (7446); record the build either way.
3. **Full run.** The database backup, the tag dump and the flow import take a while, so run it in the background and wait:
   ```sh
   python3 gather_mik_data.py --out ~/mik-gather --flow "$FLOW"
   ```
   One `--flow` per file if the capture is split. Add `--music-dir "<folder>"` for each scanned folder only if the log's list
   of scanned folders looks wrong (the tool derives them from the analysed files). Do not use `--limit` here.
4. **Checks.** Read `~/mik-gather/report.json` and write every number below into `NOTES.md`, marked PASS or FLAG. A FLAG does not
   stop the job and nothing is repaired: state the number and carry on.

   | Check | Field | PASS if |
   | --- | --- | --- |
   | songs in the database | `export.songs` | about 4,000 or more, equal to the owner's scan count if known |
   | captured calls joined to a song | `capture.pairs_joined` over `export.songs` | at least 98% |
   | database songs without a captured call | `capture.database_songs_without_pair` | under 2% of songs (`missing_from_capture.txt` lists them; do not rescan) |
   | captured calls without a song | `capture.pairs_without_database_song` | under 2% of pairs |
   | calls with an error or no result | `capture.pairs_with_error_or_no_segments` | record only |
   | stored result equals the response | `capture.key_equal`, `energy_equal`, `tempo_equal_1e-3`, `cue_count_equal`, `response_cues_all_in_database`, `downbeat_on_stored_grid_10ms` | each close to `pairs_joined`; record all |
   | tags read | `tags.read`, `tags.error`, `tags.missing` | errors and missing small; record |
   | files in the scanned folders | `inventory.files`, `inventory.audio`, `inventory.audio_not_in_database`, extension counts | record only |
   | settings and build | `setup.mik_version`, `setup.mik_build`, `setup.settings` | record |

5. **Package.**
   ```sh
   python3 build_handback.py --gather ~/mik-gather --notes ~/NOTES.md --out ~/mik-handback
   ```
   It ends with `VERIFY OK`. Use `--compress-db` only if the owner asks for a smaller package. If verification fails, say what
   it printed and rebuild from the gather folder; do not hand over a package that failed.
6. **Final message to the owner**: the package path and size, the check table, anything FLAGged or failed, and these
   instructions. Copy the whole `~/mik-handback` folder to the research machine (not the flow file and not
   `com.mixedinkey.application.plist`, which holds a licence code), then run `python3 tools/build_handback.py --verify <folder>`.

## `NOTES.md`

Facts only, written by you: date; each command run and how long it took; versions (macOS, Python, mitmproxy, Mixed In Key
build); the flow file's size and SHA-256; free disk space before; the check table; every error or warning and what you did.
No titles, artists, paths or other per-track information, and no conclusions about the music data.

## What the package holds

| Item | What it is |
| --- | --- |
| `mik-pairs.zip` | the sanitized `<id>.request.json` / `<id>.response.json` pairs: credential-looking values replaced, every other number exactly as received |
| `mik-metadata.zip` | `final.jsonl` (per song: stored key, energy, tempo, genre, cues, segments, decoded beat grid and every other stored value), `tags.jsonl` (every tag frame written to each file, no artwork), `inventory.jsonl` (every file under the scanned folders), `setup.json`, `report.json`, `missing_from_capture.txt`, `gather.log` |
| `Collection11.backup.mikdb` | a consistent copy of the whole Mixed In Key database |
| `MANIFEST.json`, `HANDBACK.md`, `NOTES.md`, `tools/` | sizes and SHA-256 of everything, counts, how to continue, your notes, the exact scripts used |
