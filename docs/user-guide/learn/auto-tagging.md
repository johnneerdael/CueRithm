# Set up catalog tagging

Turn on automatic catalog tagging, choose which fields to write, and learn how CueRithm protects your audio files during tag writes.

Tagging is off by default. Analysis and cue generation work offline with tagging disabled.

## 1. Open the settings

Go to **Settings > Automatic tagging**.

<figure markdown>
  ![Settings, Automatic tagging page, with provider order Beatport then Traxsource](../assets/screens/12-settings-automatic-tagging.webp){ .screen loading=lazy }
  <figcaption>Automatic tagging. Providers run in the order listed.</figcaption>
</figure>

## 2. Switch it on

Tick **Automatic tagging**. Note the line beneath: *Catalog lookups use an internet connection. Analysis and cue generation work offline.*

## 3. Choose providers

Select providers with their checkboxes and arrange them with the up and down
arrows. Beatport and Traxsource remain selected by default. MusicBrainz, SonoVault,
Discogs, iTunes, BPM Supreme, Bandcamp and Deezer are
additional opt-in choices. See the [provider list](../guides/tagging-settings.md#provider-setup)
for their IDs and setup requirements. A listed source is not a promise that its
service is reachable; source errors leave an actionable result in Incoming.

- **Combine providers** off (the default): the first provider that finds a match wins.
- **Combine providers** on: every selected provider is asked, and their matches apply in order.

### Optional provider accounts { #provider-setup }

MusicBrainz is keyless. Beatport, Traxsource, iTunes,
Bandcamp and Deezer have no end-user account setup in CueRithm. A detected source
challenge may still need the [optional solver](../guides/catalog-solver.md).

For SonoVault, Discogs or BPM Supreme:

1. Open **Set up** beside the provider. Use **Manage** for an existing credential.
2. For SonoVault, follow **Get a free API key**, verify the account email, then
   enter the personal key. Discogs accepts an optional personal token. BPM Supreme
   uses separate email and password fields.
3. Press the provider's **Save** button. This stores the credential immediately
   in the system credential store; it does not enable the provider or save your
   other settings. Key/password inputs are masked and cleared after submission.
4. Press **Test saved … connection** to check it. SonoVault testing spends one
   metadata credit. It does not upload audio or perform recognition. A mock/test
   key cannot establish live access. **Cancel provider operation** stops a pending
   test; **Remove** revokes the credential, including an in-flight test.
5. Select the provider if wanted, then press **Save settings** to apply its
   selection, order and field policies.

Discogs works without an account at up to 25 requests/minute. A valid account
token verified by CueRithm allows up to 60; no developer application registration
is required. SonoVault requires a personal key only if you choose to use it.
The signup link is [sonovault.now/signup](https://sonovault.now/signup).

If a test reports an unverified email, rejected credential or exhausted allowance,
resolve that condition before retrying. Unknown quota counts or reset times stay
unknown. Request pacing is shared across libraries and survives restart. A second
CueRithm process may report private credential access as busy; offline analysis
and editing remain available. Keys are not read automatically from user key files
and never appear in exported presets.

## 4. Decide what happens without a match

**No catalog match** can be:

- **Keep existing tags and finish**: the track continues and moves into Tracks (the default).
- **Keep in Incoming for attention**: the track waits for you.

A provider *error*, such as being offline, is never treated as "no match". The track stays in Incoming so you can retry.

## 5. Choose the fields to write

Scroll the page to **Fields to write**. By default CueRithm writes genre, BPM, style, label and release date. Title, artist, album, key, artwork, ISRC and identifiers are off until you tick them. Measured key and BPM from CueRithm's own analysis are written by default, not the catalog's values, and the library always keeps its own measurements.

## 6. Save and run it

Press **Save settings**. New tracks, tracks arriving in a watch folder and
**Analyze again** use your settings. Existing tracks are not retagged just because
you changed a setting. Optional file organization is off by default; an enabled
naming rule runs last after accepted tagging and cleanup, before the track becomes
ready. See [File organization](../guides/file-organization.md).

## Identify an unresolved recording

Expand **Identify unresolved recordings**. Both consent and identification are
off by default. Enable **Allow fingerprint lookup**, then **Identify unresolved
tracks**, and save settings. CueRithm creates the Chromaprint fingerprint locally
and sends only the compressed fingerprint and duration to AcoustID. No audio is
uploaded, and no end-user application key is needed. If the packaged build lacks
its registered application identity, lookup is shown as unavailable.

This fallback runs only when normal matching cannot resolve a readable track,
with one lookup per preparation attempt. Short clips may not have enough usable
audio. MusicBrainz recording/release candidates stay in Incoming for edition
review; recognition never silently chooses a mix or release. Key, BPM, energy and
cues retain their measured analysis. Local consent is not exported in presets.

Spotify is skipped. AudD and RapidAPI are not selectable; research/trial checks
do not enable uploads or provider use in this interface.

## Match a folder as a release

With **Album matching** enabled, matching tracks in the same Incoming folder can
identify a release once the configured match ratio is reached. Both Beatport and
Traxsource supply complete release tracklists for the remaining files. CueRithm
keeps each provider's release identity separate and retains the original track
position and total. Traxsource follows the release linked from a track match;
it does not guess a release from its title.

Traxsource release lookups use the same optional challenge solver as track
lookups. Missing, malformed or blocked release pages are errors and leave the
job in Incoming. These release paths are covered by offline fixtures; they have
not been checked against the live catalog.

## The safeguards

- CueRithm writes tags to a staged copy of the file.
- It decodes that copy again and checks that the audio matches the original sample for sample. A mismatch stops the write.
- The original is kept in recovery backups, whose size is shown on the settings page.
- An atomic swap replaces your file. If anything fails afterwards, CueRithm restores the original. If you have edited the file in the meantime, it preserves your newer edit.

!!! warning "Traxsource status"
    Traxsource can require a browser challenge. Configure an optional [FlareSolverr-compatible solver](../guides/catalog-solver.md) to use it when blocked. CueRithm remembers detected restrictions and stops direct retries; without a solver the source becomes unavailable. A failed required lookup leaves tracks in Incoming. Live fetching and parsing were verified with Trawl, without writing music tags.

## What just happened

Matching normalises titles and artists, searches the catalog and ranks candidates
against your strictness setting (0.70 by default). A similarity score is a ranking
signal, not a probability that the recording or edition is correct. Known conflicts
and ambiguous choices stay in Incoming for [match review](../guides/metadata-editing.md).
See [Catalog matching](../technical/matching.md), [Safe tag writing](../technical/tag-writing.md)
and [Automatic tagging settings](../guides/tagging-settings.md). The tagging code
is adapted from [OneTagger](../about/credits.md#onetagger).
