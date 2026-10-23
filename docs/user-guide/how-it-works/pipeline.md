# How does CueRithm analyse a track?

<p class="lead">Every track takes the same path from file to ready. Knowing the path tells you what a stage label in Incoming means, what CueRithm checks before it saves anything, and what happens when a stage fails.</p>

!!! abstract "What this means for you"
    - **A half-copied file is never read.** CueRithm waits until the file stops changing, then hashes it.
    - **Nothing is saved until every required result exists.** A track needs a key, a BPM with a grid, an energy rating and at least one cue before it can leave Incoming. If a stage fails, the track stays in Incoming with its error and keeps its previous cues.
    - **With tagging and cleanup off, your audio file is never written.** Analysis results live in the library, not in the file.
    - To follow a track on screen, see [Follow a track through Incoming](../files/follow-incoming.md).

```text
file → wait → hash → decode → analyse → [match] → [tag and clean up] → check → save → Tracks
```

Steps in brackets run only when you turned automatic tagging or cleanup on.

## 1. Waiting for the file { #1-waiting-for-the-file }

A folder watcher and a rescan every five seconds feed discovery. Only files with a supported extension are considered (`mp3`, `aac`, `m4a`, `alac`, `flac`, `wav`, `wave`, `aif`, `aiff`, `aifc`, `ogg`, `oga`, `opus`, `wma`), and names that start with a dot are ignored. See [Check that your files are supported](../files/supported-formats.md).

A file must be stable: its size and modified time must be unchanged across two checks, two seconds apart by default. The interval is validated between 10 ms and 60 s and is not exposed in the interface. A file already older than twice the interval starts at once. A file that is still changing after 300 seconds fails with *File copy did not settle*. Once stable, the file is hashed with SHA-256. If the hash changes before work starts, the job is superseded and queued again with the new hash.

## 2. Decoding { #2-decoding }

The bundled decoder copies the file to a private snapshot, hashes it before and after the copy, and decodes the first audio stream to 32-bit float PCM at 44,100 Hz stereo. Any decode error is fatal for that track. Details are in [How are audio files decoded?](decoding.md).

## 3. Analysis { #3-analysis }

The engine reads that PCM and produces four results.

| Result | Input | Method |
|---|---|---|
| **Key** | Whole track, mono, 44.1 kHz | [Constant-Q spectrum plus a neural network](key.md) |
| **BPM and first beat** | Left channel, 44.1 kHz | [Local signal processing with explicit correction rules](tempo.md) |
| **Energy** | Whole track, mono, 22.05 kHz | [A fixed acoustic formula](energy.md) |
| **Cues** | Left channel, 44.1 kHz, plus the BPM and first beat | [Bar-by-bar energy analysis](cues.md) |

Workers run in parallel, using about three quarters of your logical cores (rounded down, at least one). The same source, settings, engine and decoder reuse a cached result, and the cache key includes the application executable's hash, so a new build never reuses an old build's results. [Analyze a track again](../analysis/analyze-again.md) clears the cache for that track.

Key, BPM and energy are predicted from the audio and explicit settings only, never from tags, filenames or reference labels. The cue detector also reads the genre tag: only a genre containing "techno" or "hardcore" changes its behaviour.

## 4. Matching, tagging and cleanup (optional) { #4-matching-tagging-and-cleanup-optional }

With automatic tagging on, CueRithm [matches the track against the catalogs you chose](matching.md). Tags, and cleanup edits if cleanup is on, are applied to a staged copy of the file. The copy is decoded again and must contain identical audio. See [How are tags written safely?](tag-writing.md) and [How does cleanup decide what to change?](cleanup-engine.md).

If the final genre differs from the genre the cue detector used, the cues are recomputed with the measured timing reused. Key, BPM and energy do not change.

The rules for a job are fixed when it is queued. A folder with its own [preparation preset](../files/folder-presets.md) uses that preset's rules; changing Settings later does not alter a job already in flight.

## 5. Publication and promotion { #5-publication-and-promotion }

Before a track becomes ready, CueRithm validates the results:

- the analysis is complete and the duration is positive and finite;
- a key is resolved;
- the BPM is greater than zero and the beatgrid is non-empty and finite;
- the energy is between 1 and 10;
- there is at least one cue, finite and inside the track.

Then, in a single SQLite transaction, CueRithm stores the analysis, writes key, BPM, energy and duration, inserts a new cue revision and sets the track to *ready*. If tags were written, the staged file has already been swapped in atomically and journaled.

If automatic file organisation is on, the track stays in Incoming for one more stage, *Organizing files*, and leaves only when it finishes. See [Rename and sort files into folders](../rename/organize-files.md).

!!! note "No tags, no write"
    If neither tagging nor cleanup is enabled, the audio file is not written. The track is analysed, its cues are refreshed using whatever genre tag the file already has, and it is promoted.

## What is stored where { #what-is-stored-where }

| Item | Library | Audio file tags |
|---|---|---|
| Key | Always, as a Camelot code | Only if the Key field is selected |
| BPM | Always (the saved grid value) | If the BPM field is selected (on by default) |
| Energy 1 to 10 | Always | `ENERGY` custom field, on by default |
| Beatgrid | Yes | Never |
| Cues | Yes, as append-only revisions | Never |
| Waveform | Cache | Never |

## Where you see each stage in the app { #where-you-see-it }

The stage appears under the title of a row in **Incoming**. It is the label from the app, not a status you configure.

<!-- figure:existing-21-incoming-analyzing-how-it-works-pipeline -->
<figure markdown>
![Incoming with tracks in the Decoding, Analyzing and Queued states](../assets/screens/21-incoming-analyzing.webp){ .screen loading=lazy }
<figcaption>Each stage of the pipeline is visible on the Incoming row.</figcaption>
</figure>
<!-- /figure -->

| Stage | Label in Incoming | Runs when |
|---|---|---|
| Queued, setup | Queued, Setup | Always |
| Waiting | Waiting for file | Always |
| Decoding | Decoding | Always |
| Analysis | Analyzing | Always, unless a cached result is reused |
| Matching | Matching catalog | Automatic tagging is on |
| Tagging | Preparing tags | Automatic tagging is on |
| Cleanup | `cleanup` (the app has no friendlier label) | Cleanup is on |
| Writing | Writing tags | Tagging or cleanup is on |
| Committing | Applying cues | Tagging and cleanup are both off |
| Organising | Organizing files | File organisation is on |

## What happens when a stage fails { #failure-semantics }

A failed required stage sets the job to *attention* with the stage and message, keeps the track in Incoming and keeps its previous cues. Every update is checked against the job's attempt counter, so a stale attempt cannot touch a job that was reused. A provider or network failure is never treated as "no match".

Saving **Settings** retries every track that needs attention, using the new settings. To fix one yourself, follow [Fix a track that needs attention](../files/needs-attention.md) or [A track is stuck in Incoming](../help/troubleshooting/incoming-stuck.md).

## Related

- [How are audio files decoded?](decoding.md)
- [How are tags written safely?](tag-writing.md)
- [How is the library stored?](library.md)
- [Follow a track through Incoming](../files/follow-incoming.md)

<!-- verified: crates/cuerithm-library/src/watch.rs (supported_path, wait_for_stable 300 s, 2-check stability), settings.rs (stability_millis 2000, 10..=60000), prepare.rs (5 s rescan, stages, worker_count 75%, no-tag branch, organizing), jobs.rs (validate_analysis, store_analysis_tx, fail_for_run, retry), analysis_cache.rs (cache key incl. executable hash), camelot-core engine.rs (inputs, refresh_prepared_cues genre), apps/desktop/src/components/TrackTable.tsx (stage labels; no label for cleanup), src-tauri commands.rs save_settings (retry_attention), crates/cuerithm-autotag write.rs (ENERGY, key, BPM fields); legacy technical/pipeline.md re-checked -->
