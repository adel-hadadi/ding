#!/bin/sh
# Put Ding in the Ubuntu app menu so you can open it without a terminal.
set -eu

HERE=$(CDPATH= cd -- "$(dirname "$0")" && pwd)

if [ -f "$HERE/../../target/release/ding-gui" ]; then
  BIN="$HERE/../../target/release/ding-gui"
  DESKTOP="$HERE/com.adelhadadi.ding.desktop"
  ICON="$HERE/com.adelhadadi.ding.svg"
elif [ -f "$HERE/ding-gui" ]; then
  BIN="$HERE/ding-gui"
  DESKTOP="$HERE/com.adelhadadi.ding.desktop"
  ICON="$HERE/com.adelhadadi.ding.svg"
else
  echo "Build the window first:" >&2
  echo "  cargo build --release -p ding-client --bin ding-gui" >&2
  exit 1
fi

BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"
cp "$BIN" "$BIN_DIR/ding-gui"
chmod +x "$BIN_DIR/ding-gui"
cp "$ICON" "$ICON_DIR/com.adelhadadi.ding.svg"

# Point Exec at the file we just installed, so the menu works even if
# ~/.local/bin is not on the GUI session PATH.
sed "s|^Exec=.*|Exec=${BIN_DIR}/ding-gui|" "$DESKTOP" > "$APP_DIR/com.adelhadadi.ding.desktop"

command -v update-desktop-database >/dev/null 2>&1 &&
  update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true

echo "Ding is in your app menu. Search for “Ding” and click it."
echo "Installed: $BIN_DIR/ding-gui"
