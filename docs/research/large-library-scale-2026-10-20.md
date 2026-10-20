# Large-library scale measurements, 2026-10-20

Aggregate numbers only. One machine: Apple silicon, 14 logical cores, APFS, release build. The runs use the library
crate directly (no desktop window), so they measure the code under the Maintenance screens, not screen rendering.

## What was run

`crates/cuerithm-library/tests/scale_benchmark.rs`, ignored by default:

```sh
CUERITHM_SCALE_TRACKS=10000 cargo test -p cuerithm-library --release --test scale_benchmark large_library -- --ignored --nocapture
CUERITHM_SCALE_REAL_DIR=<folder of audio copies> cargo test -p cuerithm-library --release --test scale_benchmark real_audio -- --ignored --nocapture
```

The first generates a library of one-second 8 kHz mono WAV files in which every 20th track is a copy of the one
before it (same audio, different file bytes, so the scan has to decode to match them). The second takes a folder of
real audio and adds a byte-identical copy of every 20th file. Both import the files, run a cold and a warm duplicate
scan, start a review, reload it, make one edit, then preview, apply and undo the merge of every duplicate group. The
generated run also reopens the library and checks that a sample of 200 source files has the same hash afterwards.
Every step asserts its result (group counts, archived counts, undo restoring all tracks).

## Results

Generated libraries:

| Step | 10,000 tracks | 50,000 tracks |
| --- | ---: | ---: |
| Import (enqueue) | 2.6 s | 28.4 s |
| Duplicate scan, cold (every file hashed and fingerprinted) | 54.3 s | 286 s |
| Duplicate scan, warm (fingerprints cached) | 0.39 s | 3.9 s |
| Begin review (saved review row) | 0.19 s (16.3 MB) | 1.7 s (81.7 MB) |
| Reload review | 0.05 s | 0.22 s |
| One review edit | 0.09 s | 0.61 s |
| Preview merge of all duplicate groups (500 / 2,500) | 0.11 s | 0.52 s |
| Apply merge | 0.16 s | 0.92 s |
| Undo merge | 0.03 s | 0.18 s |
| Peak resident memory | 280 MB | 828 MB |

Real audio, 364 FLAC files of about six minutes each plus 19 copies (383 files, 15.4 GB), 19 duplicate groups:

| Step | Time |
| --- | ---: |
| Duplicate scan, cold | 40.8 s (9.4 files/s, 377 MB/s of source audio) |
| Duplicate scan, warm | 6.8 s |
| Begin review | 3.4 s |
| Reload review, one edit, preview, apply, undo | 0.34 s each |
| Peak resident memory | 39 MB |

Everything grows about linearly with the number of tracks. All groups were found, undo restored every merged track,
and no sampled source file changed. Extrapolating the real-audio numbers linearly to 10,000 such files gives roughly
18 minutes for a cold scan, 3 minutes for a warm one, 90 seconds to begin a review, and about 9 seconds for each review
click if 5% of the files sit in duplicate groups; these are estimates, not measurements.

## What changed because of this

Three things were serial or re-read the whole library, and the generated one-second files hid how much that costs on
real files.

- **The cold scan** hashed and fingerprinted one file at a time. It now runs on the pool the preparation service uses
  (75% of the logical cores, rounded down), returns results in library order and reports progress as each file
  finishes. At 10,000 generated tracks that took 521 s and now takes 54 s with the same 500 groups.
- **Every review action re-hashed every scanned file, serially.** Reloading, editing, previewing, applying or undoing a
  review took 29 s on 364 real files. The hashing now runs on the same pool (29 s became 3 s), and a review only
  proves the files of the tracks in its duplicate groups unchanged, since those are all it can act on. The merge
  journal likewise records only those files for Undo. The library and scope checks still cover every track, and the scan
  itself still proves every file. On the same real files a review action now takes 0.34 s, and at 50,000 generated
  tracks a click went from 3.9 s to 0.6 s.
- Tests cover order, progress and cache counts under the pool, and that a review stays fresh when a file outside its
  groups changes and goes stale when a group member changes, while the scan itself notices both.

## What this does not show

- Screen rendering and the packaged app: the Maintenance screens were not walked through on a large library.
- Real user libraries: tags, formats, folder layouts and file sizes are uniform here, and the audio in the large runs is
  generated.
- Other maintenance scans (unused files, broken tracks, unused-file removal and backup disposal), analysis throughput,
  provider throughput and retained recovery-storage growth.
- The saved review is still one row that is read and written whole on every edit: 16 MB at 10,000 tracks and 82 MB at
  50,000 with 5% duplicates. It costs 0.09 s and 0.61 s per edit now, and grows with the library and the duplicate count.
- Beginning a review still proves every scanned file unchanged (3.4 s for 383 real files, so about 90 s for 10,000).
- One machine, one run per size.
