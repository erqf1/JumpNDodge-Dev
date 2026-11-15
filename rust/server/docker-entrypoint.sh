#!/bin/sh
# Start des Servers im Container.
#
# 1. Rechte: CasaOS & Co. legen den Datenordner (/data) als root an. Gestartet wird deshalb als root,
#    /data wird dem Benutzer jnd gegeben und dann als jnd weitergemacht (setpriv). Klappt das nicht
#    (z.B. Netzlaufwerk ohne chown), läuft der Server als root weiter - Hauptsache, er läuft.
# 2. Version: nimmt die neuere von zwei Server-Dateien:
#      /usr/local/bin/JumpNDodge-Server  (im Image)
#      /data/bin/JumpNDodge-Server       (vom Auto-Update geladen, liegt im Volume)
#    Verglichen wird über "--version" (sort -V). Startet die /data-Datei nicht (falsche Architektur,
#    fehlende Bibliotheken), wird die Datei aus dem Image benutzt.
# 3. Cloudflare Tunnel: macht der Server selbst (TUNNEL_TOKEN bzw. "tunnel_token" in server.json);
#    cloudflared liegt schon im Image, damit nichts geladen werden muss.

if [ "$(id -u)" = "0" ]; then
    mkdir -p /data/bin 2>/dev/null
    if chown -R 1000:1000 /data 2>/dev/null && command -v setpriv >/dev/null 2>&1; then
        exec setpriv --reuid=1000 --regid=1000 --init-groups "$0" "$@"
    fi
    echo "Hinweis: /data lässt sich nicht an den Benutzer jnd übergeben - der Server läuft als root."
fi

if ! ( touch /data/.schreibtest && rm -f /data/.schreibtest ) 2>/dev/null; then
    echo "Hinweis: /data ist nicht beschreibbar - server.json, Chat-Verlauf und Auto-Update gehen nicht, der Server läuft trotzdem."
fi

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

echo "Starte $BIN ($("$BIN" --version 2>/dev/null | head -n1)) als Benutzer $(id -u)"
exec "$BIN" --config /data/server.json "$@"
