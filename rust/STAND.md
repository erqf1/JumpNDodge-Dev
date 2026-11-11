# Stand der Rust Edition (wird nach jedem Arbeitsschritt aktualisiert)

Letzte Aktualisierung: 2026-11-11 (Nachfolge-Routine)

## Fertig
- Port 1:1 von JumpNDodge.py (außer Mods), Spielstand kompatibel (siehe PORT_NOTES.md).
- Multiplayer Phase 1 (jnd_core::sim2p) und Phase 2 (Protokoll, UDP-Kanal, JumpNDodge-Server, Tests).
- **A/C (Teil 1): Bildrate unabhängig von der Spiellogik**
  - `src/game/timestep.rs`: feste Logikrate 60 Schritte/s (`FixedStep`), Zeichnen mit Monitor-Bildrate
    (`Display::refresh_rate()`, z. B. 144/165 Hz) und Interpolation von Spieler/Hindernissen/1-UPs.
  - 1P (`run_game`) und lokales 2P (`run_game_2p`) nutzen das; Sprunghöhe, Tempo, Spawn-Zeiten bleiben exakt wie in Python.
  - Einstellung `fps_limit` in den Settings (0 = wie Monitor, sonst feste Obergrenze 30–1000). Noch ohne Menüeintrag.
  - Überblendungen (Fade) sind jetzt zeitbasiert statt pro Bild.
  - Vollbild/HiDPI: bei ganzzahligem Skalierungsfaktor (z. B. 200 %) werden Pixel exakt verdoppelt statt
    bilinear verwischt.
  - Test: `cargo test -p jumpndodge timestep` (60 Schritte/s bei 30/60/144/165/240 FPS), Selbsttest 1P/2P unsichtbar.

## Offen (in dieser Reihenfolge)
- A Rest: Menüs laufen weiter fest mit 60 FPS in eigenen while-Schleifen (blockierend pro Screen);
  starre 800x600-Annahmen prüfen; Online-Aufrufe sind schon asynchron (online_request_async), Rest prüfen.
- C Rest: echtes scharfes Vollbild bei nicht-ganzzahliger DPI (125 %/150 %) – dafür müsste in physischer
  Auflösung gezeichnet werden; Menüeintrag für `fps_limit`.
- B Phase 3: Multiplayer-Screens im Spiel (Serverliste, Per IP verbinden, Räume, Bereit, Countdown,
  Runde mit Vorhersage/Interpolation, Ergebnis + Rematch) mit `jnd_core::net::client`.
- B Phase 4: Docker-Image für den Server + Website-Seite "Server hosten" (Portweiterleitung UDP 47800).
- B Phase 5: automatischer Test mit zwei Clients (Spiel-seitig).
- D: Auto-Updater, Installer (Win/Linux/macOS, Übernahme Python-Spielstand), Website `website/jump/`, W-Hub-Katalog.
- E: Grafik/Texturen.
- F: Replays/Geister, Controller, Effekte, Lua-Mods.

## Was der Nutzer selbst tun muss
- Nichts für diesen Schritt. (Build unter Linux braucht `libasound2-dev`, `libxkbcommon-dev`; zum Starten `libxkbcommon-x11-0`.)
