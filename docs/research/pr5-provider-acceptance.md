---
title: PR5 provider and identification acceptance
date: 2026-10-21
status: implementation-evidence
---

# PR5 provider and identification acceptance

This supersedes the earlier provider-health assumptions in the [feature evaluation](onetagger-feature-evaluation.md). The [complete OneTagger issue ledger](onetagger-issue-audit.md) remains the historical disposition record. Pinned source, local fixtures, a bounded live response and a packaged application test are separate evidence layers. These checks establish no catalog-wide accuracy or DJ-hardware compatibility claim.

## Catalog delivery and observations

All added catalog providers are opt-in. Existing Beatport/Traxsource selections are preserved. Credentials are optional for CueRithm itself; only choosing a capability requiring personal authentication prompts for it.

| Provider | Implementation and bounded observation | Remaining limit |
|---|---|---|
| Beatport | Existing adapter retained; physical auth/API starts now use the shared scheduler. Token refresh is coalesced. | Upstream embed identity and endpoint changes remain an operational dependency. |
| Traxsource | Existing code and UI retained. Explicit Cloudflare detection persists unavailable status until a compatible global solver is configured. Artwork retains the required fixed site Referer. | Solver configuration alone does not prove current provider health. |
| MusicBrainz | Owned recording/release/track IDs, ordered credits, partial dates and release-bound Cover Art Archive. Native public sample returned ten review candidates. | Recording agreement does not prove an edition. No exact-edition accuracy claim. |
| SonoVault | Optional native personal key; missing/invalid/email/mock/quota states; search, ISRC and identity-bound enrichment. Explicit genres probe accepted the developer key and consumed one metadata request. | Search accuracy and sustained allowance were not established by that probe. No paid recognition. Source-native export policy excludes platform IDs, artwork and provider DSP. |
| Discogs | Accountless search/release mapping and raw track positions. Native public sample returned six review candidates. Shared ceiling 25/min anonymously; 60 only after current account verification. | Developer file did not match the accepted token format; no live authenticated request was sent. Verified tier and revocation are fixture-tested. |
| Volumo | Optional personal token, documented ISRC resolver and identity-bound public page metadata; known Volumo track IDs also supported. Native acceptance returned one identity-matched candidate with title, artists, release and artwork URL using one API HTTP200 plus one anonymous public metadata request. | No undocumented artist/title API search. Live resolver tracking parameters are removed before public metadata fetch or persistence. |
| iTunes | Search/lookup with recording and collection identity. Native public sample returned one match. | This is catalog metadata, not Apple Music account integration. |
| Deezer | Search, ISRC direct lookup and selected-album identity checks. Native public sample returned one match. | No streaming/account integration or comprehensive catalog accuracy claim. |
| Bandcamp | Repaired public JSON autocomplete plus authoritative page credits; native public sample returned one match. | HTML/endpoint changes and explicit Cloudflare restrictions remain actionable failures. |
| BPM Supreme | Personal-account vault/session setup and edition-aware mapping; missing editions do not fabricate a base track. | No personal account was supplied for live login/catalog acceptance. Local fixtures do not establish current account access. |

Beatsource is excluded by the user's final scope decision: the [operator now redirects users to Beatport](https://www.beatsource.com/). No registry entry, setup row, backend or disabled placeholder ships. Historical issue/source references are retained as research. Spotify is separately excluded by user decision. Musicfetch is excluded; no free plan was established. Musixmatch lyrics remain outside this metadata program.

Juno Download is also excluded by explicit user decision after failed native DNS access. Its closure is [reported by DnB Doctor](https://dnbdoctor.com/news/junodownload-is-shutting-down), a label that distributed through the store. This is distinguishable from a current operator announcement or successful endpoint response.

## Recognition acceptance

The selectable implementation is local PCM → statically linked Rust Chromaprint → AcoustID → MusicBrainz edition review. It defaults off and requires explicit fingerprint transmission consent. No external fingerprint executable, Python runtime or raw-audio upload is required. The registered application lookup key is supplied privately at build time, not requested from end users. Releases pass the same application version into the MusicBrainz User-Agent as the packaged desktop.

A generated 30-second signal produced the same compressed fingerprint as the reference Chromaprint executable and returned a valid AcoustID no-match using the registered key. This proves the local fingerprint/transport/key path, not known-song recognition accuracy. Short audio cannot satisfy the minimum 12-second fingerprint requirement. Alternative recording identities share the bounded candidate budget; none becomes an exact edition automatically.

The user authorized exactly one AudD request using AudD's public demonstration URL. That request succeeded with title/artist/MusicBrainz metadata. No personal audio was sent, no subscription was created, and the allowance must not be retried automatically. A URL demonstration is not acceptance of a production file-upload adapter. AudD and the three researched RapidAPI offerings remain absent from selectable product adapters. See [recognition candidates](recognition-api-candidates.md) for their independent access and quota decisions.

## Reliability and preservation repairs

The issue ledger informed concrete repairs: native identifier types and read-after-write checks; no fabricated date day; album/recording identity validation; no invented BPM Supreme edition; Bandcamp credit resolution before fuzzy rejection; cache/coalescing, bounded bodies/redirects, cancellable actual-start pacing, and distinct unavailable versus no-match outcomes. SonoVault payload permissions cannot grant native-write permissions. Recognition exceptions are bound to source hash, recording identity and captured consent, rather than globally enabling MusicBrainz.

The native vault namespace and rate/cache authority are application-wide across UI, manual review, Incoming and headless processes. Unknown/busy authority blocks network work while offline library operations remain usable. The [automation interface](../automation.md) reuses library ownership, previews and recovery journals. Reports exclude source payloads and credentials.

Private acceptance inputs/results remain outside Git. No user library was processed for these provider checks. Native window lifecycle, real OS-vault integration on all three platforms, signed installers, full known-song recognition accuracy and physical DJ playback are not established by local tests.

Volumo acceptance used three API metadata requests in total: an initial native URL-validation failure, one bounded diagnostic HTTP200, and the final successful native path. A separate unsupported development-file label failed before any request. Nine synthetic Volumo mapping/transport/canonical-URL tests pass. The successful sample establishes that path, not catalog-wide coverage or artwork-image download acceptance.
