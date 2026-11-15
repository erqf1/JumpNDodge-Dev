# Offizieller JumpNDodge-Server auf CasaOS (play.mert.win)

Diese Anleitung richtet den **offiziellen** Server ein, mit dem sich das Spiel unter
„Offizieller Server“ verbindet (`play.mert.win`). Der Cloudflare Tunnel ist **im Server-Container
eingebaut** – eine App, ein Container, keine Portweiterleitung nötig.

## 1. Tunnel in Cloudflare anlegen

1. Cloudflare-Dashboard → **Zero Trust → Networks → Tunnels → Create a tunnel** → **Cloudflared**.
2. Name z. B. `jumpndodge`. Danach zeigt Cloudflare einen Docker-Befehl mit `--token eyJ…` –
   den langen Teil nach `--token` kopieren. **Geheim halten** (nicht ins Repo, nicht teilen).
3. Weiter zu **Public Hostnames → Add a public hostname**:
   - Subdomain `play`, Domain `mert.win`
   - Service: Typ **HTTP**, URL **`localhost:47801`**
   - Speichern.

Steht `play.mert.win` schon in einem **anderen** Tunnel (z. B. dem für mert.win) oder gibt es unter
**DNS** einen alten `play`-Eintrag: diesen vorher löschen – ein Name kann nur auf einen Tunnel zeigen.

## 2. App in CasaOS installieren

1. CasaOS: **App-Store → „+“ → Benutzerdefinierte App → Importieren** → Inhalt von
   `docker-compose.yml` aus diesem Ordner einfügen.
2. Bei **TUNNEL_TOKEN** den kopierten Token eintragen (in CasaOS unter den Umgebungsvariablen der App,
   oder vor dem Importieren direkt im Text zwischen die Anführungszeichen).
3. **Installieren**.

## 3. Prüfen

In CasaOS bei der App auf **Logs**. Da muss stehen:

- `Cloudflare Tunnel: wird gestartet`
- `[Tunnel] … Registered tunnel connection` (meist 4×)
- `Starte /usr/local/bin/JumpNDodge-Server (…) als Benutzer 1000`
- `Port: 47800 (UDP)` und `Web: 47801 (TCP, WebSocket …)`

Im Cloudflare-Dashboard steht der Tunnel dann auf **HEALTHY**. Im Spiel zeigt der offizielle
Server „online“.

Die Daten (`server.json`, Chat-Verlauf, Replays, Updates in `bin/`) liegen in
`/DATA/AppData/jumpndodge-server`. Den Ordner muss man nicht anlegen – der Container kümmert sich.

## Ohne eingebauten Tunnel

Läuft schon ein cloudflared (z. B. für mert.win), kann `TUNNEL_TOKEN` leer bleiben. Dann im
vorhandenen Tunnel `play.mert.win` → **HTTP** → **Adresse des CasaOS-Rechners:47801** eintragen
(dieselbe Adresse, mit der du CasaOS öffnest, nur mit Port 47801).

## Wie sich das Spiel verbindet

Cloudflare leitet **kein UDP** weiter. Das Spiel versucht bei `play.mert.win` zuerst UDP; kommt
keine Antwort, weicht es **automatisch auf `wss://play.mert.win`** aus. Die Spieler müssen nichts
einstellen. Replays (Freundes-Geister, Teilen) laufen über `https://play.mert.win/replays/…`.

Optional zusätzlich direkt per UDP: im Router UDP 47800 an den CasaOS-Rechner weiterleiten und in
Cloudflare einen **eigenen** Namen (z. B. `udp.mert.win`, „Nur DNS“ / graue Wolke) anlegen.

## Updates

Der Server aktualisiert sich selbst (GitHub-Release, geprüft gegen `SHA256SUMS.txt`, abgelegt in
`/data/bin`) und startet nur neu, **wenn niemand verbunden ist**. Der Tunnel kommt dabei mit.
Neue Image-Versionen (z. B. ein neueres cloudflared) gibt es in CasaOS über **Aktualisieren**.

## Fehlersuche

| Problem | Lösung |
|---|---|
| Log: `[Tunnel] … beendet - neuer Versuch` | Token falsch oder unvollständig kopiert – nochmal den Teil nach `--token` nehmen. |
| Cloudflare zeigt 502 / Bad gateway | Service im Public Hostname muss **HTTP** und **`localhost:47801`** sein (nicht HTTPS). |
| Spiel: „Server antwortet nicht“ | In den Logs muss `Web: 47801` stehen; der Tunnel muss HEALTHY sein. |
| App startet immer wieder neu | Logs ansehen – der Tunnel selbst bringt den Container nie zum Absturz. |
| Image lässt sich nicht laden (`denied`) | GitHub → Packages → jumpndodge-server → öffentlich stellen. |
