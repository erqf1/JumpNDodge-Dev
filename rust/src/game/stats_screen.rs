//! Statistik-Bildschirm (Python 10816-11337).

use super::state::G;
use crate::game::save::DIFFICULTIES;
use crate::pg::*;
use crate::util::*;
use serde_json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatTab { Main(usize), Sub(usize), Back }

pub const STAT_CATEGORIES: [&str; 4] = ["stat_highscores", "stat_achievements", "stat_total", "stat_modded"];

/// str(timedelta(seconds=n))
pub fn py_timedelta(secs: i64) -> String {
    let days = secs / 86400;
    let rem = secs % 86400;
    let s = format!("{}:{:02}:{:02}", rem / 3600, (rem % 3600) / 60, rem % 60);
    if days == 0 { s } else if days == 1 { format!("1 day, {}", s) } else { format!("{} days, {}", days, s) }
}

fn diff_col_map(d: &str) -> Option<(i32, i32, i32)> {
    match d {
        "Leicht" => Some((0, 200, 0)), "Mittel" => Some((200, 200, 0)), "Schwer" => Some((200, 100, 0)), "Hardcore" => Some((200, 0, 0)),
        "Unmöglich" => Some((150, 0, 150)), "Albtraum" => Some((139, 0, 0)), "Hölle" => Some((20, 20, 20)), "Dynamisch" => Some((0, 150, 200)), _ => None,
    }
}

fn fade_alpha(y: i32, h: i32, fade_top: i32, fade_bottom: i32, fade_dist: i32) -> i32 {
    if y < fade_top { 0.max((255.0 * (y - (fade_top - fade_dist)) as f64 / fade_dist as f64) as i32) }
    else if y + h > fade_bottom { 0.max((255.0 * ((fade_bottom - y) as f64 / fade_dist as f64)) as i32) }
    else { 255 }
}

impl G {
    fn draw_padlock_notice(&self, bs: &Surface, cx: i32, cy: i32, ly: i32) {
        let lc = (120, 120, 140);
        let lx = cx - 18;
        draw::rect(bs, lc, Rect::new(lx, ly + 18, 36, 28), 0, 5);
        draw::arc(bs, lc, Rect::new(lx + 6, ly, 24, 26), 0.0, std::f64::consts::PI, 4);
        draw::circle(bs, (80, 80, 100), (cx, ly + 31), 5, 0);
        let d1 = self.menu_font.render(&self.get_text("achievements_disabled_notice"), true, self.get_text_color());
        let d2 = self.small_font.render(&self.get_text("future_version_notice"), true, (140, 140, 160));
        bs.blit(&d1, (cx - d1.get_width() / 2, cy - 10));
        bs.blit(&d2, (cx - d2.get_width() / 2, cy + 26));
    }

    fn draw_score_table(&self, bs: &Surface, y: &mut i32, scores: &serde_json::Map<String, Value>, session: &serde_json::Map<String, Value>,
                        best: &serde_json::Map<String, Value>, stimes: &serde_json::Map<String, Value>, with_avg: bool, hdr_border: (i32, i32, i32), ft: i32, fb: i32) {
        let w = self.WIDTH;
        let sf = &self.small_font;
        let hb = Rect::new(70, *y, w - 140, 35);
        draw::rect(bs, self.get_panel_color(), hb, 0, 8);
        draw::rect(bs, hdr_border, hb, 2, 8);
        bs.blit(&sf.render(&self.get_text("table_difficulty"), true, self.get_text_color()), (90, *y + 8));
        bs.blit(&sf.render(&self.get_text("table_highscore"), true, self.get_text_color()), (300, *y + 8));
        bs.blit(&sf.render(&self.get_text("table_session"), true, self.get_text_color()), (450, *y + 8));
        if with_avg { bs.blit(&sf.render(&self.get_text("avg_score"), true, self.get_text_color()), (620, *y + 8)); }
        *y += 45;
        let dim = if self.is_dark_mode() { (120, 120, 140) } else { (110, 110, 130) };
        for (i, diff) in DIFFICULTIES.iter().enumerate() {
            let record = py_str(scores.get(*diff).unwrap_or(&Value::from(0)));
            let sv = py_str(session.get(*diff).unwrap_or(&Value::from(0)));
            let rt = py_str(best.get(*diff).unwrap_or(&Value::from(0)));
            let st = py_str(stimes.get(*diff).unwrap_or(&Value::from(0)));
            let rounds = self.stats.gobj("rounds_by_diff").gi(diff, 0);
            let avg = if rounds > 0 { py_float((self.stats.gobj("score_sum_by_diff").gf(diff, 0.0) / rounds as f64 * 10.0).round() / 10.0) } else { "0".into() };
            let alpha = fade_alpha(*y, 36, ft, fb, 20);
            if alpha > 0 {
                let rb = Rect::new(70, *y, w - 140, 36);
                draw::rect(bs, if i % 2 == 0 { self.get_bg_color() } else { self.get_panel_color() }, rb, 0, 6);
                draw::rect(bs, (180, 180, 180), rb, 1, 6);
                let ds = sf.render(&self.get_difficulty_name(diff), true, diff_col_map(diff).map(|c| super::ui_c::readable(c, self.is_dark_mode())).unwrap_or(self.get_text_color()));
                ds.set_alpha(Some(alpha));
                bs.blit(&ds, (90, *y + 10));
                for (sv_, tv, cx) in [(record.clone(), rt.clone(), 320), (sv.clone(), st.clone(), 470)] {
                    let s = sf.render(&sv_, true, self.get_text_color());
                    let t = sf.render(&format!(" {}s", tv), true, dim);
                    s.set_alpha(Some(alpha));
                    t.set_alpha(Some(alpha));
                    bs.blit(&s, (cx, *y + 10));
                    bs.blit(&t, (cx + s.get_width(), *y + 10));
                }
                if with_avg {
                    let a = sf.render(&avg, true, self.get_text_color());
                    a.set_alpha(Some(alpha));
                    bs.blit(&a, (620, *y + 10));
                }
            }
            *y += 40;
        }
    }

    fn draw_stat_rows(&self, bs: &Surface, y: &mut i32, items: &[(String, String)], ft: i32, fb: i32) {
        let w = self.WIDTH;
        for (label, value) in items {
            let alpha = fade_alpha(*y, 40, ft, fb, 20);
            if alpha > 0 {
                let sr = Rect::new(70, *y, w - 140, 40);
                draw::rect(bs, self.get_panel_color(), sr, 0, 8);
                draw::rect(bs, (200, 200, 200), sr, 1, 8);
                let ls = self.small_font.render(&format!("{}:", label), true, self.get_text_color());
                let vs = self.small_font.render(value, true, self.get_text_color());
                ls.set_alpha(Some(alpha));
                vs.set_alpha(Some(alpha));
                bs.blit(&ls, (80, *y + 10));
                bs.blit(&vs, (300, *y + 10));
            }
            *y += 45;
        }
    }

    pub fn draw_statistics(&mut self) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        draw::rect(&bs, self.get_bg_color(), Rect::new(0, 0, w, 108), 0, 0);
        draw::rect(&bs, self.get_bg_color(), Rect::new(0, h - 65, w, 65), 0, 0);
        let t = self.font.render(&self.get_text("stats"), true, self.get_text_color());
        bs.blit(&self.font.render(&self.get_text("stats"), true, (100, 100, 120)), (w / 2 - t.get_width() / 2 + 3, 15));
        bs.blit(&t, (w / 2 - t.get_width() / 2, 12));
        let has_modded = self.mods_installed();
        let vis: Vec<&str> = STAT_CATEGORIES.iter().copied().filter(|c| *c != "stat_modded" || has_modded).collect();
        if self.current_stat_tab >= vis.len() { self.current_stat_tab = 0; }
        let modded_idx = vis.iter().position(|c| *c == "stat_modded");
        let modded_active = modded_idx == Some(self.current_stat_tab);
        let nav_total = vis.len() + if modded_active { 3 } else { 0 };
        self.stat_tab_rects.clear();
        let (tw, tg) = (170, 8);
        let total = vis.len() as i32 * tw + (vis.len() as i32 - 1) * tg;
        let tx0 = (w - total) / 2;
        let sf = self.small_font.clone();
        for (i, key) in vis.iter().enumerate() {
            let r = Rect::new(tx0 + i as i32 * (tw + tg), 65, tw, 35);
            draw::rect(&bs, if i == self.current_stat_tab { self.get_button_color() } else { self.get_panel_color() }, r, 0, 10);
            let mut brd = if *key == "stat_modded" && i == self.current_stat_tab { (220, 160, 0) } else { (180, 180, 180) };
            if self.nav_is_focused("statistics", i, nav_total) { brd = (255, 255, 255); }
            draw::rect(&bs, brd, r, 2, 10);
            let ls = sf.render(&self.get_text(key), true, self.get_text_color());
            bs.blit(&ls, (r.centerx() - ls.get_width() / 2, r.centery() - ls.get_height() / 2));
            self.stat_tab_rects.push((r, StatTab::Main(i)));
        }
        let cp = Rect::new(50, 160, w - 100, h - 240);
        draw::rect(&bs, self.get_panel_color(), cp, 0, 16);
        draw::rect(&bs, (60, 80, 140), cp, 3, 16);
        let (ft, fb) = (160, h - 80);
        let mut y = 180 - self.stat_scroll;
        if self.current_stat_tab == 0 {
            let (ss, ssc, bt, st) = (self.saved_scores.clone(), self.session_scores.clone(), self.best_times.clone(), self.session_times.clone());
            self.draw_score_table(&bs, &mut y, &ss, &ssc, &bt, &st, true, (60, 80, 140), ft, fb);
        } else if self.current_stat_tab == 1 {
            self.draw_padlock_notice(&bs, w / 2, cp.centery(), cp.centery() - 72);
        } else if self.current_stat_tab == 2 {
            let s = &self.stats;
            let items = vec![
                (self.get_text("total_time"), py_timedelta(s.gf("total_time_played", 0.0) as i64)),
                (self.get_text("rounds_played"), py_str(s.get("total_rounds").unwrap_or(&Value::from(0)))),
                (self.get_text("1ups_collected"), py_str(s.get("total_1ups").unwrap_or(&Value::from(0)))),
                (self.get_text("coins"), py_str(s.get("coins").unwrap_or(&Value::from(0)))),
                (self.get_text("total_jumps"), py_str(s.get("total_jumps").unwrap_or(&Value::from(0)))),
                (self.get_text("total_deaths"), py_str(s.get("total_deaths").unwrap_or(&Value::from(0)))),
                (self.get_text("total_coins_earned"), py_str(s.get("total_coins_earned").unwrap_or(&Value::from(0)))),
            ];
            self.draw_stat_rows(&bs, &mut y, &items, ft, fb);
        } else if modded_active {
            let dark = self.is_dark_mode();
            let labels = ["Highscores", "Achievements", "Stats"];
            let (sw, sg) = (140, 6);
            let sx0 = (w - (3 * sw + 2 * sg)) / 2;
            for (si, sl) in labels.iter().enumerate() {
                let sr = Rect::new(sx0 + si as i32 * (sw + sg), y, sw, 32);
                draw::rect(&bs, if si == self.modded_sub_tab { self.get_button_color() } else { self.get_panel_color() }, sr, 0, 8);
                let brd = if self.nav_is_focused("statistics", vis.len() + si, nav_total) { (255, 255, 255) } else { (220, 160, 0) };
                draw::rect(&bs, brd, sr, 2, 8);
                let ss = sf.render(sl, true, self.get_text_color());
                bs.blit(&ss, (sr.centerx() - ss.get_width() / 2, sr.centery() - ss.get_height() / 2));
                self.stat_tab_rects.push((sr, StatTab::Sub(si)));
            }
            y += 44;
            let ov = Rect::new(55, y, w - 110, 34);
            draw::rect(&bs, if dark { (40, 30, 10) } else { (255, 248, 220) }, ov, 0, 8);
            draw::rect(&bs, (200, 140, 0), ov, 2, 8);
            let os = sf.render("Mods geladen: 0   Aktiv: 0   Fehler: 0", true, (220, 160, 0));
            bs.blit(&os, (ov.centerx() - os.get_width() / 2, ov.centery() - os.get_height() / 2));
            y += 42;
            if self.modded_sub_tab == 0 {
                let (ms, mss, mbt, mst) = (self.modded_scores.clone(), self.modded_session_scores.clone(), self.modded_best_times.clone(), self.modded_session_times.clone());
                self.draw_score_table(&bs, &mut y, &ms, &mss, &mbt, &mst, false, (220, 160, 0), ft, fb);
            } else if self.modded_sub_tab == 1 {
                self.draw_padlock_notice(&bs, w / 2, cp.centery() + 30, cp.centery() + 30 - 52);
            } else {
                let m = &self.modded_stats;
                let items = vec![
                    (self.get_text("total_time"), py_timedelta(m.gf("total_time_played", 0.0) as i64)),
                    (self.get_text("rounds_played"), py_str(m.get("total_rounds").unwrap_or(&Value::from(0)))),
                    (self.get_text("1ups_collected"), py_str(m.get("total_1ups").unwrap_or(&Value::from(0)))),
                    (self.get_text("coins"), py_str(m.get("coins").unwrap_or(&Value::from(0)))),
                ];
                self.draw_stat_rows(&bs, &mut y, &items, ft, fb);
            }
            y += 10;
            let nm = sf.render("Keine Mods geladen.", true, (160, 160, 160));
            bs.blit(&nm, (w / 2 - nm.get_width() / 2, y + 10));
        }
        let br = Rect::new(w - 180, h - 60, 150, 40);
        draw::rect(&bs, self.get_button_color(), br, 0, 10);
        draw::rect(&bs, (180, 180, 180), br, 2, 10);
        let bl = sf.render(&self.get_text("back"), true, self.get_text_color());
        bs.blit(&bl, (br.centerx() - bl.get_width() / 2, br.centery() - bl.get_height() / 2));
        self.stat_tab_rects.push((br, StatTab::Back));
        self.flip();
    }
}
