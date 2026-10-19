# Install

CueRithm is distributed as GitHub release assets. Open the [releases page](https://github.com/johnneerdael/CueRithm/releases) and pick the latest release.

## What is in a release

| File | What it is |
| --- | --- |
| `Cuerithm-arm64-<version>.dmg` or `CueRithm-arm64-<version>.dmg` | macOS app for Apple Silicon. Signed with a Developer ID certificate and notarized by Apple. |
| `Cuerithm-linux-x64-<version>.deb` | Linux x86-64 package. Unsigned. |
| `Cuerithm-windows-x64-<version>.zip` | Windows x86-64 portable app with its `media` folder. Unsigned. |
| `cuerithm-source.tar.gz` | The application source matching the build. |
| `ffmpeg-9.0.2.tar.xz` | The FFmpeg source used for the bundled decoder, for LGPL compliance. |
| `THIRD_PARTY_NOTICES.md` | Licences and notices for bundled components. |

The file name casing follows the app name at the time of the build. The first releases used "Cuerithm"; newer builds use "CueRithm".

## macOS

1. Open the `.dmg`.
2. Drag the app into **Applications**.
3. Open it. Because the app is notarized, macOS opens it without the "unidentified developer" warning. If you downloaded it through a browser you may see the usual "downloaded from the internet" confirmation once.

There is no Intel Mac build. Apple Silicon only.

## Windows

1. Unzip the archive somewhere permanent, for example `C:\Apps\CueRithm`.
2. Keep the `media` folder **next to** `cuerithm-desktop.exe`. It contains the bundled decoder; the app refuses to start without it.
3. Run `cuerithm-desktop.exe`.

The build is unsigned, so Windows SmartScreen may warn on first launch. Choose **More info**, then **Run anyway**, if you trust the download. You can compare the file with the source and build recipe in the repository.

## Linux

```sh
sudo apt install ./Cuerithm-linux-x64-<version>.deb
```

The package declares the system libraries it needs (the WebKit GTK stack and audio support). CueRithm uses your system's default audio output.

## Check that the install is healthy

The app can verify its own bundled resources and exit. This is the same check the build pipeline runs on every package.

=== "macOS"

    ```sh
    /Applications/CueRithm.app/Contents/MacOS/cuerithm-desktop --diagnostics
    ```

=== "Windows"

    ```powershell
    .\cuerithm-desktop.exe --diagnostics
    ```

=== "Linux"

    ```sh
    /usr/bin/cuerithm-desktop --diagnostics
    ```

On success it prints one JSON line such as:

```json
{"application":"Cuerithm","version":"0.1.0","decoder":"<recipe>:<ffmpeg-sha256>:<ffprobe-sha256>","resources_verified":true,"built_in_analysis_verified":true}
```

`resources_verified` means the bundled `ffmpeg` and `ffprobe` match the SHA-256 hashes sealed at build time. `built_in_analysis_verified` means the embedded key-detection model matches its pinned hash. On failure the command prints the reason to the error stream and exits with code 1. See [Decoding and the media runtime](../technical/decoding.md).

## Where CueRithm keeps its data

| System | Data folder |
| --- | --- |
| macOS | `~/Library/Application Support/com.cuerithm.app` |
| Windows | `%APPDATA%\com.cuerithm.app` (platform convention; not yet verified by the project) |
| Linux | `~/.local/share/com.cuerithm.app` (platform convention; not yet verified by the project) |

It holds `library.sqlite` (the library), caches for artwork and waveforms, the embedded analysis model, and `tagging/`, where original-file backups are kept. Your music stays wherever you keep it. Details in [The library database](../technical/library.md).

!!! note "Advanced: isolated profiles"
    Setting the environment variable `CUERITHM_DATA_DIR` to a folder makes CueRithm use that folder instead. This is how the screenshots in this guide were made without touching a real library.

## Uninstall

Delete the app (on Linux, remove the package with your package manager). To remove all CueRithm data, also delete the data folder above. Your audio files are never inside it.
