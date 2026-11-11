#!/bin/bash
# Baut die Linux-Pakete aus einem fertigen Release-Build:
#   jumpndodge_amd64.deb, jumpndodge-linux-x86_64.tar.gz, JumpNDodge-v<ver>-linux.zip (Archiv/alter Updater),
#   JumpNDodge-Server-linux-x86_64.zip und die Dateien für das Arch-Paket (arch-src/).
# Nutzung: packaging/linux/build-packages.sh <version> <ausgabeordner>   (im Ordner rust/, nach cargo build --release)
set -euo pipefail
VER=$1
OUT=$(mkdir -p "$2" && cd "$2" && pwd)
cd "$(dirname "$0")/../.."
BIN=target/release/JumpNDodge
SRV=target/release/JumpNDodge-Server
P=packaging
W=$(mktemp -d)

# --- .deb
D=$W/deb
install -Dm755 "$BIN" "$D/usr/bin/jumpndodge"
install -Dm644 $P/linux/jumpndodge.desktop "$D/usr/share/applications/jumpndodge.desktop"
for s in 16 32 48 64 128 256 512; do install -Dm644 "$P/icons/jumpndodge-$s.png" "$D/usr/share/icons/hicolor/${s}x${s}/apps/jumpndodge.png"; done
install -Dm644 LICENSE "$D/usr/share/doc/jumpndodge/copyright"
mkdir -p "$D/DEBIAN"
SIZE=$(du -sk "$D/usr" | cut -f1)
cat > "$D/DEBIAN/control" <<CTRL
Package: jumpndodge
Version: $VER
Section: games
Priority: optional
Architecture: amd64
Installed-Size: $SIZE
Depends: libc6 (>= 2.35), libasound2t64 | libasound2
Recommends: libxkbcommon-x11-0, libxkbcommon0, libx11-xcb1, libxcursor1, libxrandr2, libxi6, libwayland-client0
Maintainer: W Software <erqf1@users.noreply.github.com>
Homepage: https://mert.win/jump/
Description: JumpNDodge - springen, ausweichen, überleben
 Ein schnelles 2D-Ausweichspiel: acht Schwierigkeiten, Powers, Kosmetik,
 2-Spieler-Modus an einer Tastatur und Online-Multiplayer (VS und Koop).
 Rust Edition mit eingebautem Updater.
CTRL
dpkg-deb --build --root-owner-group "$D" "$OUT/jumpndodge_amd64.deb" >/dev/null

# --- tar.gz (portabel, mit install.sh)
T=$W/jumpndodge-linux-x86_64
mkdir -p "$T"
install -m755 "$BIN" "$T/JumpNDodge"
cp $P/icons/jumpndodge-256.png "$T/jumpndodge.png"
cp $P/linux/jumpndodge.desktop $P/linux/install.sh LICENSE "$T/"
chmod +x "$T/install.sh"
tar -C "$W" -czf "$OUT/jumpndodge-linux-x86_64.tar.gz" jumpndodge-linux-x86_64

# --- Archiv-Zip im alten Format (flach) - der alte JumpNDodgeUpdater der Python-Version
#     installiert darüber automatisch die Rust Edition
A=$W/archive
mkdir -p "$A"
install -m755 "$BIN" "$A/JumpNDodge"
cp ../icon.ico "$A/icon.ico"
echo "$VER" > "$A/version.txt"
(cd "$A" && zip -qr "$OUT/JumpNDodge-v$VER-linux.zip" .)

# --- Server
S=$W/JumpNDodge-Server-linux-x86_64
mkdir -p "$S"
install -m755 "$SRV" "$S/JumpNDodge-Server"
cp server/LIESMICH.txt server/docker-compose.yml LICENSE "$S/"
(cd "$W" && zip -qr "$OUT/JumpNDodge-Server-linux-x86_64.zip" JumpNDodge-Server-linux-x86_64)

# --- Quellen für makepkg (Arch-Job)
mkdir -p "$OUT/arch-src"
install -m755 "$BIN" "$OUT/arch-src/JumpNDodge"
cp $P/linux/jumpndodge.desktop LICENSE "$OUT/arch-src/"
cp $P/icons/jumpndodge-256.png "$OUT/arch-src/jumpndodge.png"
sed "s/^pkgver=.*/pkgver=$VER/" $P/arch/PKGBUILD > "$OUT/arch-src/PKGBUILD"

rm -rf "$W"
ls -la "$OUT"
