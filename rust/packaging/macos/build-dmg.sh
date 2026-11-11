#!/bin/bash
# Baut JumpNDodge.app, die DMG für den Updater/W Hub, ein Archiv-Zip und das Server-Zip.
# Nutzung (im Ordner rust/, nach cargo build --release): packaging/macos/build-dmg.sh <version> <apple-silicon|intel> <ausgabeordner>
set -euo pipefail
VER=$1; NAME=$2
OUT=$(mkdir -p "$3" && cd "$3" && pwd)
cd "$(dirname "$0")/../.."
P=packaging
W=$(mktemp -d)
APP="$W/dmg/JumpNDodge.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
install -m755 target/release/JumpNDodge "$APP/Contents/MacOS/JumpNDodge"
sed "s/@VERSION@/$VER/g" $P/macos/Info.plist > "$APP/Contents/Info.plist"
# Icon (.icns) aus den PNGs
IS="$W/JumpNDodge.iconset"
mkdir -p "$IS"
for s in 16 32 128 256 512; do
  sips -z $s $s $P/icons/jumpndodge-512.png --out "$IS/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2)); [ $d -le 512 ] && sips -z $d $d $P/icons/jumpndodge-512.png --out "$IS/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$IS" -o "$APP/Contents/Resources/JumpNDodge.icns"
cp LICENSE "$APP/Contents/Resources/"
# Ad-hoc-Signatur (ohne Apple-Entwicklerkonto): beim ersten Start Rechtsklick > Öffnen
codesign --force --deep -s - "$APP"
ln -s /Applications "$W/dmg/Applications"
hdiutil create -volname JumpNDodge -srcfolder "$W/dmg" -ov -format UDZO "$OUT/JumpNDodge-macos-$NAME.dmg" >/dev/null
# Archiv (portabel)
ditto -c -k --keepParent "$APP" "$OUT/JumpNDodge-v$VER-macos-$NAME.zip"
# Server
S="$W/JumpNDodge-Server-macos-$NAME"
mkdir -p "$S"
install -m755 target/release/JumpNDodge-Server "$S/JumpNDodge-Server"
cp server/LIESMICH.txt server/docker-compose.yml LICENSE "$S/"
(cd "$W" && zip -qr "$OUT/JumpNDodge-Server-macos-$NAME.zip" "JumpNDodge-Server-macos-$NAME")
rm -rf "$W"
ls -la "$OUT"
