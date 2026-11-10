//! 2-Spieler-Modus: run_game_2p() (Python 15739-16153).

use super::core::*;
use super::gameplay::addf;
use super::state::{Snd, G};
use crate::game::save::DIFFICULTIES;
use crate::pg::*;
use crate::util::*;
use serde_json::json;

impl G {
    pub fn run_game_2p(&mut self) {
        let mut clock = Clock::new();
        let difficulty = DIFFICULTIES[self.difficulty_index].to_string();
        let mut speed = difficulty_speed(&difficulty) as f64;
        self.play_ingame_music();
        let (mut min_delay, mut max_delay) = difficulty_obstacle_interval(&difficulty);
        let dynamic_mode = difficulty == "Dynamisch";
        let mut dynamic_timer = 0i64;
        let floor_y = self.HEIGHT - 50;
        let move_speed = 8;
        let gravity = 1;
        let mut p1 = Rect::new(60, floor_y - 40, 40, 40);
        let mut p2 = Rect::new(140, floor_y - 40, 40, 40);
        let (mut vy1, mut vy2) = (0i32, 0i32);
        let (mut jump1, mut jump2) = (false, false);
        let (mut up1_was, mut up2_was) = (false, false);
        let (mut alive1, mut alive2) = (true, true);
        let (mut lives1, mut lives2) = (3i64, 3i64);
        let (mut invuln1, mut invuln2) = (0i32, 0i32);
        let mut obstacles: Vec<Obstacle> = vec![];
        let mut top_obstacles: Vec<Obstacle> = vec![];
        let mut powerups: Vec<Rect> = vec![];
        let mut obs_timer = 0i64;
        let mut obs_delay = randint(min_delay, max_delay);
        let mut top_timer = 0i64;
        let mut top_delay = (randint(900, 1600) as f64 * (800.0 / 400.max(self.WIDTH) as f64)) as i64;
        let mut score = 0i64;
        let vs_mode = self.two_player_submode == "vs";
        let mut round_start = now_secs();
        let mut last_frame = self.base_surface.copy();
        loop {
            self.sync_window_size();
            if !self.game_running { self.stop_music(); return; }
            let dt = clock.tick(60);
            self.fill_bg(true);
            let bs = self.base_surface.clone();
            let (w, h) = (self.WIDTH, self.HEIGHT);
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } if self.action_matches_key(key, "pause") => {
                        let pt = now_secs();
                        let res = self._run_pause_menu(Some(last_frame.clone()));
                        round_start += now_secs() - pt;
                        clock.tick(0);
                        if res == "menu" { self.stop_music(); self.game_running = false; self.in_menu = true; return; }
                        else if res == "restart" { self.stop_music(); return; }
                    }
                    _ => {}
                }
            }
            let mut jumps = 0;
            if alive1 {
                let j = self.any_action_pressed("p1_jump");
                if j && !up1_was && !jump1 { vy1 = -20; jump1 = true; jumps += 1; }
                up1_was = j;
                if self.any_action_pressed("p1_left") { p1.x -= move_speed; }
                if self.any_action_pressed("p1_right") { p1.x += move_speed; }
                p1.set_left(0.max(p1.left()));
                p1.set_right(w.min(p1.right()));
                vy1 += gravity;
                p1.y += vy1;
                if p1.bottom() >= floor_y { p1.set_bottom(floor_y); vy1 = 0; if jump1 { jump1 = false; up1_was = false; } }
            }
            if alive2 {
                let j = self.any_action_pressed("p2_jump");
                if j && !up2_was && !jump2 { vy2 = -20; jump2 = true; jumps += 1; }
                up2_was = j;
                if self.any_action_pressed("p2_left") { p2.x -= move_speed; }
                if self.any_action_pressed("p2_right") { p2.x += move_speed; }
                p2.set_left(0.max(p2.left()));
                p2.set_right(w.min(p2.right()));
                vy2 += gravity;
                p2.y += vy2;
                if p2.bottom() >= floor_y { p2.set_bottom(floor_y); vy2 = 0; if jump2 { jump2 = false; up2_was = false; } }
            }
            if jumps > 0 { let t = self.stats.gi("total_jumps", 0) + jumps; self.stats.insert("total_jumps".into(), json!(t)); }
            if invuln1 > 0 { invuln1 -= 1; }
            if invuln2 > 0 { invuln2 -= 1; }
            obs_timer += dt;
            if obs_timer >= obs_delay {
                obs_timer = 0;
                obs_delay = randint(min_delay, max_delay);
                if random() < 0.015 { powerups.push(Rect::new(w + 50, floor_y - 50, 38, 38)); }
                else {
                    let t = self.pick_obstacle_type(&difficulty, if dynamic_mode { Some(speed) } else { None });
                    obstacles.extend(self.make_obstacle(w + 50, &t));
                }
            }
            let top_active = ["Hardcore", "Unmöglich", "Albtraum", "Hölle"].contains(&difficulty.as_str()) || (dynamic_mode && speed >= 14.0);
            if top_active {
                top_timer += dt;
                if top_timer >= top_delay {
                    top_timer = 0;
                    top_obstacles.push(self.make_top_obstacle());
                    top_delay = (randint(900, 1600) as f64 * (800.0 / 400.max(w) as f64)) as i64;
                }
            }
            let cs = speed;
            let fall = (cs * 0.85).max(2.0);
            let before = obstacles.len();
            for o in obstacles.iter_mut() { o.r.x = addf(o.r.x, -cs); }
            obstacles.retain(|o| o.r.right() >= 0);
            score += (before - obstacles.len()) as i64;
            for o in top_obstacles.iter_mut() { o.r.y = addf(o.r.y, fall); }
            top_obstacles.retain(|o| o.r.top() <= h);
            for p in powerups.iter_mut() { p.x = addf(p.x, -cs); }
            powerups.retain(|p| p.right() >= 0);
            let revive = 45;
            for i in 0..powerups.len() {
                let p = powerups[i];
                if alive1 && p1.colliderect(&p) {
                    if !alive2 {
                        alive2 = true; lives2 = 1; p2.x = 140; p2.set_bottom(floor_y); vy2 = 0; jump2 = false; up2_was = false; invuln2 = revive;
                        self.push_overlay("P2 wiederbelebt!", (255, 160, 50), 1.5, Some((p2.centerx(), p2.top() - 20)));
                    } else {
                        lives1 += 1;
                        self.push_overlay("1-UP! (P1)", (0, 200, 0), 1.5, Some((p1.centerx(), p1.top() - 20)));
                    }
                    powerups.remove(i);
                    break;
                } else if alive2 && p2.colliderect(&p) {
                    if !alive1 {
                        alive1 = true; lives1 = 1; p1.x = 60; p1.set_bottom(floor_y); vy1 = 0; jump1 = false; up1_was = false; invuln1 = revive;
                        self.push_overlay("P1 wiederbelebt!", (100, 150, 255), 1.5, Some((p1.centerx(), p1.top() - 20)));
                    } else {
                        lives2 += 1;
                        self.push_overlay("1-UP! (P2)", (0, 200, 0), 1.5, Some((p2.centerx(), p2.top() - 20)));
                    }
                    powerups.remove(i);
                    break;
                }
            }
            let (mut died1, mut died2) = (false, false);
            for pl in 0..2 {
                for which in 0..2 {
                    let (alive, inv, pr) = if pl == 0 { (alive1, invuln1, p1) } else { (alive2, invuln2, p2) };
                    let list = if which == 0 { &mut obstacles } else { &mut top_obstacles };
                    if let Some(idx) = list.iter().position(|o| alive && inv == 0 && pr.colliderect(&o.r)) {
                        list.remove(idx);
                        self.play_sound_v(Snd::Hit, "hit_volume");
                        if pl == 0 {
                            lives1 -= 1;
                            if lives1 <= 0 { alive1 = false; died1 = true; } else { invuln1 = 90; self.push_overlay("P1 Treffer!", (100, 150, 255), 1.5, Some((p1.centerx(), p1.top() - 20))); }
                        } else {
                            lives2 -= 1;
                            if lives2 <= 0 { alive2 = false; died2 = true; } else { invuln2 = 90; self.push_overlay("P2 Treffer!", (255, 160, 50), 1.5, Some((p2.centerx(), p2.top() - 20))); }
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
            let mut game_over = false;
            let mut winner: Option<&str> = None;
            if vs_mode {
                if died1 && died2 { winner = Some("draw"); game_over = true; }
                else if died1 { winner = Some("p2"); game_over = true; }
                else if died2 { winner = Some("p1"); game_over = true; }
            } else if !alive1 && !alive2 { game_over = true; }

            for o in &obstacles {
                draw::rect(&bs, (150, 50, 50), Rect::new(o.r.x + 2, o.r.y + 2, o.r.w, o.r.h), 0, 6);
                draw::rect(&bs, RED, o.r, 0, 6);
            }
            for o in &top_obstacles {
                draw::rect(&bs, (80, 0, 110), Rect::new(o.r.x + 2, o.r.y + 2, o.r.w, o.r.h), 0, 6);
                draw::rect(&bs, (160, 0, 200), o.r, 0, 6);
                let (ax, ay) = (o.r.centerx(), o.r.top() - 8);
                draw::polygon(&bs, (200, 100, 255), &[(ax, ay + 7), (ax - 6, ay), (ax + 6, ay)], 0);
            }
            for p in &powerups {
                draw::rect(&bs, (100, 255, 100), Rect::new(p.x - 3, p.y - 3, p.w + 6, p.h + 6), 0, 10);
                draw::rect(&bs, GREEN, *p, 0, 8);
                let t = self.small_font.render("1-UP", true, WHITE);
                bs.blit(&t, (p.centerx() - t.get_width() / 2, p.centery() - t.get_height() / 2));
            }
            let sf = self.small_font.clone();
            let mf = self.menu_font.clone();
            for (alive, inv, pr, sh, col, lbl) in [(alive1, invuln1, p1, (70, 70, 130), BLUE, "P1"), (alive2, invuln2, p2, (130, 70, 30), (255, 120, 0), "P2")] {
                if !alive { continue; }
                if inv == 0 || (inv / 5) % 2 == 0 {
                    draw::rect(&bs, sh, Rect::new(pr.x + 3, pr.y + 3, pr.w, pr.h), 0, 8);
                    draw::rect(&bs, col, pr, 0, 8);
                }
                let l = sf.render(lbl, true, WHITE);
                let ls = sf.render(lbl, true, (0, 0, 0));
                let (lx, ly) = (pr.centerx() - l.get_width() / 2, pr.top() - 16);
                bs.blit(&ls, (lx + 1, ly + 1));
                bs.blit(&l, (lx, ly));
            }
            let p1c = if alive1 { BLUE } else { (70, 70, 70) };
            let l1 = mf.render("P1", true, p1c);
            bs.blit(&l1, (12, 10));
            for i in 0..3.max(lives1) {
                let c = if i < lives1 { p1c } else { (55, 55, 55) };
                draw::rect(&bs, c, Rect::new(12 + i as i32 * 15, 10 + l1.get_height() + 3, 11, 11), 0, 3);
            }
            let p2c = if alive2 { (255, 120, 0) } else { (70, 70, 70) };
            let l2 = mf.render("P2", true, p2c);
            bs.blit(&l2, (w - 12 - l2.get_width(), 10));
            let cnt = 3.max(lives2);
            for i in 0..cnt {
                let c = if i < lives2 { p2c } else { (55, 55, 55) };
                draw::rect(&bs, c, Rect::new(w - 12 - (cnt - i) as i32 * 15, 10 + l2.get_height() + 3, 11, 11), 0, 3);
            }
            let elapsed = (now_secs() - round_start) as i64;
            let tag = if vs_mode { self.get_text("vs_mode_label") } else { self.get_text("coop_mode_label") };
            let ss = mf.render(&format!("{}: {}", self.get_text("score"), score), true, self.get_text_color());
            let is = sf.render(&format!("{}  ·  {}s  ·  {}", tag, elapsed, self.get_difficulty_name(&difficulty)), true, (140, 60, 200));
            bs.blit(&ss, (w / 2 - ss.get_width() / 2, 10));
            bs.blit(&is, (w / 2 - is.get_width() / 2, 10 + ss.get_height() + 2));
            let hint = sf.render(&self.get_text("esc_menu_hint"), true, (90, 90, 110));
            bs.blit(&hint, (w / 2 - hint.get_width() / 2, h - 18));
            if !alive1 { let e = mf.render("P1 ELIMINATED", true, (200, 70, 70)); bs.blit(&e, (20, h / 2 - e.get_height() / 2)); }
            if !alive2 { let e = mf.render("P2 ELIMINATED", true, (200, 70, 70)); bs.blit(&e, (w - e.get_width() - 20, h / 2 - e.get_height() / 2)); }
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
            if game_over {
                let secs = (now_secs() - round_start) as i64;
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
                self.show_game_over_2p(score, winner, secs);
                return;
            }
        }
    }
}
