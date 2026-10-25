# Acceptance fixtures

`click128.apple.m4a` is not music. It is the harness's synthetic click track (a 2 kHz tone burst
on every beat of a 128 BPM grid, the first at 0.5 s, silence in between, 24 s long) encoded to
AAC by Apple's `afconvert` (`-f m4af -d aac -b 192000`), which writes 2112 samples of priming in
an `iTunSMPB` tag and no edit list.

The harness makes this file itself on macOS. Windows has no `afconvert`, so `run.py --windows`
uses this copy to test how Mixxx's decoder there treats that encoder's output.
