#!/usr/bin/env bash
# Sign, notarize and staple a Cuerithm macOS app archive, producing a DMG.
# usage: sign-macos.sh <Cuerithm-*.app.tar.gz> <output.dmg>
# env: SIGN_IDENTITY, NOTARY_KEY_PATH (App Store Connect .p8), NOTARY_KEY_ID, NOTARY_ISSUER_ID
set -euo pipefail
archive=$1
dmg=$2
work=$(cd "$(mktemp -d)" && pwd -P)  # the app rejects symlinked paths such as /var
trap 'rm -rf "$work"' EXIT
tar -xzf "$archive" -C "$work"
app="$work/CueRithm.app"
media="$app/Contents/Resources/media"

sign() { codesign --force --timestamp --options runtime --sign "$SIGN_IDENTITY" "$@"; }

# The sealed runtime manifest records ffmpeg/ffprobe hashes, so sign them first and re-seal.
for tool in ffmpeg ffprobe; do sign "$media/$tool"; done
python3 - "$media/runtime.json" <<'PY'
import hashlib, json, sys
from pathlib import Path
path = Path(sys.argv[1])
manifest = json.loads(path.read_text())
for name in ("ffmpeg", "ffprobe"):
    manifest[name + "_sha256"] = hashlib.sha256((path.parent / name).read_bytes()).hexdigest()
path.write_text(json.dumps(manifest, indent=2) + "\n")
PY

# Remaining nested Mach-O files (inside-out), then the main executable and the bundle.
main="$app/Contents/MacOS/cuerithm-desktop"
while IFS= read -r -d '' file; do
  [ "$file" = "$main" ] && continue
  case "$file" in "$media/ffmpeg"|"$media/ffprobe") continue ;; esac
  if file -b "$file" | grep -q 'Mach-O'; then sign "$file"; fi
done < <(find "$app/Contents" -type f -print0)
sign "$main"
sign "$app"
codesign --verify --deep --strict --verbose=2 "$app"
"$main" --diagnostics

# DMG, signed and notarized as a whole so the stapled ticket travels with it.
stage="$work/dmg"
mkdir "$stage"
ditto "$app" "$stage/CueRithm.app"
ln -s /Applications "$stage/Applications"
hdiutil create -quiet -volname CueRithm -srcfolder "$stage" -format UDZO "$dmg"
sign "$dmg"
xcrun notarytool submit "$dmg" --key "$NOTARY_KEY_PATH" --key-id "$NOTARY_KEY_ID" \
  --issuer "$NOTARY_ISSUER_ID" --wait | tee "$work/notary.txt"
grep -q 'status: Accepted' "$work/notary.txt" || { echo 'Notarization was not accepted' >&2; exit 1; }
# Stapling downloads the ticket from Apple's CloudKit, which can drop the connection after a
# notarization was already Accepted ("The network connection was lost", stapler error 68, seen on
# two runs in a row), so try again a few times before failing the release.
staple() {
  local attempts=6 attempt
  for attempt in $(seq 1 "$attempts"); do
    if xcrun stapler staple "$dmg"; then return 0; fi
    echo "stapler staple failed (attempt $attempt of $attempts)" >&2
    if [ "$attempt" -lt "$attempts" ]; then sleep $((attempt * 30)); fi
  done
  return 1
}
staple
xcrun stapler validate "$dmg"
spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"
