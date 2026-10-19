---
title: CueRithm user guide
hide:
  - navigation
  - toc
---

<div class="cr-hero" markdown>

![CueRithm](assets/brand/cuerithm-logo-on-light-descriptor.svg#only-light){ .cr-hero__logo }
![CueRithm](assets/brand/cuerithm-logo-on-dark-descriptor.svg#only-dark){ .cr-hero__logo }

# Know your music. Shape your set.

CueRithm is a desktop library app for DJs. Drop in your tracks and it listens to every one of them, offline: musical key, BPM and beatgrid, a single energy rating, structural cues and a colour waveform. It then helps you tag, clean and organise the library, with playlists and smart playlists that update themselves.

[Get started](get-started/index.md){ .md-button .md-button--primary }
[Install CueRithm](get-started/install.md){ .md-button }
[How it works](technical/index.md){ .md-button }

</div>

<figure markdown>
  ![CueRithm showing a track library with key, BPM and energy columns above a colour waveform with five labelled cues](assets/screens/40-player.webp){ .screen loading=lazy }
  <figcaption>The CueRithm library: sidebar, track table and the persistent player with beatgrid, waveform and cues.</figcaption>
</figure>

## What CueRithm does

<div class="grid cards" markdown>

-   :material-waveform: **Listens to your music**

    Key, BPM, one whole-track energy rating from 1 to 10, and five or six structural cues per track. Everything is computed on your computer from the decoded audio. No uploads, no accounts.

    [:octicons-arrow-right-24: Under the hood](technical/index.md)

-   :material-tray-arrow-down: **Prepares tracks automatically**

    Watch a folder, drop files on the window or use **Add tracks**. Every track passes through Incoming: decode, analyse, optionally tag and clean up, then it moves into your library. Your files stay where they are.

    [:octicons-arrow-right-24: Your first tracks](learn/first-tracks.md)

-   :material-tag-multiple: **Tags and cleans**

    Optional catalog tagging from Beatport and Traxsource, and a review-then-apply cleanup workspace with Undo, so messy titles, genres and artists never need hand editing.

    [:octicons-arrow-right-24: Cleanup tutorial](learn/cleanup.md)

-   :material-playlist-music: **Playlists that keep up**

    Ordinary playlists, folders and smart playlists with rules over BPM, key, energy, genre, dates, cues and more. Smart playlists re-evaluate in the background.

    [:octicons-arrow-right-24: Smart playlists](learn/smart-playlist.md)

</div>

## Honest by design

- **Offline analysis.** Analysis and cue generation never touch the network. Only the optional catalog tagging does.
- **Your files are safe.** CueRithm never moves your audio. When it writes tags it stages a copy, proves the playable audio is unchanged, keeps the original, and swaps atomically.
- **Clear about what is missing.** Direct rekordbox sync and the duplicate and file-maintenance screens are not available in this release. See [Not available yet](about/roadmap.md).
- **Open source and credited.** CueRithm builds on [OneTagger](about/credits.md#onetagger), FFmpeg, TagLib and many others. It was inspired by Mixed In Key and Lexicon DJ, studied through reverse engineering, and written independently in Rust. Read [how CueRithm was made](about/provenance.md).

!!! info "Status: version 0.1"
    CueRithm is early software. macOS builds for Apple Silicon are signed with a Developer ID and notarized by Apple. Windows and Linux builds are unsigned. There is no Intel Mac build.

## Where to go next

| If you want to... | Read |
| --- | --- |
| Install and open the app | [Install](get-started/install.md) |
| Understand the window | [The main window](get-started/main-window.md) |
| Follow a hands-on tutorial | [Learn](learn/index.md) |
| Look up a control or setting | [Guides](guides/index.md) |
| Know exactly how a number is computed | [Under the hood](technical/index.md) |
| See licences, credits and provenance | [About](about/index.md) |
