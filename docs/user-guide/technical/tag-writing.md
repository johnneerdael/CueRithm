# Safe tag writing

Writing tags is the only time CueRithm changes your audio files. The process is built so that a crash, a power loss or a competing edit cannot leave you with a damaged or silently changed file.

## Which tags go where

The container is detected from the file's actual codec, not its extension.

| Container | Tag system | Notes |
| --- | --- | --- |
| MP3 | ID3v2.4 (default) or v2.3 | Written with the `id3` library |
| AAC (ADTS) | ID3v2 | Prepended to the stream |
| AAC and ALAC in M4A | MP4 / iTunes atoms | Standard atoms plus freeform `com.apple.iTunes:*` fields |
| FLAC | Vorbis comments | |
| Ogg Vorbis, Opus | Vorbis comments | |
| WAV | ID3 chunk, mirrored into `LIST/INFO` | Other chunks preserved |
| AIFF | ID3 chunk | |
| WMA | ASF attributes | Via a statically linked TagLib 2.3.2, used for WMA only |

For measured data: **key** uses the standard key field of each format, BPM uses the BPM field (MP4 gets both the integer `tmpo` atom and a full-precision `BPM` freeform), **energy** is a custom `ENERGY` field, and an optional `CUERITHM_TAGGEDDATE` marker records when a file was tagged. **Cues and beatgrids are never written to tags.**

### ID3 versions

If the setting says v2.4 and a file has v2.3, it is upgraded when a write is needed, preserving literal slashes in names such as "AC/DC" and keeping frame flags and encodings. v2.3 writing uses the older date frames and Latin-1 picture encoding for DJ-software compatibility. The tag is rewritten only when something actually changes.

## The seven safeguards

1. **Snapshot.** The source is copied privately, with SHA-256 before and after. If it changes during the copy the write aborts (*Source changed during capture*).
2. **Two copies.** The snapshot is copied into the data folder as `original.<ext>` (your recovery backup) and `staged.<ext>`.
3. **Edit the staged copy only.** An unreadable existing tag causes a refusal, never a silent discard.
4. **Prove the audio is unchanged.** The staged file is decoded again and must have the **same frame count and identical PCM hash**. If not, the write is refused with *Tagging changed playable audio or timeline*. For cleanup the tags read back must equal exactly what was intended.
5. **Recheck the source.** The original file is hashed again to ensure you did not change it meanwhile.
6. **Journal, flush, swap.** A journal row records the intent. Files and their parent directories are flushed to disk. The staged file replaces the original with an **atomic exchange** (macOS `renamex_np` swap, Linux `renameat2` exchange, Windows `ReplaceFileW`). The displaced original is hash-checked.
7. **Commit together.** Only then does the database transaction store the analysis and cues and mark the track ready.

Targets must be regular, writable files in the original folder, not symbolic links.

## Rollback and crash recovery

- Any failure after the swap rolls the files back and marks the journal *rolled_back* (or *conflict* if rollback could not complete).
- **A later user edit is never overwritten.** If the file's hash is neither the original nor CueRithm's, rollback stops with *Later user edit preserved; original backup retained*.
- After a crash, a journal in *prepared* is rolled back at the next start and the job is queued again.
- A cover sidecar file and the audio are published and rolled back as one unit.

## Backups

The original bytes of every changed file stay under `tagging/<workspace>/original.<ext>` indefinitely. A write that changes nothing keeps no backup. Only manual Cleanup has a user-facing **Undo**. There is no restore button for Incoming tagging, and no retention manager yet.

## TagLib

TagLib 2.3.2 is built statically with all other formats, shared libraries and tests disabled, and a small C++ bridge exposes only the staged-file ASF operations. One checked build-time modification keeps repeated ASF values together so multi-artist order is preserved. Licences are retained in the vendored folder. You never install TagLib.
