---
title: User-suggested recognition APIs
date: 2026-10-20
status: acceptance-candidates
---

# User-suggested recognition APIs

These candidates extend PR5's identification research. The RapidAPI listings/pricing were inspected in a browser because their static pages expose little documentation. AudD's public site and API docs were read; its dashboard redirects to login. No account signup, subscription, key read, provider request or audio upload was performed.

The observed products identify recordings and return catalog context. No documented replacement for Spotify's danceability/valence/acousticness/instrumentalness descriptor vector was established. A Spotify result block is catalog metadata, not Audio Features permission or computation. Keep CueRithm's own measured analysis separate.

## Allowances and implementation fit

| Candidate | Current public allowance | Input and useful output | Assessment |
|---|---|---|---|
| **Eipiai Music Identify** | Basic $0: **600/month**, hard request limit, **1 request/second**. Marketplace bandwidth charges are shown separately. | Provider advertises compressed MP3 upload, title/artist/album/year and lyrics. One identify endpoint is listed. Detailed response, maximum clip and retention need acceptance. | Largest suggested recurring free request allowance; worth a bounded recognition trial. Do not infer uptime or edition accuracy from the listing. [Listing](https://rapidapi.com/eipiai-eipiai-default/api/music-identify), [pricing](https://rapidapi.com/eipiai-eipiai-default/api/music-identify/pricing). |
| **Tipsters Shazam Core** | Basic $0: **100/month**, hard limit; the page currently shows **1,000/hour**, not the paid tier's five/second. | Provider documents file recognition, search, details, related tracks, counts and media. The recognition tutorial contradicts itself about maximum upload size; use a small bounded excerpt and test the actual schema. | Useful fallback and DJ-context research; a small monthly allowance cannot cover an entire library. It is a third-party marketplace product; Apple operation/endorsement was not established. [Listing](https://rapidapi.com/tipsters/api/shazam-core), [pricing](https://rapidapi.com/tipsters/api/shazam-core/pricing). |
| **API Dojo Shazam** | Basic $0: **500/month**, hard limit, **5/second**. Separate marketplace bandwidth fee appears. | Current `songs/v3/detect` documents base64 **raw mono 44.1 kHz signed 16-bit little-endian PCM**, under 500 KB; compressed/RIFF files are not accepted by that endpoint. Search/details are separate calls. | Strong recurring-free candidate; reuse bundled decode to create the documented payload. Do not copy a deprecated detect endpoint or send an entire compressed track. [Listing](https://rapidapi.com/apidojo/api/shazam), [request contract](https://rapidapi.com/apidojo/api/shazam/playground/songs%2Fv3%2Fdetect), [pricing](https://rapidapi.com/apidojo/api/shazam/pricing). |
| **AudD direct** | **First 300 requests free on signup**, not 300 every month. Production plans are paid. | Standard recognition accepts a short file/URL with `api_token`; structured catalog results can include identifiers and optional service metadata. The current docs cap standard files at 10 MB. | Best immediate documented direct-service trial because the user already has a token. No intermediary RapidAPI subscription is needed. Apply an explicit free-credit/attempt budget; never silently proceed to paid usage. [Public offer](https://audd.io/), [API](https://docs.audd.io/), [dashboard](https://dashboard.audd.io/). |

The listed monthly allowances belong to separate API subscriptions; one RapidAPI account/key does not automatically grant every product. Request counts and response lookups can differ. Confirm active plan, endpoint costs, quota reset, overage and bandwidth before activation; do not present a marketplace free request allowance as a blanket zero-cost guarantee. No paid tier is selected by this research.

## PR5 decision and acceptance

Prioritize a small **AudD direct** trial, then compare **API Dojo** and **Music Identify** where their free plans can be activated by the user; retain Shazam Core as a fourth candidate. Keep local Chromaprint → AcoustID → MusicBrainz as the fingerprint-only alternative. Select network adapters individually from actual endpoint behavior and DJ-edition value; this request does not automatically enable four services or mandate a paid dependency.

The current bundled runtime decodes the required formats but does not include an MP3 encoder. Verify Music Identify accepts an available excerpt format before promising coverage, or plan an approved bundled encoding resource; users must not install a codec themselves.

Use metadata/ID matching first and recognition only for configured unresolved/readable-untagged cases. Pick bounded useful excerpts using bundled decoding, with explicit upload consent, one clear primary provider, cancellation, per-track retry/credit limits and visible no-match/unavailable/quota-exhausted states. A returned song identifier is candidate evidence; review Club/Dub/Extended/Radio edition differences against duration/IDs and accepted metadata. Do not apply the recognized recording's labels or cues as measured audio predictions.

Use native credential storage for personal AudD/RapidAPI keys, with the same no-key-return/redaction/revocation rules as SonoVault. Send RapidAPI keys only to the selected approved host in authentication headers; send AudD tokens in a native POST body rather than URLs/logs. Never embed credentials in the frontend or portable presets. The user supplied the main checkout’s `.audd` as a private development token location; a bounded native smoke harness may read the explicitly selected file without copying/echoing it. The installed app and CI must not auto-import developer key files. No key was read during this research.

The acceptance matrix must record: authenticated transport; known public-sample match; deliberate no-match; wrong/remixed edition; quota headers and hard-cap behavior; request/payload encoding and bounds; cancellation/deadlines; missing/revoked/rotated credentials; malformed/partial/HTTP-200 error bodies; useful returned fields, provenance and permitted persistence/export. Preserve raw user music and per-track evaluation privately. Do not use provider service-level badges or vendor accuracy claims as reproduced results.

A trial is accepted only for the capability demonstrated: recognition, catalog enrichment, artwork or contextual counts. Similar/related tracks and total recognitions are not mood/energy analysis. AudD's documentation samples include historical Spotify fields; their presence does not prove those fields remain available on every live response.
