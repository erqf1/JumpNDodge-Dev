#!/bin/sh
# JumpNDodge (portables Linux-Paket) für den aktuellen Benutzer einrichten:
# kopiert das Spiel nach ~/.local/share/JumpNDodge (dort lag auch die alte Python-Version -
# ein vorhandener Spielstand bleibt erhalten) und legt einen Eintrag im Anwendungsmenü an.
# Entfernen: ./install.sh --uninstall
set -e
SRC="$(cd "$(dirname "$0")" && pwd)"
DIR="${XDG_DATA_HOME:-$HOME/.local/share}/JumpNDodge"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/256x256/apps"
if [ "$1" = "--uninstall" ]; then
  rm -f "$DIR/JumpNDodge" "$DIR/jumpndodge.png" "$APPS/jumpndodge.desktop" "$ICONS/jumpndodge.png"
  echo "JumpNDodge entfernt (Spielstand in $DIR/data bleibt erhalten)."
  exit 0
fi
mkdir -p "$DIR" "$APPS" "$ICONS"
if [ "$SRC" != "$DIR" ]; then
  cp "$SRC/JumpNDodge" "$DIR/JumpNDodge"
  cp "$SRC/jumpndodge.png" "$DIR/jumpndodge.png"
  cp "$SRC/install.sh" "$DIR/install.sh"
fi
chmod +x "$DIR/JumpNDodge" "$DIR/install.sh"
# Reste der Python-Version
rm -f "$DIR/updater" "$DIR/version.txt"
cp "$SRC/jumpndodge.png" "$ICONS/jumpndodge.png"
sed -e "s|^Exec=.*|Exec=\"$DIR/JumpNDodge\"|" -e "s|^Icon=.*|Icon=jumpndodge|" "$SRC/jumpndodge.desktop" > "$APPS/jumpndodge.desktop"
chmod +x "$APPS/jumpndodge.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" >/dev/null 2>&1 || true
echo "JumpNDodge ist installiert ($DIR) und im Anwendungsmenü zu finden."
