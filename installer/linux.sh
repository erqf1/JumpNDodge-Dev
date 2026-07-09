#!/bin/bash
# JumpNDodge Linux-Installer.
# Erwartet JumpNDodge, updater, icon.ico und version.txt im selben Ordner wie
# dieses Skript (z.B. das entpackte Release-Zip, dieses Skript daneben kopiert).
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="$HOME/.local/share/JumpNDodge"
DESKTOP_FILE="$HOME/.local/share/applications/jumpndodge.desktop"

for f in JumpNDodge updater icon.ico version.txt; do
    if [ ! -f "$SCRIPT_DIR/$f" ]; then
        echo "Fehler: $f nicht neben diesem Skript gefunden." >&2
        exit 1
    fi
done

VERSION=$(cat "$SCRIPT_DIR/version.txt")
echo "Installiere JumpNDodge $VERSION nach $INSTALL_DIR ..."

mkdir -p "$INSTALL_DIR"
cp "$SCRIPT_DIR/JumpNDodge" "$SCRIPT_DIR/updater" "$SCRIPT_DIR/icon.ico" "$SCRIPT_DIR/version.txt" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/JumpNDodge" "$INSTALL_DIR/updater"

mkdir -p "$(dirname "$DESKTOP_FILE")"
cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=JumpNDodge
Comment=$VERSION - https://discord.gg/rPmc3YR5gR
Exec=$INSTALL_DIR/updater
Icon=$INSTALL_DIR/icon.ico
Categories=Game;
Terminal=false
EOF
chmod +x "$DESKTOP_FILE"

cat > "$INSTALL_DIR/uninstall.sh" <<EOF
#!/bin/bash
rm -rf "$INSTALL_DIR"
rm -f "$DESKTOP_FILE"
rm -f "$HOME/Desktop/jumpndodge.desktop"
echo "JumpNDodge deinstalliert."
EOF
chmod +x "$INSTALL_DIR/uninstall.sh"

echo "Fertig. JumpNDodge ist im Anwendungsmenü zu finden."

read -p "Desktop-Icon erstellen? [j/N] " create_desktop
if [[ "$create_desktop" =~ ^[jJ]$ ]]; then
    mkdir -p "$HOME/Desktop"
    cp "$DESKTOP_FILE" "$HOME/Desktop/jumpndodge.desktop"
    chmod +x "$HOME/Desktop/jumpndodge.desktop"
    echo "Desktop-Icon erstellt."
fi

read -p "JumpNDodge jetzt starten? [J/n] " launch_now
if [[ ! "$launch_now" =~ ^[nN]$ ]]; then
    "$INSTALL_DIR/updater" &
fi
