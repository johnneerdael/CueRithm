# Get started

1. **[Install CueRithm](install.md).** Download the release for your system and open it.
2. **[Learn the window](main-window.md).** A sidebar, a track table and a persistent player.
3. **[Add your music](add-music.md).** Drop files in, or point CueRithm at a folder. CueRithm analyses each track and adds it to your library automatically.

## What you need

| | |
| --- | --- |
| **Systems** | macOS on Apple Silicon (signed and notarized), Windows x86-64 and Linux x86-64 (both unsigned). There is no Intel Mac build. |
| **Audio formats** | MP3, AAC (ADTS and M4A), ALAC (M4A), FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA. Protected media is not supported. |
| **Internet** | Not needed for analysis, cues, playlists or cleanup. Only the optional catalog tagging contacts Beatport and Traxsource. |
| **Extra software** | The audio decoder is bundled. You do not need FFmpeg, Python or a codec pack. |
| **Disk** | CueRithm keeps your music where it is. It stores a library database, caches and original-file backups in its own data folder. |

## Files and saved analysis

!!! tip "Tracks are not moved"
    "Incoming" and "Tracks" are states inside CueRithm's library, not folders on disk. Preparing a track never moves or copies your audio file. See [Incoming and Archive](../guides/incoming-archive.md).

!!! tip "Analysis happens once"
    A prepared track keeps its results until you choose **Analyze again**. Changing settings does not silently rewrite finished tracks.
