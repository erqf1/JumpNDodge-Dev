//! Kern: Hintergrund, Anzeige/Skalierung, Fade + Flip, Zeit-Cache, Daily Reward,
//! Texte/Farben, UI-Helfer, Tastenbelegung und Menü-Navigation, Hindernis-Fabriken,
//! Kosmetik-Icons/Preise/Sale.

use super::state::{Snd, G};
use crate::data::d;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

pub const WHITE: (i32, i32, i32) = (255, 255, 255);
pub const BLACK: (i32, i32, i32) = (0, 0, 0);
pub const GRAY: (i32, i32, i32) = (220, 220, 220);
pub const DARK_GRAY: (i32, i32, i32) = (170, 170, 170);
pub const RED: (i32, i32, i32) = (255, 0, 0);
pub const BLUE: (i32, i32, i32) = (0, 0, 255);
pub const GREEN: (i32, i32, i32) = (0, 200, 0);
pub const GOLD: (i32, i32, i32) = (255, 215, 0);
pub const PURPLE: (i32, i32, i32) = (180, 80, 255);
pub const OVERLAY_BG: (i32, i32, i32) = (20, 20, 30);
pub const DARK_BG: (i32, i32, i32) = (30, 30, 40);
pub const DARK_PANEL: (i32, i32, i32) = (50, 50, 60);
pub const DARK_TEXT: (i32, i32, i32) = (220, 220, 220);
pub const DARK_BUTTON: (i32, i32, i32) = (70, 70, 80);
pub const DARK_BUTTON_HOVER: (i32, i32, i32) = (90, 90, 100);

pub const FADE_SPEED: i32 = 22;

pub const STANDARD_ACTIONS: [&str; 6] = ["jump", "left", "right", "power1", "power2", "pause"];
pub const TWO_P_ACTIONS: [&str; 6] = ["p1_jump", "p1_left", "p1_right", "p2_jump", "p2_left", "p2_right"];
pub const MENU_NAV_ACTIONS: [&str; 6] = ["nav_up", "nav_down", "nav_left", "nav_right", "nav_confirm", "nav_back"];
pub const NAV_UP: [i32; 2] = [K_UP, K_w];
pub const NAV_DOWN: [i32; 2] = [K_DOWN, K_s];
pub const NAV_LEFT: [i32; 2] = [K_LEFT, K_a];
pub const NAV_RIGHT: [i32; 2] = [K_RIGHT, K_d];
pub const NAV_CONFIRM_KEYS: [i32; 2] = [K_SPACE, K_RETURN];
pub const NAV_BACK_KEYS: [i32; 2] = [K_ESCAPE, K_CAPSLOCK];

pub fn default_kb() -> Vec<(&'static str, Vec<i32>)> {
    vec![
        ("jump", vec![K_SPACE, K_w, K_UP]),
        ("left", vec![K_LEFT, K_a]),
        ("right", vec![K_RIGHT, K_d]),
        ("power1", vec![K_q]),
        ("power2", vec![K_e]),
        ("pause", vec![K_ESCAPE]),
        ("p1_jump", vec![K_SPACE, K_w]),
        ("p1_left", vec![K_a]),
        ("p1_right", vec![K_d]),
        ("p2_jump", vec![K_UP]),
        ("p2_left", vec![K_LEFT]),
        ("p2_right", vec![K_RIGHT]),
    ]
}

pub fn menu_nav_default(action: &str) -> &'static [i32] {
    match action {
        "nav_up" => &NAV_UP,
        "nav_down" => &NAV_DOWN,
        "nav_left" => &NAV_LEFT,
        "nav_right" => &NAV_RIGHT,
        "nav_confirm" => &NAV_CONFIRM_KEYS,
        "nav_back" => &NAV_BACK_KEYS,
        _ => &[],
    }
}

pub const DIFFICULTY_TRANSLATIONS: [(&str, [&str; 8]); 5] = [
    ("Deutsch", ["Leicht", "Mittel", "Schwer", "Hardcore", "Unmöglich", "Albtraum", "Hölle", "Dynamisch"]),
    ("English", ["Easy", "Medium", "Hard", "Hardcore", "Impossible", "Nightmare", "Hell", "Dynamic"]),
    ("Русский", ["Легко", "Средне", "Сложно", "Хардкор", "Невозможно", "Кошмар", "Ад", "Динамично"]),
    ("Türkçe", ["Kolay", "Orta", "Zor", "Hardcore", "İmkansız", "Kabus", "Cehennem", "Dinamik"]),
    ("Schwiizerdütsch", ["Licht", "Mittel", "Schwer", "Hardcore", "Unmöglich", "Alptraum", "Höll", "Dynamisch"]),
];

/// Geschwindigkeit/Intervalle liegen in jnd_core (gleiche Werte für Spiel und Server)
pub use jnd_core::sim2p::{difficulty_obstacle_interval, difficulty_speed};

/// coin_rules (Python: Schwer 10, Hardcore 5, Unmöglich 3, Albtraum 2, Hölle 1.5)
pub fn coin_rule(d: &str) -> Option<f64> {
    match d { "Schwer" => Some(10.0), "Hardcore" => Some(5.0), "Unmöglich" => Some(3.0), "Albtraum" => Some(2.0), "Hölle" => Some(1.5), _ => None }
}
pub const MIN_SURVIVE_SECONDS_FOR_COINS: i64 = 15;

pub fn rarity_color(r: &str) -> (i32, i32, i32) {
    match r { "Common" => (0, 200, 0), "Rare" => (80, 150, 255), "Epic" => (180, 80, 255), "Legendary" => (255, 200, 50), "Mythic" => (255, 140, 0), _ => (0, 200, 0) }
}
pub const RARITY_PROBS: [(&str, i64); 5] = [("Common", 795), ("Rare", 100), ("Epic", 80), ("Legendary", 20), ("Mythic", 5)];

pub struct RarityCols { pub bg: (i32, i32, i32), pub border: (i32, i32, i32), pub badge: (i32, i32, i32), pub label: &'static str }
pub fn get_rarity_colors(r: &str) -> RarityCols {
    match r {
        "Rare" => RarityCols { bg: (40, 80, 180), border: (80, 140, 255), badge: (70, 130, 240), label: "Rare" },
        "Epic" => RarityCols { bg: (100, 40, 160), border: (180, 80, 255), badge: (160, 70, 240), label: "Epic" },
        "Legendary" => RarityCols { bg: (160, 110, 10), border: (255, 200, 0), badge: (220, 170, 20), label: "Legendary" },
        _ => RarityCols { bg: (60, 130, 60), border: (100, 220, 100), badge: (80, 200, 80), label: "Common" },
    }
}

pub fn cosmetic_category_to_slot(cat: &str) -> Option<&'static str> {
    match cat { "headwear" => Some("equipped_headwear"), "eyes" => Some("equipped_eyes"), "nose" => Some("equipped_nose"), "mouth" => Some("equipped_mouth"), _ => None }
}

/// Alle Trail-Keys in COSMETICS_DB-Reihenfolge
pub fn all_trail_keys() -> Vec<String> {
    d("COSMETICS_DB").as_object().unwrap().iter().filter(|(_, m)| m.gs("category", "") == "trail").map(|(k, _)| k.clone()).collect()
}

const SALE_ITEM_COUNT: usize = 4;
const SALE_DISCOUNTS: [i64; 5] = [20, 25, 30, 40, 50];
fn sale_rarity_weight(r: &str) -> f64 {
    match r { "Common" => 100.0, "Rare" => 45.0, "Epic" => 20.0, "Legendary" => 8.0, "Mythic" => 3.0, _ => 50.0 }
}

/// Obstacle: pygame.Rect mit Typ und Richtung (Free-Roam)
#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub r: Rect,
    pub obs_type: &'static str,
    pub dir_x: Option<f64>,
    pub dir_y: Option<f64>,
    /// Gleitkomma-Position für Free-Roam (pygame-Rects runden jede Bewegung ab)
    pub fx: f64,
    pub fy: f64,
}
impl Obstacle {
    pub fn new(x: i32, y: i32, w: i32, h: i32, t: &'static str) -> Obstacle { Obstacle { r: Rect::new(x, y, w, h), obs_type: t, dir_x: None, dir_y: None, fx: x as f64, fy: y as f64 } }
}

impl G {
    // ---------------- Hintergrund ----------------
    pub fn draw_bg_effects(&mut self, surface: &Surface, w: i32, h: i32, show_rings: bool) {
        let animated = self.settings.gb("bg_animated", true);
        let show_stars = self.settings.gb("bg_stars", true);
        let rings_on = self.settings.gb("bg_rings_enabled", true);
        let t = if animated { self.get_ticks() as f64 / 1000.0 } else { 0.0 };
        let dark = self.is_dark_mode();
        let mono = self.settings.gb("mono_bg", false);
        let area_ratio = (w * h) as f64 / (800.0 * 600.0);
        let max_stars = 8.max((55.0 * area_ratio) as usize);
        if show_rings && rings_on {
            let light = [(180,200,220),(150,180,210),(160,190,215),(170,205,225),(145,175,205),(165,195,218),(155,185,212),(175,200,222),(148,178,208),(162,192,216),(172,202,220),(158,188,213)];
            let darkc = [(100,150,200),(150,100,200),(200,150,100),(90,140,195),(140,95,195),(195,145,95),(105,155,205),(155,105,205),(205,155,105),(95,145,198),(148,98,198),(198,148,98)];
            let rcols = if !dark { light } else { darkc };
            let mono_rcol = if !dark { (40, 40, 40) } else { (210, 210, 210) };
            let ring_scale = w.min(h) as f64 / 600.0;
            let rings = d("_bg_rings").as_array().unwrap();
            for (i, rg) in rings.iter().enumerate() {
                let a = rg.as_array().unwrap();
                let (rx, ry, base_rad, lw, phase, damp) = (as_f(&a[0]).unwrap(), as_f(&a[1]).unwrap(), as_f(&a[2]).unwrap(), as_i(&a[3]).unwrap() as i32, as_f(&a[4]).unwrap(), as_f(&a[5]).unwrap());
                let pulse = (t * 1.1 + phase).sin() * 6.0;
                let drift_x = (t * 0.5 + phase).sin() * damp * 1.8;
                let drift_y = (t * 0.4 + phase).cos() * damp * 1.8;
                let cx = ((rx + drift_x) * w as f64) as i32;
                let cy = ((ry + drift_y) * h as f64) as i32;
                let rad = 4.max(((base_rad + pulse) * ring_scale) as i32);
                let col = if mono { mono_rcol } else { rcols[i % rcols.len()] };
                draw::circle(surface, col, (cx, cy), rad, lw);
                if !dark && !mono {
                    draw::circle(surface, (0.max(col.0 - 30), 0.max(col.1 - 30), 0.max(col.2 - 30)), (cx, cy), rad + 3, 1);
                }
            }
        }
        if !show_stars { return; }
        let star_cols: Vec<(i32, i32, i32)> = if mono {
            if !dark { vec![(40, 40, 40)] } else { vec![(210, 210, 210)] }
        } else if !dark {
            vec![(120, 160, 230), (220, 180, 60), (100, 200, 230), (230, 100, 140), (80, 180, 120)]
        } else {
            vec![(120, 160, 255), (255, 220, 80), (80, 200, 255), (255, 120, 180), (80, 220, 140)]
        };
        let stars = self._bg_stars.clone();
        for (i, s) in stars.iter().take(max_stars).enumerate() {
            let [sx, sy, base_r, phase, dph, dax, day] = *s;
            let beat = t * 2.2 + phase;
            let p1 = (beat.sin()).max(0.0).powi(2);
            let p2 = ((beat - 0.55).sin()).max(0.0).powi(2);
            let pulse = p1 * 0.65 + p2 * 0.35;
            let r = (1.0f64).max(base_r + pulse * 2.5);
            let drift_x = (t * 0.28 + dph).sin() * dax;
            let drift_y = (t * 0.22 + dph).cos() * day;
            let cx = ((sx + drift_x) * w as f64) as i32;
            let cy = ((sy + drift_y) * h as f64) as i32;
            let col = star_cols[i % star_cols.len()];
            let glow_r = (r + if !dark { 3.0 } else { 2.0 }) as i32;
            let add = if !dark { 50 } else { 40 };
            let glow_col = ((col.0 + add).min(255), (col.1 + add).min(255), (col.2 + add).min(255));
            draw::circle(surface, glow_col, (cx, cy), 2.max(glow_r), 1);
            draw::circle(surface, col, (cx, cy), 1.max(r as i32), 0);
        }
    }

    pub fn fill_bg(&mut self, rings: bool) {
        self.sync_window_size();
        let bs = self.base_surface.clone();
        bs.fill(self.get_bg_color());
        let (w, h) = (self.WIDTH, self.HEIGHT);
        self.draw_bg_effects(&bs, w, h, rings);
    }

    // ---------------- Anzeige ----------------
    pub fn toggle_fullscreen(&mut self) {
        let fs = !self.settings.gb("fullscreen", false);
        self.settings.insert("fullscreen".into(), json!(fs));
        self.save_section("settings", None);
        self.set_display_mode();
        self.play_sound(Snd::Click);
    }

    pub fn set_display_mode(&mut self) {
        if self.settings.gb("fullscreen", false) {
            let (sw, sh) = self.display.screen_size();
            self.display.set_mode(sw, sh, false, true);
            self.WIDTH = sw;
            self.HEIGHT = sh;
            self.base_surface = Surface::new(self.WIDTH, self.HEIGHT);
        } else {
            self.display.set_mode(self.WIDTH, self.HEIGHT, true, false);
            self.base_surface = Surface::new(self.WIDTH, self.HEIGHT);
            self._window_locked_for_gameplay = false;
        }
        self.scale_x = 1.0;
        self.scale_y = 1.0;
        self.offset_x = 0.0;
        self.offset_y = 0.0;
    }

    /// get_mouse_pos_logical()
    pub fn get_mouse_pos_logical(&mut self) -> (i32, i32) {
        let (mx, my) = self.display.mouse_pos();
        if let Some(last) = self._last_mouse_phys_pos {
            if last != (mx, my) { self._kb_nav_active = false; }
        }
        self._last_mouse_phys_pos = Some((mx, my));
        (((mx as f64 - self.offset_x) / self.scale_x) as i32, ((my as f64 - self.offset_y) / self.scale_y) as i32)
    }

    pub fn get_ticks(&self) -> i64 { self.display.get_ticks() }

    /// pygame.event.get() - plus Hintergrundarbeiten (Online-Antworten)
    pub fn event_get(&mut self) -> Vec<Event> {
        self.poll_online();
        let ev = self.display.event_get();
        self._handle_window_events(&ev);
        ev
    }

    fn _handle_window_events(&mut self, ev: &[Event]) {
        for e in ev {
            if let Event::Quit = e {
                // Python: pygame.quit(); sys.exit() an jeder Stelle - hier zentral
            }
        }
    }

    // ---------------- Admin-Keyfile ----------------
    pub fn _check_admin_keyfile(&mut self) -> bool {
        const ADMIN_KEY: &str = "K9mX2pR7qN4wL1jH8tY5bV3cF6dG0sA2eU9iZ4xK7mP1rQ8nW5lJ3hT6yB0vC4fD2gS9aE7uI1oZ5xM8kP3rN6qW2jL0hT9yB4vC7fD1gS5aE3u";
        let now = now_secs();
        if now - self._admin_keyfile_cache.1 < 2.0 { return self._admin_keyfile_cache.0; }
        let mut result = false;
        if let Ok(rd) = std::fs::read_dir(&self.BASE_DIR) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "txt").unwrap_or(false) {
                    if let Ok(c) = std::fs::read(&p) {
                        if String::from_utf8_lossy(&c).trim() == ADMIN_KEY { result = true; break; }
                    }
                }
            }
        }
        self._admin_keyfile_cache = (result, now);
        result
    }

    // ---------------- Zeit-Cache, Daily Reward ----------------
    pub fn _refresh_time_cache_if_needed(&mut self) {
        let now_local = now_secs();
        if let Some(c) = self._time_cache.checked_at {
            if now_local - c < 5.0 { return; }
        }
        // Netzwerkprüfung im Hintergrund, damit nichts einfriert: Ergebnis kommt beim nächsten Aufruf
        if !self._time_check_running {
            self._time_check_running = true;
            let tx = self._time_ch.tx.clone();
            std::thread::spawn(move || {
                let has_internet = check_internet_connection();
                // Python: get_current_datetime() fällt bei API-Fehler auf die lokale Zeit zurück
                let dt = if has_internet { Some(get_current_datetime_net().unwrap_or_else(|| chrono::Local::now().naive_local())) } else { None };
                let _ = tx.send((has_internet, dt));
            });
        }
        if let Ok((has, dt)) = self._time_ch.rx.try_recv() {
            self._time_check_running = false;
            let server_dt = match (has, dt) {
                (true, Some(d)) => d,
                _ => self._time_cache.server_datetime.unwrap_or_else(|| chrono::Local::now().naive_local()),
            };
            self._time_cache.server_datetime = Some(server_dt);
            self._time_cache.has_internet = has;
            self._time_cache.checked_at = Some(now_local);
        }
    }

    pub fn get_cached_now(&mut self) -> (chrono::NaiveDateTime, bool) {
        self._refresh_time_cache_if_needed();
        match (self._time_cache.server_datetime, self._time_cache.checked_at) {
            (Some(dt), Some(at)) => {
                let elapsed = now_secs() - at;
                (dt + chrono::Duration::milliseconds((elapsed * 1000.0) as i64), self._time_cache.has_internet)
            }
            _ => (chrono::Local::now().naive_local(), self._time_cache.has_internet),
        }
    }

    pub fn get_cached_current_date(&mut self) -> (String, bool) {
        let (now, has) = self.get_cached_now();
        (now.format("%Y-%m-%d").to_string(), has)
    }

    pub fn calculate_daily_reward(&self) -> (i64, Value) {
        let base_coins = randint(50, 100);
        let mut extra = 0;
        while extra < 150 && random() < 0.90 { extra += 1; }
        let total = base_coins + extra;
        let rarity = if extra >= 20 { "Epic" } else if extra > 0 { "Rare" } else { "Normal" };
        (total, json!({"base": base_coins, "extra": extra, "total": total, "rarity": rarity}))
    }

    pub fn can_claim_daily_reward(&mut self) -> (bool, String) {
        let (current_date, has) = self.get_cached_current_date();
        if !has { return (false, "Keine Internetverbindung".into()); }
        if self.stats.gs("last_daily_reward_date", "") == current_date { return (false, "Bereits heute erhalten".into()); }
        (true, current_date)
    }

    pub fn claim_daily_reward(&mut self) -> bool {
        let (can, message) = self.can_claim_daily_reward();
        if !can {
            self.push_overlay(&format!("Daily Reward: {}", message), (200, 100, 100), 2.0, None);
            return false;
        }
        let (coins, rs) = self.calculate_daily_reward();
        let c = self.stats.gi("coins", 0) + coins;
        self.stats.insert("coins".into(), json!(c));
        let t = self.stats.gi("total_coins_earned", 0) + coins;
        self.stats.insert("total_coins_earned".into(), json!(t));
        self.stats.insert("last_daily_reward_date".into(), json!(message));
        self.stats.insert("last_daily_reward_stats".into(), rs.clone());
        self.save_section("stats", None);
        let rarity = rs.gs("rarity", "Normal");
        let color = match rarity.as_str() { "Normal" => (100, 100, 100), "Rare" => (80, 150, 255), "Epic" => (180, 80, 255), _ => GOLD };
        self.push_overlay(&format!("Daily Reward: +{} Coins! [{}]", coins, rarity), color, 3.0, None);
        true
    }

    // ---------------- Fade / Sync / Flip ----------------
    pub fn request_fade(&mut self, cb: Box<dyn FnOnce(&mut G)>) {
        self._fade_alpha = 0;
        self._fade_dir = 1;
        self._fade_callback = Some(cb);
    }

    pub fn sync_window_size(&mut self) {
        self.poll_online();
        self._process_dev_commands();
        self._debug_track_screen_transitions();
        if let Some(n) = self._submit_pending_local_name.take() { self.rename_current_profile_local(&n); }
        self._apply_confirmed_online_deletions();
        let fullscreen = self.settings.gb("fullscreen", false);
        if !fullscreen && self.game_running != self._window_locked_for_gameplay {
            let (w, h) = (self.WIDTH, self.HEIGHT);
            let gr = self.game_running;
            // Fenstergröße bleibt, nur Resizebarkeit wird umgeschaltet
            let (cw, ch) = self.display.size();
            let _ = (w, h);
            self.display.set_mode(cw, ch, !gr, false);
            self._window_locked_for_gameplay = gr;
        }
        let (mut win_w, mut win_h) = self.display.size();
        if !fullscreen {
            win_w = win_w.max(400);
            win_h = win_h.max(300);
        }
        let gui_scale = if self.game_running { 1.0 } else { self.settings.gf("gui_scale", 1.0).clamp(0.5, 2.0) };
        let lw = 400.max((win_w as f64 / gui_scale) as i32);
        let lh = 300.max((win_h as f64 / gui_scale) as i32);
        // Scharfe Darstellung: in echter Pixelauflösung zeichnen statt (wie pygame) klein
        // zeichnen und dann unscharf hochziehen. Abschaltbar über settings["sharp_rendering"].
        let (pw, ph) = self.display.physical_size();
        let k = if self.settings.gb("sharp_rendering", true) {
            ((pw as f64 / lw as f64).min(ph as f64 / lh as f64)).clamp(0.25, 4.0) as f32
        } else { 1.0 };
        let k = (k * 1000.0).round() / 1000.0;
        crate::pg::surface::set_render_scale(k);
        if lw != self.WIDTH || lh != self.HEIGHT || (self.base_surface.scale_k() - k).abs() > 1e-4 {
            self.WIDTH = lw;
            self.HEIGHT = lh;
            self.base_surface = Surface::new(self.WIDTH, self.HEIGHT);
        }
        self.scale_x = win_w as f64 / self.WIDTH as f64;
        self.scale_y = win_h as f64 / self.HEIGHT as f64;
        self.offset_x = 0.0;
        self.offset_y = 0.0;
    }

    /// pygame.display.flip() (= _jnd_flip + _patched_flip)
    pub fn flip(&mut self) {
        self.poll_online();
        // base_surface hat bereits Fensterauflösung (siehe sync_window_size) - keine Neuskalierung
        let target = if self._fade_dir != 0 || self._fade_alpha > 0 { Some(self.base_surface.copy()) } else { None };
        if self._fade_dir != 0 || self._fade_alpha > 0 {
            self._fade_alpha += self._fade_dir * FADE_SPEED;
            if self._fade_dir == 1 && self._fade_alpha >= 255 {
                self._fade_alpha = 255;
                if let Some(cb) = self._fade_callback.take() { cb(self); }
                self._fade_dir = -1;
            } else if self._fade_dir == -1 && self._fade_alpha <= 0 {
                self._fade_alpha = 0;
                self._fade_dir = 0;
            }
        }
        let shown = target.unwrap_or_else(|| self.base_surface.clone());
        if self._fade_alpha > 0 {
            let (sw, sh) = shown.get_size();
            let fs = Surface::new_alpha(sw, sh);
            fs.fill((0, 0, 0, self._fade_alpha.clamp(0, 255)));
            shown.blit(&fs, (0, 0));
        }
        self.display.present(&shown);
        self._frame_counter += 1;
        self._screenshot_hook(&shown);
    }

    // ---------------- Texte & Farben ----------------
    pub fn lang(&self) -> String { self.languages[self.language_index].clone() }

    pub fn get_text(&self, key: &str) -> String {
        let lang = self.lang();
        let texts = &self.texts;
        let de = texts.get("Deutsch");
        let pack = texts.get(&lang).or(de);
        if let Some(Value::String(s)) = pack.and_then(|p| p.get(key)) { return s.clone(); }
        if let Some(Value::String(s)) = de.and_then(|p| p.get(key)) { return s.clone(); }
        key.to_string()
    }

    pub fn get_difficulty_name(&self, key: &str) -> String {
        let lang = self.lang();
        for (l, arr) in DIFFICULTY_TRANSLATIONS.iter() {
            if *l == lang {
                if let Some(i) = crate::game::save::DIFFICULTIES.iter().position(|x| *x == key) { return arr[i].to_string(); }
                return key.to_string();
            }
        }
        key.to_string()
    }

    pub fn get_difficulty_display_name(&self, difficulty: &str) -> String { self.get_text(&format!("diff_{}", difficulty)) }

    pub fn is_dark_mode(&self) -> bool { self.settings.gb("dark_mode", true) }
    pub fn get_bg_color(&self) -> (i32, i32, i32) { if self.is_dark_mode() { DARK_BG } else { (240, 245, 255) } }
    pub fn get_panel_color(&self) -> (i32, i32, i32) { if self.is_dark_mode() { DARK_PANEL } else { (255, 255, 255) } }
    pub fn get_text_color(&self) -> (i32, i32, i32) { if self.is_dark_mode() { DARK_TEXT } else { (30, 30, 60) } }
    pub fn get_button_color(&self) -> (i32, i32, i32) { if self.is_dark_mode() { DARK_BUTTON } else { (255, 255, 255) } }
    pub fn get_button_hover_color(&self) -> (i32, i32, i32) { if self.is_dark_mode() { DARK_BUTTON_HOVER } else { (200, 220, 255) } }

    pub fn toggle_dark_mode(&mut self) {
        let v = !self.settings.gb("dark_mode", false);
        self.settings.insert("dark_mode".into(), json!(v));
        self.save_section("settings", None);
        self.start_loading_screen(false);
        self.play_sound(Snd::Click);
    }

    pub fn get_power_desc(&self, power_name: &str, detailed: bool) -> String {
        let pd = d("POWERS_DB").gv(power_name).cloned().unwrap_or(json!({}));
        let key = if detailed { "detailed_desc" } else { "desc" };
        let desc = pd.gv(key).or_else(|| pd.gv("desc")).cloned().unwrap_or(json!({}));
        if let Value::String(s) = &desc { return s.clone(); }
        let lang = self.lang();
        desc.gos(&lang).or_else(|| desc.gos("Deutsch")).unwrap_or_else(|| "Keine Beschreibung".into())
    }

    pub fn get_power_name(&self, power_name: &str) -> String {
        let pd = d("POWERS_DB").gv(power_name).cloned().unwrap_or(json!({}));
        let nd = pd.gv("name").cloned().unwrap_or(json!({}));
        if let Value::String(s) = &nd { return s.clone(); }
        nd.gos(&self.lang()).or_else(|| nd.gos("English")).unwrap_or_else(|| power_name.to_string())
    }

    pub fn get_cosmetic_name(&self, key: &str) -> String {
        d("COSMETICS_DB").gv(key).and_then(|m| m.gv("name")).and_then(|n| n.gos(&self.lang())).unwrap_or_else(|| key.to_string())
    }

    // ---------------- UI-Helfer ----------------
    pub fn draw_card(&self, surface: &Surface, rect: Rect, radius: i32, shadow: bool, border_color: Option<(i32, i32, i32)>, border_width: i32, glow: bool, glow_color: Option<(i32, i32, i32)>) {
        let dark = self.is_dark_mode();
        if shadow {
            let sr = Rect::new(rect.x + 5, rect.y + 5, rect.w, rect.h);
            let c: Color = if dark { (0, 0, 0, 80).into() } else { (170, 170, 190).into() };
            draw::rect(surface, c, sr, 0, radius);
        }
        draw::rect(surface, self.get_panel_color(), rect, 0, radius);
        if glow {
            let gc = glow_color.unwrap_or((255, 195, 50));
            let pulse = ui_glow_pulse(gc, 5.0, 0.7, None);
            let outer = (gc.0 / 4, gc.1 / 4, gc.2 / 4);
            draw::rect(surface, outer, rect.inflate(6, 6), 2, radius + 3);
            draw::rect(surface, pulse, rect, border_width, radius);
        } else {
            let bc = border_color.unwrap_or(if dark { (70, 90, 110) } else { (60, 80, 140) });
            draw::rect(surface, bc, rect, border_width, radius);
        }
    }

    /// draw_ui_button(surface, rect, text, font=None, hovered=False, ...)
    pub fn draw_ui_button(&self, surface: &Surface, rect: Rect, text: &str, o: BtnOpts) {
        let font = o.font.clone().unwrap_or_else(|| self.menu_font.clone());
        let bg = if o.hovered { o.hover_color.unwrap_or(self.get_button_hover_color()) } else { o.base_color.unwrap_or(self.get_button_color()) };
        let radius = o.radius.unwrap_or(12);
        draw::rect(surface, bg, rect, 0, radius);
        let bc = if o.hovered { o.hover_border_color.unwrap_or((150, 180, 220)) } else { o.border_color.unwrap_or((180, 180, 180)) };
        let bw = if o.hovered { o.hover_border_width.unwrap_or(3) } else { o.border_width.unwrap_or(2) };
        draw::rect(surface, bc, rect, bw, radius);
        let tc = o.text_color.unwrap_or(self.get_text_color());
        let ts = font.render(text, true, tc);
        surface.blit(&ts, (rect.centerx() - ts.get_width() / 2, rect.centery() - ts.get_height() / 2));
    }

    // ---------------- Tastenbelegung ----------------
    pub fn get_action_label(&self, action: &str) -> String {
        let t = d("_ACTION_LABELS_TRANSLATIONS");
        let de = &t["Deutsch"];
        let pack = t.gv(&self.lang()).unwrap_or(de);
        pack.gos(action).or_else(|| de.gos(action)).unwrap_or_else(|| action.to_string())
    }

    pub fn get_keybindings(&self) -> Vec<(String, Vec<i32>)> {
        let stored = self.settings.gobj("keybindings");
        default_kb().into_iter().map(|(a, defs)| {
            let v = match stored.get(a) { Some(Value::Array(arr)) => arr.iter().filter_map(as_i).map(|x| x as i32).collect(), _ => defs };
            (a.to_string(), v)
        }).collect()
    }
    pub fn kb(&self, action: &str) -> Vec<i32> {
        self.get_keybindings().into_iter().find(|(a, _)| a == action).map(|(_, v)| v).unwrap_or_default()
    }
    pub fn save_keybindings(&mut self, kb: &[(String, Vec<i32>)]) {
        let mut m = serde_json::Map::new();
        for (a, v) in kb { m.insert(a.clone(), json!(v)); }
        self.settings.insert("keybindings".into(), Value::Object(m));
        self.save_section("settings", None);
    }

    pub fn any_action_pressed(&self, action: &str) -> bool {
        self.kb(action).iter().any(|k| self.display.key_pressed(*k))
    }
    pub fn action_matches_key(&self, key: i32, action: &str) -> bool { self.kb(action).contains(&key) }

    pub fn get_menu_nav_extra_keys(&mut self) -> serde_json::Map<String, Value> {
        let mut extras = self.settings.gobj("menu_nav_extra_keys");
        for a in MENU_NAV_ACTIONS { if !extras.contains_key(a) { extras.insert(a.into(), json!([])); } }
        self.settings.insert("menu_nav_extra_keys".into(), Value::Object(extras.clone()));
        extras
    }
    pub fn save_menu_nav_extra_keys(&mut self, extras: serde_json::Map<String, Value>) {
        self.settings.insert("menu_nav_extra_keys".into(), Value::Object(extras));
        self.save_section("settings", None);
    }
    fn extra_has(&mut self, action: &str, key: i32) -> bool {
        let e = self.get_menu_nav_extra_keys();
        ints(e.get(action).unwrap_or(&json!([]))).contains(&(key as i64))
    }

    pub fn nav_direction(&mut self, key: i32) -> Option<&'static str> {
        let d = if NAV_UP.contains(&key) || self.extra_has("nav_up", key) { Some("up") }
            else if NAV_DOWN.contains(&key) || self.extra_has("nav_down", key) { Some("down") }
            else if NAV_LEFT.contains(&key) || self.extra_has("nav_left", key) { Some("left") }
            else if NAV_RIGHT.contains(&key) || self.extra_has("nav_right", key) { Some("right") }
            else { None };
        if d.is_some() { self._kb_nav_active = true; }
        d
    }
    pub fn is_nav_confirm(&mut self, key: i32) -> bool { NAV_CONFIRM_KEYS.contains(&key) || self.extra_has("nav_confirm", key) }
    pub fn is_back_key(&mut self, key: i32) -> bool { self.is_back_key_ex(key, true) }
    pub fn is_back_key_ex(&mut self, key: i32, allow_backspace: bool) -> bool {
        if NAV_BACK_KEYS.contains(&key) || self.extra_has("nav_back", key) { return true; }
        allow_backspace && key == K_BACKSPACE
    }
    pub fn nav_get_focus(&self, screen_id: &str, count: usize) -> Option<usize> { self.nav_get_focus_d(screen_id, count, 0) }
    pub fn nav_get_focus_d(&self, screen_id: &str, count: usize, default_idx: usize) -> Option<usize> {
        if count == 0 { return None; }
        let idx = self._nav_focus.get(screen_id).copied().unwrap_or(default_idx as i64);
        if idx >= 0 && (idx as usize) < count { Some(idx as usize) } else { Some(default_idx) }
    }
    pub fn nav_set_focus(&mut self, screen_id: &str, idx: Option<usize>) {
        self._nav_focus.insert(screen_id.to_string(), idx.map(|i| i as i64).unwrap_or(-1));
    }
    pub fn nav_is_focused(&self, screen_id: &str, idx: usize, count: usize) -> bool { self.nav_is_focused_d(screen_id, idx, count, 0) }
    pub fn nav_is_focused_d(&self, screen_id: &str, idx: usize, count: usize, default_idx: usize) -> bool {
        if !self._kb_nav_active { return false; }
        self.nav_get_focus_d(screen_id, count, default_idx) == Some(idx)
    }

    // ---------------- Hindernisse ----------------
    pub fn pick_obstacle_type(&self, difficulty: &str, dynamic_speed: Option<f64>) -> String {
        if self.klassik_mode { return "normal".into(); }
        let weights: Vec<(String, f64)> = if difficulty == "Dynamisch" && dynamic_speed.is_some() {
            vec![("normal".into(), 100.0)]
        } else {
            let w = d("difficulty_obstacle_types").gobj(difficulty);
            if w.is_empty() { vec![("normal".into(), 100.0)] } else { w.iter().map(|(k, v)| (k.clone(), as_f(v).unwrap_or(0.0))).collect() }
        };
        let types: Vec<String> = weights.iter().map(|x| x.0.clone()).collect();
        let probs: Vec<f64> = weights.iter().map(|x| x.1).collect();
        weighted_choice(&types, &probs)
    }

    pub fn make_obstacle(&self, spawn_x: i32, obs_type: &str) -> Vec<Obstacle> {
        let h = self.HEIGHT;
        if obs_type == "double" {
            let (s1, s2) = (50, 50);
            let gap = randint(80, 120) as i32;
            vec![Obstacle::new(spawn_x, h - 50 - s1, s1, s1, "normal"), Obstacle::new(spawn_x + s1 + gap, h - 50 - s2, s2, s2, "normal")]
        } else {
            let s = 50;
            vec![Obstacle::new(spawn_x, h - 50 - s, s, s, "normal")]
        }
    }

    pub fn make_top_obstacle(&self) -> Obstacle {
        let rx = randint(60, (self.WIDTH - 110) as i64) as i32;
        Obstacle::new(rx, -55, 50, 50, "floating")
    }

    pub fn make_free_roam_obstacle(&self) -> Obstacle {
        let s = 50;
        let edge = choice(&["left", "right", "top", "bottom"]);
        let skew = uniform(-0.8, 0.8);
        let (x, y, mut dx, mut dy);
        if edge == "left" || edge == "right" {
            y = randint(0, (self.HEIGHT - s) as i64) as i32;
            x = if edge == "left" { -s } else { self.WIDTH };
            dx = if edge == "left" { 1.0 } else { -1.0 };
            dy = skew;
        } else {
            x = randint(0, (self.WIDTH - s) as i64) as i32;
            y = if edge == "top" { -s } else { self.HEIGHT };
            dy = if edge == "top" { 1.0 } else { -1.0 };
            dx = skew;
        }
        let mag = (dx * dx + dy * dy).sqrt();
        dx /= mag;
        dy /= mag;
        let mut o = Obstacle::new(x, y, s, s, "normal");
        o.dir_x = Some(dx);
        o.dir_y = Some(dy);
        o
    }

    // ---------------- Kosmetik: Preise, Sale, Kauf ----------------
    pub fn initialize_cosmetic_prices(&mut self) {
        let mut prices = self.cosmetics_state.gobj("prices");
        let mut changed = !self.cosmetics_state.contains_key("prices");
        for (key, meta) in d("COSMETICS_DB").as_object().unwrap() {
            if !prices.contains_key(key) {
                let pr = meta.garr("price_range");
                let (mn, mx) = if pr.len() == 2 { (as_i(&pr[0]).unwrap_or(100), as_i(&pr[1]).unwrap_or(300)) } else { (100, 300) };
                prices.insert(key.clone(), json!(randint(mn, mx)));
                changed = true;
            }
        }
        self.cosmetics_state.insert("prices".into(), Value::Object(prices));
        if changed { self.save_section("cosmetics", None); }
    }

    pub fn get_cosmetic_base_price(&self, key: &str) -> i64 {
        let prices = self.cosmetics_state.gobj("prices");
        if let Some(p) = prices.get(key).and_then(as_i) { return p; }
        let pr = d("COSMETICS_DB").gv(key).map(|m| m.garr("price_range")).unwrap_or_default();
        if pr.len() == 2 { (as_i(&pr[0]).unwrap_or(100) + as_i(&pr[1]).unwrap_or(300)) / 2 } else { 200 }
    }

    pub fn get_cosmetic_price(&self, key: &str) -> i64 {
        let base = self.get_cosmetic_base_price(key);
        let disc = self.get_sale_discount(key);
        if disc != 0 { return 1.max(py_round(base as f64 * (100 - disc) as f64 / 100.0)); }
        base
    }

    pub fn ensure_daily_sale(&mut self) {
        let (current_date, has) = self.get_cached_current_date();
        if !has { return; }
        if self.sale_state.gs("date", "") == current_date && self.sale_state.truthy("items") { return; }
        let weighted: Vec<(String, f64)> = d("COSMETICS_DB").as_object().unwrap().iter().map(|(k, m)| (k.clone(), sale_rarity_weight(&m.gs("rarity", "")))).collect();
        let count = SALE_ITEM_COUNT.min(weighted.len());
        let chosen = weighted_sample_without_replacement(weighted, count);
        self.sale_state.insert("date".into(), json!(current_date));
        let mut items = serde_json::Map::new();
        for k in chosen { items.insert(k, json!(choice(&SALE_DISCOUNTS))); }
        self.sale_state.insert("items".into(), Value::Object(items));
        self.save_section("sale", None);
    }

    pub fn get_sale_discount(&self, key: &str) -> i64 { self.sale_state.gobj("items").gi(key, 0) }
    pub fn get_sale_items(&self) -> Vec<String> { self.sale_state.gobj("items").keys().cloned().collect() }

    pub fn buy_cosmetic(&mut self, key: &str) -> bool {
        let owned = self.cosmetics_state.gstrs("owned");
        if owned.iter().any(|o| o == key) {
            let t = self.get_text("power_already_owned");
            let t = if t.is_empty() { "Bereits gekauft".to_string() } else { t };
            self.push_overlay(&t, (200, 120, 120), 1.5, None);
            return false;
        }
        let price = self.get_cosmetic_price(key);
        if self.stats.gi("coins", 0) < price {
            let t = self.get_text("insufficient_coins");
            self.push_overlay(&t, (200, 100, 100), 1.5, None);
            return false;
        }
        let c = self.stats.gi("coins", 0) - price;
        self.stats.insert("coins".into(), json!(c));
        let mut o = self.cosmetics_state.garr("owned");
        o.push(json!(key));
        self.cosmetics_state.insert("owned".into(), Value::Array(o));
        self.save_section("stats", None);
        self.save_section("cosmetics", None);
        let name = d("COSMETICS_DB").gv(key).and_then(|m| m.gv("name")).and_then(|n| n.gos(&self.lang())).unwrap_or_else(|| key.to_string());
        let t = py_format(&self.get_text("item_acquired"), &[&name]);
        self.push_overlay(&t, (255, 215, 0), 1.5, None);
        true
    }

    pub fn get_max_slots(&self) -> usize {
        if self.powers_state.gstrs("owned").iter().any(|p| p == "Extra Slot") { 2 } else { 1 }
    }

    // ---------------- Kosmetik-Icons ----------------
    pub fn draw_cosmetic_icon(&self, surface: &Surface, key: &str, icx: i32, icy: i32, big: bool) {
        draw_cosmetic_icon(surface, key, icx, icy, big);
    }
}

/// Optionen für draw_ui_button (Python-Keyword-Argumente)
#[derive(Clone, Default)]
pub struct BtnOpts {
    pub font: Option<Font>,
    pub hovered: bool,
    pub base_color: Option<(i32, i32, i32)>,
    pub hover_color: Option<(i32, i32, i32)>,
    pub text_color: Option<(i32, i32, i32)>,
    pub border_color: Option<(i32, i32, i32)>,
    pub hover_border_color: Option<(i32, i32, i32)>,
    pub radius: Option<i32>,
    pub border_width: Option<i32>,
    pub hover_border_width: Option<i32>,
}

pub fn now_secs() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

pub fn ui_glow_pulse(c: (i32, i32, i32), speed: f64, low: f64, t: Option<f64>) -> (i32, i32, i32) {
    let t = t.unwrap_or_else(now_secs);
    let k = low + (1.0 - low) * (0.5 + 0.5 * (t * speed).sin());
    (((c.0 as f64 * k) as i32).min(255), ((c.1 as f64 * k) as i32).min(255), ((c.2 as f64 * k) as i32).min(255))
}

pub fn nav_move(rects: &[Rect], current: Option<usize>, direction: &str) -> Option<usize> { nav_move_w(rects, current, direction, 2.5) }
pub fn nav_move_w(rects: &[Rect], current: Option<usize>, direction: &str, lateral_weight: f64) -> Option<usize> {
    if rects.is_empty() { return current; }
    let cur = match current { Some(c) if c < rects.len() => c, _ => return Some(0) };
    let (cx, cy) = rects[cur].center();
    let mut best: Option<(usize, f64)> = None;
    for (i, r) in rects.iter().enumerate() {
        if i == cur { continue; }
        let (dx, dy) = ((r.centerx() - cx) as f64, (r.centery() - cy) as f64);
        let (primary, lateral) = match direction {
            "right" if dx > 0.0 => (dx, dy),
            "left" if dx < 0.0 => (-dx, dy),
            "down" if dy > 0.0 => (dy, dx),
            "up" if dy < 0.0 => (-dy, dx),
            _ => continue,
        };
        let score = primary + lateral_weight * lateral.abs();
        if best.map(|b| score < b.1).unwrap_or(true) { best = Some((i, score)); }
    }
    Some(best.map(|b| b.0).unwrap_or(cur))
}

pub fn nav_reveal_delta(rect: Rect, fade_top: i32, fade_bottom: i32) -> i32 {
    if rect.top() < fade_top { return rect.top() - fade_top; }
    if rect.bottom() > fade_bottom { return rect.bottom() - fade_bottom; }
    0
}

pub fn key_display_name(k: i32) -> String {
    match k {
        K_SPACE => "Space".into(), K_RETURN => "Enter".into(), K_LEFT => "Links".into(), K_RIGHT => "Rechts".into(),
        K_UP => "Hoch".into(), K_DOWN => "Runter".into(), K_LSHIFT => "LShift".into(), K_RSHIFT => "RShift".into(),
        K_LCTRL => "LCtrl".into(), K_RCTRL => "RCtrl".into(), K_LALT => "LAlt".into(), K_RALT => "RAlt".into(),
        K_BACKSPACE => "Backspace".into(), K_TAB => "Tab".into(), K_ESCAPE => "Escape".into(),
        _ => key_name(k).to_uppercase(),
    }
}

fn weighted_sample_without_replacement(items: Vec<(String, f64)>, k: usize) -> Vec<String> {
    let mut pool = items;
    let mut chosen = vec![];
    for _ in 0..k.min(pool.len()) {
        let total: f64 = pool.iter().map(|x| x.1).sum();
        if total <= 0.0 { break; }
        let roll = uniform(0.0, total);
        let mut running = 0.0;
        for i in 0..pool.len() {
            running += pool[i].1;
            if running >= roll {
                chosen.push(pool[i].0.clone());
                pool.remove(i);
                break;
            }
        }
    }
    chosen
}

pub fn check_internet_connection() -> bool {
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(5)).build();
    match agent.get("http://www.google.com").call() {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true,
        Err(_) => false,
    }
}

/// get_current_datetime() übers Netz (WorldTimeAPI Europe/Berlin); None bei Fehler
pub fn get_current_datetime_net() -> Option<chrono::NaiveDateTime> {
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(5)).build();
    let r = agent.get("http://worldtimeapi.org/api/timezone/Europe/Berlin").call().ok()?;
    let v: Value = serde_json::from_str(&r.into_string().ok()?).ok()?;
    let s = v.gs("datetime", "");
    chrono::DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.naive_local())
        .or_else(|| chrono::NaiveDateTime::parse_from_str(&s[..19.min(s.len())], "%Y-%m-%dT%H:%M:%S").ok())
}

/// str.format() mit {} / {0} Platzhaltern
pub fn py_format(fmt: &str, args: &[&str]) -> String {
    let mut out = String::new();
    let mut auto = 0usize;
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') { chars.next(); out.push('{'); continue; }
            let mut inner = String::new();
            while let Some(&n) = chars.peek() { chars.next(); if n == '}' { break; } inner.push(n); }
            let name = inner.split(':').next().unwrap_or("").to_string();
            let idx = if name.is_empty() { let i = auto; auto += 1; i } else { name.parse().unwrap_or(0) };
            out.push_str(args.get(idx).copied().unwrap_or(""));
        } else if c == '}' {
            if chars.peek() == Some(&'}') { chars.next(); }
            out.push('}');
        } else {
            out.push(c);
        }
    }
    out
}

/// draw_cosmetic_icon(surface, key, icx, icy, big)
pub fn draw_cosmetic_icon(surface: &Surface, key: &str, icx: i32, icy: i32, big: bool) {
    let (shape, ic) = match d("ITEM_ICONS").gv(key).and_then(|v| v.as_array()) {
        Some(a) => {
            let c = a[1].as_array().unwrap();
            (a[0].as_str().unwrap().to_string(), (as_i(&c[0]).unwrap() as i32, as_i(&c[1]).unwrap() as i32, as_i(&c[2]).unwrap() as i32))
        }
        None => ("circle".to_string(), (200, 200, 200)),
    };
    let b = |big_v: i32, small_v: i32| if big { big_v } else { small_v };
    let (x, y) = (icx, icy);
    let poly = |pts: &[(i32, i32)], c: (i32, i32, i32)| draw::polygon(surface, c, pts, 0);
    let rr = |c: (i32, i32, i32), r: (i32, i32, i32, i32), w: i32, rad: i32| { draw::rect(surface, c, rect(r.0, r.1, r.2, r.3), w, rad); };
    match shape.as_str() {
        "circle" => { draw::circle(surface, ic, (x, y), b(16, 9), 0); }
        "triangle" => poly(&[(x, y - b(18, 10)), (x - b(16, 9), y + b(13, 7)), (x + b(16, 9), y + b(13, 7))], ic),
        "diamond" => { let dd = b(18, 10); poly(&[(x, y - dd), (x + dd, y), (x, y + dd), (x - dd, y)], ic) }
        "star4" => {
            let (o, i2) = if big { (20, 7) } else { (11, 4) };
            poly(&[(x, y - o), (x + i2, y - i2), (x + o, y), (x + i2, y + i2), (x, y + o), (x - i2, y + i2), (x - o, y), (x - i2, y - i2)], ic)
        }
        "hexagon" => {
            let r = b(18, 10) as f64;
            let pts: Vec<(i32, i32)> = (0..6).map(|n| { let a = ((60 * n - 30) as f64).to_radians(); (x + (r * a.cos()) as i32, y + (r * a.sin()) as i32) }).collect();
            poly(&pts, ic)
        }
        "crown" => {
            if big {
                rr(ic, (x - 16, y + 4, 32, 12), 0, 0);
                poly(&[(x - 16, y + 4), (x - 16, y - 14), (x - 6, y - 4)], ic);
                poly(&[(x, y + 4), (x, y - 18), (x + 8, y - 4)], ic);
                poly(&[(x + 16, y + 4), (x + 16, y - 14), (x + 6, y - 4)], ic);
            } else {
                rr(ic, (x - 9, y + 2, 18, 7), 0, 0);
                poly(&[(x - 9, y + 2), (x - 9, y - 8), (x - 4, y - 2)], ic);
                poly(&[(x, y + 2), (x, y - 10), (x + 5, y - 2)], ic);
                poly(&[(x + 9, y + 2), (x + 9, y - 8), (x + 4, y - 2)], ic);
            }
        }
        "bolt" => {
            if big { poly(&[(x + 5, y - 19), (x - 9, y + 2), (x - 2, y + 2), (x - 7, y + 19), (x + 10, y - 3), (x + 2, y - 3)], ic) }
            else { poly(&[(x + 3, y - 11), (x - 5, y + 1), (x - 1, y + 1), (x - 4, y + 11), (x + 6, y - 2), (x + 1, y - 2)], ic) }
        }
        "eyes" => {
            if big {
                rr((255, 255, 255), (x - 14, y - 7, 10, 10), 0, 0); rr((255, 255, 255), (x + 4, y - 7, 10, 10), 0, 0);
                rr(ic, (x - 11, y - 4, 4, 4), 0, 0); rr(ic, (x + 7, y - 4, 4, 4), 0, 0);
            } else {
                rr((255, 255, 255), (x - 8, y - 4, 6, 6), 0, 0); rr((255, 255, 255), (x + 2, y - 4, 6, 6), 0, 0);
                rr(ic, (x - 6, y - 2, 2, 2), 0, 0); rr(ic, (x + 4, y - 2, 2, 2), 0, 0);
            }
        }
        "tophat" => {
            if big { rr(ic, (x - 16, y + 7, 32, 7), 0, 0); rr(ic, (x - 11, y - 17, 22, 26), 0, 0); rr((150, 20, 20), (x - 11, y + 2, 22, 5), 0, 0); }
            else { rr(ic, (x - 9, y + 4, 18, 4), 0, 0); rr(ic, (x - 6, y - 10, 12, 15), 0, 0); rr((150, 20, 20), (x - 6, y + 1, 12, 3), 0, 0); }
        }
        "horns" => {
            if big { poly(&[(x - 4, y + 14), (x - 19, y - 10), (x - 7, y + 5)], ic); poly(&[(x + 4, y + 14), (x + 19, y - 10), (x + 7, y + 5)], ic); }
            else { poly(&[(x - 2, y + 8), (x - 11, y - 6), (x - 4, y + 3)], ic); poly(&[(x + 2, y + 8), (x + 11, y - 6), (x + 4, y + 3)], ic); }
        }
        "nose" => {
            let r = b(14, 8);
            draw::circle(surface, ic, (x, y), r, 0);
            draw::circle(surface, (0.max(ic.0 - 60), 0.max(ic.1 - 60), 0.max(ic.2 - 60)), (x, y), r, 2);
        }
        "grin" => {
            let px = b(3, 1);
            let pat = ["1..........1", "1..........1", "1..........1", "1..........1", "1..........1", "111111111111"];
            for (pr, prow) in pat.iter().enumerate() {
                for (pc, ch) in prow.chars().enumerate() {
                    if ch == '1' { rr((10, 10, 15), (x - fdiv(13 * px, 2) + pc as i32 * px, y - 3 * px + pr as i32 * px, px, px), 0, 0); }
                }
            }
        }
        "rainbow" => {
            let cols = [(230, 60, 60), (240, 160, 40), (230, 220, 60), (90, 200, 90), (70, 140, 230)];
            for (ri, c2) in cols.iter().enumerate() {
                let ri = ri as i32;
                if big { draw::arc(surface, *c2, rect(x - 20, y - 10 + ri * 2, 40, 40 - ri * 4), 3.14159, 2.0 * 3.14159, 3); }
                else { draw::arc(surface, *c2, rect(x - 12, y - 6 + ri, 24, 24 - ri * 2), 3.14159, 2.0 * 3.14159, 2); }
            }
        }
        "cloud" => {
            let (r1, r2, dy) = if big { (10, 12, 4) } else { (6, 7, 2) };
            draw::circle(surface, ic, (x - b(10, 6), y + dy), r1, 0);
            draw::circle(surface, ic, (x + b(10, 6), y + dy), r1, 0);
            draw::circle(surface, ic, (x, y - b(5, 3)), r2, 0);
        }
        "drop" => {
            if big { poly(&[(x, y - 18), (x - 14, y + 6), (x + 14, y + 6)], ic); draw::circle(surface, ic, (x, y + 8), 12, 0); }
            else { poly(&[(x, y - 10), (x - 8, y + 4), (x + 8, y + 4)], ic); draw::circle(surface, ic, (x, y + 5), 7, 0); }
        }
        "heart" => {
            if big {
                draw::circle(surface, ic, (x - 9, y - 5), 11, 0); draw::circle(surface, ic, (x + 9, y - 5), 11, 0);
                poly(&[(x - 18, y - 2), (x + 18, y - 2), (x, y + 18)], ic);
            } else {
                draw::circle(surface, ic, (x - 5, y - 3), 6, 0); draw::circle(surface, ic, (x + 5, y - 3), 6, 0);
                poly(&[(x - 10, y - 1), (x + 10, y - 1), (x, y + 10)], ic);
            }
        }
        "halo" => {
            if big { draw::ellipse(surface, ic, rect(x - 15, y - 10, 30, 20), 4); } else { draw::ellipse(surface, ic, rect(x - 9, y - 6, 18, 12), 3); }
        }
        "propeller" => {
            if big {
                poly(&[(x - 16, y + 14), (x + 16, y + 14), (x + 10, y - 2), (x - 10, y - 2)], ic);
                draw::line(surface, (60, 60, 65), (x - 18, y - 12), (x + 18, y - 12), 3);
                draw::line(surface, (60, 60, 65), (x, y - 24), (x, y - 2), 3);
                draw::circle(surface, (255, 210, 40), (x, y - 12), 4, 0);
            } else {
                poly(&[(x - 9, y + 8), (x + 9, y + 8), (x + 6, y - 2), (x - 6, y - 2)], ic);
                draw::line(surface, (60, 60, 65), (x - 10, y - 7), (x + 10, y - 7), 2);
                draw::line(surface, (60, 60, 65), (x, y - 13), (x, y - 2), 2);
                draw::circle(surface, (255, 210, 40), (x, y - 7), 2, 0);
            }
        }
        "headband" => {
            if big {
                rr(ic, (x - 18, y - 5, 36, 11), 0, 0);
                poly(&[(x + 14, y + 6), (x + 28, y + 18), (x + 16, y + 18)], (190, 20, 20));
                draw::circle(surface, (190, 20, 20), (x, y), 3, 0);
            } else {
                rr(ic, (x - 11, y - 3, 22, 7), 0, 0);
                poly(&[(x + 8, y + 4), (x + 16, y + 11), (x + 10, y + 11)], (190, 20, 20));
                draw::circle(surface, (190, 20, 20), (x, y), 2, 0);
            }
        }
        "helmet" => {
            if big {
                draw::circle(surface, ic, (x, y), 19, 0);
                draw::ellipse(surface, (140, 210, 255), rect(x - 11, y - 10, 22, 18), 0);
                draw::ellipse(surface, (255, 255, 255), rect(x - 11, y - 10, 22, 18), 2);
            } else {
                draw::circle(surface, ic, (x, y), 11, 0);
                draw::ellipse(surface, (140, 210, 255), rect(x - 6, y - 6, 13, 11), 0);
                draw::ellipse(surface, (255, 255, 255), rect(x - 6, y - 6, 13, 11), 1);
            }
        }
        "sunglasses" => {
            if big {
                draw::circle(surface, ic, (x - 10, y), 11, 0); draw::circle(surface, ic, (x + 10, y), 11, 0);
                draw::circle(surface, (30, 30, 35), (x - 10, y), 8, 0); draw::circle(surface, (30, 30, 35), (x + 10, y), 8, 0);
                draw::line(surface, (140, 170, 200), (x - 14, y - 4), (x - 7, y + 3), 2);
                draw::line(surface, (140, 170, 200), (x + 6, y - 4), (x + 13, y + 3), 2);
                rr(ic, (x - 3, y - 2, 6, 4), 0, 0);
                draw::line(surface, ic, (x - 20, y - 2), (x - 21, y - 6), 2);
                draw::line(surface, ic, (x + 20, y - 2), (x + 21, y - 6), 2);
            } else {
                draw::circle(surface, ic, (x - 6, y), 7, 0); draw::circle(surface, ic, (x + 6, y), 7, 0);
                draw::circle(surface, (30, 30, 35), (x - 6, y), 5, 0); draw::circle(surface, (30, 30, 35), (x + 6, y), 5, 0);
                draw::line(surface, (140, 170, 200), (x - 9, y - 2), (x - 5, y + 2), 1);
                draw::line(surface, (140, 170, 200), (x + 3, y - 2), (x + 7, y + 2), 1);
                rr(ic, (x - 2, y - 1, 4, 2), 0, 0);
            }
        }
        "3d_glasses" => {
            if big { rr((200, 30, 30), (x - 17, y - 8, 14, 12), 0, 0); rr((40, 120, 220), (x + 3, y - 8, 14, 12), 0, 0); rr((20, 20, 25), (x - 4, y - 3, 8, 3), 0, 0); }
            else { rr((200, 30, 30), (x - 10, y - 5, 8, 7), 0, 0); rr((40, 120, 220), (x + 2, y - 5, 8, 7), 0, 0); rr((20, 20, 25), (x - 2, y - 2, 4, 2), 0, 0); }
        }
        "robot_visor" => {
            if big { rr((30, 30, 35), (x - 18, y - 8, 36, 16), 0, 6); rr(ic, (x - 14, y - 4, 28, 8), 0, 3); }
            else { rr((30, 30, 35), (x - 10, y - 5, 20, 10), 0, 4); rr(ic, (x - 7, y - 2, 14, 5), 0, 2); }
        }
        "pig_nose" => {
            if big {
                draw::ellipse(surface, ic, rect(x - 13, y - 9, 26, 18), 0);
                draw::ellipse(surface, (190, 100, 120), rect(x - 13, y - 9, 26, 18), 2);
                draw::ellipse(surface, (190, 100, 120), rect(x - 7, y - 3, 5, 7), 0);
                draw::ellipse(surface, (190, 100, 120), rect(x + 2, y - 3, 5, 7), 0);
            } else {
                draw::ellipse(surface, ic, rect(x - 8, y - 5, 16, 11), 0);
                draw::ellipse(surface, (190, 100, 120), rect(x - 4, y - 2, 3, 4), 0);
                draw::ellipse(surface, (190, 100, 120), rect(x + 1, y - 2, 3, 4), 0);
            }
        }
        "bird_beak" => {
            if big { poly(&[(x - 14, y - 5), (x + 15, y), (x - 11, y + 4)], ic); poly(&[(x - 14, y + 2), (x + 10, y + 5), (x - 11, y + 10)], (200, 140, 15)); }
            else { poly(&[(x - 8, y - 3), (x + 9, y), (x - 6, y + 2)], ic); poly(&[(x - 8, y + 1), (x + 6, y + 3), (x - 6, y + 6)], (200, 140, 15)); }
        }
        "vampire_fangs" => {
            if big { poly(&[(x - 9, y - 6), (x - 3, y - 6), (x - 6, y + 8)], ic); poly(&[(x + 3, y - 6), (x + 9, y - 6), (x + 6, y + 8)], ic); }
            else { poly(&[(x - 5, y - 4), (x - 1, y - 4), (x - 3, y + 5)], ic); poly(&[(x + 1, y - 4), (x + 5, y - 4), (x + 3, y + 5)], ic); }
        }
        "nerd_teeth" => {
            if big {
                draw::line(surface, (10, 10, 15), (x - 10, y - 6), (x + 10, y - 6), 2);
                rr(ic, (x - 9, y - 6, 8, 12), 0, 2); rr(ic, (x + 1, y - 6, 8, 12), 0, 2);
                rr((200, 200, 195), (x - 9, y - 6, 8, 12), 1, 2); rr((200, 200, 195), (x + 1, y - 6, 8, 12), 1, 2);
            } else {
                draw::line(surface, (10, 10, 15), (x - 6, y - 4), (x + 6, y - 4), 1);
                rr(ic, (x - 5, y - 4, 5, 7), 0, 1); rr(ic, (x + 1, y - 4, 5, 7), 0, 1);
            }
        }
        _ => {}
    }
}
