#!/bin/sh
# Runs inside the build container. Use packaging/build-windows-linux.sh instead of running this directly.
set -eu

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ROOT=$PWD

if [ "$(uname -m)" != "x86_64" ]; then
    export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
    export AR_x86_64_unknown_linux_gnu=x86_64-linux-gnu-ar
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
fi

cargo build --release --locked -p pigeon-app --target x86_64-unknown-linux-gnu
cargo build --release --locked -p pigeon-app --target x86_64-pc-windows-gnu

LINUX_BIN=$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release/pigeon
WINDOWS_BIN=$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/pigeon.exe
WORK=$(mktemp -d)
mkdir -p dist

# ---- Windows: portable zip and installer
ZIP_DIR=$WORK/Pigeon-$VERSION-windows-x86_64
mkdir -p "$ZIP_DIR"
cp "$WINDOWS_BIN" "$ZIP_DIR/pigeon.exe"
(cd "$WORK" && zip -qr "$ROOT/dist/Pigeon-$VERSION-windows-x86_64.zip" "Pigeon-$VERSION-windows-x86_64")
makensis -V2 \
    -DVERSION="$VERSION" \
    -DEXE="$WINDOWS_BIN" \
    -DICON="$ROOT/assets/icon/pigeon.ico" \
    -DOUT="$ROOT/dist/Pigeon-$VERSION-windows-x86_64-setup.exe" \
    packaging/windows/pigeon.nsi

# ---- Linux: archive
TAR_DIR=$WORK/Pigeon-$VERSION-linux-x86_64
mkdir -p "$TAR_DIR"
cp "$LINUX_BIN" "$TAR_DIR/pigeon"
cp packaging/linux/pigeon.desktop packaging/linux/install.sh assets/icon/pigeon-256.png assets/icon/pigeon-512.png "$TAR_DIR/"
chmod +x "$TAR_DIR/install.sh"
tar -czf "dist/Pigeon-$VERSION-linux-x86_64.tar.gz" -C "$WORK" "Pigeon-$VERSION-linux-x86_64"

# ---- Linux: Debian and Ubuntu package
DEB=$WORK/deb
mkdir -p "$DEB/DEBIAN" "$DEB/usr/bin" "$DEB/usr/share/applications" \
    "$DEB/usr/share/icons/hicolor/256x256/apps" "$DEB/usr/share/icons/hicolor/512x512/apps"
install -m 755 "$LINUX_BIN" "$DEB/usr/bin/pigeon"
install -m 644 packaging/linux/pigeon.desktop "$DEB/usr/share/applications/pigeon.desktop"
install -m 644 assets/icon/pigeon-256.png "$DEB/usr/share/icons/hicolor/256x256/apps/pigeon.png"
install -m 644 assets/icon/pigeon-512.png "$DEB/usr/share/icons/hicolor/512x512/apps/pigeon.png"
SIZE=$(du -sk "$DEB/usr" | cut -f1)
cat > "$DEB/DEBIAN/control" <<CONTROL
Package: pigeon
Version: $VERSION
Section: devel
Priority: optional
Architecture: amd64
Installed-Size: $SIZE
Depends: libc6 (>= 2.35), libgcc-s1, git, libgl1, libxkbcommon0
Recommends: xdg-desktop-portal, zenity
Maintainer: Pigeon <noreply@example.com>
Description: Desktop API client whose collections sync through git
 Build and test API requests, and share them with your team
 through a git repository with pull and push.
CONTROL
dpkg-deb --root-owner-group --build "$DEB" "dist/pigeon_${VERSION}_amd64.deb" >/dev/null
