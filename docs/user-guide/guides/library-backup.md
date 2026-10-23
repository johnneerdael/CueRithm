# Back up and restore your library

**Library data only. Music files are not included.** Copy your music separately.
Open **Library backup** under Tools for portable exports and library replacement.
Same-installation recovery snapshots remain in a separate expandable section.

## Export a portable library

1. Finish preparation and any metadata, organization or DJ sync operations.
2. Choose **Export .cuerithm** and a destination file.
3. Keep **Include musical revision history** enabled to retain older accepted
   cue and tempo revisions as well as the selected revision.

CueRithm writes a logical archive, reads it back and checks its record counts and
checksums before publishing it. Existing destination files are preserved; choose
another filename when one already exists. Offline music does not prevent export.
The archive contains source locations and library metadata. Store it privately.

## Restore or import a Lexicon backup

1. Choose **Inspect backup or Lexicon ZIP**. CueRithm detects the format from its
   contents. Lexicon import currently accepts the database-backup v26 profile.
2. For Lexicon timestamps without an offset, set the library's original timezone.
3. Review track, cue and playlist totals and all compatibility notes.
4. Map each source music folder to its folder on this computer, or leave it
   unresolved. An unresolved track keeps its metadata, cues and playlist entries.
5. Acknowledge excluded or unsupported data, then confirm the named active library
   will be replaced and retained for recovery.
6. Choose **Restore and restart CueRithm**. Finish the existing unsaved-draft
   prompt. Cancelling that prompt also cancels the scheduled replacement.

Restore builds and validates a separate library before restart. Startup activates
only the validated generation; the previous library stays retained. Restored
watch folders, automatic tagging and external DJ write targets are disabled.
Import does not prepare tracks, write music tags or change another DJ database.
An imported BPM, key or energy is saved source data, rather than a CueRithm DSP
measurement. Saved cues and tempo markers remain usable without analysis.

A changed, invalid or colliding destination file stays unresolved. Use **Relocate**
to review an explicit replacement. A different file can require checking saved cue
and beat timing. Copying music or selecting folders does not move the source files.

## Return to a retained library

Choose a library under **Retained libraries**, then **Use retained library and
restart**. This returns to its complete saved state; later edits in the current
library remain there and are not merged. Retained generations are not deleted
by an automatic retention policy.

## Same-installation recovery snapshots

These retain the older database-and-location-manifest format. Keep both files
in each dated backup folder together. They include preparation settings, presets
and local recovery history and have stricter file-consistency requirements than
portable archives. They cannot migrate a library to another installation.

### Create a recovery snapshot

1. Finish preparation, metadata/Curate writes, file organization and any cleanup, maintenance or DJ sync work. Resolve
   recovery warnings and discard unused sync previews.
2. Open **Library backup** and choose **Create backup**.
3. Choose a destination. CueRithm creates a dated folder and shows its location.

CueRithm checks source bytes and retained recovery files while creating the
snapshot. A changed source must be prepared again before making a fresh backup.
Missing source files stay listed as unavailable; their audio is not recovered.

### Restore a recovery snapshot

1. Choose **Inspect backup folder** and select a dated CueRithm backup folder.
2. Review its date, track count and unavailable sources.
3. Confirm replacement of the current preparation state, then choose
   **Restore on next launch**.
4. Close and reopen CueRithm. Cancel the scheduled restore before closing if
   needed.

Restore applies before the library opens. CueRithm checks compatibility and file
consistency again and keeps a pre-restore backup in the application's data folder
under `library-backups/pre-restore-<request-id>-<attempt-id>/`. The backup folder inside it can
be inspected and scheduled for restore in the same way. Cancelling a scheduled restore removes its owned staged copies. Completed
pre-restore backups remain available; this delivery does not manage their retention.

If startup checks block the restore, CueRithm opens your current library and keeps
that restore request visible with its reason. Choose **Cancel scheduled restore**
to keep working, or fix the reported file state and choose **Retry on next launch**.
A blocked request is never retried silently. An interruption during the database
update is handled separately; startup stops if its retained recovery snapshot
cannot be validated.

### Recovery snapshot limits

This delivery supports the same CueRithm version, operating system and library
installation. It does not move a library to another computer. The installation
identity file in the application data folder must remain available.

Music, owned sidecars, analysis models and retained tagging, cleanup, maintenance,
organization and DJ sync resources must match their saved state. Organization
originals, Copy-source ownership, partial stages and native reference recovery
files keep their exact file and folder identities. They are checked in place;
the backup does not copy their audio or restore external DJ databases. Later tagging, organization, removal, backup disposal
or DJ sync can make an older database snapshot ineligible for restore.

Restoring the database cannot reverse edits to music tags or changes in DJ
software. CueRithm refuses the restore when its file and recovery checks fail;
it does not silently drop recovery history or overwrite music to make it fit.

Backups currently support database files up to 1 GiB and up to 50,000 source
locations. Retained resource inventories also have file, depth and size limits.
CueRithm checks complete source and resource hashes; this can take time on a large
collection. A failed backup does not replace your library.
