#!/usr/bin/env bash
# Pretty macOS DMG for SuperFlow: dark spotlight background with a glowing
# arrow between the app icon (left) and the Applications symlink (right).
# No text in the artwork. Replaces tauri-bundler's stock DMG face.
#
# Usage: bash scripts/package-dmg.sh [path-to-SuperFlow.app] [version]
# Defaults: src-tauri/target/release/bundle/macos/SuperFlow.app + version
# from src-tauri/tauri.conf.json. Output: src-tauri/target/release/bundle/dmg/
#
# Requires macOS + ffmpeg (artwork composite) + hdiutil/osascript (stock).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${1:-$ROOT/src-tauri/target/release/bundle/macos/SuperFlow.app}"
VERSION="${2:-$(python3 -c "import json; print(json.load(open('$ROOT/src-tauri/tauri.conf.json'))['version'])")}"
VOL="SuperFlow"
OUT_DIR="$ROOT/src-tauri/target/release/bundle/dmg"
OUT_DMG="$OUT_DIR/SuperFlow_${VERSION}.dmg"

# Window geometry in points (background is exactly 2x for retina).
WIN_W=926
WIN_H=424
APP_X=200
APP_Y=240
APPS_X=726
APPS_Y=240

command -v ffmpeg >/dev/null || { echo "package-dmg: ffmpeg not found" >&2; exit 1; }
[ -d "$APP" ] || { echo "package-dmg: app not found: $APP" >&2; exit 1; }
mkdir -p "$OUT_DIR"

STAGE="$(mktemp -d /tmp/superflow-dmg.XXXXXX)"
RW_DMG="$STAGE/rw.dmg"
cleanup() {
  hdiutil detach "/Volumes/$VOL" -force -quiet 2>/dev/null || true
  rm -rf "$STAGE"
}
trap cleanup EXIT

# 1. Flatten background + arrow into one retina DMG backdrop (no text).
ffmpeg -y -v error \
  -i "$ROOT/public/background.avif" -i "$ROOT/public/arrow.avif" \
  -filter_complex "[0]scale=1852:848:flags=lanczos[bg];[1]scale=380:-1:flags=lanczos,split[a][b];[b]format=gray,curves=m='0/0 0.68/0 0.85/0.5 1/1'[alpha];[a][alpha]alphamerge[fg];[bg][fg]overlay=(W-w)/2:290:format=auto:shortest=1" \
  "$STAGE/.background.png"

# 2. Stage volume contents.
APP_MB="$(du -sm "$APP" | cut -f1)"
SIZE_MB=$((APP_MB + 150))
hdiutil create -size "${SIZE_MB}m" -fs HFS+ -volname "$VOL" -ov "$RW_DMG" -quiet
hdiutil attach "$RW_DMG" -mountpoint "/Volumes/$VOL" -nobrowse -quiet
cp -R "$APP" "/Volumes/$VOL/"
ln -s /Applications "/Volumes/$VOL/Applications"
cp "$STAGE/.background.png" "/Volumes/$VOL/.background.png"

# 3. Finder layout: icon view, bounds, backdrop, icon slots.
osascript <<EOF
tell application "Finder"
  tell disk "$VOL"
    open
    set current view of container window to icon view
    set toolbar visible of container window to false
    set statusbar visible of container window to false
    set the bounds of container window to {100, 100, $((100 + WIN_W)), $((100 + WIN_H))}
    set viewOptions to icon view options of container window
    set arrangement of viewOptions to not arranged
    set icon size of viewOptions to 128
    set text size of viewOptions to 12
    set background picture of viewOptions to file ".background.png"
    delay 1
    set position of item "SuperFlow.app" of container window to {$APP_X, $APP_Y}
    set position of item "Applications" of container window to {$APPS_X, $APPS_Y}
    update without registering applications
    delay 1
    close
  end tell
  delay 1
end tell
EOF

# 4. Compress + auto-open on download.
/usr/bin/hdiutil detach "/Volumes/$VOL" -quiet
hdiutil convert "$RW_DMG" -format UDZO -imagekey zlib-level=9 -o "$OUT_DMG" -ov -quiet
hdiutil internet-enable -yes "$OUT_DMG" >/dev/null
trap - EXIT
rm -rf "$STAGE"
echo "package-dmg: wrote $OUT_DMG"
