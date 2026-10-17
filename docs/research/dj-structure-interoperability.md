# Optional structural analysis and DJ interoperability

Research checked 2026-10-17. This is a documentation/source feasibility review,
not an application import test or a promise of supported exporters.

The user's proposed optional All-In-One analysis is useful independently of
whether another application can display the full structure. Preserve the full
section timeline and derive target-specific performance markers separately.
Section labels, section boundaries, beat grids and useful DJ cues require
separate quality assessments.

## Compatibility findings

| Target | Documented representation | External integration implication |
| --- | --- | --- |
| Rekordbox 7 | Editable native phrase regions; named hot/memory cues. The manual allows ten memory cues per track. | Documented XML includes named position marks and beat grids, but no phrase-region element. Start with named cues; native phrases need a separate adapter and validation. |
| Serato DJ Pro | Up to eight cues, with editable names and colors, saved in the audio file. | Names such as `Intro` and `Break 2` fit the UI. An application-specific metadata writer and file-format tests are still required; UI support does not establish an external import API. |
| Traktor Pro | Named stored cues, eight direct Hotcue buttons, and cue types with playback semantics. | NML import is documented. A generated NML cue adapter is a candidate, pending fixture and application round-trip tests. Do not equate eight buttons with a verified total stored-cue limit. |
| Engine DJ 5.1 | Eight named/colorable hot cues and eight saved loops. | Its documented third-party library imports offer a candidate route. Confirm preservation of cue name, color, position and slot per source and target version. No direct CamelotWheel integration is validated. |
| VirtualDJ | Named POIs, marker-only cues and an unlimited number of Remix Points. | A promising target for keeping many structural boundaries without spending the usual hotcue slots. Database POI fields are documented, but the vendor discourages external database writes; a safe import/adapter route remains to be tested. |
| Mixxx 2.5 | Named/colorable hotcues; native intro/outro ranges, each defined by two points. | Intro/outro predictions have a useful native representation. A whole-song chorus/verse region lane and an external range-import route were not established by this review. |

Primary references:

- Rekordbox [7.2.14 manual](https://cdn.rekordbox.com/files/20260409151936/rekordbox7.214_manual_EN.pdf), phrase editing pages 76/136, memory cues pages 95/156, XML import pages 19/45; [published XML schema](https://cdn.rekordbox.com/files/20200410160904/xml_format_list.pdf). The older schema lists only hotcue A-C; use current app fixtures to validate extended slots rather than treating that old enumeration as a current limit.
- Serato [cue count and storage](https://support.serato.com/hc/en-us/articles/226518228-Cue-Points), [names and colors](https://support.serato.com/hc/en-us/articles/233435487-Visual-Aid-Cue-Point-Naming-Color-Status).
- Traktor [cue management](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/advanced-usage-tutorials), [NML import](https://docs.native-instruments.com/ni-tech-manuals/traktor-pro-manual/en/working-with-playlists).
- Engine DJ [5.1 user guide](https://cdn.inmusicbrands.com/engine/51/Engine%20DJ%20-%20User%20Guide%20-%20v5.1.0.pdf), performance pads and importing libraries chapters.
- VirtualDJ [POI editor](https://virtualdj.com/manuals/virtualdj/editors/poieditor.html), [database schema and vendor caution](https://virtualdj.com/wiki/VDJ_database.html).
- Mixxx [hotcue labels/colors](https://mixxx.org/news/2020-08-25-new-in-2-3-hotcue-colors/), [2.5 intro/outro ranges](https://manual.mixxx.org/2.5/en/chapters/djing_with_mixxx.html#intro-and-outro-cues).

## Rekordbox native phrases: separate research track

The maintainer's [pyrekordbox analysis-format documentation](https://pyrekordbox.readthedocs.io/en/stable/formats/anlz.html#pssi-song-structure-tag)
describes a PSSI song-structure tag in EXT analysis files. Entries use beat
positions and a mood-dependent phrase vocabulary. This is evidence of a
structured representation, not an arbitrary text-label field or a supported
public phrase-import API. The [project README](https://github.com/dylanljones/pyrekordbox)
still describes general ANLZ creation/modification as planned. Thus PSSI makes
native integration plausible, but does not prove that writing it alone will
update desktop Rekordbox, survive reanalysis, or work on every player.

## Approved direction and proposed add-on behavior

The user approved the optional All-In-One direction on 2026-10-17. The detailed
export policies below remain recommendations, not implemented behavior:

1. Make deep structure analysis optional, offline and cached by audio identity,
   model version and analysis configuration. Retain beats, downbeats and every
   section with its start/end time and original predicted label.
2. Keep raw predictions distinct from reviewed labels and from exported cues.
   A model boundary score is not calibrated confidence in the musical label.
   Do not silently rename every `chorus` as `drop`, or a `break` as a proven
   safe mixing passage.
3. Display all regions in CamelotWheel's eventual timeline. For limited targets,
   select useful entry, exit and transition cues, retain numbered section names,
   and report which sections cannot fit. Preserve the full analysis internally.
4. Prefer passive named markers by default. Traktor Fade/Load cues, VirtualDJ
   Action/Automix points, saved/active loops and Mixxx Auto DJ ranges can affect
   playback. Assign those semantics only through a deliberate export policy.
5. Keep user-edited cues authoritative. Review conflicts before replacing them;
   prove merge behavior and idempotence using disposable libraries and audio
   copies before touching real libraries or tags.

The first real All-In-One pilot predicted 22 sections while the experimental
Rust selector emitted eight cues. This illustrates why full structure storage
must not be constrained to the exporter's slot count; it does not establish
that those 22 labels are musically correct.

## Current CamelotWheel boundary

`crates/camelot-core/src/rekordbox.rs` already renders escaped cue names as XML
`POSITION_MARK` elements, with at most eight chronological hotcues. It does not
export memory cues, colors or phrase regions, and it has no live DJ-app import
validation. The All-In-One neural reference remains a private Python research
pipeline; its Rust cue selector is separate from production integration.

Next interoperability validation should use a small fixed fixture containing
repeated labels, Unicode, more than eight sections, exact boundaries and existing
user cues. For each app/version, import into a disposable library, reopen and
export again; compare timing, names, colors, slots, ranges, existing-cue
preservation and reanalysis behavior. Hardware visibility requires its own test.
