# Look up a setting

<p class="lead">CueRithm has one Settings screen with nine sections. Find the section that holds the control you need, then open that section's page for every control, its default and its valid range. About two minutes.</p>

!!! tip "New to this?"
    You do not need to change anything to start. A new library already watches one folder and prepares every track with sensible defaults. To set up a folder like `~/Music/Beatport` step by step, follow [Keep a folder like Beatport in your library](../../files/watch-folders.md). To find a setting by what you want to do, use [Find the setting you need](../../customize/find-a-setting.md).

## Open the Settings screen { #open }

Choose **Settings** at the bottom of the sidebar, or press the sliders button (**Preparation settings**) at the top right of the library. **Library** at the top left takes you back.

The sections are listed down the left side of the screen, in the order below. Moving between sections keeps your unsaved changes.

## The nine sections { #sections }

| Section | What it controls | How it is saved | Reference page |
|---|---|---|---|
| **Incoming** | Which folders CueRithm watches, and which preparation preset each folder uses | **Save settings** | [Incoming](incoming.md) |
| **Cue generation** | Minimum breakdown length, drop at the start of a track, the emergency loop | **Save settings** | [Cue generation](cue-generation.md) |
| **Automatic tagging** | Beatport and Traxsource tagging, other providers, fields to write, fingerprint fallback, Cloudflare solver | **Save settings** (provider keys save on their own) | [Automatic tagging](automatic-tagging.md) |
| **Cleanup** | Cleaning titles, genres and artists on every new track | **Save settings** | [Cleanup](cleanup.md) |
| **File organization** | Renaming, moving or copying a file after preparation | **Save settings** | [File organization](file-organization.md) |
| **Preparation presets** | Named bundles of cue, tagging, cleanup and naming rules | Preset buttons act at once; **Load into settings draft** needs **Save settings** | [Incoming: preparation presets](incoming.md#preparation-presets) |
| **Library preferences** | Time zone for calendar rules, key compatibility mode | **Save library preferences** | [Library preferences](library.md) |
| **Smart Playlists** | Lists that CueRithm builds and maintains for you | **Save Smart Playlist settings** | [Smart Playlists](smart-playlists.md) |
| **Appearance** | Waveform style | Immediately, on this computer only | [Appearance](appearance.md) |

!!! note "Where Preparation presets is documented"
    Presets and folder assignments belong together, so the Preparation presets section is described on the [Incoming](incoming.md#preparation-presets) page.

## How settings are saved { #saving }

- **Save settings** appears at the top right of the first six sections. It reads **Saving…** while it works and **Settings saved** afterwards. One press saves every change you made in those six sections, including folder assignments.
- **Library preferences** and **Smart Playlists** have their own buttons and no **Save settings**. They save separately because they change playlists, not preparation.
- **Appearance** has no button. The waveform style applies at once and is remembered on this computer. The light and dark theme is not in Settings: it is the **Colour theme** button in the header ([Switch between light and dark](../../customize/theme.md)).
- Provider keys and accounts in **Automatic tagging** save to your system credential store the moment you press their own buttons. They never go into **Save settings** or into a preset.
- Saving also gives every track in Incoming that **needs attention** another try with the new settings, so a corrected rule can rescue a stuck track ([Fix a track that needs attention](../../files/needs-attention.md)).
- New settings apply to tracks prepared after you save. They do not change finished tracks. To apply them to one, choose **Analyze again** ([Analyze a track again](../../analysis/analyze-again.md)).

### When Save settings is unavailable { #save-blocked }

| You see | Why | What to do |
|---|---|---|
| **Save settings** is greyed out and a red banner names a Cleanup problem, such as "Use at most 64 Smart Fixes." | A cleanup rule is invalid. The banner repeats the message followed by "Review Cleanup settings." | Open [Cleanup](cleanup.md#limits), fix the rule, and the button returns. |
| **Save settings** is greyed out while File organization is set to rename, move or copy, and **Example filenames** shows an error or "Checking naming rules…" | The filename pattern or a policy is invalid, or CueRithm is still checking it | Open [File organization](file-organization.md#pattern) and correct the pattern; see [File naming tokens](../naming-tokens.md). |
| "Settings changed elsewhere. Your unsaved draft is retained. Reload before saving." | The saved settings changed after you started editing, for example in another window | Press **Discard settings draft and reload**, then make your change again. |
| A red banner with a message from CueRithm after you press **Save settings** | The settings did not pass validation | Read the message, correct that control, press **Save settings** again. |

## What is not a setting { #not-settings }

| You may be looking for | Where it really is |
|---|---|
| The wait that makes sure a file has finished copying | Fixed at 2 seconds. It is not adjustable. |
| Cue templates, a cue's name or colour, loops, the beatgrid | **Edit cues & grid** in the player: [Fix a wrong cue, loop or beatgrid](../../analysis/corrections/index.md) |
| Light, dark or system theme | The **Colour theme** button in the header |
| Sidebar width, player height, table columns | Saved on this computer as you change them: [Resize the sidebar and player](../../customize/layout.md), [Show the columns you need](../../analysis/choose-columns.md) |
| Sync destinations and options | **Sync** in the sidebar: [Send your library to DJ software](../../sync/index.md) |
| An analysis model, a tempo range or an audio output device | There are none. Analysis is always full-track, cues are always replaced after a successful analysis, and playback uses the system default output. |
| Provider passwords and keys | Stored in the system credential store from **Automatic tagging**. They are not part of settings files or presets. |

## Related

- [Find the setting you need](../../customize/find-a-setting.md) (by goal)
- [Keep a folder like Beatport in your library](../../files/watch-folders.md)
- [Save your setup as a preset](../../files/presets.md)
- [Reference: what are you looking up?](../index.md)

<!-- verified: apps/desktop/src/views/Settings.tsx (pages list, Save settings states, conflict banner, which pages hide Save); apps/desktop/src-tauri/src/commands.rs save_settings (retry_attention); crates/cuerithm-library/src/settings.rs (validate: full-track, replacement cues, stability 10-60000 ms, default 2000); apps/desktop/src/App.tsx (Preparation settings sliders button, Colour theme button); apps/desktop/src/test/settings-pages.test.tsx; legacy guides/settings.md re-checked -->
