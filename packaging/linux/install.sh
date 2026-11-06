#!/bin/sh
# Installs Pigeon for the current user from this archive. No administrator rights needed.
set -eu
cd "$(dirname "$0")"

BIN=${XDG_BIN_HOME:-$HOME/.local/bin}
DATA=${XDG_DATA_HOME:-$HOME/.local/share}

mkdir -p "$BIN" "$DATA/applications" "$DATA/icons/hicolor/256x256/apps" "$DATA/icons/hicolor/512x512/apps"
cp pigeon "$BIN/pigeon"
chmod +x "$BIN/pigeon"
sed "s|^Exec=pigeon|Exec=$BIN/pigeon|" pigeon.desktop > "$DATA/applications/pigeon.desktop"
cp pigeon-256.png "$DATA/icons/hicolor/256x256/apps/pigeon.png"
cp pigeon-512.png "$DATA/icons/hicolor/512x512/apps/pigeon.png"

echo "Pigeon is installed. Find it in your applications menu, or run: $BIN/pigeon"
