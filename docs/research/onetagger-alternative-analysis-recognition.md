---
title: Current alternatives for audio analysis and recording identification
date: 2026-10-20
status: planning-evidence
---

# Current alternatives for audio analysis and recording identification

The user's subsequent recognition shortlist is evaluated in [Music Identify, Shazam Core, API Dojo and AudD](recognition-api-candidates.md), including verified recurring allowances, AudD's initial free credits and native payload differences. PR5 evaluates these individually alongside the fingerprint-only path.

Research date: **2026-10-20**. Official documentation, upstream repositories, licences and vendor terms were inspected. No account was created; no credentials, uploads, recognition/API calls, installed software changes or user audio were used. Public documentation availability is not measured service availability or DJ-library accuracy. Vendor catalog counts below are claims, not measured DJ/remix coverage. Prices/terms are the pages observed today and should be pinned again before implementation.

## Decision

Two separate capabilities must remain separate: **analysis of arbitrary local audio** (genre/mood/instrument suggestions, measured BPM/key/energy) and **recognition against a reference catalog** (candidate artist/title/IDs). A recognition match does not supply a trustworthy local beatgrid, establish the exact extended/clean/remastered edition, or recover every tag. Neither task requires restoring the unavailable OneTagger Spotify Audio Features stage.

- **Best open recognition candidate:** local Chromaprint → AcoustID → MusicBrainz, under the appropriate service agreement, with a reviewable identity confidence/release selection step and durable caching.
- **Practical paid cross-platform recognition candidate:** AudD or ACRCloud behind a typed optional adapter, with explicit upload consent, quotas/cost cap, request provenance and uncertain-edition handling. These are alternatives to broken undocumented Shazam calls, not feature-analysis engines.
- **Most credible local analysis research candidate:** Essentia plus a carefully selected/tagged model, initially private evaluation. It can process previously unknown audio offline, but licensing/model terms, platform packaging and DJ-specific quality require a separate gate. Do not change the already validated CueRithm key/BPM/cue/energy calculations as part of adding tagging.
- **ReccoBeats:** useful optional experiment, not a required unattended Incoming dependency at scale: public free endpoints exist, but quotas and durability guarantees are unspecified and uploaded analysis truncates after 30 seconds.
- **AcousticBrainz/GetSongBPM:** lookup enrichment only. They cannot analyze a new DJ edit or unlisted file; AcousticBrainz is a frozen historical dataset and GetSongBPM’s added features partly derive from it.

## Analysis options

### Essentia / Essentia.js — local audio analysis, conditional adoption

Essentia is a native C++ DSP/MIR library with bindings; Essentia.js compiles its backend to WebAssembly with JavaScript/TypeScript and optional TensorFlow.js inference. Both offline and real-time use are documented; the JS project warns of API evolution and incomplete manual testing. Native Rust orchestration/FFI or an isolated bundled helper is a plausible CueRithm route; WASM workers are another evaluation route, not proof all desktop webviews perform equivalently. No Python runtime need be imposed on users. [Overview](https://essentia.upf.edu/documentation.html), [official Essentia.js repository](https://github.com/MTG/essentia.js).

Models include genre/style, mood/theme and instruments, with separate embeddings/classifier heads. A concrete mood-theme head documents 56 outputs, 16kHz audio through a Discogs-EffNet embedding, and test ROC-AUC0.76/PR-AUC0.14 on its dataset. These numbers are **not DJ-mood accuracy** or a calibrated guarantee. A model output should initially be a sourced suggestion, with thresholds, abstention and user mapping to the existing taxonomy, not a replacement for user-assigned mood. [Model catalog](https://essentia.upf.edu/models.html), [specific mood-theme model metadata](https://essentia.upf.edu/models/classification-heads/mtg_jamendo_moodtheme/mtg_jamendo_moodtheme-discogs-effnet-1.json).

**Licensing requires an explicit decision.** The library is AGPLv3 with proprietary licensing available, and dependencies have their own terms. AGPL is not itself a noncommercial licence: its actual text permits charging for distribution and includes GPLv3 combination provisions; distribution/source obligations must be reviewed for the chosen integration. Do not infer that CueRithm being GPLv3 automatically clears every bundled component. [Library licensing](https://essentia.upf.edu/licensing_information.html), [actual library COPYING](https://raw.githubusercontent.com/MTG/essentia/master/COPYING.txt).

The official model licences are inconsistent: licensing page says **CC-BY-NC-ND4.0**, model catalog says **CC-BY-NC-SA4.0**, and actual directory LICENSE names NC-ND but links NC-SA legalcode and permits adaptations in its summary. The LICENSE was read with a plain HTTPS curl GET because browser tooling rejected its octet-stream MIME type. Thus pin exact weights, embeddings and notices; request written clarification/proprietary terms from UPF before distribution for commercial-capable DJ workflows. A permissive architecture implementation does not remove NC restrictions from trained weights. [Model LICENSE](https://essentia.upf.edu/models/LICENSE).

**Can fill today:** technically suitable for analyzing arbitrary new local files into candidate genre/mood/instrument labels; no per-track network quota or audio upload required. **Unproven:** chosen-model rights, resource budget, native multi-platform build, taxonomy precision on DJ edits, and usefulness beyond existing analysis. First deliverable should be a licensed offline benchmark with model hashes/preprocessing/version and a held-out DJ corpus, not a production feature switch.

### ReccoBeats — free remote lookup and short-upload feature extraction

Catalog endpoints include `/v1/track/search`, `/v1/track/:id/audio-features` and batch `/v1/audio-features`. These are catalog lookups, subject to identity and coverage gaps. Documentation advertises millions of tracks and no authentication. Do not rely on community claims about exact batch sizes or Spotify ID support without confirming its actual schema during adapter work. [Introduction](https://reccobeats.com/docs/documentation/introduction), [single features](https://reccobeats.com/docs/apis/get-track-audio-features), [batch features](https://reccobeats.com/docs/apis/get-audio-features), [search](https://reccobeats.com/docs/apis/search-tracks).

Upload endpoint `POST /v1/analysis/audio-features` accepts multipart audioFile up to **5MB**, supported MP3/OGG/Vorbis/AIFF/AIFC/WAV, and analyzes at most **30seconds**, truncating longer audio. It returns acousticness, danceability, energy, instrumentalness, liveness, loudness, speechiness, tempo and valence; the documented response does not include key, grid or structural cues. Suggested chunk/average handling is not evidence that averaged segment outputs equal calibrated whole-track predictions. Decoding to supported clips could cover other CueRithm formats, but that would be our own tested preprocessing contract. [Upload API](https://reccobeats.com/docs/documentation/Analysis/audio-features-extraction).

Rate limits are internally configured, with 429/Retry-After and possible blocking; no numeric sustained allowance is published. Terms updated May25,2026 allow free personal/commercial access, disclaim uptime/accuracy/security warranties and reserve IP blocking. They say foundational metadata derives from Spotify and callers must comply with relevant third-party terms, while analysis is ReccoBeats-generated. This is **not blanket permission to redistribute Spotify-derived metadata** or evidence of guaranteed scale. No upload-retention/privacy promise was located in the reviewed extraction/terms pages. [Rate limits](https://reccobeats.com/docs/documentation/rate-limiting), [terms](https://reccobeats.com/docs/documentation/terms-of-service).

**Verdict:** optional sourced experimental metadata only after retention/rights/volume terms are clarified; never silently upload a library or make unknown quota a required stage. Preserve CueRithm’s own whole-track energy and avoid importing excluded danceability/valence merely because a numeric endpoint exists.

### AcousticBrainz — historical lookup, not a restarted service

The current homepage explicitly says collection ended in2022 while website/API remain available, indexed by MusicBrainz recording MBID, with CC0 data. It reports about7.56million unique recordings and29.46million submissions; multiple submissions are not unique catalog coverage. Downloads are dated July6,2022. [Current homepage](https://acousticbrainz.org/), [data downloads](https://acousticbrainz.org/download).

MetaBrainz’s shutdown explanation says old key/BPM predictions and classifiers were unreliable across the full collection and lacked useful confidence. Current site availability does not mean new submissions restarted. It cannot fill arbitrary new-audio analysis or current release coverage; MBID-level results may not identify the exact local edition. At most use it as explicitly historical optional lookup/provenance or research baseline. [Owner explanation](https://musicbrainz.wordpress.com/2022/02/16/acousticbrainz-making-a-hard-decision-to-end-the-project/), [submission shutdown update](https://community.metabrainz.org/t/acousticbrainz-submissions-data-dumps-and-next-steps/589843).

### GetSongBPM — legitimate metadata lookup API, limited extra value

Official API exposes title/artist search and song/BPM/key lookup using a registered API key; it is free including commercial applications with mandatory backlink/attribution. Published limit is **3000requests/hour**, with a one-hour block for exceeding it. The changelog identifies danceability/acousticness additions as AcousticBrainz-derived. It does not accept arbitrary audio for analysis. This can supply a second sourced key/BPM opinion for catalog matches, but cannot replace local analysis or establish remix/edition correctness. Because CueRithm already measures key/BPM, prioritize only if measured coverage/quality justifies another provider. [Official API documentation](https://getsongbpm.com/api).

Do not confuse it with SongBPM: SongBPM’s current terms explicitly prohibit automated access/bulk scraping, so a browser-visible BPM value is not a licensed integration route. [SongBPM terms](https://songbpm.com/legal/terms).

## Recognition options

### Chromaprint + AcoustID + MusicBrainz — preferred open route

Chromaprint computes fingerprints locally from decoded PCM and supports near-identical full-file identification, not general robust recognition of arbitrary distorted excerpts. Feed existing bundled decoder output through its C API or bundle a pinned fpcalc; do not make users install runtimes. Fingerprint lookup transfers fingerprint/duration rather than source audio, but still reveals musical identity to a service. It cannot identify an unknown/unindexed recording or generate acoustic features. [Chromaprint docs](https://acoustid.org/chromaprint), [project README](https://raw.githubusercontent.com/acoustid/chromaprint/master/README.md).

AcoustID needs a registered application client key; submitting fingerprints requires separate user key and must remain opt-in. Public free use is noncommercial at **3requests/second** and significant traffic should be discussed with operator. Its commercial page lists Open Source free/3rps and paid plans starting **€50/month/1million searches**, with10k-search trial and higher tiers. Confirm the exact CueRithm distribution/user-use arrangement with operator rather than assuming GPL licensing grants unrestricted hosted-service usage. [Web-service contract](https://acoustid.org/webservice), [operator pricing](https://acoustid.biz/).

Chromaprint’s own code is MIT but its LICENSE says the combined library is LGPL2.1 due included FFmpeg code; selected FFT/backend dependencies affect final binary licence (FFTW implies GPL). Database fingerprint/metadata is CC-BY-SA3.0 except MusicBrainz mapping CC0. Pin build options and notices, including bundled decoder components. [Actual code licence](https://raw.githubusercontent.com/acoustid/chromaprint/master/LICENSE.md), [data/service licences](https://acoustid.org/license).

Use AcoustID candidates to obtain MBIDs, then MusicBrainz recording/release data for artist/title/release disambiguation. Read-only MusicBrainz queries need no key but require meaningful User-Agent and **at most1request/second**; noncommercial access is free, commercial use has separate plans. It is metadata, not a guaranteed recording recognizer or audio-feature source. [MusicBrainz API](https://musicbrainz.org/doc/MusicBrainz_API), [rate policy](https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting).

**Scale implication (arithmetic, not a benchmark):** 100k one-request AcoustID lookups require at least9.3hours at3rps; one MusicBrainz query each requires27.8hours at1rps, before network/retries/decoding and extra release queries. Deduplicate source hashes/MBIDs, batch where documented, cache results and schedule provider-specific work separately from75%-CPU analysis workers. Larger deployments need commercial agreements or properly operated mirrors, not bypassed public limits.

### AudD — paid cross-platform HTTP recognition

AudD accepts file/URL audio and returns candidate artist/title/album/date/label/timecode/link, with optional music-service/MusicBrainz metadata. Standard recognition analyzes the **first12seconds**, with10MB input limit; long/multi-song files use enterprise. No special OS runtime is required for HTTPS submission from Rust, but audio leaves the device. Vendor claims no standard-endpoint rate limit, asks advance notice for large volume, and has enterprise concurrency limits. Validate actual DJ edit coverage; response success with null result is no match. [API docs](https://docs.audd.io/), [official operational guide](https://audd.io/resources/agents/cli).

Current price page lists300trial requests/14days where available; Indie$5/month includes1000requests then$5/1000, Startup$450/100k, Pro$850/200k, with custom high-volume/on-premises quotes. Retries/multiple excerpts can increase per-track cost. ISRC/UPC/match-score enterprise access is on request, not guaranteed in every base result. [Pricing](https://audd.io/pricing/index.html).

Current terms allow storing results inside customer products but restrict competing recognition datasets/models and standalone redistribution; third-party metadata rights remain caller responsibility. Customer must have rights/consents for uploads. Terms permit deidentified aggregate improvement, so do not assume zero training/retention from a marketing claim; obtain explicit applicable terms for unreleased/private audio. [API terms, July7,2026](https://audd.io/terms/index.php).

**Verdict:** practical paid recognition candidate, cost-capped and opt-in, then verify edition with metadata/duration/manual evidence. Not an Audio Features substitute.

### ACRCloud — paid recognition with local fingerprint upload option

Documented Identification API accepts audio or SDK fingerprint, HMAC-signed using project access key/secret and host; sample under5MB, short clips recommended. Local fingerprint generation can reduce transferred audio/bandwidth but adds proprietary SDK/build/distribution work. Vendor advertises150million-track reference catalog and ISRC/UPC/service links; this is a coverage claim, not a DJ-remix benchmark. [Identification reference](https://docs.acrcloud.com/reference/identification-api/identification-api), [service](https://www.acrcloud.com/music-recognition/).

Trial is normally14days or quota first; paid price/QPS/volume is plan/order dependent. The public pricing URL presently renders a general landing page rather than a numerical tariff. Request-count and QPS-exceeded codes are documented separately. Critically, current terms permit customer-facing product/service use only when expressly allowed by plan/order/docs: internal-business terms alone do not establish app redistribution rights. [Terms](https://www.acrcloud.com/terms/), [error codes](https://docs.acrcloud.com/sdk-reference/error-codes).

DPA currently dated October20,2026 says raw audio/video is transient and deleted after success/failure, with region selection and separate fingerprints/results/log retention rules; customer remains responsible for lawful inputs. Confirm actual order/region/SDK terms and do not generalize transient audio deletion to all derived data. [DPA](https://www.acrcloud.com/dpa/).

**Verdict:** credible contract-backed alternative for scaled recognition. Compare HTTP audio upload versus permitted bundled fingerprint SDK using a consented held-out DJ corpus; numeric cost/SLA/platform-build support remains procurement/acceptance work, not a claimed free replacement.

### Apple ShazamKit — legitimate macOS recognition, not a portable tag API

Apple officially supports ShazamKit on Apple platforms and Android; its list does not include Windows or Linux. It matches signatures against Shazam or custom catalogs, with title/artist/genre/timecode metadata. Apple states audio is not shared and signatures are noninvertible. Custom catalogs recognize audio already supplied by the developer; they do not identify a previously unknown file against a universal offline catalog. [ShazamKit](https://developer.apple.com/shazamkit/), [Android setup](https://developer.apple.com/shazamkit/android/).

Shazam catalog requires registered service entitlement/App ID; Android uses developer tokens/private-key setup. No public guaranteed bulk quota, all-desktop SDK or universal service SLA was established in reviewed docs. [Account setup](https://developer.apple.com/help/account/services/shazamkit).

More than platform work blocks automatic tag reuse: current Apple Developer agreement requires Apple Music links for displayed corresponding content and restricts copying/modifying/publishing ShazamKit Content except expressly permitted; it also prohibits using outputs to build another recognition service. These rules require a specific clearance for persistent local tag writing/export. Do not assume an SDK recognition match grants unrestricted catalog metadata redistribution. [Apple agreement, section E ShazamKit](https://developer.apple.com/support/terms/apple-developer-program-license-agreement/).

**Verdict:** potential optional native macOS candidate resolver after rights and bulk-use review, not the common Windows/Linux/macOS recognition backend and not a reason to reuse undocumented Shazam endpoints.

## Spotify baseline and integration consequences

Spotify’s November27,2024 change restricted new Web API access to Audio Features/Audio Analysis and related endpoints; grandfathered access is not a dependable shipping contract. Fixing optional popularity parsing in OneTagger does not restore its explicitly disabled Audio Features workflow. Do not scrape legacy websites or borrow somebody else’s tokens to recreate access. [Spotify announcement](https://developer.spotify.com/blog/2024-11-27-changes-to-the-web-api).

CueRithm should model optional identity providers and optional suggestion analyzers separately from existing measured analysis. Each result needs source hash, provider/model/version, preprocessing/excerpt interval, identity IDs, received date, confidence/abstention, candidate edition and per-field provenance. Provider errors/quota exhaustion must stay distinct from no match and must never overwrite valid prior tags/cues. Remote upload must be opt-in and transparent, with a cost ceiling and cancelable cooldown/retry; no hidden full-library scanning.

A worthwhile implementation sequence is: (1) shared manual candidate/editor and safe transform/organization contracts; (2) licensed AcoustID fingerprint identification pilot; (3) one paid recognition pilot if it materially improves DJ coverage; (4) separately licensed local genre/mood/instrument evaluation. ReccoBeats/historical lookup remain optional experiments until rights, accuracy and service budgets are settled. No service here proves accurate clean/explicit version, mix identity, original release date or beatgrid automatically; preserve manual correction and version-aware acceptance.
