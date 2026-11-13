//! Das eigentliche Spiel: run_game() (Python 14459-15733).

use super::core::*;
use super::runtime::RuntimePowers;
use super::state::{Snd, G};
use crate::data::d;
use crate::game::save::DIFFICULTIES;
use crate::pg::*;
use crate::util::*;
use serde_json::json;
use std::collections::HashMap;

/// pygame.Rect-Zuweisung mit Float: Ergebnis wird Richtung 0 abgeschnitten
pub use jnd_core::rect::addf;

#[derive(Clone, Debug)]
pub struct Particle { pub x: f64, pub y: f64, pub vx: f64, pub vy: f64, pub life: f64, pub max_life: f64, pub trail: String, pub size: f64, pub angle: f64 }

fn wind_roll_phase() -> (f64, i64) {
    let roll = random();
    if roll < 0.3 { (0.0, randint(1200, 3000)) }
    else if roll < 0.7 { (uniform(0.25, 0.55) * choice(&[-1.0, 1.0]), randint(1800, 4000)) }
    else { (uniform(0.6, 1.0) * choice(&[-1.0, 1.0]), randint(1200, 2800)) }
}

fn trail_particle_count(t: &str) -> i32 {
    match t { "fire_trail" => 5, "ice_trail" => 4, "shadow_trail" => 3, "sparkle_trail" => 6, "toxic_trail" => 4, "electric_trail" => 5,
              "rainbow_trail" => 6, "star_trail" => 4, "smoke_trail" => 3, "blood_trail" => 4, "heart_trail" => 4, _ => 3 }
}
fn trail_size_mult(t: &str) -> f64 {
    match t { "fire_trail" => 1.2, "ice_trail" => 1.0, "shadow_trail" => 0.8, "sparkle_trail" => 1.5, "toxic_trail" => 1.1, "electric_trail" => 0.9,
              "rainbow_trail" => 1.3, "star_trail" => 1.2, "smoke_trail" => 1.4, "blood_trail" => 1.1, "heart_trail" => 1.3, _ => 1.0 }
}

/// neue Partikel für eine Spur (gleiche Zufallsbereiche wie in Python)
pub fn spawn_trail_particle(trail: &str, spawn_x: f64, vx: f64, cy: f64) -> Particle {
    let (vy, life) = match trail {
        "sparkle_trail" => (uniform(-2.0, 1.0), uniform(30.0, 60.0)),
        "shadow_trail" => (uniform(-0.5, 0.5), uniform(25.0, 45.0)),
        "ice_trail" => (uniform(-1.5, 0.5), uniform(35.0, 55.0)),
        "toxic_trail" => (uniform(-1.0, 0.0), uniform(40.0, 65.0)),
        "electric_trail" => (uniform(-1.5, 1.5), uniform(18.0, 32.0)),
        "rainbow_trail" => (uniform(-1.5, 0.5), uniform(35.0, 60.0)),
        "star_trail" => (uniform(-1.5, 0.5), uniform(35.0, 55.0)),
        "smoke_trail" => (uniform(-1.2, -0.2), uniform(40.0, 70.0)),
        "blood_trail" => (uniform(0.0, 1.5), uniform(18.0, 32.0)),
        "heart_trail" => (uniform(-1.8, -0.3), uniform(30.0, 55.0)),
        _ => (uniform(-1.0, 0.0), uniform(20.0, 40.0)),
    };
    Particle { x: spawn_x, y: cy + uniform(-8.0, 8.0), vx, vy, life, max_life: life, trail: trail.into(), size: uniform(1.5, 3.5), angle: uniform(0.0, 360.0) }
}

/// Physik-Update aller Partikel einer Liste (Python: "Update all particle types")
pub fn update_particles(ps: &mut Vec<Particle>) {
    ps.retain(|p| p.life > 0.0);
    for p in ps.iter_mut() {
        p.x += p.vx;
        p.y += p.vy;
        match p.trail.as_str() {
            "sparkle_trail" => { p.vy += 0.05; p.angle = (p.angle + 5.0) % 360.0; }
            "shadow_trail" => { p.vy += 0.02; }
            "ice_trail" => { p.vy += 0.08; p.vx *= 0.98; }
            "toxic_trail" => { p.vy += 0.06; p.vx *= 0.99; }
            "electric_trail" => { p.vx += uniform(-0.15, 0.15); p.vy += uniform(-0.15, 0.15); }
            "rainbow_trail" => { p.vy += 0.03; p.angle = (p.angle + 4.0) % 360.0; }
            "star_trail" => { p.vy += 0.04; p.angle = (p.angle + 3.0) % 360.0; }
            "smoke_trail" => { p.vy += 0.015; p.vx *= 0.985; p.size += 0.03; }
            "blood_trail" => { p.vy += 0.18; }
            "heart_trail" => { p.vy += 0.02; p.vx += (p.angle * 0.05).sin() * 0.05; }
            _ => { p.vy += 0.1; }
        }
        p.life -= 1.0;
    }
}

/// Zeichnet die Partikel einer Spur (Python: "Draw all trail particles")
pub fn draw_trail_particles(bs: &Surface, trail: &str, ps: &[Particle]) {
    let cc = |c: i32| c.clamp(0, 255);
    for p in ps {
        let progress = p.life / p.max_life;
        if progress <= 0.0 { continue; }
        let radius = 1.max((p.size * progress * trail_size_mult(trail)) as i32);
        if !super::art::legacy() {
            // Texturen: etwas größer als die alten Punkte, blenden mit dem Lebensende aus
            let size = (p.size * trail_size_mult(trail) * (0.5 + 0.5 * progress) * 2.3).max(3.0);
            let hue = if trail == "rainbow_trail" {
                let h = ((p.angle / 30.0).floor() * 30.0 / 360.0).rem_euclid(1.0);
                let (r, g, b) = hsv_to_rgb(h, 0.8, 1.0);
                Some(((r * 255.0) as i32, (g * 255.0) as i32, (b * 255.0) as i32))
            } else { None };
            let alpha = (progress as f32).clamp(0.0, 1.0);
            if super::art::draw_trail_tex(bs, trail, p.x, p.y, size, alpha, hue) { continue; }
        }
        let color: (i32, i32, i32) = match trail {
            "fire_trail" => {
                let b = cc(200 + (55.0 * progress) as i32);
                if progress > 0.8 { (b, b, 0) } else if progress > 0.5 { (b, cc((b as f64 * 0.65) as i32), 0) } else if progress > 0.2 { (b, cc((b as f64 * 0.3) as i32), 0) } else { (b, 0, 0) }
            }
            "ice_trail" => if progress > 0.7 { (220, 240, 255) } else if progress > 0.4 { (150, 200, 255) } else { (80, 160, 255) },
            "shadow_trail" => { let i = cc((120.0 * progress) as i32); (i, i, i) }
            "sparkle_trail" => {
                let tw = 0.8 + 0.4 * if ((p.angle * progress) as i64).rem_euclid(3) == 0 { 1.0 } else { 0.0 };
                let b = cc((255.0 * progress * tw) as i32);
                if progress > 0.6 { (b, b, cc((b as f64 * 0.8) as i32)) } else if progress > 0.3 { (b, cc((b as f64 * 0.9) as i32), cc((b as f64 * 0.3) as i32)) } else { (cc((b as f64 * 0.8) as i32), cc((b as f64 * 0.7) as i32), 0) }
            }
            "toxic_trail" => if progress > 0.7 { (150, 220, 100) } else if progress > 0.4 { (120, 180, 150) } else { (100, 80, 180) },
            "electric_trail" => if progress > 0.7 { (225, 250, 255) } else if progress > 0.4 { (120, 220, 255) } else { (40, 130, 220) },
            "rainbow_trail" => {
                let hue = (p.angle / 360.0).rem_euclid(1.0);
                let (r, g, b) = hsv_to_rgb(hue, 0.85, 0.55 + 0.45 * progress);
                (cc((r * 255.0) as i32), cc((g * 255.0) as i32), cc((b * 255.0) as i32))
            }
            "star_trail" => {
                let tw = 0.75 + 0.35 * if ((p.angle * progress) as i64).rem_euclid(3) == 0 { 1.0 } else { 0.0 };
                let b = cc((255.0 * progress * tw) as i32);
                if progress > 0.6 { (b, b, b) } else { (cc((b as f64 * 0.8) as i32), cc((b as f64 * 0.9) as i32), b) }
            }
            "smoke_trail" => { let g = cc((90.0 + 100.0 * progress) as i32); (g, g, cc(g + 5)) }
            "blood_trail" => if progress > 0.6 { (200, 20, 30) } else if progress > 0.3 { (150, 10, 20) } else { (80, 5, 10) },
            "heart_trail" => if progress > 0.6 { (255, 180, 200) } else if progress > 0.3 { (230, 40, 90) } else { (160, 10, 50) },
            _ => (255, 255, 255),
        };
        let (px, py) = (p.x as i32, p.y as i32);
        draw::circle(bs, color, (px, py), radius, 0);
        match trail {
            "ice_trail" if progress > 0.5 => {
                let gc = ((color.0 as f64 * 0.7) as i32, (color.1 as f64 * 0.7) as i32, (color.2 as f64 * 0.7) as i32);
                draw::circle(bs, gc, (px, py), radius + 1, 0);
            }
            "sparkle_trail" => {
                if radius > 2 {
                    for a in (0..360).step_by(45) {
                        let sx = (p.x + (radius + 2) as f64 * 0.7 * (1.0 - progress) * (a as f64 / 180.0)) as i32;
                        draw::circle(bs, (255, 255, 200), (sx, py), 1, 0);
                    }
                }
            }
            "toxic_trail" if progress > 0.3 => {
                let gc = ((color.0 as f64 * 0.5) as i32, (color.1 as f64 * 0.5) as i32, (color.2 as f64 * 0.5) as i32);
                draw::circle(bs, gc, (px, py), 2.max(radius + 1), 0);
            }
            "electric_trail" if progress > 0.4 => {
                let jx = (p.x + uniform(-4.0, 4.0)) as i32;
                let jy = (p.y + uniform(-4.0, 4.0)) as i32;
                draw::line(bs, (200, 240, 255), (px, py), (jx, jy), 1);
            }
            "rainbow_trail" if radius > 1 => { draw::circle(bs, (255, 255, 255), (px, py), 1.max(radius - 1), 1); }
            "star_trail" if radius > 1 => {
                for sa in [0.0, 90.0, 180.0, 270.0f64] {
                    let r = (sa + p.angle).to_radians();
                    let sx2 = (p.x + r.cos() * (radius + 3) as f64 * progress) as i32;
                    let sy2 = (p.y + r.sin() * (radius + 3) as f64 * progress) as i32;
                    draw::line(bs, (255, 255, 255), (px, py), (sx2, sy2), 1);
                }
            }
            "smoke_trail" if progress > 0.2 => { draw::circle(bs, (cc(color.0 + 15), cc(color.1 + 15), cc(color.2 + 15)), (px, py), radius + 2, 1); }
            "blood_trail" if progress > 0.4 => {
                let tx2 = (p.x - p.vx * 2.0) as i32;
                let ty2 = (p.y - p.vy * 2.0) as i32;
                draw::line(bs, color, (px, py), (tx2, ty2), 1.max(radius - 1));
            }
            "heart_trail" if radius > 1 => { draw::circle(bs, (255, 220, 230), (px, py), 1.max(radius - 2), 0); }
            _ => {}
        }
    }
}

impl G {
    pub fn equipped_trails(&self) -> Vec<String> {
        let owned = self.cosmetics_state.gstrs("owned");
        let eq = self.cosmetics_state.gstrs("equipped");
        all_trail_keys().into_iter().filter(|t| owned.contains(t) && eq.contains(t)).collect()
    }

    /// HUD-Text mit Schatten
    pub fn hud_text(&self, line: &str, x: i32, y: i32) {
        let tc = if self.is_dark_mode() { (230, 235, 255) } else { (20, 20, 50) };
        let sc = if self.is_dark_mode() { (0, 0, 0) } else { (180, 185, 210) };
        let bs = self.base_surface.clone();
        bs.blit(&self.small_font.render(line, true, sc), (x + 1, y + 1));
        bs.blit(&self.small_font.render(line, true, tc), (x, y));
    }

    /// Rote Herzschlag-Vignette (oben/links/rechts) wenn nur noch 1 Leben
    pub fn draw_almost_dead_vignette(&self, pulse: f64) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let layers = 30;
        let vig_top = h / 3;
        let vig_side = w / 5;
        let top_s = Surface::new_alpha(w, vig_top);
        for li in 0..layers {
            let frac = (layers - li) as f64 / layers as f64;
            let a = (90.0 * pulse * frac.powf(1.6)) as i32;
            let sh = 1.max(vig_top / layers + 2);
            draw::rect(&top_s, (210, 0, 0, a), Rect::new(0, li * (vig_top / layers), w, sh), 0, 0);
        }
        bs.blit(&top_s, (0, 0));
        let left_s = Surface::new_alpha(vig_side, h);
        for li in 0..layers {
            let frac = (layers - li) as f64 / layers as f64;
            let a = (90.0 * pulse * frac.powf(1.6)) as i32;
            let sw = 1.max(vig_side / layers + 2);
            draw::rect(&left_s, (210, 0, 0, a), Rect::new(li * (vig_side / layers), 0, sw, h), 0, 0);
        }
        bs.blit(&left_s, (0, 0));
        let right_s = transform::flip(&left_s, true, false);
        bs.blit(&right_s, (w - vig_side, 0));
    }

    /// Ende einer Runde (in Python zweimal identisch im Kollisionscode)
    fn end_round_standard(&mut self, score: i64, difficulty: &str, round_start: f64, rp: &RuntimePowers) {
        let round_duration = now_secs() - round_start;
        let mut gained = self.compute_coins(score, difficulty, round_duration);
        if rp.equipped.iter().any(|e| e == "Recovery") && ["Schwer", "Hardcore", "Unmöglich", "Dynamisch"].contains(&difficulty) {
            let req = if difficulty == "Schwer" { 15.0 } else { 10.0 };
            if round_duration >= req { gained += 3; }
        }
        let round_seconds = round_duration as i64;
        if self._round_modded {
            // Mod-Runde: eigene Statistik/Highscores, keine echten Coins, nicht online (wie in Python)
            let mut ms = self.modded_stats.clone();
            ms.insert("total_time_played".into(), jf(ms.gf("total_time_played", 0.0) + round_duration));
            ms.insert("total_rounds".into(), json!(ms.gi("total_rounds", 0) + 1));
            ms.insert("coins".into(), json!(ms.gi("coins", 0) + gained));
            self.modded_stats = ms.clone();
            self.save_modded_section("modded_stats", serde_json::Value::Object(ms));
            let old = self.modded_scores.gi(difficulty, 0);
            let new_record = score > old;
            self.modded_scores.insert(difficulty.into(), json!(old.max(score)));
            let ss = self.modded_session_scores.gi(difficulty, 0).max(score);
            self.modded_session_scores.insert(difficulty.into(), json!(ss));
            self.save_modded_section("modded_highscores", serde_json::Value::Object(self.modded_scores.clone()));
            let bt = self.modded_best_times.gi(difficulty, 0).max(round_seconds);
            self.modded_best_times.insert(difficulty.into(), json!(bt));
            let st = self.modded_session_times.gi(difficulty, 0).max(round_seconds);
            self.modded_session_times.insert(difficulty.into(), json!(st));
            self.save_modded_section("modded_best_times", serde_json::Value::Object(self.modded_best_times.clone()));
            if let Some(mut r) = self._last_recording.take() { r.score = score; self.save_recording(&r, false); }
            self.stop_music();
            self.show_game_over(score, gained, old, new_record, round_seconds);
            return;
        }
        let s = &mut self.stats;
        let ttp = s.gf("total_time_played", 0.0) + round_duration;
        s.insert("total_time_played".into(), jf(ttp));
        let tr = s.gi("total_rounds", 0) + 1;
        s.insert("total_rounds".into(), json!(tr));
        let c = s.gi("coins", 0) + gained;
        s.insert("coins".into(), json!(c));
        let td = s.gi("total_deaths", 0) + 1;
        s.insert("total_deaths".into(), json!(td));
        let mut ssd = s.gobj("score_sum_by_diff");
        let v = ssd.gi(difficulty, 0) + score;
        ssd.insert(difficulty.into(), json!(v));
        s.insert("score_sum_by_diff".into(), serde_json::Value::Object(ssd));
        let mut rbd = s.gobj("rounds_by_diff");
        let v = rbd.gi(difficulty, 0) + 1;
        rbd.insert(difficulty.into(), json!(v));
        s.insert("rounds_by_diff".into(), serde_json::Value::Object(rbd));
        let tce = s.gi("total_coins_earned", 0) + gained;
        s.insert("total_coins_earned".into(), json!(tce));
        self.save_section("stats", None);
        let old = self.saved_scores.gi(difficulty, 0);
        let new_record = score > old;
        if let Some(mut r) = self._last_recording.take() {
            r.score = score;
            let best = new_record && difficulty != "Custom" && !self.klassik_mode;
            self.save_recording(&r, best);
        }
        self.saved_scores.insert(difficulty.into(), json!(old.max(score)));
        let ss = self.session_scores.gi(difficulty, 0).max(score);
        self.session_scores.insert(difficulty.into(), json!(ss));
        self.save_section("highscores", None);
        if !self.klassik_mode { self.submit_score_online("standard", difficulty, score); }
        let bt = self.best_times.gi(difficulty, 0).max(round_seconds);
        self.best_times.insert(difficulty.into(), json!(bt));
        let st = self.session_times.gi(difficulty, 0).max(round_seconds);
        self.session_times.insert(difficulty.into(), json!(st));
        self.save_section("best_times", None);
        self.stop_music();
        self.show_game_over(score, gained, old, new_record, round_seconds);
    }

    pub fn run_game(&mut self) {
        let mut clock = Clock::new();
        let mod_mode = if self.current_game_mode == "mod" { self._mod_mode.clone() } else { None };
        let (difficulty, mut speed, mut min_delay, mut max_delay): (String, f64, i64, i64) = if let Some(m) = &mod_mode {
            (format!("Mod: {}", m.name), m.speed, m.min_delay, m.max_delay)
        } else if self.current_game_mode == "custom" {
            ("Custom".into(), self.settings.gf("custom_speed", 10.0), self.settings.gi("custom_min_delay", 900), self.settings.gi("custom_max_delay", 1600))
        } else {
            let dd = DIFFICULTIES[self.difficulty_index];
            let (a, b) = difficulty_obstacle_interval(dd);
            (dd.into(), difficulty_speed(dd) as f64, a, b)
        };
        self.play_ingame_music();
        let dynamic_mode = difficulty == "Dynamisch";
        let mut dynamic_timer = 0i64;
        let (w0, h0) = (self.WIDTH, self.HEIGHT);
        let mut player = Rect::new(50, h0 - 50, 50, 50);
        let mut vy: f64 = 0.0;
        let mut trails: HashMap<String, Vec<Particle>> = all_trail_keys().into_iter().map(|k| (k, vec![])).collect();
        // Lua-Mods: Spielwerte, eigene Hindernisse, Ereignisse (Runden zählen getrennt)
        let modded = self.mods.any_active() || mod_mode.is_some();
        self._round_modded = modded;
        let modm = if modded { self.mods.modifier() } else { super::mods::ModModifier::default() };
        let mod_obs = if modded { self.mods.obstacles() } else { vec![] };
        let mod_w_sum: f64 = mod_obs.iter().map(|o| o.weight).sum();
        speed *= modm.speed;
        let gravity = 1.0 * modm.gravity;
        if modded { let (w0m, h0m, gm) = (self.WIDTH, self.HEIGHT, self.current_game_mode.clone()); self.mods.on_round_start(&difficulty, w0m, h0m, &gm); }
        let mut is_jumping = false;
        let mut used_double_jump;
        let move_speed = 8;
        let mut heartbeat_timer = 0i64;
        let heartbeat_interval = 850.0;
        let mut almost_dead_pulse = 0.0f64;
        let mut obstacles: Vec<Obstacle> = vec![];
        let mut top_obstacles: Vec<Obstacle> = vec![];
        let mut powerups: Vec<Obstacle> = vec![];
        let mut obstacle_timer = 0i64;
        let mut obstacle_delay = randint(min_delay, max_delay);
        let mut top_timer = 0i64;
        let mut top_delay = (randint(900, 1600) as f64 * (800.0 / 400.max(w0) as f64)) as i64;
        let mut score: i64 = 0;
        let mut lives: i64 = (mod_mode.as_ref().map(|m| m.lives).unwrap_or(3) + modm.lives).max(1);
        let mut oneups_collected = 0;
        let mut hit_invuln: i64 = 0;
        let custom = self.current_game_mode == "custom";
        let wind_h = custom && (self.settings.gb("custom_wind_sides", false) || self.settings.gb("custom_wind_all", false));
        let wind_v = custom && self.settings.gb("custom_wind_all", false);
        let free_roam = custom && self.settings.gb("custom_free_roam", false);
        let free_speed = 10;
        let (mut wh, mut wh_t, mut wh_timer) = (0.0f64, 0.0f64, 0i64);
        let (mut wv, mut wv_t, mut wv_timer) = (0.0f64, 0.0f64, 0i64);
        let mut wind_particles: Vec<[f64; 3]> = if wind_h || wind_v {
            (0..26).map(|_| [randint(0, w0 as i64) as f64, randint(0, h0 as i64) as f64, randint(30, 70) as f64]).collect()
        } else { vec![] };
        let mut round_start = now_secs();
        let max_slots = self.get_max_slots();
        let equipped: Vec<String> = self.powers_state.gstrs("equipped").into_iter().take(max_slots).collect();
        let mut rp = RuntimePowers::new(&equipped);
        let mut space_was_down = false;
        used_double_jump = false;
        let mut last_frame = self.base_surface.copy();
        // Feste Logikrate: die Spielregeln laufen immer mit 60 Schritten/s (wie das pygame-Original),
        // gezeichnet wird so oft wie der Monitor kann - dazwischen wird interpoliert.
        let mut acc_ms = LOGIC_TICK_MS;
        let mut tick_no: u64 = 0;
        let mut jump_latch = false;
        let mut focus_pause = false;
        let mut prev_player = player;
        let (mut floor_scroll, mut prev_floor_scroll) = (0.0f64, 0.0f64);
        let mut land_ms = 1000.0f64;
        let mut fx = super::art::Fx::new();
        let fx_on = self.effects_on();
        // Aufzeichnung (Replay) und Geist des Rekordlaufs
        let mut rec = super::replay::Recording::new(w0, h0, self.get_player_color(), &difficulty);
        let ghost = if self.settings.gb("ghost", true) && difficulty != "Custom" && !self.klassik_mode && !free_roam { self.load_best_recording(&difficulty) } else { None };
        let dark_round = self.is_dark_mode();
        loop {
            self.sync_window_size();
            if !self.game_running { self.stop_music(); return; }
            let fps = self.target_fps();
            acc_ms += clock.tick_f(fps).min(250.0);
            self.fill_bg(true);
            let bs = self.base_surface.clone();
            let (w, h) = (self.WIDTH, self.HEIGHT);
            if self.settings.gb("show_floor_line", true) && !free_roam {
                let fs = prev_floor_scroll + (floor_scroll - prev_floor_scroll) * (acc_ms / LOGIC_TICK_MS).clamp(0.0, 1.0);
                let dark = self.is_dark_mode();
                super::art::draw_floor(&bs, w, h, h - 48, fs, dark);
            }
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self.action_matches_key(key, "jump") { jump_latch = true; }
                        if self.action_matches_key(key, "pause") || focus_pause {
                            focus_pause = false;
                            let pt = now_secs();
                            let res = self._run_pause_menu(Some(last_frame.clone()));
                            round_start += now_secs() - pt;
                            clock.tick(0);
                            acc_ms = 0.0;
                            if res == "menu" { self.stop_music(); self.game_running = false; self.in_menu = true; return; }
                            else if res == "restart" { self.stop_music(); return; }
                        } else {
                            let binds = rp.active_bindings();
                            if !binds.is_empty() {
                                if self.action_matches_key(key, "power1") && !binds.is_empty() { let b = binds[0].clone(); self.rp_activate(&mut rp, &b); }
                                else if self.action_matches_key(key, "power2") && binds.len() >= 2 { let b = binds[1].clone(); self.rp_activate(&mut rp, &b); }
                            }
                        }
                    }
                    // Fenster verliert den Fokus (Alt+Tab, Benachrichtigung ...): automatisch pausieren
                    Event::FocusLost if self.settings.gb("pause_on_focus_loss", true) && !self.display.hidden => {
                        focus_pause = true;
                        self.display.event_post(Event::KeyDown { key: 0, unicode: String::new(), mods: 0 });
                    }
                    _ => {}
                }
            }
            let mut steps = 0;
            while acc_ms >= LOGIC_TICK_MS {
                acc_ms -= LOGIC_TICK_MS;
                steps += 1;
                if steps > 8 { acc_ms = 0.0; }
                let dt = logic_dt(tick_no);
                tick_no += 1;
                prev_player = player;
                prev_floor_scroll = floor_scroll;
                land_ms += dt as f64;
                for o in obstacles.iter_mut().chain(top_obstacles.iter_mut()).chain(powerups.iter_mut()) { o.prev = Some((o.r.x, o.r.y)); }
                for (act, arg) in self._drain_dev_round_actions() {
                    match act.as_str() {
                        "collect_1up" => { let n = arg.unwrap_or(1); lives += n; oneups_collected += n; let t = self.stats.gi("total_1ups", 0) + n; self.stats.insert("total_1ups".into(), json!(t)); }
                        "add_score" => score += arg.unwrap_or(0),
                        "add_lives" => lives += arg.unwrap_or(0),
                        "set_lives" => { if let Some(a) = arg { lives = a; } }
                        "god_mode" => hit_invuln = if arg.unwrap_or(0) != 0 { 999_999_999 } else { 0 },
                        _ => {}
                    }
                }
                if self.flash_timer > 0.0 { self.flash_timer -= dt as f64; }
                let left_down = self.any_action_pressed("left");
                let right_down = self.any_action_pressed("right");
                if !free_roam {
                    let space_down = self.any_action_pressed("jump") || jump_latch;
                    jump_latch = false;
                    if space_down && !space_was_down {
                        if !is_jumping {
                            vy = -20.0 * modm.jump;
                            is_jumping = true;
                            used_double_jump = false;
                            let t = self.stats.gi("total_jumps", 0) + 1;
                            self.stats.insert("total_jumps".into(), json!(t));
                        } else if rp.double_jump && !used_double_jump {
                            vy = -18.0 * modm.jump;
                            used_double_jump = true;
                            let t = self.stats.gi("total_jumps", 0) + 1;
                            self.stats.insert("total_jumps".into(), json!(t));
                            let txt = format!("{}!", self.get_power_name("Double Jump"));
                            self.push_overlay(&txt, (120, 200, 255), 1.0, Some((player.centerx(), player.top() - 20)));
                        }
                    }
                    space_was_down = space_down;
                }
                if rp.freeze_remaining == 0.0 {
                    if wind_h {
                        wh_timer -= dt;
                        if wh_timer <= 0 { let (t, d) = wind_roll_phase(); wh_t = t; wh_timer = d; }
                        wh += (wh_t - wh) * 0.02;
                    }
                    if wind_v {
                        wv_timer -= dt;
                        if wv_timer <= 0 { let (t, d) = wind_roll_phase(); wv_t = t; wv_timer = d; }
                        wv += (wv_t - wv) * 0.02;
                    }
                }
                if free_roam {
                    if self.display.key_pressed(K_w) { player.y -= free_speed; }
                    if self.display.key_pressed(K_s) { player.y += free_speed; }
                    if self.display.key_pressed(K_a) { player.x -= free_speed; }
                    if self.display.key_pressed(K_d) { player.x += free_speed; }
                    if wind_h && rp.freeze_remaining == 0.0 { player.x = addf(player.x, wh * 4.5); }
                    if wind_v && rp.freeze_remaining == 0.0 { player.y = addf(player.y, wv * 3.5); }
                    if player.top() < 0 { player.set_top(0); }
                    if player.bottom() > h { player.set_bottom(h); }
                } else {
                    if left_down { player.x -= move_speed; }
                    if right_down { player.x += move_speed; }
                    if wind_h && rp.freeze_remaining == 0.0 { player.x = addf(player.x, wh * 4.5); }
                }
                if player.left() < 0 { player.set_left(0); }
                if player.right() > w { player.set_right(w); }

                let eq_trails = self.equipped_trails();
                if !eq_trails.is_empty() && (left_down || right_down || vy < 0.0) {
                    for t in &eq_trails {
                        let list = trails.entry(t.clone()).or_default();
                        for _ in 0..trail_particle_count(t) {
                            let (sx, vx) = if left_down { (player.right() as f64 + uniform(-5.0, 5.0), uniform(-3.0, -0.5)) }
                                else if right_down { (player.left() as f64 + uniform(-5.0, 5.0), uniform(0.5, 3.0)) }
                                else { (player.centerx() as f64 + uniform(-8.0, 8.0), uniform(-2.0, 2.0)) };
                            list.push(spawn_trail_particle(t, sx, vx, player.centery() as f64));
                        }
                    }
                }
                for list in trails.values_mut() { update_particles(list); }

                if !free_roam {
                    vy += gravity;
                    if wind_v && rp.freeze_remaining == 0.0 { vy += wv * 0.6; }
                    player.y = addf(player.y, vy);
                    if player.top() < 0 { player.set_top(0); vy = 0.0; }
                    if player.bottom() >= h - 50 {
                        player.set_bottom(h - 50);
                        vy = 0.0;
                        if is_jumping || used_double_jump {
                            is_jumping = false; used_double_jump = false; land_ms = 0.0;
                            if fx_on { fx.dust(player.centerx() as f64, player.bottom() as f64, dark_round); }
                        }
                        space_was_down = false;
                    }
                }

                if rp.freeze_remaining == 0.0 { obstacle_timer += dt; }
                if obstacle_timer >= obstacle_delay {
                    obstacle_timer = 0;
                    let mult = rp.speed_multiplier();
                    obstacle_delay = if mult > 0.0 { (randint(min_delay, max_delay) as f64 / mult) as i64 } else { 999_999 };
                    let mut chance = 0.015 * if rp.equipped.iter().any(|e| e == "Medic") { 1.15 } else { 1.0 };
                    if difficulty == "Custom" && !self.settings.gb("custom_oneups", true) { chance = 0.0; }
                    if random() < chance { powerups.push(Obstacle::new(w, h - 100, 50, 50, "1up")); }
                    else if free_roam { obstacles.push(self.make_free_roam_obstacle()); }
                    else if mod_w_sum > 0.0 && random() * (100.0 + mod_w_sum) >= 100.0 {
                        // eigenes Hindernis einer Lua-Mod (nach Gewicht)
                        let mut r = random() * mod_w_sum;
                        let pick = mod_obs.iter().find(|o| { r -= o.weight; r < 0.0 }).or(mod_obs.last());
                        if let Some(o) = pick { if o.top { top_obstacles.push(Obstacle::from_mod(o, w, h)); } else { obstacles.push(Obstacle::from_mod(o, w, h)); } }
                    } else {
                        let t = self.pick_obstacle_type(&difficulty, if dynamic_mode { Some(speed) } else { None });
                        obstacles.extend(self.make_obstacle(w, &t));
                    }
                }
                let top_active = !self.klassik_mode && !free_roam && (["Hardcore", "Unmöglich", "Albtraum", "Hölle"].contains(&difficulty.as_str())
                    || mod_mode.as_ref().map(|m| m.top).unwrap_or(false)
                    || (dynamic_mode && speed >= 14.0) || (difficulty == "Custom" && self.settings.gb("custom_vertical_obstacles", false)));
                if top_active {
                    if rp.freeze_remaining == 0.0 { top_timer += dt; }
                    if top_timer >= top_delay {
                        top_timer = 0;
                        top_obstacles.push(self.make_top_obstacle());
                        top_delay = (randint(900, 1600) as f64 * (800.0 / 400.max(w) as f64)) as i64;
                    }
                }
                let current_speed = speed * rp.speed_multiplier();
            floor_scroll += current_speed;
                let fall_speed = (current_speed * 0.85).max(2.0);
                let mut i = 0;
                while i < obstacles.len() {
                    let o = &mut obstacles[i];
                    let fr = o.dir_x.is_some() || o.dir_y.is_some();
                    if fr {
                        o.r.x = addf(o.r.x, o.dir_x.unwrap_or(0.0) * current_speed);
                        o.r.y = addf(o.r.y, o.dir_y.unwrap_or(0.0) * current_speed);
                        if rp.freeze_remaining == 0.0 {
                            if wind_h { o.r.x = addf(o.r.x, wh * 5.0); }
                            if wind_v { o.r.y = addf(o.r.y, wv * 4.5); }
                        }
                    } else {
                        o.r.x = addf(o.r.x, -current_speed * o.spd);
                        if wind_h && rp.freeze_remaining == 0.0 { o.r.x = addf(o.r.x, wh * 5.0); }
                    }
                    let off = if fr { o.r.right() < 0 || o.r.left() > w || o.r.bottom() < 0 || o.r.top() > h } else { o.r.right() < 0 };
                    if off { obstacles.remove(i); score += 1; continue; }
                    i += 1;
                }
                if rp.freeze_remaining == 0.0 {
                    let mut i = 0;
                    while i < top_obstacles.len() {
                        let o = &mut top_obstacles[i];
                        o.r.y = addf(o.r.y, fall_speed * o.spd);
                        if wind_v { o.r.y = addf(o.r.y, wv * 4.5); }
                        if wind_h { o.r.x = addf(o.r.x, wh * 5.0); }
                        if o.r.top() > h { top_obstacles.remove(i); continue; }
                        i += 1;
                    }
                }
                if rp.nuke_remaining > 0.0 {
                    let (pcx, pcy) = player.center();
                    let near = |o: &Obstacle| { let dx = (o.r.centerx() - pcx) as i64; let dy = (o.r.centery() - pcy) as i64; dx * dx + dy * dy < 40000 };
                    obstacles.retain(|o| !near(o));
                    top_obstacles.retain(|o| !near(o));
                }
                let mut i = 0;
                while i < powerups.len() {
                    let p = &mut powerups[i].r;
                    p.x = addf(p.x, -current_speed);
                    if rp.magnet {
                        let dx = (player.centerx() - p.centerx()) as f64;
                        let dy = (player.centery() - p.centery()) as f64;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist > 0.0 && dist < 200.0 {
                            let pull = 7.0 * (1.0 - dist / 200.0);
                            p.x += (dx / dist * pull) as i32;
                            p.y += (dy / dist * pull) as i32;
                        }
                    }
                    if p.right() < 0 { powerups.remove(i); continue; }
                    i += 1;
                }
                let mut i = 0;
                while i < powerups.len() {
                    if player.colliderect(&powerups[i].r) {
                        lives += 1;
                        oneups_collected += 1;
                        let t = self.stats.gi("total_1ups", 0) + 1;
                        self.stats.insert("total_1ups".into(), json!(t));
                        if fx_on { let pr = powerups[i].r; fx.sparkle(pr.centerx() as f64, pr.centery() as f64, super::art::ONEUP, 18); }
                        powerups.remove(i);
                        self.play_sound(Snd::OneUp);
                        self.show_1up_text_overlay(Some(player));
                        self.push_overlay("1-UP eingesammelt", super::art::darken(super::art::ONEUP, 0.1), 1.5, Some((player.centerx(), player.top() - 20)));
                        continue;
                    }
                    i += 1;
                }
                // Kollision (Boden- und dann Top-Hindernisse)
                for which in 0..2 {
                    let mut i = 0;
                    loop {
                        let list = if which == 0 { &mut obstacles } else { &mut top_obstacles };
                        if i >= list.len() { break; }
                        if !player.colliderect(&list[i].r) { i += 1; continue; }
                        if rp.freeze_remaining > 0.0 || hit_invuln > 0 { i += 1; continue; }
                        if rp.on_hit_block() {
                            let t = self.get_text("protected");
                            self.push_overlay(&t, PURPLE, 1.5, Some((player.centerx(), player.top() - 20)));
                        } else if rp.auto_dodge && random() < 0.05 {
                            self.push_overlay("Auto Dodge!", (0, 200, 0), 1.5, Some((player.centerx(), player.top() - 20)));
                        } else {
                            lives -= 1;
                            hit_invuln = 750;
                            self.play_sound_v(Snd::Hit, "hit_volume");
                            if fx_on { fx.burst(player.centerx() as f64, player.centery() as f64, super::art::HAZARD, 22); }
                            self.shake(7.0);
                            if modded { self.mods.on_hit(score, lives); }
                        }
                        let list = if which == 0 { &mut obstacles } else { &mut top_obstacles };
                        list.remove(i);
                        if lives <= 0 {
                            if rp.phoenix && rp.phoenix_cd == 0.0 {
                                lives = 2;
                                rp.phoenix_cd = 120000.0;
                                rp.phoenix_invuln_remaining = 2000.0;
                                rp.phoenix_slow_remaining = 6000.0;
                                self.flash_timer = 1000.0;
                                self.push_overlay("Phoenix Revival!", (150, 50, 200), 2.0, None);
                            } else {
                                self._last_recording = Some(std::mem::replace(&mut rec, super::replay::Recording::new(0, 0, (0, 0, 0), "")));
                                self.end_round_standard(score, &difficulty, round_start, &rp);
                                return;
                            }
                        }
                    }
                }
                if dynamic_mode {
                    dynamic_timer += dt;
                    if dynamic_timer >= 5000 {
                        dynamic_timer = 0;
                        speed += 0.5;
                        min_delay = 20.max(min_delay - 50);
                        max_delay = (min_delay + 20).max(max_delay - 100);
                    }
                }
                rp.update_cooldowns(dt as f64);
                fx.update(dt as f64);
                if modded {
                    let res = self.mods.on_tick(tick_no, score, lives, player.x, player.y, w, h);
                    for id in res.spawns {
                        if let Some(o) = mod_obs.iter().find(|o| o.id == id) { if o.top { top_obstacles.push(Obstacle::from_mod(o, w, h)); } else { obstacles.push(Obstacle::from_mod(o, w, h)); } }
                    }
                }
                rec.push(player, hit_invuln == 0 || (tick_no / 5) % 2 == 0, score, lives,
                    obstacles.iter().map(|o| (0u8, o.r)).chain(top_obstacles.iter().map(|o| (1u8, o.r))).chain(powerups.iter().map(|o| (2u8, o.r))));
                rp.tick_shield_rearm();
                if hit_invuln > 0 { hit_invuln = 0.max(hit_invuln - dt); }

                if (wind_h || wind_v) && rp.freeze_remaining == 0.0 {
                    for wp in wind_particles.iter_mut() {
                        wp[0] += wh * 16.0;
                        wp[1] += wv * 16.0;
                        if wp[0] < -60.0 || wp[0] > (w + 60) as f64 || wp[1] < -60.0 || wp[1] > (h + 60) as f64 {
                            wp[0] = randint(0, w as i64) as f64;
                            wp[1] = randint(0, h as i64) as f64;
                            wp[2] = randint(30, 70) as f64;
                        }
                    }
                }
                if lives == 1 {
                    almost_dead_pulse += dt as f64;
                    heartbeat_timer += dt;
                    if heartbeat_timer as f64 >= heartbeat_interval { heartbeat_timer = 0; self.play_sound_v(Snd::Heartbeat, "sfx_volume"); }
                } else {
                    almost_dead_pulse = 0.0;
                    heartbeat_timer = 0;
                }
            }
            let alpha = (acc_ms / LOGIC_TICK_MS).clamp(0.0, 1.0);
            // ---- Zeichnen ----
            let show_player = hit_invuln == 0 || (self.get_ticks() / 80) % 2 == 0;
            let tsec = self.get_ticks() as f64 / 1000.0;
            let dplayer = super::art::squash(lerp_rect(Some((prev_player.x, prev_player.y)), player, alpha), if free_roam { 0.0 } else { vy }, if free_roam { 1000.0 } else { land_ms });
            for p in powerups.iter().map(|p| p.draw_rect(alpha)) {
                super::art::draw_oneup(&bs, p, tsec, &self.badge_font);
            }
            for o in &obstacles { super::art::draw_hazard_c(&bs, o.draw_rect(alpha), o.tint.unwrap_or(super::art::HAZARD)); }
            for o in &top_obstacles {
                let r = o.draw_rect(alpha);
                super::art::draw_faller_c(&bs, r, o.tint.unwrap_or(super::art::FALLER));
                super::art::draw_faller_marker(&bs, r, tsec);
            }
            if let Some(g) = &ghost {
                // Geist: Position aus dem Rekordlauf, Höhe relativ zum Boden (Fenstergröße darf abweichen)
                let gi = tick_no as usize;
                if let (Some(a), Some(b)) = (g.frames.get(gi.saturating_sub(1)), g.frames.get(gi)) {
                    let gx = a.px as f64 + (b.px - a.px) as f64 * alpha;
                    let gy = a.py as f64 + (b.py - a.py) as f64 * alpha;
                    let gy = (h - 50) as f64 - ((g.h - 50) as f64 - gy);
                    let gr = Rect::new(gx.round() as i32, gy.round() as i32, 50, 50);
                    super::art::draw_ghost(&bs, gr, g.color);
                    let lbl = self.badge_font.render(&self.tde("Rekord", "Best"), true, (190, 190, 215));
                    bs.blit(&lbl, (gr.centerx() - lbl.get_width() / 2, gr.top() - lbl.get_height() - 2));
                }
            }
            fx.draw(&bs);
            if show_player { super::art::draw_player(&bs, dplayer, self.get_player_color()); }
            self.draw_player_cosmetics(&bs, dplayer, None);
            if wind_h || wind_v {
                let mag = wh.abs().max(wv.abs());
                if mag > 0.04 {
                    let a = 200.min((mag * 220.0) as i32);
                    let dist = (wh * wh + wv * wv).sqrt().max(0.001);
                    let (ux, uy) = (wh / dist, wv / dist);
                    let layer = Surface::new_alpha(w, h);
                    for wp in &wind_particles {
                        draw::line(&layer, (220, 240, 255, a), (wp[0] - ux * wp[2], wp[1] - uy * wp[2]), (wp[0], wp[1]), 2);
                    }
                    bs.blit(&layer, (0, 0));
                }
            }
            let lh = 22;
            let (tx, ty) = (12, 10);
            let elapsed = (now_secs() - round_start) as i64;
            let hs = if modded { self.modded_scores.gi(&difficulty, 0) } else { self.saved_scores.gi(&difficulty, 0) };
            let coins = self.stats.gi("coins", 0);
            let lines = [format!("{}: {}", self.get_text("score"), score), format!("{}: {}", self.get_text("lives"), lives),
                         format!("{}: {}", self.get_text("highscore"), hs), format!("{}: {}", self.get_text("coins"), coins)];
            for (k, l) in lines.iter().enumerate() { self.hud_text(l, tx, ty + k as i32 * lh); }
            let sw = self.small_font.size(&lines[0]).0;
            self.hud_text(&format!("{}s", elapsed), tx + sw + 14, ty);
            if wind_h || wind_v {
                let hd = if wh > 0.05 { 1.0 } else if wh < -0.05 { -1.0 } else { 0.0 };
                let vd = if wv > 0.05 { 1.0 } else if wv < -0.05 { -1.0 } else { 0.0 };
                let pct = (wh.abs().max(wv.abs()) * 100.0) as i64;
                let wl = format!("Wind: {}%", pct);
                let wy = ty + 4 * lh;
                self.hud_text(&wl, tx, wy);
                let tc = if self.is_dark_mode() { (230, 235, 255) } else { (20, 20, 50) };
                let acx = (tx + self.small_font.size(&wl).0 + 16) as f64;
                let acy = (wy + self.small_font.get_height() / 2) as f64;
                if hd == 0.0 && vd == 0.0 { draw::circle(&bs, tc, (acx, acy), 3, 0); } else {
                    let al = ((hd * hd + vd * vd) as f64).sqrt().max(0.001);
                    let (aux, auy) = (hd / al, vd / al);
                    let asz = 9.0;
                    let tip = (acx + aux * asz, acy + auy * asz);
                    let tail = (acx - aux * asz, acy - auy * asz);
                    let perp = (-auy, aux);
                    let w1 = (tip.0 - aux * asz * 0.7 + perp.0 * asz * 0.5, tip.1 - auy * asz * 0.7 + perp.1 * asz * 0.5);
                    let w2 = (tip.0 - aux * asz * 0.7 - perp.0 * asz * 0.5, tip.1 - auy * asz * 0.7 - perp.1 * asz * 0.5);
                    draw::line(&bs, tc, tail, tip, 2);
                    draw::polygon(&bs, tc, &[tip, w1, w2], 0);
                }
            }
            let mut pys = 10;
            for name in rp.equipped.clone() {
                let rar = d("POWERS_DB").gv(&name).map(|p| p.gs("rarity", "?")).unwrap_or_else(|| "?".into());
                let color = if ["Common", "Rare", "Epic", "Legendary", "Mythic"].contains(&rar.as_str()) { rarity_color(&rar) } else { GRAY };
                let ix = w - 220;
                draw::rect(&bs, color, Rect::new(ix, pys + 2, 16, 16), 0, 3);
                let mut label = self.get_power_name(&name);
                let ms = |v: f64| (v as i64).div_euclid(1000);
                if rp.timer(&name) > 0.0 { label += &format!(" ({}s)", ms(rp.timer(&name))); }
                if name == "Slow Motion" && rp.slow_remaining > 0.0 { label += &format!(" [{}s]", ms(rp.slow_remaining)); }
                if name == "Time Freeze" {
                    if rp.freeze_remaining > 0.0 { label += &format!(" [Freeze {}s]", ms(rp.freeze_remaining)); }
                    else if rp.freeze_slow_remaining > 0.0 { label += &format!(" [Slow {}s]", ms(rp.freeze_slow_remaining)); }
                }
                if name == "Ghost Mode" && rp.ghost_remaining > 0.0 { label += &format!(" [{}s]", ms(rp.ghost_remaining)); }
                if name == "Nuke" && rp.nuke_remaining > 0.0 { label += &format!(" [{}s]", ms(rp.nuke_remaining)); }
                if name == "Phoenix" {
                    if rp.phoenix_invuln_remaining > 0.0 { label += &format!(" [Invuln {}s]", ms(rp.phoenix_invuln_remaining)); }
                    else if rp.phoenix_slow_remaining > 0.0 { label += &format!(" [Slow {}s]", ms(rp.phoenix_slow_remaining)); }
                    else if rp.phoenix_cd > 0.0 { label += &format!(" [{}:{:02}]", (rp.phoenix_cd as i64) / 60000, ms(rp.phoenix_cd) % 60); }
                }
                self.hud_text(&label, ix + 16 + 6, pys);
                pys += lh;
            }
            let binds = rp.active_bindings();
            if !binds.is_empty() { self.hud_text(&format!("Q: {}", binds[0]), w - 220, pys); pys += lh; }
            if binds.len() >= 2 { self.hud_text(&format!("E: {}", binds[1]), w - 220, pys); }
            self.draw_overlays_ingame();
            self.draw_ingame_achievement_banner(&bs);
            for t in self.equipped_trails() { if let Some(l) = trails.get(&t) { draw_trail_particles(&bs, &t, l); } }
            if self.flash_timer > 0.0 {
                let fs = Surface::new(w, h);
                fs.set_alpha(Some(255.min((self.flash_timer / 10.0) as i32)));
                fs.fill((255, 255, 255));
                bs.blit(&fs, (0, 0));
            }
            if lives == 1 {
                let pulse = 0.3 + 0.55 * (almost_dead_pulse * std::f64::consts::PI / heartbeat_interval).sin().abs();
                self.draw_almost_dead_vignette(pulse);
            }
            last_frame = self.base_surface.copy();
            let _ = oneups_collected;
            self.flip();
        }
    }

    /// Overlays im Spiel (oben mittig oder an der Weltposition)
    pub fn draw_overlays_ingame(&mut self) {
        self.cleanup_overlays();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        for o in self.overlays.clone() {
            let t = self.small_font.render(&o.text, true, o.color);
            match o.pos {
                None => bs.blit(&t, ((w - t.get_width()) / 2, 8)),
                Some((px, py)) => {
                    let sx = 8.max((w - t.get_width() - 8).min(px - t.get_width() / 2));
                    let sy = 8.max((h - t.get_height() - 8).min(py - t.get_height()));
                    bs.blit(&t, (sx, sy));
                }
            }
        }
    }
}
