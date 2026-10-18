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
1.2-second silent-intro recipe. The Rust onset-helper unit tests now match all
306 rows within 1e-12. The helper inputs include
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
output fields are omitted. `phase-authored-reference.json` adds 180 rows with
explicit authored transient-start candidates. Those candidate times are
component inputs, not outputs from a detector; they do not establish that a
complete analysis supplies those candidates.

`phase-edge-reference.json` adds 25 rows on three original recipes: intermittent
offbeat hats trigger half-beat correction; a soft/quiet pickup triggers riser B
and grid advancement; a short input triggers onset refinement without changing
subsequent-correction tracking. The short helper fixture does not establish that
short audio is suitable for complete track analysis. The longer silent-intro
recipe in the phase fixtures differs from the onset fixture's shorter intro.

The Rust tests match all 790 phase result/state rows within 1e-12 on the current
host and require positive cases for each changing helper, including riser A/B.
The near-zero helper is intentionally unchanged in the source. Separate tests
check per-analysis state isolation and retention across no-op calls. Complete
anchor-tree and desktop parity are still outstanding.

Verify the edge inputs independently with:

```sh
python3 tests/fixtures/lexicon-timing/generate.py --reference tests/fixtures/lexicon-timing/phase-edge-reference.json
```

`anchor-primitives-reference.json` adds 216 passing Rust intro/drop/DnB/
immediate-start reference rows across 15 original inputs. It includes all four
intro classifications, empty/short input behavior, explicit candidate lists,
optional DnB anchor preference and a 48000 Hz case. Three piecewise-level inputs
exercise intro classification only; they are not evidence of musical rhythm or
complete track timing. Each input's exact PCM hash is checked.

`walkback-reference.json` contains 1,224 passing Rust first-beat walkback numeric
references on those inputs plus explicit no-candidate and late-bed/half-step
controls. It varies detected/authored candidates, drop
positions, optional attack strengths and intro kind. Optional null strength in
the fixture was passed as JavaScript `undefined` to the original helper. The
returned walkback count includes tested half beats. The input hashes are checked
in Rust and independently regenerated by the Python fixture utility. This proves
the measured helper boundary, not the complete detector or musical accuracy. Debug reason text and application code are not included.

`complete-stage-reference.json` records 15 complete original-worker outputs with
observational snapshots immediately before final-anchor correction, plus eight
explicit candidate/state component inputs. The eight original worker baselines
remain bitwise unchanged by those taps. The complete cases include a 22-second
quiet intro, wider 50–300 BPM bounds with 220/274 BPM inputs, and 8000/48000 Hz.
The component-only inputs bypass initial selection in the private reference copy
and test nudge veto/allowance, half-beat and riser-B sequencing. They are marked
`component_only` and are not complete audio-to-timing predictions.

Rust final-correction tests match all 23 given-candidate/state outputs on this
host. The initial module 358 selector is still **unimplemented**, so this is
not full Rust audio-to-anchor parity or desktop integration. Source correction
bookkeeping is an explicit input; output taps and private source copies are not
distributed. Only original recipes, numeric outputs and hashes are published.
