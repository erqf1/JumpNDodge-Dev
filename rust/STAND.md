# Stand der Rust Edition

Letzte Aktualisierung: 2026-11-11 · Arbeits-Branch: `claude/dazzling-hopper-gg013f` (enthält auch `main`)

## Fertig
- **Port** 1:1 von JumpNDodge.py (außer Mods), Spielstand in beide Richtungen kompatibel.
- **A – Python-Altlasten** (Details in PORT_NOTES.md):
  - Scharfes Zeichnen in echter Pixelauflösung (Windows-Skalierung 125/150 %, Vollbild) statt unscharfem Hochziehen.
  - Spielrunden mit fester Logikrate 60/s + Interpolation; Menüs mit Monitor-Bildrate; Überblendung zeitbasiert.
  - Kurze Sprung-Tipps gehen nicht mehr verloren, Auto-Pause bei Fokusverlust, Vollbild-Schalter (auch F11).
  - Spielstand wird im Hintergrund geschrieben (kein Ruckeln), künstliche 2-s-Ladezeit auf 0,6 s.
  - HTTPS mit System-Zertifikaten und Proxy aus der Umgebung.
- **C – Bildrate/Vollbild:** Monitor-Hz (144/165 …), FPS-Auswahl unter Einstellungen → Mehr.
- **B – Online-Multiplayer:**
  - Phase 3: Online → Multiplayer (Serverauswahl mit Status, per IP, zuletzt genutzt, Lobby, Räume VS/Koop + Schwierigkeit,
    Code, Bereit, Countdown, Runde mit Vorhersage/Interpolation, Ergebnis + Rematch, keine Pause online, Versionsprüfung).
  - Phase 4: Docker-Image (`rust/server/Dockerfile`, `docker-compose.yml` mit CasaOS-Daten), Website-Seite „Server hosten“.
  - Phase 5: `rust/tests/mp_e2e.sh` – echter Server + zwei unsichtbare Spiele, zwei Runden inkl. Rematch (läuft in der CI).
- **D – Rust Edition 5.0.0 als Hauptversion:**
  - Eingebauter Updater (`src/updater.rs`, `src/game/update_ui.rs`) wie bei Vidline – ersetzt den alten JumpNDodgeUpdater.
  - Pakete: Windows-Setup (gleiche AppId wie bisher → Update an Ort und Stelle), Windows-Zip, .deb, Arch-Paket,
    tar.gz mit install.sh, macOS-DMG (Apple Silicon + Intel), Server-Zips, Archiv-Zips im alten Format
    (darüber wechseln alte Python-Installationen automatisch zur Rust Edition).
  - Workflow `.github/workflows/build-rust.yml`: Tests, E2E, alle Pakete, Docker-Image, Releases in
    erqf1/JumpNDodge (Latest) und erqf1/JumpNDodge-Archive.
  - Website `website/jump/` (Hauptseite + `server/`).

## In Arbeit / offen
- D: W-Hub-Katalog (erqf1/whub) auf die neuen Pakete umstellen + Release, Release v5.0.0 veröffentlichen.
- E: Grafik/Texturen.
- F: Replays/Geister, Controller, Effekte, Lua-Mods.

## Was der Nutzer selbst tun muss
- (wird am Ende vervollständigt)
