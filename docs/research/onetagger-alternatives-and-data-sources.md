---
title: Current alternatives and large DJ data sources
date: 2026-10-20
status: planning-evidence
---

# Current alternatives and large DJ data sources

**Approved decision update:** implement MusicBrainz and optional personal-key SonoVault in PR5 of the [five-PR delivery plan](../plans/2026-10-20-002-feat-library-preparation-five-pr-delivery-plan.md). CueRithm requires no SonoVault account; only users choosing that provider enter a key. The user approved proceeding with that interpretation after the section 9 concern was raised. Musicfetch is excluded: its official current offer is a seven-day card-backed trial and paid plans, not an ongoing free plan. The broader inventory below remains research, not an adapter commitment.

There are useful replacements for most of the *capabilities* exposed by OneTagger. There is no verified one-for-one replacement for its old Spotify Audio Features workflow that also supplies unrestricted bulk access, exact DJ-edition matching, all-platform offline analysis and permanent tagging rights.

The recommended combination is CueRithm's own measured analysis and durable preparation; beets/Picard/Kid3/Mp3tag/Lexicon ideas for library workflows; open recording/release identity where appropriate; and a separately accepted commercial source when its DJ coverage and data contract justify it. This conclusion is a planning assessment from current documentation, not a benchmark of those products.

## Research coverage and evidence

Checked 2026-10-20. The detailed reports retain primary official links, current release evidence, fields, access, pricing snapshots, technical limits and unresolved contract questions:

- [Alternative tagging/library software](onetagger-alternative-software.md): beets, Picard, Mp3tag Windows/macOS, TagScanner, Lexicon, Mixed In Key and Kid3.
- [Fifteen catalog and DJ data sources](onetagger-alternative-data-sources.md): open catalogs/artwork, approved developer/partner routes, commercial aggregators, taxonomy and DJ-play context.
- [Analysis and recognition alternatives](onetagger-alternative-analysis-recognition.md): Essentia, ReccoBeats, AcousticBrainz, GetSongBPM, Chromaprint/AcoustID, AudD, ACRCloud and ShazamKit, with separate local-analysis and recording-identification roles.
- [User-suggested recognition API comparison](recognition-api-candidates.md): Music Identify, Shazam Core, API Dojo and AudD, with current free allowances and endpoint input requirements.

No accounts, paid subscriptions, API credentials, audio uploads or bulk datasets were used. Public documentation proves a documented product/contract surface, not uptime, exact-edition accuracy or permission for a proposed CueRithm deployment. Prices and service terms can change; the dated report is not a purchase quote.

## Gap-to-alternative decisions

| Broken or missing capability | Current alternatives worth evaluating | Decision for CueRithm |
|---|---|---|
| Automatic tagging with uncertainty/retry | **beets** quiet/resumable import and candidate review; **Picard** recording/release matching and old/new field review | Implement the workflow concepts in current Rust services. Do not require Python or a running GUI. Beets has a real CLI; Picard's new CLI utility is not a headless tagger. |
| Unsafe rename/move/copy and incomplete preview | **beets**, **Picard**, **Mp3tag**, **TagScanner**, **Kid3** | Use explicit modes, complete plan, collision handling, final metadata, references and recovery. Other tools' own path/cross-volume bug fixes are regression lessons, not proof their executors can replace CueRithm's coordinator. |
| Missing bulk manual tag/artwork editing | **Mp3tag**, **Kid3**, **Picard** | Common typed fields and independent-reader validation first; advanced native records later. Closed editors are design/interoperability references, not embeddable engines. |
| Fast human DJ categories and library-aware changes | **Lexicon** custom tags, recipes and native-library workflow; Quick Tag's useful interaction concepts | One Curate inspector and shared category model in the table/player workspace. Keep human rating distinct from measured energy. |
| Wrong releases or insufficient identity data | **MusicBrainz**, **Discogs**, **Music Story**, **SonoVault** | Prioritize recording/release IDs, ISRC, version and date provenance. An ISRC or canonical track does not always prove the purchased mix/master/edition. |
| Broken store scrapers / limited source coverage | Official **Beatport** partner route; optional personal-account **SonoVault**; licensed **Music Story/Soundcharts** | Implement the selected SonoVault adapter with individual technical acceptance before considering more fragile store adapters. Retain visible Traxsource under the shared persisted detected-Cloudflare/optional-solver policy. A configured endpoint is not proof of health; unavailable runtime services remain explicit failures. A source's advertised store coverage is not direct API permission or verified field quality. |
| Missing artwork | **Cover Art Archive** after release matching; contracted artwork modules where approved | Match artwork to the release; store source/image identity and preserve rights boundaries. An image URL or CC0 identifier does not license the image itself. |
| Unavailable Spotify feature descriptors | Local **Essentia** research; documented **Soundcharts** descriptor endpoints; **ReccoBeats** experiment | Preserve CueRithm key/BPM/energy/cues. Optional descriptors are sourced suggestions with separate quality/licensing gates, not replacement measured truth or a required Incoming stage. |
| Broken unofficial Shazam recognition | **Chromaprint → AcoustID → MusicBrainz**, **AudD**, **ACRCloud**; ShazamKit only where supported/cleared | Optional identification adapter with edition review and explicit privacy/cost budget. Recognition is not acoustic analysis and cannot supply an exact beatgrid. |
| Large reference dataset / genre consistency | **MusicBrainz** core dumps; separately verified **Discogs** dump channel; curated **Wikidata** subset | Use a licensed lookup/cache or controlled offline reference service. Do not ship enormous catalogs with every desktop or force a crowd taxonomy over the user's tags. |
| DJ context beyond generic popularity | **Songstats** documented 1001Tracklists/Beatport/Traxsource coverage; **Viberate/Chartmetric** if a specific need justifies them | Optional discovery/context. Confirm exact track/edition/appearance fields; artist popularity does not describe a track's mix suitability or set role. |
| Repeatable automation and independent format validation | **beets** CLI, **Kid3** CLI/JSON commands; opt-in **Lexicon Local API** interoperability | Later headless CueRithm commands should call the same library services as the UI. Do not import another unauthenticated server or duplicate library authority. |

## Recommended source shortlist

### Open foundation: MusicBrainz and Cover Art Archive

MusicBrainz supplies recording/release/artist identities, credits, ISRC and release information; its core metadata has a CC0 channel and database dumps. Supplementary genre/tag data has different terms. Cover Art Archive connects images to release MBIDs, but image rights are separate. Use this pair for identity and release-aware enrichment, not as an acoustic-feature service. Public API pacing and commercial service arrangements must be honored. [MusicBrainz API](https://musicbrainz.org/doc/MusicBrainz_API), [data licences](https://musicbrainz.org/doc/About/Data_License), [Cover Art Archive API](https://musicbrainz.org/doc/Cover_Art_Archive/API).

Discogs remains valuable for electronic back catalogs, original releases, formats, catalog numbers and styles. Current API terms and bulk-dump terms are separate channels; the current dump index and exact licence snapshot were not accessible in this research. Do not convert that observation into either blanket permanent-use permission or a claim that dumps no longer exist. Confirm the intended channel before building a local mirror. [API terms](https://support.discogs.com/hc/en-us/articles/360009334593-API-Terms-of-Use), [data index](https://data.discogs.com/).

### Selected DJ-focused provider: optional personal-key SonoVault

**SonoVault is particularly relevant** because current Lexicon enrichment uses it and OneTagger issue #530 requests integration. Its official material advertises more than 93 million recordings and identity/date/label/genre/subgenre/cross-platform data; that is a vendor scale claim, not demonstrated coverage of extended mixes, bootlegs or the user's library. It offers bulk lookup and recognition. It does not supply artwork, mood, energy or charts in the documented core field surface. [About/data origin](https://sonovault.now/about), [API](https://sonovault.now/docs), [pricing](https://sonovault.now/pricing).

The contract has important implementation consequences. It expressly permits permanent end-user audio tag writes for a named set of factual fields, but that list does not establish the same permission for BPM/key/platform IDs, indefinite application-database retention or native DJ-database export. Application-data retention depends on the paid plan and termination rules. Standard keys are intended for controlled infrastructure, and the terms prohibit using end-user keys to multiply quotas. Its representative release/merged canonical track must not silently replace a purchased edition. [Terms, especially sections 7 and 9](https://sonovault.now/terms).

The user selected optional personal-key SonoVault and approved proceeding after reviewing the section 9 concern. Implement that design without an application-wide key requirement, shared key or hosted broker. Keep technical/source-capability acceptance and independent local preparation; this records the product decision, not vendor clearance.

**Musicfetch is excluded.** Its documented offer is a seven-day card-backed trial followed by paid plans; no ongoing free API tier was established. Keep its coverage in the research inventory only; no trial or adapter is planned. [Pricing](https://musicfetch.io/#pricing).

### Larger contracted enrichment: Music Story and Soundcharts

Music Story is oriented toward catalog identities, versions, credits and full/incremental delivery. Soundcharts documents song metadata and descriptor endpoints plus batch/feed infrastructure. These are stronger candidates for a serious catalog-enrichment agreement than inferring a bulk API from a consumer dashboard. Require exact field entitlements, edition matching, permitted permanent tags/exports, retention and budget. Source breadth and descriptor meaning remain trial questions. [Music Story documentation](https://doc.music-story.com/), [Soundcharts developer product](https://developers.soundcharts.com/).

For specifically DJ context, Songstats' maintained developer materials name relevant platforms. That is worth investigating before paying for generic audience analytics. API access/fields/retention are enterprise questions; the MIT licence of an API example collection does not license the underlying data. [Official developer portal](https://developers.stats.company/), [official API collection](https://github.com/Songstats/songstats-api).

## What cannot be treated as a working drop-in

- **Spotify Audio Features:** OneTagger's current source explicitly disables it. Spotify's access changes and 2026 response-model changes are different problems; making `popularity` optional does not restore Audio Features permission. [2024 change](https://developer.spotify.com/blog/2024-11-27-changes-to-the-web-api), [2026 migration](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide).
- **AcousticBrainz:** still useful as a historical MBID-indexed CC0 lookup/research dataset, but data collection ended in 2022. It cannot analyze a new unknown DJ edit or guarantee current releases. [Project](https://acousticbrainz.org/), [owner's explanation](https://musicbrainz.wordpress.com/2022/02/16/acousticbrainz-making-a-hard-decision-to-end-the-project/).
- **ReccoBeats:** an actual documented API, including upload analysis, but upload limits are 5 MB and the first 30 seconds, rates are unspecified and no service guarantee was established. It is an experiment, not an unlimited full-track unattended stage. [Extraction contract](https://reccobeats.com/docs/documentation/Analysis/audio-features-extraction), [rate policy](https://reccobeats.com/docs/documentation/rate-limiting).
- **Essentia models:** technically promising for local genre/mood/instrument suggestions, but library/dependency and model-weight terms are separate. Official model licensing pages and the model LICENSE conflict. Pin the actual artifact and obtain clarification before distribution; do not replace validated CueRithm DSP in this feature program. [Library/model licensing](https://essentia.upf.edu/licensing_information.html), [models](https://essentia.upf.edu/models.html), [model licence](https://essentia.upf.edu/models/LICENSE).
- **ShazamKit:** an official recognition route, but not a common Windows/Linux/macOS backend; platform and persistent-output rights need their own acceptance. It is not permission to copy undocumented Shazam endpoints. [Apple ShazamKit](https://developer.apple.com/shazamkit/).
- **Websites and affiliate pages:** browser-visible data, an affiliate link, a vendor's login portal or an open API client library does not establish a full catalog feed or integration licence. Juno/Bandcamp/Beatport routes are documented at different access levels in the detailed report. No challenge/rate-limit bypass is proposed.

## Large-library architecture consequences

Treat **catalog identity**, **optional semantic descriptors** and **DJ context** as separate provider capabilities. A value must carry its source/entity/version, retrieval time, exact-edition evidence and allowed persistence/export policy. Keep original release date, edition release date and recording date distinct; preserve date precision instead of inventing January 1.

Separate query rate, monthly quota, endpoint credit cost, concurrency, batch size and retry budget. As arithmetic floors, 100,000 one-call MusicBrainz queries take at least 27.8 hours at one request/second; a six-request/minute subscription needs roughly 11.6 continuous days. These are not benchmarks and omit retries/additional release lookups. Cache/deduplicate approved identities, query only missing/uncertain fields and expose estimated work/cost before a large optional enrichment run. A library with 100k files is not a reason to download every catalog dump or secretly launch paid requests.

Keep byte/PCM-based preparation independent of service availability. A provider outage should preserve completed local analysis and existing metadata, with a clear attention or configured optional-enrichment outcome. Uploaded recognition is opt-in and distinguishes an audio excerpt from a locally computed fingerprint. No audio upload or account purchase is part of this planning work.

## Practical implementation sequence

1. Deliver current local value: safe naming/organization, truthful field outcomes, review/edit/Curate and preservation/recovery.
2. Add release-aware open identity support and a service agreement suitable for the intended distribution/use. Use beets/Picard/Kid3 as design and independent-fixture references, not required user installations.
3. Implement the approved optional personal-key **SonoVault** adapter with a signup link, native secret storage and bounded technical acceptance. Exclude Musicfetch. Keep Music Story/Soundcharts and Songstats as separately accepted research alternatives.
4. Pilot fingerprint recognition under the appropriate AcoustID agreement, or a paid AudD/ACRCloud option, only when missing-metadata coverage warrants it. Retain edition review, source provenance, consent and budgets.
5. Evaluate licensed local semantic models or a descriptor API separately; expose useful suggestions only after DJ-specific quality tests, with abstention and user control. Do not turn unavailable Spotify-style scores into the core product objective.

This sequence preserves Traxsource code and honest detected-source availability and the primary goal: a track enters Incoming and leaves with reliable local analysis, useful accepted tags and a consistent name, with human intervention reserved for real exceptions.
