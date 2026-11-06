#!/bin/sh
# Clears dist/ and builds the macOS, Windows and Linux packages into it, with checksums.
# Run on a Mac with Docker running.
#
#   ./packaging/build-all.sh           reuses the compile caches, so only changed code is rebuilt
#   ./packaging/build-all.sh --clean   also clears the compile caches and recompiles everything
set -eu
cd "$(dirname "$0")/.."

CLEAN=0
case "${1:-}" in
    "") ;;
    --clean) CLEAN=1 ;;
    *)
        echo "Usage: $0 [--clean]" >&2
        exit 2
        ;;
esac

# Check everything first, so a failed start leaves the existing packages in place.
if [ "$(uname -s)" != "Darwin" ]; then
    echo "The macOS package can only be built on a Mac. Run this script on a Mac." >&2
    exit 1
fi
if ! docker info >/dev/null 2>&1; then
    echo "Docker isn't running. Start Docker Desktop, then run this script again." >&2
    exit 1
fi

if [ -n "$(git status --porcelain)" ]; then
    echo "Note: you have uncommitted changes, and they will be included in these packages."
fi

echo "==> Clearing old packages"
rm -rf dist

if [ "$CLEAN" = 1 ]; then
    echo "==> Clearing compile caches"
    cargo clean
    if docker volume inspect pigeon-target >/dev/null 2>&1; then
        docker volume rm pigeon-target >/dev/null
    fi
fi

echo "==> Building for macOS"
./packaging/macos/package.sh

echo "==> Building for Windows and Linux"
./packaging/build-windows-linux.sh

echo "==> Writing checksums"
(cd dist && shasum -a 256 ./*.dmg ./*.exe ./*.zip ./*.tar.gz ./*.deb | sed 's|  \./|  |' > SHA256SUMS.txt)

echo
echo "All packages are in dist/:"
ls -lh dist
