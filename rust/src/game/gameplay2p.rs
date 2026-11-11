//! 2-Spieler-Modus: run_game_2p() (Python 15739-16153).
//! Die Regeln stecken in jnd_core::sim2p (geteilt mit dem späteren Multiplayer-Server),
//! hier bleiben nur Eingabe, Zeichnen, Overlays, Sound und Statistik.

use super::core::*;
use super::state::{Snd, G};
use crate::data::d;
use crate::game::save::DIFFICULTIES;
use crate::pg::*;
use crate::util::*;
use jnd_core::sim2p::{self, Input, SimEvent, Sim2p};
use serde_json::json;

/// Spielerfarben (P2 pink statt orange - nie Orange neben dem 1-UP)
const P_COLORS: [(i32, i32, i32); 2] = super::multiplayer::MP_COLORS;
const P_OVERLAY: [(i32, i32, i32); 2] = [(110, 160, 255), (240, 120, 190)];

impl G {
    fn input_2p(&self, prefix: &str) -> Input {
        Input {
            jump: self.any_action_pressed(&format!("{}_jump", prefix)),
            left: self.any_action_pressed(&format!("{}_left", prefix)),
            right: self.any_action_pressed(&format!("{}_right", prefix)),
        }
    }

    pub fn run_game_2p(&mut self) {
        let mut clock = Clock::new();
        let difficulty = DIFFICULTIES[self.difficulty_index].to_string();
        self.play_ingame_music();
        let weights: Vec<(String, f64)> = d("difficulty_obstacle_types").gobj(&difficulty).iter()
            .map(|(k, v)| (k.clone(), as_f(v).unwrap_or(0.0))).collect();
        let cfg = sim2p::Config { difficulty: difficulty.clone(), vs: self.two_player_submode == "vs", obstacle_weights: weights, seed: rand::random() };
        let vs_mode = cfg.vs;
        let mut sim = Sim2p::new(cfg, self.WIDTH, self.HEIGHT);
        let mut round_start = now_secs();
        let mut last_frame = self.base_surface.copy();
        // Feste Logikrate (60/s) wie Server und pygame-Original, Zeichnen mit Monitor-Hz
        let mut acc_ms = LOGIC_TICK_MS;
        let mut tick_no: u64 = 0;
        let mut jump_latch = [false; 2];
        let mut focus_pause = false;
        let mut prev_players = [sim.players[0].rect, sim.players[1].rect];
        loop {
            self.sync_window_size();
            if !self.game_running { self.stop_music(); return; }
            let fps = self.target_fps();
            acc_ms += clock.tick_f(fps).min(250.0);
            self.fill_bg(true);
            let bs = self.base_surface.clone();
            let (w, h) = (self.WIDTH, self.HEIGHT);
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } if self.action_matches_key(key, "p1_jump") || self.action_matches_key(key, "p2_jump") => {
                        if self.action_matches_key(key, "p1_jump") { jump_latch[0] = true; }
                        if self.action_matches_key(key, "p2_jump") { jump_latch[1] = true; }
                    }
                    Event::FocusLost if self.settings.gb("pause_on_focus_loss", true) && !self.display.hidden => {
                        focus_pause = true;
                        self.display.event_post(Event::KeyDown { key: 0, unicode: String::new(), mods: 0 });
                    }
                    Event::KeyDown { key, .. } if self.action_matches_key(key, "pause") || focus_pause => {
                        focus_pause = false;
                        let pt = now_secs();
                        let res = self._run_pause_menu(Some(last_frame.clone()));
                        round_start += now_secs() - pt;
                        clock.tick(0);
                        acc_ms = 0.0;
                        if res == "menu" { self.stop_music(); self.game_running = false; self.in_menu = true; return; }
                        else if res == "restart" { self.stop_music(); return; }
                    }
                    _ => {}
                }
            }
            let mut steps = 0;
            let mut game_over: Option<Option<sim2p::Winner>> = None;
            while acc_ms >= LOGIC_TICK_MS && game_over.is_none() {
                acc_ms -= LOGIC_TICK_MS;
                steps += 1;
                if steps > 8 { acc_ms = 0.0; }
                let dt = logic_dt(tick_no);
                tick_no += 1;
                prev_players = [sim.players[0].rect, sim.players[1].rect];
                let mut inputs = [self.input_2p("p1"), self.input_2p("p2")];
                for i in 0..2 { inputs[i].jump |= jump_latch[i]; jump_latch[i] = false; }
                let res = sim.step(dt, inputs, w, h);
                if res.jumps > 0 { let t = self.stats.gi("total_jumps", 0) + res.jumps; self.stats.insert("total_jumps".into(), json!(t)); }
                for ev in &res.events {
                    let r = |i: usize| sim.players[i].rect;
                    let at = |i: usize| Some((r(i).centerx(), r(i).top() - 20));
                    match *ev {
                        SimEvent::Revived(i) => self.push_overlay(&format!("P{} wiederbelebt!", i + 1), P_OVERLAY[i], 1.5, at(i)),
                        SimEvent::OneUp(i) => self.push_overlay(&format!("1-UP! (P{})", i + 1), super::art::ONEUP, 1.5, at(i)),
                        SimEvent::Hit(i) => { self.play_sound_v(Snd::Hit, "hit_volume"); self.push_overlay(&format!("P{} Treffer!", i + 1), P_OVERLAY[i], 1.5, at(i)); }
                        SimEvent::Died(_) => self.play_sound_v(Snd::Hit, "hit_volume"),
                    }
                }

                if res.game_over { game_over = Some(res.winner); }
            }
            let alpha = (acc_ms / LOGIC_TICK_MS).clamp(0.0, 1.0);
            // Hindernisse bewegen sich gleichmäßig: Position zwischen letztem und aktuellem Schritt
            let back = 1.0 - alpha;
            let cs = sim.speed;
            let fall = (cs * 0.85).max(2.0);
            let gnd = |r: Rect| Rect::new(r.x + (cs * back).round() as i32, r.y, r.w, r.h);
            let top = |r: Rect| Rect::new(r.x, r.y - (fall * back).round() as i32, r.w, r.h);
            let tsec = self.get_ticks() as f64 / 1000.0;
            if self.settings.gb("show_floor_line", true) {
                let dark = self.is_dark_mode();
                super::art::draw_floor(&bs, w, h, sim.floor_y + 2, (sim.frame as f64 + alpha) * cs, dark);
            }
            for p in sim.powerups.iter().map(|p| gnd(*p)) { super::art::draw_oneup(&bs, p, tsec, &self.badge_font); }
            for o in &sim.obstacles { super::art::draw_hazard(&bs, gnd(o.rect)); }
            for o in &sim.top_obstacles {
                let r = top(o.rect);
                super::art::draw_faller(&bs, r);
                super::art::draw_faller_marker(&bs, r, tsec);
            }
            let sf = self.small_font.clone();
            let mf = self.menu_font.clone();
            for (i, p) in sim.players.iter().enumerate() {
                if !p.alive { continue; }
                let pr = lerp_rect(Some((prev_players[i].x, prev_players[i].y)), p.rect, alpha);
                if p.invuln == 0 || (p.invuln / 5) % 2 == 0 {
                    super::art::draw_player(&bs, pr, P_COLORS[i]);
                }
                let lbl = format!("P{}", i + 1);
                let l = sf.render(&lbl, true, WHITE);
                let ls = sf.render(&lbl, true, (0, 0, 0));
                let (lx, ly) = (pr.centerx() - l.get_width() / 2, pr.top() - 16);
                bs.blit(&ls, (lx + 1, ly + 1));
                bs.blit(&l, (lx, ly));
            }
            // Leben-Anzeige: P1 links, P2 rechts
            for i in 0..2 {
                let p = &sim.players[i];
                let col = if p.alive { P_COLORS[i] } else { (70, 70, 70) };
                let lbl = mf.render(&format!("P{}", i + 1), true, col);
                let cnt = 3.max(p.lives);
                if i == 0 { bs.blit(&lbl, (12, 10)); } else { bs.blit(&lbl, (w - 12 - lbl.get_width(), 10)); }
                for k in 0..cnt {
                    let c = if k < p.lives { col } else { (55, 55, 55) };
                    let x = if i == 0 { 12 + k as i32 * 15 } else { w - 12 - (cnt - k) as i32 * 15 };
                    draw::rect(&bs, c, Rect::new(x, 10 + lbl.get_height() + 3, 11, 11), 0, 3);
                }
            }
            let elapsed = (now_secs() - round_start) as i64;
            let tag = if vs_mode { self.get_text("vs_mode_label") } else { self.get_text("coop_mode_label") };
            let ss = mf.render(&format!("{}: {}", self.get_text("score"), sim.score), true, self.get_text_color());
            let is = sf.render(&format!("{}  ·  {}s  ·  {}", tag, elapsed, self.get_difficulty_name(&difficulty)), true, (140, 60, 200));
            bs.blit(&ss, (w / 2 - ss.get_width() / 2, 10));
            bs.blit(&is, (w / 2 - is.get_width() / 2, 10 + ss.get_height() + 2));
            let hint = sf.render(&self.get_text("esc_menu_hint"), true, (90, 90, 110));
            bs.blit(&hint, (w / 2 - hint.get_width() / 2, h - 18));
            if !sim.players[0].alive { let e = mf.render("P1 ELIMINATED", true, (200, 70, 70)); bs.blit(&e, (20, h / 2 - e.get_height() / 2)); }
            if !sim.players[1].alive { let e = mf.render("P2 ELIMINATED", true, (200, 70, 70)); bs.blit(&e, (w - e.get_width() - 20, h / 2 - e.get_height() / 2)); }
            self.cleanup_overlays();
            for o in self.overlays.clone() {
                let t = sf.render(&o.text, true, o.color);
                match o.pos {
                    None => bs.blit(&t, ((w - t.get_width()) / 2, 55)),
                    Some((px, py)) => {
                        let sx = 8.max((w - t.get_width() - 8).min(px - t.get_width() / 2));
                        let sy = 8.max((h - t.get_height() - 8).min(py - t.get_height()));
                        bs.blit(&t, (sx, sy));
                    }
                }
            }
            last_frame = self.base_surface.copy();
            self.flip();
            if let Some(winner) = game_over {
                let secs = (now_secs() - round_start) as i64;
                let score = sim.score;
                self.stop_music();
                let s = &mut self.stats;
                let tr = s.gi("total_rounds", 0) + 1;
                s.insert("total_rounds".into(), json!(tr));
                let td = s.gi("total_deaths", 0) + 1;
                s.insert("total_deaths".into(), json!(td));
                let mut ssd = s.gobj("score_sum_by_diff");
                let v = ssd.gi(&difficulty, 0) + score;
                ssd.insert(difficulty.clone(), json!(v));
                s.insert("score_sum_by_diff".into(), serde_json::Value::Object(ssd));
                let mut rbd = s.gobj("rounds_by_diff");
                let v = rbd.gi(&difficulty, 0) + 1;
                rbd.insert(difficulty.clone(), json!(v));
                s.insert("rounds_by_diff".into(), serde_json::Value::Object(rbd));
                self.save_section("stats", None);
                self.show_game_over_2p(score, winner.map(|w| w.as_str()), secs);
                return;
            }
        }
    }
}
