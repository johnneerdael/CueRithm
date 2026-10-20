# Incoming and Archive

Incoming and Archive are track states in CueRithm's library. They do not correspond to folders or move your audio files.

## Incoming

A track stays *incoming* from the moment it is added until preparation succeeds. The subtitle reads: *Automatic preparation. Ready tracks move into your library.*

<figure markdown>
  ![Incoming view showing stages](../assets/screens/21-incoming-analyzing.webp){ .screen loading=lazy }
</figure>

### Stages

| Stage label | Meaning |
| --- | --- |
| Queued | Waiting for a free worker. |
| Waiting for file | The file is still changing. |
| Decoding | The bundled decoder is producing the working copy. |
| Analyzing | Key, tempo, energy and cues. |
| Matching catalog | Looking up Beatport or Traxsource (tagging on). |
| Preparing tags | Building the tag plan (tagging or cleanup on). |
| Writing tags | Staging and publishing tags. |
| Applying cues | Refreshing cues, for example after a genre change. |

### When a track needs attention

A failed stage sets the track to *Needs attention*. It stays in Incoming with the stage and error. The usual causes are an unsupported or protected file, a file longer than about 24 minutes, a disk or permission problem, being offline while tagging is on, or a provider error.

Press the **retry** icon on the row to try again. Saving **Settings** also re-queues every track that needs attention, using the new settings.

### What "ready" requires

A track becomes ready only when it has a key, a positive BPM with a non-empty beatgrid, an energy rating from 1 to 10 and at least one cue, with all positions inside the track. Otherwise it stays in Incoming.

### Empty Incoming

*Nothing incoming. Drop files here, or add music to a watch folder. Preparation runs automatically.*

## Archive

**Archive** (selection toolbar or row menu) takes a track out of Tracks and out of ordinary playlist views without deleting anything. The subtitle reads *Archived tracks keep their analysis and cues.* Archived tracks stay in the library database, in their playlists' membership and on disk.

<figure markdown>
  ![The Archive view with one track](../assets/screens/80-archive.webp){ .screen loading=lazy }
</figure>

Select a track and press **Restore** to bring it back. Restoring re-queues it for preparation against your current settings.

Smart playlists include archived tracks only if a rule asks for them with *Archived is true*, and include incoming tracks only with *Incoming is true*.
