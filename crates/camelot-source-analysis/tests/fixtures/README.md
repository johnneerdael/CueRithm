# Synthetic audio fixtures

These contain independently generated samples, with no music or reference labels.

- `stereo16-44100.flac`: 64 frames repeating four signed 16-bit stereo pairs, for
  exact normalization and decoder-integrity checks.
- `stereo16-48000.flac`: unsupported-rate negative control.
- `pulses64-stereo16.flac`: 64 seconds of seeded, decaying noise bursts every
  22,050 frames, identical stereo channels, 44,100 Hz and 16-bit samples.
  Generator: `generate_pulses.py`. PCM SHA-256:
  `02c4ff3ee8b4e7f6ddd3db4fae0239ca764bd89972ad88ddabb8c75dea605565`.

Fixture regeneration uses Python and the `flac` preparation tool. Engine builds,
tests and runtime consume the committed FLAC files directly and require neither.
