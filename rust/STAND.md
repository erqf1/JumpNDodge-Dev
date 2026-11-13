# Stand der Rust Edition

Letzte Aktualisierung: 2026-11-13 · Branch: `main`

## Fertig und veröffentlicht
- **Releases:** v5.0.0 bis v5.2.0 in **erqf1/JumpNDodge** (Latest) mit allen Paketen
  (Windows-Setup + Zip, .deb, Arch, tar.gz, macOS-DMG Apple Silicon/Intel, Server-Zips) und portable Zips in
  **erqf1/JumpNDodge-Archive**. W Hub **v1.2.1** (erqf1/whub) installiert/aktualisiert die Rust Edition, **v1.2.2** behebt
  hängende Downloads.
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
- **5.3.0 – neue Grafik (Stil C):** Alle Texturen sind jetzt Dateien in `assets/textures/` statt Code: flach wie die
  Python-Version, harte Schatten, Treppen-Ecken, scharf in jeder Auflösung (gerendert mit resvg, Crate `tex/` = jnd_tex).
  Quellen sind Pixel-Raster in `assets/textures/pix/*.pix` (Zeichen-Raster + Farbtabelle, siehe `tools/texturwerk/src/pix.rs`).
  Werkzeug **Texturwerk** (`cargo run -p texturwerk --release -- <befehl>`): `pix` (Raster → SVG), `check`, `bogen` (Vorschau aller
  Texturen), `szene` (Probe-Szene), `export`, `figur` (Spielfigur mit Cosmetics als PNG). Boden steht still, Bodenhindernisse
  haben Fahrtstreifen, Cosmetics/Spuren/Shop-Symbole als Texturen, Menüknöpfe und Panels im selben Stil mit Reinfahr- und
  Hover-Animation (`src/game/ui_c.rs`). **Einstellungen → Mehr → Legacy-Grafik** schaltet den alten Look zurück.
  Eigene Texturen: Ordner `textures/` neben dem Spiel (`player.png`, `cos_<name>.svg` …). Dev-Konsole: `run equip <cosmetic>`.
  Außerdem drin (war in 5.2.2 vorbereitet): Rangliste über mert.win, Multiplayer-Server play.mert.win.
- **Spielstand-Schlüssel (5.3):** steht nicht mehr im Code. `build.rs` setzt ihn aus `JND_SAVE_KEY` verschleiert ein
  (GitHub-Secret; Sicherung liegt außerhalb des Repos in `JumpNDodge/Geheim (nicht hochladen)/`). Release-Builds ohne Secret
  brechen ab (`JND_REQUIRE_SAVE_KEY=1`). Alte Stände werden mit dem alten Schlüssel gelesen und sofort neu gespeichert;
  den alten Schlüssel in ein paar Versionen ganz rausnehmen. Ohne Variable (Entwickler-Builds) gilt der alte Schlüssel.
- **5.2.0:** eingebaute Sounds (die Releases waren ohne eigene Sounddateien stumm), Mehr → Replays (letzte Runde + Rekordläufe).

## Was du selbst tun musst
1. **Website hochladen** (CasaOS): Ordner `website/jump/` aus diesem Repo nach `mert.win/jump/` kopieren
   (`index.html` im Aufbau der Line-Seiten, `img/` mit `shots/`, `tex/`, `figur/`; `style.css` + `site.js` braucht nur noch `server/index.html`). Für W Hub zusätzlich
   `website-update/hub/changelog-1.2.1.html` und `changelog-1.2.2.html` aus erqf1/whub oben in `mert.win/hub/changelog.html` einfügen.
2. **Offizieller Multiplayer-Server:** Im Spiel ist `play.mert.win` eingetragen (seit 5.2.2; jumpndodge.download gibt es nicht mehr).
   - Bei Cloudflare einen DNS-Eintrag `play` in der Zone mert.win (Typ A, deine öffentliche IPv4) anlegen – **„Nur DNS“ (graue Wolke)**,
     denn Cloudflare-Proxy/Tunnel leiten kein UDP weiter.
   - Im Router **UDP 47800** an den CasaOS-Rechner weiterleiten.
   - Auf CasaOS: App-Store → „+“ → Importieren → Inhalt von `rust/server/docker-compose.yml`.
     Das Image `ghcr.io/erqf1/jumpndodge-server` entsteht beim nächsten Release-Lauf (bei 5.0.0 scheiterte nur dieser Teil,
     ist behoben). Unter github.com/erqf1 → Packages → jumpndodge-server die Sichtbarkeit auf **Public** stellen,
     sonst kann CasaOS es nicht ohne Login laden. Alternativ das Server-Zip (Linux) direkt starten.
   - Ohne eigene IPv4 (DS-Lite) bitte beim Anbieter eine IPv4 anfragen – Details auf der Seite „Server hosten“.
3. **Rangliste über mert.win** (seit 5.2.2): Das Spiel ruft `https://mert.win/jump/api/...` auf. Die neue `nginx.conf` (Latein Trainer/latein-sync) leitet `/jump/api/` an den Ranglisten-Container (Port 5000 auf dem CasaOS-Rechner) weiter – hochladen, Container `latein-trainer` neu starten, Ranglisten-Container muss mit Port 5000 laufen. Der alte Tunnel für leaderboard.jumpndodge.download wird nicht mehr gebraucht.
4. **GitHub Actions:** Seit 11.11. starten keine Jobs mehr („recent account payments have failed or your spending limit needs
   to be increased“). Unter github.com → Settings → Billing & plans beheben, dann Release 5.3.0 per Run workflow starten.
5. **Linux-Nutzer der alten Python-Version** wechseln nicht automatisch (der alte Python-Updater verliert unter Linux das
   Ausführungsrecht): einmal `.deb`/Arch-Paket/`tar.gz` installieren – der Spielstand wird übernommen. Windows wechselt automatisch.
   Ab 5.0.1 heißt das Linux-Zip im Archiv `*-linux-portable.zip`, der alte Updater lässt Linux dann in Ruhe (geprüft).
   Nur wer zwischen 07:11 und 07:20 Uhr (11.11.) die alte Linux-Version gestartet hat, bekam 5.0.0 ohne
   Ausführungsrecht – Abhilfe: `chmod +x ~/.local/share/JumpNDodge/JumpNDodge` oder das .deb installieren.
6. Optional: macOS-Apps sind nur ad-hoc signiert (Rechtsklick → Öffnen). Für „normales“ Öffnen bräuchte es ein
   Apple-Entwicklerkonto (99 €/Jahr) zum Beglaubigen.

## Hinweise
- **W Hub Selbsttest unter Windows** hing zeitweise bei „abgebrochene Download-Stücke neu laden“. Ursache: hängende
  Verbindungen ohne Fehler. Behoben in 1.2.2 (Wächter: 15 s ohne Daten → Stück neu laden), danach 2× hintereinander grün.
- Die Release-Läufe starten per Actions → „Rust Edition – Build & Release“ → Run workflow (Branch wählen, tag = vX.Y.Z).

## Offen / Ideen für später
- Lua-Mods: eigene Powers im Shop.
- Restliche Bildschirme (Online-Hub, Rangliste, Chat) zeichnen Knöpfe teils noch selbst im alten Stil.
- Online: Bestenliste für Online-Runden, Zuschauen.
- GPU-Ausgabe (aktuell CPU über softbuffer – flüssig auch in 4K, aber eine GPU-Ausgabe würde Strom sparen).
