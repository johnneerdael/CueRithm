# Install on Linux

<p class="lead">Install the unsigned <code>.deb</code> package with apt, which pulls in the system libraries CueRithm needs, then start it from your application menu. Allow about three minutes.</p>

**Before you start:** CueRithm is built for 64-bit Intel and AMD (amd64) and needs a distribution that installs `.deb` packages and has glibc 2.39 or newer. Ubuntu 24.04 and newer meet that; releases with an older glibc, such as Ubuntu 22.04 or Debian 12, cannot start this build. Check yours with `ldd --version`. There is no Arm build, no Flatpak, no AppImage and no apt repository. The full list of requirements is on [Download and install CueRithm](../install.md#requirements).

## 1. Download the package and compare its digest { #download }

Download `Cuerithm-linux-x64-<version>.deb` from the [latest release](https://github.com/johnneerdael/CueRithm/releases/latest). The package is unsigned, so compare its SHA-256 digest with the one GitHub lists; [Verify your download](../install.md#verify-download) has the command.

## 2. Install it with apt { #install }

Run apt from the folder that holds the file. The leading `./` tells apt to install a local file.

```sh
sudo apt install ./Cuerithm-linux-x64-<version>.deb
```

The package is named `cue-rithm`. It declares two dependencies, `libwebkit2gtk-4.1-0` and `libgtk-3-0`, and apt installs them if they are missing. It puts these files on your system:

| Path | What it is |
|---|---|
| `/usr/bin/cuerithm-desktop` | The program. |
| `/usr/lib/CueRithm/media/` | The bundled decoder: `ffmpeg`, `ffprobe`, `runtime.json` and `COPYING.LGPLv2.1`. |
| `/usr/share/applications/CueRithm.desktop` | The menu entry named CueRithm. |

CueRithm plays audio through ALSA (`libasound.so.2`). The package does not list it as a dependency because desktop installs already have it. If the program fails to start and mentions that library, install your distribution's ALSA library (`libasound2`, or `libasound2t64` on Ubuntu 24.04).

## 3. Start CueRithm { #first-launch }

Search your application menu for **CueRithm**, or run `cuerithm-desktop` in a terminal. **You should see** a window on **Tracks** that says *Your library starts here*. CueRithm has also created `Cuerithm/Incoming` inside your Music folder and watches it. Continue with [Prepare your first tracks](../first-tracks.md).

CueRithm plays through your system's default audio output and has no output picker. If a track plays silently, set the default output in your system sound settings first.

## Check the install { #check }

```sh
/usr/bin/cuerithm-desktop --diagnostics
```

A healthy install prints one line of JSON with `"resources_verified":true` and `"built_in_analysis_verified":true`. [Check that the install is healthy](../install.md#check-that-the-install-is-healthy) explains the fields and the errors.

## Update or remove CueRithm { #update-remove }

- **Update:** quit CueRithm and run `sudo apt install ./Cuerithm-linux-x64-<newer-version>.deb`. CueRithm does not update itself and the package has no repository, so apt will not offer new versions; see [Update to a newer version](../install.md#update).
- **Remove:** run `sudo apt remove cue-rithm`. Your library stays in `~/.local/share/com.cuerithm.app` until you delete it. Your audio files are never inside it.

## Use a Windows DJ library from Linux { #windows-libraries }

CueRithm can sync to a Windows DJ library that you run through Wine or Proton, using saved application locations and drive mappings. Playback of that library through Wine or Proton is not yet verified. See [Use a Windows DJ library on Linux](../../sync/windows-libraries-on-linux.md).

## Where CueRithm stands on Linux { #status }

The Linux package is produced and checked by the same release pipeline as the Mac build, but acceptance of native playback, import and restart on Linux is not complete. [What CueRithm does not do yet](../../help/not-available-yet.md) lists the open items. If something misbehaves, [report a problem](../../help/report-a-problem.md) with your distribution and version.

## Related

- [Download and install CueRithm](../install.md) for requirements, what is bundled and where the data lives.
- [Prepare your first tracks](../first-tracks.md) is the next step.
- [A track shows a dash or will not play](../../help/troubleshooting/no-waveform-or-playback.md) if the window opens but a track is silent.

<!-- verified: .github/workflows/package.yml (ubuntu-24.04 build, tauri bundle deb, dpkg-deb extract and --diagnostics) and release.yml (unsigned .deb); downloaded v0.1.13 .deb: control file (Package cue-rithm, Architecture amd64, Depends libwebkit2gtk-4.1-0, libgtk-3-0), data.tar.gz layout (usr/bin/cuerithm-desktop, usr/lib/CueRithm/media, CueRithm.desktop), strings of the ELF (needs GLIBC_2.39, links libasound.so.2, loads libwebkit2gtk-4.1.so.0); crates/cuerithm-library/Cargo.toml (rodio playback); host.rs and lib.rs (first-run watch folder, app_data_dir); release notes v0.1.12 (Wine/Proton mapping, playback not verified); docs/release-readiness.md. UNVERIFIED: not run on Linux. The Ubuntu 22.04 and Debian 12 statements are inferred from the binary's GLIBC_2.39 requirement and those releases' glibc versions; the ~/.local/share data path is Tauri's app_data_dir convention; the libasound2 package names are the distribution names, not tested. -->
