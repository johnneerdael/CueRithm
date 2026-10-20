# Cloudflare challenge solver

Some catalog sites, including Traxsource, can require a browser challenge before
returning results. CueRithm can fetch those pages through a service that implements
the FlareSolverr API. The service is optional and runs separately from CueRithm.
Local analysis, playback, cleanup and cues do not need it.

## Choose a service

Install one of these on your computer, a NAS or a server you trust, following its
own setup instructions:

| Service | Project and installation instructions |
| --- | --- |
| FlareSolverr | [FlareSolverr/FlareSolverr](https://github.com/FlareSolverr/FlareSolverr) |
| Byparr | [ThePhaseless/Byparr](https://github.com/ThePhaseless/Byparr) |
| Trawl | [germondai/trawl](https://github.com/germondai/trawl) |

CueRithm uses the common `POST /v1` / `request.get` interface. Live fetching and
parsing have been checked with Trawl; FlareSolverr and Byparr use the same API
contract but have not been tested live with CueRithm. Success depends on the
site, service version and the service's network connection.

## Configure CueRithm

1. Open **Settings > Automatic tagging > Cloudflare challenge solver**.
2. Enter **Solver API URL**. On the same computer this is often
   `http://localhost:8191`; for a NAS, use its LAN address and exposed port.
   A full endpoint such as `https://solver.example/v1` also works. CueRithm adds
   `/v1` when it is missing, including after a reverse-proxy path prefix.
3. Leave **Solver timeout** at **120 seconds** initially. A first challenge can
   take much longer than a subsequent lookup. Increase it up to 300 seconds if
   needed. The HTTP connection allows ten additional seconds for the response.
4. Select **Traxsource**, arrange the provider order, and **Save settings**.
   Retry an affected Incoming job so it uses the newly saved settings.

The URL is empty by default. CueRithm does not supply a public solver or start
one automatically. Use an address reachable from the machine running CueRithm;
the service itself must be able to reach Traxsource. No browser extension,
system-wide proxy or extra certificate authority is needed.

## Source availability

CueRithm first tries direct access. When the shared catalog fetcher sees
Cloudflare's challenge header or an identifiable challenge page, it remembers
that the source requires a solver. This state survives app restarts.

- With a solver configured, the request is retried through that service. Later
  page requests for that source go straight to the solver.
- Without one, the source is shown as unavailable and cannot be newly selected.
  Previously saved selections remain intact; deselect the source or configure
  a solver. A required lookup fails promptly without further direct requests,
  keeping the track in Incoming rather than pretending there was no match.
- An ordinary 403, a site using Cloudflare, or an embedded Turnstile widget does
  not by itself mark a source as blocked.

**Retry direct source access** clears all remembered restrictions. Use it after
a site or network changes. The next lookup tries direct access again and can
detect a challenge again. It does not change your provider selections.

Only affected catalog page requests use the solver. Beatport keeps its existing
direct catalog API connection. The shared fetcher is reusable by future public
catalog adapters; it cannot automatically change an adapter that uses a different
network path. Artwork downloads are still fetched directly.

## Timeouts and errors

CueRithm retries a solver timeout or temporary unavailability once, after a short
delay. There are at most two solve attempts per page; there is no endless retry
loop or fallback to repeated direct requests. Cancellation stops an active
solver request and prevents retries.

The service owns its browser sessions and clearance cache. CueRithm consumes the
returned page, checks its target status and rejects unresolved challenges; it
does not copy browser clearance cookies to desktop requests. A service reporting
success alone is insufficient: the catalog parser must also recognize the page.

A genuine empty Traxsource search follows **No catalog match**. A timeout,
unresolved challenge or malformed page is an error, leaving the job in Incoming.

If lookups fail, check that the service URL is reachable, inspect its logs, and
confirm that it can open the catalog from its network. If the hostname was just
created, allow DNS changes to propagate or use a reachable LAN address.

## Data sent to the service

The configured service receives catalog URLs, including artist/title search
queries, and can retain its own browser cookies or request history. It does not
receive your audio files, local paths or CueRithm library database. Choose a
service you trust. Embedded URL credentials and query-string API keys are not
supported; do not paste them into this setting.
