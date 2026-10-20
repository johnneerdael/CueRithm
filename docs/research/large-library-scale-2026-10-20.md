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
before it (same audio, different file bytes, so the scan has to decode to match them). It imports them, runs a cold and
a warm duplicate scan, starts a review, makes one review edit, previews, applies and undoes the merge of every
duplicate group, reopens the library, and checks that a sample of 200 source files has the same hash afterwards. Every
step also asserts its result (group counts, archived counts, undo restoring all tracks). The second runs the scan over
real audio files, which shows decode and hashing cost that one-second files cannot.

## Results

| Step | 10,000 tracks | 50,000 tracks |
| --- | ---: | ---: |
| Import (enqueue) | 3.9 s | 15.8 s |
| Duplicate scan, cold (every file hashed and fingerprinted) | 60.2 s | 285 s |
| Duplicate scan, warm (fingerprints cached) | 0.95 s | 5.6 s |
| Begin review (saved review row) | 0.73 s (16.3 MB) | 3.9 s (81.8 MB) |
| Reload review | 0.72 s | 3.5 s |
| One review edit | 0.73 s | 3.9 s |
| Preview merge of all duplicate groups (500 / 2,500) | 0.71 s | 3.9 s |
| Apply merge | 0.80 s | 4.3 s |
| Undo merge | 0.67 s | 3.6 s |
| Peak resident memory | 307 MB | 1,048 MB |

Everything grows about linearly with the number of tracks. All groups were found, all merged tracks were restored by
undo, and no sampled source file changed.

Real audio, 364 FLAC files (14.6 GB, about six minutes each): the cold scan took 62.9 s (5.8 files per second, 232 MB/s
of source audio) and the warm scan 30.9 s, because the scan hashes every file again to prove the cache still matches.
Resident memory stayed near 24 MB. Extrapolating linearly to 10,000 such files is about 29 minutes cold and 14 minutes
warm; that is an estimate, not a measurement.

## What changed because of this

The cold scan used to hash and fingerprint one file at a time. At 10,000 generated tracks that took 521 s. It now runs
on the same pool the preparation service uses (75% of the logical cores, rounded down), returns results in library
order, reports progress as each file finishes, and produces the same report: 521 s became 60 s with the same 500
groups. `maintenance_duplicates.rs` has a test for order, progress counts and cache counts under the pool.

## What this does not show

- Screen rendering and the packaged app: the Maintenance screens were not walked through on a large library.
- Real user libraries: tags, formats, folder layouts and file sizes are uniform here, and the audio is generated.
- Other maintenance scans (unused files, broken tracks, unused-file removal and backup disposal), analysis throughput,
  provider throughput and retained recovery-storage growth.
- The review is one saved row that is read and written whole on every edit. It is 16 MB at 10,000 tracks and 82 MB at
  50,000 (with 5% duplicates), and each click costs 0.7 s and 3.9 s. Larger libraries or more duplicates will make that
  row, and each edit, larger.
- One machine, one run per size.
