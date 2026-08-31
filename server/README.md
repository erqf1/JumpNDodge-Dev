# JumpNDodge Online-Leaderboard-Server

Kleiner Flask + SQLite Service für das Online-Leaderboard. Läuft komplett
unabhängig vom Spiel, keine Cloud-Abhängigkeit - gedacht für den eigenen
CasaOS-Heimserver.

## Deployment auf CasaOS (ohne Terminal)

GitHub Actions baut das Image automatisch und veröffentlicht es nach GitHub
Container Registry (ghcr.io) - CasaOS kann es dann direkt per Klick
installieren, ganz ohne SSH/Terminal auf dem Heimserver.

**Einmalig einrichten:**

1. Diesen `server/`-Ordner (inkl. `.github/workflows/build-leaderboard-server.yml`)
   nach GitHub pushen. Der Workflow "Build & Publish Leaderboard Server" baut
   danach automatisch das Image und lädt es nach
   `ghcr.io/erqf1/jumpndodge-leaderboard` hoch (Fortschritt im "Actions"-Tab
   des Repos auf github.com).
2. **Einmalig CasaOS für das (private) Image anmelden.** Das Package bleibt
   damit privat - "public" würde hier nur heissen, dass irgendwer das
   Docker-Image (den Bauplan) herunterladen könnte, nicht dass Highscores/
   Spielerdaten öffentlich wären (die liegen sowieso nur auf dem Heimserver),
   aber privat ist trotzdem der sauberere Default:
   1. Auf github.com einen Personal Access Token erstellen: Profilbild ->
      *Settings* -> *Developer settings* -> *Personal access tokens* ->
      *Tokens (classic)* -> *Generate new token (classic)*, Häkchen bei
      `read:packages`, Token generieren und kopieren.
   2. In CasaOS das eingebaute Terminal öffnen (im Dashboard, browserbasiert -
      kein separates Programm nötig) und einmalig:
      ```bash
      docker login ghcr.io -u erqf1 -p DEIN_TOKEN
      ```
   3. Ab jetzt kann dieser Server das private Image jederzeit ziehen.

   *Alternative, falls das eingebaute Terminal nicht verfügbar ist:* Package
   stattdessen einmalig auf öffentlich stellen (*Profil -> Packages ->
   jumpndodge-leaderboard -> Package settings -> Danger Zone -> Change
   visibility -> Public*) - dann entfällt der Login-Schritt komplett.

**In CasaOS installieren:**

1. CasaOS öffnen -> *App Store* -> oben rechts *Custom Install* -> *Import*.
2. Den Inhalt von [`docker-compose.yml`](docker-compose.yml) (aus diesem
   Ordner) einfügen/hochladen und bestätigen.
3. CasaOS lädt das Image von ghcr.io und startet den Container - läuft danach
   dauerhaft (`restart: unless-stopped`) auf Port `5000` und erscheint als
   normale App im CasaOS-Dashboard.

**Updates**: Sobald sich etwas in `server/` ändert und auf GitHub gepusht
wird, baut die Action automatisch ein neues Image. In CasaOS dann bei der App
auf *Update* klicken (oder die App neu installieren), damit die neue Version
gezogen wird.

## Alternative: mit Terminal/SSH-Zugriff

Falls doch mal SSH-Zugriff aufs CasaOS zur Verfügung steht, geht es auch ganz
ohne GitHub/Registry - `server/`-Ordner auf den Server kopieren und dort:

```bash
cd server
docker compose up -d --build
```

(Manche älteren Docker-Installationen kennen nur `docker-compose` mit
Bindestrich statt `docker compose` als Plugin - im Zweifel beides probieren.)

Für Zugriff von ausserhalb des Heimnetzes braucht der Router in jedem Fall
noch Port-Forwarding (extern -> Port 5000 auf die Heimserver-IP) oder eine
DynDNS/Tunnel-Lösung - das ist eure Netzwerk-Konfiguration und hier nicht
weiter dokumentiert.

Danach in `JumpNDodge.py` `ONLINE_SERVER_URL` auf die von aussen erreichbare
Adresse setzen (z.B. `http://eure-adresse.dyndns.org:5000`).

## Daten & Wartung

- Die SQLite-Datenbank (`/app/data` im Container) liegt in einem benannten
  Docker-Volume (`leaderboard_data`) - übersteht also Image-Updates/Neustarts,
  unabhängig davon, ob per CasaOS-GUI oder Terminal installiert.
- `banned_words.txt` (Namensfilter-Blockliste) ist fest im Image enthalten.
  Ändern: Datei im Repo bearbeiten, pushen, Action baut neues Image, in
  CasaOS die App aktualisieren. Die Startliste ist bewusst nur ein kleiner
  Grundstock (DE/EN) - je nach Community selbst erweitern.

## API

| Methode | Pfad                     | Zweck                                          |
|---------|--------------------------|-------------------------------------------------|
| POST    | `/api/submit`            | Score + Profil-Snapshot einreichen (Token-Auth) |
| GET     | `/api/leaderboard`       | Rangliste für `?mode=&difficulty=`              |
| GET     | `/api/players/search`    | Namenssuche `?q=`                               |
| GET     | `/api/players/<name>`    | Öffentliches Profil + alle Bucket-Highscores    |
| GET     | `/api/banned_words`      | Aktuelle Blockliste (für Client-seitigen Cache) |
