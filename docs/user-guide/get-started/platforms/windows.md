# Install on Windows

<p class="lead">Unzip CueRithm into a folder you will keep, leave its <code>media</code> folder beside the program and run it. There is no installer. The build is unsigned, so Windows SmartScreen may warn on the first launch. Allow about five minutes.</p>

**Before you start:** CueRithm needs a 64-bit Intel or AMD PC (x64) running Windows 10 or 11 with the Microsoft Edge WebView2 Runtime, which Windows 11 and up-to-date Windows 10 include. There is no build for Windows on Arm. The full list of requirements is on [Download and install CueRithm](../install.md#requirements).

## 1. Download the zip and compare its digest { #download }

Download `Cuerithm-windows-x64-<version>.zip` from the [latest release](https://github.com/johnneerdael/CueRithm/releases/latest). Because the build is unsigned, Windows cannot tell you who made it, so compare the file's SHA-256 digest with the one GitHub lists. [Verify your download](../install.md#verify-download) has the PowerShell command.

## 2. Unzip it somewhere permanent { #unzip }

Right-click the zip, choose **Extract All** and pick a folder you will not move later, for example `C:\Apps\CueRithm`. You should end up with exactly this:

```text
CueRithm\
  cuerithm-desktop.exe
  media\
    ffmpeg.exe
    ffprobe.exe
    runtime.json
    COPYING.LGPLv2.1
```

Keep the `media` folder **next to** `cuerithm-desktop.exe`. It holds the bundled decoder, and CueRithm will not start without it. Do not unzip into a temporary or Downloads folder you clear out.

## 3. Run it and answer SmartScreen { #first-launch }

Double-click `cuerithm-desktop.exe`. Windows may show **Windows protected your PC** because the program is unsigned and new to your machine. If you trust the download, choose **More info**, then **Run anyway**. If Windows still blocks the files, close the program, right-click the `.zip`, choose **Properties**, tick **Unblock** if it is offered, and extract again. [My computer blocks CueRithm at first launch](../../help/troubleshooting/first-launch-blocked.md) covers the other messages.

**You should see** a window on **Tracks** that says *Your library starts here*. CueRithm has also created `Cuerithm\Incoming` inside your Music folder and watches it. Continue with [Prepare your first tracks](../first-tracks.md).

## 4. Pin it so you can find it { #shortcut }

Because there is no installer, CueRithm has no Start menu entry. Right-click `cuerithm-desktop.exe` and choose **Pin to Start** or **Pin to taskbar**, or **Send to → Desktop (create shortcut)**.

## Check the install { #check }

The Windows program does not open a console, so ask PowerShell to wait for it and save the output. Run this from the folder that holds `cuerithm-desktop.exe`:

```powershell
Start-Process -Wait -NoNewWindow -RedirectStandardOutput diagnostics.txt -RedirectStandardError diagnostics-error.txt .\cuerithm-desktop.exe -ArgumentList "--diagnostics"
Get-Content diagnostics.txt, diagnostics-error.txt
```

A healthy install prints one line of JSON with `"resources_verified":true` and `"built_in_analysis_verified":true`. [Check that the install is healthy](../install.md#check-that-the-install-is-healthy) explains the fields and the errors.

## Update or remove CueRithm { #update-remove }

- **Update:** quit CueRithm, then download the newer zip and extract it over the old folder, replacing the files, or into a new folder. Your library is not in this folder. CueRithm does not update itself; see [Update to a newer version](../install.md#update).
- **Remove:** delete the folder you unzipped. To remove CueRithm's data as well, delete `%APPDATA%\com.cuerithm.app` (press ++win+r++, paste that path and press ++enter++). Your audio files are never inside it.

## Where CueRithm stands on Windows { #status }

The Windows build is produced and checked by the same release pipeline as the Mac build, but acceptance of native playback, import and restart on Windows is not complete. [What CueRithm does not do yet](../../help/not-available-yet.md) lists the open items. If something misbehaves, [report a problem](../../help/report-a-problem.md) with your Windows version.

## Related

- [Download and install CueRithm](../install.md) for requirements, what is bundled and where the data lives.
- [Prepare your first tracks](../first-tracks.md) is the next step.
- [A track shows a dash or will not play](../../help/troubleshooting/no-waveform-or-playback.md) if the window opens but a track is silent.

<!-- verified: .github/workflows/package.yml ("Build Windows portable app" --no-bundle; stage cuerithm-desktop.exe plus tools/media-runtime/bin as media; --diagnostics) and release.yml (zip, unsigned); downloaded v0.1.13 Cuerithm-windows-x64 zip listing (cuerithm-desktop.exe, media/ffmpeg.exe, ffprobe.exe, runtime.json, COPYING.LGPLv2.1); apps/desktop/src-tauri/src/main.rs (windows_subsystem = "windows", --diagnostics), lib.rs (app_data_dir, first-run watch folder), crates/cuerithm-media/src/runtime.rs (bundled decoder messages); docs/release-readiness.md (Windows acceptance not complete). UNVERIFIED: not run on Windows. SmartScreen wording and the Unblock checkbox are standard Windows behaviour; the Start-Process diagnostics command, the %APPDATA% data path and the Windows 10 floor were not tested on a Windows machine. -->
