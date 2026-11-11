//! Spielstand: JSON-Helfer, verschlüsselte gameplay.json mit Abschnitten, Profile, Tutorial-Daten.

use super::state::G;
use crate::crypto;
use crate::util::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn load_json_file(filename: &Path, default: Value) -> Value {
    if !filename.exists() {
        if let Some(d) = filename.parent() { let _ = std::fs::create_dir_all(d); }
        let _ = std::fs::write(filename, serde_json::to_string_pretty(&default).unwrap_or_default());
        return default;
    }
    std::fs::read_to_string(filename).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(default)
}

pub fn save_json_file(filename: &Path, data: &Value) {
    if let Some(d) = filename.parent() { let _ = std::fs::create_dir_all(d); }
    // Python: json.dump(indent=4, ensure_ascii=False)
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    use serde::Serialize;
    if data.serialize(&mut ser).is_ok() { let _ = std::fs::write(filename, buf); }
}

/// Abschnitte, die in Python dasselbe dict-Objekt wie die globale Variable sind
const LIVE_SECTIONS: [&str; 8] = ["stats", "achievements", "highscores", "best_times", "settings", "powers", "cosmetics", "sale"];

impl G {
    fn live_section(&self, name: &str) -> Option<Value> {
        let o = match name {
            "stats" => &self.stats,
            "achievements" => &self.achievements,
            "highscores" => &self.saved_scores,
            "best_times" => &self.best_times,
            "settings" => &self.settings,
            "powers" => &self.powers_state,
            "cosmetics" => &self.cosmetics_state,
            "sale" => &self.sale_state,
            _ => return None,
        };
        Some(Value::Object(o.clone()))
    }

    /// Spiegelt die Live-Felder in den Cache (Python-Aliasing) und schreibt die Datei
    pub fn write_gamedata_file(&mut self) {
        if self._live_sections_ready {
            for s in LIVE_SECTIONS {
                if self._gamedata_cache.contains_key(s) {
                    if let Some(v) = self.live_section(s) { self._gamedata_cache.insert(s.into(), v); }
                }
            }
        }
        let path = self._GAMEDATA_FILE.clone();
        crypto::save_encrypted_json_bg(&path, Value::Object(self._gamedata_cache.clone()));
    }

    pub fn _delete_old_plaintext_savedata(&self) {
        let b = &self.BASE_DIR;
        let old = [b.join("gamedata.json"), b.join("profiles.json"), b.join("stats.json"), b.join("achievements.json"),
            b.join("highscores.json"), b.join("settings.json"), b.join("powers.json"), b.join("cosmetics.json"), self._DATA_DIR.join("tutorial.json")];
        for p in old.iter() {
            if p.is_file() { let _ = std::fs::remove_file(p); }
        }
    }

    pub fn _ensure_gamedata_loaded(&mut self) {
        if self._gamedata_cache.is_empty() {
            self._delete_old_plaintext_savedata();
            let v = crypto::load_encrypted_json(&self._GAMEDATA_FILE).unwrap_or(json!({}));
            self._gamedata_cache = obj_of(&v);
        }
    }

    pub fn _ensure_modded_loaded(&mut self) {
        if self._modded_cache.is_empty() {
            if self._MODDED_DATA_FILE.is_file() {
                self._modded_cache = std::fs::read_to_string(&self._MODDED_DATA_FILE).ok()
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok()).map(|v| obj_of(&v)).unwrap_or_default();
            }
        }
    }

    pub fn load_section(&mut self, section: &str, default: Value) -> Value {
        self._ensure_gamedata_loaded();
        if !self._gamedata_cache.contains_key(section) {
            self._gamedata_cache.insert(section.into(), default.clone());
        }
        self._gamedata_cache.get(section).cloned().unwrap_or(default)
    }

    /// save_section(name, data) - `data` = None heißt: aktuelles Live-Feld speichern
    pub fn save_section(&mut self, section: &str, data: Option<Value>) {
        self.debug_log(&format!("save_section: {}", section));
        self._ensure_gamedata_loaded();
        let v = match data { Some(v) => v, None => self.live_section(section).unwrap_or(Value::Null) };
        self._gamedata_cache.insert(section.into(), v);
        self.write_gamedata_file();
        self.save_current_profile();
    }

    pub fn load_modded_section(&mut self, section: &str, default: Value) -> Value {
        self._ensure_modded_loaded();
        if !self._modded_cache.contains_key(section) {
            self._modded_cache.insert(section.into(), default.clone());
        }
        self._modded_cache.get(section).cloned().unwrap_or(default)
    }

    pub fn save_modded_section(&mut self, section: &str, data: Value) {
        self._modded_cache.insert(section.into(), data);
        let p = self._MODDED_DATA_FILE.clone();
        save_json_file(&p, &Value::Object(self._modded_cache.clone()));
    }

    // ---- Profile ----
    pub fn _load_profiles_file(&mut self) -> Obj {
        self._ensure_gamedata_loaded();
        match self._gamedata_cache.get("_profiles") {
            Some(Value::Object(o)) => o.clone(),
            _ => obj_of(&json!({"active": null, "profiles": {}})),
        }
    }

    pub fn _save_profiles_file(&mut self, pdata: Obj) {
        self._gamedata_cache.insert("_profiles".into(), Value::Object(pdata));
        self.write_gamedata_file();
    }

    pub fn get_all_profiles(&mut self) -> Obj { self._load_profiles_file().gobj("profiles") }

    pub fn save_current_profile(&mut self) {
        let Some(name) = self.current_profile_name.clone() else { return; };
        if self._profile_save_lock { return; }
        self._profile_save_lock = true;
        let mut pdata = self._load_profiles_file();
        let mut profiles = pdata.gobj("profiles");
        pdata.insert("active".into(), json!(name));
        profiles.insert(name.clone(), json!({
            "name": name,
            "stats": Value::Object(self.stats.clone()),
            "achievements": Value::Object(self.achievements.clone()),
            "highscores": Value::Object(self.saved_scores.clone()),
            "settings": Value::Object(self.settings.clone()),
            "powers": Value::Object(self.powers_state.clone()),
            "cosmetics": Value::Object(self.cosmetics_state.clone()),
            "online_token": self.online_token.clone(),
            "username": self.current_profile_username.clone(),
            "redeemed_codes": self.redeemed_codes_local.clone(),
            "friends": self.friends_local.clone(),
        }));
        pdata.insert("profiles".into(), Value::Object(profiles));
        self._save_profiles_file(pdata);
        self._profile_save_lock = false;
    }

    pub fn load_profile(&mut self, name: &str) -> bool {
        let pdata = self._load_profiles_file();
        let Some(p) = pdata.gobj("profiles").get(name).cloned() else { return false; };
        let p = obj_of(&p);
        self.current_profile_name = Some(name.to_string());
        self._notif_pending_friend_requests = 0;
        self._notif_pending_clan_requests = 0;
        self._notif_unread_conversations = 0;
        self._notif_last_checked_ms = 0;
        self.stats = merged(&obj_of(&self.default_stats()), &p.gobj("stats"));
        self.achievements = merged(&Obj::new(), &p.gobj("achievements"));
        self.saved_scores = merged(&obj_of(&default_highscores()), &p.gobj("highscores"));
        self.best_times = merged(&obj_of(&default_highscores()), &p.gobj("best_times"));
        self.settings = merged(&obj_of(&default_settings()), &p.gobj("settings"));
        self.powers_state = merged(&obj_of(&json!({"owned": [], "equipped": []})), &p.gobj("powers"));
        self.cosmetics_state = merged(&obj_of(&default_cosmetics()), &p.gobj("cosmetics"));
        self.online_token = p.gos("online_token");
        self.current_profile_username = p.gos("username");
        self.redeemed_codes_local = p.gstrs("redeemed_codes");
        self.friends_local = p.garr("friends").into_iter().map(|f| {
            if f.is_object() { f } else { json!({"username": null, "name": f}) }
        }).collect();
        let mut cs = std::mem::take(&mut self.cosmetics_state);
        self._migrate_cosmetics_equipped_player(&mut cs);
        self.cosmetics_state = cs;
        let mut ps = std::mem::take(&mut self.powers_state);
        let changed = self._strip_unknown_powers(&mut ps);
        self.powers_state = ps;
        for s in ["stats", "achievements", "highscores", "best_times", "settings", "powers", "cosmetics"] {
            if let Some(v) = self.live_section(s) { self._gamedata_cache.insert(s.into(), v); }
        }
        if changed { self.save_current_profile(); }
        let lang = self.settings.gs("language", "Deutsch");
        if let Some(i) = self.languages.iter().position(|l| *l == lang) { self.language_index = i; }
        self.sound_enabled = self.settings.gb("sound_enabled", true);
        self.current_game_mode = self.settings.gs("game_mode", "standard");
        self.two_player_submode = self.settings.gs("2player_submode", "coop");
        self.klassik_mode = self.current_game_mode == "klassik";
        let dpm = self.settings.gobj("difficulty_per_mode");
        let fallback = self.settings.gs("difficulty", "Mittel");
        let diff = dpm.gs(&self.current_game_mode.clone(), &fallback);
        self.settings.insert("difficulty".into(), json!(diff));
        if let Some(i) = DIFFICULTIES.iter().position(|d| *d == diff) { self.difficulty_index = i; }
        true
    }

    pub fn create_profile(&mut self, name: &str) {
        self.current_profile_name = Some(name.to_string());
        self._notif_pending_friend_requests = 0;
        self._notif_pending_clan_requests = 0;
        self._notif_unread_conversations = 0;
        self._notif_last_checked_ms = 0;
        self.stats = obj_of(&self.default_stats());
        self.achievements = Obj::new();
        self.saved_scores = obj_of(&default_highscores());
        self.best_times = obj_of(&default_highscores());
        self.settings = obj_of(&default_settings());
        self.powers_state = obj_of(&json!({"owned": [], "equipped": []}));
        self.cosmetics_state = obj_of(&default_cosmetics());
        self.online_token = None;
        self.current_profile_username = None;
        self.redeemed_codes_local = vec![];
        self.friends_local = vec![];
        for s in ["stats", "achievements", "highscores", "best_times", "settings", "powers", "cosmetics"] {
            if let Some(v) = self.live_section(s) { self._gamedata_cache.insert(s.into(), v); }
        }
        self.save_current_profile();
    }

    pub fn delete_profile(&mut self, name: &str) {
        let mut pdata = self._load_profiles_file();
        let mut profiles = pdata.gobj("profiles");
        let profile = profiles.shift_remove(name);
        pdata.insert("profiles".into(), Value::Object(profiles));
        if pdata.gos("active").as_deref() == Some(name) { pdata.insert("active".into(), Value::Null); }
        if pdata.gos("auto_start").as_deref() == Some(name) { pdata.insert("auto_start".into(), Value::Null); }
        let token = profile.as_ref().and_then(|p| p.gos("online_token"));
        if let Some(t) = &token {
            let mut pending = pdata.gstrs("_pending_online_deletions");
            if !pending.contains(t) { pending.push(t.clone()); }
            pdata.insert("_pending_online_deletions".into(), json!(pending));
        }
        self._save_profiles_file(pdata);
        if token.is_some() { self._flush_pending_online_deletions(); }
    }

    pub fn get_auto_start_profile(&mut self) -> Option<String> { self._load_profiles_file().gos("auto_start") }

    pub fn set_auto_start_profile(&mut self, name: Option<String>) {
        let mut pdata = self._load_profiles_file();
        pdata.insert("auto_start".into(), json!(name));
        self._save_profiles_file(pdata);
    }

    pub fn save_tutorial_data(&mut self, data: Value) { self.save_section("_tutorial", Some(data)); }
    pub fn load_tutorial_data(&mut self, fallback: Value) -> Value {
        let result = self.load_section("_tutorial", if fallback.is_null() { json!({}) } else { fallback.clone() });
        if is_truthy(&result) { result } else { fallback }
    }

    pub fn default_stats(&self) -> Value { crate::data::d("default_stats").clone() }

    pub fn _migrate_cosmetics_equipped_player(&self, state: &mut Obj) {
        let old = state.shift_remove("equipped_player");
        if let Some(Value::String(old)) = old {
            if old.is_empty() { return; }
            let category = crate::data::d("COSMETICS_DB").gv(&old).map(|m| m.gs("category", "")).unwrap_or_default();
            if category == "headwear" && !state.truthy("equipped_headwear") {
                state.insert("equipped_headwear".into(), json!(old));
            } else if ["eyes", "nose", "mouth"].contains(&category.as_str()) && !state.truthy(&format!("equipped_{}", category)) {
                state.insert(format!("equipped_{}", category), json!(old));
            }
        }
    }

    pub fn _migrate_cosmetics_equipped_face(&self, state: &mut Obj) {
        let old = state.shift_remove("equipped_face");
        if let Some(Value::String(old)) = old {
            if old.is_empty() { return; }
            let category = crate::data::d("COSMETICS_DB").gv(&old).map(|m| m.gs("category", "")).unwrap_or_default();
            if ["eyes", "nose", "mouth"].contains(&category.as_str()) && !state.truthy(&format!("equipped_{}", category)) {
                state.insert(format!("equipped_{}", category), json!(old));
            }
        }
    }

    /// Entfernt Power-Namen, die es in POWERS_DB nicht (mehr) gibt. Gibt zurück, ob sich etwas geändert hat.
    pub fn _strip_unknown_powers(&self, state: &mut Obj) -> bool {
        let db = crate::data::d("POWERS_DB");
        let mut changed = false;
        for key in ["owned", "equipped"] {
            if let Some(Value::Array(lst)) = state.get(key) {
                let cleaned: Vec<Value> = lst.iter().filter(|v| v.as_str().map(|n| db.gv(n).is_some()).unwrap_or(false)).cloned().collect();
                if cleaned.len() != lst.len() {
                    state.insert(key.into(), Value::Array(cleaned));
                    changed = true;
                }
            }
        }
        changed
    }
}

pub const DIFFICULTIES: [&str; 8] = ["Leicht", "Mittel", "Schwer", "Hardcore", "Unmöglich", "Albtraum", "Hölle", "Dynamisch"];

pub fn default_highscores() -> Value {
    json!({"Leicht": 0, "Mittel": 0, "Schwer": 0, "Hardcore": 0, "Unmöglich": 0, "Albtraum": 0, "Hölle": 0, "Dynamisch": 0})
}

pub fn default_cosmetics() -> Value { crate::data::d("default_cosmetics").clone() }

/// default_settings (enthält pygame-Tastencodes, daher nicht automatisch extrahiert)
pub fn default_settings() -> Value {
    use crate::pg::*;
    json!({
        "language": "English",
        "difficulty": "Mittel",
        "fullscreen": false,
        "music_volume": 0.8,
        "sfx_volume": 0.8,
        "dark_mode": true,
        "sound_enabled": true,
        "keybindings": {
            "jump":   [K_SPACE, K_w, K_UP],
            "left":   [K_LEFT, K_a],
            "right":  [K_RIGHT, K_d],
            "power1": [K_q],
            "power2": [K_e],
        },
        "menu_click_volume": 0.8,
        "click_volume": 0.8,
        "click_back_volume": 0.8,
        "hit_volume": 0.8,
        "gameover_volume": 0.8,
        "menu_music_volume": 0.8,
        "ingame_music_volume": 0.8,
        "ingame_music_track": "standard",
        "show_floor_line": true,
        "game_mode": "standard",
        "difficulty_per_mode": {"standard": "Mittel", "klassik": "Mittel", "2player": "Mittel"},
        "2player_submode": "coop",
        "custom_speed": 10,
        "custom_min_delay": 900,
        "custom_max_delay": 1600,
        "custom_oneups": true,
        "custom_vertical_obstacles": false,
        "custom_wind_sides": false,
        "custom_wind_all": false,
        "custom_free_roam": false,
    })
}

/// Ordner für Spielstand (data/), Sounds, Custom-Sprachen und key.txt.
/// - Portable, Windows-Installation, alte Python-Installation: neben dem Programm (wie bisher).
/// - Paket-Installation (/usr, /opt, macOS-.app) oder schreibgeschützter Ordner (z.B. Program Files):
///   Benutzerordner (Windows %APPDATA%\JumpNDodge, macOS ~/Library/Application Support/JumpNDodge,
///   Linux ~/.local/share/JumpNDodge = Ordner der alten Python-Installation). Liegt neben dem
///   Programm noch ein Spielstand, wird er beim ersten Start dorthin übernommen.
pub fn base_dir() -> PathBuf {
    if let Ok(d) = std::env::var("JND_BASE_DIR") { return PathBuf::from(d); }
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok().or(Some(p))).and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| PathBuf::from("."));
    if !is_system_install(&exe_dir) && dir_writable(&exe_dir) && (exe_dir.join("data").is_dir() == false || dir_writable(&exe_dir.join("data"))) {
        return exe_dir;
    }
    let ud = user_data_dir();
    let _ = std::fs::create_dir_all(ud.join("data"));
    let old = exe_dir.join("data").join("gameplay.json");
    let new = ud.join("data").join("gameplay.json");
    if old.is_file() && !new.exists() {
        if let Ok(rd) = std::fs::read_dir(exe_dir.join("data")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_file() { let _ = std::fs::copy(&p, ud.join("data").join(e.file_name())); }
            }
        }
    }
    ud
}

fn dir_writable(d: &std::path::Path) -> bool {
    let t = d.join(format!(".jnd-write-test-{}", std::process::id()));
    match std::fs::write(&t, b"x") { Ok(_) => { let _ = std::fs::remove_file(&t); true } Err(_) => false }
}

fn is_system_install(d: &std::path::Path) -> bool {
    if cfg!(windows) { return false; }
    let s = d.to_string_lossy();
    s.starts_with("/usr/") || s.starts_with("/opt/") || s.starts_with("/app/") || s.contains(".app/Contents/MacOS")
}

pub fn user_data_dir() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| home.join("AppData").join("Roaming")).join("JumpNDodge")
    } else if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("JumpNDodge")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".local").join("share")).join("JumpNDodge")
    }
}
