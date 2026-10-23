# Catalog tagging finds nothing for a track

<p class="lead">If a track comes out of preparation without tags from Beatport or Traxsource, or waits at <strong>Matching catalog</strong>, the row and the Settings page tell you why. Work through this page in about ten minutes.</p>

## Why it happens { #why }

Automatic tagging is off until you switch it on, and it needs the internet. When it is on, CueRithm searches the providers you chose, in the order you set, and accepts a match only if the title, artist and length are close enough. Seven things can leave you without tags:

| What you see | Cause |
|---|---|
| The track finished with no tag status icon | **Automatic tagging** is off in **Settings** → **Automatic tagging**, or no provider is ticked. |
| The icon reads *No catalog match; existing tags kept* | The providers had no close enough result. The track is ready, with its old tags. |
| The row stays in Incoming with `No catalog match; review required` | Same result, but **No catalog match** is set to **Keep in Incoming for attention**. |
| `Catalog identity or availability requires review` | Several releases fit, or a provider could not answer. Open **Review match**. |
| `Source requires a FlareSolverr-compatible solver; configure it in Settings → Automatic tagging`, or the provider reads **Unavailable · configure a solver** | The site (usually Traxsource) answered with a Cloudflare browser check. |
| The row waits at **Matching catalog**, or ends in a network error | You are offline, the provider is slow, or it is rate limiting. CueRithm spaces its requests, and Traxsource runs one request at a time. |
| The icon reads *Already tagged* and nothing changed | **Skip previously tagged catalog lookups** recognised an earlier OneTagger or CueRithm tagging marker, so the lookup was skipped. Analysis and cues still ran. |

A provider failure is never reported as "no match". It keeps the track in Incoming so you can retry.

## Fix it { #fix }

1. **Check the switch and the providers.** Open **Settings** → **Automatic tagging**. Turn on **Automatic tagging**, tick at least one provider (**Use Beatport**, **Use Traxsource**), and choose **Save settings**. Saving also retries tracks that need attention. [Tag new tracks from Beatport and Traxsource](../../tagging/tag-new-tracks-automatically.md) walks through the whole setup, and [Choose providers and fields to write](../../tagging/choose-providers.md) explains each option.
2. **Provider needs an account.** SonoVault, BPM Supreme and Volumo stay unavailable until you connect them. See [Connect provider accounts](../../tagging/provider-accounts.md).
3. **Cloudflare block.** Traxsource may need a FlareSolverr-compatible solver that you run yourself. Enter its address under **Solver API URL**, save, and retry the track. If a site was marked as blocked and you have since fixed the network, choose **Retry direct source access**. [Get past a Cloudflare block](../../tagging/cloudflare-solver.md) has the full setup.
4. **A real no-match.** The catalog may not hold the track, or the file's title and artist may differ too much from the catalog's. Try these, one at a time:
    - Correct the title or artist with **Edit metadata** ([Edit tags and artwork](../../tagging/edit-tags-by-hand.md)), then choose **Review match** and **Requery**.
    - Lower **Matching strictness** a little. Higher values demand a closer title and artist match.
    - Turn off **Match duration**, or raise **Maximum duration difference**, if an extended mix has a different length in the catalog.
    - Turn on **Read artist and title from filename** when the tags are empty.
    - For a track with no usable tags at all, see [Identify a track with no match](../../tagging/identify-unknown-tracks.md).
5. **Ambiguous match.** Click the warning icon on the row to open **Review match**, compare the proposed fields, and pick the right release ([Review and fix a wrong match](../../tagging/review-a-match.md)). A folder of one release can be matched together ([Match a folder as a release](../../tagging/match-a-release.md)).
6. **Look up a finished track again.** After changing settings, choose **Analyze again** on the track's actions menu. It repeats the whole preparation with your current settings, including the catalog lookup.

## Check it worked { #check }

- The track's status icon reads *Tagged from catalog match*, and the title, artist, label or genre now carry the catalog values you chose to write.
- The track left Incoming and appears in **Tracks**.
- If you used **Review match**, the preview showed the fields it applied.

## Still stuck? { #still-stuck }

Note the exact row text, the provider order, and whether **Solver API URL** is set. Then [report the problem](../report-a-problem.md). Do not include credentials or API tokens.

## Related

- [Tag new tracks from Beatport and Traxsource](../../tagging/tag-new-tracks-automatically.md)
- [Get past a Cloudflare block](../../tagging/cloudflare-solver.md)
- [Settings: Automatic tagging](../../reference/settings/automatic-tagging.md)
- [A track is stuck in Incoming](incoming-stuck.md)

<!-- verified: apps/desktop/src/views/TaggingSettings.tsx (labels: Automatic tagging, Use <provider>, Solver API URL, Retry direct source access, No catalog match options, Matching strictness, Match duration, Maximum duration difference, Read artist and title from filename, Skip previously tagged catalog lookups, "Unavailable · configure a solver", "One request at a time"); TrackTable.tsx (tag status icon titles); crates/cuerithm-autotag/src/write.rs ("No catalog match; review required", "Catalog identity or availability requires review"); vendor/onetagger/providers/http.rs ("Source requires a FlareSolverr-compatible solver; configure it in Settings → Automatic tagging", is_challenge); crates/cuerithm-library/src/catalog_access.rs (remembered restriction, reset); settings.rs (autotag off by default); commands.rs save_settings -> retry_attention; prepare.rs (provider error -> attention); Analyze again = reprepare; legacy guides/catalog-solver.md re-checked -->
