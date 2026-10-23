# About CueRithm

<p class="lead">CueRithm is an independent, open-source desktop library app for DJs. It is written in Rust with a Tauri and React interface, and it runs on macOS, Windows and Linux. This page gives the project facts and points to the pages about its origins, its licences and its limits.</p>

## Where to read more { #pages }

| Your question | Page |
|---|---|
| Whose work does CueRithm build on, and under which licences? | [Credits and open source](credits.md) |
| What role did Mixed In Key and Lexicon DJ play, and what is and is not claimed? | [How CueRithm was made](provenance.md) |
| Which DJ software has been tested, and how far? | [Check what works with your DJ software](../compatibility.md) |
| What is missing, limited or in development? | [What CueRithm does not do yet](../not-available-yet.md) |
| Where are the common questions answered? | [Frequently asked questions](../faq.md) |
| How do I report a problem or ask for a feature? | [Report a problem or ask for a feature](../report-a-problem.md) |

## Project facts { #project-facts }

- **Source:** [github.com/johnneerdael/CueRithm](https://github.com/johnneerdael/CueRithm).
- **Name and tagline:** the app is called CueRithm. The tagline is *Know your music. Shape your set.*
- **Licences:** the analysis engine is MIT. The desktop application is GPL-3.0-only, because it includes OneTagger code. Details are in [How the licences fit together](credits.md#licences).
- **Platforms:** macOS on Apple Silicon (signed and notarized), Windows on x86-64 and Linux on x86-64. The Windows and Linux builds are unsigned, and there is no Intel Mac build ([My computer blocks CueRithm at first launch](../troubleshooting/first-launch-blocked.md)).
- **Status:** version 0.1, early software. Releases are published automatically on GitHub, and CueRithm does not update itself.
- **Your music:** stays where it is. Analysis runs on your computer, nothing is uploaded, and there is no account ([Frequently asked questions](../faq.md)).

CueRithm is not affiliated with, endorsed by or sponsored by Mixed In Key, Lexicon, Beatport, Traxsource, rekordbox, OneTagger or any other product named in this guide.

## Related

- [Credits and open source](credits.md)
- [How CueRithm was made](provenance.md)
- [Download and install CueRithm](../../get-started/install.md)

<!-- verified: README.md (intro, Install table, Licence, Credits, tagline); apps/desktop/src-tauri/Cargo.toml and Cargo.toml (GPL-3.0-only desktop, MIT workspace); .github/workflows/release.yml (automatic releases, signed macOS only); mkdocs.yml (copyright line); no updater plugin in Cargo.toml/tauri.conf.json; legacy about/index.md re-checked -->
