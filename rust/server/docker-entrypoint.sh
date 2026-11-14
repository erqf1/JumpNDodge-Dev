#!/bin/sh
# Startet die neuere von zwei Server-Dateien:
#   /usr/local/bin/JumpNDodge-Server  (im Image)
#   /data/bin/JumpNDodge-Server       (vom Auto-Update geladen, liegt im Volume)
# Verglichen wird über "--version" (sort -V). Startet die /data-Datei nicht (z.B. falsche
# Architektur oder fehlende Bibliotheken), wird die Datei aus dem Image benutzt.
IMG=/usr/local/bin/JumpNDodge-Server
UPD=/data/bin/JumpNDodge-Server
BIN=$IMG

img_ver=$("$IMG" --version 2>/dev/null | head -n1)
if [ -x "$UPD" ]; then
    upd_ver=$("$UPD" --version 2>/dev/null | head -n1)
    if [ -n "$upd_ver" ]; then
        newest=$(printf '%s\n%s\n' "$img_ver" "$upd_ver" | sort -V | tail -n1)
        if [ "$newest" = "$upd_ver" ] && [ "$upd_ver" != "$img_ver" ]; then
            BIN=$UPD
        fi
    else
        echo "Update-Datei $UPD startet nicht - nehme die Version aus dem Image ($img_ver)"
    fi
fi

echo "Starte $BIN ($("$BIN" --version 2>/dev/null | head -n1))"
exec "$BIN" --config /data/server.json "$@"
