## JumpNDodge 5.0.0 – Rust Edition

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
- macOS: Die App ist nicht von Apple beglaubigt – beim ersten Start Rechtsklick → Öffnen.
- Mods gibt es in der Rust Edition noch nicht (kommen später als sichere Lua-Mods).

---

**English:** JumpNDodge has been rewritten in Rust and is now the main version (your save is kept). New: online multiplayer
(versus/co-op, room codes, host your own server), new graphics, built-in updater, frame rate that matches your monitor, sharp rendering at
any scaling, fullscreen toggle (F11). Pick the file for your system from the table above or install it with W Hub.
