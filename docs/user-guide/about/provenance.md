# How CueRithm was made

Mixed In Key and Lexicon DJ shaped how CueRithm's key, tempo, energy and cue features behave. This page says how, and what I do and don't claim.

!!! abstract "In one paragraph"
    I studied how these two products behave, using copies I own, and wrote CueRithm's code myself, from scratch, in Rust. No code, binary, model, interface asset or music from either product is in CueRithm or its repository. CueRithm is not affiliated with either company.

## What I studied

### Mixed In Key

I looked at Mixed In Key 11 to understand how it computes key, energy and cues. My working notes stayed private and are not in the repository.

I no longer follow it. CueRithm does not imitate Mixed In Key's cue inference, attach energy labels to cues or show per-segment energy. Key detection now comes from OpenKeyScan, a separate open-source model, and energy from a different calculation, described below. I used Mixed In Key's exported labels only to measure agreement, never as an input to a prediction.

### Lexicon DJ

I studied Lexicon and ran its analysis on synthetic audio I wrote myself, recording the numeric results. Only those inputs and outputs are kept, as test fixtures. My notes on the application itself stayed private.

This informed three areas of CueRithm. The product versions I compared against are listed in [How accurate is it?](../technical/accuracy.md).

- Tempo and first beat: the Rust timing detector reproduces the behaviour I observed, and its results match the original on every track in my test library.
- Energy: the Rust energy formula reproduces an acoustic calculation from the original, matching it on all but three deliberate differences in 29 synthetic cases.
- Cues: this is the area closest in structure to the original. I validated it against the original's numeric output on authored recipes and on my test library.

## What is in CueRithm

- Code: written in Rust, from scratch, for CueRithm.
- Data: no Mixed In Key or Lexicon model, weights, resource, icon or text is shipped.
- Tests: fixtures contain synthetic audio recipes and numeric outputs only. No third-party music, user library data or credentials are included.
- Dependencies: open-source components, credited in [Credits and open source](credits.md).

## Scope of the work

I did the analysis and wrote the implementation myself, so the code was written with knowledge of how the originals behave. A stricter process would have one person write a specification and another, who had never seen the originals, write the code. I did not work that way, so this page calls CueRithm an independent reimplementation and goes no further.

CueRithm does not claim identical results. Agreement with another program on a particular test set does not establish that either program is musically correct. See [How accurate is it?](../technical/accuracy.md).

This is not legal advice or a statement about those companies' licence terms. If your use of CueRithm depends on this, read the licences of the products involved and seek your own advice.

## Where else the ideas come from

- OpenKeyScan supplies the key network and its constant-Q frontend design.
- OneTagger supplies the catalog matching and tag-writing foundation. See [Credits](credits.md#onetagger).
- The Camelot notation and the idea of a smart playlist are common in DJ software and are used here as general concepts.

## Questions and corrections

If you believe something on this page is wrong, or a statement about a product is inaccurate, please [open an issue](https://github.com/johnneerdael/CueRithm/issues).
