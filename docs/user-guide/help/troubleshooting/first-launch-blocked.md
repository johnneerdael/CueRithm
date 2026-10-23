# My computer blocks CueRithm at first launch

<p class="lead">When your system warns about CueRithm or it closes straight after you open it, the cause depends on your platform. The macOS app is signed and notarized. The Windows and Linux packages are not signed, so Windows may warn you once. Find your platform below; each fix takes a few minutes.</p>

## Why it happens { #why }

| Platform | Package | Signing | What to expect |
|---|---|---|---|
| macOS, Apple Silicon | `.dmg` | Signed with a Developer ID certificate and notarized by Apple | A normal "downloaded from the Internet" confirmation, at most once. There is no Intel Mac build. |
| Windows, x86-64 | `.zip` (portable) | Unsigned | Windows SmartScreen may warn on first launch. |
| Linux, x86-64 | `.deb` | Unsigned | No warning, but the package needs its system libraries installed. |

Beyond signing, CueRithm checks its own bundled decoder when it starts. If the decoder files are missing or changed, the app cannot start. An incomplete download, a partly extracted ZIP or an antivirus tool that removed a file can cause this. Every release lists its files on the [releases page](https://github.com/johnneerdael/CueRithm/releases/latest), with the matching source and `THIRD_PARTY_NOTICES.md`, so you can compare the download with the source and the build recipe.

## Fix it { #fix }

=== "macOS"

    1. Open the `.dmg` and drag **CueRithm** into **Applications**. Open it from there, not from inside the disk image.
    2. If macOS asks whether you want to open an app downloaded from the Internet, choose **Open**.
    3. If macOS says the app is damaged or cannot be verified, delete it and the `.dmg`, and download the file again. An interrupted download or a copy made by another tool can break the signature.
    4. If macOS says the app cannot run on this Mac, check the chip. CueRithm builds only for Apple Silicon. [Install on a Mac](../../get-started/platforms/macos.md) has the details.

=== "Windows"

    1. Extract the whole ZIP to a permanent folder such as `C:\Apps\CueRithm`. Do not run the program from inside the ZIP.
    2. Keep the `media` folder next to `cuerithm-desktop.exe`. It holds the bundled decoder, and CueRithm cannot start without it.
    3. Run `cuerithm-desktop.exe`. If SmartScreen shows *Windows protected your PC*, choose **More info**, then **Run anyway**, but only if you downloaded the file from the releases page.
    4. If your security software removed or blocked files, restore them or extract the ZIP again, then allow the folder.
    5. If the window never appears, make sure the Microsoft Edge WebView2 runtime is installed. <!-- UNVERIFIED: whether the portable build needs a separately installed WebView2 runtime on older Windows 10; Tauri 2 on Windows uses WebView2 and tauri.conf.json sets no installer mode -->

    [Install on Windows](../../get-started/platforms/windows.md) has the details.

=== "Linux"

    1. Install with your package manager, so that dependencies are fetched:

        ```sh
        sudo apt install ./Cuerithm-linux-x64-<version>.deb
        ```

    2. Start **CueRithm** from your application menu, or run `cuerithm-desktop`.
    3. The package declares the libraries it needs, including the WebKit GTK stack and audio support. CueRithm plays through your system's default audio output.

    [Install on Linux](../../get-started/platforms/linux.md) has the details.

### Run the built-in check { #diagnostics }

If CueRithm still closes at start, run its self-check. It verifies the bundled decoder and the key model, prints one line, and exits. On macOS:

```sh
/Applications/CueRithm.app/Contents/MacOS/cuerithm-desktop --diagnostics
```

`resources_verified` and `built_in_analysis_verified` should both read `true`. On failure it prints the reason, such as a decoder hash mismatch, and exits with an error. If it does, reinstall from a fresh download. [Download and install CueRithm](../../get-started/install.md) shows the command for Windows and Linux.

Two other start-up messages have their own meaning. `This library is already open` means a second CueRithm is using the same library, so close the other one. `Library schema is newer than this application` means the library was opened by a newer version, and CueRithm cannot go back to an older one ([what CueRithm does not do yet](../not-available-yet.md#library-schema)).

## Check it worked { #check }

- The CueRithm window opens with **Tracks**, **Incoming** and **Archive** under LIBRARY in the sidebar.
- The sidebar footer reads CUERITHM 0.1.
- `--diagnostics` prints `"resources_verified":true`.

## Still stuck? { #still-stuck }

[Report the problem](../report-a-problem.md) with your operating system and version, the file you downloaded, and the exact message. For a start-up failure, add the `--diagnostics` output. Do not attach your music or library folder.

## Related

- [Download and install CueRithm](../../get-started/install.md)
- [Install on a Mac](../../get-started/platforms/macos.md)
- [Install on Windows](../../get-started/platforms/windows.md)
- [Report a problem or ask for a feature](../report-a-problem.md)

<!-- verified: README.md Install table (DMG signed + notarized, ZIP portable with media folder, deb unsigned, no Intel build); .github/workflows/release.yml and package.yml (arm64 macOS only, unsigned Linux/Windows, deb named Cuerithm-linux-x64-<version>.deb); tools/release/sign-macos.sh (Developer ID signing, notarytool, stapler); apps/desktop/src-tauri/src/main.rs and lib.rs (--diagnostics output fields; runtime from_bundle hash checks "Decoder hash mismatch"); crates/cuerithm-library/src/store.rs ("This library is already open", "Library schema is newer than this application"); Sidebar.tsx (LIBRARY section, footer CUERITHM 0.1); legacy get-started/install.md re-checked. UNVERIFIED: Windows SmartScreen wording, macOS dialog wording and WebView2 need are OS behaviour, not read from the code -->
