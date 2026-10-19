# The analysis pipeline

Every track follows the same path from file to ready. This page walks it in order.

```text
file ──► wait until stable ──► hash ──► decode (bundled FFmpeg) ──► analyse ──► [match] ──► [tag + cleanup]
                                                                                     │
                       promote to Tracks  ◄── publish atomically ◄── verify staged copy ◄┘
```

Steps in brackets only run when you turned automatic tagging or cleanup on.

## 1. Waiting for the file

A watcher and a five-second rescan feed discovery. A file must be **stable**: its size and modified time must be unchanged across two checks, two seconds apart by default (validated between 10 ms and 60 s, but not exposed in the interface). If the file is already older than twice that interval it starts at once. A file still changing after 300 seconds fails with *File copy did not settle*. The file is then hashed with SHA-256. If the hash changes before work starts, the job is superseded and queued again with the new hash.

## 2. Decoding

The bundled FFmpeg first copies the file to a private snapshot, hashing it before and after to be sure nothing changed during the copy, and decodes the **first audio stream** into 32-bit float PCM at **44,100 Hz stereo**. Any decode error is fatal. See [Decoding and the media runtime](decoding.md).

## 3. Analysis

The engine reads that PCM and produces the four results.

| Result | Input | Method |
| --- | --- | --- |
| **Key** | Whole track, 44.1 kHz mono | [Constant-Q spectrum plus a neural network](key.md) |
| **Tempo and first beat** | Left channel, 44.1 kHz | [Local DSP with explicit correction rules](tempo.md) |
| **Energy** | Mono, 22.05 kHz | [A fixed acoustic formula](energy.md) |
| **Cues** | Left channel, 44.1 kHz, plus the BPM and anchor | [Section-based energy analysis](cues.md) |

Workers run in parallel, using about three quarters of your logical cores (rounded down, at least one). Identical source, settings, engine and decoder reuse a cached result. The cache key includes the application executable's hash, so a new build never reuses an old build's results.

Only the PCM and explicit settings are inputs. Tags, filenames and any reference labels are **not** used to predict key, BPM or energy. One deliberate exception: the genre tag is passed to the cue detector, and only the words "techno" and "hardcore" change its behaviour.

## 4. Matching, tagging and cleanup (optional)

With tagging on, CueRithm [matches the track against Beatport and Traxsource](matching.md). Tags (and cleanup edits, if on) are applied to a **staged copy** of the file, which is re-decoded and proven to contain identical audio. See [Safe tag writing](tag-writing.md).

If the final genre differs from the one used for cue detection, cues are recomputed with the measured timing reused. Key, BPM and energy do not change.

## 5. Publication and promotion

Before anything is promoted, the result is validated:

- analysis complete and the duration positive and finite,
- a key resolved,
- BPM greater than zero with a non-empty, finite beatgrid,
- energy between 1 and 10,
- at least one cue, finite and within the track.

Then, in a single SQLite transaction, CueRithm stores the analysis, writes key, BPM, energy and duration, inserts a new **cue revision** and sets the track's state to *ready*. If tags were written, the staged file has already been swapped in atomically and journaled.

!!! note "No tags, no write"
    If neither tagging nor cleanup is enabled, the audio file is never written. The track is analysed, its cues refreshed using whatever genre tag the file already has, and it is promoted.

## What is stored where

| Item | Library database | Audio file tags |
| --- | --- | --- |
| Key | Always (Camelot) | Only if the *Key* field is selected |
| BPM | Always (saved grid value) | If the *BPM* field is selected (on by default) |
| Energy 1 to 10 | Always | `ENERGY` custom field, on by default |
| Beatgrid | Yes | Never |
| Cues | Yes, as append-only revisions | Never |
| Waveform | Cache | Never |

## Failure semantics

A failed required stage sets the job to *attention* with the stage and message, keeps the track in Incoming and keeps its previous cues. Stale attempts cannot touch a job that was re-used, because every update is checked against the job's attempt counter. A provider or network failure is never treated as "no match".
