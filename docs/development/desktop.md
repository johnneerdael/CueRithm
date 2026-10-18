# Desktop development

Use Rust stable **1.90 or newer** for the Tauri host, Node 22 and pnpm 12.14.0.
The independent analysis crates retain their declared Rust 1.89 floor. Install
the platform's [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
and a native C/C++ compiler/CMake for OpenKeyScan. Linux also needs ALSA headers
(`libasound2-dev` on Ubuntu) for Rust playback. Engine-only builds do not require
WebKit/GTK or ALSA: select `-p camelot-cli` or exclude both desktop and library.

## Build the decoder once

`bash tools/media-runtime/build.sh` downloads the pinned FFmpeg source, verifies
its hash, builds the minimal decoder and verifies codec coverage and system-only
dynamic dependencies. On Intel Macs/Linux install NASM. On Windows run the same
script in MSYS2 MINGW64 with make, curl, NASM, diffutils, MinGW GCC and Python.
See `.github/actions/media-runtime/action.yml` for exact CI setup. Windows codec
executables statically link compiler support rather than requiring MSYS2 DLLs.

The resulting ignored `tools/media-runtime/bin/` contains ffmpeg, ffprobe,
runtime hashes/recipe and LGPL notice. Never substitute Homebrew-linked binaries
or download an unverified decoder while preparing music. Python is build/test
tooling only. Models, music and library databases remain outside Git.

```sh
pnpm --dir apps/desktop install --frozen-lockfile
pnpm --dir apps/desktop tauri dev
```

In unbundled debug mode, set `CUERITHM_MEDIA_RUNTIME` to the absolute runtime
folder if needed. `CUERITHM_DATA_DIR` selects an isolated acceptance database;
avoid pointing tests at a real library. A new acceptance database may read
`CUERITHM_BOOTSTRAP_SETTINGS` (a complete PreparationSettings JSON). Existing
databases always retain their own settings. Normal users use Settings once.

Set the local energy model path, optionally the OpenKeyScan model, and watch
folders in Settings. Enabled unavailable stages stay in Incoming with a reason;
there is no placeholder success. Full successful analysis replaces library cues
atomically and promotes into Tracks. This foundation does not write file tags
or sync a target DJ database. Source audio files remain untouched.

## Checks

```sh
pnpm --dir apps/desktop test
pnpm --dir apps/desktop build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --exclude cuerithm-desktop --release --locked
cargo test -p cuerithm-desktop --lib --locked
python3 -m unittest discover -s tools/fixtures/rekordbox -v
python3 -m unittest discover -s tools/media-runtime -v
```

Required format tests fail if the packaged runtime is absent. The original
generated 14-case fixture archive covers codecs, padding/edit lists, channels,
sample rates, high-depth/float PCM, seek and shared waveform clocks. Lossless
PCM hashes are exact; lossy checkpoints allow 0.00005 float rounding differences
across architectures. Synthetic SVC fixtures exercise the complete stage contract
without claiming trained-model accuracy. Real development-model checks are
explicit and private, with holdouts reserved.

The native output-device test is ignored by headless CI; on a machine with an
output device, run `cargo test -p cuerithm-library --test playback -- --ignored`.
Actual app acceptance must separately check audible preview, cue/seek position,
background promotion, cue replacement, Settings persistence and restart. Browser
mocks and successful device initialization alone are insufficient evidence.

## Unsigned test packaging

```sh
# macOS
pnpm --dir apps/desktop tauri build --bundles app
target/release/bundle/macos/Cuerithm.app/Contents/MacOS/cuerithm-desktop --diagnostics

# Linux
pnpm --dir apps/desktop tauri build --bundles deb

# Windows: build the executable, stage it beside a media/ copy of the runtime
pnpm --dir apps/desktop tauri build --no-bundle
```

`--diagnostics` resolves the same platform resource directory as Tauri and
requires the bundled codec files/hashes. It exits with an error if resources are
missing. Linux CI extracts the actual deb before checking it. Windows CI stages
a portable executable with its media folder; macOS archives the actual app with
executable permissions preserved. Artifacts include source/rebuild materials.

The [desktop workflow](../../.github/workflows/desktop.yml) covers Linux x86_64,
Windows x86_64 and macOS arm64/x86_64 on
[GitHub's native runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Tests and diagnostics establish what each run reports; CI configuration alone
does not establish a green build, live Windows/Linux UI behavior, signing,
notarization or target DJ-app round trips. Signing/public release is a later gate.

The approved scope after this foundation remains persistent Beatport/Traxsource
tagging, cleanup, ordinary/full smart playlists and direct rekordbox 6/7 sync.
See [the early direct contract](../research/rekordbox-direct-contract.md) for
known identities, static evidence and precise missing target acceptance.
