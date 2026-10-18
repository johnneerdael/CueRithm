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
