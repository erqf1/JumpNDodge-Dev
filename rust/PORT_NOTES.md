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
- Platzhalter in `game/stubs.rs` (open_chat, open_clan_view, draw_player_cosmetics, draw_hover_tooltip, push_overlay, debug_log, Dev-Konsole) – beim Portieren ersetzen.

## Als Nächstes (Python-Zeilen)
9650–10380
Erfolge, Steuerung, Einstellungen · 10380–10580 Classic · 10580–11350 Laden, Daily Rewards, Hilfe, Statistik ·
11350–13560 Glücksrad-Shop, Loadout, Kosmetik-Shop/Inventar, Tooltip, Admin · 13554–13800 Mod-Lader (später) ·
13800–14460 Erfolge, Coins, RuntimePowers, Pause, Kosmetik zeichnen · 14459–16420 run_game, run_game_2p, Game Over ·
16420–18042 Dev-Konsole, Hauptschleife (Event-Dispatch) → danach main.rs auf das echte Spiel umstellen.
