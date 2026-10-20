#!/bin/sh
# Installs MX Shooting Calculator for this user: the program in ~/.local/bin,
# and a menu entry with its icon. Run it from the unpacked folder.
set -e
here=$(dirname "$0")
data=${XDG_DATA_HOME:-$HOME/.local/share}
mkdir -p "$HOME/.local/bin" "$data/applications" "$data/icons/hicolor/256x256/apps"
install -m 755 "$here/mxsc" "$HOME/.local/bin/mxsc"
install -m 644 "$here/mxsc.png" "$data/icons/hicolor/256x256/apps/mxsc.png"
install -m 644 "$here/mxsc.desktop" "$data/applications/mxsc.desktop"
echo "Installed. Start it from the app menu, or run: mxsc"
