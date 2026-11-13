//! Spielstart (Modul-Ebene von JumpNDodge.py) und Hauptschleife (Python 16800-18042).

use super::core::*;
use super::state::{Snd, G};
use super::stats_screen::StatTab;
use crate::data::d;
use crate::game::save::*;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

const DS_TOGGLE_DEFS: [(&str, bool); 5] = [
    ("custom_oneups", true), ("custom_vertical_obstacles", false),
    ("custom_wind_sides", false), ("custom_wind_all", false), ("custom_free_roam", false),
];
const BG_TOGGLE_DEFS: [(&str, bool); 5] = [
    ("bg_stars", true), ("bg_rings_enabled", true), ("bg_animated", true), ("mono_bg", false), ("show_floor_line", true),
];

#[derive(Clone, Copy)]
enum MenuAct { Btn(super::menus::MenuAction), Mods, Daily, ProfileHub, Online }

impl G {
    /// Entspricht dem gesamten Modul-Ebene-Code von JumpNDodge.py bis zur Hauptschleife.
    pub fn new() -> G {
        let mut g = G::default();
        g.DEBUG_MODE = !cfg!(debug_assertions);
        g.WIDTH = 800;
        g.HEIGHT = 600;
        g.scale_x = 1.0;
        g.scale_y = 1.0;
        g.base_surface = Surface::new(800, 600);
        g.sound_enabled = true;
        g._time_cache.has_internet = true;
        g.font = Font::new(64);
        g.menu_font = Font::new(40);
        g.small_font = Font::new(26);
        g.mono_font = Font::sys("consolas", 22, false);
        g.badge_font = Font::new(18);
        g.tooltip_font = Font::new(32);
        g.tab_font = Font::new(20);
        g.languages = ["Deutsch", "English", "Русский", "Türkçe", "Schwiizerdütsch"].iter().map(|s| s.to_string()).collect();
        g.texts = obj_of(d("texts"));
        g._bg_stars = (0..55).map(|_| [random(), random(), uniform(1.2, 3.0), uniform(0.0, 6.28), uniform(0.0, 6.28), uniform(0.008, 0.022), uniform(0.006, 0.018)]).collect();

        g.BASE_DIR = base_dir();
        g._DATA_DIR = g.BASE_DIR.join("data");
        let _ = std::fs::create_dir_all(&g._DATA_DIR);
        g._GAMEDATA_FILE = g._DATA_DIR.join("gameplay.json");
        g._MODDED_DATA_FILE = g._DATA_DIR.join("mods").join("modded_data.json");
        if g.DEBUG_MODE {
            let l = format!("[{}] === JumpNDodge (Rust) gestartet ===", super::devconsole::now_hms_ms());
            g._debug_log_write(&l);
        }

        // Spielstand laden (Python 2195-2205)
        g.stats = obj_of(&g.load_section("stats", g.default_stats()));
        g.achievements = obj_of(&g.load_section("achievements", json!({})));
        g.saved_scores = obj_of(&g.load_section("highscores", default_highscores()));
        g.best_times = obj_of(&g.load_section("best_times", default_highscores()));
        g.modded_scores = obj_of(&g.load_modded_section("modded_highscores", default_highscores()));
        g.modded_best_times = obj_of(&g.load_modded_section("modded_best_times", default_highscores()));
        g.modded_stats = obj_of(&g.load_modded_section("modded_stats", json!({"total_time_played": 0, "total_rounds": 0, "total_1ups": 0, "coins": 0})));
        g.settings = obj_of(&g.load_section("settings", default_settings()));
        super::art::init_texture_dir(&g.BASE_DIR);
        super::art::set_legacy(g.settings.gb("legacy_art", false));
        g.powers_state = obj_of(&g.load_section("powers", json!({"owned": [], "equipped": []})));
        g.cosmetics_state = obj_of(&g.load_section("cosmetics", default_cosmetics()));
        g.sale_state = obj_of(&g.load_section("sale", json!({"date": "", "items": {}})));
        g._live_sections_ready = true;

        g.set_display_mode();
        g.reload_mods();
        g.load_sound_theme();
        g.load_custom_languages();

        let lang = g.settings.gs("language", "Deutsch");
        g.language_index = g.languages.iter().position(|l| *l == lang).unwrap_or(0);
        let diff = g.settings.gs("difficulty", "Mittel");
        g.difficulty_index = DIFFICULTIES.iter().position(|d| *d == diff).unwrap_or(1);

        let mut ps = g.powers_state.clone();
        if g._strip_unknown_powers(&mut ps) { g.powers_state = ps; g.save_section("powers", None); }
        if g.cosmetics_state.contains_key("equipped_player") {
            let mut cs = g.cosmetics_state.clone();
            g._migrate_cosmetics_equipped_player(&mut cs);
            g.cosmetics_state = cs;
            g.save_section("cosmetics", None);
        }
        if g.cosmetics_state.contains_key("equipped_face") {
            let mut cs = g.cosmetics_state.clone();
            g._migrate_cosmetics_equipped_face(&mut cs);
            g.cosmetics_state = cs;
            g.save_section("cosmetics", None);
        }
        g.initialize_cosmetic_prices();

        g.create_main_buttons();
        g.current_game_mode = "standard".into();
        g.two_player_submode = "coop".into();
        // Python brauchte die 2 s zum Laden von pygame/Sounds - Rust ist sofort fertig, kurzer Vorspann reicht
        g.loading_duration = 600;
        g.loading_to_daily_rewards = true;
        g._load_help_texts();

        g._username_gate_checking = true;
        g._friends_hub_tab = "roster".into();
        g._clan_hub_tab = "roster".into();
        g._clan_create_focus = "tag".into();
        g._clan_view_return_screen = "in_public_profile".into();
        g._chat_return_screen = "in_public_profile".into();
        g._chat_loading = true;
        g._cosmetic_inventory_return_screen = "in_loadout_menu".into();
        g.face_subcategory = "eyes".into();
        g.shop_face_subcategory = "eyes".into();
        g.admin_amount_input = "0".into();
        g._player_search_return_screen = "in_online_leaderboard".into();
        g._public_profile_return_screen = "in_online_leaderboard".into();

        g._ensure_tutorial_json();

        // Startup: Profilauswahl (Python 13859-13866)
        let profiles = g.get_all_profiles();
        let auto = g.get_auto_start_profile();
        if profiles.is_empty() {
            g.in_profile_name_input = true;
        } else if let Some(a) = auto.filter(|a| profiles.contains_key(a)) {
            g._enter_profile(&a);
        } else {
            g.in_profile_select = true;
        }
        for dname in DIFFICULTIES {
            g.session_scores.insert(dname.into(), json!(0));
            g.modded_session_scores.insert(dname.into(), json!(0));
            g.session_times.insert(dname.into(), json!(0));
            g.modded_session_times.insert(dname.into(), json!(0));
            if !g.saved_scores.contains_key(dname) { g.saved_scores.insert(dname.into(), json!(0)); }
        }

        g._start_dev_console();
        g._test_init();
        g
    }

    fn _dpm_mode_diff(&mut self, mode: &str) -> String {
        let dpm = self.settings.gobj("difficulty_per_mode");
        let dpm = if dpm.is_empty() { obj_of(&json!({"standard": "Mittel", "klassik": "Mittel", "2player": "Mittel"})) } else { dpm };
        let r = dpm.gs(mode, "Mittel");
        self.settings.insert("difficulty_per_mode".into(), Value::Object(dpm));
        r
    }
    fn _dpm_set(&mut self, mode: &str, diff: &str) {
        let _ = self._dpm_mode_diff(mode);
        let mut dpm = self.settings.gobj("difficulty_per_mode");
        dpm.insert(mode.into(), json!(diff));
        self.settings.insert("difficulty_per_mode".into(), Value::Object(dpm));
    }
    fn _set_dark(&mut self, v: bool) {
        self.settings.insert("dark_mode".into(), json!(v));
        self.save_section("settings", None);
        self.play_sound(Snd::Click);
    }
    fn _ds_select_mode(&mut self, idx: usize) {
        self.current_game_mode = ["custom", "standard", "klassik", "2player"][idx].into();
        self.klassik_mode = self.current_game_mode == "klassik";
        let m = self.current_game_mode.clone();
        self.settings.insert("game_mode".into(), json!(m));
        if m != "custom" {
            let dfm = self._dpm_mode_diff(&m);
            if let Some(i) = DIFFICULTIES.iter().position(|x| *x == dfm) { self.difficulty_index = i; }
            self.settings.insert("difficulty".into(), json!(DIFFICULTIES[self.difficulty_index]));
        }
        self.save_section("settings", None);
        self.update_button_texts();
    }
    fn _ds_set_submode(&mut self, m: &str) {
        self.two_player_submode = m.into();
        self.settings.insert("2player_submode".into(), json!(m));
        self.save_section("settings", None);
        self.play_sound(Snd::Click);
    }
    fn _ds_toggle(&mut self, ti: usize) {
        let (skey, default) = DS_TOGGLE_DEFS[ti];
        let nv = !self.settings.gb(skey, default);
        self.settings.insert(skey.into(), json!(nv));
        if nv && skey == "custom_wind_sides" { self.settings.insert("custom_wind_all".into(), json!(false)); }
        else if nv && skey == "custom_wind_all" { self.settings.insert("custom_wind_sides".into(), json!(false)); }
        self.save_section("settings", None);
        self.play_sound(Snd::Click);
    }
    fn _ds_pick_diff(&mut self, i: usize) {
        self.difficulty_index = i;
        self.settings.insert("difficulty".into(), json!(DIFFICULTIES[i]));
        let m = self.current_game_mode.clone();
        self._dpm_set(&m, DIFFICULTIES[i]);
        self.save_section("settings", None);
        self.update_button_texts();
        self.play_sound(Snd::Click);
    }
    fn _ds_slider_geom(&self) -> (i32, i32) {
        let main_x = 24 + 180 + 24;
        let card_w = 300.max(self.WIDTH - main_x - 24);
        let label_w = ["Geschwindigkeit:", "Min-Pause:", "Max-Pause:"].iter().map(|l| self.menu_font.size(l).0).max().unwrap_or(0);
        (main_x + label_w + 24, card_w - (label_w + 24) - 80)
    }
    fn _back_to_menu_from_ds(&mut self) { self.in_difficulty_select = false; self.in_menu = true; }

    fn _ctrl_bind_key(&mut self, action: &str, key: i32) {
        if MENU_NAV_ACTIONS.contains(&action) {
            let mut ex = self.get_menu_nav_extra_keys();
            let mut l = ints(ex.get(action).unwrap_or(&json!([])));
            if !l.contains(&(key as i64)) { l.push(key as i64); }
            ex.insert(action.into(), json!(l));
            self.save_menu_nav_extra_keys(ex);
        } else {
            let mut kb = self.get_keybindings();
            if let Some(e) = kb.iter_mut().find(|(a, _)| a == action) { if !e.1.contains(&key) { e.1.push(key); } }
            self.save_keybindings(&kb);
        }
    }
    fn _ctrl_unbind_key(&mut self, action: &str, key: i32) {
        if MENU_NAV_ACTIONS.contains(&action) {
            let mut ex = self.get_menu_nav_extra_keys();
            let mut l = ints(ex.get(action).unwrap_or(&json!([])));
            if let Some(p) = l.iter().position(|k| *k == key as i64) {
                l.remove(p);
                ex.insert(action.into(), json!(l));
                self.save_menu_nav_extra_keys(ex);
            }
        } else {
            let mut kb = self.get_keybindings();
            if let Some(e) = kb.iter_mut().find(|(a, _)| a == action) {
                if let Some(p) = e.1.iter().position(|k| *k == key) { e.1.remove(p); self.save_keybindings(&kb); }
            }
        }
    }
    fn _ctrl_reset_active_tab(&mut self) {
        if self._ctrl_active_tab == 2 {
            let mut m = serde_json::Map::new();
            for a in MENU_NAV_ACTIONS { m.insert(a.into(), json!([])); }
            self.save_menu_nav_extra_keys(m);
        } else {
            let acts: &[&str] = if self._ctrl_active_tab == 0 { &STANDARD_ACTIONS } else { &TWO_P_ACTIONS };
            let mut kb = self.get_keybindings();
            let defs = default_kb();
            for a in acts {
                let dv = defs.iter().find(|(n, _)| n == a).map(|(_, v)| v.clone()).unwrap_or_default();
                if let Some(e) = kb.iter_mut().find(|(n, _)| n == a) { e.1 = dv; } else { kb.push((a.to_string(), dv)); }
            }
            self.save_keybindings(&kb);
        }
    }

    fn _run_menu_act(&mut self, a: MenuAct) {
        match a {
            MenuAct::Btn(b) => self.run_menu_action(b),
            MenuAct::Mods => { self.play_sound(Snd::MenuClick); self.in_menu = false; self.in_mods_menu = true; }
            MenuAct::Daily => self.open_daily_rewards_menu(),
            MenuAct::ProfileHub => self.open_profile_hub(),
            MenuAct::Online => self.open_online_hub(),
        }
    }

    /// Die Hauptschleife (while True: am Ende von JumpNDodge.py)
    pub fn main_loop(&mut self) -> ! {
        self.update_button_texts();
        self.maybe_auto_update_check();
        let mut clock = Clock::new();
        loop {
            self.sync_window_size();
            if self.in_profile_name_input {
                let confirm_rect = self.draw_profile_name_input();
                for ev in self.event_get() {
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, unicode, .. } => {
                            if key == K_RETURN { self._confirm_profile_name(); }
                            else if key == K_BACKSPACE { self._profile_name_text.pop(); }
                            else if self.is_back_key_ex(key, false) {
                                if !self.get_all_profiles().is_empty() { self.in_profile_name_input = false; self.in_profile_select = true; }
                            } else if let Some(ch) = unicode.chars().next() {
                                if (32..=126).contains(&(ch as u32)) && self._profile_name_text.chars().count() < 20 { self._profile_name_text.push(ch); }
                            }
                        }
                        Event::MouseButtonDown { .. } => {
                            if confirm_rect.collidepoint(self.get_mouse_pos_logical()) { self._confirm_profile_name(); }
                        }
                        _ => {}
                    }
                }
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_profile_select {
                let (cards, checks) = self.draw_profile_select();
                let rects: Vec<Rect> = cards.iter().map(|(r, _)| *r).collect();
                'ev: for ev in self.event_get() {
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, .. } => {
                            if let Some(dd) = self.nav_direction(key) {
                                let cur = self.nav_get_focus("profile_select", rects.len());
                                self.nav_set_focus("profile_select", nav_move(&rects, cur, dd));
                            } else if self.is_nav_confirm(key) {
                                if let Some(fi) = self.nav_get_focus("profile_select", rects.len()).filter(|i| *i < cards.len()) {
                                    let name = cards[fi].1.clone();
                                    self.play_sound(Snd::MenuClick);
                                    if name == "+" { self._open_new_profile_input(true); } else { self._enter_profile(&name); }
                                }
                            }
                        }
                        Event::MouseButtonDown { .. } => {
                            let mp = self.get_mouse_pos_logical();
                            for (cb, cn) in &checks {
                                if cb.collidepoint(mp) {
                                    let cur = self.get_auto_start_profile();
                                    self.set_auto_start_profile(if cur.as_deref() == Some(cn.as_str()) { None } else { Some(cn.clone()) });
                                    self.play_sound(Snd::Click);
                                    continue 'ev;
                                }
                            }
                            for (r, name) in &cards {
                                if r.collidepoint(mp) {
                                    if name == "+" { self.play_sound(Snd::MenuClick); self._open_new_profile_input(true); }
                                    else {
                                        let del = Rect::new(r.right() - 28, r.y + 6, 22, 22);
                                        if del.collidepoint(mp) {
                                            self.play_sound(Snd::ClickBack);
                                            self.delete_profile(name);
                                            if self.get_all_profiles().is_empty() { self.in_profile_select = false; self.in_profile_name_input = true; }
                                        } else { self.play_sound(Snd::MenuClick); self._enter_profile(name); }
                                    }
                                    break;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_language_select {
                let (weiter, light, dark, cards) = self.draw_language_select();
                let mut rects = cards.clone();
                if !self._lang_from_settings { rects.push(light); rects.push(dark); }
                rects.push(weiter);
                let wi = rects.len() - 1;
                let langs: Vec<String> = d("_LANG_DATA").as_array().map(|a| a.iter().map(|l| l.gs("key", "")).collect()).unwrap_or_default();
                for ev in self.event_get() {
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, .. } => {
                            if let Some(dd) = self.nav_direction(key) {
                                let cur = self.nav_get_focus_d("language_select", rects.len(), wi);
                                self.nav_set_focus("language_select", nav_move(&rects, cur, dd));
                            } else if self.is_nav_confirm(key) {
                                let fi = self.nav_get_focus_d("language_select", rects.len(), wi);
                                if fi == Some(wi) { self._finish_language_select(); }
                                else if let Some(f) = fi.filter(|f| *f < cards.len()) { if let Some(k) = langs.get(f) { self._select_language(k); } self.play_sound(Snd::Click); }
                                else if !self._lang_from_settings && fi == Some(cards.len()) { self._set_dark(false); }
                                else if !self._lang_from_settings && fi == Some(cards.len() + 1) { self._set_dark(true); }
                            }
                        }
                        Event::MouseButtonDown { .. } => {
                            let pt = self.get_mouse_pos_logical();
                            for (i, cr) in cards.iter().enumerate() {
                                if i < langs.len() && cr.collidepoint(pt) { let k = langs[i].clone(); self._select_language(&k); self.play_sound(Snd::Click); break; }
                            }
                            if light.collidepoint(pt) { self._set_dark(false); } else if dark.collidepoint(pt) { self._set_dark(true); }
                            if weiter.collidepoint(pt) { self.play_sound(Snd::MenuClick); self._finish_language_select(); }
                        }
                        _ => {}
                    }
                }
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_tutorial {
                let total = self._get_tutorial_steps().len();
                for ev in self.event_get() {
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, .. } => {
                            if self.is_back_key(key) { self._finish_tutorial(); }
                            else if [K_RIGHT, K_d, K_RETURN, K_SPACE].contains(&key) {
                                if self.tutorial_step + 1 >= total { self._finish_tutorial(); } else { self.tutorial_step += 1; }
                            } else if [K_LEFT, K_a].contains(&key) && self.tutorial_step > 0 { self.tutorial_step -= 1; }
                        }
                        Event::MouseButtonDown { .. } => {
                            let mp = self.get_mouse_pos_logical();
                            let (pw, ph) = (560, 320);
                            let (px, py) = (self.WIDTH / 2 - pw / 2, self.HEIGHT / 2 - ph / 2);
                            let by = py + ph - 52;
                            let next = Rect::new(px + pw - 160, by, 140, 40);
                            let back = Rect::new(px + 20, by, 140, 40);
                            if next.collidepoint(mp) {
                                if self.tutorial_step + 1 >= total { self._finish_tutorial(); } else { self.tutorial_step += 1; }
                            } else if back.collidepoint(mp) && self.tutorial_step > 0 { self.tutorial_step -= 1; }
                        }
                        _ => {}
                    }
                }
                self.draw_tutorial();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_menu && self.update_dialog_if_due() {
                continue;
            } else if self.in_menu {
                if self.current_game_mode == "mod" { self.current_game_mode = "standard".into(); self._mod_mode = None; }
                let has_mods = self.mods_installed(); // Knopf unten links nur mit installierten Mods (sonst unter Mehr)
                let has_online = self.WIDTH >= 500;
                let daily = Rect::new(self.WIDTH - 200, 15, 180, 40);
                let mods_btn = Rect::new(20, self.HEIGHT - 56, 130, 36);
                let hub = self._get_profile_hub_btn_rect();
                let online = Rect::new(self.WIDTH - 235, self.HEIGHT - 118, 220, 56);
                let mut rects: Vec<Rect> = self.buttons.iter().map(|b| b.rect).collect();
                let mut acts: Vec<MenuAct> = self.buttons.iter().map(|b| MenuAct::Btn(b.action)).collect();
                if has_mods { rects.push(mods_btn); acts.push(MenuAct::Mods); }
                rects.push(daily); acts.push(MenuAct::Daily);
                rects.push(hub); acts.push(MenuAct::ProfileHub);
                if has_online { rects.push(online); acts.push(MenuAct::Online); }
                for ev in self.event_get() {
                    if let Event::Quit = ev { self.quit(); }
                    if let Event::KeyDown { key, .. } = ev {
                        if let Some(dd) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("menu", rects.len());
                            self.nav_set_focus("menu", nav_move(&rects, cur, dd));
                        } else if self.is_nav_confirm(key) {
                            if let Some(i) = self.nav_get_focus("menu", rects.len()).filter(|i| *i < acts.len()) { self._run_menu_act(acts[i]); }
                        }
                    }
                    let mp = self.get_mouse_pos_logical();
                    if let Event::MouseButtonDown { .. } = ev {
                        let btns: Vec<(Rect, super::menus::MenuAction)> = self.buttons.iter().map(|b| (b.rect, b.action)).collect();
                        for (r, a) in btns { if r.collidepoint(mp) { self.run_menu_action(a); } }
                        if daily.collidepoint(mp) { self.play_sound(Snd::MenuClick); self.open_daily_rewards_menu(); }
                        if has_mods && mods_btn.collidepoint(mp) { self._run_menu_act(MenuAct::Mods); }
                        if hub.collidepoint(mp) { self.open_profile_hub(); }
                        if has_online && online.collidepoint(mp) { self.open_online_hub(); }
                    }
                }
                self.draw_menu();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_difficulty_select {
                self._difficulty_select_events();
                self.draw_difficulty_select();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_profile_hub {
                let (swatches, names, hub_btns) = self.draw_profile_hub();
                'ev: for ev in self.event_get() {
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, .. } => {
                            if key == K_m || self.is_back_key(key) { self.back_to_main_from_profile_hub(); continue; }
                            let mut hr = swatches.clone();
                            hr.extend(hub_btns.iter().copied());
                            if let Some(dd) = self.nav_direction(key) {
                                let cur = self.nav_get_focus("profile_hub", hr.len());
                                self.nav_set_focus("profile_hub", nav_move(&hr, cur, dd));
                            } else if self.is_nav_confirm(key) {
                                match self.nav_get_focus("profile_hub", hr.len()) {
                                    Some(h) if h < names.len() => { self.settings.insert("player_color".into(), json!(names[h])); self.save_section("settings", None); self.play_sound(Snd::Click); }
                                    Some(h) => self._profile_hub_btn(h - names.len()),
                                    None => {}
                                }
                            }
                        }
                        Event::MouseButtonDown { .. } => {
                            let mp = self.get_mouse_pos_logical();
                            for (sr, cn) in swatches.iter().zip(names.iter()) {
                                if sr.collidepoint(mp) { self.settings.insert("player_color".into(), json!(cn)); self.save_section("settings", None); self.play_sound(Snd::Click); continue 'ev; }
                            }
                            if let Some(idx) = hub_btns.iter().position(|r| r.collidepoint(mp)) { self._profile_hub_btn(idx); }
                        }
                        _ => {}
                    }
                }
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_more_menu {
                let (sx, sy, gap) = (self.WIDTH / 2 - 180, 128, 54);
                let actions: Vec<&str> = if self._check_admin_keyfile() { vec!["admin", "classic", "help", "redeem_code", "replays", "mods", "back"] } else { vec!["classic", "help", "redeem_code", "replays", "mods", "back"] };
                let rects: Vec<Rect> = (0..actions.len()).map(|i| Rect::new(sx, sy + i as i32 * gap, 360, 48)).collect();
                for ev in self.event_get() {
                    let mut act: Option<&str> = None;
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, .. } => {
                            if key == K_m || self.is_back_key(key) { act = Some("back"); }
                            else if let Some(dd) = self.nav_direction(key) {
                                let cur = self.nav_get_focus("more_menu", rects.len());
                                self.nav_set_focus("more_menu", nav_move(&rects, cur, dd));
                            } else if self.is_nav_confirm(key) { act = Some(actions[self.nav_get_focus("more_menu", rects.len()).unwrap_or(0).min(actions.len() - 1)]); }
                        }
                        Event::MouseButtonDown { .. } => {
                            let mp = self.get_mouse_pos_logical();
                            if let Some(i) = rects.iter().position(|r| r.collidepoint(mp)) { act = Some(actions[i]); }
                        }
                        _ => {}
                    }
                    if let Some(a) = act {
                        self.play_sound(if a == "back" { Snd::ClickBack } else { Snd::MenuClick });
                        match a {
                            "admin" => self.set_more_to_admin(),
                            "classic" => self.open_classic_game(),
                            "help" => { self.in_more_menu = false; self.in_tutorial = true; self.tutorial_step = 0; }
                            "redeem_code" => self.open_redeem_code_from_more(),
                            "replays" => self.run_replays_menu(),
                            "mods" => { self.in_more_menu = false; self.in_mods_menu = true; }
                            _ => self.back_to_main_from_more(),
                        }
                    }
                }
                self.draw_more_menu();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_classic_game {
                self.run_classic_game();
            } else if self.in_mods_menu {
                // Rust Edition: Lua-Mods (die Python-Mods werden nicht unterstützt)
                self.mods_menu_loop();
            } else if self.in_achievements_screen {
                self._achievements_events();
                self.draw_achievements_screen();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_settings_menu {
                self._settings_events();
                self.draw_settings_menu();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_controls_menu {
                self._controls_events();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_settings_more_menu {
                if self.update_dialog_if_due() { continue; }
                self.settings_page_loop();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_experimental_menu {
                for ev in self.event_get() {
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, .. } => {
                            if key == K_m || self.is_back_key(key) || self.is_nav_confirm(key) { self.back_to_settings_from_experimental(); }
                        }
                        Event::MouseButtonDown { .. } => {
                            let mp = self.get_mouse_pos_logical();
                            if Rect::new(self.WIDTH / 2 - 180, 180, 360, 48).collidepoint(mp) { self.play_sound(Snd::ClickBack); self.back_to_settings_from_experimental(); }
                        }
                        _ => {}
                    }
                }
                self.draw_experimental_menu();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_settings_sound_menu {
                self._sound_menu_events();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_help {
                for ev in self.event_get() {
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, .. } => {
                            if key == K_m || self.is_back_key(key) { self.in_help = false; self.in_menu = true; }
                            else if key == K_UP { self.help_scroll = 0.max(self.help_scroll - 28); }
                            else if key == K_DOWN { self.help_scroll += 28; }
                        }
                        _ => {}
                    }
                }
                self.draw_help_menu();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.game_running {
                if self.current_game_mode == "2player" { self.run_game_2p(); } else { self.run_game(); }
            } else if self.showing_stats {
                self.draw_statistics();
                self._stats_events();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_shop { self.shop_loop(); }
            else if self.in_loadout { self.loadout_loop(); }
            else if self.in_shop_menu { self.shop_menu_loop(); }
            else if self.in_cosmetic_shop { self.cosmetic_shop_loop(); }
            else if self.in_loadout_menu { self.loadout_menu_loop(); }
            else if self.in_cosmetic_inventory { self.cosmetic_inventory_loop(); }
            else if self.in_online_leaderboard { self.online_leaderboard_loop(); }
            else if self.in_player_search { self.player_search_loop(); }
            else if self.in_public_profile { self.public_profile_loop(); }
            else if self.in_online_hub { self.online_hub_loop(); }
            else if self.in_multiplayer { self.multiplayer_loop(); }
            else if self.in_friends_hub { self.friends_hub_loop(); }
            else if self.in_messages_hub { self.messages_hub_loop(); }
            else if self.in_clan_hub { self.clan_hub_loop(); }
            else if self.in_clan_create { self.clan_create_loop(); }
            else if self.in_clan_search { self.clan_search_loop(); }
            else if self.in_clan_view { self.clan_view_loop(); }
            else if self.in_chat { self.chat_loop(); }
            else if self.in_username_gate { self.username_gate_loop(); }
            else if self.in_rename_profile { self.rename_profile_loop(); }
            else if self.in_redeem_code { self.redeem_code_loop(); }
            else if self.in_admin_login { self.admin_login_loop(); }
            else if self.in_admin_panel { self.admin_panel_loop(); }
            else if self.in_loading {
                for ev in self.event_get() { if let Event::Quit = ev { self.quit(); } }
                self.draw_loading_screen();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else if self.in_daily_rewards {
                let (can_claim, _) = self.can_claim_daily_reward();
                let claim = Rect::new(self.WIDTH / 2 - 100, 200, 200, 50);
                let back = Rect::new(self.WIDTH - 180, self.HEIGHT - 60, 150, 40);
                let mut rects = vec![];
                let mut acts = vec![];
                if can_claim { rects.push(claim); acts.push("claim"); }
                rects.push(back); acts.push("back");
                for ev in self.event_get() {
                    let mut a: Option<&str> = None;
                    match ev {
                        Event::Quit => self.quit(),
                        Event::KeyDown { key, .. } => {
                            if key == K_m || self.is_back_key(key) { a = Some("back"); }
                            else if let Some(dd) = self.nav_direction(key) {
                                let cur = self.nav_get_focus("daily_rewards", rects.len());
                                self.nav_set_focus("daily_rewards", nav_move(&rects, cur, dd));
                            } else if self.is_nav_confirm(key) {
                                if let Some(i) = self.nav_get_focus("daily_rewards", rects.len()).filter(|i| *i < acts.len()) { a = Some(acts[i]); }
                            }
                        }
                        Event::MouseButtonDown { .. } => {
                            let mp = self.get_mouse_pos_logical();
                            if can_claim && claim.collidepoint(mp) { a = Some("claim"); } else if back.collidepoint(mp) { a = Some("back"); }
                        }
                        _ => {}
                    }
                    match a {
                        Some("claim") => { self.play_sound(Snd::Click); self.claim_daily_reward_from_menu(); }
                        Some("back") => { self.play_sound(Snd::ClickBack); self.back_to_main_from_daily_rewards(); }
                        _ => {}
                    }
                }
                self.draw_daily_rewards_menu();
                { let fps = self.target_fps(); clock.tick(fps); }
            } else {
                // Kein Screen aktiv (sollte nie passieren) - Frame trotzdem abschließen
                for ev in self.event_get() { if let Event::Quit = ev { self.quit(); } }
                { let fps = self.target_fps(); clock.tick(fps); }
            }
        }
    }

    fn _profile_hub_btn(&mut self, i: usize) {
        match i {
            0 => self.open_stats_from_profile_hub(),
            1 => self.open_profile_select_from_hub(),
            2 => self.open_rename_profile_from_hub(),
            3 => self.back_to_main_from_profile_hub(),
            _ => {}
        }
    }

    fn _difficulty_select_events(&mut self) {
        let (sbx, sbw) = (24, 180);
        let main_x = sbx + sbw + 24;
        let main_w = 300.max(self.WIDTH - main_x - 24);
        let y0 = 92; // wie draw_difficulty_select
        let custom_btn = Rect::new(sbx, y0, sbw, 56);
        let preset_y = custom_btn.bottom() + 20 + 22;
        let standard_btn = Rect::new(sbx, preset_y, sbw, 42);
        let klassik_btn = Rect::new(sbx, standard_btn.bottom() + 8, sbw, 42);
        let tp_btn = Rect::new(sbx, klassik_btn.bottom() + 8, sbw, 42);
        let mut sidebar = vec![custom_btn, standard_btn, klassik_btn, tp_btn];
        let sub_w = (sbw - 8) / 2;
        let sub_y = tp_btn.bottom() + 10;
        let tab_coop = Rect::new(sbx, sub_y, sub_w, 32);
        let tab_vs = Rect::new(sbx + sub_w + 8, sub_y, sub_w, 32);
        if self.current_game_mode == "2player" { sidebar.push(tab_coop); sidebar.push(tab_vs); }
        let main_rects: Vec<Rect> = if self.current_game_mode == "custom" {
            let mut v: Vec<Rect> = [30, 100, 170].iter().map(|sy| Rect::new(main_x, y0 + sy - 20, main_w, 40)).collect();
            let ty = y0 + 210 + 34;
            for ti in 0..DS_TOGGLE_DEFS.len() as i32 { v.push(Rect::new(main_x + (ti % 2) * (220 + 12), ty + (ti / 2) * (34 + 10), 220, 34)); }
            v
        } else {
            (0..DIFFICULTIES.len() as i32).map(|i| Rect::new(main_x, y0 + i * 54, main_w, 48)).collect()
        };
        let by = self.HEIGHT - 54;
        let start_rect = Rect::new(self.WIDTH / 2 + 8, by, 200, 44);
        let back_rect = Rect::new(self.WIDTH / 2 - 208, by, 200, 44);
        let mut all = sidebar.clone();
        all.extend(main_rects.iter().copied());
        all.push(start_rect);
        all.push(back_rect);
        let sc = sidebar.len();
        let start_idx = sc + main_rects.len();
        let back_idx = start_idx + 1;

        for ev in self.event_get() {
            match ev {
                Event::Quit => self.quit(),
                Event::KeyDown { key, .. } => {
                    if let Some(se) = self._ds_slider_edit {
                        let sd = self.nav_direction(key);
                        if sd == Some("left") || sd == Some("right") {
                            let delta: i64 = if sd == Some("left") { -1 } else { 1 };
                            match se {
                                0 => { let c = self.settings.gi("custom_speed", 10); self.settings.insert("custom_speed".into(), json!((c + delta).clamp(4, 35))); }
                                1 => { let c = self.settings.gi("custom_min_delay", 900); let mx = self.settings.gi("custom_max_delay", 1600) - 100; self.settings.insert("custom_min_delay".into(), json!(100.max((c + delta * 50).min(mx)))); }
                                2 => { let c = self.settings.gi("custom_max_delay", 1600); let mn = self.settings.gi("custom_min_delay", 900) + 100; self.settings.insert("custom_max_delay".into(), json!(2500.min((c + delta * 50).max(mn)))); }
                                _ => {}
                            }
                            self.save_section("settings", None);
                        } else if self.is_nav_confirm(key) || self.is_back_key(key) { self._ds_slider_edit = None; }
                    } else if key == K_m || self.is_back_key(key) {
                        self._back_to_menu_from_ds();
                    } else if let Some(dd) = self.nav_direction(key) {
                        let cur = self.nav_get_focus("difficulty_select", all.len());
                        self.nav_set_focus("difficulty_select", nav_move(&all, cur, dd));
                    } else if self.is_nav_confirm(key) {
                        let fi = self.nav_get_focus("difficulty_select", all.len());
                        match fi {
                            Some(f) if f < 4 => {
                                self._ds_select_mode(f);
                                self.play_sound(Snd::Click);
                                self.nav_set_focus("difficulty_select", Some(f));
                                self._ds_slider_edit = None;
                            }
                            Some(4) if self.current_game_mode == "2player" => self._ds_set_submode("coop"),
                            Some(5) if self.current_game_mode == "2player" => self._ds_set_submode("vs"),
                            Some(f) if f == start_idx => self.actually_start_game(),
                            Some(f) if f == back_idx => { self.play_sound(Snd::ClickBack); self._back_to_menu_from_ds(); }
                            Some(f) if f >= sc && f < start_idx => {
                                let mi = f - sc;
                                if self.current_game_mode == "custom" && mi < 3 { self._ds_slider_edit = Some(mi as i32); self.play_sound(Snd::Click); }
                                else if self.current_game_mode == "custom" { self._ds_toggle(mi - 3); }
                                else { self._ds_pick_diff(mi); }
                            }
                            _ => {}
                        }
                    }
                }
                Event::MouseButtonDown { .. } => {
                    let mp = self.get_mouse_pos_logical();
                    let (mx, my) = mp;
                    let tab = [custom_btn, standard_btn, klassik_btn, tp_btn].iter().position(|r| r.collidepoint(mp));
                    if let Some(t) = tab {
                        self.current_game_mode = ["custom", "standard", "klassik", "2player"][t].into();
                        self.klassik_mode = t == 2;
                        self.play_sound(Snd::Click);
                        self._ds_select_mode(t);
                        self.nav_set_focus("difficulty_select", Some(t));
                        self._ds_slider_edit = None;
                    }
                    if self.current_game_mode == "2player" {
                        if tab_coop.collidepoint(mp) { self._ds_set_submode("coop"); }
                        else if tab_vs.collidepoint(mp) { self._ds_set_submode("vs"); }
                    }
                    if self.current_game_mode == "custom" {
                        let (slx, slw) = self._ds_slider_geom();
                        let hx = |v: f64, lo: f64, hi: f64| (slx as f64 + ((v - lo) / (hi - lo)).clamp(0.0, 1.0) * slw as f64) as i32;
                        let (ys, ymin, ymax) = (y0 + 30, y0 + 100, y0 + 170);
                        let hs = hx(self.settings.gf("custom_speed", 10.0), 4.0, 35.0);
                        let hmin = hx(self.settings.gf("custom_min_delay", 900.0), 100.0, 2000.0);
                        let hmax = hx(self.settings.gf("custom_max_delay", 1600.0), 200.0, 2500.0);
                        if (mx - hs).abs() <= 14 && (my - ys).abs() <= 14 { self._custom_speed_dragging = true; }
                        else if (mx - hmin).abs() <= 14 && (my - ymin).abs() <= 14 { self._custom_min_delay_dragging = true; }
                        else if (mx - hmax).abs() <= 14 && (my - ymax).abs() <= 14 { self._custom_max_delay_dragging = true; }
                        else {
                            let ty = y0 + 210 + 34;
                            for ti in 0..DS_TOGGLE_DEFS.len() as i32 {
                                let tr = Rect::new(main_x + (ti % 2) * (220 + 12), ty + (ti / 2) * (34 + 10), 220, 34);
                                if tr.collidepoint(mp) { self._ds_toggle(ti as usize); break; }
                            }
                        }
                        if start_rect.collidepoint(mp) { self.actually_start_game(); }
                        else if back_rect.collidepoint(mp) { self.play_sound(Snd::ClickBack); self._back_to_menu_from_ds(); }
                    } else {
                        let mut clicked = false;
                        for i in 0..DIFFICULTIES.len() {
                            if Rect::new(main_x, y0 + i as i32 * 56, main_w, 50).collidepoint(mp) { self._ds_pick_diff(i); clicked = true; break; }
                        }
                        if !clicked {
                            if start_rect.collidepoint(mp) { self.actually_start_game(); }
                            else if back_rect.collidepoint(mp) { self.play_sound(Snd::ClickBack); self._back_to_menu_from_ds(); }
                        }
                    }
                }
                Event::MouseButtonUp { .. } => {
                    self._custom_speed_dragging = false;
                    self._custom_min_delay_dragging = false;
                    self._custom_max_delay_dragging = false;
                }
                Event::MouseMotion { .. } if self.current_game_mode == "custom" && (self._custom_speed_dragging || self._custom_min_delay_dragging || self._custom_max_delay_dragging) => {
                    let (mx, _) = self.get_mouse_pos_logical();
                    let (slx, slw) = self._ds_slider_geom();
                    let frac = ((mx - slx) as f64 / slw as f64).clamp(0.0, 1.0);
                    if self._custom_speed_dragging { self.settings.insert("custom_speed".into(), json!(py_round(4.0 + frac * 31.0) as i64)); }
                    else if self._custom_min_delay_dragging {
                        let nm = py_round(100.0 + frac * 1900.0) as i64;
                        let v = 100.max(nm.min(self.settings.gi("custom_max_delay", 1600) - 100));
                        self.settings.insert("custom_min_delay".into(), json!(v));
                    } else if self._custom_max_delay_dragging {
                        let nm = py_round(200.0 + frac * 2300.0) as i64;
                        let v = 2500.min(nm.max(self.settings.gi("custom_min_delay", 900) + 100));
                        self.settings.insert("custom_max_delay".into(), json!(v));
                    }
                    self.save_section("settings", None);
                }
                _ => {}
            }
        }
    }

    fn _achievements_events(&mut self) {
        for ev in self.event_get() {
            match ev {
                Event::Quit => self.quit(),
                Event::KeyDown { key, .. } => {
                    if key == K_m || self.is_back_key(key) {
                        if self._ach_detail_code.is_some() { self._ach_detail_code = None; }
                        else { self.in_achievements_screen = false; self.in_more_menu = true; }
                    } else if self._ach_detail_code.is_some() {
                        if self.is_nav_confirm(key) { self._ach_detail_code = None; }
                    } else {
                        let (ft, fb) = (75 + 18, self.HEIGHT - 45);
                        let mut nr: Vec<Rect> = self._ach_row_rects.iter().map(|(r, _)| *r).collect();
                        nr.push(Rect::new(self.WIDTH - 120, 15, 100, 35));
                        let n_rows = self._ach_row_rects.len();
                        if let Some(dd) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("achievements", nr.len());
                            let ni = nav_move(&nr, cur, dd);
                            if ni != cur {
                                self.nav_set_focus("achievements", ni);
                                if let Some(n) = ni.filter(|n| *n < n_rows) { self._ach_scroll = 0.max(self._ach_scroll + nav_reveal_delta(nr[n], ft, fb)); }
                            }
                        } else if self.is_nav_confirm(key) {
                            match self.nav_get_focus("achievements", nr.len()) {
                                Some(i) if i < n_rows => { self.play_sound(Snd::MenuClick); self._ach_detail_code = Some(self._ach_row_rects[i].1.clone()); }
                                Some(i) if i == n_rows => { self.play_sound(Snd::ClickBack); self.in_achievements_screen = false; self.in_more_menu = true; }
                                _ => {}
                            }
                        }
                    }
                }
                Event::MouseWheel { y, .. } => { if self._ach_detail_code.is_none() { self._ach_scroll = 0.max(self._ach_scroll - y * 30); } }
                Event::MouseButtonDown { .. } => {
                    let mp = self.get_mouse_pos_logical();
                    if self._ach_detail_code.is_some() { self._ach_detail_code = None; }
                    else if Rect::new(self.WIDTH - 120, 15, 100, 35).collidepoint(mp) { self.play_sound(Snd::ClickBack); self.in_achievements_screen = false; self.in_more_menu = true; }
                    else if let Some((_, code)) = self._ach_row_rects.iter().find(|(r, _)| r.collidepoint(mp)).cloned() { self.play_sound(Snd::MenuClick); self._ach_detail_code = Some(code); }
                }
                _ => {}
            }
        }
    }


    fn _controls_events(&mut self) {
        let (key_rects, add_rects, reset, back, tab_rects) = self.draw_controls_menu();
        let mut nav: Vec<Rect> = add_rects.iter().map(|(_, r)| *r).collect();
        nav.push(reset);
        nav.push(back);
        for ev in self.event_get() {
            match ev {
                Event::Quit => self.quit(),
                Event::KeyDown { key, .. } => {
                    if let Some(act) = self._ctrl_listening.clone() {
                        if key == K_ESCAPE || key == K_CAPSLOCK { self._ctrl_listening = None; }
                        else { self._ctrl_bind_key(&act, key); self._ctrl_listening = None; }
                    } else if key == K_m || self.is_back_key(key) { self.in_controls_menu = false; self.in_settings_menu = true; }
                    else if let Some(dd) = self.nav_direction(key) {
                        let cur = self.nav_get_focus("controls_menu", nav.len());
                        self.nav_set_focus("controls_menu", nav_move(&nav, cur, dd));
                    } else if self.is_nav_confirm(key) {
                        match self.nav_get_focus("controls_menu", nav.len()) {
                            Some(i) if i < add_rects.len() => { self._ctrl_listening = Some(add_rects[i].0.clone()); self.play_sound(Snd::MenuClick); }
                            Some(i) if i == add_rects.len() => { self._ctrl_reset_active_tab(); self.play_sound(Snd::Click); }
                            Some(i) if i == add_rects.len() + 1 => { self.in_controls_menu = false; self.in_settings_menu = true; self.play_sound(Snd::ClickBack); }
                            _ => {}
                        }
                    }
                }
                Event::MouseWheel { y, .. } => self._ctrl_scroll -= y * 40,
                Event::MouseButtonDown { .. } => {
                    let mp = self.get_mouse_pos_logical();
                    if let Some(ti) = tab_rects.iter().position(|r| r.collidepoint(mp)) {
                        if ti != self._ctrl_active_tab { self._ctrl_active_tab = ti; self._ctrl_scroll = 0; self.play_sound(Snd::Click); }
                        continue;
                    }
                    if let Some((a, k, _)) = key_rects.iter().find(|(_, _, r)| r.collidepoint(mp)).cloned() { self._ctrl_unbind_key(&a, k); self.play_sound(Snd::Click); continue; }
                    if let Some((a, _)) = add_rects.iter().find(|(_, r)| r.collidepoint(mp)).cloned() { self._ctrl_listening = Some(a); self.play_sound(Snd::MenuClick); continue; }
                    if reset.collidepoint(mp) { self._ctrl_reset_active_tab(); self.play_sound(Snd::Click); }
                    else if back.collidepoint(mp) { self.in_controls_menu = false; self.in_settings_menu = true; self.play_sound(Snd::ClickBack); }
                }
                _ => {}
            }
        }
    }

    fn _sound_menu_events(&mut self) {
        enum K { Toggle, Back, Adj(&'static str, f64), Track(String) }
        let (toggle, back, minus, plus, tracks) = self.draw_settings_sound_menu();
        let mut items: Vec<(Rect, K)> = vec![(toggle, K::Toggle), (back, K::Back)];
        for (r, k, dl) in minus.iter().chain(plus.iter()) { items.push((*r, K::Adj(k, *dl))); }
        for (r, t) in &tracks { items.push((*r, K::Track(t.clone()))); }
        let rects: Vec<Rect> = items.iter().map(|i| i.0).collect();
        for ev in self.event_get() {
            let mut idx: Option<usize> = None;
            match ev {
                Event::Quit => self.quit(),
                Event::KeyDown { key, .. } => {
                    if key == K_m || self.is_back_key(key) { self.back_to_settings_from_sound(); }
                    else if let Some(dd) = self.nav_direction(key) {
                        let cur = self.nav_get_focus("settings_sound_menu", rects.len());
                        self.nav_set_focus("settings_sound_menu", nav_move(&rects, cur, dd));
                    } else if self.is_nav_confirm(key) { idx = self.nav_get_focus("settings_sound_menu", rects.len()); }
                }
                Event::MouseButtonDown { .. } => {
                    let mp = self.get_mouse_pos_logical();
                    idx = rects.iter().position(|r| r.collidepoint(mp));
                }
                _ => {}
            }
            let Some(i) = idx.filter(|i| *i < items.len()) else { continue; };
            match &items[i].1 {
                K::Toggle => { self.play_sound_v(Snd::Click, "click_volume"); self.toggle_sound_enabled(); }
                K::Back => { self.play_sound_v(Snd::ClickBack, "click_volume"); self.back_to_settings_from_sound(); }
                K::Adj(k, dl) => {
                    self.play_sound_v(Snd::Click, "click_volume");
                    match *k {
                        "menu_click_volume" => self.adjust_menu_click_volume(*dl),
                        "click_volume" => self.adjust_click_volume(*dl),
                        "click_back_volume" => self.adjust_click_back_volume(*dl),
                        "hit_volume" => self.adjust_hit_volume(*dl),
                        "gameover_volume" => self.adjust_gameover_volume(*dl),
                        "menu_music_volume" => self.adjust_menu_music_volume(*dl),
                        "ingame_music_volume" => self.adjust_ingame_music_volume(*dl),
                        _ => {}
                    }
                }
                K::Track(t) => {
                    let t = t.clone();
                    self.settings.insert("ingame_music_track".into(), json!(t));
                    self.save_section("settings", None);
                    self.preview_ingame_music_track(&t, 5000);
                }
            }
        }
    }

    fn _stats_events(&mut self) {
        for ev in self.event_get() {
            match ev {
                Event::Quit => self.quit(),
                Event::MouseButtonDown { button: 1, .. } => {
                    let mp = self.get_mouse_pos_logical();
                    for (r, idx) in self.stat_tab_rects.clone() {
                        if r.collidepoint(mp) {
                            match idx {
                                StatTab::Back => { self.play_sound(Snd::ClickBack); self.showing_stats = false; self.in_profile_hub = true; }
                                StatTab::Sub(s) => { self.play_sound(Snd::Click); self.modded_sub_tab = s; self.stat_scroll = 0; }
                                StatTab::Main(m) => { self.play_sound(Snd::Click); self.current_stat_tab = m; self.stat_scroll = 0; }
                            }
                        }
                    }
                }
                Event::MouseWheel { y, .. } => self.adjust_stat_scroll(-y * 30),
                Event::KeyDown { key, .. } => {
                    if key == K_TAB { self.switch_stat_tab(); }
                    else if key == K_m || self.is_back_key(key) { self.in_profile_hub = true; self.showing_stats = false; }
                    else {
                        let dd = self.nav_direction(key);
                        let tabs: Vec<(Rect, StatTab)> = self.stat_tab_rects.iter().filter(|(_, i)| !matches!(i, StatTab::Back)).cloned().collect();
                        let nr: Vec<Rect> = tabs.iter().map(|t| t.0).collect();
                        if dd == Some("left") || dd == Some("right") {
                            let cur = self.nav_get_focus("statistics", nr.len());
                            self.nav_set_focus("statistics", nav_move(&nr, cur, dd.unwrap()));
                        } else if dd == Some("up") { self.adjust_stat_scroll(-30); }
                        else if dd == Some("down") { self.adjust_stat_scroll(30); }
                        else if self.is_nav_confirm(key) {
                            if let Some(fi) = self.nav_get_focus("statistics", nr.len()).filter(|f| *f < tabs.len()) {
                                self.play_sound(Snd::Click);
                                match tabs[fi].1 {
                                    StatTab::Sub(s) => { self.modded_sub_tab = s; self.stat_scroll = 0; }
                                    StatTab::Main(m) => { self.current_stat_tab = m; self.stat_scroll = 0; }
                                    StatTab::Back => {}
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // ---------------- Versteckter Selbsttest (nur Debug-Build) ----------------
    /// JND_SCRIPT="40:shot:menu.png;45:key:down;50:cmd:run shop;60:click:400,300;90:quit"
    fn _test_init(&mut self) {
        if !cfg!(debug_assertions) { return; }
        let Ok(s) = std::env::var("JND_SCRIPT") else { return; };
        for part in s.split(';') {
            let mut it = part.splitn(3, ':');
            let (Some(f), Some(cmd)) = (it.next(), it.next()) else { continue; };
            let Ok(f) = f.trim().parse::<u64>() else { continue; };
            self._test_script.push((f, cmd.trim().to_string(), it.next().unwrap_or("").to_string()));
        }
        self._test_script.sort_by_key(|t| t.0);
    }

    pub fn _screenshot_hook(&mut self, shown: &Surface) {
        if self._test_script.is_empty() { return; }
        let fc = self._frame_counter;
        while let Some((f, cmd, arg)) = self._test_script.first().cloned() {
            if f > fc { break; }
            self._test_script.remove(0);
            match cmd.as_str() {
                "shot" => { shown.save_png(&arg); println!("[TEST] Screenshot {} (Frame {})", arg, fc); }
                "cmd" => { let unlocked = self._dev_console_unlocked; self._dev_console_unlocked = true; self._dispatch_dev_command(&arg); self._dev_console_unlocked = unlocked; }
                "key" => {
                    let k = test_key(&arg);
                    let uni = if arg.chars().count() == 1 { arg.clone() } else { String::new() };
                    self.display.event_post(Event::KeyDown { key: k, unicode: uni, mods: 0 });
                    self.display.event_post(Event::KeyUp { key: k, mods: 0 });
                }
                "pad" => {
                    // pad:<1|2>:<a|b|x|y|start|up|down|left|right|stick=-1..1>:<down|up>
                    let p: Vec<&str> = arg.split(':').collect();
                    let n = p.first().and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).max(1) - 1;
                    let b = p.get(1).copied().unwrap_or("a");
                    if let Some(v) = b.strip_prefix("stick=") { self.display.test_pad_stick(n, v.parse().unwrap_or(0.0)); }
                    else { self.display.test_pad_button(n, b, p.get(2).map(|d| *d != "up").unwrap_or(true)); }
                }
                "hold" => { let k = test_key(&arg); self.display.test_set_key(k, true); self.display.event_post(Event::KeyDown { key: k, unicode: String::new(), mods: 0 }); }
                "release" => { let k = test_key(&arg); self.display.test_set_key(k, false); self.display.event_post(Event::KeyUp { key: k, mods: 0 }); }
                "type" => { for ch in arg.chars() { self.display.event_post(Event::KeyDown { key: ch as i32, unicode: ch.to_string(), mods: 0 }); } }
                "click" => {
                    let p: Vec<i32> = arg.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                    if p.len() == 2 {
                        let s = self.scale_x.max(0.0001);
                        let (px, py) = ((p[0] as f64 * s + self.offset_x) as i32, (p[1] as f64 * self.scale_y + self.offset_y) as i32);
                        self.display.test_set_mouse(px, py);
                        self.display.event_post(Event::MouseButtonDown { button: 1, pos: (px, py) });
                        self.display.event_post(Event::MouseButtonUp { button: 1, pos: (px, py) });
                    }
                }
                "quit" => { println!("[TEST] Ende bei Frame {}", fc); crate::crypto::flush_saves(); std::process::exit(0); }
                _ => {}
            }
        }
    }
}

fn test_key(name: &str) -> i32 {
    match name {
        "return" | "enter" => K_RETURN, "escape" | "esc" => K_ESCAPE, "up" => K_UP, "down" => K_DOWN,
        "left" => K_LEFT, "right" => K_RIGHT, "space" => K_SPACE, "tab" => K_TAB, "backspace" => K_BACKSPACE,
        s if s.chars().count() == 1 => s.chars().next().unwrap() as i32,
        _ => 0,
    }
}
