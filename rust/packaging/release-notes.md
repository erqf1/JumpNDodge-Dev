## JumpNDodge 5.2.1

- Hauptmenü: unter dem Logo steht jetzt **RUST EDITION** (statt „Ein actionreiches Sprungspiel“).

## JumpNDodge 5.2.0

- **Sound für alle:** Klick, Zurück, Treffer, 1-UP und Game Over sind jetzt eingebaut (vorher war das Spiel ohne eigene
  Sounddateien stumm). Eigene Sounds in `data/sound` haben weiter Vorrang.
- **Replays-Menü:** Mehr → Replays – letzte Runde und deine Rekordläufe je Schwierigkeit ansehen.
- Kleinere Farbkorrekturen im Mehr-Menü.

## JumpNDodge 5.1.0

- **Replays:** Nach jeder Runde im Game Over „Replay (V)“ – mit Pause, 5 s spulen und 1×/2×/4× Tempo.
- **Rekord-Geist:** Dein bester Lauf der gewählten Schwierigkeit läuft halbtransparent mit (abschaltbar unter Einstellungen → Mehr).
- **Controller:** Xbox, PlayStation und andere Gamepads. Controller 1 steuert Menüs und Spiel, Controller 2 ist P2 im lokalen 2-Spieler-Modus.
  A springen/bestätigen · B/Start zurück/Pause · X/LB Power 1 · Y/RB Power 2 · Steuerkreuz/Stick laufen.
- **Lua-Mods:** Hauptmenü → Mods. Eigene Hindernisse, Spielwerte und Modi als `.lua`-Datei – sicher in einer Sandbox
  (kein Zugriff auf Dateien oder Netzwerk). „Beispiel-Mod anlegen“ zeigt, wie es geht; Anleitung: `LUA_MODS.md`.
  Runden mit Mods zählen getrennt und nicht online.

## JumpNDodge 5.0.1

- **Effekte:** Splitter bei Treffern, Funkeln beim 1-UP, Staub beim Landen und kurzes Bildschirmwackeln – im Einzelspieler,
  lokal zu zweit und online. Abschaltbar unter Einstellungen → Mehr → Effekte.
- Server als Docker-Image für CasaOS: `ghcr.io/erqf1/jumpndodge-server`.
- Updater: kleine Korrekturen.

---

## JumpNDodge 5 – Rust Edition

JumpNDodge ist komplett neu in Rust geschrieben und ab jetzt die Hauptversion. Dein Spielstand wird übernommen.

### Neu
- **Online-Multiplayer:** Online → Multiplayer. VS oder Koop zu zweit, Räume mit 4-stelligem Code, Rematch.
  Offizieller Server oder per IP/Hostname verbinden – oder selbst einen Server hosten (Download unten, auch als Docker-Image für CasaOS).
- **Eingebauter Updater** wie bei Vidline & Co.: neue Versionen werden beim Start gefunden und mit einem Klick installiert.
  Der separate JumpNDodgeUpdater ist Geschichte.
- **Neue Grafik:** Spielfigur mit Glanz und Squash & Stretch, Hindernisse mit Warnstreifen, Kristall-Fallblöcke,
  türkises 1-UP-Herz, Boden mit Tempo-Markierungen und Hintergrund mit Farbverlauf.
- **Flüssig auf jedem Bildschirm:** Bildrate passend zum Monitor (144/165 Hz …), die Spiellogik läuft trotzdem genau gleich schnell.
- **Scharf bei jeder Skalierung:** gezeichnet wird in echter Bildschirmauflösung – auch bei 125/150 % und im Vollbild.
- Vollbild-Schalter (auch F11), FPS-Auswahl und Auto-Pause beim Wechsel in ein anderes Fenster (Einstellungen → Mehr).
- Startet sofort, eine einzige Datei, kein Python mehr.

### Downloads
| System | Datei |
|---|---|
| Windows (Installer) | `JumpNDodge-windows-x64-setup.exe` |
| Windows (portabel) | `JumpNDodge-windows-x64.zip` |
| Linux (Debian/Ubuntu/Mint) | `jumpndodge_amd64.deb` |
| Linux (Arch/Manjaro) | `jumpndodge-x86_64.pkg.tar.zst` |
| Linux (portabel) | `jumpndodge-linux-x86_64.tar.gz` (mit `install.sh`) |
| macOS Apple Silicon / Intel | `JumpNDodge-macos-apple-silicon.dmg` / `JumpNDodge-macos-intel.dmg` |
| Eigener Server | `JumpNDodge-Server-*.zip` oder `ghcr.io/erqf1/jumpndodge-server` |

Oder einfach über **W Hub** installieren und aktuell halten.

### Gut zu wissen
- Bestehende Installationen werden an Ort und Stelle aktualisiert, der Ordner `data` (Spielstand, eigene Sounds) bleibt.
- Linux mit der alten Python-Version: bitte einmal das `.deb`/Arch-Paket oder `jumpndodge-linux-x86_64.tar.gz` (`./install.sh`) installieren –
  der Spielstand in `~/.local/share/JumpNDodge` wird übernommen.
- macOS: Die App ist nicht von Apple beglaubigt – beim ersten Start Rechtsklick → Öffnen.
- Mods: die Rust Edition hat sichere Lua-Mods (Hauptmenü → Mods); Python-Mods (.py) laufen nur in der Python-Version.

---

**English:** JumpNDodge has been rewritten in Rust and is now the main version (your save is kept). New: online multiplayer
(versus/co-op, room codes, host your own server), new graphics, built-in updater, frame rate that matches your monitor, sharp rendering at
any scaling, fullscreen toggle (F11). Pick the file for your system from the table above or install it with W Hub.
