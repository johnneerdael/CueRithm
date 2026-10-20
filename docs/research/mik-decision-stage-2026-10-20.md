# Mixed In Key decision stage: what is understood and what is not, 2026-10-20

Aggregates only. The request/response captures, the music and per-song results stay private. The code that produced these
numbers is in `research/mik-decision-stage/`, with a task file for continuing the work elsewhere.

## The picture

Mixed In Key 11 computes its features on the machine and the vendor's service makes the decisions. Static analysis of the
installed versions 11.2.6, 10.3.3 (macOS) and 8.5 (Windows) shows the same cloud path in every one that was traced; only
the old Windows 2.5 build has a local key classifier (four pairs of 12-value profiles), which is a different algorithm.
The service's classifiers are therefore not in any binary and have to be fitted from request/response pairs.

**The request side is solved.** Rebuilding the request from the audio with `crates/camelot-source-analysis` matched the
captured request on every field on all 272 tuning songs (distinct songs with a captured pair): tempo, downbeat, cue start
beat, the per-beat cue scores, the 352 key features, the duration and every 183-value energy window, bit for bit, about
10 s per song. The beat grid that the cue scores depend on is computed locally by the client, appears nowhere in the
response, and is reproduced exactly. (The profile is 44.1 kHz stereo 16-bit FLAC; other formats depend on matching the
client's decoder.)

**The response side is fitted, not recovered.**

| Decision | Best so far (tuning songs, out of fold) | Notes |
| --- | --- | --- |
| Key | 96.0% of 272 (plain linear model 92.7%) | linear in the tonic-aligned unfolded features; mismatches sit at low server confidence |
| Window energy level | 85.3% exact, 96.6% within one (5,956 windows) | RBF SVC on raw features; standardizing loses 12 points |
| Overall energy | 76.5% exact, errors +-1 | 80th percentile of predicted window levels; even the true levels give only about 80.5% |
| Cue set | about 60% of tracks fully right, 91-96% of cues found | selection rule not recovered; the first cue and cue times are exact |

## Checks against the patents

- **Key (US 7,842,878).** Cluster-correlation and naive-Bayes classifiers over the folded 12-class profile of our banks reach
  77-89%, below a plain linear model on all 352 features (92.7%) and far below the tonic-rotation-shared linear model on the
  unfolded bins (96.0%). The bins are 4 banks of 88 piano keys; bin j is pitch class (j+9)%12, and the strongest folded class
  is the tonic for the minor keys that dominate the corpus. Folding octaves first is not what the current service does.
- **Energy (US 8,865,993).** The windows are the 32-beat segments of the patent (15.6 s at 123 BPM) and a reference-trained
  SVM is what fits best. Window neighbours, position in the track and track-wide statistics made the window classifier worse
  (neighbours 77%, previous window 80%, position 56%, track mean 81%, against 85.3% alone), so the level looks like a function
  of the window's own features. Mismatches sit at the extremes (level 8 is right 55%) and next to level changes (80.8% there,
  92.6% inside runs). The patent's median/mean/75th percentile aggregators did not beat the 80th percentile.
- **Cues (US 10,366,121).** The client's per-beat scores need no genre input (reproduced exactly without it); 87.5% of gaps
  between cues are multiples of 32 beats and cues are at least 32 beats apart, as the patent says.

## Genre

227 of the 272 tuning songs are melodic house and techno, 45 are other genres.

| | melodic house and techno | other |
| --- | --- | --- |
| Key | 96.0% | 88.9% |
| Window energy level | 84.9% | 87.3% |
| Overall energy | 75.8% | 80.0% |
| Every cue right (baseline ranker) | 50.7% | 40.0% |

Most mismatches are in melodic house and techno, so genre does not explain them. The small differences for key and cues may
come from the training mix (only 11 major keys in the corpus; fewer songs outside melodic house and techno) rather than from
genre-specific weights; eight songs or fewer per other genre cannot decide that. A set with many songs per genre can.

## What would move the numbers

- **More songs.** The window classifier gains about 3 points per doubling of training songs (21 songs 71.1%, 217 songs
  84.4%). The owner's rekordbox library holds 3,400 tracks that Mixed In Key analysed (3,400 comments of the form
  `<key> - Energy <n>`, 2,024 tracks with eight hot plus eight memory cues named `Energy <n>`, 87% melodic house and techno,
  the rest other genres), on an external drive that is not attached to this machine. The request and response for each can be
  captured on another machine; the decision-stage models need only those pairs, not the audio.
- **The overall-energy aggregator.** Given the response's own (true) window levels and volumes, the best rule or learned model
  explains only about 80.5% of the overall ratings, so the service must use something finer than the rounded levels. That, not
  the level classifier (76.5% from predicted levels), is the bottleneck, and it needs a bag-level model on many songs.
- **The cue selection rule**, which is the least understood part.

`research/mik-decision-stage/AGENT_TASK.md` lists the ranked experiments, the success bar, the protocol (sealed held-out songs,
no replay to the vendor, aggregates only) and how to hand results back.
