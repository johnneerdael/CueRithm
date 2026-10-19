# Cleanup engine

The cleanup engine is a **pure function**: it takes a track's metadata and the saved rules and returns the proposed new metadata. It performs no input or output. The library service around it handles scanning, previews, journaling, backups and undo.

## Design rules

- **Deterministic and idempotent.** Running the same rules twice changes nothing the second time.
- **Fixed algorithms, not user regexes.** Each of the [eleven Smart Fixes](../guides/cleanup-rules.md#smart-fixes) is a purpose-built routine. The only user-facing regular expressions in CueRithm are in smart playlists and the optional tagging "title cleanup pattern".
- **Never blank a value.** A fix that would empty a field leaves it alone.
- **Protect declared values.** Ignored values and active genre or artist mapping sources and targets are skipped by text fixes in their roles.
- **Human fields only.** Title, artist, album, album artist, genre, comment, grouping, label, remixer, mix, producer, composer and lyricist. Never paths, audio or analysis.

## Order

```text
enabled Smart Fixes (in list order) ─► genre conversion and mapping ─► artist mapping
```

Genre mapping follows chains to the end, rejects cycles and collapses duplicates. A mapping to nothing deletes the value. Artist mapping covers only the roles you tick.

## Scan, apply, undo

1. **Scan** reads the files in scope and runs the engine. It writes nothing. The preview lists proposals, and counts of unchanged and unavailable tracks.
2. **Apply** stages each changed file and writes the new tags there, re-reads them and checks they equal what was intended, then publishes with the same atomic, journaled swap as [tag writing](tag-writing.md), together with the library metadata. Each track is independent: a conflict or failure on one does not stop the others.
3. **Undo** restores the original bytes and metadata when the file still matches what the apply produced. If it does not, your newer edit is kept and the track is reported as a conflict.

Unrepresentable metadata (for example a value a tag format cannot hold) is reported per track as a failure, not truncated.

## Custom tags

Genre conversion can turn genres into **categorised tags** (default category *Subgenres*). Those live in the CueRithm library, not in file tags.

## Validation

The same limits are enforced in the interface and the backend: at most 64 fixes, 4,096 mappings, 4,096 ignored and 4,096 protected values, values of at most 4,096 bytes, and per-track limits of 128 values per field and 1 MiB total. Errors appear in a banner before you can save.

## Automatic mode

With *Clean incoming tracks automatically*, the engine's output is added to the same staged write plan as catalog tags, after tagging and before promotion. Cleanup never contacts a provider.
