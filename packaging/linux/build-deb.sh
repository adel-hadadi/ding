#!/bin/sh
# Build an amd64 .deb you can install with: sudo apt install ./ding_*.deb
set -eu

HERE=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$HERE/../.." && pwd)
OUT="${1:-$ROOT/dist}"

GUI="$ROOT/target/release/ding-gui"
CLI="$ROOT/target/release/ding"

if [ ! -f "$GUI" ]; then
  echo "Build the window first:" >&2
  echo "  cargo build --release -p ding-client --bins" >&2
  exit 1
fi

VERSION=$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -n1)
ARCH=$(dpkg --print-architecture 2>/dev/null || echo amd64)
NAME="ding_${VERSION}_${ARCH}"

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

mkdir -p \
  "$STAGE/DEBIAN" \
  "$STAGE/usr/bin" \
  "$STAGE/usr/share/applications" \
  "$STAGE/usr/share/icons/hicolor/scalable/apps" \
  "$STAGE/usr/share/doc/ding"

install -m 755 "$GUI" "$STAGE/usr/bin/ding-gui"
if [ -f "$CLI" ]; then
  install -m 755 "$CLI" "$STAGE/usr/bin/ding"
fi

sed 's|^Exec=.*|Exec=/usr/bin/ding-gui|' \
  "$HERE/com.adelhadadi.ding.desktop" \
  > "$STAGE/usr/share/applications/com.adelhadadi.ding.desktop"

install -m 644 "$HERE/com.adelhadadi.ding.svg" \
  "$STAGE/usr/share/icons/hicolor/scalable/apps/com.adelhadadi.ding.svg"

SIZE=$(du -sk "$STAGE" | awk '{print $1}')

cat > "$STAGE/DEBIAN/control" <<EOF
Package: ding
Version: ${VERSION}
Section: net
Priority: optional
Architecture: ${ARCH}
Maintainer: Adel Hadadi <hadadiadel81@gmail.com>
Homepage: https://github.com/adel-hadadi/ding
Installed-Size: ${SIZE}
Depends: libc6, libgcc-s1, libasound2 | libasound2t64, libxkbcommon0, libwayland-client0, libegl1
Description: ding a coworker on the local network
 A small desktop app. People on the same Wi-Fi show up in a window;
 click them to play a bell, knock, or airhorn on their screen.
EOF

cat > "$STAGE/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q /usr/share/icons/hicolor || true
fi
EOF
chmod 755 "$STAGE/DEBIAN/postinst"

cat > "$STAGE/usr/share/doc/ding/copyright" <<'EOF'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: ding
Source: https://github.com/adel-hadadi/ding

Files: *
Copyright: 2026 Adel Hadadi
License: UNKNOWN
 The project license is the one shipped with the source repository.
EOF

mkdir -p "$OUT"
DEB="$OUT/${NAME}.deb"
dpkg-deb --root-owner-group --build "$STAGE" "$DEB" >/dev/null
echo "Wrote $DEB"
