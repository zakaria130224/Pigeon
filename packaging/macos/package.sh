#!/bin/sh
# Builds Pigeon.app and a disk image into dist/. Run on a Mac.
#
# Optional, for a build other people can open without a warning:
#   PIGEON_MACOS_SIGN_IDENTITY  e.g. "Developer ID Application: Your Name (TEAMID)"
#   PIGEON_NOTARY_PROFILE       a profile saved with: xcrun notarytool store-credentials
set -eu
cd "$(dirname "$0")/../.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH=$(uname -m)
IDENTITY=${PIGEON_MACOS_SIGN_IDENTITY:--}
STAGE=dist/macos
APP=$STAGE/Pigeon.app
DMG=dist/Pigeon-$VERSION-macos-$ARCH.dmg

cargo build --release --locked -p pigeon-app

rm -rf "$STAGE"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/pigeon "$APP/Contents/MacOS/pigeon"
cp assets/icon/pigeon.icns "$APP/Contents/Resources/pigeon.icns"
cp assets/fonts/FONT-LICENSES.txt "$APP/Contents/Resources/FONT-LICENSES.txt"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

if [ "$IDENTITY" = "-" ]; then
    echo "Signing for this computer only (no Developer ID given)."
    codesign --force --sign - "$APP"
else
    codesign --force --options runtime --timestamp --sign "$IDENTITY" "$APP"
fi
codesign --verify --strict "$APP"

ln -s /Applications "$STAGE/Applications"
hdiutil create -quiet -volname "Pigeon $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG"

if [ -n "${PIGEON_NOTARY_PROFILE:-}" ]; then
    codesign --force --timestamp --sign "$IDENTITY" "$DMG"
    xcrun notarytool submit "$DMG" --keychain-profile "$PIGEON_NOTARY_PROFILE" --wait
    xcrun stapler staple "$DMG"
fi

echo "Done: $DMG"
