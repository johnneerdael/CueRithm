# Match a reference tracklist to owned music

Status: approved home-DJ roadmap delivery; implementation decisions recorded
2026-10-21 under the user's instruction to finish the approved work.

## Outcome

Paste a tracklist, or choose a local text/CSV file, review which recordings in
CueRithm could match each entry, and create an ordinary ordered playlist from
explicit selections. Keep unmatched and ambiguous entries visible. Metadata
similarity never establishes that the audio or remix is the desired recording.

## Flow and decisions

Use a dedicated matcher view following existing workspace styling. Provide input,
format settings, parsed results and candidate choices, then a playlist name and
Save action. Keep results and playlist creation separate. Nothing is selected
automatically, including a unique same-tag candidate. Offer playback of candidates
before choosing them. Changing input/options clears prior results/selections.

Text mode defaults to `Artist - Title` using the first ` - ` separator. Offer a
swap-artist/title option. A line with no separator is a title-only query. Skip
blank lines; reject rows with an empty title. Preserve source order and row numbers.

CSV uses a standards-compliant parser, including quoted commas/newlines and
escaped quotes. With headers enabled, locate explicit `artist` and `title`
columns case-insensitively; with headers disabled, use the first two columns.
Show the parsed artist/title before allowing a playlist save. Reject malformed
records instead of silently shifting columns. Header-only input has no matches.

Reject input over 1 MiB, more than 1,000 entries, and fields over 4,096 bytes.
Match prepared, nonarchived local library tracks only. Cap a matching invocation
at 10 million candidate comparisons; exceedance is an actionable error, never a
partial successful list. Use an indexed candidate pool or bounded complete scan.

Normalize case, Unicode composition, punctuation and repeated whitespace while
retaining remix/version words. Exact mode compares the resulting artist/title
values; title-only inputs compare only titles. Fuzzy mode may offer conservative
character-trigram candidates, explicitly labelled similar tags. Define/test its
thresholds and deterministic ranking in the implementation. Do not strip version
labels or infer recording identity from a fuzzy score. Return up to ten candidates
per row and disclose that bound.

An unselected row is skipped when saving. Display the matched/selected/skipped
counts, and retain an export/copyable unmatched list. Repeated selections of the
same library track produce one member, at its first input position, consistent
with ordinary playlist membership. Explain deduplication before saving.

Create the playlist and its ordered members in one existing revision-guarded
batch edit. Recheck selected IDs and readiness at save time. On concurrent changes,
retain the review and require a fresh library revision; never leave an empty
playlist after failed membership publication. Include existing playlist Undo.

## Boundaries and acceptance

Use backend-owned matching over one coherent library snapshot and a typed native
bridge. The view must discard late responses after input changes or unmount.
File input reads only the selected local text/CSV; no service login, streaming
audio or remote playlist dependency is introduced.

Tests cover Unicode and quoted input, remix ambiguity, same-tag duplicates,
title-only entries, malformed/oversized input, deterministic ranking, unknown
matches, stale selections, atomic playlist creation and playback/selection. Verify
rendered UI with synthetic metadata. Do not publish user tracklists or library paths.
