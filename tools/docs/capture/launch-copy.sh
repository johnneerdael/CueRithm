#!/bin/sh
# Starts the installed CueRithm on a COPY of its data folder, in a macOS sandbox that lets it write
# only to that copy and to the app's own cache, web-view, log and temporary folders.
#
# A copied data folder still names the owner's real music folders, and CueRithm starts its background
# work as soon as it opens: it watches the saved watch folders, renames or tags new files there
# according to the saved settings, and continues queued jobs. The sandbox makes sure that nothing can
# rename, tag, move or delete music whatever those settings say, wherever the music is (an external
# drive, ~/Music, or a cloud folder under ~/Library/CloudStorage). Reading is unaffected, so waveforms,
# playback and scans work. A write the sandbox blocks shows up as a failed job or an error banner:
# reject that screenshot instead of widening the sandbox.
#
# Usage: launch-copy.sh DATA_COPY [CUERITHM_APP]
#        launch-copy.sh --print-profile DATA_COPY      # print the sandbox profile and stop
#   DATA_COPY      a copy of the data folder (it holds library.sqlite); the app migrates it
#   CUERITHM_APP   defaults to /Applications/CueRithm.app
set -eu

print_only=
if [ "${1:-}" = --print-profile ]; then print_only=1; shift; fi
copy=${1:?usage: launch-copy.sh [--print-profile] DATA_COPY [CUERITHM_APP]}
app=${2:-/Applications/CueRithm.app}
binary="$app/Contents/MacOS/cuerithm-desktop"
live="$HOME/Library/Application Support/com.cuerithm.app"
id=com.cuerithm.app

[ "$(uname)" = Darwin ] || { echo "launch-copy.sh needs macOS (sandbox-exec)" >&2; exit 1; }
[ -d "$copy" ] || { echo "$copy is not a folder" >&2; exit 1; }
[ -e "$copy/library.sqlite" ] || [ -e "$copy/active-library.json" ] || {
  echo "$copy does not look like a CueRithm data folder (no library.sqlite)" >&2; exit 1; }

copy=$(cd "$copy" && pwd -P)
live_real=$(cd "$live" 2>/dev/null && pwd -P || echo "$live")
case "$copy/" in
  "$live_real"/*) echo "Refusing to run on the live data folder $live_real" >&2; exit 1 ;;
esac
case "$live_real/" in
  "$copy"/*) echo "Refusing: $copy contains the live data folder" >&2; exit 1 ;;
esac

# Later rules win: everything is read-only, then only the copy and the app's own scratch folders are
# writable, and the live data folder is read-only again.
profile="(version 1)
(allow default)
(deny file-write*)
(allow file-write*
  (subpath \"$copy\")
  (subpath \"$HOME/Library/Caches/$id\")
  (subpath \"$HOME/Library/WebKit/$id\")
  (subpath \"$HOME/Library/HTTPStorages/$id\")
  (subpath \"$HOME/Library/Saved Application State/$id.savedState\")
  (subpath \"$HOME/Library/Logs/$id\")
  (subpath \"/private/var/folders\")
  (subpath \"/dev\"))
(deny file-write* (subpath \"$live_real\"))"

if [ -n "$print_only" ]; then printf '%s\n' "$profile"; exit 0; fi

[ -x "$binary" ] || { echo "No CueRithm at $app" >&2; exit 1; }
if pgrep -x cuerithm-desktop >/dev/null; then
  echo "CueRithm is already running; quit it first (it would run outside the sandbox)" >&2
  exit 1
fi

echo "Starting $app on $copy (writes allowed only there and in the app's own scratch folders)"
CUERITHM_DATA_DIR="$copy" exec sandbox-exec -p "$profile" "$binary"
