# Offizieller JumpNDodge-Server auf CasaOS (play.mert.win)

Diese Anleitung richtet den **offiziellen** Server ein, mit dem sich das Spiel unter
„Offizieller Server“ verbindet (`play.mert.win`). Für einen eigenen, privaten Server reicht
die einfache `../docker-compose.yml` (nur UDP).

## 1. App in CasaOS installieren

1. CasaOS: **App-Store → „+“ → Benutzerdefinierte App → Importieren** → Inhalt von
   `docker-compose.yml` aus diesem Ordner einfügen → **Installieren**. (Kein Token nötig.)
2. In CasaOS bei der App auf **Logs** schauen. Da muss stehen:
   - `Starte /usr/local/bin/JumpNDodge-Server (5.5.x) als Benutzer 1000`
   - `Port: 47800 (UDP)` und `Web: 47801 (TCP, WebSocket …)`

Die Daten (`server.json`, Chat-Verlauf `chat.json`, Updates in `bin/`) liegen in
`/DATA/AppData/jumpndodge-server`. Den Ordner muss man **nicht** selbst anlegen oder freigeben –
der Container kümmert sich um die Rechte.

## 2. play.mert.win im vorhandenen Tunnel eintragen

Du hast schon einen Cloudflare Tunnel für mert.win. Dort kommt einfach ein Name dazu:

1. Cloudflare-Dashboard → **Zero Trust → Networks → Tunnels** → deinen Tunnel → **Public Hostnames
   → Add a public hostname**.
2. Subdomain `play`, Domain `mert.win`.
3. Service: Typ **HTTP**, URL = **lokale Adresse deines CasaOS-Rechners** + `:47801`
   (dieselbe Adresse, über die du CasaOS im Browser öffnest, nur mit Port 47801).
   - Läuft dein cloudflared direkt auf dem CasaOS-Rechner im Host-Netz, geht auch `localhost:47801`.
4. Speichern. WebSockets sind bei Cloudflare standardmäßig an (sonst: Domain → Network → WebSockets).

Gab es schon einen DNS-Eintrag `play` (z. B. graue Wolke von früher), diesen vorher löschen – der
Tunnel legt den richtigen (orange Wolke) selbst an.

**Lieber einen eigenen Tunnel nur für den Server?** `cloudflared-optional.yml` genauso importieren,
vorher den Token eintragen; Service-URL im Dashboard dann `http://localhost:47801`.

## 3. Wie sich das Spiel verbindet

Cloudflare leitet **kein UDP** weiter. Das Spiel versucht bei `play.mert.win` zuerst UDP; kommt
keine Antwort, weicht es **automatisch auf `wss://play.mert.win`** aus. Die Spieler müssen nichts
einstellen; die allererste Verbindung dauert dadurch ein paar Sekunden länger.

## 4. Optional: zusätzlich direkt per UDP

1. Im Router **UDP-Port 47800** an den CasaOS-Rechner weiterleiten.
2. In Cloudflare unter **DNS** einen eigenen Namen anlegen, z. B. `udp.mert.win` → deine öffentliche
   Adresse, **„Nur DNS“ (graue Wolke)**. Nicht `play.mert.win` nehmen – der läuft über den Tunnel.

## 5. Updates

- **Der Server aktualisiert sich selbst:** neue Version aus dem GitHub-Release, geprüft gegen
  `SHA256SUMS.txt`, abgelegt in `/data/bin`. Neu gestartet wird nur, **wenn niemand verbunden ist**.
- Verbindet sich jemand mit einer neueren Spielversion, sucht der Server sofort nach einem Update.

## Fehlersuche

| Problem | Lösung |
|---|---|
| App startet immer wieder neu | Logs ansehen. Seit 5.5.1 läuft der Server auch, wenn `/data` nicht beschreibbar ist – dann steht dort ein Hinweis. |
| Cloudflare zeigt „Bad gateway“ / 502 | Service-URL prüfen: Adresse des CasaOS-Rechners + `:47801`, Typ HTTP (nicht HTTPS). |
| Spiel: „Server antwortet nicht“ | In den Logs muss `Web: 47801` stehen; im Tunnel muss der Name `play` eingetragen sein. |
| Image lässt sich nicht laden (`denied`) | GitHub → Packages → jumpndodge-server → öffentlich stellen. |
