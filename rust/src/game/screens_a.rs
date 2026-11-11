//! Tutorial, Sprachauswahl, Profil-Bildschirme, Mehr-Menü, Profil-Hub (Python 5873-6740).

use super::core::*;
use super::menus::fit_surf;
use super::state::{Snd, G};
use crate::data::d;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

pub const PLAYER_COLOR_NAMES: [&str; 6] = ["blue", "darkgreen", "lightblue", "pink", "orange", "yellow"];
pub fn player_color(name: &str) -> Option<(i32, i32, i32)> {
    match name {
        "blue" => Some((45, 95, 235)), "darkgreen" => Some((20, 100, 40)), "lightblue" => Some((80, 180, 255)),
        "pink" => Some((255, 105, 180)), "orange" => Some((255, 120, 0)), "yellow" => Some(GOLD), _ => None,
    }
}

fn lang_keys_map(k: &str) -> &str {
    match k { "Russisch" => "Русский", "Turkce" => "Türkçe", "Schwiizerdutsch" => "Schwiizerdütsch", other => other }
}

pub fn tutorial_fallback() -> Value { d("_TUTORIAL_FALLBACK").clone() }

/// Text mit Wortumbruch in Zeilen zerlegen (Muster aus draw_tutorial)
pub fn wrap_words(font: &Font, text: &str, max_w: i32) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for w in text.split_whitespace() {
        let test = format!("{} {}", cur, w).trim().to_string();
        if font.size(&test).0 > max_w {
            out.push(cur.clone());
            cur = w.to_string();
        } else {
            cur = test;
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

fn langs_get(v: &Value, lang: &str) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => v.gos(lang).or_else(|| v.gos("Deutsch")).unwrap_or_default(),
    }
}

impl G {
    // ---------------- Tutorial ----------------
    pub fn _ensure_tutorial_json(&mut self) {
        let data = self.load_tutorial_data(Value::Null);
        if data.is_null() || data.gv("steps").is_none() { self.save_tutorial_data(tutorial_fallback()); }
    }
    pub fn _get_tutorial_steps(&mut self) -> Vec<Value> {
        self._ensure_tutorial_json();
        let data = self.load_tutorial_data(tutorial_fallback());
        if !is_truthy(&data) || data.gv("steps").is_none() { return tutorial_fallback().garr("steps"); }
        data.garr("steps")
    }
    pub fn _tutorial_lang(&self) -> String {
        let l = self.settings.gs("language", "Deutsch");
        if ["Deutsch", "English", "Русский", "Türkçe", "Schwiizerdütsch"].contains(&l.as_str()) { l } else { "Deutsch".into() }
    }

    pub fn draw_tutorial(&mut self) -> (Option<Rect>, Option<Rect>) {
        let steps = self._get_tutorial_steps();
        let lang = self._tutorial_lang();
        let total = steps.len().max(1);
        let step = steps.get(self.tutorial_step.min(total - 1)).cloned().unwrap_or(json!({}));
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let dark = self.is_dark_mode();
        let (pw, ph) = (560, 320);
        let px = self.WIDTH / 2 - pw / 2;
        let py = self.HEIGHT / 2 - ph / 2;
        let panel = Rect::new(px, py, pw, ph);
        draw::rect(&bs, self.get_panel_color(), panel, 0, 18);
        let border_col = if !dark { (100, 160, 255) } else { (80, 120, 200) };
        draw::rect(&bs, border_col, panel, 3, 18);
        let sf = self.small_font.clone();
        let ct = sf.render(&format!("{} / {}", self.tutorial_step + 1, total), true, (150, 150, 180));
        bs.blit(&ct, (px + pw - ct.get_width() - 16, py + 14));
        let title = langs_get(step.gv("title").unwrap_or(&json!({})), &lang);
        let ts = self.menu_font.render(&title, true, if !dark { (80, 140, 255) } else { (120, 170, 255) });
        bs.blit(&ts, (px + pw / 2 - ts.get_width() / 2, py + 20));
        draw::line(&bs, border_col, (px + 20, py + 60), (px + pw - 20, py + 60), 1);
        let body = langs_get(step.gv("text").unwrap_or(&json!({})), &lang);
        let mut ty = py + 72;
        for ln in wrap_words(&sf, &body, pw - 40) {
            bs.blit(&sf.render(&ln, true, self.get_text_color()), (px + 20, ty));
            ty += 24;
        }
        let hint = step.gv("hint").map(|h| langs_get(h, &lang)).unwrap_or_default();
        if !hint.is_empty() {
            let parts: Vec<String> = if hint.contains('|') { hint.split('|').map(|p| p.trim().to_string()).collect() } else { vec![hint.clone()] };
            let (part_h, gap_h) = (28, 5);
            let th = parts.len() as i32 * part_h + (parts.len() as i32 - 1) * gap_h;
            let hy0 = py + ph - 60 - th;
            let bgc = if dark { (55, 75, 115) } else { (215, 228, 255) };
            let txc = if dark { (180, 210, 255) } else { (30, 55, 140) };
            for (pi, part) in parts.iter().enumerate() {
                let hy = hy0 + pi as i32 * (part_h + gap_h);
                let hbg = Rect::new(px + 20, hy, pw - 40, part_h);
                draw::rect(&bs, bgc, hbg, 0, 7);
                let hs = sf.render(part, true, txc);
                bs.blit(&hs, (hbg.centerx() - hs.get_width() / 2, hbg.centery() - hs.get_height() / 2));
            }
        }
        let (btn_y, btn_h) = (py + ph - 52, 40);
        let is_last = self.tutorial_step >= total - 1;
        let next_label = match lang.as_str() {
            "English" => if is_last { "Done!" } else { "Next >>" },
            "Русский" => if is_last { "Готово!" } else { "Далее >>" },
            "Türkçe" => if is_last { "Bitti!" } else { "Ileri >>" },
            "Schwiizerdütsch" => if is_last { "Fertig!" } else { "Witer >>" },
            _ => if is_last { "Fertig!" } else { "Weiter >>" },
        };
        let nc = if is_last { (80, 200, 120) } else { (80, 140, 255) };
        let next_rect = Rect::new(px + pw - 160, btn_y, 140, btn_h);
        let mp = self.get_mouse_pos_logical();
        self.draw_ui_button(&bs, next_rect, next_label, BtnOpts {
            font: Some(self.menu_font.clone()), hovered: next_rect.collidepoint(mp), base_color: Some(nc),
            hover_color: Some(((nc.0 + 30).min(255), (nc.1 + 30).min(255), (nc.2 + 30).min(255))), text_color: Some((255, 255, 255)),
            border_color: Some((0.max(nc.0 - 40), 0.max(nc.1 - 40), 0.max(nc.2 - 40))), hover_border_color: Some((255, 255, 255)), ..Default::default()
        });
        let mut back_rect = None;
        if self.tutorial_step > 0 {
            let bl = match lang.as_str() { "English" => "<< Back", "Русский" => "<< Назад", "Türkçe" => "<< Geri", "Schwiizerdütsch" => "<< Zrugg", _ => "<< Zurück" };
            let br = Rect::new(px + 20, btn_y, 140, btn_h);
            self.draw_ui_button(&bs, br, bl, BtnOpts { font: Some(self.menu_font.clone()), hovered: br.collidepoint(mp), border_color: Some((160, 160, 180)), border_width: Some(1), ..Default::default() });
            back_rect = Some(br);
        }
        let dot_y = btn_y + btn_h / 2;
        let dot_start = self.WIDTH / 2 - (total as i32 * 14) / 2;
        for i in 0..total {
            let col = if i == self.tutorial_step { (80, 140, 255) } else if !dark { (160, 160, 200) } else { (80, 80, 100) };
            draw::circle(&bs, col, (dot_start + i as i32 * 14, dot_y), if i == self.tutorial_step { 4 } else { 3 }, 0);
        }
        self.flip();
        (Some(next_rect), back_rect)
    }

    pub fn _finish_tutorial(&mut self) {
        self.stats.insert("tutorial_seen".into(), json!(true));
        self.save_section("stats", None);
        self.in_tutorial = false;
        self._route_to_menu_or_gate();
        self.tutorial_step = 0;
    }

    // ---------------- Sprachauswahl ----------------
    pub fn _draw_flag(&self, s: &Surface, r: Rect, flag_type: &str, colors: &[(i32, i32, i32)]) {
        let (x, y, w, h) = (r.x, r.y, r.w, r.h);
        draw::rect(s, (80, 80, 80), r, 0, 6);
        match flag_type {
            "hstripes" => {
                let sh = h / 3;
                for (i, c) in colors.iter().take(3).enumerate() {
                    let i = i as i32;
                    draw::rect(s, *c, Rect::new(x, y + i * sh, w, sh + if i == 2 { 1 } else { 0 }), 0, 0);
                }
            }
            "english" => {
                let sh = 1.max(h / 7);
                for i in 0..7 {
                    let c = if i % 2 == 0 { colors[1] } else { colors[2] };
                    draw::rect(s, c, Rect::new(x, y + i * sh, w, sh + if i == 6 { h - 7 * sh } else { 0 }), 0, 0);
                }
                let cw = w * 2 / 5;
                let chh = sh * 4;
                draw::rect(s, colors[0], Rect::new(x, y, cw, chh), 0, 0);
                for row in 0..3 {
                    for col in 0..(if row % 2 == 0 { 3 } else { 2 }) {
                        let sx = x + cw / 6 + col * (cw / 3) + if row % 2 != 0 { cw / 6 } else { 0 };
                        let sy = y + chh / 7 + row * (chh / 4);
                        draw::circle(s, colors[2], (sx, sy), 2, 0);
                    }
                }
            }
            "turkey" => {
                draw::rect(s, colors[0], r, 0, 0);
                let (cx, cy) = (x + w * 2 / 5, y + h / 2);
                let cr = h / 4;
                draw::circle(s, colors[1], (cx, cy), cr, 0);
                draw::circle(s, colors[0], (cx + cr / 3, cy), (cr as f64 * 0.82) as i32, 0);
                let stx = (cx as f64 + cr as f64 * 1.2) as i32;
                let sty = cy;
                let ro = (cr as f64 * 0.45) as i32;
                let ri = (cr as f64 * 0.18) as i32;
                let pts: Vec<(f64, f64)> = (0..10).map(|i| {
                    let rr = if i % 2 == 0 { ro } else { ri } as f64;
                    let a = ((-90 + i * 36) as f64).to_radians();
                    (stx as f64 + rr * a.cos(), sty as f64 + rr * a.sin())
                }).collect();
                draw::polygon(s, colors[1], &pts, 0);
            }
            "swiss" => {
                draw::rect(s, colors[0], r, 0, 0);
                let (cx, cy) = (x + w / 2, y + h / 2);
                let arm = 4.max(h / 4);
                let th = 3.max(h / 8);
                draw::rect(s, colors[1], Rect::new(cx - th / 2, cy - arm, th, arm * 2), 0, 0);
                draw::rect(s, colors[1], Rect::new(cx - arm, cy - th / 2, arm * 2, th), 0, 0);
            }
            _ => {}
        }
        draw::rect(s, (40, 40, 40), r, 2, 6);
    }

    pub fn draw_language_select(&mut self) -> (Rect, Rect, Rect, Vec<Rect>) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let dark = self.is_dark_mode();
        let mp = self.get_mouse_pos_logical();
        let accent = if !dark { (80, 140, 255) } else { (120, 170, 255) };
        let total = 5 + if self._lang_from_settings { 0 } else { 2 } + 1;
        let weiter_idx = total - 1;
        let w = self.WIDTH;
        let t1 = self.font.render("JumpNDodge", true, accent);
        let t1s = self.font.render("JumpNDodge", true, if dark { (80, 80, 110) } else { (140, 140, 170) });
        bs.blit(&t1s, (w / 2 - t1.get_width() / 2 + 3, 13));
        bs.blit(&t1, (w / 2 - t1.get_width() / 2, 10));
        let t2 = self.small_font.render(&self.get_text("choose_language"), true, self.get_text_color());
        bs.blit(&t2, (w / 2 - t2.get_width() / 2, 58));
        let (card_w, card_h, flag_h, spacing) = (160, 150, 80, 18);
        let langs = d("_LANG_DATA").as_array().unwrap().clone();
        let (row1_y, row2_y) = (80, 80 + card_h + 14);
        let row_start = |n: i32| w / 2 - (n * card_w + (n - 1) * spacing) / 2;
        let mut card_rects = vec![Rect::default(); 5];
        let sf = self.small_font.clone();
        for (i, ld) in langs.iter().enumerate() {
            let (cx, cy) = if i < 3 { (row_start(3) + i as i32 * (card_w + spacing), row1_y) } else { (row_start(2) + (i as i32 - 3) * (card_w + spacing), row2_y) };
            let cr = Rect::new(cx, cy, card_w, card_h);
            let fr = Rect::new(cx + 6, cy + 6, card_w - 12, flag_h);
            card_rects[i] = cr;
            let hov = cr.collidepoint(mp) || self.nav_is_focused_d("language_select", i, total, weiter_idx);
            let key = ld.gs("key", "");
            let selected = self.settings.gs("language", "Deutsch") == lang_keys_map(&key);
            draw::rect(&bs, if dark { (100, 100, 120) } else { (185, 185, 205) }, Rect::new(cx + 3, cy + 3, card_w, card_h), 0, 14);
            let mut bg = if hov { self.get_button_hover_color() } else { self.get_panel_color() };
            if selected { bg = if dark { (38, 78, 42) } else { (210, 248, 218) }; }
            draw::rect(&bs, bg, cr, 0, 14);
            let bw = if selected || hov { 3 } else { 1 };
            let bc = if selected { (80, 220, 110) } else if hov { (100, 160, 255) } else { (160, 160, 180) };
            draw::rect(&bs, bc, cr, bw, 14);
            let cols: Vec<(i32, i32, i32)> = ld.garr("flag_colors").iter().map(|c| { let a = ints(c); (a[0] as i32, a[1] as i32, a[2] as i32) }).collect();
            self._draw_flag(&bs, fr, &ld.gs("flag_type", ""), &cols);
            let mut ns = sf.render(&ld.gs("display", ""), true, self.get_text_color());
            let mnw = card_w - 12;
            if ns.get_width() > mnw { ns = ns.subsurface(Rect::new(0, 0, mnw, ns.get_height())); }
            bs.blit(&ns, (cx + card_w / 2 - ns.get_width().min(mnw) / 2, cy + flag_h + 10));
            let sc = if !dark { (60, 170, 90) } else { (70, 180, 100) };
            let mut ss = sf.render(&ld.gs("sample", ""), true, sc);
            let msw = card_w - 10;
            if ss.get_width() > msw { ss = ss.subsurface(Rect::new(0, 0, msw, ss.get_height())); }
            bs.blit(&ss, (cx + card_w / 2 - ss.get_width().min(msw) / 2, cy + flag_h + 32));
            if selected {
                let ck = sf.render("✔", true, (80, 220, 110));
                bs.blit(&ck, (cx + card_w - ck.get_width() - 6, cy + 4));
            }
        }
        let mode_y = row2_y + card_h + 18;
        let (btn_w, btn_h, gap) = (160, 38, 16);
        let mut light_rect = Rect::new(-999, -999, 1, 1);
        let mut dark_rect = Rect::new(-999, -999, 1, 1);
        let weiter_y;
        if !self._lang_from_settings {
            let ml = sf.render(&format!("{}:", self.get_text("color_scheme")), true, self.get_text_color());
            bs.blit(&ml, (w / 2 - ml.get_width() / 2, mode_y));
            light_rect = Rect::new(w / 2 - btn_w - gap / 2, mode_y + 28, btn_w, btn_h);
            dark_rect = Rect::new(w / 2 + gap / 2, mode_y + 28, btn_w, btn_h);
            for (mi, (r, label, sel, is_light)) in [(light_rect, self.get_text("light_mode"), !dark, true), (dark_rect, self.get_text("dark_mode"), dark, false)].iter().enumerate() {
                let hov = r.collidepoint(mp) || self.nav_is_focused_d("language_select", 5 + mi, total, weiter_idx);
                let (bg, border) = if *sel {
                    (if *is_light { (255, 220, 80) } else { (40, 40, 60) }, if *is_light { (220, 180, 0) } else { (120, 120, 200) })
                } else {
                    (if hov { self.get_button_hover_color() } else { self.get_button_color() }, (160, 160, 180))
                };
                draw::rect(&bs, bg, *r, 0, 10);
                draw::rect(&bs, border, *r, if *sel { 3 } else { 2 }, 10);
                let ls = sf.render(label, true, if *sel && *is_light { (30, 30, 30) } else { self.get_text_color() });
                bs.blit(&ls, (r.centerx() - ls.get_width() / 2, r.centery() - ls.get_height() / 2));
            }
            weiter_y = mode_y + 28 + btn_h + 12;
        } else {
            weiter_y = mode_y + 12;
        }
        let wr = Rect::new(w / 2 - 120, weiter_y, 240, 46);
        let wh = wr.collidepoint(mp) || self.nav_is_focused_d("language_select", weiter_idx, total, weiter_idx);
        draw::rect(&bs, if !wh { (80, 200, 120) } else { (100, 230, 140) }, wr, 0, 12);
        let ws = self.menu_font.render(&self.get_text("lang_continue"), true, (255, 255, 255));
        bs.blit(&ws, (wr.centerx() - ws.get_width() / 2, wr.centery() - ws.get_height() / 2));
        self.flip();
        (wr, light_rect, dark_rect, card_rects)
    }

    pub fn _select_language(&mut self, lang_key: &str) {
        let real = lang_keys_map(lang_key).to_string();
        self.settings.insert("language".into(), json!(real));
        if let Some(i) = self.languages.iter().position(|l| *l == real) { self.language_index = i; }
        self.save_section("settings", None);
        self.update_button_texts();
    }

    pub fn _finish_language_select(&mut self) {
        self.in_language_select = false;
        if self._lang_from_settings {
            self._lang_from_settings = false;
            self.in_settings_menu = true;
        } else {
            self.in_tutorial = true;
        }
    }

    pub fn open_language_select_from_settings(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_settings_menu = false;
        self.in_language_select = true;
        self._lang_from_settings = true;
    }

    pub fn set_all_menu_flags(&mut self, val: bool) {
        self.in_menu = val;
        self.game_running = false;
        self.showing_stats = false;
        self.in_shop = false;
        self.in_loadout = false;
        self.in_admin_login = false;
        self.in_admin_panel = false;
        self.in_more_menu = false;
        self.in_classic_game = false;
        self.in_settings_menu = false;
        self.in_experimental_menu = false;
        self.in_help = false;
        self.in_daily_rewards = false;
        self.in_loading = false;
        self.in_tutorial = false;
        self.in_language_select = false;
        self.in_achievements_screen = false;
        self.in_profile_select = false;
        self.in_profile_name_input = false;
        self.in_profile_hub = false;
    }

    // ---------------- Profile ----------------
    pub fn _enter_profile(&mut self, name: &str) {
        self.load_profile(name);
        self.check_local_verified_key_file();
        self._flush_pending_online_deletions();
        self.set_all_menu_flags(false);
        if !self.stats.gb("tutorial_seen", false) { self.in_language_select = true; } else { self._route_to_menu_or_gate(); }
        self.update_button_texts();
    }

    pub fn _open_new_profile_input(&mut self, from_settings: bool) {
        self._profile_name_text.clear();
        self._profile_name_msg.clear();
        self._profile_from_settings = from_settings;
        self.in_profile_select = false;
        self.in_settings_menu = false;
        self.in_profile_name_input = true;
    }

    pub fn draw_profile_name_input(&mut self) -> Rect {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let dark = self.is_dark_mode();
        let (pw, ph) = (520, 280);
        let px = self.WIDTH / 2 - pw / 2;
        let py = self.HEIGHT / 2 - ph / 2;
        let panel = Rect::new(px, py, pw, ph);
        draw::rect(&bs, self.get_panel_color(), panel, 0, 18);
        draw::rect(&bs, if !dark { (100, 160, 255) } else { (80, 120, 200) }, panel, 3, 18);
        let t = self.menu_font.render(&self.get_text("enter_name"), true, if !dark { (80, 140, 255) } else { (120, 170, 255) });
        bs.blit(&t, (px + pw / 2 - t.get_width() / 2, py + 24));
        let bx = Rect::new(px + 40, py + 90, pw - 80, 46);
        draw::rect(&bs, self.get_bg_color(), bx, 0, 10);
        draw::rect(&bs, (120, 180, 255), bx, 2, 10);
        let cursor = if (self.get_ticks() / 500) % 2 == 0 { "|" } else { " " };
        let inp = self.menu_font.render(&format!("{}{}", self._profile_name_text, cursor), true, self.get_text_color());
        bs.blit(&inp, (bx.x + 10, bx.centery() - inp.get_height() / 2));
        if !self._profile_name_msg.is_empty() {
            let m = self.small_font.render(&self._profile_name_msg, true, (220, 60, 60));
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, py + 150));
        }
        let cr = Rect::new(px + pw / 2 - 100, py + ph - 70, 200, 44);
        let mp = self.get_mouse_pos_logical();
        draw::rect(&bs, if !cr.collidepoint(mp) { (60, 180, 100) } else { (80, 210, 120) }, cr, 0, 12);
        let cl = self.menu_font.render(&self.get_text("confirm"), true, (255, 255, 255));
        bs.blit(&cl, (cr.centerx() - cl.get_width() / 2, cr.centery() - cl.get_height() / 2));
        self.flip();
        cr
    }

    pub fn _confirm_profile_name(&mut self) {
        let name = self._profile_name_text.trim().to_string();
        if name.is_empty() { self._profile_name_msg = self.get_text("name_empty"); return; }
        if self.get_all_profiles().contains_key(&name) { self._profile_name_msg = self.get_text("name_taken"); return; }
        if !self.is_name_allowed(&name) { self._profile_name_msg = self.get_text("name_filtered"); return; }
        self.create_profile(&name);
        self.in_profile_name_input = false;
        self.in_language_select = true;
    }

    pub fn draw_profile_select(&mut self) -> (Vec<(Rect, String)>, Vec<(Rect, String)>) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let dark = self.is_dark_mode();
        let all = self.get_all_profiles();
        let auto = self.get_auto_start_profile();
        let mp = self.get_mouse_pos_logical();
        let w = self.WIDTH;
        let title = self.font.render(&self.get_text("who_is_playing"), true, self.get_text_color());
        let sh = self.font.render(&self.get_text("who_is_playing"), true, (100, 100, 120));
        bs.blit(&sh, (w / 2 - title.get_width() / 2 + 2, 42));
        bs.blit(&title, (w / 2 - title.get_width() / 2, 40));
        let mut items: Vec<String> = all.keys().cloned().collect();
        items.push("+".into());
        let cols = 3.min(items.len()) as i32;
        let (card_w, card_h, gap) = (220, 120, 24);
        let total_w = cols * card_w + (cols - 1) * gap;
        let sx = w / 2 - total_w / 2;
        let sy = 130;
        let mut card_rects = vec![];
        let mut cb_rects = vec![];
        let mut last_bottom = sy;
        let sf = self.small_font.clone();
        for (i, name) in items.iter().enumerate() {
            let (col, row) = (i as i32 % cols, i as i32 / cols);
            let r = Rect::new(sx + col * (card_w + gap), sy + row * (card_h + gap), card_w, card_h);
            last_bottom = last_bottom.max(r.bottom());
            card_rects.push((r, name.clone()));
            let hov = r.collidepoint(mp) || self.nav_is_focused("profile_select", i, items.len());
            if name == "+" {
                draw::rect(&bs, if !hov { (60, 140, 80) } else { (80, 170, 100) }, r, 0, 14);
                draw::rect(&bs, (100, 220, 140), r, 2, 14);
                let plus = self.font.render("+", true, (255, 255, 255));
                bs.blit(&plus, (r.centerx() - plus.get_width() / 2, r.centery() - plus.get_height() / 2));
                let lbl = sf.render(&self.get_text("add_profile"), true, (200, 255, 210));
                bs.blit(&lbl, (r.centerx() - lbl.get_width() / 2, r.bottom() - 26));
            } else {
                draw::rect(&bs, if hov { self.get_button_hover_color() } else { self.get_button_color() }, r, 0, 14);
                draw::rect(&bs, if !dark { (120, 180, 255) } else { (80, 130, 200) }, r, 2, 14);
                let pdata = all.get(name).cloned().unwrap_or(json!({}));
                let pstats = pdata.gobj("stats");
                let ns = self.menu_font.render(name, true, self.get_text_color());
                bs.blit(&ns, (r.centerx() - ns.get_width() / 2, r.y + 16));
                bs.blit(&sf.render(&format!("Coins: {}", pstats.gi("coins", 0)), true, if !dark { (180, 180, 100) } else { (220, 210, 100) }), (r.x + 14, r.y + 56));
                bs.blit(&sf.render(&format!("Runden: {}", pstats.gi("rounds", 0)), true, self.get_text_color()), (r.x + 14, r.y + 80));
                let pc = player_color(&pdata.gobj("settings").gs("player_color", "blue")).unwrap_or(BLUE);
                self.draw_avatar(&bs, Rect::new(r.right() - 46, r.y + 40, 32, 32), Some(pc), false, None);
                let dr = Rect::new(r.right() - 28, r.y + 6, 22, 22);
                draw::rect(&bs, if dr.collidepoint(mp) { (200, 60, 60) } else { (140, 40, 40) }, dr, 0, 6);
                let m = 5;
                draw::line(&bs, (255, 255, 255), (dr.x + m, dr.y + m), (dr.right() - m, dr.bottom() - m), 2);
                draw::line(&bs, (255, 255, 255), (dr.right() - m, dr.y + m), (dr.x + m, dr.bottom() - m), 2);
                let checked = auto.as_deref() == Some(name.as_str());
                let cb = Rect::new(r.x + 10, r.bottom() - 20, 14, 14);
                cb_rects.push((cb, name.clone()));
                let cbh = cb.collidepoint(mp);
                draw::rect(&bs, self.get_bg_color(), cb, 0, 3);
                draw::rect(&bs, if checked || cbh { (255, 210, 60) } else { (140, 140, 150) }, cb, 2, 3);
                if checked {
                    draw::line(&bs, (255, 210, 60), (cb.x + 3, cb.centery()), (cb.centerx() - 1, cb.bottom() - 3), 2);
                    draw::line(&bs, (255, 210, 60), (cb.centerx() - 1, cb.bottom() - 3), (cb.right() - 2, cb.y + 2), 2);
                }
                let cl = self.tab_font.render(&self.get_text("auto_start_profile"), true, self.get_text_color());
                bs.blit(&cl, (cb.right() + 6, cb.centery() - cl.get_height() / 2));
            }
        }
        let hs = sf.render(&self.get_text("auto_start_hint"), true, self.get_text_color());
        bs.blit(&hs, (w / 2 - hs.get_width() / 2, last_bottom + 16));
        self.flip();
        (card_rects, cb_rects)
    }

    pub fn draw_coin_display(&self, bs: &Surface) {
        let coin_bg = Rect::new(15, 15, 200, 35);
        draw::rect(bs, self.get_panel_color(), coin_bg, 0, 18);
        draw::rect(bs, (200, 200, 200), coin_bg, 2, 18);
        let ct = self.small_font.render(&format!("{}: {}", self.get_text("coins"), self.stats.gi("coins", 0)), true, self.get_text_color());
        bs.blit(&ct, (25, 22));
    }

    pub fn more_menu_buttons(&mut self) -> Vec<(String, (i32, i32, i32), &'static str)> {
        if self._check_admin_keyfile() {
            vec![(self.get_text("admin"), (200, 100, 100), "admin"), (self.get_text("classic_mode"), (120, 170, 120), "classic"),
                 (self.get_text("help"), (200, 150, 100), "help"), (self.get_text("redeem_code_btn"), (80, 170, 220), "redeem"), (self.get_text("back"), (150, 150, 150), "back")]
        } else {
            vec![(self.get_text("classic_mode"), (120, 170, 120), "classic"), (self.get_text("help"), (200, 150, 100), "help"),
                 (self.get_text("redeem_code_btn"), (80, 170, 220), "redeem"), (self.get_text("back"), (150, 150, 150), "back")]
        }
    }

    pub fn draw_more_menu(&mut self) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let t = self.font.render(&self.get_text("more"), true, self.get_text_color());
        let ts = self.font.render(&self.get_text("more"), true, (100, 100, 120));
        bs.blit(&ts, (w / 2 - t.get_width() / 2 + 2, 62));
        bs.blit(&t, (w / 2 - t.get_width() / 2, 60));
        self.draw_coin_display(&bs);
        let btns = self.more_menu_buttons();
        let (sx, sy, gap) = (w / 2 - 180, 140, 58);
        for (i, (text, color, _)) in btns.iter().enumerate() {
            let r = Rect::new(sx, sy + i as i32 * gap, 360, 48);
            let hov = r.collidepoint(mp) || self.nav_is_focused("more_menu", i, btns.len());
            self.draw_ui_button(&bs, r, text, BtnOpts { font: Some(self.menu_font.clone()), hovered: hov, border_color: Some(*color), hover_border_color: Some(*color), hover_border_width: Some(3), ..Default::default() });
        }
        draw::rect(&bs, self.get_panel_color(), Rect::new(0, h - 30, w, 30), 0, 0);
        draw::line(&bs, if self.is_dark_mode() { (90, 90, 100) } else { (200, 200, 200) }, (0, h - 30), (w, h - 30), 1);
        let ft = self.small_font.render(&self.get_text("back_to_main"), true, self.get_text_color());
        bs.blit(&ft, (w / 2 - ft.get_width() / 2, h - 25));
        self.flip();
    }

    pub fn draw_profile_hub(&mut self) -> (Vec<Rect>, Vec<&'static str>, Vec<Rect>) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let dark = self.is_dark_mode();
        let w = self.WIDTH;
        let t = self.font.render(&self.get_text("profile_hub_title"), true, self.get_text_color());
        let tsh = self.font.render(&self.get_text("profile_hub_title"), true, (100, 100, 120));
        bs.blit(&tsh, (w / 2 - t.get_width() / 2 + 2, 22));
        bs.blit(&t, (w / 2 - t.get_width() / 2, 20));
        self.draw_coin_display(&bs);
        if self._server_reachable == Some(false) {
            let o = self.small_font.render(&self.get_text("offline_hint"), true, (210, 150, 70));
            bs.blit(&o, (w / 2 - o.get_width() / 2, 60));
        }
        let size = 50;
        let ar = Rect::new(w / 2 - size / 2, 100, size, size);
        draw::rect(&bs, (100, 100, 150), Rect::new(ar.x + 4, ar.y + 4, size, size), 0, 10);
        self.draw_avatar(&bs, ar, None, true, None);
        let ns = self.menu_font.render(self.current_profile_name.as_deref().unwrap_or("-"), true, self.get_text_color());
        let ny = ar.bottom() + 8;
        let us = self.current_profile_username.as_ref().map(|u| self.small_font.render(&format!(" @{}", u), true, (150, 150, 170)));
        let cw = ns.get_width() + us.as_ref().map(|u| u.get_width()).unwrap_or(0);
        let nx = w / 2 - cw / 2;
        bs.blit(&ns, (nx, ny));
        if let Some(u) = &us { bs.blit(u, (nx + ns.get_width(), ny + ns.get_height() - u.get_height())); }
        let (ss, sg) = (40, 14);
        let tw = PLAYER_COLOR_NAMES.len() as i32 * ss + (PLAYER_COLOR_NAMES.len() as i32 - 1) * sg;
        let sx0 = w / 2 - tw / 2;
        let sy = ar.bottom() + 44;
        let mut swatch_rects = vec![];
        let cur = self.settings.gs("player_color", "blue");
        for (i, cn) in PLAYER_COLOR_NAMES.iter().enumerate() {
            let sr = Rect::new(sx0 + i as i32 * (ss + sg), sy, ss, ss);
            swatch_rects.push(sr);
            draw::rect(&bs, player_color(cn).unwrap(), sr, 0, 8);
            if *cn == cur { draw::rect(&bs, if dark { (255, 255, 255) } else { (0, 0, 0) }, sr, 3, 8); }
            else if sr.collidepoint(mp) { draw::rect(&bs, (200, 200, 200), sr, 2, 8); }
        }
        let (sx, sy2, gap) = (w / 2 - 180, sy + ss + 24, 56);
        let texts = [self.get_text("stats"), self.get_text("switch_profile"), self.get_text("rename_btn"), self.get_text("back")];
        let colors = [(100, 150, 200), (200, 150, 100), (170, 130, 200), (150, 150, 150)];
        let total = PLAYER_COLOR_NAMES.len() + texts.len();
        let mut btn_rects = vec![];
        for (i, (text, color)) in texts.iter().zip(colors.iter()).enumerate() {
            let r = Rect::new(sx, sy2 + i as i32 * gap, 360, 44);
            btn_rects.push(r);
            let hov = r.collidepoint(mp) || self.nav_is_focused("profile_hub", PLAYER_COLOR_NAMES.len() + i, total);
            self.draw_ui_button(&bs, r, text, BtnOpts { font: Some(self.menu_font.clone()), hovered: hov, border_color: Some(*color), hover_border_color: Some(*color), hover_border_width: Some(3), ..Default::default() });
        }
        self.flip();
        (swatch_rects, PLAYER_COLOR_NAMES.to_vec(), btn_rects)
    }

    pub fn get_player_color(&self) -> (i32, i32, i32) { player_color(&self.settings.gs("player_color", "blue")).unwrap_or((45, 95, 235)) }

    /// draw_avatar(surface, rect, color=None, cosmetics=True, cosmetics_overrides=None)
    pub fn draw_avatar(&self, surface: &Surface, r: Rect, color: Option<(i32, i32, i32)>, cosmetics: bool, overrides: Option<&serde_json::Map<String, Value>>) {
        super::art::draw_player(surface, r, color.unwrap_or(self.get_player_color()));
        if cosmetics { self.draw_player_cosmetics(surface, r, overrides); }
    }
}
