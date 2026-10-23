# Report a problem or ask for a feature

<p class="lead">Problems and feature requests go to the CueRithm issue tracker on GitHub. A good report names your version, your system, the steps and the exact message, and leaves out your music and private paths. Writing one takes about five minutes.</p>

## Before you report { #before }

Check these first. They answer most questions and tell you whether you have found a fault or a gap:

1. [Fix a problem](troubleshooting/index.md) matches the symptom to a page with steps.
2. [Frequently asked questions](faq.md) covers formats, limits, privacy and data.
3. [What CueRithm does not do yet](not-available-yet.md) lists features that do not exist. If yours is there, a feature request is still welcome, but the behaviour is not a bug.
4. [Check what works with your DJ software](compatibility.md) shows what has been tested in each DJ program.

## Find your version { #find-version }

CueRithm has no About window or Help menu. The sidebar footer shows only the release series (`CUERITHM 0.1`). Use one of these for the full number:

- **The file you downloaded.** The version is in its name, for example `Cuerithm-arm64-0.1.14.dmg`, `Cuerithm-linux-x64-0.1.14.deb` or `Cuerithm-windows-x64-0.1.14.zip`.
- **The releases page.** The [latest release](https://github.com/johnneerdael/CueRithm/releases/latest) shows the newest number. Yours may be older.
- **The built-in check.** `cuerithm-desktop --diagnostics` prints a line of JSON that includes `"version"`. [Download and install CueRithm](../get-started/install.md) shows the command for each system.

Also note your operating system and its version, and on a Mac whether it has an Apple chip.

## Write the report { #write-report }

1. Open [github.com/johnneerdael/CueRithm/issues](https://github.com/johnneerdael/CueRithm/issues) and search for your problem. If an issue already exists, add your details to it instead of opening a new one.
2. Choose **New issue**. You need a GitHub account.
3. Give it a title that names the symptom, such as "Preview shows a dash for tracks in a watch folder".
4. Fill in the body. This template covers what is usually needed:

    ```text
    CueRithm version:
    Operating system and version:
    What I wanted to do:
    Steps:
    1.
    2.
    3.
    What I expected:
    What happened (copy any message exactly):
    File type of the track (FLAC, MP3, ...):
    Other software involved (rekordbox version, Mixxx version, ...):
    ```

5. For a start-up failure, paste the `--diagnostics` output. For a sync problem, say which destination, which scope and which options you chose, and paste the status text or warning.
6. For a feature request, describe the job you are trying to do and why the current tools fall short, before you suggest a solution.

A screenshot helps when it shows the message or the table. Crop it to the part that matters.

## What not to attach { #what-not-to-attach }

Issues are public. Leave out:

- **Music files.** Do not attach copyrighted audio. Name the file type and, where useful, the title and artist as they appear in the catalog.
- **Your library database or data folder.** `library.sqlite` holds the paths and tags of your whole collection ([Where CueRithm keeps your data](../files/library/where-data-lives.md)).
- **Credentials.** Provider passwords, API tokens, keys, and any solver address that contains a user name, password or key. Say that an account is connected, not how.
- **A rekordbox database or its Agent options file.** They are encrypted and tied to your installation.
- **Private paths.** Replace your user name and folder names with generic ones such as `~/Music/Beatport` before you paste a message or a screenshot.

## After you report { #after }

Answer questions on the issue, and say when it is fixed in a newer release. Fixes ship in new releases, which you download yourself because CueRithm does not update itself ([What CueRithm does not do yet](not-available-yet.md#no-auto-update)).

If you find a mistake in this guide, choose the pencil icon at the top of the page to suggest a change on GitHub, or open an issue.

## Related

- [Fix a problem](troubleshooting/index.md)
- [Frequently asked questions](faq.md)
- [How CueRithm was made](about/provenance.md)
- [Download and install CueRithm](../get-started/install.md)

<!-- verified: apps/desktop/src/components/Sidebar.tsx (footer "CUERITHM 0.1", no About view), src-tauri/src/lib.rs (no menu builder; diagnostics() prints version), .github/workflows/release.yml (file names Cuerithm-<platform>-<version>.<ext>), mkdocs.yml (repo_url, edit_uri, content.action.edit), legacy about/faq.md ("Please do not attach copyrighted audio"), provenance.md (open an issue for corrections), tauri.conf.json (productName/identifier) -->
