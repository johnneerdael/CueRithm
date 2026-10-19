# Original stream-metadata fixtures

Both files contain an original generated 0.5-second 440 Hz sine at 48 kHz.
Generate with FFmpeg 9.0.2, `-f lavfi -i sine=frequency=440:sample_rate=48000:duration=0.5`,
and metadata `title=Éclair`, `artist=Studio Artist`, `album=Original Test`,
`genre=House`. Encode Opus with `-c:a libopus`; encode Vorbis with
`-ac 2 -c:a vorbis -strict experimental`. No reference music is included.
The application runtime needs only their decoders, not these fixture encoders.

`covered.flac` contains a generated 0.5-second 440 Hz / 44100 Hz sine,
encoded as FLAC, with the original Cuerithm 128px PNG icon copied into an attached
front-cover stream (`-map 0:a -map 1:v -c:a flac -c:v copy -disposition:v attached_pic`).
The image is preserved byte-for-byte; no reference music or external artwork is included.

`authored-video.mp4` is an original one-second 16×16 black MPEG-4 video with
silent stereo AAC, generated with a full build of FFmpeg for a negative media
classification test:

```sh
ffmpeg -f lavfi -i color=c=black:s=16x16:r=1 -f lavfi -i anullsrc=r=44100:cl=stereo -t 1 -c:v mpeg4 -c:a aac authored-video.mp4
```

The test checks it under an incorrect audio suffix. The application only probes
its video stream; these fixture encoders/filters are not enabled or required in
the bundled application runtime.

`authored-multistream.mov` contains two generated 0.25-second signed-16 PCM
streams: a default mono 44.1 kHz 440 Hz sine and non-default stereo 48 kHz silence.
It verifies default-stream priority despite the other stream's larger channel
layout. The original reference binary's digest and the file hash are recorded
in `../fingerprint-reference.json`.
