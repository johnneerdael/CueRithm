# Catalog matching

Automatic tagging finds the right Beatport or Traxsource entry for a track. The matching engine and the two providers come from [OneTagger](../about/credits.md#onetagger), with CueRithm changes listed in the credits.

## Providers

| | Beatport | Traxsource |
| --- | --- | --- |
| Described in the catalog as | "Overall more specialized in Techno, can match using ISRC" | "Overall more specialized in House" |
| Account needed | No | No |
| Concurrency | Up to your *Tagging concurrency* (default 16) | One request at a time |
| Search | Catalog search API, 50 results per page | Site search, HTML result list parsed |

Beatport access uses an anonymous OAuth client-credentials token. No user account or password is stored. The token is cached in memory only and refreshed shortly before it expires. Neither provider's credentials are settings.

## Building the query

1. **By embedded ID** (Beatport, only with *Match by embedded IDs*): a stored `BEATPORT_TRACK_ID` fetches the track directly, accuracy 1.0, reason *id*.
2. **By ISRC** (Beatport, same switch): searches the ISRC and takes the first hit, accuracy 1.0, reason *isrc*.
3. **By text:** `"<first artist> <cleaned title>"`. Artists come from the tag, or from the filename if you enabled that and the tags are missing. Beatport rejects searches with more than one pair of parentheses, so brackets and commas are removed from the query.

Beatport scans up to **Max pages** pages, moving on only if nothing matched (or always, when you prefer the oldest or newest release).

## Normalising

Titles are cleaned before comparison:

- lower-case, dashes to spaces, collapsed spaces, a leading "a", "an" or "the" dropped;
- a trailing "(original mix)", "(original version)" or "(original edit)" removed;
- 23 listed attributes removed, such as "(intro)", "(clean)", "(dirty)", "(extended)", "(radio edit)", "(club version)" and "(instrumental)";
- "feat." and "ft." clauses removed;
- for the actual comparison, the word "edit" and special characters are also removed and the text is transliterated to ASCII.

Artists are cleaned the same way and compared as sorted sets.

## Scoring

1. **Exact.** The file's title is tried through seven progressively stronger cleaning steps. If one equals the cleaned catalog title and at least one artist matches, the candidate scores 1.0.
2. **Fuzzy.** Otherwise, for each candidate whose artist matches, the score is the normalised Levenshtein similarity of the cleaned titles, and it is kept if it is at or above the strictness (default 0.70).
3. **Artist match** is true if any cleaned artist is equal, or one joined artist string contains the other's name, or the joined artists are similar enough by the same measure.
4. **Duration** (only if enabled): rejects candidates more than the allowed seconds away from the decoded track length (default 30). A zero or unknown duration passes.
5. **Ordering.** *Best match* sorts by score. *Oldest* or *newest release* sort by date. The first candidate wins.

There is no separate low-confidence band: a candidate is accepted or dropped.

Matching is intentionally independent of the audio analysis. After a winner is chosen, the provider fetches the full track and, if needed, the release record for album artist, track total, catalogue number and artwork.

## Album matching

With *Album matching* on, when enough Incoming files in one folder resolve to the same Beatport release, that release's track list is fetched once and used as the candidate pool for the rest of the folder.

## Outcomes

| Status | Meaning |
| --- | --- |
| `matched` | A candidate cleared the threshold. |
| `no_match` | None did. What happens next follows *No catalog match*. |
| `already_tagged` | *Skip already tagged* is on and the file carries a marker. Analysis and cues still run. |

An empty Traxsource result list is a no-match. A missing result list or a malformed row is an error, so a changed or blocked page leaves the track in Incoming instead of silently skipping the provider.

## Network behaviour

The HTTP client has a 30-second timeout, follows at most five redirects and caps response bodies at 8 MiB. There is no automatic retry or backoff on HTTP 429. Quitting the app cancels waiting provider calls. Offline, lookups fail, the job goes to *attention* at the matching stage and is retried when you save Settings or press retry.

!!! warning "Live status"
    Beatport has one successful native probe. Traxsource requests returned HTTP 403 on the validation machine, so live Traxsource success is not established.
