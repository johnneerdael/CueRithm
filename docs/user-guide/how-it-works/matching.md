# How does catalog matching work?

<p class="lead">Automatic tagging looks each track up in the catalogs you chose, scores the candidates against your file's title and artist, and either applies one clear winner or stops and asks you. This page explains how it scores, and when it refuses to guess.</p>

!!! abstract "What this means for you"
    - **Matching reads your tags, not the audio.** Title and artist (or the filename, if you allow it) drive the search. Key, BPM, energy and cues are never used to pick a release.
    - **Similarity is a ranking, not a probability.** A score of 0.95 does not mean "95% likely correct".
    - **CueRithm stops when it is not sure.** A tie between candidates, or a disagreement in edition (for example a different mix), ISRC, album or release ID, leaves the track in Incoming for you to review. A provider outage is never treated as "no match".
    - To resolve a stopped track, use [Review and fix a wrong match](../tagging/review-a-match.md). If nothing matches at all, see [Catalog tagging finds nothing](../help/troubleshooting/no-catalog-match.md).

## Which catalogs are searched { #providers }

The matching engine and the first two providers come from [OneTagger](../help/about/credits.md), with CueRithm changes listed in the credits. Beatport and Traxsource are the default selection. MusicBrainz, SonoVault, Discogs, iTunes, BPM Supreme, Bandcamp, Deezer and Volumo are opt-in; you can reorder the providers you select. With **Combine providers** off, the first provider that gives a clear match supplies the metadata. With it on, every selected provider is consulted. See [Choose providers and fields to write](../tagging/choose-providers.md).

| | Beatport | Traxsource |
|---|---|---|
| Described in the app as | "Overall more specialized in Techno, can match using ISRC" | "Overall more specialized in House" |
| Account needed | No | No |
| Concurrency | Up to your **Tagging concurrency** (default 16) | One request at a time |
| Search | Catalog search API, 50 results per page; **Max pages** pages are scanned | Site search, HTML result list parsed |

Beatport access uses an anonymous client-credentials token. No account or password is stored. The token is kept in memory and refreshed shortly before it expires. Traxsource is a web page and may need a [Cloudflare solver](../tagging/cloudflare-solver.md). The opt-in providers that need a key or account are set up in [Connect provider accounts](../tagging/provider-accounts.md).

## Building the query { #building-the-query }

1. **By embedded ID** (Beatport, only with **Match by embedded IDs**): a stored `BEATPORT_TRACK_ID` fetches the track directly, with similarity 1.0 and reason *id*.
2. **By ISRC** (same switch): searches the ISRC and takes the first hit, with similarity 1.0 and reason *isrc*.
3. **By text:** `<first artist> <cleaned title>`. Artists come from the tag, or from the filename if **Read artist and title from filename** is on and the tags are missing. Beatport rejects searches with more than one pair of parentheses, so parentheses, brackets and commas are replaced with spaces in the query.

## How titles are cleaned for comparison { #normalising }

Titles are cleaned before comparison:

- lower-case, dashes to spaces, collapsed spaces, a leading "a", "an" or "the" dropped;
- a trailing "(original mix)", "(original version)" or "(original edit)" removed;
- 23 listed attributes removed, such as "(intro)", "(clean)", "(dirty)", "(extended)", "(radio edit)", "(club version)" and "(instrumental)";
- "feat." and "ft." clauses removed;
- for the actual comparison, the word "edit" and special characters are also removed and the text is transliterated to ASCII.

Artists are cleaned the same way and compared as sorted sets.

## How candidates are scored { #scoring }

1. **Exact.** The file's title is tried through seven progressively stronger cleaning steps. If one equals the cleaned catalog title and at least one artist matches, the candidate scores 1.0.
2. **Fuzzy.** Otherwise, for each candidate whose artist matches, the score is the normalised Levenshtein similarity of the cleaned titles. It is kept if it is at or above **Matching strictness** (default 0.70).
3. **Artist match** is true if any cleaned artist is equal, or one joined artist string contains the other's name, or the joined artists are similar enough by the same measure.
4. **Duration** (only if **Match duration** is on): rejects candidates more than **Maximum duration difference** seconds (default 30) from the decoded track length. A zero or unknown duration passes.
5. **Ordering.** **Best match** sorts by score; **Oldest release** and **Newest release** sort by date.

## When CueRithm stops and asks { #when-it-stops }

After scoring, CueRithm compares the best candidate with what your file already says. It stops, and keeps the track in Incoming, when:

- under **Best match**, the top two candidates are within 0.01 of each other;
- the candidate's mix or version text differs from the edition in your tags or title, for example "Extended Mix" against "Radio Edit";
- the ISRC, album or provider release ID disagrees with a value already in your file;
- **Match duration** is on and the lengths differ by more than your limit;
- two candidates look like different recordings of the same title.

Up to 10 candidates are kept for review. Review shows your file's metadata beside each candidate, and names the conflict. Choosing **Use selected match**, **Keep existing once** or **Requery** is yours. If **Identify unresolved tracks** is on, a recognised recording also stays in Incoming for edition review. See [Identify a track with no match](../tagging/identify-unknown-tracks.md).

## Album matching { #album-matching }

With **Album matching** on, when enough Incoming files in one folder resolve to the same release, that release's track list is fetched once and used as the candidate pool for the rest of the folder. The share of files that must agree is **Album matching ratio** (default 0.5). See [Match a folder as a release](../tagging/match-a-release.md).

## What each outcome means { #outcomes }

| Status | Meaning |
|---|---|
| `matched` | One clear candidate cleared the threshold and conflicts. |
| `no_match` | None did. What happens next follows **No catalog match**: *Keep existing tags and finish*, or *Keep in Incoming for attention*. |
| `already_tagged` | **Skip previously tagged catalog lookups** is on and the file carries a marker. Analysis and cues still run. |
| `ambiguous` | Ties or conflicts, as above. The track waits for review. |
| `unavailable` | A provider failed: rate limit, timeout, block or a page that could not be read. The track waits for a retry. |

An empty Traxsource result list is a no-match, but a missing result list or a malformed row is an error, so a changed or blocked page leaves the track in Incoming instead of silently skipping the provider.

## What happens on a slow or failing network { #network-behaviour }

Each provider stage has a deadline of 95 seconds (longer when a solver is configured). Rate-limit, timeout and connection failures are tried up to three times with short growing waits. Beatport and Traxsource requests time out after 30 seconds, follow at most five redirects, and fetched pages are capped at 8 MiB. Provider request budgets are shared across libraries and survive restarts. Quitting the app cancels waiting provider calls. Offline, lookups fail, the job goes to *attention* and is retried when you save **Settings** or press retry.

!!! warning "Live status"
    Beatport has one successful native probe. Direct Traxsource requests returned HTTP 403 on the validation machine; a read-only probe through a solver fetched and parsed live search, track and release metadata. This does not establish universal catalog coverage or end-to-end tag writes.

## Where you see it in the app { #where-you-see-it }

| Place | What shows |
|---|---|
| **Settings → Automatic tagging** | Providers and their order, **Matching** options (strictness, concurrency, duration, IDs, album matching) and **No catalog match** |
| **Incoming** | *Matching catalog*, then *Preparing tags*; a warning icon when a track needs review |
| Track row menu | **Review match**, with candidates, conflicts and the *Name similarity* note |

## Related

- [Tag new tracks from Beatport and Traxsource](../tagging/tag-new-tracks-automatically.md)
- [Review and fix a wrong match](../tagging/review-a-match.md)
- [Get past a Cloudflare block](../tagging/cloudflare-solver.md)
- [How are tags written safely?](tag-writing.md)

<!-- verified: vendor/onetagger/onetagger-tagger/src/lib.rs (MatchingUtils: clean steps 1-7, 23 attributes, exact fallback, normalised Levenshtein, artist match, duration, sorting; defaults strictness 0.7, threads 16, match duration off, max difference 30, album ratio 0.5), vendor/onetagger/providers/beatport.rs and traxsource.rs (ID, ISRC, query, 50 per page, max pages, descriptions, token), crates/cuerithm-autotag/src/lib.rs (MatchStatus, ambiguity rules, 0.01 tie, Unavailable, recognition fallback), evidence.rs (identity conflicts, 10 candidates), write.rs build() (ambiguous or unavailable stops; no-match policy), providers.rs (client 30 s, five redirects; match_by_id gate; options list), retry.rs (three attempts, 250 ms doubling), catalog_ports/mod.rs (shared query), config.rs defaults, apps/desktop TaggingSettings.tsx and MatchReview.tsx (labels), docs/automatic-tagging.md (providers, accounts, budgets), legacy technical/matching.md re-checked. Changed from legacy: opt-in providers, ambiguity stop, retries (legacy said no retry on 429) -->
