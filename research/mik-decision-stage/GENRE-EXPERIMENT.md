# Task: build and package the genre test for the Mixed In Key service

Question: does the genre tag that Mixed In Key sends change the key, energy or cue decisions of its service? The patents say
cue weights "may be chosen by genre", and 98% of the first 4,000 songs are melodic house and techno, so the captures cannot
answer it. This test can: **identical audio, only the genre tag differs**. If every output stays the same across 12 variants
of a track, genre is not an input. If something changes, the pairs show which output and how.

You are on the laptop that has Mixed In Key 11, the music and the earlier `~/mik-gather` folder. You prepare the files and
package the result. The owner does the scanning. **Do not analyse anything on this device** beyond the counts the tool prints.

## Rules

- Read-only on the music, the Mixed In Key database and its preferences. The tool copies a few FLAC files and edits the tags
  of the **copies** only. Do not launch, script or modify Mixed In Key; the owner scans.
- Never replay a request or contact Mixed In Key's service. Never print, paste, upload or commit the flow file, file names,
  titles or paths. Nothing leaves this machine except the package folder that the owner copies by hand.
- Do not read the Mixed In Key preferences file or any licence or login data.
- Leave the scripts alone. If one fails, make the smallest fix, say so in `NOTES.md` and keep the diff.

## Files in this folder

| File | Use |
| --- | --- |
| `genre_experiment.py` | `make` (prepare the copies), `pack` (flow file to package, with checks), `verify` |
| `import_capture.py` | used by `pack`: saved flows to sanitized request/response pairs (offline, credentials redacted) |
| `requirements-gather.txt` | `mutagen` |

## Steps

Keep `~/NOTES-genre.md` as you go: date, each command and how long it took, versions, free disk, every warning.

1. **Preflight.**
   ```sh
   python3 --version && python3 -m pip install --user mutagen
   mitmdump --version
   ls ~/mik-gather/final.jsonl            # from the first task; needed to choose the tracks
   df -h ~                                # about 12 GB for the default 24 tracks, plus 3 GB spare
   ```
2. **Make the files.**
   ```sh
   python3 genre_experiment.py make --gather ~/mik-gather --out ~/genre-test --tracks 24
   ```
   It chooses 24 FLAC tracks spread over low, middle and high key confidence, and writes 12 copies of each into
   `~/genre-test/files`: ten genres (`g00` is the track's own genre), `none` (no genre tag) and `meta` (own genre, title and
   artist replaced by neutral text). Every other tag and the cover art are removed from the copies; the audio is
   byte-identical. `plan.json` lists the variants by synthetic name. Use fewer `--tracks` if the disk is tight.
3. **Ask the owner to scan** (you do not): tell them exactly this, then wait.
   - Start the capture as before and save to a new file: `mitmweb -w ~/genre.flow`, with the same proxy settings as the big run.
   - In Mixed In Key add the folder `~/genre-test/files` and analyse it with unchanged settings. Do not edit tags in Mixed In Key.
     Mixed In Key writes its results into those copies; that is fine.
   - When the last file is done, stop mitmweb so the flows are saved, and quit Mixed In Key.
4. **Pack and check.**
   ```sh
   python3 genre_experiment.py pack --plan ~/genre-test/plan.json --flow ~/genre.flow --out ~/genre-handback
   ```
   It prints counts and ends with `VERIFY OK`. Write the printed numbers into `NOTES-genre.md` and mark each PASS or FLAG;
   a FLAG does not stop the job and nothing is repaired:

   | Check | PASS if |
   | --- | --- |
   | `files_captured` | equals `files_planned` (otherwise note which variants are missing as counts, do not rescan) |
   | `captured_more_than_once`, `error_responses` | 0 |
   | `genre_sent_equals_tag` | all of them (this proves the tag reaches the request) |
   | `tracks_with_identical_features_in_all_variants` | equals `tracks_compared` (this proves the audio inputs are the same) |
   | `verdict`, `field_differences_*` | record only; do not interpret |
5. **Final message to the owner**: package path and size, the check table, anything FLAGged, and: copy the whole
   `~/genre-handback` folder (not the flow file) to the research machine and run
   `python3 tools/genre_experiment.py verify <folder>` there.

## What comes back

`genre-pairs.zip` (sanitized request/response pairs of all variants), `plan.json` (variants, the stored key, confidence and
energy of each track from the earlier scan, no titles or paths), `genre_report.json` (counts only), `MANIFEST.json` (size and
SHA-256 of each file), your notes and the two scripts used.
