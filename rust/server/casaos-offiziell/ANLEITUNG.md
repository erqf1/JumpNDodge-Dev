# Offizieller JumpNDodge-Server auf CasaOS (play.mert.win)

Diese Anleitung richtet den **offiziellen** Server ein, mit dem sich das Spiel unter
„Offizieller Server“ verbindet (`play.mert.win`). Für einen eigenen, privaten Server reicht
die einfache `../docker-compose.yml` (nur UDP).

Die Compose-Datei startet zwei Container:

| Dienst | Aufgabe |
|---|---|
| `jumpndodge-server` | Der Spielserver (Image `ghcr.io/erqf1/jumpndodge-server:latest`). UDP 47800 für direkte Verbindungen, WebSocket auf 47801 (nur intern für den Tunnel). |
| `cloudflared` | Cloudflare Tunnel: macht den WebSocket-Port unter `play.mert.win` erreichbar, ohne Portfreigabe im Router. |

## 1. Tunnel in Cloudflare anlegen

1. Im Cloudflare-Dashboard **Zero Trust → Networks → Tunnels → Create a tunnel** öffnen.
2. Typ **Cloudflared** wählen, einen Namen vergeben (z. B. `jumpndodge`).
3. Bei „Install and run a connector“ **Docker** auswählen. Im angezeigten Befehl steht hinter
   `--token` eine lange Zeichenkette – das ist der **Tunnel-Token**. Kopieren, aber **nirgends
   ins Repo** schreiben.
4. Weiter zu **Public Hostnames → Add a public hostname**:
   - Subdomain: `play`, Domain: `mert.win`
   - Service: Typ **HTTP**, URL **`jumpndodge-server:47801`**
     (also `http://jumpndodge-server:47801` – der Containername, nicht `localhost`)
   - Speichern. Cloudflare legt den DNS-Eintrag `play.mert.win` (orange Wolke) automatisch an.

WebSockets sind bei Cloudflare standardmäßig an. Falls nicht: Domain → **Network → WebSockets** einschalten.

## 2. App in CasaOS installieren

1. `docker-compose.yml` aus diesem Ordner öffnen und bei `TUNNEL_TOKEN: ""` den Token zwischen
   die Anführungszeichen setzen. (Alternativ erst importieren und den Token danach in CasaOS
   bei der App unter „Einstellungen → Umgebungsvariablen“ eintragen.)
2. CasaOS: **App-Store → „+“ → Benutzerdefinierte App → Importieren** → Inhalt einfügen → Installieren.
3. Prüfen: In den Logs von `jumpndodge-server` steht
   `Web: 47801 (TCP, WebSocket …)`, in den Logs von `cloudflared` steht `Registered tunnel connection`.
   Im Cloudflare-Dashboard ist der Tunnel dann **Healthy**.

Die Daten (Einstellungen `server.json`, Updates) liegen in `/DATA/AppData/jumpndodge-server`.

## 3. Wie sich das Spiel verbindet

Cloudflare leitet **kein UDP** weiter. Das Spiel versucht bei `play.mert.win` zuerst UDP; kommt
keine Antwort, weicht es **automatisch auf `wss://play.mert.win`** (WebSocket über den Tunnel) aus.
Die Spieler müssen dafür nichts einstellen. Die allererste Verbindung kann deshalb ein paar
Sekunden länger dauern.

## 4. Optional: zusätzlich direkt per UDP

Wer möglichst kurze Wege will, kann den Server zusätzlich direkt per UDP erreichbar machen:

1. Im Router **UDP-Port 47800** an den CasaOS-Rechner weiterleiten (die Compose-Datei
   veröffentlicht 47800/udp schon).
2. In Cloudflare unter **DNS** einen eigenen Eintrag anlegen, z. B. `udp.mert.win` → deine
   öffentliche Adresse, Proxy-Status **„Nur DNS“ (graue Wolke)**.
3. Spieler können dann `udp.mert.win` als Server eintragen.

Wichtig: Dafür **nicht** `play.mert.win` nehmen – dieser Name läuft über den Tunnel (orange
Wolke) und muss so bleiben. Bei einer wechselnden Adresse zu Hause braucht der UDP-Name ein
DynDNS-Update (z. B. einen zusätzlichen DDNS-Container); der Tunnel braucht das nicht.

## 5. Updates

- **Der Server aktualisiert sich selbst** (`JND_SERVER_AUTO_UPDATE=1`): Er schaut regelmäßig ins
  neueste GitHub-Release, lädt die passende Datei, prüft sie gegen `SHA256SUMS.txt` und legt sie
  nach `/data/bin`. Neu gestartet wird erst, **wenn niemand verbunden ist**. Docker startet den
  Container neu, und das Startskript nimmt die neuere Version (Image oder `/data/bin`).
  Startet die geladene Datei nicht, läuft automatisch die Version aus dem Image weiter.
- Ein neues Image (`:latest`) holt CasaOS bei „Aktualisieren“ bzw. ein Watchtower-Container –
  nötig ist das wegen des Auto-Updates aber nicht.
- Spieler mit anderer Spielversion können mitspielen, solange das **Netzwerk-Protokoll** gleich ist.
  Nur bei einem anderen Protokoll bekommt man „bitte aktualisieren“.

## 6. Einmalig auf GitHub

Das Paket **`jumpndodge-server`** (GitHub → Profil → Packages → jumpndodge-server →
Package settings → Change visibility) muss **öffentlich** sein, sonst kann CasaOS das Image nicht
ohne Anmeldung laden. Das Image wird beim Veröffentlichen eines Releases automatisch gebaut.

## Fehlersuche

| Problem | Lösung |
|---|---|
| Tunnel „Down“ / `cloudflared` startet neu | Token falsch oder leer – `TUNNEL_TOKEN` prüfen. |
| Cloudflare zeigt „Bad gateway“ / 502 | Service-URL muss `http://jumpndodge-server:47801` sein (Containername), beide Container in derselben App. |
| Spiel: „Server antwortet nicht“ | Logs von `jumpndodge-server` ansehen; steht dort `WebSocket-Port 47801 … nicht verfügbar`, ist der Port belegt. |
| Image lässt sich nicht laden (`denied`) | Paket auf GitHub öffentlich stellen (Punkt 6). |
