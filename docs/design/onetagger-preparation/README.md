# Preparation design concepts

Open `index.html` in a browser. It is a self-contained proposal with four scenarios: Incoming naming settings, match review, Curate and existing-file organization. The top controls switch scenarios and light/dark themes. The naming input illustrates examples; it is not the proposed production template validator.

No library, filesystem, network or Tauri operation runs. All tracks, paths and waveform data are invented. The shell uses snapshots of CueRithm's own fonts, mark and design tokens, with font notices embedded in the HTML. The visual proposal is not evidence of implemented functionality or provider health.

The implementation plan defines state, persistence, failure and safety contracts. The current table/sidebar/player hierarchy and unnumbered Settings navigation remain the design foundation.

Captured examples: [Incoming naming](incoming-naming-dark.png), [match review](match-review-dark.png), [Curate dark](curate-dark.png), [Curate light](curate-light.png) and [organization blockers](organize-light.png). These illustrate proposed functionality; the interactive HTML remains the primary design artifact.

## PR2 implementation captures

The implemented React screens use the production `{title}` grammar and existing CueRithm fonts, tokens and player. The older proposal's percent-token examples are illustrative only. The default production rule is `{artist} - {title}` so a final native title already containing a mix name is not duplicated.

Browser verification used a temporary synthetic bridge with 53 invented tracks, complete counts, a destination collision, a linked-target blocker, recovery history and a changed copy original. It did not invoke native commands or touch user music. This proves UI behavior and layout separately from native file/reference integration.

Normal light/dark captures appear in the [organization guide](../../user-guide/rename/organize-files.md). Compact captures: [Organize](organization-compact-implementation.jpg) and [Settings](settings-compact-implementation.jpg). A 490×340 CSS viewport tests the layout equivalent of a 980×680 window at 200% zoom; this was responsive viewport emulation, not a native browser zoom or OS accessibility test. The measured main/player right edges stayed within 490 pixels, and keyboard focus reached Preview, Apply, Discard, the pattern input and naming tokens. Both main views scroll their headings and content at that size. The persistent player retains transport, time and volume controls.

Preview pagination reached rows 51–53 while complete totals remained 53 tracks with 49 eligible, one unchanged, one skipped and two blocked. UI fixture results are not evidence of successful native reference repair or physical DJ player acceptance.
