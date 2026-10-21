# Automatic tagging settings

Open **Settings > Automatic tagging** to configure tagging for new tracks, watch-folder arrivals and **Analyze again**. Changing these settings does not retag finished tracks.

<figure markdown>
  ![Automatic tagging settings](../assets/screens/12-settings-automatic-tagging.webp){ .screen loading=lazy }
</figure>

## Master switches

| Setting | Default | Meaning |
| --- | --- | --- |
| **Automatic tagging** | off | Catalog lookups need the internet. Analysis and cues stay offline. |
| **Recovery backups** | n/a | Read-only. Size and count of retained original files. |
| **Providers** | Beatport, then Traxsource | Select and order providers. Additional providers are opt-in. |
| **Combine providers** | off | Off: first provider with a match wins. On: apply matches from every provider in order. |
| **No catalog match** | Keep existing tags and finish | Or *Keep in Incoming for attention*. |

## Provider setup

| Provider | ID | End-user setup |
| --- | --- | --- |
| Beatport | `beatport` | No account. |
| Traxsource | `traxsource` | No account; a detected challenge may require a solver. |
| MusicBrainz | `musicbrainz` | No account or API key. |
| SonoVault | `sonovault` | Optional personal API key and verified email. |
| Discogs | `discogs` | Anonymous by default; optional verified personal token. |
| iTunes | `itunes` | No account. |
| BPM Supreme | `bpmsupreme` | Optional account email and password. |
| Bandcamp | `bandcamp` | No account; source restrictions can affect access. |
| Deezer | `deezer` | No account. |
| Volumo | `volumo` | Optional personal API key; official ISRC-to-links lookup and public track metadata. |

**Set up** and **Manage** open credential controls for the optional providers.
**Save**, **Test**, **Replace** and **Remove** act immediately on the system
credential store. **Save settings** separately saves provider selection, order,
field policies and other preparation rules. Saving a key does not enable a
provider. Key and password inputs stay masked and are cleared after submission
or when leaving; saved secrets are never shown or exported. User key files are not discovered
automatically. Follow [Set up catalog tagging](../learn/auto-tagging.md#provider-setup)
for the steps.

SonoVault links to [Get a free API key](https://sonovault.now/signup). Verify the
account email first. **Test saved SonoVault connection** uses one metadata credit,
without uploading audio or invoking recognition. Mock/test keys do not establish
live access. Rate or quota messages show known timing only; unknown remaining
allowance and reset times are not estimated. If no reset is known after a quota
error, check the allowance before explicitly testing again.

For Volumo, use **Create a Volumo API token** to open
[account token settings](https://volumo.com/profile/api-tokens). A personal key is
required only when choosing this provider. **Test saved Volumo connection** checks
one sample ISRC lookup. Volumo's official API resolves ISRCs to links, with public
track-page metadata available from the returned link; it is not a full free
catalog-search API. Missing or withdrawn sample data leaves the key unverified,
not invalid. Unknown rate or quota details are not estimated.

Discogs needs no developer application registration. Anonymous access is capped
at 25 requests/minute; a successfully verified account token allows up to 60.
Stricter server limits still apply. Provider budgets are shared across libraries
and retained across restarts. Private setup can be busy in a second CueRithm
process; offline analysis and editing remain usable.

Spotify is skipped. AudD and RapidAPI are not selectable; their trial/research
evaluation does not enable audio uploads in this interface.

## Identify unresolved recordings

This section is collapsed and off by default. **Allow fingerprint lookup** permits
sending a locally generated Chromaprint fingerprint and duration to AcoustID;
the audio file is not uploaded. **Identify unresolved tracks** enables the
fallback, with one lookup per preparation attempt. Save settings to apply it.
The build supplies the registered application key. If absent, the controls show
that lookup is unavailable; end users do not obtain an application key.

Results stay in Incoming for recording/release edition review, even when a
recording is recognized. Short clips may be unavailable. Measured analysis stays
independent. Fingerprint consent is local and is excluded from portable presets,
along with credentials and local folder connections.

## Cloudflare challenge solver

Enter an optional **Solver API URL** for a FlareSolverr-compatible service. The
default **Solver timeout** is 120 seconds, with one retry for temporary failures.
Detected blocked sources use the solver; other sources keep their normal
connection. Without a solver, a detected blocked source is unavailable.
**Retry direct source access** clears remembered restrictions.
See [Cloudflare challenge solver](catalog-solver.md) for setup with FlareSolverr,
Byparr or Trawl, source status, privacy and troubleshooting.

## Matching

| Setting | Default | Meaning |
| --- | --- | --- |
| **Matching strictness** | 0.70 | Minimum title similarity (0 to 1) for an accepted match. |
| **Match duration** / **Maximum duration difference** | off / 30 s | Rejects candidates whose length differs from the track by more than the allowance. |
| **Match by embedded IDs** | off | Use provider IDs or ISRC where the selected provider supports them. Identity and edition checks still apply. |
| **Read artist and title from filename** | off | When tags are missing. Template default `%artists% - %title%`. |
| **Title cleanup pattern** | empty | A regular expression removed from the title before matching. |
| **Skip already tagged** | off | Skips the catalog lookup for files carrying a CueRithm or legacy OneTagger tagging marker. Analysis and cues still run. |
| **Album matching** | off | When enough Incoming files in one folder match the same Beatport or Traxsource release (ratio default 0.5), use that release's track list for the rest. |
| **Tagging concurrency** | 16 | Shared catalog concurrency (1 to 64). Provider-specific pacing and serial limits still apply. |
| **Ordering** | Best match | Or oldest or newest release. |
| Beatport: **Max pages** | 1 | 1 to 10 pages of 50 results. |
| Beatport: **Ignore version when matching** | off | Ignores mix text while scoring titles. Known edition, recording ID and duration conflicts still require review. |

## Fields to write

Each field has **Write** and **Overwrite** checkboxes, and there is a global **Overwrite all selected fields** (on by default; it forces every overwrite box on). Unselected fields are never touched.

Genre, BPM, style, label and release date are on by default. Title, artist, album, key, artwork, ISRC, identifiers and the tagging marker are off.

Fields the providers can supply include title, version, artist, album artist, album, BPM, genre, style, label, URL, release and publish date, key, album art, remixer, catalogue number, track number and total, ISRC, duration and provider IDs. Each field shows which providers can fill it.

MusicBrainz review distinguishes the recording, release, release group and release
track. Scoped identifiers and ordered artist credits are read-only evidence in
Review; common metadata, dates and selected artwork use the normal field choices.
A recording's first-release date is not assumed to be the selected release date.
Incomplete dates keep their precision. Artwork is optional and provider-specific.

## How measured values are written

| Setting | Default | Meaning |
| --- | --- | --- |
| **Key and BPM written to files** | CueRithm analysis | Or catalog metadata. The library always keeps its own measurements. |
| **Write Camelot keys** | off | Writes `8A`-style keys instead of musical names. |
| **Write track energy to files** | on | One number from 1 to 10 in a custom `ENERGY` field. |
| **Write short titles** | off | Omit the mix or version from the title field. |
| **Write year only** | off | Dates as the year only. |
| **Track number padding** | 0 | Minimum digits, up to 8. |

## Genre and style routing

There are eight modes. The default is **Genres and styles separately**; the others are genres only, styles only, merge both into genres, merge both into styles, styles into genres, genres into styles, or styles into a custom field (default name `STYLE`). **Merge with existing genres** and **Capitalize genres** are both off by default.

## Artwork

Artwork is off unless **Album art** is ticked. **Artwork destination** is **Embed in audio file** (default) or **External cover file** (a `cover.jpg`-style file beside the audio, with a template you can edit). **Replace all embedded covers** removes every picture type first. Beatport **Album art resolution** is 200 to 1600 pixels, default 500. Images must be PNG, JPEG, GIF or WebP, decode within 4096 by 4096 pixels and be under 8 MiB.

## File tag format

Choose the **ID3 version** (v2.4 by default, v2.3 optional), an optional ID3 **comment language**, and **value separators** for ID3 (default `, `), Vorbis (empty means separate native values) and MP4 (default `, `).

!!! info "Attribution"
    The page ends with: *Tagging uses OneTagger source under GPLv3. CueRithm desktop is GPLv3; the separate analysis engine remains MIT.*
