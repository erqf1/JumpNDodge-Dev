# Stand der Rust Edition

Letzte Aktualisierung: 2026-11-16 · Branch: `main` (5.13.0 kommt per PR von `claude/inspiring-wright-jckfn8`)

## Fertig und veröffentlicht
- **5.13.0 – Musik-Engine + fünf neue Songs (`src/game/music/`):**
  `dsp.rs` (bandbegrenzte Wellentabellen, SVF-Filter, Freeverb-Hall, Ping-Pong-Echo, Limiter), `inst.rs` (Instrumente als
  Stimmen: Klavier, FM-Glocke, Karplus-Strong-Zupfsaite, Synth für Pad/Supersaw/Pluck/Streicher, Formant-Chor, Sub, Reese,
  Kick/Snare/Clap/Hat/Becken/Tom, Riser/Downlifter/Impact, Rauschflächen, Sampler), `engine.rs` (8 Busse mit Filterspuren,
  Sidechain, Sends; Master mit Stottern/Bandstopp, Gain-Lücken, Limiter; rendert in Blöcken und in Abschnitten parallel – jeder
  Abschnitt mit 7 s Vorlauf, der erste vom Song-Ende → nahtlose Schleife), `arr.rs` (Notenschrift `"g5:4 eb5:2 | r:4"`,
  Akkordschrift `"Cm Ab Eb:2 Bb/D:2"`, synthetischer Break zum Zerhacken mit Chop-Mustern `"0 _ 4s 12t 30^ 4r"`, Helfer für
  Melodie/Arpeggio/Fläche/Comping/Bass/Effekte), `songs/` (je Song eine Funktion; `common.rs` = Chop-Bibliothek + Übergänge).
  Songs: Falling Pixels (174, unverändert komponiert), Banners in the Rain (172), Gearbreaker (182), Frostlight (168),
  Hollow Star (164), Ember Rush (180), je 1:36–1:51. Mix-Analyse im Test `music::tests::songs_rendern` (RMS je 8 Takte, Bänder,
  Busse, Stereo-Breite, Naht); WAVs mit `JND_MUSIC_OUT=<ordner>`, einzelner Song mit `JND_MUSIC_ONLY=<schlüssel>`,
  Profil `cargo test --release --bin JumpNDodge profil -- --ignored`. Speicher: Hintergrund-Thread rendert nur Menü + gewählten
  Song, Cache hält höchstens Menü + 2 Songs (`music::request/get`), `Mixer::music_play_shared` spielt ohne Kopie.
  Neuer Song: Datei in `songs/` anlegen und in `songs::LIST` eintragen (Schlüssel = Einstellungswert `ingame_music_track`).
- **5.13.0 – GPU-Ausgabe (Punkt 16):** `src/pg/gpu.rs` (wgpu 25: Vulkan/Metal/DX12, sonst OpenGL). Das CPU-gezeichnete Bild
  (tiny-skia) wird als Textur hochgeladen und per GPU gezeigt (1:1 scharf, sonst linear skaliert, Vsync über Mailbox/Fifo).
  Das Zeichnen selbst bleibt CPU – die GPU übernimmt Present/Skalierung/Vsync (spart die Pixel-Umwandlung und das Kopieren in
  softbuffer). `Display` wählt beim ersten Bild: Einstellung `render_output` (`auto`/`gpu`/`cpu`, Darstellung → Ausgabe, gilt
  ab Neustart). „Automatisch“ lässt reine Software-Adapter (llvmpipe/WARP) weg, unsichtbare Tests (`JND_HIDDEN=1`) nehmen
  immer CPU; `JND_OUTPUT=gpu|cpu|auto` erzwingt einen Weg. Jeder GPU-Fehler (kein Adapter, Gerät verloren …) → softbuffer.
  Getestet unter Xvfb mit Mesa-Vulkan (lavapipe): Bild identisch zum CPU-Weg; ohne Treiber greift der Rückfall.
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
  Hover-Animation (`src/game/ui_c.rs`). **Einstellungen → Mehr → Legacy-Grafik** zeichnet alles wie die ursprüngliche
  Python-Version (rote Blöcke, grünes 1-UP, zwei Bodenlinien, einfarbig, kein Squash); Dev-Befehl `run legacy_art [off]`.
  Eigene Texturen: Ordner `textures/` neben dem Spiel (`player.png`, `cos_<name>.svg` …). Dev-Konsole: `run equip <cosmetic>`.
  Außerdem drin (war in 5.2.2 vorbereitet): Rangliste über mert.win, Multiplayer-Server play.mert.win.
- **Spielstand-Schlüssel (5.3):** steht nicht mehr im Code. `build.rs` setzt ihn aus `JND_SAVE_KEY` verschleiert ein
  (GitHub-Secret; Sicherung liegt außerhalb des Repos in `JumpNDodge/Geheim (nicht hochladen)/`). Release-Builds ohne Secret
  brechen ab (`JND_REQUIRE_SAVE_KEY=1`). Alte Stände werden mit dem alten Schlüssel gelesen und sofort neu gespeichert;
  den alten Schlüssel in ein paar Versionen ganz rausnehmen. Ohne Variable (Entwickler-Builds) gilt der alte Schlüssel.
- **Updater-Fix (nach 5.3.0):** Portable Windows-Installationen luden das Server-Zip statt des Spiels (beide endeten auf
  `-windows-x64.zip`). Im Release 5.3.0 heißt die Server-Datei jetzt `JumpNDodge-Server-win-x64.zip` (damit klappt es auch
  für alte 5.2.1), der Updater sucht ab 5.3.1 nur noch exakte Spiel-Dateinamen und gibt PowerShell normale Pfade.
- **5.6.0 – 5.11.0 (Feature-Fahrplan 14.11.):** Schnell spielen, Raum-Chef (starten/kicken/privat), Schnell-Nachrichten
  (Tasten 1–4), CasaOS-Fix (Container startet als root, chown /data, setpriv 1000) · Kosmetik online sichtbar, Zuschauen ·
  Server-Browser (öffentliche Liste beim offiziellen Server: `server/src/registry.rs`, `listed`/`public_address`; LAN-Broadcast),
  Freunde einladen (DM `[jnd-join ADRESSE CODE]`, Banner) · Steuerung: Sprung vormerken 120 ms (online per gehaltener Taste),
  Controller-Vibration, RT/LT, Experimentell schnell fallen (Aktion `down`) + kurz springen · Beta-Hindernisse im Custom-Modus
  (`tall`, `bouncer`, `dasher`, `bar`) · Replays online: HTTP auf dem WebSocket-Port (`ws.rs` `bind_with_http`,
  `server/src/replays.rs`), Spiel `src/game/replay_net.rs` (Rekord hochladen, bis 3 Freundes-Geister, Teilen per Code).
  Historie: `CHANGELOG.md`; Release-Text je Version: `packaging/release-notes.md`.
- **5.5.0:** Online bis 8 Spieler (Protokoll 3): `Sim2p::new_n`, `players: Vec`, `Winner::Other(u8)`/`of`/`index`,
  `check_winner` (VS: Letzter übrig gewinnt). Räume mit 2–8 Plätzen, Start wenn ≥2 da und alle bereit; `Start` schickt
  `you` + `names`. Online-Feld in echten Pixeln = kleinster Bildschirm aller Spieler (5.4.6). Chat-Verlauf 24 h im Server
  (`chat_history_hours`, Datei `chat.json` neben der `server.json`, beim Beitreten mit `CHAT_HISTORY_ID` nachgeschickt).
  Test mit mehreren Clients: Skript-Muster in `tests/mp_e2e.sh`, Raum per `run mp create vs Hölle 4`.
- **5.4.0:** Menüs überall im Startmenü-Look (`ui_c.rs`, `G::draw_panel`/`draw_tab`), Einstellungen neu sortiert
  (`settings_ui.rs`), Übergänge mit Herkunft und **Pop-ups** (`transitions.rs`: Einstellungen, Belohnungen, Profil, Online,
  Anleitung, Code, Mods, Schwierigkeit als Karte über dem Menü; direkt in Kartengröße gezeichnet), Einstellungen als Zahnrad
  neben dem Profil, Hover-Streifen. **Server-Chat** (Protokoll 2: nur live weitergereicht, kein Verlauf/Log, Spam-Limit,
  `server.json` → `chat`). Spielgefühl: Combo + Meilensteine, "Knapp!" mit kurzer Zeitlupe, Hit-Stop, Punkte-Pops,
  Sprungstaub, leichtes Licht-Shading, Spur-Band. **Leistung:** schneller Blit/Überblend-Weg in `pg/surface.rs`
  (`fast_blit`, `fill_blend`, `edge_vignette`) - vorher bei 1440p 25 ms pro Spielbild (45 ms bei 1 HP), jetzt ~3 ms (~8 ms).
  Messen: `JND_PERF=1 JND_WIN_SIZE=2560x1440` mit Debug-Build (Zeiten pro Abschnitt über `art::pmark`).
- **Nach 5.4.0 (auf main, kommt mit 5.4.1):** Updater-Fix, Pop-ups wieder raus, Überschriften über
  `G::draw_screen_title`, Knöpfe über `draw_ui_button`. **Protokoll-Sperre:** Verbinden entscheidet nur noch
  `PROTO_VERSION` (`core/src/net/proto.rs`), nicht die Spielversion; die Serverliste zeigt „andere Version“ nur bei anderem
  Protokoll. **WebSocket:** Server hört zusätzlich auf TCP 47801 (`ws_port`, `core/src/net/ws.rs`); das Spiel nimmt `ws://`/`wss://`
  direkt und weicht bei reinem Hostnamen ohne UDP-Antwort automatisch auf `wss://<host>` aus (Cloudflare Tunnel).
  **Server-Auto-Update** (`server/src/update.rs`): neuestes Release, SHA256-Prüfung, Neustart wenn niemand verbunden ist;
  Docker legt die Datei nach `/data/bin`, `server/docker-entrypoint.sh` nimmt die neuere (Image oder /data, Rückfall aufs Image).
  **CasaOS offiziell:** `server/casaos-offiziell/` (Server + cloudflared, `ANLEITUNG.md`). **Bewegung:** federnde Spielfigur
  (`art::squash_spring`), wippende Hindernisse (`draw_hazard_live`), Kristall-Aufprall (`draw_faller_live`), verformtes Blitten
  `Surface::blit_xf`; nur Optik, folgt der Effekte-Einstellung, Legacy unverändert; Vorschau `run art_preview`. Spur-Band entfernt.
  **Anleitung als Pop-up** (`open_tutorial_popup`: Hintergrund einmal weichgezeichnet, kein Seitenübergang). **Wiki** `website/jump/wiki/`.
  Historie jetzt in `CHANGELOG.md`, `packaging/release-notes.md` nur noch der kurze Text fürs nächste Release.
- **5.2.0:** eingebaute Sounds (die Releases waren ohne eigene Sounddateien stumm), Mehr → Replays (letzte Runde + Rekordläufe).

## Was du selbst tun musst
1. **Website hochladen** (CasaOS): Ordner `website/jump/` aus diesem Repo nach `mert.win/jump/` kopieren (inkl. neuem `wiki/` – JumpNDodge-Wiki mit allen Werten aus dem Code; bei Änderungen an Werten in `assets/data.json`/`core.rs` mitpflegen)
   (`index.html` im Aufbau der Line-Seiten, `img/` mit `shots/`, `tex/`, `figur/`; `style.css` + `site.js` braucht nur noch `server/index.html`). Für W Hub zusätzlich
   `website-update/hub/changelog-1.2.1.html` und `changelog-1.2.2.html` aus erqf1/whub oben in `mert.win/hub/changelog.html` einfügen.
2. **Offizieller Multiplayer-Server** (`play.mert.win`, über Cloudflare Tunnel): Anleitung `rust/server/casaos-offiziell/ANLEITUNG.md`.
   - Cloudflare Zero Trust → Tunnel anlegen, Token in `docker-compose.yml` bei `TUNNEL_TOKEN` eintragen (nur in CasaOS, nie ins Repo).
   - Public Hostname `play.mert.win` → HTTP → `jumpndodge-server:47801`. Ein alter DNS-Eintrag `play` (A, graue Wolke) muss dafür weg.
   - CasaOS: App-Store → „+“ → Importieren → Inhalt von `rust/server/casaos-offiziell/docker-compose.yml`.
   - Paket `jumpndodge-server` auf GitHub öffentlich stellen. Optional zusätzlich direkt per UDP unter einem anderen Namen
     (graue Wolke + Portweiterleitung UDP 47800).
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
- Online: Bestenliste für Online-Runden.
- GPU-Ausgabe: nächster Schritt wäre, auch das Zeichnen (Sprites/Texturen) auf die GPU zu legen – bisher macht die GPU nur
  Present/Skalierung/Vsync. Unter echten Grafikkarten (Windows/macOS) noch nicht von Hand ausprobiert.
- Musik: Menü-Fassung ist noch die ruhige Version von Falling Pixels; denkbar wäre je Song eine eigene ruhige Fassung.
