# Identify a track with no match

<p class="lead">When a file has no usable artist or title and the catalogs find nothing, CueRithm can recognise the recording from its sound. It sends a fingerprint, never the audio, and you choose the result yourself. Setting it up takes about three minutes.</p>

**Before you start**, tagging must already work for normal tracks. Follow [Tag new tracks from Beatport and Traxsource](tag-new-tracks-automatically.md) if it does not. Identification is off by default and sits beside the other catalog settings in **Settings → Automatic tagging**.

## What identification does { #what-it-does }

Identification is a fallback. It runs only when the normal catalog search found nothing, or could not even start because the file has no title or artist tag. It runs at most once for each preparation attempt.

1. CueRithm builds an audio fingerprint on your computer from up to the first 120 seconds of the track. The track needs at least 12 seconds of audio.
2. It sends that compressed fingerprint and the track's length to AcoustID.
3. AcoustID names matching recordings. CueRithm keeps only strong matches, up to five recordings, and asks MusicBrainz for the releases each one appears on.
4. You get up to ten edition candidates in [Review match](review-a-match.md). CueRithm never picks a mix or a release for you, even when a recording is recognised.

The measured key, BPM, energy and cues are not touched, because they come from your own analysis and not from any catalog.

## Turn it on { #identify-unresolved-recordings }

1. Open **Settings → Automatic tagging** and expand **Identify unresolved recordings**. The section is collapsed until you open it.
2. Tick **Allow fingerprint lookup**. This is your consent to send a compressed fingerprint and the track length to AcoustID. It is stored on this computer only and is left out of exported presets.
3. Tick **Identify unresolved tracks**. It stays disabled until you have given consent, and clearing the consent switches identification off again in the same draft.
4. Choose **Save settings**.

If the section says *Fingerprint lookup is unavailable in this build because its AcoustID application identity is missing*, the two boxes are disabled. That is a property of the build, not of your setup, and you never need to obtain an AcoustID key yourself.

## Identify a track { #identify-a-track }

The example is a file named `track01.flac` with empty artist and title tags.

1. Add the file with **Add tracks**, or drop it into a watch folder.
2. In **Incoming**, the row passes **Matching catalog**. Without a title and artist the catalogs cannot be asked, so identification starts.
3. When it finishes, the row shows a warning triangle. That is expected: recognition always asks you to confirm the edition.
4. Select the triangle to open **Review match**. The candidates are listed with **MusicBrainz** as the provider, and **Recording and release evidence** shows the recording and release identifiers, credits and the release date.
5. Pick the candidate whose release you own. If it is a recording without a release, the conflict **Release edition could not be resolved** asks you to confirm it. Tick **I reviewed the recording/version conflicts and want to use this candidate**.
6. Change the fields you want from **Keep current** to **Replace**, then choose **Use selected match**.

The track continues through preparation and moves into Tracks. [Review and fix a wrong match](review-a-match.md) explains each control.

!!! tip "Try the lighter options first"
    If the file name holds the artist and title, tick **Read artist and title from filename** under **Matching** and save. The usual catalog search then runs without a fingerprint. [Choose providers and fields to write](choose-providers.md#matching) lists the **Filename template**. For a single file you can also fill in the two tags yourself with [Edit tags and artwork](edit-tags-by-hand.md).

## What is sent { #what-is-sent }

| Goes to | What | Why |
|---|---|---|
| AcoustID | The compressed fingerprint, the track length rounded to seconds and CueRithm's application identity | To find recordings with a matching sound |
| MusicBrainz | The identifiers of up to five recordings AcoustID returned | To read their releases, credits and dates |

No audio file is uploaded, and CueRithm has no setting that uploads one. File paths, tags and your library are not sent. Requests keep the same pacing as other providers, and MusicBrainz needs no account.

## What can go wrong { #what-can-go-wrong }

| Symptom | Cause | Fix |
|---|---|---|
| **Identify unresolved tracks** is disabled | You have not ticked **Allow fingerprint lookup**, or the build has no AcoustID identity | Give consent; if the build message appears, use a filename or manual edit instead |
| A warning *Identification skipped: fingerprint lookup needs at least 12 seconds of audio* | The clip is too short | Edit the tags by hand |
| No candidates appear | No recording scored high enough. The track then follows **No catalog match**, or waits in Incoming if the lookup could not start. | Tag it by hand, or choose **Keep existing once** in Review match |
| The candidate list lacks the release you own | A warning says only a bounded selection of editions is shown. **Requery** repeats the normal catalog search, not the fingerprint. | Pick the closest recording and correct the rest with [Edit tags and artwork](edit-tags-by-hand.md) |
| The track waits with a provider error | AcoustID or MusicBrainz was unreachable or rate limited | Retry the row. See [Catalog tagging finds nothing](../help/troubleshooting/no-catalog-match.md). |

## Related { #related }

- [Review and fix a wrong match](review-a-match.md): choose the edition and decide each field.
- [Choose providers and fields to write](choose-providers.md#what-is-sent): the full list of what leaves your computer.
- [Edit tags and artwork](edit-tags-by-hand.md): fill in a track yourself.
- [How does catalog matching work?](../how-it-works/matching.md)

<!-- verified: apps/desktop/src/components/RecognitionSettings.tsx; apps/desktop/src/test/recognition-settings.test.tsx; crates/cuerithm-autotag/src/recognition.rs (120 s, 12 s, score 0.85, five recordings, ten editions, AcoustID fields); crates/cuerithm-autotag/src/lib.rs match_prepared_with_album_guarded (fallback conditions, Ambiguous status); crates/cuerithm-autotag/src/evidence.rs (release_edition_unresolved); crates/cuerithm-autotag/src/input.rs (missing artist/title); .github/workflows/release.yml (key supplied at build time); legacy learn/auto-tagging.md#identify-an-unresolved-recording re-checked -->
<!-- UNVERIFIED: whether the published 0.1.13 downloads contain the AcoustID application identity; the release workflow passes it from a CI secret, which could not be checked here. -->
