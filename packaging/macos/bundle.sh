#!/bin/sh
# Wrap ding-gui in Ding.app so Finder and Launchpad can open it.
set -eu

BIN="${1:?path to ding-gui}"
DEST="${2:?path to Ding.app}"
HERE=$(CDPATH= cd -- "$(dirname "$0")" && pwd)

rm -rf "$DEST"
mkdir -p "$DEST/Contents/MacOS" "$DEST/Contents/Resources"
cp "$BIN" "$DEST/Contents/MacOS/Ding"
chmod +x "$DEST/Contents/MacOS/Ding"
cp "$HERE/Info.plist" "$DEST/Contents/Info.plist"
echo "Wrote $DEST"
