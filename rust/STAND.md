# Stand der Rust Edition

Letzte Aktualisierung: 2026-11-11 · Arbeits-Branch: `claude/dazzling-hopper-gg013f` (enthält auch `main`)

## Fertig und veröffentlicht
- **Releases:** v5.0.0, v5.0.1 und v5.1.0 in **erqf1/JumpNDodge** (Latest) mit allen Paketen
  (Windows-Setup + Zip, .deb, Arch, tar.gz, macOS-DMG Apple Silicon/Intel, Server-Zips) und portable Zips in
  **erqf1/JumpNDodge-Archive**. W Hub **v1.2.1** (erqf1/whub) installiert/aktualisiert die Rust Edition.
- **A – Python-Altlasten:** scharfes Zeichnen in echter Pixelauflösung (125/150 %, Vollbild), feste Logikrate 60/s +
  Interpolation, Monitor-Bildrate in Menüs, zeitbasierte Überblendung, Sprung-Tipps gehen nicht verloren, Auto-Pause,
  Vollbild-Schalter (F11), Spielstand im Hintergrund schreiben, kurzer Start, System-Zertifikate/Proxy.
- **B – Online-Multiplayer:** Phase 3 (Bildschirme, Vorhersage/Interpolation, Rematch), Phase 4 (Docker-Image,
  CasaOS-Compose, Website „Server hosten“), Phase 5 (`tests/mp_e2e.sh`, läuft in der CI).
- **C – Bildrate/Vollbild:** Monitor-Hz, FPS-Auswahl, scharfes Vollbild.
- **D – Rust Edition als Hauptversion:** eingebauter Updater (echt getestet gegen das veröffentlichte Release),
  Installer/Pakete für alle Systeme mit Übernahme des Spielstands, Workflows, Website `website/jump/`, W-Hub-Katalog,
  READMEs in erqf1/JumpNDodge und erqf1/JumpNDodge-Archive.
- **E – Grafik:** neue Spielfigur (Glanz, Squash & Stretch), Hindernisse, Kristall-Fallblöcke, 1-UP-Herz, Boden,
  Hintergrund-Verlauf; nie Orange und Grün zusammen.
- **F – Extras:** Effekte (Partikel, Wackeln), Replays + Rekord-Geist, Controller (Xbox/PlayStation, 2 Controller für 2P),
  sichere Lua-Mods (Hindernisse, Spielwerte, Modi, Ereignisse; Anleitung `LUA_MODS.md`).

## Was du selbst tun musst
1. **Website hochladen** (CasaOS): Ordner `website/jump/` aus diesem Repo nach `mert.win/jump/` kopieren
   (`index.html`, `style.css`, `site.js`, `img/`, `server/index.html`). Für W Hub zusätzlich
   `website-update/hub/changelog-1.2.1.html` aus erqf1/whub oben in `mert.win/hub/changelog.html` einfügen.
2. **Offizieller Multiplayer-Server:** Im Spiel ist `play.jumpndodge.download` eingetragen.
   - Bei Cloudflare einen DNS-Eintrag `play` (Typ A, deine öffentliche IPv4) anlegen – **„Nur DNS“ (graue Wolke)**,
     denn Cloudflare-Proxy/Tunnel leiten kein UDP weiter.
   - Im Router **UDP 47800** an den CasaOS-Rechner weiterleiten.
   - Auf CasaOS: App-Store → „+“ → Importieren → Inhalt von `rust/server/docker-compose.yml`.
     Das Image `ghcr.io/erqf1/jumpndodge-server` entsteht beim nächsten Release-Lauf (bei 5.0.0 scheiterte nur dieser Teil,
     ist behoben). Unter github.com/erqf1 → Packages → jumpndodge-server die Sichtbarkeit auf **Public** stellen,
     sonst kann CasaOS es nicht ohne Login laden. Alternativ das Server-Zip (Linux) direkt starten.
   - Ohne eigene IPv4 (DS-Lite) bitte beim Anbieter eine IPv4 anfragen – Details auf der Seite „Server hosten“.
3. **Cloudflare-Tunnel der Rangliste** (Fehler 1033): auf dem Heimserver den `cloudflared`-Dienst/Container neu starten.
4. **Nachfolge-Routine „JumpNDodge Rust Edition – weitermachen nach Limit-Reset“** (claude.ai → Routinen):
   Sie schaut auf `main` und kennt diesen Branch nicht. Entweder abschalten (alles ist erledigt) oder diesen Branch nach
   `main` übernehmen (Pull Request). Ich durfte sie nicht ändern, weil du sie angelegt hast.
5. **Linux-Nutzer der alten Python-Version** wechseln nicht automatisch (der alte Python-Updater verliert unter Linux das
   Ausführungsrecht): einmal `.deb`/Arch-Paket/`tar.gz` installieren – der Spielstand wird übernommen. Windows wechselt automatisch.
6. Optional: macOS-Apps sind nur ad-hoc signiert (Rechtsklick → Öffnen). Für „normales“ Öffnen bräuchte es ein
   Apple-Entwicklerkonto (99 €/Jahr) zum Beglaubigen.

## Offen / Ideen für später
- Replays der Rekordläufe direkt aus dem Statistik-Menü ansehen (Dateien liegen schon in `data/replays/`).
- Lua-Mods: eigene Powers im Shop, eigene Grafiken.
- Online: Bestenliste für Online-Runden, Zuschauen.
- GPU-Ausgabe (aktuell CPU über softbuffer – flüssig auch in 4K, aber eine GPU-Ausgabe würde Strom sparen).
