# Original synthetic timing references

These eight original input recipes were evaluated with Lexicon 1.11.15.0's
shipped worker in its Electron 32.3.3 / Node 20.18.1 runtime. Only numeric
outputs and independently authored input recipes are included. No application
code, executable, music, library data or credentials are included.

`reference.json` records two boundaries for each exact input:

- `expected.tempo`: the unchanged module 745 tempo helper, including its
  confidence and sample-index peaks. A private export exposed this helper.
- `expected.worker`: the original worker's complete timing request, returning
  tempo and first beat after all anchor corrections.

The worker received raw first-channel Float32 PCM, with the renderer's default
98–195 BPM bounds. A separate worker context was created for each case. The
application entry point, library, license, authentication and network APIs were
not used. Confidence is an internal heuristic, not calibrated musical accuracy.

The PCM uses dyadic amplitudes and integer square-wave bursts so it can be
regenerated without a platform-dependent sine/exp implementation. Each recipe
includes its full little-endian Float32 SHA-256. Verify every recipe with:

```sh
python3 tests/fixtures/lexicon-timing/generate.py
```

Use `--output /path/to/new/private/directory` only when actual PCM files are
needed for a research runner. Python is a fixture utility, not a product runtime.

The Rust tempo-helper tests now pass all eight BPM/peak references and
confidence within 1e-12. The final worker-anchor references remain porting
references, not passing Rust tests or proof of musical quality.
In particular, a 94 BPM input becomes 188 BPM under the configured range. The
detected anchor is not necessarily the exact start of the authored burst.
Further coverage is needed for other ranges, sample rates, stereo channel
policy, long intros, irregular rhythm, tempo changes and complete music files.

See `docs/research/lexicon-beatgrid.md` for the source trace, original silent
input timeout and the boundary between this research and desktop integration.

`onset-reference.json` adds 306 original numeric rows for the six module 974
attack/onset/snapping helpers, using those eight exact inputs plus one original
1.2-second silent-intro recipe. The Rust onset-helper unit tests now match all 306 rows within 1e-12. The helper inputs include
explicit time/peak/period arguments; they are component tests, not full detector
outputs. No real music is included.

`dp-reference.json` records eight original spectral-flux envelopes (88,168
Float32 values), beat chains, eight applied phase corrections and sixteen
unchanged/early-anchor decisions from module 809. The independent Rust tests
compare every envelope value bit-for-bit and check the chain/correction outputs.
This is component evidence on the current host; cross-platform and complete
anchor-tree validation remain outstanding. The other module 522 calibration,
riser and grid corrections are not covered by these DP results.

`phase-reference.json` records 585 module 522 functional result rows and
per-call correction tracking across nine original synthetic inputs. Debug-only
output fields are omitted. These are **pending** Rust parity references, not
passing phase-helper tests. They cover applied subsequent-beat, silent rescue,
weak-phase and grid-correlation cases. The separate authored-candidate references below add positive calibration,
drop verification and riser-A cases; half-beat and riser-B coverage is still
outstanding.
The long silent-intro recipe is distinct from the shorter onset fixture recipe.

`phase-authored-reference.json` adds 180 component rows for the calibration,
drop-verification and riser helpers. Their candidate peak times are explicitly
supplied authored transient starts, not outputs from a detector. Input PCM
hashes are unchanged. These rows test the helpers' explicit-input contracts;
they do not establish that a complete analysis will supply those candidates.
Rust phase-helper parity remains pending.
