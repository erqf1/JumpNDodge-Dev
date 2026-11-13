//! RuntimePowers, Pause-Menü, Kosmetik am Spieler zeichnen (Python 14019-14424).

use super::core::*;
use super::state::{Snd, G};
use crate::data::d;
use crate::pg::*;
use crate::util::*;
use serde_json::Value;
use std::collections::HashMap;

pub const ACTIVE_NAMES: [&str; 4] = ["Slow Motion", "Time Freeze", "Ghost Mode", "Nuke"];

fn pdb_f(name: &str, key: &str) -> f64 { d("POWERS_DB").gv(name).and_then(|p| p.gv(key)).and_then(as_f).unwrap_or(0.0) }

/// class RuntimePowers
#[derive(Clone, Debug, Default)]
pub struct RuntimePowers {
    pub equipped: Vec<String>,
    pub double_jump: bool,
    pub slow_motion: bool,
    pub shield: bool,
    pub auto_dodge: bool,
    pub phoenix: bool,
    pub magnet: bool,
    pub timers: HashMap<String, f64>,
    pub timer_order: Vec<String>,
    pub slow_remaining: f64,
    pub freeze_remaining: f64,
    pub freeze_slow_remaining: f64,
    pub ghost_remaining: f64,
    pub shield_ready: bool,
    pub nuke_remaining: f64,
    pub phoenix_cd: f64,
    pub phoenix_invuln_remaining: f64,
    pub phoenix_slow_remaining: f64,
}

impl RuntimePowers {
    pub fn new(eq: &[String]) -> RuntimePowers {
        let has = |n: &str| eq.iter().any(|e| e == n);
        let mut timers = HashMap::new();
        for n in eq { timers.insert(n.clone(), 0.0); }
        RuntimePowers {
            equipped: eq.to_vec(), double_jump: has("Double Jump"), slow_motion: has("Slow Motion"), shield: has("Shield"),
            auto_dodge: has("Auto Dodge"), phoenix: has("Phoenix"), magnet: has("Magnet"), timers, timer_order: eq.to_vec(),
            shield_ready: true, ..Default::default()
        }
    }
    pub fn timer(&self, n: &str) -> f64 { self.timers.get(n).copied().unwrap_or(0.0) }
    pub fn update_cooldowns(&mut self, dt: f64) {
        for v in self.timers.values_mut() { if *v > 0.0 { *v = (*v - dt).max(0.0); } }
        if self.slow_remaining > 0.0 { self.slow_remaining = (self.slow_remaining - dt).max(0.0); }
        if self.freeze_remaining > 0.0 { self.freeze_remaining = (self.freeze_remaining - dt).max(0.0); }
        else if self.freeze_slow_remaining > 0.0 { self.freeze_slow_remaining = (self.freeze_slow_remaining - dt).max(0.0); }
        if self.ghost_remaining > 0.0 { self.ghost_remaining = (self.ghost_remaining - dt).max(0.0); }
        if self.nuke_remaining > 0.0 { self.nuke_remaining = (self.nuke_remaining - dt).max(0.0); }
        if self.phoenix_cd > 0.0 { self.phoenix_cd = (self.phoenix_cd - dt).max(0.0); }
        if self.phoenix_invuln_remaining > 0.0 { self.phoenix_invuln_remaining = (self.phoenix_invuln_remaining - dt).max(0.0); }
        if self.phoenix_slow_remaining > 0.0 { self.phoenix_slow_remaining = (self.phoenix_slow_remaining - dt).max(0.0); }
    }
    pub fn on_hit_block(&mut self) -> bool {
        if self.ghost_remaining > 0.0 { return true; }
        if self.phoenix_invuln_remaining > 0.0 { return true; }
        if self.shield && self.shield_ready {
            self.shield_ready = false;
            self.timers.insert("Shield".into(), pdb_f("Shield", "cd") * 1000.0);
            return true;
        }
        false
    }
    pub fn tick_shield_rearm(&mut self) {
        if self.shield && !self.shield_ready && self.timer("Shield") == 0.0 { self.shield_ready = true; }
    }
    pub fn active_bindings(&self) -> Vec<String> {
        self.equipped.iter().filter(|n| ACTIVE_NAMES.contains(&n.as_str())).take(2).cloned().collect()
    }
    pub fn speed_multiplier(&self) -> f64 {
        if self.freeze_remaining > 0.0 { return 0.0; }
        if self.freeze_slow_remaining > 0.0 || self.slow_remaining > 0.0 || self.phoenix_slow_remaining > 0.0 { return 0.5; }
        1.0
    }
}

impl G {
    /// RuntimePowers.activate(power_name) -> Some(nuke?) oder None
    pub fn rp_activate(&mut self, rp: &mut RuntimePowers, name: &str) -> Option<bool> {
        if !rp.equipped.iter().any(|e| e == name) { return None; }
        if rp.timers.contains_key(name) && rp.timer(name) > 0.0 { return None; }
        match name {
            "Slow Motion" => {
                rp.slow_remaining = pdb_f("Slow Motion", "duration") * 1000.0;
                rp.timers.insert(name.into(), pdb_f(name, "cd") * 1000.0);
                let t = self.get_text("slow_motion");
                self.push_overlay(&t, (80, 150, 255), 1.0, None);
            }
            "Time Freeze" => {
                rp.freeze_remaining = 2000.0;
                rp.freeze_slow_remaining = 5000.0;
                rp.timers.insert(name.into(), pdb_f(name, "cd") * 1000.0);
                let t = self.get_text("time_freeze");
                self.push_overlay(&t, (180, 80, 255), 1.0, None);
            }
            "Ghost Mode" => {
                let dr = d("POWERS_DB")["Ghost Mode"]["duration"].as_array().cloned().unwrap_or_default();
                let (a, b) = (dr.first().and_then(as_i).unwrap_or(3), dr.get(1).and_then(as_i).unwrap_or(5));
                rp.ghost_remaining = (randint(a, b) * 1000) as f64;
                rp.timers.insert(name.into(), pdb_f(name, "cd") * 1000.0);
                let t = self.get_text("ghost_mode");
                self.push_overlay(&t, (180, 180, 80), 1.0, None);
            }
            "Nuke" => {
                rp.nuke_remaining = 4000.0;
                rp.timers.insert(name.into(), pdb_f(name, "cd") * 1000.0);
                let t = self.get_text("nuke");
                self.push_overlay(&t, (200, 50, 50), 1.0, None);
            }
            _ => {}
        }
        Some(false)
    }

    pub fn _pause_resume_countdown(&mut self, frozen: &Surface, restore_vol: f64) {
        let end = self.get_ticks() + 1000;
        let mut cl = Clock::new();
        loop {
            { let fps = self.target_fps(); cl.tick(fps); }
            let rem = 0.max(end - self.get_ticks());
            if rem == 0 { break; }
            self.sync_window_size();
            let bs = self.base_surface.clone();
            bs.blit(frozen, (0, 0));
            let dim = Surface::new_alpha(self.WIDTH, self.HEIGHT);
            dim.fill((0, 0, 0, 100));
            bs.blit(&dim, (0, 0));
            let t = self.font.render(&format!("{} ms", rem), true, (255, 255, 255));
            bs.blit(&t, (self.WIDTH / 2 - t.get_width() / 2, self.HEIGHT / 2 - t.get_height() / 2));
            self.flip();
            for ev in self.event_get() { if let Event::Quit = ev { self.quit(); } }
        }
        self.mixer.music_set_volume(restore_vol);
    }

    /// _run_pause_menu(frozen) -> "resume" | "restart" | "menu"
    pub fn _run_pause_menu(&mut self, frozen: Option<Surface>) -> &'static str {
        let frozen = frozen.unwrap_or_else(|| self.base_surface.copy());
        let normal_vol = self.mixer.music_get_volume();
        self.mixer.music_set_volume(normal_vol * 0.3);
        let (bw, bh, bg) = (280, 44, 10);
        let pw = bw + 60;
        let ph = 80 + 3 * (bh + bg);
        loop {
            self.sync_window_size();
            let bs = self.base_surface.clone();
            bs.blit(&frozen, (0, 0));
            let dim = Surface::new_alpha(self.WIDTH, self.HEIGHT);
            dim.fill(if self.is_dark_mode() { (0, 0, 0, 170) } else { (236, 238, 252, 170) });
            bs.blit(&dim, (0, 0));
            let dark = self.is_dark_mode();
            let px = self.WIDTH / 2 - pw / 2;
            let py = self.HEIGHT / 2 - ph / 2;
            draw::rect(&bs, if dark { (20, 22, 36) } else { (238, 242, 252) }, Rect::new(px, py, pw, ph), 0, 16);
            draw::rect(&bs, if dark { (80, 110, 215) } else { (140, 170, 225) }, Rect::new(px, py, pw, ph), 2, 16);
            let t = self.menu_font.render(&self.tde("Pausiert", "Paused"), true, self.get_text_color());
            bs.blit(&t, (self.WIDTH / 2 - t.get_width() / 2, py + 16));
            let mp = self.get_mouse_pos_logical();
            let bx = self.WIDTH / 2 - bw / 2;
            let by = py + 68;
            let btns: [(String, &'static str, (i32, i32, i32), (i32, i32, i32)); 3] = [
                (self.tde("Weitermachen", "Resume"), "resume", (0, 110, 50), (255, 255, 255)),
                (self.tde("Neu starten", "Restart"), "restart", self.get_button_color(), self.get_text_color()),
                (self.tde("Hauptmenü", "Main menu"), "menu", self.get_button_color(), self.get_text_color()),
            ];
            let mut rects: Vec<(Rect, &'static str)> = vec![];
            for (i, (label, action, base, tc)) in btns.iter().enumerate() {
                let r = Rect::new(bx, by + i as i32 * (bh + bg), bw, bh);
                rects.push((r, action));
                let hv = r.collidepoint(mp) || self.nav_is_focused("pause_menu", i, 3);
                let resume = *action == "resume";
                self.draw_ui_button(&bs, r, label, BtnOpts { font: Some(self.menu_font.clone()), hovered: hv, base_color: Some(*base), hover_color: Some(if resume { (0, 140, 60) } else { self.get_button_hover_color() }), text_color: Some(*tc), border_color: Some(if resume { (60, 220, 120) } else { (150, 130, 230) }), ..Default::default() });
            }
            let hint = self.small_font.render(&self.tde("ESC = Weitermachen", "ESC = resume"), true, if dark { (130, 128, 150) } else { (100, 100, 120) });
            bs.blit(&hint, (self.WIDTH / 2 - hint.get_width() / 2, py + ph - 22));
            self.flip();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self.action_matches_key(key, "pause") || self.is_back_key(key) {
                            self._pause_resume_countdown(&frozen, normal_vol);
                            return "resume";
                        }
                        if let Some(dd) = self.nav_direction(key) {
                            let rs: Vec<Rect> = rects.iter().map(|x| x.0).collect();
                            let cur = self.nav_get_focus("pause_menu", 3);
                            let n = nav_move(&rs, cur, dd);
                            self.nav_set_focus("pause_menu", n);
                        } else if self.is_nav_confirm(key) {
                            if let Some(fi) = self.nav_get_focus("pause_menu", 3) {
                                let a = rects[fi].1;
                                self.play_sound(if a == "menu" { Snd::ClickBack } else { Snd::MenuClick });
                                if a == "resume" { self._pause_resume_countdown(&frozen, normal_vol); } else { self.mixer.music_set_volume(normal_vol); }
                                return a;
                            }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let p = self.get_mouse_pos_logical();
                        for (r, a) in rects.iter() {
                            if r.collidepoint(p) {
                                self.play_sound(if *a == "menu" { Snd::ClickBack } else { Snd::MenuClick });
                                if *a == "resume" { self._pause_resume_countdown(&frozen, normal_vol); } else { self.mixer.music_set_volume(normal_vol); }
                                return a;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// draw_player_cosmetics(surface, rect, overrides=None)
    pub fn draw_player_cosmetics(&self, s: &Surface, r: Rect, overrides: Option<&serde_json::Map<String, Value>>) {
        let c = overrides.unwrap_or(&self.cosmetics_state);
        let hw = c.gos("equipped_headwear").unwrap_or_default();
        let eyes = c.gos("equipped_eyes").unwrap_or_default();
        let nose = c.gos("equipped_nose").unwrap_or_default();
        let mouth = c.gos("equipped_mouth").unwrap_or_default();
        if super::art::draw_cosmetics_tex(s, r, &hw, &eyes, &nose, &mouth, self.get_ticks()) { return; }
        let rr =|col: (i32, i32, i32), x: i32, y: i32, w: i32, h: i32, width: i32, rad: i32| { draw::rect(s, col, rect(x, y, w, h), width, rad); };
        let (cx, cy, top) = (r.centerx(), r.centery(), r.top());
        match hw.as_str() {
            "yellow_crown" => {
                let (x, y) = (cx - 10, top - 5);
                draw::polygon(s, (255, 215, 0), &[(x, y), (x + 5, y - 8), (x + 10, y), (x + 15, y - 8), (x + 20, y)], 0);
            }
            "top_hat" => {
                rr((25, 25, 30), cx - 15, top - 6, 30, 6, 0, 0);
                rr((25, 25, 30), cx - 10, top - 24, 20, 20, 0, 0);
                rr((150, 20, 20), cx - 10, top - 9, 20, 4, 0, 0);
            }
            "devil_horns" => {
                draw::polygon(s, (210, 30, 30), &[(cx - 6, top + 4), (cx - 16, top - 14), (cx - 4, top - 2)], 0);
                draw::polygon(s, (210, 30, 30), &[(cx + 6, top + 4), (cx + 16, top - 14), (cx + 4, top - 2)], 0);
            }
            "halo" => { draw::ellipse(s, (255, 200, 20), rect(cx - 24, top - 18 - 12, 48, 24), 6); }
            "propeller_hat" => {
                draw::polygon(s, (230, 70, 60), &[(cx - 24, top + 8), (cx + 24, top + 8), (cx + 15, top - 18), (cx - 15, top - 18)], 0);
                rr((230, 70, 60), cx - 24, top + 2, 48, 8, 0, 3);
                let ang = (self.get_ticks() as f64 / 60.0) % 360.0;
                let rad = ang.to_radians();
                let dx = (26.0 * rad.cos()) as i32;
                let dy = (6.0 * rad.sin()) as i32;
                draw::line(s, (60, 60, 65), (cx - dx, top - 34 - dy), (cx + dx, top - 34 + dy), 4);
                draw::line(s, (60, 60, 65), (cx, top - 44), (cx, top - 18), 4);
                draw::circle(s, (255, 210, 40), (cx, top - 34), 6, 0);
            }
            "ninja_headband" => {
                let ny = top + 6;
                rr((25, 25, 30), cx - 26, ny - 8, 52, 16, 0, 0);
                draw::polygon(s, (190, 20, 20), &[(cx + 20, ny + 8), (cx + 42, ny + 26), (cx + 22, ny + 24)], 0);
                draw::polygon(s, (190, 20, 20), &[(cx + 24, ny + 6), (cx + 46, ny + 18), (cx + 26, ny + 20)], 0);
                draw::circle(s, (190, 20, 20), (cx, ny), 5, 0);
            }
            _ => {}
        }
        let ey = cy - 8;
        match eyes.as_str() {
            "pixel_eyes" => {
                rr((255, 255, 255), cx - 14, ey - 5, 9, 9, 0, 0); rr((255, 255, 255), cx + 5, ey - 5, 9, 9, 0, 0);
                rr((20, 20, 25), cx - 11, ey - 2, 4, 4, 0, 0); rr((20, 20, 25), cx + 8, ey - 2, 4, 4, 0, 0);
            }
            "sunglasses" => {
                draw::circle(s, (20, 20, 25), (cx - 9, ey), 10, 0); draw::circle(s, (20, 20, 25), (cx + 9, ey), 10, 0);
                draw::circle(s, (35, 35, 40), (cx - 9, ey), 7, 0); draw::circle(s, (35, 35, 40), (cx + 9, ey), 7, 0);
                draw::line(s, (150, 180, 210), (cx - 13, ey - 3), (cx - 6, ey + 3), 2);
                draw::line(s, (150, 180, 210), (cx + 5, ey - 3), (cx + 12, ey + 3), 2);
                rr((20, 20, 25), cx - 4, ey - 2, 8, 4, 0, 0);
                draw::line(s, (20, 20, 25), (cx - 19, ey - 2), (cx - 22, ey - 6), 2);
                draw::line(s, (20, 20, 25), (cx + 19, ey - 2), (cx + 22, ey - 6), 2);
            }
            "3d_glasses" => {
                rr((200, 30, 30), cx - 16, ey - 7, 13, 11, 0, 0); rr((40, 120, 220), cx + 3, ey - 7, 13, 11, 0, 0); rr((20, 20, 25), cx - 4, ey - 2, 8, 3, 0, 0);
            }
            "robot_visor" => { rr((30, 30, 35), cx - 17, ey - 7, 34, 15, 0, 6); rr((80, 220, 255), cx - 13, ey - 3, 26, 7, 0, 3); }
            _ => {}
        }
        match nose.as_str() {
            "clown_nose" => { draw::circle(s, (230, 30, 30), (cx, cy), 6, 0); draw::circle(s, (150, 10, 10), (cx, cy), 6, 1); }
            "pig_nose" => {
                draw::ellipse(s, (240, 150, 170), rect(cx - 9, cy - 6, 18, 13), 0);
                draw::ellipse(s, (190, 100, 120), rect(cx - 9, cy - 6, 18, 13), 1);
                draw::ellipse(s, (190, 100, 120), rect(cx - 5, cy - 2, 4, 5), 0);
                draw::ellipse(s, (190, 100, 120), rect(cx + 1, cy - 2, 4, 5), 0);
            }
            _ => {}
        }
        let my = cy + 10;
        match mouth.as_str() {
            "big_grin" => {
                let px = 2;
                let pat = ["1..........1", "1..........1", "1..........1", "1..........1", "1..........1", "111111111111"];
                for (wr, row) in pat.iter().enumerate() {
                    for (wc, ch) in row.chars().enumerate() {
                        if ch == '1' { rr((10, 10, 15), cx - 13 * px / 2 + wc as i32 * px, my - 3 * px + wr as i32 * px, px, px, 0, 0); }
                    }
                }
            }
            "vampire_fangs" => {
                draw::line(s, (10, 10, 15), (cx - 8, my), (cx + 8, my), 2);
                draw::polygon(s, (255, 255, 250), &[(cx - 6, my), (cx - 2, my), (cx - 4, my + 7)], 0);
                draw::polygon(s, (255, 255, 250), &[(cx + 2, my), (cx + 6, my), (cx + 4, my + 7)], 0);
            }
            "nerd_teeth" => {
                draw::line(s, (10, 10, 15), (cx - 10, my), (cx + 10, my), 2);
                rr((255, 255, 250), cx - 9, my, 8, 11, 0, 2); rr((255, 255, 250), cx + 1, my, 8, 11, 0, 2);
                rr((200, 200, 195), cx - 9, my, 8, 11, 1, 2); rr((200, 200, 195), cx + 1, my, 8, 11, 1, 2);
            }
            "bird_beak" => {
                draw::polygon(s, (240, 170, 30), &[(cx - 12, my - 3), (cx + 13, my), (cx - 10, my + 4)], 0);
                draw::polygon(s, (200, 140, 15), &[(cx - 12, my + 2), (cx + 9, my + 4), (cx - 10, my + 9)], 0);
                draw::line(s, (150, 100, 5), (cx - 11, my + 1), (cx + 8, my + 3), 1);
            }
            _ => {}
        }
    }
}
