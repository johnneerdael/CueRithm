# Key features from real audio

An independent Rust pipeline now reproduces the original key-feature frontend
on all five captured development tracks on macOS ARM64 and Linux ARM64. The
worker reads FLAC audio and computes PCM, FFTs, tracked peaks, tuning scores,
finalized tuning and cached feature frames. It loads no purchased SDK and uses
no original peaks, tuning, caches or reference labels as prediction inputs.

The first bank comparison supplied the archived refined grid and downbeat.
The latest composition replaces those with independently computed Main timing
and still reproduces every outgoing key-feature value on both platforms.
Duration and frame count are audio-derived. Only reference-run original legacy
marks/tempo remain supplied upstream; independent legacy detection, the full
request and final key/energy/cue results remain unfinished.

## Real-file comparison

| Observation | macOS ARM64 | Linux ARM64 |
| --- | --- | --- |
| Development audio files | 5/5 exact | 5/5 exact |
| Complete analyzed PCM stream digests | 5/5 match | 5/5 match |
| Float32 samples covered by those digests | 110,795,692 | 110,795,692 |
| Retained spectral frames | 27,044 exact | 27,044 exact |
| Retained peak pairs | 749,737 exact | 749,737 exact |
| Raw tuning scores | 432,704 exact | 432,704 exact |
| Finalized tuning values | 27,044 exact | 27,044 exact |
| Cached float32 feature values | 2,379,872 exact | 2,379,872 exact |
| Outgoing binary64 key-feature values | 1,760/1,760 exact | 1,760/1,760 exact |

The native decoder-only reference hashes all valid returned interleaved float32
bytes. The independent stream hashes match across all 443,182,768 analyzed PCM
bytes, including read-size sequences and first/last samples. Subsequent peak,
score, tuning and cache files are compared directly without tolerances. Native
expected data stay outside prediction workers. Linux runs without networking.

These are five development cases, not held-out or whole-library results. The
production key/BPM/cue accuracy benchmark has not been rerun or improved by this
experiment, and the components have not yet replaced production inference.

## Decoder behavior matters

The validated profile is source-rate 44,100 Hz, stereo, 16-bit FLAC. Symphonia
decodes and verifies the complete source. An unchanged, previously verified
packet-boundary component reproduces the original reader's analyzed prefix.
It receives decoded packet lengths, rather than track identities or supplied
reference lengths.

The original FLAC route converts into 4,096-frame output batches. Its EOF path
does not flush remaining converter audio. Depending on the final encoded packet
and pending samples, a partial tail can therefore be retained or dropped. Four
of these five files lose small tails; the fifth retains its partial tail. All
five analyzed lengths and float32 stream digests match. Simply using the full
FLAC duration would change the reference input.

The new decoder preserves interleaved channels. The key frontend then uses the
original channel **sum**, rather than the production decoder's mono average.
The existing production averaging/resampling path is not presented as an exact
input source for this profile.

## PCM scheduling and composition

A separate 40-case synthetic gate first validates channel folding, signed zeros,
512-frame feeding, dual FFT windows, final complete single windows and buffer
compaction. Both platforms match all 1,294 checkpoints and 343 final frames,
including 6,860 peak pairs, 5,488 scores, 343 tuning values and 30,184 cache values.
No incomplete FFT window is padded and no tracking tail is flushed.

Different feed schedules can leave different internal buffer states despite
producing the same final frame count. The original 512-frame schedule is retained
for real files. All 14 PCM/FFT tests and two new decoder tests pass on both
platforms. The frozen [spectral tracker](key-peak-reconstruction.md),
[tuning](key-tuning-reconstruction.md) and [cache](key-cache-reconstruction.md)
components are reused unchanged.

For the initial outgoing comparison, the bank worker receives new caches and
audio-derived frame count/duration alongside the two archived timing inputs.
That proof retains its original conditions. The newer bridge uses computed
Main grids/downbeats, independent caches and audio geometry only. All five
workers finish before the host reads the previously capture-validated numeric
feature references; no captured request envelope is needed. Every one of the
five 352-value arrays matches exactly on Mac and Linux. See
[timing reconstruction](beat-waveform-reconstruction.md) for the remaining
upstream legacy input and platform limits.

## Remaining work

Independent legacy beat detection is still needed to remove the remaining
supplied upstream timing. Other source rates, bit depths, channel
layouts, codecs, Windows execution and exceptional numerical inputs are not
covered by this real-file proof. General multisegment request construction and
the 12-value legacy pitch field also retain their documented limits.

Energy-request feature production and final key/energy/cue classification or
selection remain separate work. The outgoing cue fields now match with computed
Main timing under the same legacy-input condition. Matching this key-feature field
does not recover the service's weights or establish perfect full-engine results.

Private proofs are retained in `key-pcm-inputs-v1`, `native-key-pcm-stream-v1`,
`key-pcm-stream-rust-v1`, `key-real-pcm-source-map-v1`,
`native-real-pcm-digest-v1` and `key-real-stream-rust-v1` under
`.analysis/experiments/`. Original app, music and CSV were preserved; the
development analyses used existing private tuning copies.
The latest bridges are `key-with-computed-timing-v1` and
`key-with-computed-timing-linux-v1`; expected values stay outside their workers.
