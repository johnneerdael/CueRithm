---
title: OneTagger capabilities for CueRithm library preparation
status: approved
date: 2026-10-20
---

# OneTagger capabilities for CueRithm library preparation

## Intent

Evaluate the installed OneTagger application and its complete available source feature set, then select capabilities that materially help manage a DJ library. Produce a grounded integration plan and visual proposals for CueRithm's existing design system. The user has approved the selected feature scope and requested a five-PR design/implementation plan; that approval supersedes the initial planning-only status. The later user instruction replaces the initial Traxsource UI restriction with main's optional solver and detected-source availability contract.

The central workflow remains automatic Incoming preparation. Configure tagging and organization once; subsequent eligible tracks receive full analysis, matching, selected metadata writes, cleanup and optional renaming/organization before promotion into Tracks. Successful tracks must not require a per-import wizard or approval. Exceptions should be understandable and recoverable.

## Requirements from the user

- **R1 — Complete evaluation:** cover OneTagger's app artifact, source, routed tools, provider adapters, CLI and format layer. Distinguish the installed release from newer source and existing CueRithm adaptations.
- **R2 — Value selection:** recommend features because of concrete DJ-library value. Identify existing functionality, missing UI over existing backends, genuinely new capabilities, conditional additions and features that should not be imported.
- **R3 — Incoming first:** integrate automatic tagging and organization into the existing preparation lifecycle and persistent configuration. Manual tools should complement that workflow.
- **R4 — Renaming:** include filename templates, naming previews and optional folder organization. Evaluate move/copy behavior and collisions rather than equating template parsing with a safe renaming implementation.
- **R5 — Existing design:** preserve CueRithm's current sidebar, metadata table, persistent player and Settings experience. Extend the actual cobalt/coral tokens and light/dark themes.
- **R6 — Issue history:** evaluate every upstream issue relevant to these decisions, including closed reports and their discussions. Explain what is broken, repaired, already adapted, unverified or not applicable. Do not infer a fix from issue closure.
- **R7 — Traxsource:** retain provider code and schema/configuration compatibility. Show provider selection/options/capabilities. Allow initial/reset direct access; persist detected Cloudflare restrictions across restart. A blocked source is unavailable without a configured compatible solver, while an existing selected source remains deselectable. Endpoint presence is not a service-health guarantee; runtime failures must remain honest unavailable outcomes.
- **R8 — Concrete deliverables:** deliver a feature matrix, complete issue disposition ledger, prioritized implementation plan with file/test seams, and visual design examples.
- **R9 — Current alternatives and DJ data:** for broken or missing capabilities, research usable open and commercial software, services and large metadata/data sources. Distinguish software inspiration/interoperation from reusable code or a licensed API/feed. Evaluate current availability, access, edition identity, coverage, cost model, data rights and practical integration fit.
- **R10 — Selected providers and credentials:** include MusicBrainz with a meaningful versioned/contactable User-Agent. SonoVault is optional; users who choose it enter a personal API key, while all other CueRithm functionality remains usable without an account. Provide Get a free API key → `https://sonovault.now/signup` and native secure storage. The developer key in the main checkout’s `.sonovault` is never bundled, committed or automatically imported by the app. Exclude Musicfetch. The user explicitly approved proceeding with the optional personal-account interpretation after the section 9 concern was raised; do not impose a vendor-approval hold or substitute a shared key/broker.
- **R11 — Five-PR delivery:** organize the approved scope into five sequential design/implementation PRs, retaining all U0–U11 contracts and individual provider/recognition acceptance. Scope approval does not mean those features are already implemented or shipped.
- **R12 — Recognition candidates:** evaluate Eipiai Music Identify, Tipsters Shazam Core, API Dojo Shazam and AudD direct (private development token in `.audd`) as PR5 alternatives. Distinguish recurring free allowances from initial trial credits, recording identification from audio descriptors, and documented endpoints from demonstrated behavior. Keep keys private and accept adapters individually.

- **R13 — Existing OneTagger metadata providers:** PR5 includes ports and validation of iTunes, BPM Supreme (personal account), Bandcamp and Deezer. Reuse existing upstream adapters and repair issue-audited defects. Per-provider technical acceptance does not require another scope approval; report specific current blockers. Keep Spotify Audio Features excluded.

## Existing product contracts carried forward

- Keep measured key, BPM, canonical beatgrid, structural cues and one whole-track energy rating independent of catalog metadata. Do not introduce energy labels on structural cues.
- Preserve all required analysis/playback formats: MP3, AAC/ADTS, AAC/M4A, ALAC/M4A, FLAC, WAV, AIFF, Ogg Vorbis, Opus and WMA. Native metadata capabilities need their own container-specific proof.
- Retain bundled decoders, source-change detection, staged writes, PCM/timeline verification, persistent recovery and stable track IDs.
- Keep original files in place by default. Physical renaming/organization is opt-in; changing this default requires a separate product decision.
- Keep direct DJ synchronization and playlist identities coherent after metadata/path changes. Do not add rekordbox XML transport.
- Preserve the current-version DJ sync scope in the existing sync specification. Lexicon's support for older software versions is explicitly excluded; this OneTagger plan does not add compatibility ports.
- Reuse selected OneTagger code with its existing provenance and licensing boundary; preserve the independent DSP engine's separation.

## Planning decisions and assumptions

1. Recommend extending CueRithm's existing library/preparation services rather than embedding a second OneTagger application or HTTP/WebSocket runtime.
2. Present uncertainty as an exception requiring attention, not as a mandatory review step for every good match. A similarity score is not a probability of correct recording/version identity.
3. Prefer rename-in-place as the first organization mode, followed by move/copy under an explicit managed root using the same durable plan. Include all modes in the design and acceptance criteria.
4. Do not offer destructive destination overwrite. Conflicts should block or use a configured deterministic suffix. No silent success on skipped moves.
5. Keep structured human categories independent of lossy comment-field encodings. Retain rating separately from measured energy.
6. Evaluate named preparation presets and per-folder assignment, but do not make those a prerequisite for adding organization to the existing global preparation settings.
7. Treat provider availability as an implementation acceptance gate, not as a fact proved by source code or an issue comment. No provider credentials or live library writes are needed for this evaluation.

The user approved the feature program and continuing implementation on 2026-10-20. Existing saved provider selections/custom options remain intact. The later solver instruction supersedes blanket hiding; shared detected-source availability must not silently migrate selections or claim a configured service is healthy. SonoVault remains an independently chosen personal-account provider, not an application access requirement.

## Evidence and deliverables

- Evaluation: `docs/research/onetagger-feature-evaluation.md`.
- All-issue ledger and limitations: `docs/research/onetagger-issue-audit.md` and its JSON companion.
- Current alternatives: `docs/research/onetagger-alternatives-and-data-sources.md`.
- Canonical delivery plan: `docs/plans/2026-10-20-002-feat-library-preparation-five-pr-delivery-plan.md`.
- Detailed technical contracts: `docs/plans/2026-10-20-001-feat-library-preparation-workflows-plan.md`.
- Visual proposal: `docs/design/onetagger-preparation/`.

No live-provider health, actual recording-match accuracy, metadata-write preservation on user music, or runtime behavior of the installed OneTagger app is claimed by static inspection. Those require the implementation acceptance work identified in the plan.

Implementation scope update (2026-10-21): the user excluded Beatsource after its merger into Beatport. No Beatsource provider, setup entry or placeholder is shipped. Spotify remains excluded by user decision.

Implementation scope update (2026-10-21): the user also excluded Juno Download after failed live access and reports of store closure. No Juno Download provider or disabled placeholder is shipped.
