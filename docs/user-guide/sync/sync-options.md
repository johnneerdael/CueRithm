# Set the sync options

<p class="lead">The controls under the destination change how colours, keys, grids and text fields reach your DJ software. This page lists every option, its default and what it does, so you can set a sync up in a few minutes.</p>

!!! tip "New to this?"
    Leave the defaults on your first sync. [Sync to rekordbox 6 or 7](rekordbox-direct.md) walks through a complete one.

Changing any option throws away the current preview; you preview again before applying ([Preview, apply and restore a sync](preview-apply-restore.md#preview)). CueRithm remembers the options you set for each destination except rekordbox, whose options go back to the defaults each time you open Sync ([Save a destination for next time](saved-destinations.md#remembered-choices)).

## Choose what rekordbox receives { #rekordbox-options }

Under **Include in sync** when **Sync to** is **rekordbox 6 / 7**:

| Control | Default | What it does |
|---|---|---|
| **Cues and loops** | on | Sends the cues and loops of the track's current cue set, including manual corrections. Cues become hot cues or memory cues as your cue layout says; names and colours are kept; the active loop stays active |
| **Beatgrids** | on | Sends one constant BPM and a first-beat anchor. A variable-tempo grid already in rekordbox can be kept with **Keep rekordbox**; a complete grid cannot be cleared |
| **Artwork** | on | Sends the cover as three JPEG sizes with the aspect ratio kept. A track that already has the same cover is not rewritten |
| **Playlists and folders** | on | Sends the playlists and folders in scope, with their order. Only lists CueRithm created can later be deleted by a sync |
| **My Tags** | on | Sends the categories and custom tags you made with Curate ([Group tracks with categories and custom tags](../tagging/curate-tracks.md)) as My Tag categories and My Tags |
| **Convert smart playlists to track lists** | off | Sends each smart playlist as an ordinary list of its current tracks. Needed whenever a smart playlist is in scope ([Choose what to sync](choose-what-to-sync.md#smart-playlists)) |
| **Create compatible WAV copies for unsupported audio** | off | Creates separate 16-bit stereo WAV copies for files rekordbox cannot play ([details](#audio-copies)) |
| **Use nearest supported cue colors** | off | Lets a cue colour that rekordbox cannot show use the closest one it can. With it off, such a colour stops the preview so you can decide. The preview counts the cues that change |
| **Metadata fields** | all ticked | Which text and number fields are sent ([below](#metadata-fields)) |

### Choose which metadata fields to send { #metadata-fields }

Expand **Metadata fields** to untick fields you want rekordbox to keep. The 18 checkboxes are **Title**, **Artist**, **Album**, **Album artist**, **Genre**, **Label**, **Comment**, **Remixer**, **Composer**, **Lyricist**, **Mix name**, **Key**, **Bpm**, **Rating**, **Release year**, **Release date**, **Track number** and **Disc number**.

- **Title** must stay ticked while there are tracks rekordbox does not have yet. Without it the preview stops: "New rekordbox tracks need a title. Include title in Metadata fields without skipping it, then preview again."
- A value that is empty in CueRithm never clears the field in rekordbox. Clearing is a separate, explicit choice in the review ([Resolve rekordbox sync conflicts](rekordbox-conflicts.md#choices)).
- CueRithm's energy rating is not a field and is not sent.

## Options for the other destinations { #other-options }

A dash means the control does not appear for that destination.

| Control | VirtualDJ | Engine DJ | djay Pro | Serato DJ Pro 4 | Pioneer USB | Mixxx |
|---|---|---|---|---|---|---|
| **All smartlists to playlists** | off | off | off | off | off | not shown |
| **Use nearest supported colors** | on | on | on | on | on | not shown |
| **Keep existing beatgrids** | off | off | off | off | not shown | **Do not send beatgrids**, off |
| **Beatgrid locks** | not shown | **Default** | not shown | **Default** | not shown | **Default** |
| **Convert keys** | not shown | not shown | not shown | **Original keys** | **Original keys** | not shown |
| **Add album art** | not shown | off | not shown | not shown | off | not shown |
| **Copy audio to the target drive** | not shown | off | not shown | not shown | not shown | not shown |
| **Cue destination** | not shown | not shown | not shown | not shown | **Hot cues** | not shown |
| **CDJ compatibility** | not shown | not shown | not shown | not shown | **No detection** | not shown |
| **Clean up unused files** | not shown | not shown | not shown | not shown | off | not shown |
| **Extra cue markers** | not shown | not shown | not shown | not shown | off | not shown |
| **Field mappings** | shown | shown | shown | shown | shown | shown |

Mixxx adds its own controls: **Mixxx library**, **Existing Mixxx tracks**, **Replace track details**, **Replace keys**, **Replace hot cues and loops** and **Playlists become**. They are described in [Sync to Mixxx](mixxx.md#choose-a-mode). The Pioneer USB and Engine DJ controls are described in [Prepare a Pioneer USB stick](pioneer-usb.md#options) and [Sync to Engine DJ](engine-dj.md#options).

## Map one field to another { #field-mappings }

Open **Field mappings** to choose which CueRithm value fills each destination field. The seven fields are title, artist, album, genre, label, comment and key; each has a menu that lists the same seven. By default every field fills itself.

Mappings are applied all at once to the original track, so you can swap two fields without one overwriting the other. A common use is to send the record label where the destination has no label field. Mixxx has no label column, so map **label** to **comment** to keep it:

```text
comment ← label
```

In the menu, set the **comment** row to `label`.

## Convert keys { #key-notation }

**Convert keys** writes the key in the notation you choose. It appears for Serato DJ Pro 4 and Pioneer USB. rekordbox and Mixxx store a key as a number and show it in whichever notation you set in that program, so they ignore this option.

| Choice | Track in 6A | Track in 12B |
|---|---|---|
| **Original keys** | the key as CueRithm holds it | the key as CueRithm holds it |
| **Camelot** | 6A | 12B |
| **Open Key** | 11m | 5d |
| **Musical notation** | Gm | E |

## Match cue colours to the destination { #colours }

Each destination offers a fixed set of cue colours. **Use nearest supported colors** (on by default for every destination that shows it) moves each cue colour to the closest one the destination has. With it off, Engine DJ keeps the exact colour value, and Pioneer USB drops the colour of a cue that is not exactly one of its eight. In rekordbox the option is called **Use nearest supported cue colors**, it is off by default, and an unsupported colour stops the preview, as the table above says. Mixxx shows any colour, so it has no such option.

## Send beatgrids and lock them { #beatgrids }

CueRithm sends one BPM and a first-beat anchor per track. **Keep existing beatgrids** (VirtualDJ, Engine DJ, djay Pro, Serato) leaves the destination's own grids alone; Mixxx calls it **Do not send beatgrids**; Pioneer USB has no such option because a stick needs a grid. [Fix a wrong beatgrid](../analysis/corrections/fix-the-beatgrid.md) explains the grid CueRithm holds.

**Beatgrid locks** decide whether the destination may re-analyse a grid you trust:

| Choice | Engine DJ | Serato DJ Pro 4 | Mixxx |
|---|---|---|---|
| **Default** | Keeps the track's current lock; a new track with a CueRithm grid is locked | Locks every track that has a grid | Leaves the lock alone; a new track starts unlocked |
| **Lock** | Locks | Locks | Locks (only when a grid is sent) |
| **Unlock** | Unlocks | Unlocks | Unlocks (only when a grid is sent) |

## Decide when audio is copied { #audio-copies }

Most destinations point at your music where it already is. Three can create copies:

- **rekordbox** plays MP3, AAC-LC in M4A or MP4, Apple Lossless in M4A, FLAC, and 16- or 24-bit PCM WAV and AIFF, mono or stereo, at the sample rates it supports. Anything else, such as Ogg Vorbis, Opus, WMA, raw AAC files or unusual sample rates, stops the preview unless **Create compatible WAV copies for unsupported audio** is ticked. The copies are 16-bit stereo WAV files kept in the rekordbox library, they leave your source file alone, and the preview reports how many and how much space they take. See [Check that your files are supported](../files/supported-formats.md).
- **Engine DJ** copies audio when **Copy audio to the target drive** is on, and automatically for libraries on a recognised drive.
- **Pioneer USB** always copies audio onto the stick, and converts incompatible files when you choose **Convert incompatible audio**.

## What can go wrong { #what-can-go-wrong }

| What you see | Why | What to do |
|---|---|---|
| "This codec/container layout cannot be played by native rekordbox; enable compatible audio copies to create a separate PCM WAV" | A file in scope is a format rekordbox cannot play | Tick **Create compatible WAV copies for unsupported audio**, or leave the track out |
| The preview stops on a cue colour | **Use nearest supported cue colors** is off and a colour has no match | Tick it, or change the cue's colour ([Move or add a cue](../analysis/corrections/move-or-add-cues.md)) |
| Keys look different in the DJ program | The program shows the key in its own notation, or **Convert keys** is on | Change the program's key display, or the **Convert keys** choice |

## Related

- [Choose what to sync](choose-what-to-sync.md)
- [Resolve rekordbox sync conflicts](rekordbox-conflicts.md)
- [Prepare a Pioneer USB stick](pioneer-usb.md)
- [Understand Camelot keys](../analysis/camelot-keys.md)

<!-- verified: apps/desktop/src/views/Sync.tsx (option visibility per destination); views/RekordboxSync.tsx; rekordbox-sync-types.ts (defaultRekordboxOptions, rekordboxMetadataFields); sync-types.ts (defaultSyncOptions); crates/cuerithm-dj-sync/src/formatting.rs (format_key, simultaneous mappings); engine_dj.rs (lock default, colour); serato4/media.rs (BPMLOCK); mixxx/tracks.rs (bpm_lock); pioneer_usb.rs (color_id, copies); native_rekordbox/source.rs (supported formats); rekordbox_sync_service/assets.rs (cue colour projection, WAV copies); planning.rs (title warning) -->
<!-- UNVERIFIED: that the Mixxx, Engine DJ and Serato lock behaviour table reflects what each program then does with the lock; only the value CueRithm writes was verified -->
