# Tuning reconstruction from retained spectral peaks

This private component study moved the key-request reconstruction boundary
earlier. Its original three-stage proof supplied retained peaks and archived beat
context while removing supplied finalized tuning and cached88 values. Subsequent
[real-audio reconstruction](key-real-audio-reconstruction.md) also computes those
peaks independently on five development files on Mac/Linux; only the refined beat
grid and downbeat remain supplied. Final key classification remains unfinished,
and the production benchmark is unchanged.

## Source mechanisms

Retained MIKAnalysis instructions establish a16-bin tuning score vector for each
spectral frame. Peak frequencies are fractional FFT bins; the scalar is
float32(44100)/float32(4096). AddPitches0x1ead4 combines a float32 logarithm/floor
chain, double A-weighting and a double power denominator with exponent0.25.
Its wrapped Hanning contributions are squared and accumulated with separate
double multiply/add operations before narrowing each score to float32.

Slice0x1f41c uses signed floor and periodic interpolation in the source FMA
order. Region0x1f330 sums33 temporal positions and keeps out-of-range frames
zero without renormalizing edge weights. BestCurve0x1ef74 starts with a phase
scan, then performs eight ordered125-candidate searches. Search seeds remain
fixed during each search, winners update only on strict improvement, and the
first candidate wins ties. Finish0x1c12c aligns integer phase and sequentially
blends overlapping curves.

The instruction trace corrects two decompiler artifacts: BestCurve returns three
doubles through the HFA ABI, and Finish takes only the object pointer. Incorrect
signatures or truncation in place of signed floor would change the reconstruction.
Retained REA Evidence:
`ev_ea106289b5f1b1c83a0bcbb3281498a3b4f748bdb202ba36470f06332523b71e`,
`ev_bae7a937bbcb23be553a1ab76699828b52c76f0c2fb35c55781a4e533274b0ad`.

## Current verification

On five development cases, the independent Rust implementation exactly matches:

- All432,704 original float32 intermediate scores.
- All27,044 original binary64 finalized tuning values.
- All1,760 outgoing key-feature values after composing the new tuning with the
  frozen independent cache finalizer and feature-bank aggregator on macOS ARM64.
  The final portable chain also passes this comparison on Linux ARM64.

The worker receives only packed retained peaks and a score/tuning stage choice.
References are evaluation-only, opened after worker exit. The composed chain
supplies newly computed tuning and caches to later workers; it never falls back
to the original tuning or cached88 bytes.

A separate original-component replay validates that the same retained peaks are
sufficient inputs. It uses no audio or application attachment. Tuning symbols
are not visible through dlsym; the helper resolves a validated exported C1
constructor to establish image base, checks the installed ARM64 slice/entry
bytes, and calls the pinned offsets. Early symbol-guard failures are retained.
Those original-component results are reference evidence, not independent code.

Six controls contain0/1/2/16/17/33 empty peak frames. All12 Rust stage jobs match,
including69 tuning values and1,104 score values. Every nonempty zero-score curve
is nonzero because the optimizer keeps the first candidate on ties. A silence
shortcut returning zero tuning would therefore disagree with the reference.

Eight initial tests, formatting, Clippy and release build pass on macOS. Three initial
tests were observed failing with a zero-output stub. Input contracts reject
reference fields, malformed bytes, nonfinite inputs and underflowed frequency
ratios. Original five-case proofs are preserved after contract/test changes;
replay outputs remain identical.

The initial Linux run matched all finalized tuning values and controls, but
15,007/432,704 raw scores differed, with maximum955ULP. That strict failure was
preserved. Promoting the logarithm to double changed11 raw scores on macOS, so
that alternative was also rejected. Matching final tuning on these cases did not
justify accepting different internal scores.

A handwritten positive-finite logarithm now follows the pinned runtime's
instructions and65-pair table without host math calls. It matches53,492/53,492
native scalar results on macOS ARM64 and Linux ARM64, including exponent/table
boundaries, subnormal inputs and values near one. The observations were collected
only in an agent-owned diagnostic process, without attaching to the purchased app.

The separate portable tuning revision uses that logarithm and passes all22 Linux
score/tuning jobs exactly:432,704 real scores,27,044 real tuning values,1,104
control scores and69 control tuning values. All13 tests and release checks pass
on both platforms; macOS formatting/Clippy also pass. Double pow/cos remain
dependencies. Other configurations, all floating inputs and Windows execution
are unverified.

The final portable chain combines tuning V2, cache finalizer V4 and the unchanged
request aggregator. On both macOS ARM64 and Linux ARM64, all five cases match
every freshly computed stage:27,044 tuning values,2,379,872 cache values and1,760
request feature values. The Linux audit verifies that newly computed tuning is
the cache worker's sole tuning input and newly computed cache is the bank
worker's sole cache input. Original tuning/cache fallbacks are absent. Cache V4
uses the same source-derived logarithm and passes23 tests and its independent
five-case cache comparison on both platforms.

## Retained identities

| Artifact | Identity |
| --- | --- |
| Rust73-file freeze | `3e5b064dec656f405b49113bd0e7ff0167fe08bdd7c49919c15b30c237c87bb5` |
| Final Rust source | `31cb7f23914bba734ad492d36f6b263e5dc615ecf2256c4c0e5f0dd1da918e88` |
| macOS worker | `b5e168ed4589a3bc156684840c765cf2c6971014357de98448097d6431dc92cd` |
| Original-component73-file freeze | `686ab7661af3c2d99ba7089f6b25feede5cc877483df64897c106a726e020c35` |
| Original-component replay report | `fc40f5cf5b0cba21be9a058a6461dce502e4651c2da675be536524f49d1357e7` |
| Three-stage macOS composition | `0fa99fd8f0eb772333a469c4d446e908079e588b85df245a749d9390440760e2` |
| Failed initial Linux tuning report | `76f53820fd1299b2985fdb8bee3ecf0bcff09014a8312d143682aa2501211816` |
| Positive logarithm22-file freeze | `c59ff0a984c386d836764cf13006d82ae27021108ee52a6f3e278344e514c204` |
| Portable tuning73-file freeze | `95f7f3ecbabb0871f948f1f1835f33813dc03f4d31c3e787eeca937f913da8d4` |
| Exact portable Linux tuning report | `dcdf2274380f8b1ee28167feb80a38fd83377b28dab29cc9db8162a6a1a5c5e2` |
| Source-faithful cache V4 freeze | `9a2ea4a959b616cf2143d2733e0f4893712105db2379f564461a6cfd4d54afed` |
| Final portable macOS chain freeze | `f26525e2bad3c00362b7e08683cd85ea908356702935524510788ccbd9adc94b` |
| Exact Linux cache V4 report | `81601cc54947253c35fb9c3553624363f90c639287eb73a3c91d649351844598` |
| Exact Linux three-stage chain report | `3ba61551c306cc32eac816c5ebe4ca29916f116ef9753ec4a26024ea3b19ea50` |
| Combined completion record | `e767a54f7e0af7524dc168cd33ed25ef8ac318d7b5657f4ffc1383f6ed252e61` |

All per-case data, code witnesses and experiments remain private. No new audio
read, service analysis, held-out access, production integration or UI work was
needed. The next source boundary is producing the tracked peaks from audio.
