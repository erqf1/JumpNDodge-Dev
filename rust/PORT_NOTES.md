# Rust-Port von JumpNDodge.py – Arbeitsnotizen

Ziel: 1:1-Nachbau von `../JumpNDodge.py` (18 042 Zeilen, pygame) in Rust. Gleiche Bildschirme,
gleiche Logik, gleicher Spielstand (`data/gameplay.json`, verschlüsselt, kompatibel – getestet).

## Aufbau
- `src/pg/` – pygame-Nachbildung: `Surface` (Rc, wie Python-Referenzen; `.copy()` = echte Kopie),
  `draw::{rect, circle, ellipse, line, lines, polygon, arc, aaline}`, `transform::{smoothscale, flip, rotate}`,
  `Font::default(size)` (= pygame SysFont(None, size), freesansbold * 0.6875), `Font::sys(name, size, bold)`,
  `Display` (winit pump + softbuffer; `event_get()`, `key_pressed(k)`, `mouse_pos()`, `get_ticks()`),
  `Clock`, `Mixer` (rodio), Tastencodes = pygame/SDL-Codes (`K_SPACE` …), `key_name()`.
- `src/crypto.rs` – Spielstand-Verschlüsselung (identisch zu Python).
- `src/util.rs` – `PyGet` (`gb/gf/gi/gs/garr/gobj` = `d.get(k, default)`), random wie Python, `hsv_to_rgb` …
- `src/data.rs` – `assets/data.json` (per Python-ast automatisch aus JumpNDodge.py extrahiert:
  texts, POWERS_DB, COSMETICS_DB, ITEM_ICONS, Übersetzungen, Tutorial, …).
- `src/game/` – das eigentliche Spiel. Alle Python-Globals sind Felder von `G` (state.rs) mit
  **gleichem Namen** (auch mit führendem `_`). Alle Python-Funktionen sind `impl G`-Methoden mit
  **gleichem Namen**, verteilt auf Dateien nach Abschnitten der Python-Datei.

## Konventionen
- `pygame.display.flip()` → `self.flip()`; `pygame.event.get()` → `self.event_get()`.
- Zeichnen auf base_surface: `let bs = self.base_surface.clone();` dann `draw::rect(&bs, …)`.
- Farben als `(i32, i32, i32)` / `(i32, i32, i32, i32)`.
- `settings`, `stats`, … sind `Obj` (serde_json Map). Speichern über `save_section(name)`; die
  Live-Felder werden beim Schreiben in den Cache gespiegelt (Python-Aliasing nachgebildet).
- Online-Callbacks: `self.online_request_async(m, path, payload, Some(Box::new(|g, res| …)))` –
  laufen im Hauptthread (in `sync_window_size`/`flip`), nie im Hintergrundthread.
- Tests unsichtbar: `JND_HIDDEN=1 JND_MUTE=1` (Fenster unsichtbar, kein Ton).

## Fortschritt (Python-Zeilen → Rust-Datei)
- 1–2357 Setup, Hintergrund, Anzeige, Online-Client, Zeit/Daily, Spielstand, Profile, Sound, Defaults, Fade/Flip → `game/core.rs`, `game/online.rs`, `game/save.rs`, `game/sound.rs`
- 4450–5820 Menü-Funktionen, Hauptmenü, Schwierigkeitsauswahl → `game/menus.rs` (Mods-Menü 4958–5340 bewusst ausgelassen: Mods werden später besprochen)
- 5873–6740 Tutorial, Sprachauswahl, Profile, Mehr-Menü, Profil-Hub → `game/screens_a.rs`
- 6740–7700 Rangliste, Spielersuche, öffentliches Profil, Username-Gate → `game/online_screens.rs`
- 7700–9650 Freunde-Hub, Online-Hub, Nachrichten, Squads (Hub/Erstellen/Suche/Ansicht), Chat, Zwischenablage, Code einlösen, Umbenennen → `game/online_screens2.rs`, `game/online_screens3.rs`
- 9650–11340 Erfolge, Steuerung, Einstellungen, Sound-Menü, Classic, Laden, Daily Rewards, Hilfe, Statistik (+ Overlays/Popups 11380–11485) → `game/screens_b.rs`, `game/screens_c.rs`, `game/stats_screen.rs`
- 11339–12370 Glücksrad-Shop, Loadout-Menü, Kosmetik-Inventar → `game/shop.rs`, `game/cosmetics_inv.rs`
- 12370–13546 Kosmetik-Shop, Shop-Menü, Tooltip, Power-Loadout, Admin → `game/cosmetic_shop.rs`, `game/loadout.rs`
- 14019–14424 RuntimePowers, Pause-Menü, Kosmetik am Spieler → `game/runtime.rs`
- 14459–15733 run_game (Spiel) → `game/gameplay.rs` (Mod-Hooks entfallen, solange Mods ausgelassen sind)
- 15739–16384 run_game_2p, Game-Over-Screens → `game/gameplay2p.rs`, `game/gameover.rs`
- 130–215 + 16387–16797 Debug-Log, Dev-Konsole (stdin-Thread, `run help`) → `game/devconsole.rs`
- Modulebene + 16800–18042 Spielstart (`G::new`) und Hauptschleife → `game/mainloop.rs`

## Stand
Port vollständig bis auf Mods (Mod-Menü 4958–5340, Mod-Lader 13554–13800): `in_mods_menu` springt sofort ins Hauptmenü zurück.
Spielstand in beide Richtungen kompatibel (getestet: Rust schreibt, Python liest).

## Multiplayer – Phase 1 (erledigt)
2P-Regeln liegen in der Bibliothek `rust/core` (`jnd_core::sim2p::Sim2p`): ohne Grafik, eigener Zufall per Seed,
`step(dt, [Input; 2], w, h)` liefert Ereignisse (Treffer/Ausgeschieden/1-UP/Wiederbelebt) und Rundenende/Sieger.
`run_game_2p` zeichnet nur noch. `Rect`/`addf` und die Schwierigkeitswerte kommen ebenfalls aus `jnd_core`.
Tests: `cargo test -p jnd_core` (u.a. Determinismus: gleicher Seed + gleiche Eingaben = gleicher Verlauf).

## Multiplayer – Phase 2 (erledigt)
- `jnd_core::net::proto`: alle Nachrichten (ClientMsg/ServerMsg, Snapshot), PROTO_VERSION, Standard-Port 47800, Feld 800x600, 60 Ticks/s, 30 Snapshots/s.
- `jnd_core::net::channel`: UDP mit zuverlässigen (Reihenfolge, Wiederholung) und unzuverlässigen Nachrichten, Timeout 10 s.
- `jnd_core::net::client`: `Client::connect/poll/send/send_input/disconnect`, `query_info` (Serverliste), `parse_addr`.
- `rust/server` (`JumpNDodge-Server`): server.json, Räume mit 4-stelligem Code, Bereit → Countdown → Runde → Lobby (Rematch).
  VS-Abbruch → Gegner gewinnt, Koop-Abbruch → Partner spielt allein weiter. Versions-/Passwortprüfung.
- Tests: `cargo test -p jnd_core -p jnd_server` (echter Server + 2 Clients, auch mit 15 % Paketverlust).
Nächste Phase 3: Multiplayer-Screens im Spiel + Netzwerk in der Runde (Vorhersage eigener Spieler, Interpolation).

## Python-Altlasten behoben (Rust-Version weicht hier bewusst ab)
- **Scharfe Darstellung**: Jede `Surface` hat einen Faktor `k` (echte Pixel pro logischem Pixel,
  `pg::surface::set_render_scale`). Der Spielcode rechnet weiter logisch (z.B. 800x600), gezeichnet wird in
  Fensterauflösung; `sync_window_size` setzt k = physische Größe / logische Größe. `present` kopiert 1:1.
  Bei k > 1 werden Formen geglättet. Abschaltbar: `settings["sharp_rendering"] = false`.
  Test mit HiDPI unter Xvfb: `WINIT_X11_SCALE_FACTOR=2`.
- **Bildrate**: Menüs ticken mit `self.target_fps()` (Monitor-Hz, `settings["fps_limit"]`, 0 = Monitor;
  im versteckten Test immer 60). Spielrunden (1P/2P): feste Logik mit 60 Schritten/s
  (`LOGIC_TICK_MS`, `logic_dt(n)` = 16/17 ms), Zeichnen interpoliert (`Obstacle::prev`, `lerp_rect`).
  Sprungtaste wird gemerkt (`jump_latch`), Überblendung zeitbasiert.
- Auto-Pause bei Fokusverlust (`pause_on_focus_loss`), Vollbild-Schalter + F11, FPS-Auswahl unter
  Einstellungen > Mehr.
- Spielstand wird im Hintergrund-Thread geschrieben (`crypto::save_encrypted_json_bg`, `flush_saves()` beim Beenden).
- `net::client::Client::pump()`: Netzwerk bedienen ohne Nachrichten abzuholen (sonst trennt der Server nach 10 s).

## Rust Edition (ab 5.0) – neue Teile
- `src/updater.rs` + `game/update_ui.rs`: eingebauter Updater (erqf1/JumpNDodge, releases/latest). Testvariablen:
  `UPDATER_FAKE_VERSION`, `UPDATER_AUTO=now|show`, `UPDATER_DRY=1`, `UPDATER_REPO`, `UPDATER_ASSET_SUFFIX`.
- `game/save.rs::base_dir()`: Datenordner neben dem Programm, bei /usr, /opt, .app oder schreibgeschützt im Benutzerordner.
- `game/multiplayer.rs` (Bildschirme) + `jnd_core::net::netround` (Vorhersage/Interpolation). Dev: `run multiplayer`, `run mp …`.
- `game/art.rs`: Sprites (Figur, Hindernisse, 1-UP, Boden, Hintergrund, Geist) + Partikel `Fx`. Dev: `run art_preview <png>`.
- `game/replay.rs`: Aufzeichnung pro Logikschritt (`data/replays/*.jnr`), Replay-Ansicht, Rekord-Geist.
- `pg/display.rs`: Controller über gilrs, als Tasten abgebildet (Pad 1 = WASD/Leertaste, Pad 2 = Pfeile).
  Test: `pad:<nr>:<knopf>:<down|up>`, `pad:<nr>:stick=<x>`.
- `game/mods.rs` + `game/mods_ui.rs`: Lua-Mods (mlua, Lua 5.4 eingebaut, Sandbox). Anleitung `LUA_MODS.md`.
  Mod-Runden wie in Python getrennt (`modded_*`), nicht online.
- Pakete: `packaging/` (Inno, deb, Arch, tar.gz, macOS) + `.github/workflows/build-rust.yml` (Release per Run workflow, tag = vX.Y.Z).

## Versteckter Selbsttest (nur Debug-Build)
`JND_HIDDEN=1 JND_MUTE=1 JND_BASE_DIR=<Testordner> JND_SCRIPT="<frame>:<aktion>:<arg>;..."`
Aktionen: `shot:<png>`, `cmd:run <befehl>` (Dev-Konsole, ohne key.txt), `key:<taste>`, `hold:<taste>`, `release:<taste>`,
`type:<text>`, `click:<x>,<y>`, `quit`. Beispiel: `30:cmd:run shop;60:shot:shop.png;70:quit`.

## Release
Workflow `.github/workflows/build-rust.yml`: Tag `rust-v*` → Vorabversion im Archive-Repo (Windows/Linux/macOS).
Der Updater ignoriert `rust-v*`-Tags (parse_version liefert None).
