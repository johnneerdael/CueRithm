# How CueRithm was made

CueRithm's key, tempo, energy and cue behaviour was **inspired by two commercial DJ products, Mixed In Key and Lexicon DJ**. This page says exactly how, and what that does and does not mean.

!!! abstract "In one paragraph"
    The behaviour of these products was studied through **reverse engineering of the copies the project's author owns**. CueRithm's code is an **independent implementation written from scratch in Rust**. No original code, binary, model, interface asset or music is included in CueRithm or published in its repository. CueRithm is not affiliated with either company.

## What was studied

### Mixed In Key

The project studied Mixed In Key 11 (version 11.2.6) in order to understand how key, energy and cues are computed and what it exchanges with its service. The work used static analysis and decompilation of the application on a private copy, and observation of its network exchanges with the application's own analysis service. The installed application was left unmodified, and the working records were kept private and excluded from the repository.

That line of work is **retired in the product**. CueRithm no longer imitates Mixed In Key's cue inference, does not attach energy labels to cues and does not show per-segment energy. Key detection now comes from OpenKeyScan, a separate open-source model, and energy from a different calculation described below. Mixed In Key's exported labels were used as a **reference for measuring agreement**, never as an input to any prediction.

### Lexicon DJ

The project studied Lexicon 1.11.15. It extracted the application's code on a private copy, examined it, and ran its shipped analysis worker in isolation on **synthetic, originally authored audio** to record numeric results. Only those authored inputs and numeric outputs were kept as test fixtures. The records of the extracted code were kept private.

This work informed three areas of CueRithm:

- **Tempo and first beat.** The Rust timing detector reproduces the observed behaviour, and its results match the observed worker on every track in the project's test library.
- **Energy.** The Rust energy formula reproduces an acoustic calculation observed in the original, matching it on all but three deliberate differences in 29 synthetic cases.
- **Cues.** The cue detector is the area closest in structure to what was observed. It was validated against the original's numeric output on authored recipes and on the project's test library.

## What is in CueRithm

- **Code**: written in Rust, from scratch, for CueRithm.
- **Data**: no Mixed In Key or Lexicon model, weights, resource, icon or text is shipped.
- **Tests**: fixtures contain synthetic audio recipes and numeric outputs only. No third-party music, user library data or credentials are included.
- **Dependencies**: open-source components, credited in [Credits and open source](credits.md).

## What this is not

- It is **not a separated-team process**. A setup where one group writes a specification and a different group, who never saw the originals, writes the code would be a different and stricter thing. CueRithm's analysis and implementation were done within the same project, and the implementation was written with knowledge of how the originals behave. That is why this page describes an independent reimplementation, and goes no further.
- It is **not a claim of identical results**. Where CueRithm agrees with another program, that is agreement with that program's output on a particular test set, not proof that either is musically correct. See [How accurate is it?](../technical/accuracy.md).
- It is **not legal advice or a statement about those companies' licence terms**. If your use of CueRithm depends on this, read the licences of the products involved and take your own advice.

## Where else the ideas come from

- **OpenKeyScan** supplies the key network and its constant-Q frontend design.
- **OneTagger** supplies the catalog matching and tag-writing foundation. See [Credits](credits.md#onetagger).
- The **Camelot** notation and the idea of a smart playlist are common in DJ software and are used here as general concepts.

## Questions and corrections

If you believe something on this page is wrong, or a statement about a product is inaccurate, please [open an issue](https://github.com/johnneerdael/CueRithm/issues). Corrections are welcome.
