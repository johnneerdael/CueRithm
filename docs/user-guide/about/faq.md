# FAQ and troubleshooting

## General

**Does CueRithm change my audio files?**
Only if you turn on automatic tagging or cleanup, and then only the tags, through a staged copy that is proven to contain identical audio. With both off, CueRithm never writes to your files. See [Safe tag writing](../technical/tag-writing.md).

**Does it move or copy my music?**
No. Incoming and Tracks are states inside the library. Files stay where they are.

**Does it need an internet connection?**
Not for analysis, cues, playlists or cleanup. Only catalog tagging, which is off by default, contacts Beatport and Traxsource.

**Is my music uploaded anywhere?**
No. Analysis runs on your computer. Tagging sends a title and artist search to the catalog, not audio.

**Why is there no Intel Mac build?**
Intel Macs are not a target for this project. Apple Silicon only.

**Can I sync to rekordbox?**
Not yet. See [Not available yet](roadmap.md).

## Preparation

**A track is stuck in Incoming. Why?**
Open Incoming and read the row. The second line shows the stage or error. Common causes: the file is longer than about 24 minutes; it is protected or not a supported audio format; the file is still being copied; you are offline with tagging on; or a provider returned an error. Fix the cause, then press the retry icon, or save Settings to re-queue everything that needs attention.

**A file I dropped never appears.**
Individually dropped files are not filtered by extension, so an unsupported file fails with an error in Incoming. For folders and watch folders only `mp3`, `aac`, `m4a`, `alac`, `flac`, `wav`, `wave`, `aif`, `aiff`, `aifc`, `ogg`, `oga`, `opus` and `wma` are picked up, and hidden files are ignored.

**Why did my track not get a Breakdown cue?**
Breakdowns need a long enough section after the drop (64 beats by default), not in the first 25% or the last 30 seconds. See [The cue engine](../technical/cues.md). You can lower the minimum in Settings, but already prepared tracks change only when you choose **Analyze again**.

**The BPM looks doubled or halved.**
The detector searches 98 to 195 BPM and folds tracks into that range, so a 94 BPM track shows as 188. There is no range setting yet.

**The beat lines drift on a live-drummer track.**
The grid is one constant tempo. See [Tempo and beatgrid](../technical/tempo.md).

**I changed a setting and old tracks did not change.**
By design. Analyze again to apply new settings to a finished track.

## Tagging

**Why is tagging off?**
It needs the network and rewrites tags, so it is opt-in.

**Tracks are stuck at "Matching catalog".**
You may be offline, or the provider may be blocking requests. They stay in Incoming. Check your connection and retry. Traxsource in particular has not been confirmed to work from every network.

**Where are my original files after a tag write?**
Kept in the data folder under `tagging/`, shown as *Recovery backups* in Settings.

## Cleanup and playlists

**Can I undo a cleanup?**
Yes, from the Undo history in Cleanup. A track edited since is kept as you left it and reported as a conflict.

**Why can't I add a track to a smart playlist?**
Smart playlists are rule-driven. Use an ordinary playlist for hand-picked tracks, or take a snapshot with the Playlist tools.

**My smart playlist is empty or says it is stale.**
Preview its rules again in the editor. If an evaluation fails, the playlist keeps its last good result and shows an error.

## Technical

**The app says the library is already open.**
Only one CueRithm process can open a library at a time. Close the other one.

**The app will not start and mentions a decoder hash.**
The bundled decoder does not match its sealed hash, which means the installation is damaged. Reinstall it, and run `--diagnostics` (see [Install](../get-started/install.md#check-that-the-install-is-healthy)).

**Where is my data?**
See [Install](../get-started/install.md#where-cuerithm-keeps-its-data).

**Where do I report a problem?**
[github.com/johnneerdael/CueRithm/issues](https://github.com/johnneerdael/CueRithm/issues). Please do not attach copyrighted audio.
