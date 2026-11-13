//! Online-Bildschirme Teil 1: Rangliste, Spielersuche, öffentliches Profil, Username-Gate
//! (Python 6740-7700).

use super::core::*;
use super::menus::fit_surf;
use super::screens_a::player_color;
use super::state::{Snd, G};
use crate::game::save::DIFFICULTIES;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

/// f"{n:,}".replace(",", ".")
pub fn fmt_thousands(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 { out.push('.'); }
        out.push(c);
    }
    if n < 0 { format!("-{}", out) } else { out }
}

pub fn _format_playtime(seconds: i64) -> String {
    let (hours, rem) = (seconds / 3600, seconds % 3600);
    let minutes = rem / 60;
    if hours > 0 { format!("{}h {}m", hours, minutes) } else { format!("{}m", minutes) }
}

pub fn _is_valid_username_format(u: &str) -> bool {
    let n = u.chars().count();
    if !(3..=20).contains(&n) { return false; }
    let first = u.chars().next().unwrap();
    if !(first.is_ascii() && first.is_ascii_alphabetic()) { return false; }
    u.chars().all(|c| c.is_ascii() && (c.is_ascii_alphanumeric() || c == '_'))
}

pub fn _draw_verified_badge(s: &Surface, cx: i32, cy: i32, radius: i32) {
    draw::circle(s, (60, 150, 255), (cx, cy), radius, 0);
    let r = radius as f64;
    let (cx, cy) = (cx as f64, cy as f64);
    let pts = [(cx - r * 0.5, cy + r * 0.05), (cx - r * 0.1, cy + r * 0.45), (cx + r * 0.55, cy - r * 0.35)];
    draw::lines(s, (255, 255, 255), false, &pts, 2.max(radius / 4));
}

impl G {
    pub fn quit(&mut self) -> ! {
        self.stop_music();
        crate::crypto::flush_saves();
        std::process::exit(0);
    }

    /// _draw_wrapped_text(text, x, y, max_w, font, color, line_gap=4) -> y danach
    pub fn _draw_wrapped_text(&self, text: &str, x: i32, y: i32, max_w: i32, f: &Font, color: (i32, i32, i32), line_gap: i32) -> i32 {
        let mut lines = vec![];
        let mut line = String::new();
        for w in text.split_whitespace() {
            let test = format!("{} {}", line, w).trim().to_string();
            if f.size(&test).0 <= max_w { line = test; } else {
                if !line.is_empty() { lines.push(line.clone()); }
                line = w.to_string();
            }
        }
        if !line.is_empty() { lines.push(line); }
        let mut y = y;
        let bs = self.base_surface.clone();
        for ln in lines {
            let s = f.render(&ln, true, color);
            bs.blit(&s, (x, y));
            y += s.get_height() + line_gap;
        }
        y
    }

    // ---------------- Rangliste ----------------
    pub fn _lb_start_fetch(&mut self) {
        self._lb_request_id += 1;
        self._lb_my_rank = None;
        let my_id = self._lb_request_id;
        let difficulty = DIFFICULTIES[self._lb_difficulty_idx];
        if self._lb_friends_only && self.friends_local.is_empty() {
            self._lb_loading = false;
            self._lb_entries = Some(vec![]);
            return;
        }
        self._lb_loading = true;
        let mut usernames = None;
        if self._lb_friends_only {
            let mut u: Vec<String> = self.friends_local.iter().filter_map(|f| f.gos("username")).filter(|s| !s.is_empty()).collect();
            if let Some(me) = &self.current_profile_username { if !u.contains(me) { u.push(me.clone()); } }
            usernames = Some(u);
        }
        self.fetch_leaderboard("standard", difficulty, 50, usernames, Some(Box::new(move |g, result| {
            if my_id != g._lb_request_id { return; }
            g._lb_loading = false;
            g._lb_entries = Some(match &result { Some(r) if r.truthy("ok") => r.garr("entries"), _ => vec![] });
        })));
        self.fetch_my_rank("standard", difficulty, Some(Box::new(move |g, result| {
            if my_id != g._lb_request_id { return; }
            g._lb_my_rank = Some(match &result {
                Some(r) if r.truthy("ok") => (r.gv("rank").and_then(as_i), r.gv("score").and_then(as_i)),
                _ => (None, None),
            });
        })));
    }

    pub fn _refresh_friends_local(&mut self) {
        if self.current_profile_name.is_none() { return; }
        let Some(u) = self.current_profile_username.clone() else { return; };
        self.fetch_friends_list(&u, Some(Box::new(|g, result| {
            if let Some(r) = result { if r.truthy("ok") { g._friends_local_pending = Some(r.garr("friends")); } }
        })));
    }

    pub fn _apply_friends_local_pending(&mut self) {
        if let Some(p) = self._friends_local_pending.take() {
            self.friends_local = p;
            self.save_current_profile();
        }
    }

    pub fn draw_online_leaderboard(&mut self) -> [Rect; 7] {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        self._lb_row_rects.clear();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let shift = (70.0 * ((w - 500) as f64 / 300.0).clamp(0.0, 1.0)) as i32;
        let tt = self.get_text("online_leaderboard_btn");
        self.draw_screen_title(&bs, &tt, 20);
        let sf = self.small_font.clone();
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), radius: Some(10), ..Default::default() });
        let search = Rect::new(20, 15, 110, 35);
        self.draw_ui_button(&bs, search, &self.get_text("search"), BtnOpts { font: Some(sf.clone()), hovered: search.collidepoint(mp), radius: Some(10), ..Default::default() });
        let all_tab = Rect::new(60, 65, 100, 36);
        let fr_tab = Rect::new(166, 65, 100, 36);
        for (r, active, label) in [(all_tab, !self._lb_friends_only, self.get_text("all_tab_lb")), (fr_tab, self._lb_friends_only, self.get_text("friends_tab"))] {
            let hov = r.collidepoint(mp);
            if active {
                draw::rect(&bs, self.get_button_color(), r, 0, 10);
                draw::rect(&bs, (100, 160, 255), r, 2, 10);
            } else {
                draw::rect(&bs, if hov { self.get_button_hover_color() } else { self.get_panel_color() }, r, 0, 10);
                draw::rect(&bs, (180, 180, 180), r, 1, 10);
            }
            let t = sf.render(&label, true, self.get_text_color());
            bs.blit(&t, (r.centerx() - t.get_width() / 2, r.centery() - t.get_height() / 2));
        }
        let dl = self.get_difficulty_display_name(DIFFICULTIES[self._lb_difficulty_idx]);
        let dr = Rect::new(w / 2 - 95, 65, 190, 36);
        let dleft = Rect::new(dr.x - 32, dr.y, 26, 36);
        let dright = Rect::new(dr.right() + 6, dr.y, 26, 36);
        self.draw_panel(&bs, dr, 10, (180, 180, 180), 2);
        let ls = sf.render(&dl, true, self.get_text_color());
        bs.blit(&ls, (dr.centerx() - ls.get_width() / 2, dr.centery() - ls.get_height() / 2));
        for (ar, at) in [(dleft, "<"), (dright, ">")] {
            self.draw_ui_button(&bs, ar, at, BtnOpts { font: Some(sf.clone()), hovered: ar.collidepoint(mp), radius: Some(6), border_width: Some(1), ..Default::default() });
        }
        let hy = 112;
        let hc = (140, 140, 160);
        bs.blit(&sf.render("#", true, hc), (72, hy));
        bs.blit(&sf.render("Name", true, hc), (120, hy));
        bs.blit(&sf.render(&self.get_text("table_highscore"), true, hc), (330 + shift, hy));
        bs.blit(&sf.render(&self.get_text("powers_used_label"), true, hc), (420 + shift, hy));
        let content_top = 136;
        let fade_bottom = h - 90;
        if self._lb_loading || self._lb_entries.is_none() {
            let s = sf.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, content_top + 30));
        } else if self._lb_entries.as_ref().unwrap().is_empty() {
            let k = if self._lb_friends_only && self.friends_local.is_empty() { "no_friends_yet" } else { "no_entries_yet" };
            let s = sf.render(&self.get_text(k), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, content_top + 30));
        } else {
            let mut y = content_top;
            let row_h = 38;
            for e in self._lb_entries.clone().unwrap() {
                if y + row_h > fade_bottom { break; }
                let rr = Rect::new(60, y, w - 120, row_h - 6);
                self._lb_row_rects.push((rr, e.gos("username")));
                let hov = rr.collidepoint(mp);
                draw::rect(&bs, if hov { self.get_button_hover_color() } else { self.get_panel_color() }, rr, 0, 8);
                draw::rect(&bs, (180, 180, 180), rr, 1, 8);
                let rank = e.gi("rank", 0);
                let medal = match rank { 1 => Some((255, 210, 60)), 2 => Some((200, 200, 210)), 3 => Some((205, 140, 80)), _ => None };
                if let Some(mc) = medal {
                    draw::circle(&bs, mc, (rr.x + 20, rr.centery()), 12, 0);
                    draw::circle(&bs, (0, 0, 0), (rr.x + 20, rr.centery()), 12, 1);
                    let rs = sf.render(&rank.to_string(), true, (40, 30, 10));
                    bs.blit(&rs, (rr.x + 20 - rs.get_width() / 2, rr.centery() - rs.get_height() / 2));
                } else {
                    let rs = sf.render(&format!("#{}", rank), true, self.get_text_color());
                    bs.blit(&rs, (rr.x + 12, rr.centery() - rs.get_height() / 2));
                }
                let ns = sf.render(&e.gs("name", ""), true, self.get_text_color());
                bs.blit(&ns, (rr.x + 60, rr.centery() - ns.get_height() / 2));
                let mut tag_x = rr.x + 68 + ns.get_width();
                if e.truthy("verified") { _draw_verified_badge(&bs, tag_x, rr.centery(), 8); tag_x += 16; }
                if e.truthy("clan_tag") {
                    let cs = sf.render(&format!("[{}]", e.gs("clan_tag", "")), true, (120, 160, 220));
                    bs.blit(&cs, (tag_x + 6, rr.centery() - cs.get_height() / 2));
                    tag_x += 6 + cs.get_width();
                }
                if e.truthy("username") {
                    let avail = (rr.x + 262 + shift) - (tag_x + 8);
                    if avail > 24 {
                        let us = fit_surf(&self.badge_font.render(&format!("@{}", e.gs("username", "")), true, (140, 140, 165)), avail);
                        bs.blit(&us, (tag_x + 8, rr.centery() - us.get_height() / 2));
                    }
                }
                let sc = sf.render(&py_str(e.gv("score").unwrap_or(&json!(0))), true, self.get_text_color());
                bs.blit(&sc, (rr.x + 270 + shift, rr.centery() - sc.get_height() / 2));
                let pu = e.gstrs("powers_used");
                let mut pt = if pu.is_empty() { "-".to_string() } else { pu.join(", ") };
                let max_w = rr.right() - (rr.x + 360 + shift) - 10;
                if sf.size(&pt).0 > max_w {
                    while !pt.is_empty() && sf.size(&format!("{}…", pt)).0 > max_w { pt.pop(); }
                    pt.push('…');
                }
                let ps = sf.render(&pt, true, (150, 150, 170));
                bs.blit(&ps, (rr.x + 360 + shift, rr.centery() - ps.get_height() / 2));
                y += row_h;
            }
        }
        let dock = Rect::new(60, h - 72, w - 120, 52);
        let dh = self.current_profile_name.is_some() && dock.collidepoint(mp);
        draw::rect(&bs, if dh { self.get_button_hover_color() } else { self.get_panel_color() }, dock, 0, 10);
        draw::rect(&bs, (120, 180, 255), dock, 2, 10);
        let pname = self.current_profile_name.clone().unwrap_or_else(|| "-".into());
        let dt = match self._lb_my_rank {
            None => self.get_text("loading"),
            Some((None, _)) => format!("{}: {}", pname, self.get_text("no_entries_yet")),
            Some((Some(r), s)) => format!("#{}  {}: {}", r, pname, fmt_thousands(s.unwrap_or(0))),
        };
        let ds = sf.render(&dt, true, self.get_text_color());
        bs.blit(&ds, (dock.x + 16, dock.centery() - ds.get_height() / 2));
        self.flip();
        [back, search, dleft, dright, dock, all_tab, fr_tab]
    }

    pub fn online_leaderboard_loop(&mut self) {
        let mut clock = Clock::new();
        self._lb_start_fetch();
        self._refresh_friends_local();
        let mut last_key = (self._lb_difficulty_idx, self._lb_friends_only);
        while self.in_online_leaderboard {
            let [back, search, dleft, dright, dock, all_tab, fr_tab] = self.draw_online_leaderboard();
            self._apply_friends_local_pending();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if key == K_m || self.is_back_key(key) { self.back_to_hub_from_online_leaderboard(); }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if back.collidepoint(pos) { self.back_to_hub_from_online_leaderboard(); }
                        else if search.collidepoint(pos) { self.open_player_search("in_online_leaderboard"); }
                        else if all_tab.collidepoint(pos) { self._lb_friends_only = false; self.play_sound(Snd::Click); }
                        else if fr_tab.collidepoint(pos) { self._lb_friends_only = true; self.play_sound(Snd::Click); }
                        else if dleft.collidepoint(pos) { self._lb_difficulty_idx = (self._lb_difficulty_idx + DIFFICULTIES.len() - 1) % DIFFICULTIES.len(); self.play_sound(Snd::Click); }
                        else if dright.collidepoint(pos) { self._lb_difficulty_idx = (self._lb_difficulty_idx + 1) % DIFFICULTIES.len(); self.play_sound(Snd::Click); }
                        else if dock.collidepoint(pos) && self.current_profile_username.is_some() {
                            let u = self.current_profile_username.clone().unwrap();
                            self.open_public_profile(&u, "in_online_leaderboard");
                        } else {
                            for (r, u) in self._lb_row_rects.clone() {
                                if r.collidepoint(pos) { if let Some(u) = u { self.open_public_profile(&u, "in_online_leaderboard"); break; } }
                            }
                        }
                    }
                    _ => {}
                }
            }
            let nk = (self._lb_difficulty_idx, self._lb_friends_only);
            if nk != last_key && self.in_online_leaderboard { last_key = nk; self._lb_start_fetch(); }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    // ---------------- Spielersuche ----------------
    pub fn _search_trigger(&mut self, query: &str) {
        let q = query.trim().to_string();
        if q.is_empty() { self._search_results = Some(vec![]); self._search_loading = false; return; }
        self._search_loading = true;
        self._search_request_id += 1;
        let my_id = self._search_request_id;
        self.search_players_online(&q, Some(Box::new(move |g, result| {
            if my_id != g._search_request_id { return; }
            g._search_loading = false;
            g._search_results = Some(match &result { Some(r) if r.truthy("ok") => r.garr("results"), _ => vec![] });
        })));
    }

    pub fn draw_player_search(&mut self) -> Rect {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        self._search_row_rects.clear();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let tt = self.get_text("player_search_title");
        self.draw_screen_title(&bs, &tt, 20);
        let sf = self.small_font.clone();
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), radius: Some(10), ..Default::default() });
        let bx = Rect::new(w / 2 - 220, 75, 440, 46);
        draw::rect(&bs, self.get_bg_color(), bx, 0, 10);
        draw::rect(&bs, (120, 180, 255), bx, 2, 10);
        let cursor = if (self.get_ticks() / 500) % 2 == 0 { "|" } else { " " };
        let inp = self.menu_font.render(&format!("{}{}", self._search_query, cursor), true, self.get_text_color());
        bs.blit(&inp, (bx.x + 12, bx.centery() - inp.get_height() / 2));
        let content_top = 140;
        let fade_bottom = h - 45;
        if self._search_loading {
            let s = sf.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, content_top + 20));
        } else if self._search_results.as_ref().map(|r| r.is_empty()).unwrap_or(false) {
            let s = sf.render(&self.get_text("player_not_found"), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, content_top + 20));
        } else if let Some(results) = self._search_results.clone() {
            let mut y = content_top;
            let row_h = 44;
            for e in results {
                if y + row_h > fade_bottom { break; }
                let rr = Rect::new(w / 2 - 220, y, 440, row_h - 6);
                self._search_row_rects.push((rr, e.gos("username")));
                draw::rect(&bs, if rr.collidepoint(mp) { self.get_button_hover_color() } else { self.get_panel_color() }, rr, 0, 8);
                draw::rect(&bs, (180, 180, 180), rr, 1, 8);
                let ns = self.menu_font.render(&e.gs("name", ""), true, self.get_text_color());
                bs.blit(&ns, (rr.x + 14, rr.centery() - ns.get_height() / 2));
                let mut tag_x = rr.x + 20 + ns.get_width();
                if e.truthy("verified") { _draw_verified_badge(&bs, tag_x, rr.centery(), 9); tag_x += 18; }
                if e.truthy("clan_tag") {
                    let cs = sf.render(&format!("[{}]", e.gs("clan_tag", "")), true, (120, 160, 220));
                    bs.blit(&cs, (tag_x + 6, rr.centery() - cs.get_height() / 2));
                    tag_x += 6 + cs.get_width();
                }
                if e.truthy("username") {
                    let avail = (rr.right() - 10) - (tag_x + 8);
                    if avail > 24 {
                        let us = fit_surf(&self.badge_font.render(&format!("@{}", e.gs("username", "")), true, (140, 140, 165)), avail);
                        bs.blit(&us, (tag_x + 8, rr.centery() - us.get_height() / 2));
                    }
                }
                y += row_h;
            }
        }
        self.flip();
        back
    }

    pub fn player_search_loop(&mut self) {
        let mut clock = Clock::new();
        while self.in_player_search {
            let back = self.draw_player_search();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, unicode, .. } => {
                        if key == K_BACKSPACE {
                            self._search_query.pop();
                            let q = self._search_query.clone();
                            self._search_trigger(&q);
                        } else if self.is_back_key_ex(key, false) {
                            self.back_from_player_search();
                        } else if key != K_RETURN {
                            if let Some(ch) = unicode.chars().next() {
                                if (32..=126).contains(&(ch as u32)) && char_len(&self._search_query) < 20 {
                                    self._search_query.push(ch);
                                    let q = self._search_query.clone();
                                    self._search_trigger(&q);
                                }
                            }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if back.collidepoint(pos) { self.back_from_player_search(); } else {
                            for (r, u) in self._search_row_rects.clone() {
                                if r.collidepoint(pos) { if let Some(u) = u { self.open_public_profile(&u, "in_player_search"); break; } }
                            }
                        }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    // ---------------- Öffentliches Profil ----------------
    pub fn _public_profile_start_fetch(&mut self) {
        self._public_profile_loading = true;
        self._public_profile_data = None;
        let username = self._public_profile_username.clone().unwrap_or_default();
        let u2 = username.clone();
        self.fetch_public_profile(&username, Some(Box::new(move |g, result| {
            if Some(u2.clone()) != g._public_profile_username { return; }
            g._public_profile_loading = false;
            g._public_profile_data = Some(match result { Some(r) if r.truthy("ok") => r, _ => Value::Bool(false) });
        })));
    }

    pub fn _draw_section_card(&self, s: &Surface, r: Rect, title: &str, accent: (i32, i32, i32), dark: bool) -> i32 {
        draw::rect(s, self.get_panel_color(), r, 0, 14);
        draw::rect(s, if dark { (75, 75, 100) } else { (215, 215, 230) }, r, 1, 14);
        draw::rect_ex(s, accent, Rect::new(r.x, r.y, r.w, 5), RectOpts { width: 0, radius: 0, tl: 14, tr: 14, bl: -1, br: -1 });
        let ts = self.small_font.render(title, true, accent);
        s.blit(&ts, (r.x + 16, r.y + 15));
        r.y + 15 + ts.get_height() + 8
    }

    pub fn draw_public_profile(&mut self) -> (Rect, Option<Rect>, Option<Rect>, Option<Rect>) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let dark = self.is_dark_mode();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let sf = self.small_font.clone();
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), ..Default::default() });
        if self._public_profile_loading || self._public_profile_data.is_none() {
            let s = self.font.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, h / 2 - s.get_height() / 2));
            self.flip();
            return (back, None, None, None);
        }
        if self._public_profile_data == Some(Value::Bool(false)) {
            let s = self.font.render(&self.get_text("player_not_found"), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, h / 2 - s.get_height() / 2));
            self.flip();
            return (back, None, None, None);
        }
        let player = self._public_profile_data.as_ref().unwrap().gobj("player");
        let mut friend_rect = None;
        let mut message_rect = None;
        let vu = player.gos("username");
        if let Some(vu) = vu.as_ref().filter(|u| !u.is_empty() && Some(*u) != self.current_profile_username.as_ref()) {
            let is_friend = self.friends_local.iter().any(|f| f.gos("username").as_ref() == Some(vu));
            let fr = Rect::new(20, 15, 130, 35);
            let hov = fr.collidepoint(mp);
            let base = if is_friend { (200, 100, 100) } else { (100, 180, 120) };
            let hc = if is_friend { (220, 120, 120) } else { (120, 200, 140) };
            draw::rect(&bs, if hov { hc } else { base }, fr, 0, 10);
            draw::rect(&bs, (180, 180, 180), fr, 2, 10);
            let fl = sf.render(&self.get_text(if is_friend { "remove_friend_btn" } else { "add_friend_btn" }), true, (255, 255, 255));
            bs.blit(&fl, (fr.centerx() - fl.get_width() / 2, fr.centery() - fl.get_height() / 2));
            friend_rect = Some(fr);
            if is_friend {
                let mr = Rect::new(160, 15, 130, 35);
                draw::rect(&bs, if mr.collidepoint(mp) { (140, 180, 240) } else { (120, 160, 220) }, mr, 0, 10);
                draw::rect(&bs, (180, 180, 180), mr, 2, 10);
                let ml = sf.render(&self.get_text("message_btn"), true, (255, 255, 255));
                bs.blit(&ml, (mr.centerx() - ml.get_width() / 2, mr.centery() - ml.get_height() / 2));
                message_rect = Some(mr);
            } else if !self._public_profile_friend_msg.is_empty() {
                bs.blit(&sf.render(&self._public_profile_friend_msg, true, (210, 190, 90)), (20, 54));
            }
        }
        let pcn = player.gos("player_color").filter(|s| !s.is_empty()).unwrap_or_else(|| "blue".into());
        let ac = player_color(&pcn).unwrap_or(BLUE);
        let asz = 56;
        let (acx, acy) = (w / 2, 78);
        let glow = Surface::new_alpha(220, 220);
        for (r, a) in [(100, 22), (75, 30), (52, 40)] { draw::circle(&glow, (ac.0, ac.1, ac.2, a), (110, 110), r, 0); }
        bs.blit(&glow, (acx - 110, acy - 110));
        let ar = Rect::new(acx - asz / 2, acy - asz / 2, asz, asz);
        draw::rect(&bs, (100, 100, 150), Rect::new(ar.x + 4, ar.y + 4, asz, asz), 0, 12);
        let cosm = player.gobj("cosmetics");
        self.draw_avatar(&bs, ar, Some(ac), true, Some(&cosm));
        let tt = player.gos("name").filter(|s| !s.is_empty()).unwrap_or_else(|| "-".into());
        let ts = self.font.render(&tt, true, self.get_text_color());
        let shs = self.font.render(&tt, true, (100, 100, 120));
        let mut us = vu.as_ref().filter(|u| !u.is_empty()).map(|u| sf.render(&format!("@{}", u), true, (150, 150, 170)));
        let vex = if player.truthy("verified") { 34 } else { 0 };
        let mut uex = us.as_ref().map(|u| 10 + u.get_width()).unwrap_or(0);
        let mut cw = ts.get_width() + vex + uex;
        if cw > w - 20 { us = None; uex = 0; cw = ts.get_width() + vex; }
        let _ = uex;
        let ty = ar.bottom() + 12;
        let tx = w / 2 - cw / 2;
        bs.blit(&shs, (tx + 2, ty + 2));
        bs.blit(&ts, (tx, ty));
        let mut cx = tx + ts.get_width();
        if player.truthy("verified") { _draw_verified_badge(&bs, cx + 18, ty + ts.get_height() / 2, 12); cx += vex; }
        if let Some(u) = &us { bs.blit(u, (cx + 10, ty + ts.get_height() - u.get_height())); }
        let mut clan_rect = None;
        if let Some(tag) = player.gos("clan_tag").filter(|s| !s.is_empty()) {
            let tgs = sf.render(&format!("[{}]", tag), true, (120, 160, 220));
            let mut cr = Rect::new(0, 0, tgs.get_width() + 20, tgs.get_height() + 10);
            cr.set_centerx(w / 2);
            cr.y = ty + ts.get_height() + 6;
            draw::rect(&bs, if cr.collidepoint(mp) { self.get_button_hover_color() } else { self.get_panel_color() }, cr, 0, 8);
            draw::rect(&bs, (120, 160, 220), cr, 1, 8);
            bs.blit(&tgs, (cr.centerx() - tgs.get_width() / 2, cr.centery() - tgs.get_height() / 2));
            clan_rect = Some(cr);
        }
        let grid_top = clan_rect.map(|c| c.bottom() + 14).unwrap_or(ty + ts.get_height() + 22);
        let gap = 20;
        let col_w = (w - 80 - gap) / 2;
        let row_h = (h - grid_top - 30 - gap) / 2;
        let (lx, rx) = (40, 40 + col_w + gap);
        let r2y = grid_top + row_h + gap;
        let stats_r = Rect::new(lx, grid_top, col_w, row_h);
        let pow_r = Rect::new(rx, grid_top, col_w, row_h);
        let sc_r = Rect::new(lx, r2y, col_w, row_h);
        let co_r = Rect::new(rx, r2y, col_w, row_h);
        let ps = player.gobj("stats");
        let ctop = self._draw_section_card(&bs, stats_r, &self.get_text("stats"), (100, 170, 255), dark);
        let rows = [
            (self.get_text("coins"), fmt_thousands(ps.gi("coins", 0))),
            (self.get_text("rounds_played"), py_str(ps.gv("total_rounds").unwrap_or(&json!(0)))),
            (self.get_text("1ups_collected"), py_str(ps.gv("total_1ups").unwrap_or(&json!(0)))),
        ];
        let mut y = ctop;
        for (l, v) in rows.iter() {
            bs.blit(&sf.render(&format!("{}: {}", l, v), true, self.get_text_color()), (stats_r.x + 16, y));
            y += 24;
        }
        bs.blit(&sf.render(&format!("{}: {}", self.get_text("play_time_label"), _format_playtime(ps.gi("total_time_played", 0))), true, self.get_text_color()), (stats_r.x + 16, y));
        let ctop = self._draw_section_card(&bs, pow_r, &self.get_text("owned_powers_label"), (255, 190, 80), dark);
        let op = player.gstrs("owned_powers");
        self._draw_wrapped_text(&if op.is_empty() { "-".into() } else { op.join(", ") }, pow_r.x + 16, ctop, col_w - 32, &sf, self.get_text_color(), 4);
        let ctop = self._draw_section_card(&bs, sc_r, &self.get_text("stat_highscores"), (140, 220, 140), dark);
        let mut sbd = std::collections::HashMap::new();
        for s in player.garr("scores") { if s.gs("mode", "") == "standard" { sbd.insert(s.gs("difficulty", ""), s.gi("score", 0)); } }
        let mut y = ctop;
        for diff in DIFFICULTIES {
            if y + 19 > sc_r.bottom() - 8 { break; }
            let st = sbd.get(diff).map(|v| fmt_thousands(*v)).unwrap_or_else(|| "-".into());
            bs.blit(&sf.render(&format!("{}: {}", self.get_difficulty_display_name(diff), st), true, self.get_text_color()), (sc_r.x + 16, y));
            y += 19;
        }
        let ctop = self._draw_section_card(&bs, co_r, &self.get_text("owned_cosmetics_label"), (220, 140, 220), dark);
        let names: Vec<String> = cosm.gstrs("owned").iter().map(|c| self.get_cosmetic_name(c)).collect();
        self._draw_wrapped_text(&if names.is_empty() { "-".into() } else { names.join(", ") }, co_r.x + 16, ctop, col_w - 32, &sf, self.get_text_color(), 4);
        self.flip();
        (back, friend_rect, message_rect, clan_rect)
    }

    pub fn _add_friend_local(&mut self, username: &str, name: &str) {
        if !self.friends_local.iter().any(|f| f.gos("username").as_deref() == Some(username)) {
            self.friends_local.push(json!({"username": username, "name": name}));
            self.save_current_profile();
        }
    }
    pub fn _remove_friend_local(&mut self, username: &str) {
        if self.friends_local.iter().any(|f| f.gos("username").as_deref() == Some(username)) {
            self.friends_local.retain(|f| f.gos("username").as_deref() != Some(username));
            self.save_current_profile();
        }
    }
    pub fn _send_friend_request(&mut self, username: &str, name: &str) {
        self._public_profile_friend_msg.clear();
        self.play_sound(Snd::Click);
        let (u, n) = (username.to_string(), name.to_string());
        self.send_friend_request_online(username, Some(Box::new(move |g, result| {
            match &result {
                Some(r) if r.truthy("ok") => {
                    if r.gs("status", "") == "accepted" { g._friend_action_pending = Some(format!("{}\u{0}{}", u, n)); }
                    else { g._public_profile_friend_msg = g.get_text("friend_request_sent_msg"); }
                }
                _ => {
                    let err = result.as_ref().and_then(|r| r.gos("error")).unwrap_or_default();
                    g._public_profile_friend_msg = match err.as_str() {
                        "already_friends" => g.get_text("already_friends_msg"),
                        "target_not_found" => g.get_text("player_not_found"),
                        _ => g.get_text("generic_error_msg"),
                    };
                }
            }
        })));
    }
    pub fn _remove_friend(&mut self, username: &str) {
        self._remove_friend_local(username);
        self.play_sound(Snd::Click);
        self.remove_friend_online(username, None);
    }

    pub fn public_profile_loop(&mut self) {
        let mut clock = Clock::new();
        self._public_profile_start_fetch();
        while self.in_public_profile {
            let (back, fr, mr, cr) = self.draw_public_profile();
            if let Some(p) = self._friend_action_pending.take() {
                let mut it = p.splitn(2, '\u{0}');
                let (u, n) = (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string());
                self._add_friend_local(&u, &n);
            }
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => { if key == K_m || self.is_back_key(key) { self.back_from_public_profile(); } }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        let player = self._public_profile_data.as_ref().map(|d| d.gobj("player")).unwrap_or_default();
                        if back.collidepoint(pos) { self.back_from_public_profile(); }
                        else if fr.map(|r| r.collidepoint(pos)).unwrap_or(false) {
                            if let Some(u) = player.gos("username").filter(|s| !s.is_empty()) {
                                if self.friends_local.iter().any(|f| f.gos("username").as_ref() == Some(&u)) { self._remove_friend(&u); }
                                else { let n = player.gs("name", ""); self._send_friend_request(&u, &n); }
                            }
                        } else if mr.map(|r| r.collidepoint(pos)).unwrap_or(false) {
                            if let Some(u) = player.gos("username").filter(|s| !s.is_empty()) {
                                let n = player.gos("name");
                                self.open_chat("dm", &u, "in_public_profile", n);
                            }
                        } else if cr.map(|r| r.collidepoint(pos)).unwrap_or(false) {
                            if let Some(t) = player.gos("clan_tag").filter(|s| !s.is_empty()) { self.open_clan_view(&t, "in_public_profile"); }
                        }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    // ---------------- Username-Gate ----------------
    pub fn _username_gate_start_check(&mut self) {
        self._username_gate_checking = true;
        self._username_gate_reachable = None;
        self.ping_server_online(Some(Box::new(|g, result| {
            g._username_gate_reachable = Some(result.map(|r| r.truthy("ok")).unwrap_or(false));
        })));
    }

    pub fn _confirm_username_gate(&mut self) {
        let username = self._username_gate_text.trim().to_string();
        if !_is_valid_username_format(&username) { self._username_gate_msg = self.get_text("username_invalid_format_msg"); return; }
        if !self.is_name_allowed(&username) { self._username_gate_msg = self.get_text("name_filtered"); return; }
        self._username_gate_busy = true;
        self._username_gate_msg.clear();
        let u2 = username.clone();
        self.set_username_online(&username, Some(Box::new(move |g, result| {
            g._username_gate_busy = false;
            match &result {
                Some(r) if r.truthy("ok") => g._username_gate_pending_success = Some(r.gos("username").unwrap_or(u2)),
                _ => {
                    let err = result.as_ref().and_then(|r| r.gos("error")).unwrap_or_default();
                    g._username_gate_msg = match err.as_str() {
                        "username_taken" => g.get_text("username_taken_msg"),
                        "invalid_format" => g.get_text("username_invalid_format_msg"),
                        "name_filtered" => g.get_text("name_filtered"),
                        _ => g.get_text("generic_error_msg"),
                    };
                }
            }
        })));
    }

    pub fn draw_username_gate(&mut self) -> Option<Rect> {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let dark = self.is_dark_mode();
        let (pw, ph) = (560, 300);
        let px = self.WIDTH / 2 - pw / 2;
        let py = self.HEIGHT / 2 - ph / 2;
        let panel = Rect::new(px, py, pw, ph);
        draw::rect(&bs, self.get_panel_color(), panel, 0, 18);
        draw::rect(&bs, if !dark { (100, 160, 255) } else { (80, 120, 200) }, panel, 3, 18);
        let t = self.menu_font.render(&self.get_text("username_gate_title"), true, if !dark { (80, 140, 255) } else { (120, 170, 255) });
        bs.blit(&t, (px + pw / 2 - t.get_width() / 2, py + 22));
        let sf = self.small_font.clone();
        if self._username_gate_checking {
            let m = sf.render(&self.get_text("username_gate_checking_msg"), true, self.get_text_color());
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, py + 120));
            self.flip();
            return None;
        }
        let desc_bottom = self._draw_wrapped_text(&self.get_text("username_gate_desc"), px + 40, py + 70, pw - 80, &sf, self.get_text_color(), 4);
        let mp = self.get_mouse_pos_logical();
        let cursor = if (self.get_ticks() / 500) % 2 == 0 { "|" } else { " " };
        let bx = Rect::new(px + 40, (desc_bottom + 14).max(py + 140), pw - 80, 46);
        draw::rect(&bs, self.get_bg_color(), bx, 0, 10);
        draw::rect(&bs, (120, 180, 255), bx, 2, 10);
        let inp = self.menu_font.render(&format!("{}{}", self._username_gate_text, cursor), true, self.get_text_color());
        bs.blit(&inp, (bx.x + 10, bx.centery() - inp.get_height() / 2));
        if self._username_gate_busy {
            let m = sf.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, bx.bottom() + 12));
        } else if !self._username_gate_msg.is_empty() {
            let m = sf.render(&self._username_gate_msg, true, (220, 60, 60));
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, bx.bottom() + 12));
        }
        let cr = Rect::new(px + pw / 2 - 90, py + ph - 62, 180, 44);
        draw::rect(&bs, if !cr.collidepoint(mp) { (60, 180, 100) } else { (80, 210, 120) }, cr, 0, 12);
        let cl = sf.render(&self.get_text("confirm"), true, (255, 255, 255));
        bs.blit(&cl, (cr.centerx() - cl.get_width() / 2, cr.centery() - cl.get_height() / 2));
        self.flip();
        Some(cr)
    }

    pub fn username_gate_loop(&mut self) {
        let mut clock = Clock::new();
        self._username_gate_start_check();
        while self.in_username_gate {
            let cr = self.draw_username_gate();
            if self._username_gate_checking {
                if let Some(reach) = self._username_gate_reachable {
                    self._username_gate_checking = false;
                    if !reach { self.in_username_gate = false; self.in_menu = true; }
                }
            }
            if let Some(u) = self._username_gate_pending_success.take() {
                self.current_profile_username = Some(u);
                self.save_current_profile();
                self.in_username_gate = false;
                self.in_menu = true;
            }
            for ev in self.event_get() {
                if let Event::Quit = ev { self.quit(); }
                if self._username_gate_checking { continue; }
                match ev {
                    Event::KeyDown { key, unicode, .. } => {
                        if key == K_RETURN { self._confirm_username_gate(); }
                        else if key == K_BACKSPACE { self._username_gate_text.pop(); }
                        else if let Some(ch) = unicode.chars().next() {
                            if (32..=126).contains(&(ch as u32)) && char_len(&self._username_gate_text) < 20 { self._username_gate_text.push(ch); }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if cr.map(|r| r.collidepoint(pos)).unwrap_or(false) { self._confirm_username_gate(); }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    pub fn _route_to_menu_or_gate(&mut self) {
        if self.current_profile_username.is_none() {
            self.in_username_gate = true;
            self._username_gate_reset();
        } else {
            self.in_menu = true;
        }
    }

    pub fn _gate_blocks_online_access(&mut self) -> bool {
        if self.current_profile_username.is_none() && self._server_reachable == Some(true) {
            self.in_username_gate = true;
            self._username_gate_reset();
            return true;
        }
        false
    }

    pub fn _username_gate_reset(&mut self) {
        self._username_gate_checking = true;
        self._username_gate_reachable = None;
        self._username_gate_text.clear();
        self._username_gate_msg.clear();
        self._username_gate_busy = false;
        self._username_gate_pending_success = None;
    }
}
