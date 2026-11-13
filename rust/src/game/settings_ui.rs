//! Einstellungen (ab 5.3 aufgeräumt): Startseite mit Kategorien, jede Kategorie als Liste aus Zeilen
//! "Name ......... Wert". Klick oder Enter ändert den Wert, ←/→ auch (z.B. GUI-Größe).
//! Ton und Steuerung bleiben eigene Bildschirme.

use super::core::*;
use super::state::{Snd, G};
use crate::pg::*;
use crate::util::*;
use serde_json::json;

/// Seiten: 0 = Darstellung, 1 = Spiel, 2 = System
pub const PAGE_DISPLAY: usize = 0;
pub const PAGE_GAME: usize = 1;
pub const PAGE_SYSTEM: usize = 2;

const GUI_STEPS: [f64; 7] = [0.75, 0.85, 0.9, 1.0, 1.1, 1.25, 1.5];
const FPS_STEPS: [i64; 7] = [0, 60, 120, 144, 165, 240, 1000];

/// Eine Zeile: Schlüssel, Beschriftung, Wert (leer = Knopf ohne Wert)
pub struct Row { pub key: &'static str, pub label: String, pub value: String, pub on: Option<bool> }

impl G {
    fn onoff(&self, b: bool) -> String { if b { self.tde("An", "On") } else { self.tde("Aus", "Off") } }

    fn settings_main_items(&self) -> Vec<(String, &'static str)> {
        vec![
            (self.tde("Darstellung", "Display"), "display"),
            (self.tde("Spiel", "Game"), "game"),
            (self.get_text("sound"), "sound"),
            (self.get_text("controls_menu_btn"), "controls"),
            ("System".to_string(), "system"),
            (self.get_text("back"), "back"),
        ]
    }

    fn settings_main_rects(&self) -> Vec<Rect> {
        (0..6).map(|i| Rect::new(self.WIDTH / 2 - 180, 130 + i * 62, 360, 50)).collect()
    }

    pub fn draw_settings_menu(&mut self) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        self.draw_screen_title(&bs, &self.get_text("settings"), 60);
        self.draw_coin_display(&bs);
        let items = self.settings_main_items();
        let rects = self.settings_main_rects();
        let accents = [(150, 130, 230), (90, 190, 140), (100, 150, 220), (80, 180, 200), (200, 150, 90), (150, 150, 160)];
        for (i, ((t, _), r)) in items.iter().zip(rects.iter()).enumerate() {
            let hv = r.collidepoint(mp) || self.nav_is_focused("settings_menu", i, items.len());
            self.draw_ui_button(&bs, *r, t, BtnOpts { font: Some(self.menu_font.clone()), hovered: hv, border_color: Some(accents[i]), hover_border_color: Some(accents[i]), ..Default::default() });
        }
        self.cleanup_overlays();
        for o in self.overlays.clone() {
            let t = self.small_font.render(&o.text, true, o.color);
            bs.blit(&t, ((self.WIDTH - t.get_width()) / 2, 100));
        }
        self.draw_achievement_popups(&bs);
        self.flip();
    }

    pub fn _settings_events(&mut self) {
        let rects = self.settings_main_rects();
        let items: Vec<&'static str> = self.settings_main_items().into_iter().map(|(_, k)| k).collect();
        for ev in self.event_get() {
            let mut act: Option<&str> = None;
            match ev {
                Event::Quit => self.quit(),
                Event::KeyDown { key, .. } => {
                    if key == K_m || self.is_back_key(key) { act = Some("back"); }
                    else if let Some(dd) = self.nav_direction(key) {
                        let cur = self.nav_get_focus("settings_menu", rects.len());
                        self.nav_set_focus("settings_menu", nav_move(&rects, cur, dd));
                    } else if self.is_nav_confirm(key) {
                        if let Some(i) = self.nav_get_focus("settings_menu", rects.len()) { act = Some(items[i]); }
                    }
                }
                Event::MouseButtonDown { button: 1, .. } => {
                    let m = self.get_mouse_pos_logical();
                    if let Some(i) = rects.iter().position(|r| r.collidepoint(m)) { act = Some(items[i]); }
                }
                _ => {}
            }
            match act {
                Some("display") => self.open_settings_page(PAGE_DISPLAY),
                Some("game") => self.open_settings_page(PAGE_GAME),
                Some("system") => self.open_settings_page(PAGE_SYSTEM),
                Some("sound") => self.open_settings_sound_menu(),
                Some("controls") => { self.play_sound(Snd::MenuClick); self.in_settings_menu = false; self.in_controls_menu = true; }
                Some("back") => self.back_to_main_from_settings(),
                _ => {}
            }
        }
    }

    pub fn open_settings_page(&mut self, page: usize) {
        self.play_sound(Snd::MenuClick);
        self.settings_page = page;
        self.in_settings_menu = false;
        self.in_settings_more_menu = true;
    }

    pub fn settings_rows(&mut self) -> Vec<Row> {
        let hz = self.target_fps();
        let s = &self.settings;
        let r = |key, label: String, value: String, on: Option<bool>| Row { key, label, value, on };
        match self.settings_page {
            PAGE_DISPLAY => {
                let gs = s.gf("gui_scale", 1.0);
                let lim = s.gi("fps_limit", 0);
                let fps = if lim == 0 { format!("Monitor ({} Hz)", hz) } else if lim >= 1000 { self.tde("Unbegrenzt", "Unlimited") } else { format!("{} FPS", lim) };
                let dark = self.is_dark_mode();
                vec![
                    r("theme", self.tde("Farbschema", "Colour scheme"), if dark { self.tde("Dunkel", "Dark") } else { self.tde("Hell", "Light") }, None),
                    r("legacy_art", self.tde("Legacy-Grafik (Python-Look)", "Legacy graphics (Python look)"), self.onoff(s.gb("legacy_art", false)), Some(s.gb("legacy_art", false))),
                    r("gui_scale", self.tde("GUI-Größe", "GUI size"), format!("{}%", (gs * 100.0).round() as i64), None),
                    r("fullscreen", self.tde("Vollbild (F11)", "Fullscreen (F11)"), self.onoff(s.gb("fullscreen", false)), Some(s.gb("fullscreen", false))),
                    r("fps", self.tde("Bildrate", "Frame rate"), fps, None),
                    r("effects", self.tde("Effekte (Partikel, Wackeln)", "Effects (particles, shake)"), self.onoff(s.gb("effects", true)), Some(s.gb("effects", true))),
                    r("bg_stars", self.tde("Hintergrund: Sterne", "Background: stars"), self.onoff(s.gb("bg_stars", true)), Some(s.gb("bg_stars", true))),
                    r("bg_rings_enabled", self.tde("Hintergrund: Ringe", "Background: rings"), self.onoff(s.gb("bg_rings_enabled", true)), Some(s.gb("bg_rings_enabled", true))),
                    r("bg_animated", self.tde("Hintergrund bewegt sich", "Animated background"), self.onoff(s.gb("bg_animated", true)), Some(s.gb("bg_animated", true))),
                    r("mono_bg", self.tde("Hintergrund einfarbig", "Plain background"), self.onoff(s.gb("mono_bg", false)), Some(s.gb("mono_bg", false))),
                    r("show_floor_line", self.tde("Boden anzeigen", "Show floor"), self.onoff(s.gb("show_floor_line", true)), Some(s.gb("show_floor_line", true))),
                ]
            }
            PAGE_GAME => vec![
                r("language", self.get_text("language"), self.lang(), None),
                r("focus_pause", self.tde("Pause, wenn das Fenster den Fokus verliert", "Pause when the window loses focus"), self.onoff(s.gb("pause_on_focus_loss", true)), Some(s.gb("pause_on_focus_loss", true))),
                r("ghost", self.tde("Rekord-Geist anzeigen", "Show best-run ghost"), self.onoff(s.gb("ghost", true)), Some(s.gb("ghost", true))),
            ],
            _ => vec![
                r("update", self.tde("Nach Updates suchen", "Check for updates"), format!("v{}", env!("CARGO_PKG_VERSION")), None),
                r("auto_update_check", self.tde("Beim Start nach Updates suchen", "Check for updates at startup"), self.onoff(s.gb("auto_update_check", true)), Some(s.gb("auto_update_check", true))),
                r("export_lang", self.get_text("export_language"), String::new(), None),
            ],
        }
    }

    fn settings_page_title(&self) -> String {
        match self.settings_page { PAGE_DISPLAY => self.tde("Darstellung", "Display"), PAGE_GAME => self.tde("Spiel", "Game"), _ => "System".into() }
    }

    /// (Zeilen-Rechtecke, Zurück-Knopf)
    fn settings_page_rects(&self, n: usize) -> (Vec<Rect>, Rect) {
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let row_h = if n > 8 { 31 } else { 42 };
        let gap = 6;
        let top = if n > 8 { 98 } else { 112 };
        let rw = 520.min(w - 40);
        let rows = (0..n).map(|i| Rect::new(w / 2 - rw / 2, top + i as i32 * (row_h + gap), rw, row_h)).collect();
        let back_y = (top + n as i32 * (row_h + gap) + 10).min(h - 58);
        (rows, Rect::new(w / 2 - 120, back_y, 240, 46))
    }

    pub fn draw_settings_more_menu(&mut self) -> Vec<Rect> {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let title = self.settings_page_title();
        self.draw_screen_title(&bs, &title, 50);
        let rows = self.settings_rows();
        let (rects, back) = self.settings_page_rects(rows.len());
        let dark = self.is_dark_mode();
        let sf = self.small_font.clone();
        let n = rows.len() + 1;
        for (i, (row, r)) in rows.iter().zip(rects.iter()).enumerate() {
            let hv = r.collidepoint(mp) || self.nav_is_focused("settings_page", i, n);
            let fill = if hv { if dark { (66, 62, 94) } else { (226, 222, 250) } } else { self.get_panel_color() };
            let border = if hv { (150, 130, 230) } else if dark { (70, 66, 96) } else { (200, 196, 225) };
            draw::rect(&bs, fill, *r, 0, 8);
            draw::rect(&bs, border, *r, 2, 8);
            // Wert rechts als Pille
            let mut val_w = 0;
            if !row.value.is_empty() {
                let (pc, tc) = match row.on {
                    Some(true) => ((70, 170, 110), (255, 255, 255)),
                    Some(false) => (if dark { (80, 78, 98) } else { (200, 198, 214) }, if dark { (220, 218, 235) } else { (60, 58, 80) }),
                    None => (if dark { (90, 76, 160) } else { (130, 110, 220) }, (255, 255, 255)),
                };
                let vt = sf.render_fit(&row.value, 170, tc);
                val_w = vt.get_width() + 24;
                let pr = Rect::new(r.right() - val_w - 8, r.centery() - 13, val_w, 26);
                draw::rect(&bs, pc, pr, 0, 6);
                bs.blit(&vt, (pr.centerx() - vt.get_width() / 2, pr.centery() - vt.get_height() / 2));
            } else {
                let a = sf.render("›", true, self.get_text_color());
                bs.blit(&a, (r.right() - 24, r.centery() - a.get_height() / 2));
            }
            let lt = sf.render_fit(&row.label, r.w - val_w - 40, self.get_text_color());
            bs.blit(&lt, (r.x + 16, r.centery() - lt.get_height() / 2));
        }
        let bh = back.collidepoint(mp) || self.nav_is_focused("settings_page", rows.len(), n);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(self.menu_font.clone()), hovered: bh, ..Default::default() });
        // Meldungen (z.B. Update-Suche)
        self.cleanup_overlays();
        let mut y = 8;
        for o in self.overlays.clone() {
            let t = sf.render(&o.text, true, o.color);
            bs.blit(&t, ((self.WIDTH - t.get_width()) / 2, y));
            y += t.get_height() + 4;
        }
        self.flip();
        let mut all = rects;
        all.push(back);
        all
    }

    /// Ereignisse der Einstellungsseite
    pub fn settings_page_loop(&mut self) {
        let rects = self.draw_settings_more_menu();
        let keys: Vec<&'static str> = self.settings_rows().into_iter().map(|r| r.key).collect();
        for ev in self.event_get() {
            let mut act: Option<(&str, i32)> = None;
            match ev {
                Event::Quit => self.quit(),
                Event::KeyDown { key, .. } => {
                    if key == K_m || self.is_back_key(key) { act = Some(("back", 1)); }
                    else if key == K_LEFT || key == K_RIGHT {
                        if let Some(i) = self.nav_get_focus("settings_page", rects.len()).filter(|i| *i < keys.len()) {
                            act = Some((keys[i], if key == K_LEFT { -1 } else { 1 }));
                        }
                    } else if let Some(dd) = self.nav_direction(key) {
                        let cur = self.nav_get_focus("settings_page", rects.len());
                        self.nav_set_focus("settings_page", nav_move(&rects, cur, dd));
                    } else if self.is_nav_confirm(key) {
                        if let Some(i) = self.nav_get_focus("settings_page", rects.len()) { act = Some((if i < keys.len() { keys[i] } else { "back" }, 1)); }
                    }
                }
                Event::MouseButtonDown { button, .. } if button == 1 || button == 3 => {
                    let m = self.get_mouse_pos_logical();
                    if let Some(i) = rects.iter().position(|r| r.collidepoint(m)) {
                        act = Some((if i < keys.len() { keys[i] } else { "back" }, if button == 3 { -1 } else { 1 }));
                    }
                }
                _ => {}
            }
            if let Some((k, dir)) = act { self.settings_apply(k, dir); }
        }
    }

    fn flip_bool(&mut self, key: &str, dflt: bool) {
        let v = !self.settings.gb(key, dflt);
        self.settings.insert(key.into(), json!(v));
        self.save_section("settings", None);
    }

    fn settings_apply(&mut self, key: &str, dir: i32) {
        match key {
            "back" => { self.play_sound(Snd::ClickBack); self.in_settings_more_menu = false; self.in_settings_menu = true; return; }
            "theme" => { let v = !self.is_dark_mode(); self.settings.insert("dark_mode".into(), json!(v)); self.save_section("settings", None); }
            "legacy_art" => { self.flip_bool("legacy_art", false); super::art::set_legacy(self.settings.gb("legacy_art", false)); }
            "gui_scale" => {
                let cur = self.settings.gf("gui_scale", 1.0);
                let i = GUI_STEPS.iter().position(|v| (*v - cur).abs() < 0.01).unwrap_or(3) as i32;
                let ni = (i + dir).rem_euclid(GUI_STEPS.len() as i32) as usize;
                self.settings.insert("gui_scale".into(), json!(GUI_STEPS[ni]));
                self.save_section("settings", None);
            }
            "fullscreen" => self.toggle_fullscreen(),
            "fps" => {
                let cur = self.settings.gi("fps_limit", 0);
                let i = FPS_STEPS.iter().position(|v| *v == cur).unwrap_or(0) as i32;
                let ni = (i + dir).rem_euclid(FPS_STEPS.len() as i32) as usize;
                self.settings.insert("fps_limit".into(), json!(FPS_STEPS[ni]));
                self.save_section("settings", None);
            }
            "effects" => self.flip_bool("effects", true),
            "bg_stars" => self.flip_bool("bg_stars", true),
            "bg_rings_enabled" => self.flip_bool("bg_rings_enabled", true),
            "bg_animated" => self.flip_bool("bg_animated", true),
            "mono_bg" => self.flip_bool("mono_bg", false),
            "show_floor_line" => self.flip_bool("show_floor_line", true),
            "language" => { self.in_settings_more_menu = false; self.in_settings_menu = true; self.open_language_select_from_settings(); return; }
            "focus_pause" => self.flip_bool("pause_on_focus_loss", true),
            "ghost" => self.flip_bool("ghost", true),
            "auto_update_check" => self.flip_bool("auto_update_check", true),
            "update" => { self.play_sound(Snd::Click); self.start_update_check(true); return; }
            "export_lang" => { self.play_sound(Snd::Click); self.export_current_language(); return; }
            _ => return,
        }
        self.play_sound(Snd::Click);
    }

    /// Bildschirm-Titel mit hartem Schatten (einheitlich für alle Seiten)
    pub fn draw_screen_title(&self, bs: &Surface, text: &str, y: i32) {
        let w = self.WIDTH;
        let t = self.font.render_fit(text, w - 260, self.get_text_color());
        let sh = self.font.render_fit(text, w - 260, if self.is_dark_mode() { (12, 10, 20) } else { (190, 186, 214) });
        bs.blit(&sh, (w / 2 - t.get_width() / 2 + 3, y + 3));
        bs.blit(&t, (w / 2 - t.get_width() / 2, y));
    }
}
