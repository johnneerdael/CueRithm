# Get past a Cloudflare block

<p class="lead">Traxsource sometimes shows visitors a Cloudflare check that CueRithm cannot pass by itself. A small helper program called a solver passes it for CueRithm. You only need one if Traxsource is greyed out. Allow about fifteen minutes the first time, most of it spent installing the helper.</p>

**Before you start**, make sure tagging works with the providers that need nothing extra by following [Tag new tracks from Beatport and Traxsource](tag-new-tracks-automatically.md). Beatport does not use a solver at all. Analysis, playback, cleanup and cues never need one either.

## Do you need a solver? { #do-you-need-a-solver }

You need one when you see any of these:

- In **Settings → Automatic tagging**, **Traxsource** reads *Unavailable · configure a solver* and its **Use Traxsource** box cannot be ticked.
- In [Review match](review-a-match.md), the **Catalog access** section lists Traxsource as *Unavailable* with the reason *Unavailable: configure a FlareSolverr-compatible solver in Automatic tagging settings*.
- A track waits in Incoming with a message that a source requires a solver.

CueRithm first tries each source directly. When a reply carries Cloudflare's challenge header, or is clearly a challenge page, CueRithm marks that source as needing a solver and remembers it, even after you restart the app. A plain 403 error, a site that merely uses Cloudflare, or an embedded Turnstile box does not count.

CueRithm does not supply, host or start a solver. You run one yourself, on your computer, a NAS or a server you trust.

## Which services use a solver { #supported-sources }

| Source | Uses the solver? |
|---|---|
| Traxsource | Yes. Search results, track pages and release pages all go through it once the source is marked. |
| Volumo | Its public track pages can, after the ISRC lookup. |
| Bandcamp | Track pages can, but its search is a request type the solver cannot repeat. A solver does not unblock Bandcamp; leave it unticked while it is blocked. |
| Beatport | No. It keeps its direct catalog connection. |
| MusicBrainz, SonoVault, Discogs, iTunes, BPM Supreme, Deezer | No |

Cover images are always downloaded straight from your computer, never through the solver.

## Choose and run a solver { #choose-a-service }

Pick one of these and follow the installation instructions on its own page:

| Service | Project |
|---|---|
| FlareSolverr | [FlareSolverr/FlareSolverr](https://github.com/FlareSolverr/FlareSolverr) |
| Byparr | [ThePhaseless/Byparr](https://github.com/ThePhaseless/Byparr) |
| Trawl | [germondai/trawl](https://github.com/germondai/trawl) |

CueRithm talks to the common FlareSolverr interface: a `POST` to `/v1` with `request.get`. Live fetching and parsing of Traxsource search, track and release pages were checked with Trawl. FlareSolverr and Byparr use the same interface but have not been tried live with CueRithm, and whether a solver succeeds depends on the site, the solver's version and the solver's own network connection. Coverage of the Beatport and Traxsource catalogs is not measured either, so a track they lack still ends as "no match".

## Point CueRithm at it { #configure-cuerithm }

<!-- figure:tag-solver-settings -->
<figure markdown>
![The Cloudflare challenge solver group with the Solver API URL box showing the placeholder http://localhost:8191, Solver timeout 120 and the Retry direct source access button](../assets/screens/tag-solver-settings-light.webp#only-light){ .screen loading=lazy }
![The Cloudflare challenge solver group with the Solver API URL box showing the placeholder http://localhost:8191, Solver timeout 120 and the Retry direct source access button](../assets/screens/tag-solver-settings-dark.webp#only-dark){ .screen loading=lazy }
<figcaption>Enter the solver's address and keep the timeout at 120 seconds to start.</figcaption>
</figure>
<!-- /figure -->

1. Open **Settings → Automatic tagging** and scroll to **Cloudflare challenge solver**.
2. Enter the address in **Solver API URL**. On the same computer that is often `http://localhost:8191`. For a NAS, use its address on your network and the port the solver exposes. A full endpoint such as `https://solver.example/v1` works too; CueRithm adds `/v1` when it is missing. The URL must use HTTP or HTTPS and must not contain a username, a password, a query string or a fragment.
3. Leave **Solver timeout** at 120 seconds. The first challenge takes much longer than later ones. You can raise it to 300 seconds.
4. Tick **Use Traxsource**, check its position in the list, and choose **Save settings**.

Saving settings retries every track in Incoming that was waiting for attention, so affected tracks start again with the new setting. The field is empty by default, and the solver must be reachable from the computer running CueRithm. It must also be able to reach Traxsource itself. You need no browser extension, system proxy or extra certificate.

!!! tip "Check the status after saving"
    Once a solver URL is entered, *Unavailable · configure a solver* disappears and **Use Traxsource** can be ticked. CueRithm only contacts the solver when it needs it, so a wrong address shows up on the first lookup, not at save time.

## Source availability and retrying direct access { #source-availability }

- **With a solver**, requests for a marked source go to it from then on.
- **Without one**, the source is unavailable and cannot be newly ticked. A selection you already saved stays visible so you can clear it. A lookup that needs the source fails at once and leaves the track in Incoming; it is never reported as "no match".

**Retry direct source access** forgets every remembered restriction. Use it after the site or your network changes. The next lookup tries a direct connection again, may detect a challenge again, and does not change which providers you ticked.

## Timeouts and errors { #timeouts-and-errors }

CueRithm retries a solver timeout or a temporary outage once, after a short pause. That makes at most two attempts per page, with no endless loop and no fallback to repeated direct requests. A single Traxsource lookup can need pages for search, track and release, so with the default 120-second timeout the worst case is 878 seconds before the track is flagged. Stopping CueRithm cancels a request in progress.

The solver keeps its own browser sessions and clearance cache. CueRithm uses the page the solver returns and rejects one that still shows a challenge. A "success" from the solver is not enough on its own: the page must also be one CueRithm can read.

| Symptom | Cause | Fix |
|---|---|---|
| *Unavailable · configure a solver* does not go away | **Solver API URL** is empty or not a valid HTTP or HTTPS address | Enter it again and **Save settings** |
| The track waits with a solver error | The service is not running, is unreachable or timed out | Start it, open its logs, check it can load the Traxsource site, then retry the row |
| *Solver returned an unresolved Cloudflare challenge* | The solver could not pass the check | Update the solver or try another one from the table above |
| *Solver redirected outside the catalog origin* | The solver ended on another site | Check the solver's configuration |
| A genuine empty Traxsource search | The catalog has no such track | This follows **No catalog match**; nothing is broken |

If a hostname you created recently does not resolve, wait for DNS to catch up or use an address on your network. [Catalog tagging finds nothing](../help/troubleshooting/no-catalog-match.md) covers the other reasons a lookup returns nothing.

## Data sent to the service { #data-sent-to-the-service }

The solver receives the catalog page addresses CueRithm asks for, including the artist and title words of a search, and it may keep its own browser cookies and request history. It does not receive your audio files, local paths or your library database. Choose a service you trust, and never paste a username, password or API key into the URL.

## Related { #related }

- [Choose providers and fields to write](choose-providers.md#what-is-sent): everything CueRithm sends, by provider.
- [Connect provider accounts](provider-accounts.md): providers that need a key instead of a solver.
- [Review and fix a wrong match](review-a-match.md): see which providers were reachable for one track.
- [Automatic tagging settings](../reference/settings/automatic-tagging.md)

<!-- verified: apps/desktop/src/views/TaggingSettings.tsx (solver fields, labels, notes); apps/desktop/src/test/tagging-settings.test.tsx; vendor/onetagger/providers/http.rs (SolverConfig validation, is_challenge, two attempts, origin check); crates/cuerithm-autotag/src/providers.rs (878 s test); crates/cuerithm-autotag/src/catalog_ports/mod.rs (Bandcamp POST not replayable); crates/cuerithm-autotag/src/volumo.rs (public page via solver); crates/cuerithm-library/src/match_review/store.rs (review access reason); apps/desktop/src-tauri/src/commands.rs save_settings (retry_attention); legacy guides/catalog-solver.md and about/roadmap.md (known limitations) re-checked -->
