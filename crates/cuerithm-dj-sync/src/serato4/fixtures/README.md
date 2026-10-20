These fixtures contain 50 milliseconds of generated mono silence, with the
title `Existing title` and comment `Foreign metadata`. They contain no music.

Generate each with FFmpeg (choose mp3, flac, m4a or aiff as the extension):
For AIFF, add `-write_id3v2 1` before the output filename.
For Ogg Vorbis, use stereo input and `-c:a vorbis -strict experimental`.

```sh
ffmpeg -f lavfi -i anullsrc=r=44100:cl=mono -t 0.05 -metadata 'title=Existing title' -metadata 'comment=Foreign metadata' silence.mp3
```
