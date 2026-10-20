#!/usr/bin/env bash
# Build and test the committed tree on Linux in a Docker container, from a macOS or Linux host.
# usage: linux-container-check.sh            (Docker must be running; set KEEP=1 to keep the volumes)
# It covers the engine and library tests, the sealed ffmpeg runtime built for Linux, the desktop backend's tests
# and a 15-second start of the desktop app under a virtual X display. It does not play audio or drive the window,
# so it is not the native Linux acceptance; it only shows that the code builds, the tests pass and the app starts.
# The container uses the host's CPU architecture (aarch64 on Apple silicon). Only `git archive HEAD` is exported,
# so uncommitted changes are not tested and the host working tree is never written to.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
image=cuerithm-linux-check
vols=(cuerithm-check-src cuerithm-check-cargo cuerithm-check-target)
cleanup() { [ "${KEEP:-0}" = 1 ] || docker volume rm "${vols[@]}" >/dev/null 2>&1 || true; }
trap cleanup EXIT

docker build -q -t "$image" - <<'EOF'
FROM rust:1.90-trixie
RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake build-essential pkg-config clang python3 curl ca-certificates xz-utils nasm git libasound2-dev \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev xvfb xauth dbus-x11 libgl1-mesa-dri x11-utils xdotool \
    && rm -rf /var/lib/apt/lists/*
EOF
for v in "${vols[@]}"; do docker volume create "$v" >/dev/null; done
run() {
  docker run --rm -v cuerithm-check-src:/work -v cuerithm-check-cargo:/usr/local/cargo/registry \
    -v cuerithm-check-target:/target -w /work -e CARGO_TARGET_DIR=/target \
    -e CARGO_PROFILE_DEV_DEBUG=0 -e CARGO_PROFILE_TEST_DEBUG=0 "$@"
}

git -C "$root" archive --format=tar HEAD | docker run --rm -i -v cuerithm-check-src:/work "$image" \
  sh -c 'rm -rf /work/* && tar -x -C /work'
# The desktop crate embeds the built frontend; it is platform independent, so reuse the host's build when present.
if [ -d "$root/apps/desktop/dist" ]; then
  tar -C "$root/apps/desktop" -c dist | docker run --rm -i -v cuerithm-check-src:/work "$image" \
    tar -x -C /work/apps/desktop 2>/dev/null
else
  echo "apps/desktop/dist is missing: run 'pnpm build' in apps/desktop first" >&2
  exit 1
fi

run "$image" bash tools/media-runtime/build.sh
run "$image" cargo test --workspace --exclude cuerithm-desktop --no-fail-fast
run "$image" cargo test -p cuerithm-desktop --no-fail-fast
run -e HOME=/tmp/smoke -e WEBKIT_DISABLE_COMPOSITING_MODE=1 -e WEBKIT_DISABLE_DMABUF_RENDERER=1 "$image" bash -c '
  cargo build -p cuerithm-desktop && mkdir -p "$HOME" &&
  timeout 40 xvfb-run -a -s "-screen 0 1440x900x24" sh -c "
    /target/debug/cuerithm-desktop & pid=\$!; sleep 15
    xdotool search --name \"\" getwindowname %@ 2>/dev/null | head -5
    if kill -0 \$pid 2>/dev/null; then echo APP_STILL_RUNNING_AFTER_15S; kill \$pid; else wait \$pid; echo APP_EXITED_\$?; exit 1; fi"'
echo "Linux container check passed"
