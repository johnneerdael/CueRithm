# Choose providers and fields to write

<p class="lead">Decide which catalogs CueRithm asks, in what order, which tag fields it may write and how strict matching should be. Allow about ten minutes the first time; afterwards you only come back to change one thing.</p>

**Before you start**, turn tagging on once by following [Tag new tracks from Beatport and Traxsource](tag-new-tracks-automatically.md). Every control on this page lives in **Settings → Automatic tagging**, and nothing takes effect until you choose **Save settings**. A saved change applies to new tracks, to watch-folder arrivals and to [Analyze again](../analysis/analyze-again.md), never to tracks already finished.

## Choose providers and their order { #providers }

1. Open **Settings → Automatic tagging** and find the provider list.
2. Tick **Use** beside each provider you want. The number on the left is its position; an unticked provider shows a dash.
3. Use the up and down arrows (**Move … earlier**, **Move … later**) to reorder the ticked providers.

A provider that needs an account stays greyed out until you set it up, and **Traxsource** stays greyed out with *Unavailable · configure a solver* once CueRithm has detected a Cloudflare check and no solver is saved. See [Connect provider accounts](provider-accounts.md) and [Get past a Cloudflare block](cloudflare-solver.md).

CueRithm has ten providers. Beatport and Traxsource are ticked by default.

| Provider | Account | Pacing | Good to know |
|---|---|---|---|
| Beatport | None | One request start a second, up to 60 a minute | Can match by ISRC with **Match by embedded IDs**. Has its own settings (see [Beatport options](#beatport-options)). |
| Traxsource | None; may need a solver | One request at a time | With Beatport, one of two providers that supply key and BPM. |
| MusicBrainz | None | One request a second | Open database. Keeps the recording and the release separate, so a recording can need an edition choice in Review. |
| SonoVault | Free personal API key (verify your email first) | One request every 3 seconds, up to 20 a minute | Looks up by the ISRC in the file when there is one, otherwise by artist and title. Writes only Title, Artist, Album, Label, Genre, Release date, Duration and ISRC. |
| Discogs | None; personal token optional | 25 a minute, 60 with a verified token | Works anonymously. |
| Volumo | Personal API key | One request every 3 seconds, up to 20 a minute | Needs an ISRC in the file (or a saved Volumo track ID with **Match by embedded IDs**). It cannot search by artist and title. |
| iTunes | None | One request every 3 seconds, up to 20 a minute | |
| BPM Supreme | Email and password | One request a second | Unavailable until **Test saved BPM Supreme connection** succeeds. |
| Bandcamp | None | One request a second | If Bandcamp shows a Cloudflare check, a solver does not help its search. |
| Deezer | None | One request a second | Can match by ISRC with **Match by embedded IDs**. |

The extra eight are opt-in. A provider that fails is never counted as "no match": with **Combine providers** off CueRithm tries the next provider, and if none of them matches, the track waits in Incoming.

## Decide how providers work together { #combine-providers }

| **Combine providers** | What happens |
|---|---|
| Off (default) | CueRithm asks providers in order and stops at the first one with a match. If a provider fails, it moves on to the next. |
| On | Every ticked provider is asked and their matches are applied one after another, in list order. A failure from any provider keeps the track in Incoming. |

With **Overwrite** on for a field, a provider later in the list replaces what an earlier one wrote. With it off, a field that already has a value, including one an earlier provider wrote a moment before, stays as it is. Use Combine when one provider supplies something another lacks, such as Beatport for style and key plus Discogs for disc number.

## Decide what happens when nothing matches { #no-match }

**No catalog match** has two settings:

- **Keep existing tags and finish** (default): the track keeps its tags and moves into Tracks. Its status icon reads *No catalog match; existing tags kept*.
- **Keep in Incoming for attention**: the track waits for you. Open [Review match](review-a-match.md) to search again or continue with **Keep existing once**.

A provider error is never "no match". It always keeps the track in Incoming.

## Choose the fields to write { #fields-to-write }

Under **Fields to write**, each field has **Write** and **Overwrite** boxes. A field you do not tick **Write** for is never touched. **Overwrite all selected fields** is on by default and forces every Overwrite box on; turn it off to fill empty fields only and keep existing values.

Under each field name the app lists which of your ticked providers can supply it, or *Not supplied by the selected providers*.

| Field | Written to the file as | Beatport | Traxsource | Other providers | On by default |
|---|---|---|---|---|---|
| Title | Title, with the mix in brackets (turn on **Write short titles** to drop it) | yes | yes | yes | no |
| Version | Mix or version | yes | yes | no | no |
| Artist | Artists | yes | yes | yes | no |
| Album artist | Album artist | yes | yes | no | no |
| Album | Album | yes | yes | yes | no |
| BPM | BPM, from CueRithm's analysis or the catalog | yes | yes | no | yes |
| Genre | Genre | yes | yes | yes | yes |
| Style | Style (sub-genre) | yes | no | no | yes |
| Label | Label | yes | yes | yes | yes |
| Catalog URL | The track's page address | yes | yes | yes | no |
| Release date | Release date | yes | yes | yes | yes |
| Publish date | Original release date | yes | no | no | no |
| Key | Key | yes | yes | no | no |
| Album art | Front cover or a cover file | yes | yes | yes | no |
| Other provider tags | Beatport's extra tags | yes | no | no | no |
| Provider track ID | For example `BEATPORT_TRACK_ID` | yes | yes | yes | no |
| Provider release ID | For example `TRAXSOURCE_RELEASE_ID` | yes | yes | yes | no |
| Duration | The track's own length | yes | yes | yes | no |
| Remixer | Remixers | yes | no | no | no |
| Catalog number | Catalog number | yes | yes | no | no |
| Track number, Track total | Position on the release | yes | yes | yes | no |
| ISRC | ISRC | yes | no | yes | no |
| Disc number | Disc | no | no | yes | no |
| Tagging date marker | `CUERITHM_TAGGEDDATE`, written by CueRithm itself and read by **Skip previously tagged catalog lookups** | yes | yes | yes | no |

SonoVault supplies only Title, Artist, Album, Label, Genre, Release date, Duration and ISRC, whatever the column says for "other providers". A catalog never changes the measured key, BPM, energy or cues in your library.

## Choose how key, BPM and energy are written { #how-measured-values-are-written }

| Setting | Default | What it does |
|---|---|---|
| **Key and BPM written to files** | **Cuerithm analysis** | **Cuerithm analysis** writes CueRithm's measured key and BPM into the file's tags when the Key and BPM fields are ticked. **Catalog metadata** writes the provider's values instead. The library keeps its measured values either way. |
| **Write Camelot keys** | off | Writes keys such as `8A` instead of musical names. |
| **Write track energy to files** | on | Writes one whole-track number from 1 to 10 into a custom `ENERGY` field. It is a separate switch and is not controlled by the field list. |
| **Write short titles** | off | Keeps the mix out of the title, so `Man Like Me (Extended Mix)` becomes `Man Like Me`. |
| **Write year only** | off | Writes `2024` instead of a full date. |
| **Track number padding** | 0 | Minimum digits, up to 8. A value of 2 writes `01`, `02` and so on. |

## Route genres and styles { #genre-and-style-routing }

Beatport supplies both a genre and a more specific style. **Genre and style routing** decides where they go.

| Route | Result |
|---|---|
| **Genres and styles separately** (default) | Genre in the genre field, style in a style field |
| **Genres only** / **Styles only** | Only one of the two is written |
| **Merge both into genres** / **Merge both into styles** | Both lists land in one field |
| **Styles into genres** / **Genres into styles** | One list replaces the other's field |
| **Styles into custom fields** | Styles go to a field you name per format (**Style field · ID3**, **· VORBIS**, **· MP4**, each `STYLE` by default) |

**Merge with existing genres** and **Capitalize genres** are off by default.

## Set the artwork destination { #artwork }

Artwork is only fetched when **Album art** is ticked under Fields to write. **Artwork destination** then has two choices:

- **Embed in audio file** (default). The catalog cover becomes the front cover when **Overwrite** is on for Album art or the file has no cover. **Replace all embedded covers** removes every picture type first.
- **External cover file**. The cover is saved beside the audio as `cover.jpg` or the name in **Cover filename template** (subfolders allowed). Embedded covers stay.

Images must be PNG, JPEG, GIF or WebP, under 8 MiB and no larger than 4096 by 4096 pixels. For Beatport, **Album art resolution** sets the size to request.

## Choose the tag format { #file-tag-format }

| Setting | Default | Notes |
|---|---|---|
| **ID3 version** | ID3v2.4 | ID3v2.3 is available for older software. Applies to MP3, AAC (ADTS), WAV and AIFF. |
| **ID3 comment language** | empty | Optional three-letter code. |
| **Value separator · ID3** | `, ` | Joins several artists or genres in one frame. |
| **Value separator · VORBIS** | empty | Empty writes separate native values. |
| **Value separator · MP4** | `, ` | |

## Tune how strictly tracks are matched { #matching }

These live under **Matching**. The defaults suit most purchases.

| Setting | Default | What it does |
|---|---|---|
| **Matching strictness** | 0.70 | Lowest title and artist similarity a candidate needs, from 0 to 1. Higher means fewer, closer matches. |
| **Tagging concurrency** | 16 | Lookups in flight, 1 to 64. Provider pacing still applies. |
| **Match duration** / **Maximum duration difference** | off / 30 | Reject candidates whose length differs from the track by more than the allowance, in seconds (0 to 600). |
| **Match by embedded IDs** | off | Use a provider track ID or ISRC stored in the file, where the provider supports it. |
| **Multiple catalog matches** | Best match | **Oldest release** or **Newest release** choose among compatible candidates by date. |
| **Include subfolders** | on | Applies to watched folders and folders you add. |
| **Read artist and title from filename** | off | Used when tags are missing. **Filename template** defaults to `%artists% - %title%`. |
| **Title cleanup pattern** | empty | A regular expression removed from the title before matching. |
| **Skip previously tagged catalog lookups** | off | Skips the lookup for files with a CueRithm or OneTagger marker. Analysis and cues still run. |
| **Album matching** / **Album matching ratio** | off / 0.5 | See [Match a folder as a release](match-a-release.md). |

Similarity is a ranking score, not a probability. With **Multiple catalog matches** on **Best match**, two candidates within 0.01 of each other go to [Review match](review-a-match.md), and so do candidates that disagree about the mix, the ISRC or (with **Match duration** on) the length. CueRithm does not pick between them silently.

### Beatport options { #beatport-options }

| Setting | Default | What it does |
|---|---|---|
| **Album art resolution** | 500 | 200 to 1600 pixels in steps of 100. |
| **Max pages** | 1 | How many pages of search results to scan, 1 to 10. |
| **Ignore version when matching** | off | Ignores `(Extended Mix)`, `(Original Mix)` and similar while scoring titles. Known edition, recording ID and duration conflicts still need review. |

## What CueRithm sends { #what-is-sent }

Catalog tagging is the only part of CueRithm that goes online, and only for providers you ticked.

| Goes to | What is sent | When |
|---|---|---|
| Each ticked provider | A search made from the artist and title in the file, or from the filename if you enabled that. Beatport, Traxsource, iTunes, Deezer, Bandcamp and BPM Supreme get the first artist and a cleaned title. SonoVault and Discogs get every artist and the title as stored; MusicBrainz gets the first artist and the title as stored. | **Matching catalog**, and when you press **Requery** |
| Providers that support IDs | An ISRC or provider ID already in the file | SonoVault and MusicBrainz when the file has one; Beatport and Deezer only with **Match by embedded IDs**; Volumo, which cannot search by name, always |
| The matched provider | A request for the track page or its release, to read album, label, track total and the cover address | After a match |
| The provider's image host | A request for the cover image, straight from your computer | When **Album art** is ticked, or you press **Preview candidate artwork** |
| Your Cloudflare solver | The catalog page address, including the search words | Only for a source detected as blocked |
| A provider you set up | Your key, token or account, to that provider only | Setup, test and lookups |

CueRithm never sends audio files, file paths, your library database, key, BPM, energy or cues. [Identify a track with no match](identify-unknown-tracks.md) is a separate, opt-in feature that sends a fingerprint. Credentials live in the system credential store, not in settings or presets. Requests are spaced by the pacing in the provider table.

## What can go wrong { #what-can-go-wrong }

| Symptom | Cause | Fix |
|---|---|---|
| A provider is greyed out | It needs an account, or Traxsource needs a solver | [Connect provider accounts](provider-accounts.md) or [Get past a Cloudflare block](cloudflare-solver.md) |
| Genres you edited were replaced | **Overwrite all selected fields** is on | Turn it off, or untick **Genre** under Fields to write |
| Nothing was written for a field | The field's **Write** box is off, or no ticked provider supplies it | Tick it; check the line under the field name |
| Volumo returns nothing | The file has no ISRC | Add an ISRC with [Edit tags and artwork](edit-tags-by-hand.md), or use another provider |
| Many tracks stop in Incoming | A provider is offline or rate limited | Wait, then press the row's retry button. See [Catalog tagging finds nothing](../help/troubleshooting/no-catalog-match.md). |

## Related { #related }

- [Automatic tagging settings](../reference/settings/automatic-tagging.md): the full control list.
- [How does catalog matching work?](../how-it-works/matching.md): normalising and scoring in detail.
- [Save your setup as a preset](../files/presets.md) and [Use different rules for different folders](../files/folder-presets.md): keep several tagging setups.
- [Review and fix a wrong match](review-a-match.md)

<!-- verified: apps/desktop/src/views/TaggingSettings.tsx; apps/desktop/src/components/ProviderSetup.tsx; apps/desktop/src/test/tagging-catalog.json (defaults); crates/cuerithm-autotag/src/providers.rs (ten providers, supported tags, SonoVault policy in lib.rs provider_native_field_allowed); vendor/onetagger/providers/beatport.rs and traxsource.rs (supported_tags, custom options); crates/cuerithm-autotag/src/catalog_runtime/requests.rs (pacing); crates/cuerithm-autotag/src/write.rs (Overwrite order, titles, artwork, energy, key/BPM); crates/cuerithm-autotag/src/lib.rs (Combine, ties, errors); crates/cuerithm-autotag/src/volumo.rs and sonovault.rs (ISRC lookups); crates/cuerithm-autotag/src/image.rs (artwork limits); legacy guides/tagging-settings.md re-checked -->
