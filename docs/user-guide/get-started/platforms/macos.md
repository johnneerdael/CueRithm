# Install on a Mac

<p class="lead">Open the disk image, drag CueRithm into Applications and open it. The app is signed with a Developer ID and notarized by Apple, so it should open without an "unidentified developer" warning. Allow about two minutes.</p>

**Before you start:** CueRithm needs a Mac with Apple silicon (M1 or later) and macOS 11 or later. There is no Intel build. Choose **Apple menu → About This Mac** and look for a **Chip** line that starts with "Apple". If you see "Processor" and an Intel name instead, this version cannot run on that Mac. The overview of requirements and downloads is on [Download and install CueRithm](../install.md#requirements).

## 1. Download and check the disk image { #download }

Download `Cuerithm-arm64-<version>.dmg` from the [latest release](https://github.com/johnneerdael/CueRithm/releases/latest). If you want to be certain the file is intact, compare its digest with the one GitHub lists, as described in [Verify your download](../install.md#verify-download).

## 2. Drag CueRithm into Applications { #install }

Double-click the `.dmg`. A window opens with **CueRithm** and a shortcut to **Applications**. Drag **CueRithm** onto the shortcut, then eject the disk image from Finder's sidebar.

## 3. Open CueRithm { #first-launch }

Open **CueRithm** from **Applications** or Launchpad. Because the download came from the internet, macOS may ask once whether you are sure you want to open an app downloaded from the internet. Choose **Open**.

**You should see** a window on **Tracks** that says *Your library starts here*. CueRithm has also created `Cuerithm/Incoming` inside your Music folder and watches it, so music copied there is prepared automatically. Continue with [Prepare your first tracks](../first-tracks.md).

## If macOS will not open CueRithm { #blocked }

A notarized app should open. If macOS refuses, the download is almost always incomplete or altered, so do not look for a way around the block. Check the signature instead.

```sh
spctl --assess --type execute --verbose /Applications/CueRithm.app
```

A healthy copy answers `accepted` and `source=Notarized Developer ID`. To check the disk image before you open it, run `xcrun stapler validate` on the `.dmg`; it should say `The validate action worked!`. If either check fails, delete the app and the `.dmg`, download the file again and repeat the steps above. [My computer blocks CueRithm at first launch](../../help/troubleshooting/first-launch-blocked.md) covers the messages macOS shows.

## Allow access to your music folders { #folder-access }

macOS can ask for permission the first time CueRithm reads a protected place such as Downloads, Desktop, Documents or an external drive. Choose **Allow**. If you chose **Don't Allow** earlier, open **System Settings → Privacy & Security → Files and Folders** (or **Full Disk Access**) and switch CueRithm on. A file CueRithm cannot read fails in [Incoming](../../files/follow-incoming.md) with an error you can read. This is a macOS setting, not a CueRithm setting.

## Check the install { #check }

```sh
/Applications/CueRithm.app/Contents/MacOS/cuerithm-desktop --diagnostics
```

It prints one line of JSON with `"resources_verified":true` and `"built_in_analysis_verified":true`. [Check that the install is healthy](../install.md#check-that-the-install-is-healthy) explains each field and the errors.

## Update or remove CueRithm { #update-remove }

- **Update:** quit CueRithm, download the newer `.dmg` and drag the app onto **Applications** again, choosing **Replace**. Your library stays in `~/Library/Application Support/com.cuerithm.app`. CueRithm does not update itself; see [Update to a newer version](../install.md#update).
- **Remove:** drag **CueRithm** from **Applications** to the Bin. To remove its data as well, delete the `com.cuerithm.app` folder named above. Your audio files are never inside it.

## Related

- [Download and install CueRithm](../install.md) for requirements, what is bundled and where the data lives.
- [Prepare your first tracks](../first-tracks.md) is the next step.
- [My computer blocks CueRithm at first launch](../../help/troubleshooting/first-launch-blocked.md) if macOS still objects.

<!-- verified: .github/workflows/release.yml and tools/release/sign-macos.sh (Developer ID signing, notarization, stapling, DMG with Applications shortcut); mounted v0.1.13 Cuerithm-arm64 DMG: Info.plist, arm64 Mach-O LC_BUILD_VERSION minos 11.0, spctl "accepted source=Notarized Developer ID", xcrun stapler validate "The validate action worked!"; apps/desktop/src-tauri/src/host.rs (first-run watch folder), App.tsx (empty state). UNVERIFIED: the macOS folder-access prompts and the "downloaded from the internet" confirmation are standard macOS behaviour for apps without a sandbox and were not observed with this build; the exact wording of the Intel hint follows About This Mac on current macOS. -->
