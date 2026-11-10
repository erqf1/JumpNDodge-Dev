//! Debug-Log und Dev-Konsole (Python 130-215 und 16387-16797).
//! Die Konsole liest Zeilen von stdin (eigener Thread) und arbeitet sie im Haupt-Thread ab.

use super::state::G;
use crate::game::save::DIFFICULTIES;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};
use std::io::Write;

pub const ALL_SCREEN_FLAGS: [&str; 43] = [
    "in_menu", "game_running", "showing_stats", "in_shop", "in_loadout", "in_profile_select",
    "in_profile_name_input", "in_mods_menu", "in_more_menu", "in_classic_game", "in_controls_menu",
    "in_settings_menu", "in_settings_more_menu", "in_experimental_menu", "in_settings_sound_menu",
    "in_help", "in_daily_rewards", "in_shop_menu", "in_cosmetic_shop", "in_loadout_menu",
    "in_cosmetic_inventory", "in_loading", "in_difficulty_select", "in_tutorial", "in_language_select",
    "in_admin_login", "in_admin_panel", "in_achievements_screen", "in_profile_hub",
    "in_online_leaderboard", "in_player_search", "in_public_profile",
    "in_rename_profile", "in_redeem_code",
    "in_online_hub", "in_friends_hub", "in_messages_hub", "in_clan_hub", "in_clan_create", "in_clan_search",
    "in_clan_view", "in_chat", "in_username_gate",
];

/// (Name, Beschreibung) - Reihenfolge egal, `help` sortiert.
const DEV_COMMANDS: &[(&str, &str)] = &[
    ("menu", "Zurück zum Hauptmenü"),
    ("shop", "Shop-Hub öffnen"),
    ("shop_roll", "Power-Roll-Shop öffnen"),
    ("shop_cosmetic", "Kosmetik-Shop öffnen"),
    ("loadout", "Loadout-Hub öffnen"),
    ("loadout_powers", "Power-Loadout öffnen"),
    ("cosmetic_inventory", "Kosmetik-Inventar öffnen"),
    ("settings", "Einstellungen öffnen"),
    ("controls", "Steuerung-Screen öffnen"),
    ("more", "Mehr-Menü öffnen"),
    ("profile_hub", "Profil-Hub öffnen (Avatar/Stats/Profile)"),
    ("online_leaderboard", "Online-Rangliste öffnen"),
    ("player_search", "Spielersuche öffnen"),
    ("online_hub", "Online-Hub öffnen (Rangliste/Freunde/Squad/Nachrichten)"),
    ("friends_hub", "Freunde-Hub öffnen (Freunde/Anfragen)"),
    ("messages_hub", "Nachrichten-Hub öffnen (alle Unterhaltungen)"),
    ("username_gate", "Zwangs-Onboarding-Gate öffnen (prüft live ob online)"),
    ("clan_hub", "Clan-Hub öffnen"),
    ("chat", "Chat öffnen, z.B.: run chat dm SomeFriend / run chat clan TAG"),
    ("rename_profile", "Namen-ändern-Screen öffnen"),
    ("redeem_code", "Code-Einlösen-Screen öffnen (im Mehr-Menü)"),
    ("stats", "Statistik-Screen öffnen"),
    ("difficulty", "Schwierigkeitsauswahl öffnen"),
    ("daily_rewards", "Daily Rewards öffnen"),
    ("admin_login", "Admin-Login öffnen"),
    ("admin_panel", "Admin-Panel öffnen (Login übersprungen)"),
    ("mods", "Mods-Menü öffnen"),
    ("help", "Diese Liste anzeigen"),
    ("tutorial", "Tutorial öffnen"),
    ("achievements", "Achievements-Screen öffnen (im Spiel normal nicht erreichbar)"),
    ("language_select", "Sprachauswahl öffnen"),
    ("game", "Direkt eine Standard-Runde starten"),
    ("game2p", "Direkt eine 2-Spieler-Runde starten"),
    ("add_coins", "Coins hinzufügen, z.B.: run add_coins 500"),
    ("set_coins", "Coins setzen, z.B.: run set_coins 0"),
    ("unlock_all_powers", "Alle Powers freischalten"),
    ("unlock_all_cosmetics", "Alle Kosmetik-Items freischalten"),
    ("give_power", "Eine bestimmte Power geben, z.B.: run give_power Double Jump"),
    ("set_difficulty", "Schwierigkeit setzen, z.B.: run set_difficulty Hardcore"),
    ("set_lang", "Sprache setzen, z.B.: run set_lang English"),
    ("set_player_color", "Avatar-/Spielerfarbe setzen, z.B.: run set_player_color red"),
    ("collect_1up", "1-UP(s) einsammeln, z.B.: run collect_1up 3"),
    ("add_score", "Score erhöhen, z.B.: run add_score 100"),
    ("add_lives", "Leben hinzufügen, z.B.: run add_lives 1"),
    ("set_lives", "Leben setzen, z.B.: run set_lives 0 (Runde endet beim nächsten Treffer)"),
    ("god_mode", "Unverwundbarkeit an/aus, z.B.: run god_mode / run god_mode off"),
    ("list", "Alias für help"),
    ("state", "Aktuellen Zustand anzeigen"),
    ("debug_mode", "Debug-Logging an/aus - loggt Online-Requests, Screen-Wechsel und Fehler nach data/debug.log, z.B.: run debug_mode / run debug_mode off (geht auch ohne 'run': debug-mode / debug-mode off)"),
];

/// Navigationsbefehle -> Ziel-Flag (Python _dev_cmd_nav)
const NAV_CMDS: &[(&str, &str)] = &[
    ("menu", "in_menu"), ("shop", "in_shop_menu"), ("shop_roll", "in_shop"), ("shop_cosmetic", "in_cosmetic_shop"),
    ("loadout", "in_loadout_menu"), ("loadout_powers", "in_loadout"), ("cosmetic_inventory", "in_cosmetic_inventory"),
    ("settings", "in_settings_menu"), ("controls", "in_controls_menu"), ("more", "in_more_menu"),
    ("profile_hub", "in_profile_hub"), ("online_leaderboard", "in_online_leaderboard"), ("player_search", "in_player_search"),
    ("online_hub", "in_online_hub"), ("friends_hub", "in_friends_hub"), ("messages_hub", "in_messages_hub"),
    ("username_gate", "in_username_gate"), ("clan_hub", "in_clan_hub"), ("rename_profile", "in_rename_profile"),
    ("redeem_code", "in_redeem_code"), ("stats", "showing_stats"), ("difficulty", "in_difficulty_select"),
    ("daily_rewards", "in_daily_rewards"), ("admin_login", "in_admin_login"), ("admin_panel", "in_admin_panel"),
    ("mods", "in_mods_menu"), ("tutorial", "in_tutorial"), ("achievements", "in_achievements_screen"),
    ("language_select", "in_language_select"),
];

pub fn now_hms_ms() -> String { chrono::Local::now().format("%H:%M:%S%.3f").to_string() }

fn py_list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| format!("'{}'", s)).collect::<Vec<_>>().join(", "))
}

fn parse_int(args: &[&str], idx: usize, default: i64) -> i64 {
    args.get(idx).and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(default)
}

impl G {
    /// Zeiger auf ein Screen-Flag per Name (Python: globals()[name])
    pub fn flag_mut(&mut self, name: &str) -> Option<&mut bool> {
        Some(match name {
            "in_menu" => &mut self.in_menu, "game_running" => &mut self.game_running, "showing_stats" => &mut self.showing_stats,
            "in_shop" => &mut self.in_shop, "in_loadout" => &mut self.in_loadout, "in_profile_select" => &mut self.in_profile_select,
            "in_profile_name_input" => &mut self.in_profile_name_input, "in_mods_menu" => &mut self.in_mods_menu,
            "in_more_menu" => &mut self.in_more_menu, "in_classic_game" => &mut self.in_classic_game,
            "in_controls_menu" => &mut self.in_controls_menu, "in_settings_menu" => &mut self.in_settings_menu,
            "in_settings_more_menu" => &mut self.in_settings_more_menu, "in_experimental_menu" => &mut self.in_experimental_menu,
            "in_settings_sound_menu" => &mut self.in_settings_sound_menu, "in_help" => &mut self.in_help,
            "in_daily_rewards" => &mut self.in_daily_rewards, "in_shop_menu" => &mut self.in_shop_menu,
            "in_cosmetic_shop" => &mut self.in_cosmetic_shop, "in_loadout_menu" => &mut self.in_loadout_menu,
            "in_cosmetic_inventory" => &mut self.in_cosmetic_inventory, "in_loading" => &mut self.in_loading,
            "in_difficulty_select" => &mut self.in_difficulty_select, "in_tutorial" => &mut self.in_tutorial,
            "in_language_select" => &mut self.in_language_select, "in_admin_login" => &mut self.in_admin_login,
            "in_admin_panel" => &mut self.in_admin_panel, "in_achievements_screen" => &mut self.in_achievements_screen,
            "in_profile_hub" => &mut self.in_profile_hub, "in_online_leaderboard" => &mut self.in_online_leaderboard,
            "in_player_search" => &mut self.in_player_search, "in_public_profile" => &mut self.in_public_profile,
            "in_rename_profile" => &mut self.in_rename_profile, "in_redeem_code" => &mut self.in_redeem_code,
            "in_online_hub" => &mut self.in_online_hub, "in_friends_hub" => &mut self.in_friends_hub,
            "in_messages_hub" => &mut self.in_messages_hub, "in_clan_hub" => &mut self.in_clan_hub,
            "in_clan_create" => &mut self.in_clan_create, "in_clan_search" => &mut self.in_clan_search,
            "in_clan_view" => &mut self.in_clan_view, "in_chat" => &mut self.in_chat, "in_username_gate" => &mut self.in_username_gate,
            _ => return None,
        })
    }
    pub fn get_flag(&mut self, name: &str) -> bool { self.flag_mut(name).map(|f| *f).unwrap_or(false) }
    pub fn set_screen_flag(&mut self, name: &str, v: bool) { if let Some(f) = self.flag_mut(name) { *f = v; } }

    // ---------------- Debug-Log ----------------
    pub fn _debug_log_write(&mut self, line: &str) {
        let p = self._DATA_DIR.join("debug.log");
        if self._debug_log_failed { return; }
        match std::fs::OpenOptions::new().create(true).append(true).open(&p) {
            Ok(mut f) => { let _ = writeln!(f, "{}", line); }
            Err(_) => self._debug_log_failed = true,
        }
    }

    pub fn debug_log(&mut self, msg: &str) {
        if !self.DEBUG_MODE { return; }
        let mut m = msg.to_string();
        if m.chars().count() > 500 { m = m.chars().take(500).collect::<String>() + "...(gekuerzt)"; }
        let line = format!("[{}] {}", now_hms_ms(), m);
        println!("[DEBUG] {}", line);
        self._debug_log_write(&line);
    }

    // ---------------- Konsole ----------------
    /// Startet den stdin-Leser (Python: threading.Thread(target=_dev_console_reader))
    pub fn _start_dev_console(&mut self) {
        let key = std::fs::read_to_string(self.BASE_DIR.join("key.txt")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        self._dev_console_unlocked = key.is_some();
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        self._dev_cmd_rx = Some(rx);
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            let mut line = String::new();
            loop {
                line.clear();
                match stdin.read_line(&mut line) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => { let t = line.trim(); if !t.is_empty() && tx.send(t.to_string()).is_err() { return; } }
                }
            }
        });
        if self._dev_console_unlocked { println!("[DEV] Konsole aktiv (key.txt gefunden) - 'run help' fuer alle Befehle."); }
        else { println!("[DEV] Dev-Konsole deaktiviert (key.txt nicht gefunden)."); }
    }

    /// Strg+Shift+F: zeigt das Konsolenfenster. Die Rust-exe hat (wie die Nuitka-exe
    /// mit --windows-console-mode=hide) kein sichtbares Fenster; dort öffnen wir keins.
    pub fn _debug_check_console_hotkey(&mut self) {
        let mods = self.display.get_mods();
        let down = self.display.key_pressed(K_f) && mods & KMOD_CTRL != 0 && mods & KMOD_SHIFT != 0;
        if down && !self._debug_console_hotkey_was_down { self.debug_log("Konsole per Strg+Shift+F angefordert (Rust-Build: kein Konsolenfenster)"); }
        self._debug_console_hotkey_was_down = down;
    }

    pub fn _debug_track_screen_transitions(&mut self) {
        if !self.DEBUG_MODE { return; }
        let active: Vec<&'static str> = ALL_SCREEN_FLAGS.iter().copied().filter(|f| self.get_flag(f)).collect();
        if self._debug_last_screen_flags.as_ref() != Some(&active) {
            if let Some(prev) = self._debug_last_screen_flags.clone() {
                self.debug_log(&format!("Screen: {:?} -> {:?}", prev, active));
            }
            self._debug_last_screen_flags = Some(active);
        }
    }

    pub fn _dev_goto(&mut self, flag_name: &str, extra: Option<&str>) {
        for f in ALL_SCREEN_FLAGS { self.set_screen_flag(f, false); }
        self.set_screen_flag(flag_name, true);
        println!("[DEV] -> {}{}", flag_name, extra.map(|e| format!(" {}", e)).unwrap_or_default());
    }

    fn _dev_round_action(&mut self, name: &str, arg: Option<i64>) {
        if !self.game_running { println!("[DEV] '{}' braucht eine laufende Runde (aktuell nicht im Spiel)", name); return; }
        if self.current_game_mode == "2player" { println!("[DEV] '{}' wird aktuell nur im Singleplayer unterstützt", name); return; }
        self._dev_round_actions.push((name.to_string(), arg));
        println!("[DEV] {} eingereiht, wird im nächsten Frame angewendet...", name);
    }

    fn _dev_run(&mut self, cmd: &str, args: &[&str]) -> bool {
        if let Some((_, target)) = NAV_CMDS.iter().find(|(c, _)| *c == cmd) { self._dev_goto(target, None); return true; }
        let powers_list: Vec<String> = d("POWERS_DB").as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        match cmd {
            "game" => { self._dev_goto("game_running", Some("{'current_game_mode': 'standard'}")); self.current_game_mode = "standard".into(); }
            "game2p" => { self._dev_goto("game_running", Some("{'current_game_mode': '2player'}")); self.current_game_mode = "2player".into(); }
            "add_coins" => {
                let n = parse_int(args, 0, 100);
                let c = self.stats.gi("coins", 0) + n;
                self.stats.insert("coins".into(), json!(c));
                self.save_section("stats", None);
                println!("[DEV] add_coins({}): coins={}", n, c);
            }
            "set_coins" => {
                let n = parse_int(args, 0, 0);
                self.stats.insert("coins".into(), json!(n));
                self.save_section("stats", None);
                println!("[DEV] set_coins({}): coins={}", n, n);
            }
            "unlock_all_powers" => {
                let mut owned = self.powers_state.gstrs("owned");
                let added: Vec<String> = powers_list.iter().filter(|p| !owned.contains(p)).cloned().collect();
                owned.extend(added.iter().cloned());
                self.powers_state.insert("owned".into(), json!(owned));
                self.save_section("powers", None);
                println!("[DEV] unlock_all_powers: {} neu freigeschaltet ({} gesamt)", added.len(), owned.len());
            }
            "unlock_all_cosmetics" => {
                let mut owned = self.cosmetics_state.gstrs("owned");
                let keys: Vec<String> = d("COSMETICS_DB").as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
                let added: Vec<String> = keys.into_iter().filter(|k| !owned.contains(k)).collect();
                owned.extend(added.iter().cloned());
                self.cosmetics_state.insert("owned".into(), json!(owned));
                self.save_section("cosmetics", None);
                println!("[DEV] unlock_all_cosmetics: {} neu freigeschaltet ({} gesamt)", added.len(), owned.len());
            }
            "give_power" => {
                if args.is_empty() { println!("[DEV] give_power braucht einen Namen, z.B.: run give_power Double Jump. Verfügbar: {}", py_list(&powers_list)); return true; }
                let name = args.join(" ");
                let Some(m) = powers_list.iter().find(|p| p.to_lowercase() == name.to_lowercase()).cloned() else {
                    println!("[DEV] Unbekannte Power: '{}'. Verfügbar: {}", name, py_list(&powers_list)); return true;
                };
                let mut owned = self.powers_state.gstrs("owned");
                if owned.contains(&m) { println!("[DEV] give_power({}): bereits besessen", m); return true; }
                owned.push(m.clone());
                self.powers_state.insert("owned".into(), json!(owned));
                self.save_section("powers", None);
                println!("[DEV] give_power({}): hinzugefügt", m);
            }
            "set_difficulty" => {
                let diffs: Vec<String> = DIFFICULTIES.iter().map(|s| s.to_string()).collect();
                if args.is_empty() { println!("[DEV] set_difficulty braucht einen Namen: {}", py_list(&diffs)); return true; }
                let Some(i) = diffs.iter().position(|x| x.to_lowercase() == args[0].to_lowercase()) else {
                    println!("[DEV] Unbekannte Schwierigkeit: '{}'. Verfügbar: {}", args[0], py_list(&diffs)); return true;
                };
                self.difficulty_index = i;
                self.settings.insert("difficulty".into(), json!(diffs[i]));
                self.save_section("settings", None);
                self.update_button_texts();
                println!("[DEV] set_difficulty({})", diffs[i]);
            }
            "set_lang" => {
                let langs = self.languages.clone();
                if args.is_empty() { println!("[DEV] set_lang braucht einen Namen: {}", py_list(&langs)); return true; }
                let Some(i) = langs.iter().position(|x| x.to_lowercase() == args[0].to_lowercase()) else {
                    println!("[DEV] Unbekannte Sprache: '{}'. Verfügbar: {}", args[0], py_list(&langs)); return true;
                };
                self.language_index = i;
                self.settings.insert("language".into(), json!(langs[i]));
                self.save_section("settings", None);
                self.update_button_texts();
                println!("[DEV] set_lang({})", langs[i]);
            }
            "set_player_color" => {
                let names: Vec<String> = super::screens_a::PLAYER_COLOR_NAMES.iter().map(|s| s.to_string()).collect();
                let Some(c) = args.first().map(|a| a.to_lowercase()).filter(|a| names.contains(a)) else {
                    println!("[DEV] set_player_color braucht eine Farbe: {}", py_list(&names)); return true;
                };
                self.settings.insert("player_color".into(), json!(c));
                self.save_section("settings", None);
                println!("[DEV] set_player_color({})", c);
            }
            "chat" => {
                if args.len() < 2 || !["dm", "clan"].contains(&args[0]) { println!("[DEV] chat braucht: run chat <dm|clan> <Username-oder-Tag>"); return true; }
                self._dev_goto("in_chat", Some(&format!("{{'_chat_conversation_type': '{}', '_chat_target': '{}'}}", args[0], args[1])));
                self._chat_conversation_type = args[0].into();
                self._chat_target = args[1].into();
                self._chat_target_label = Some(args[1].into());
                self._chat_return_screen = "in_profile_hub".into();
                self._chat_messages = vec![];
                self._chat_last_id = 0;
                self._chat_last_poll = 0;
                self._chat_input_text = String::new();
                self._chat_msg = String::new();
                self._chat_loading = true;
            }
            "collect_1up" => { let n = parse_int(args, 0, 1); self._dev_round_action(cmd, Some(n)); }
            "add_score" => { let n = parse_int(args, 0, 10); self._dev_round_action(cmd, Some(n)); }
            "add_lives" => { let n = parse_int(args, 0, 1); self._dev_round_action(cmd, Some(n)); }
            "set_lives" => { let n = parse_int(args, 0, 0); self._dev_round_action(cmd, Some(n)); }
            "god_mode" => { let on = args.first().map(|a| a.to_lowercase() != "off").unwrap_or(true); self._dev_round_action(cmd, Some(on as i64)); }
            "help" | "list" => {
                println!("[DEV] Verfügbare Befehle (Aufruf: run <befehl> [args]):");
                let mut v: Vec<&(&str, &str)> = DEV_COMMANDS.iter().collect();
                v.sort_by_key(|(n, _)| *n);
                for (n, desc) in v { println!("  {:<22} {}", n, desc); }
            }
            "state" => {
                let active: Vec<String> = ALL_SCREEN_FLAGS.iter().filter(|f| self.get_flag(f)).map(|s| s.to_string()).collect();
                println!("[DEV] Aktive Screen-Flags: {}", py_list(&active));
                println!("[DEV] mode='{}'  language='{}'  difficulty='{}'  coins={}", self.current_game_mode,
                    self.languages[self.language_index], DIFFICULTIES[self.difficulty_index], self.stats.gi("coins", 0));
            }
            "debug_mode" => {
                let on = args.first().map(|a| a.to_lowercase() != "off").unwrap_or(true);
                let st = if on { "AN" } else { "AUS" };
                let line = format!("[{}] === Debug-Modus {} ===", now_hms_ms(), st);
                self._debug_log_write(&line);
                self.DEBUG_MODE = on;
                let p = self._DATA_DIR.join("debug.log");
                println!("[DEV] debug-mode: {}{}", st, if on { format!(" -> {}", p.display()) } else { String::new() });
            }
            _ => return false,
        }
        true
    }

    pub fn _dispatch_dev_command(&mut self, line: &str) {
        if !self._dev_console_unlocked { println!("[DEV] Konsole deaktiviert (key.txt nicht gefunden)."); return; }
        let mut parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { return; }
        if parts[0].to_lowercase().replace('-', "_") == "debug_mode" {
            let rest: Vec<&str> = parts[1..].to_vec();
            parts = vec!["run", "debug_mode"];
            parts.extend(rest);
        }
        if parts[0] != "run" { println!("[DEV] Unbekannte Eingabe (Befehle starten mit 'run', z.B. 'run help'): '{}'", line); return; }
        if parts.len() < 2 { println!("[DEV] 'run' braucht einen Befehlsnamen, z.B.: run help"); return; }
        let (cmd, args) = (parts[1], &parts[2..]);
        if !self._dev_run(cmd, args) { println!("[DEV] Unbekannter Befehl: '{}'. 'run help' für die Liste.", cmd); }
    }

    pub fn _process_dev_commands(&mut self) {
        loop {
            let line = match &self._dev_cmd_rx { Some(rx) => rx.try_recv().ok(), None => None };
            let Some(l) = line else { return; };
            self._dispatch_dev_command(&l);
        }
    }

    pub fn _drain_dev_round_actions(&mut self) -> Vec<(String, Option<i64>)> { std::mem::take(&mut self._dev_round_actions) }

    // ---------------- Custom-Sprachen ----------------
    pub fn load_custom_languages(&mut self) {
        for i in 1..=10 {
            let fname = format!("Custom-{}.txt", i);
            let p = self.BASE_DIR.join(&fname);
            if !p.exists() { continue; }
            let langname = format!("Custom Language {}", i);
            match std::fs::read_to_string(&p) {
                Ok(s) => {
                    let mut ld = serde_json::Map::new();
                    for line in s.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') { continue; }
                        if let Some((k, v)) = line.split_once('=') {
                            let mut v = v.trim().to_string();
                            if v.chars().count() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\''))) {
                                v = v[1..v.len() - 1].to_string();
                            }
                            ld.insert(k.trim().to_string(), json!(v));
                        }
                    }
                    let n = ld.len();
                    self.texts.insert(langname.clone(), Value::Object(ld));
                    if !self.languages.contains(&langname) { self.languages.push(langname.clone()); }
                    println!("[INFO] {} geladen -> {} ({} Einträge)", fname, langname, n);
                }
                Err(e) => println!("[FEHLER] Konnte {} nicht laden: {}", fname, e),
            }
        }
    }

    pub fn export_current_language(&mut self) {
        let cur = self.languages[self.language_index].clone();
        let ld = self.texts.gobj(&cur);
        let mut all_keys: Vec<String> = vec![];
        for l in &self.languages {
            if let Some(Value::Object(o)) = self.texts.get(l) { for k in o.keys() { if !all_keys.contains(k) { all_keys.push(k.clone()); } } }
        }
        all_keys.sort();
        let english = self.texts.gobj("English");
        let desktop = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(std::path::PathBuf::from).unwrap_or_default().join("Desktop");
        let x = (1..=10).find(|i| !desktop.join(format!("Custom-{}.txt", i)).exists()).unwrap_or(1);
        let filename = desktop.join(format!("Custom-{}.txt", x));
        let mut out = String::new();
        for k in &all_keys {
            let v = ld.get(k).or_else(|| english.get(k)).map(py_str).unwrap_or_else(|| k.clone());
            let v = if v.contains(' ') || v.contains('\t') || v.contains('\n') { format!("\"{}\"", v) } else { v };
            out.push_str(&format!("{}={}\n", k, v));
        }
        match std::fs::write(&filename, out) {
            Ok(_) => {
                self.last_export_message = format!("Last export: Custom-{}.txt on Desktop", x);
                self.push_overlay(&format!("Exported as Custom-{}.txt on Desktop", x), (0, 200, 0), 1.5, None);
            }
            Err(e) => self.push_overlay(&format!("Export failed: {}", e), (200, 0, 0), 1.5, None),
        }
        self.play_sound(super::state::Snd::Click);
    }
}

use crate::data::d;
