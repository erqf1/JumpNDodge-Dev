//! Online-Bildschirme Teil 3: Squad erstellen/suchen/ansehen, Chat, Zwischenablage,
//! Code einlösen, Profil umbenennen (Python 8589-9650).

use super::core::*;
use super::state::{Snd, G};
use crate::pg::*;
use crate::util::*;
use serde_json::Value;
use std::collections::HashSet;

/// Druckbares ASCII-Zeichen aus event.unicode (32..=126), sonst None
pub fn ascii_ch(unicode: &str) -> Option<char> {
    unicode.chars().next().filter(|c| (32..=126).contains(&(*c as u32)))
}

pub fn _get_clipboard_text() -> Option<String> {
    if let Some(t) = clipboard_get().map(|t| t.split('\u{0}').next().unwrap_or("").trim().to_string()).filter(|t| !t.is_empty()) {
        return Some(t);
    }
    if cfg!(target_os = "linux") {
        for cmd in [vec!["wl-paste", "--no-newline"], vec!["xclip", "-selection", "clipboard", "-o"], vec!["xsel", "--clipboard", "--output"]] {
            if let Ok(o) = std::process::Command::new(cmd[0]).args(&cmd[1..]).output() {
                if o.status.success() {
                    let t = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if !t.is_empty() { return Some(t); }
                }
            }
        }
    }
    None
}

pub fn _paste_into_field(max_len: usize) -> Option<String> {
    let clip = _get_clipboard_text()?;
    let filtered: String = clip.chars().filter(|c| (32..=126).contains(&(*c as u32))).collect();
    if filtered.is_empty() { None } else { Some(take_chars(&filtered, max_len)) }
}

pub fn _fit_text_tail(text: &str, f: &Font, max_w: i32) -> String {
    let chars: Vec<char> = text.chars().collect();
    for i in 0..chars.len() {
        let s: String = chars[i..].iter().collect();
        if f.size(&s).0 <= max_w { return s; }
    }
    text.to_string()
}

fn err_map(g: &G, result: &Option<Value>, pairs: &[(&str, &str)]) -> String {
    let err = result.as_ref().and_then(|r| r.gos("error")).unwrap_or_default();
    for (e, k) in pairs { if *e == err { return g.get_text(k); } }
    g.get_text("generic_error_msg")
}

impl G {
    fn input_panel(&self, pw: i32, ph: i32, title_key: &str) -> (Surface, i32, i32) {
        let bs = self.base_surface.clone();
        let dark = self.is_dark_mode();
        let px = self.WIDTH / 2 - pw / 2;
        let py = self.HEIGHT / 2 - ph / 2;
        let panel = Rect::new(px, py, pw, ph);
        draw::rect(&bs, self.get_panel_color(), panel, 0, 18);
        draw::rect(&bs, if !dark { (100, 160, 255) } else { (80, 120, 200) }, panel, 3, 18);
        let t = self.menu_font.render(&self.get_text(title_key), true, if !dark { (80, 140, 255) } else { (120, 170, 255) });
        bs.blit(&t, (px + pw / 2 - t.get_width() / 2, py + 22));
        (bs, px, py)
    }

    fn green_confirm(&self, bs: &Surface, r: Rect, mp: (i32, i32)) {
        draw::rect(bs, if !r.collidepoint(mp) { (60, 180, 100) } else { (80, 210, 120) }, r, 0, 12);
        let cl = self.small_font.render(&self.get_text("confirm"), true, (255, 255, 255));
        bs.blit(&cl, (r.centerx() - cl.get_width() / 2, r.centery() - cl.get_height() / 2));
    }

    fn cursor(&self) -> &'static str { if (self.get_ticks() / 500) % 2 == 0 { "|" } else { " " } }

    // ---------------- Squad erstellen ----------------
    pub fn _confirm_clan_create(&mut self) {
        let tag = self._clan_create_tag.trim().to_string();
        let name = self._clan_create_name.trim().to_string();
        if tag.is_empty() || name.is_empty() { self._clan_create_msg = self.get_text("clan_fields_required_msg"); return; }
        self._clan_create_busy = true;
        self._clan_create_msg.clear();
        self.create_clan_online(&tag, &name, Some(Box::new(|g, result| {
            g._clan_create_busy = false;
            if result.as_ref().map(|r| r.truthy("ok")).unwrap_or(false) { g._clan_create_pending = true; } else {
                g._clan_create_msg = err_map(g, &result, &[("already_in_clan", "clan_already_in_one_msg"), ("tag_taken", "clan_tag_taken_msg"),
                    ("name_taken", "clan_name_taken_msg"), ("name_filtered", "name_filtered"), ("unknown_player", "unknown_player_msg")]);
            }
        })));
    }

    pub fn draw_clan_create(&mut self) -> (Rect, Rect, Rect, Rect) {
        self.fill_bg(true);
        let (pw, ph) = (560, 320);
        let (bs, px, py) = self.input_panel(pw, ph, "create_clan_btn");
        let mp = self.get_mouse_pos_logical();
        let cur = self.cursor();
        let sf = self.small_font.clone();
        bs.blit(&sf.render(&self.get_text("clan_tag_label"), true, self.get_text_color()), (px + 40, py + 78));
        let tb = Rect::new(px + 40, py + 100, pw - 80, 44);
        draw::rect(&bs, self.get_bg_color(), tb, 0, 10);
        draw::rect(&bs, if self._clan_create_focus == "tag" { (120, 180, 255) } else { (150, 150, 150) }, tb, 2, 10);
        let ti = self.menu_font.render(&format!("{}{}", self._clan_create_tag, if self._clan_create_focus == "tag" { cur } else { "" }), true, self.get_text_color());
        bs.blit(&ti, (tb.x + 10, tb.centery() - ti.get_height() / 2));
        bs.blit(&sf.render(&self.get_text("clan_name_label"), true, self.get_text_color()), (px + 40, py + 156));
        let nb = Rect::new(px + 40, py + 178, pw - 80, 44);
        draw::rect(&bs, self.get_bg_color(), nb, 0, 10);
        draw::rect(&bs, if self._clan_create_focus == "name" { (120, 180, 255) } else { (150, 150, 150) }, nb, 2, 10);
        let ni = self.menu_font.render(&format!("{}{}", self._clan_create_name, if self._clan_create_focus == "name" { cur } else { "" }), true, self.get_text_color());
        bs.blit(&ni, (nb.x + 10, nb.centery() - ni.get_height() / 2));
        if self._clan_create_busy {
            let m = sf.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, py + 230));
        } else if !self._clan_create_msg.is_empty() {
            let m = sf.render(&self._clan_create_msg, true, (220, 60, 60));
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, py + 230));
        }
        let cr = Rect::new(px + pw / 2 - 200, py + ph - 62, 180, 44);
        let br = Rect::new(px + pw / 2 + 20, py + ph - 62, 180, 44);
        self.green_confirm(&bs, cr, mp);
        self.draw_ui_button(&bs, br, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: br.collidepoint(mp), ..Default::default() });
        self.flip();
        (tb, nb, cr, br)
    }

    pub fn clan_create_loop(&mut self) {
        let mut clock = Clock::new();
        while self.in_clan_create {
            let (tb, nb, cr, br) = self.draw_clan_create();
            if self._clan_create_pending { self._clan_create_pending = false; self.back_to_hub_from_clan_create(true); }
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, unicode, .. } => {
                        if key == K_RETURN { self._confirm_clan_create(); }
                        else if key == K_TAB { self._clan_create_focus = if self._clan_create_focus == "tag" { "name".into() } else { "tag".into() }; }
                        else if key == K_BACKSPACE { if self._clan_create_focus == "tag" { self._clan_create_tag.pop(); } else { self._clan_create_name.pop(); } }
                        else if self.is_back_key_ex(key, false) { self.play_sound(Snd::ClickBack); self.back_to_hub_from_clan_create(false); }
                        else if let Some(ch) = ascii_ch(&unicode) {
                            if self._clan_create_focus == "tag" && char_len(&self._clan_create_tag) < 5 { self._clan_create_tag.push(ch); }
                            else if self._clan_create_focus == "name" && char_len(&self._clan_create_name) < 24 { self._clan_create_name.push(ch); }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if tb.collidepoint(pos) { self._clan_create_focus = "tag".into(); }
                        else if nb.collidepoint(pos) { self._clan_create_focus = "name".into(); }
                        else if cr.collidepoint(pos) { self._confirm_clan_create(); }
                        else if br.collidepoint(pos) { self.play_sound(Snd::ClickBack); self.back_to_hub_from_clan_create(false); }
                    }
                    _ => {}
                }
            }
            clock.tick(60);
        }
    }

    pub fn open_clan_create_from_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_clan_hub = false;
        self.in_clan_create = true;
        self._clan_create_tag.clear();
        self._clan_create_name.clear();
        self._clan_create_msg.clear();
        self._clan_create_busy = false;
        self._clan_create_focus = "tag".into();
    }
    pub fn back_to_hub_from_clan_create(&mut self, entered: bool) {
        self.in_clan_create = false;
        self.in_clan_hub = true;
        if entered { self._clan_hub_refresh(); }
    }

    // ---------------- Squad-Suche ----------------
    pub fn _clan_search_refresh_pending(&mut self) {
        self.fetch_my_clan_join_requests(Some(Box::new(|g, result| {
            if let Some(r) = result { if r.truthy("ok") { g._clan_search_my_pending_tags = r.garr("requests").iter().map(|x| x.gs("tag", "")).collect(); } }
        })));
    }
    pub fn _clan_search_trigger(&mut self, query: &str) {
        let q = query.trim().to_string();
        if q.is_empty() { self._clan_search_results = Some(vec![]); self._clan_search_loading = false; return; }
        self._clan_search_loading = true;
        self._clan_search_request_id += 1;
        let my_id = self._clan_search_request_id;
        self.search_clans_online(&q, Some(Box::new(move |g, result| {
            if my_id != g._clan_search_request_id { return; }
            g._clan_search_loading = false;
            g._clan_search_results = Some(match &result { Some(r) if r.truthy("ok") => r.garr("results"), _ => vec![] });
        })));
    }
    pub fn _join_clan(&mut self, tag: &str) {
        self.play_sound(Snd::Click);
        self._clan_search_msg.clear();
        let t = tag.to_string();
        self.request_join_clan_online(tag, Some(Box::new(move |g, result| {
            if result.as_ref().map(|r| r.truthy("ok")).unwrap_or(false) {
                g._clan_search_my_pending_tags.insert(t);
                g._clan_search_msg = g.get_text("clan_request_sent_msg");
            } else {
                g._clan_search_msg = err_map(g, &result, &[("already_in_clan", "clan_already_in_one_msg"), ("clan_full", "clan_full_msg"), ("clan_not_found", "clan_not_found")]);
            }
        })));
    }

    pub fn draw_clan_search(&mut self) -> (Rect, Rect) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        self._clan_search_row_rects.clear();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        self.draw_title_shadow(&bs, &self.get_text("clan_search_title"), 20);
        let sf = self.small_font.clone();
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), radius: Some(10), ..Default::default() });
        let bx = Rect::new(w / 2 - 220, 75, 440, 46);
        draw::rect(&bs, self.get_bg_color(), bx, 0, 10);
        draw::rect(&bs, (120, 180, 255), bx, 2, 10);
        let inp = self.menu_font.render(&format!("{}{}", self._clan_search_query, self.cursor()), true, self.get_text_color());
        bs.blit(&inp, (bx.x + 12, bx.centery() - inp.get_height() / 2));
        let mut content_top = 140;
        let fade_bottom = h - 45;
        if !self._clan_search_msg.is_empty() {
            let m = sf.render(&self._clan_search_msg, true, (220, 60, 60));
            bs.blit(&m, (w / 2 - m.get_width() / 2, content_top));
            content_top += 30;
        }
        if self._clan_search_loading {
            let s = sf.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, content_top + 20));
        } else if self._clan_search_results.as_ref().map(|r| r.is_empty()).unwrap_or(false) {
            let s = sf.render(&self.get_text("clan_not_found"), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, content_top + 20));
        } else if let Some(res) = self._clan_search_results.clone() {
            let mut y = content_top;
            let row_h = 48;
            for e in res {
                if y + row_h > fade_bottom { break; }
                let tag = e.gs("tag", "");
                let rr = Rect::new(w / 2 - 240, y, 480, row_h - 6);
                self._clan_search_row_rects.push((rr, tag.clone()));
                draw::rect(&bs, if rr.collidepoint(mp) { self.get_button_hover_color() } else { self.get_panel_color() }, rr, 0, 8);
                draw::rect(&bs, (180, 180, 180), rr, 1, 8);
                let suf = if self._clan_search_my_pending_tags.contains(&tag) { format!("  ({})", self.get_text("clan_requested_label")) } else { String::new() };
                let lt = format!("[{}] {}  ({}){}", tag, e.gs("name", ""), py_str(e.gv("member_count").unwrap_or(&Value::from(0))), suf);
                let ns = self.menu_font.render(&lt, true, self.get_text_color());
                bs.blit(&ns, (rr.x + 14, rr.centery() - ns.get_height() / 2));
                y += row_h;
            }
        }
        self.flip();
        (back, bx)
    }

    pub fn clan_search_loop(&mut self) {
        let mut clock = Clock::new();
        self._clan_search_refresh_pending();
        while self.in_clan_search {
            let (back, _bx) = self.draw_clan_search();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, unicode, .. } => {
                        if key == K_BACKSPACE {
                            self._clan_search_query.pop();
                            let q = self._clan_search_query.clone();
                            self._clan_search_trigger(&q);
                        } else if self.is_back_key_ex(key, false) {
                            self.play_sound(Snd::ClickBack);
                            self.back_to_hub_from_clan_search(false);
                        } else if let Some(ch) = ascii_ch(&unicode) {
                            if char_len(&self._clan_search_query) < 30 {
                                self._clan_search_query.push(ch);
                                let q = self._clan_search_query.clone();
                                self._clan_search_trigger(&q);
                            }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if back.collidepoint(pos) { self.play_sound(Snd::ClickBack); self.back_to_hub_from_clan_search(false); } else {
                            for (r, t) in self._clan_search_row_rects.clone() { if r.collidepoint(pos) { self._join_clan(&t); break; } }
                        }
                    }
                    _ => {}
                }
            }
            clock.tick(60);
        }
    }

    pub fn open_clan_search_from_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_clan_hub = false;
        self.in_clan_search = true;
        self._clan_search_query.clear();
        self._clan_search_results = None;
        self._clan_search_msg.clear();
    }
    pub fn back_to_hub_from_clan_search(&mut self, entered: bool) {
        self.in_clan_search = false;
        self.in_clan_hub = true;
        if entered { self._clan_hub_refresh(); }
    }

    // ---------------- Squad ansehen ----------------
    pub fn _clan_view_start_fetch(&mut self) {
        self._clan_view_data = None;
        let tag = self._clan_view_tag.clone().unwrap_or_default();
        let t2 = tag.clone();
        self.fetch_clan_info(&tag, Some(Box::new(move |g, result| {
            if Some(t2.clone()) != g._clan_view_tag { return; }
            g._clan_view_data = Some(match &result { Some(r) if r.truthy("ok") => r.gv("clan").cloned().unwrap_or(Value::Null), _ => Value::Bool(false) });
        })));
    }
    pub fn _join_clan_from_view(&mut self) {
        self.play_sound(Snd::Click);
        self._clan_view_msg.clear();
        let tag = self._clan_view_tag.clone().unwrap_or_default();
        self.request_join_clan_online(&tag, Some(Box::new(|g, result| {
            if result.as_ref().map(|r| r.truthy("ok")).unwrap_or(false) { g._clan_view_msg = g.get_text("clan_request_sent_msg"); }
            else { g._clan_view_msg = err_map(g, &result, &[("already_in_clan", "clan_already_in_one_msg"), ("clan_full", "clan_full_msg")]); }
        })));
    }

    pub fn draw_clan_view(&mut self) -> (Rect, Option<Rect>) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let sf = self.small_font.clone();
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), radius: Some(10), ..Default::default() });
        let mut join = None;
        let data = match &self._clan_view_data {
            None => { let s = self.font.render(&self.get_text("loading"), true, self.get_text_color()); bs.blit(&s, (w / 2 - s.get_width() / 2, h / 2 - s.get_height() / 2)); self.flip(); return (back, None); }
            Some(Value::Bool(false)) | Some(Value::Null) => { let s = self.font.render(&self.get_text("clan_not_found"), true, self.get_text_color()); bs.blit(&s, (w / 2 - s.get_width() / 2, h / 2 - s.get_height() / 2)); self.flip(); return (back, None); }
            Some(d) => d.clone(),
        };
        self.draw_title_shadow(&bs, &format!("[{}] {}", data.gs("tag", ""), data.gs("name", "")), 20);
        let owner = data.gos("owner_username");
        let members = data.garr("members");
        let cs = sf.render(&format!("{}: {}", self.get_text("clan_members_label"), members.len()), true, (150, 150, 170));
        bs.blit(&cs, (w / 2 - cs.get_width() / 2, 66));
        let mut y = 100;
        let row_h = 40;
        for m in members {
            if y + row_h > h - 110 { break; }
            let rr = Rect::new(w / 2 - 220, y, 440, row_h - 6);
            draw::rect(&bs, self.get_panel_color(), rr, 0, 8);
            draw::rect(&bs, (180, 180, 180), rr, 1, 8);
            let un = m.gos("username").filter(|s| !s.is_empty());
            let lt = if un.is_some() && un == owner { format!("{}  ({})", m.gs("name", ""), self.get_text("clan_owner_label")) } else { m.gs("name", "") };
            let ns = sf.render(&lt, true, self.get_text_color());
            bs.blit(&ns, (rr.x + 12, rr.centery() - ns.get_height() / 2));
            y += row_h;
        }
        let already = self._clan_mine_data.as_ref().map(|d| is_truthy(d) && d.truthy("in_clan")).unwrap_or(false);
        if !already {
            let jr = Rect::new(w / 2 - 110, h - 90, 220, 44);
            self.draw_ui_button(&bs, jr, &self.get_text("join_clan_btn"), BtnOpts { font: Some(sf.clone()), hovered: jr.collidepoint(mp), radius: Some(10), border_color: Some((100, 150, 220)), hover_border_color: Some((100, 150, 220)), hover_border_width: Some(2), ..Default::default() });
            join = Some(jr);
        }
        if !self._clan_view_msg.is_empty() {
            let col = if self._clan_view_msg == self.get_text("clan_request_sent_msg") { (60, 190, 100) } else { (220, 60, 60) };
            let m = sf.render(&self._clan_view_msg, true, col);
            bs.blit(&m, (w / 2 - m.get_width() / 2, h - 34));
        }
        self.flip();
        (back, join)
    }

    pub fn clan_view_loop(&mut self) {
        let mut clock = Clock::new();
        self._clan_view_start_fetch();
        while self.in_clan_view {
            let (back, join) = self.draw_clan_view();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => { if key == K_m || self.is_back_key(key) { self.back_from_clan_view(); } }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if back.collidepoint(pos) { self.back_from_clan_view(); }
                        else if join.map(|r| r.collidepoint(pos)).unwrap_or(false) { self._join_clan_from_view(); }
                    }
                    _ => {}
                }
            }
            clock.tick(60);
        }
    }

    pub fn open_clan_view(&mut self, tag: &str, return_screen: &str) {
        self.play_sound(Snd::MenuClick);
        self.in_public_profile = false;
        self.in_clan_hub = false;
        self.in_clan_search = false;
        self.in_clan_view = true;
        self._clan_view_tag = Some(tag.to_string());
        self._clan_view_return_screen = return_screen.to_string();
        self._clan_view_msg.clear();
    }
    pub fn back_from_clan_view(&mut self) {
        self.play_sound(Snd::ClickBack);
        self.in_clan_view = false;
        if self._clan_view_return_screen == "in_clan_hub" { self.in_clan_hub = true; } else { self.in_public_profile = true; }
    }

    // ---------------- Chat ----------------
    pub fn _chat_poll(&mut self, force: bool) {
        let now = self.get_ticks();
        if !force && now - self._chat_last_poll < 3000 { return; }
        self._chat_last_poll = now;
        let (ct, tg, since) = (self._chat_conversation_type.clone(), self._chat_target.clone(), self._chat_last_id);
        let (ct2, tg2) = (ct.clone(), tg.clone());
        self.fetch_chat_messages(&ct, &tg, since, Some(Box::new(move |g, result| {
            g._chat_loading = false;
            if ct2 != g._chat_conversation_type || tg2 != g._chat_target { return; }
            match &result {
                Some(r) if r.truthy("ok") => {
                    let nm = r.garr("messages");
                    if !nm.is_empty() {
                        let mut all = g._chat_messages.clone();
                        all.extend(nm.iter().cloned());
                        let start = all.len().saturating_sub(50);
                        g._chat_messages = all[start..].to_vec();
                        g._chat_last_id = nm.iter().map(|m| m.gi("id", 0)).fold(g._chat_last_id, i64::max);
                        g.mark_chat_read_online(&ct2, &tg2, None);
                    }
                }
                Some(_) => {
                    g._chat_msg = err_map(g, &result, &[("not_friends", "chat_not_friends_msg"), ("not_clan_member", "chat_not_clan_member_msg"), ("target_not_found", "player_not_found")]);
                }
                None => {}
            }
        })));
    }

    pub fn _confirm_send_chat(&mut self) {
        let text = self._chat_input_text.trim().to_string();
        if text.is_empty() { return; }
        self._chat_input_text.clear();
        self._chat_send_busy = true;
        self._chat_msg.clear();
        let (ct, tg) = (self._chat_conversation_type.clone(), self._chat_target.clone());
        let (ct2, tg2) = (ct.clone(), tg.clone());
        self.send_chat_message_online(&ct, &tg, &text, Some(Box::new(move |g, result| {
            g._chat_send_busy = false;
            if ct2 != g._chat_conversation_type || tg2 != g._chat_target { return; }
            if result.as_ref().map(|r| r.truthy("ok")).unwrap_or(false) { g._chat_send_pending_poll = true; } else {
                g._chat_msg = err_map(g, &result, &[("message_too_long", "chat_message_too_long_msg"), ("message_filtered", "chat_message_filtered_msg"),
                    ("not_friends", "chat_not_friends_msg"), ("not_clan_member", "chat_not_clan_member_msg"), ("rate_limited", "chat_rate_limited_msg")]);
            }
        })));
    }

    pub fn draw_chat(&mut self) -> (Rect, Rect, Rect) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let sf = self.small_font.clone();
        let label = if self._chat_conversation_type == "clan" { format!("[{}]", self._chat_target) }
            else { self._chat_target_label.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| self._chat_target.clone()) };
        self.draw_title_shadow(&bs, &format!("{} - {}", self.get_text("chat_title"), label), 20);
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), radius: Some(10), ..Default::default() });
        let panel = Rect::new(60, 70, w - 120, h - 220);
        draw::rect(&bs, self.get_panel_color(), panel, 0, 12);
        draw::rect(&bs, (180, 180, 180), panel, 1, 12);
        if self._chat_loading && self._chat_messages.is_empty() {
            let s = sf.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&s, (panel.centerx() - s.get_width() / 2, panel.centery() - s.get_height() / 2));
        } else if self._chat_messages.is_empty() {
            let s = sf.render(&self.get_text("no_messages_yet"), true, self.get_text_color());
            bs.blit(&s, (panel.centerx() - s.get_width() / 2, panel.centery() - s.get_height() / 2));
        } else {
            let mut y = panel.bottom() - 12;
            for m in self._chat_messages.iter().rev() {
                let sender = m.gs("sender_name", "");
                let su = m.gos("sender_username");
                let body = m.gs("body", "");
                let is_me = if self.current_profile_username.is_some() { su == self.current_profile_username } else { Some(sender.clone()) == self.current_profile_name };
                let color = if is_me { (120, 170, 255) } else { self.get_text_color() };
                let show_un = self._chat_conversation_type == "clan" && su.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
                let ps = sf.render(&sender, true, color);
                let us = if show_un { Some(self.badge_font.render(&format!(" @{}", su.clone().unwrap()), true, (150, 150, 170))) } else { None };
                let bsf = sf.render(&format!(": {}", body), true, color);
                let lh = ps.get_height().max(bsf.get_height()) + 6;
                if y - lh < panel.top() + 10 { break; }
                y -= lh;
                let mut x = panel.x + 14;
                bs.blit(&ps, (x, y));
                x += ps.get_width();
                if let Some(u) = &us { bs.blit(u, (x, y + ps.get_height() - u.get_height())); x += u.get_width(); }
                bs.blit(&bsf, (x, y));
            }
        }
        let ib = Rect::new(60, panel.bottom() + 14, w - 220, 46);
        draw::rect(&bs, self.get_bg_color(), ib, 0, 10);
        draw::rect(&bs, (120, 180, 255), ib, 2, 10);
        let vis = _fit_text_tail(&self._chat_input_text, &sf, ib.w - 20);
        let is = sf.render(&format!("{}{}", vis, self.cursor()), true, self.get_text_color());
        bs.blit(&is, (ib.x + 10, ib.centery() - is.get_height() / 2));
        let sr = Rect::new(ib.right() + 12, ib.y, 90, 46);
        draw::rect(&bs, if sr.collidepoint(mp) { (80, 210, 120) } else { (60, 180, 100) }, sr, 0, 10);
        let sl = sf.render(&self.get_text("send_btn"), true, (255, 255, 255));
        bs.blit(&sl, (sr.centerx() - sl.get_width() / 2, sr.centery() - sl.get_height() / 2));
        if !self._chat_msg.is_empty() {
            let m = sf.render(&self._chat_msg, true, (220, 60, 60));
            bs.blit(&m, (w / 2 - m.get_width() / 2, ib.bottom() + 8));
        }
        self.flip();
        (back, ib, sr)
    }

    pub fn chat_loop(&mut self) {
        let mut clock = Clock::new();
        let (ct, tg) = (self._chat_conversation_type.clone(), self._chat_target.clone());
        self.mark_chat_read_online(&ct, &tg, None);
        while self.in_chat {
            let (back, _ib, sr) = self.draw_chat();
            if self._chat_send_pending_poll { self._chat_send_pending_poll = false; self._chat_poll(true); }
            self._chat_poll(false);
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, unicode, mods } => {
                        if key == K_RETURN { self._confirm_send_chat(); }
                        else if key == K_BACKSPACE { self._chat_input_text.pop(); }
                        else if key == K_v && (mods & KMOD_CTRL) != 0 { if let Some(p) = _paste_into_field(300) { self._chat_input_text = p; } }
                        else if self.is_back_key_ex(key, false) { self.play_sound(Snd::ClickBack); self.back_from_chat(); }
                        else if let Some(ch) = ascii_ch(&unicode) { if char_len(&self._chat_input_text) < 300 { self._chat_input_text.push(ch); } }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if back.collidepoint(pos) { self.play_sound(Snd::ClickBack); self.back_from_chat(); }
                        else if sr.collidepoint(pos) { self._confirm_send_chat(); }
                    }
                    _ => {}
                }
            }
            clock.tick(60);
        }
    }

    pub fn open_chat(&mut self, conversation_type: &str, target: &str, return_screen: &str, display_label: Option<String>) {
        self.play_sound(Snd::MenuClick);
        self.in_public_profile = false;
        self.in_clan_hub = false;
        self.in_messages_hub = false;
        self.in_chat = true;
        self._chat_conversation_type = conversation_type.to_string();
        self._chat_target = target.to_string();
        self._chat_target_label = display_label;
        self._chat_return_screen = return_screen.to_string();
        self._chat_messages.clear();
        self._chat_last_id = 0;
        self._chat_last_poll = 0;
        self._chat_input_text.clear();
        self._chat_msg.clear();
        self._chat_loading = true;
    }
    pub fn back_from_chat(&mut self) {
        self.play_sound(Snd::ClickBack);
        self.in_chat = false;
        match self._chat_return_screen.as_str() {
            "in_clan_hub" => self.in_clan_hub = true,
            "in_messages_hub" => self.in_messages_hub = true,
            _ => self.in_public_profile = true,
        }
    }

    // ---------------- Code einlösen ----------------
    pub fn _confirm_redeem_code(&mut self) {
        let text = self._redeem_code_text.trim().to_string();
        if text.is_empty() { self._redeem_code_msg = self.get_text("code_invalid_msg"); return; }
        if char_len(&text) == 99 {
            self._redeem_code_busy = true;
            self._redeem_code_msg.clear();
            self.redeem_verified_key_online(&text, Some(Box::new(|g, result| {
                g._redeem_code_busy = false;
                if result.as_ref().map(|r| r.truthy("ok")).unwrap_or(false) { g._redeem_code_msg = g.get_text("redeem_key_success"); }
                else { g._redeem_code_msg = err_map(g, &result, &[("invalid_key", "key_invalid_msg"), ("unknown_player", "unknown_player_msg")]); }
            })));
            return;
        }
        let code = text.to_uppercase();
        if self.redeemed_codes_local.contains(&code) { self._redeem_code_msg = self.get_text("code_already_redeemed_msg"); return; }
        self._redeem_code_busy = true;
        self._redeem_code_msg.clear();
        let c2 = code.clone();
        self.redeem_code_online(&code, Some(Box::new(move |g, result| {
            g._redeem_code_busy = false;
            match &result {
                Some(r) if r.truthy("ok") => {
                    let gained = r.gi("coins", 0);
                    g._redeem_code_pending = Some((c2, gained));
                    g._redeem_code_msg = format!("{} (+{} {})", g.get_text("redeem_code_success"), gained, g.get_text("coins"));
                }
                _ => g._redeem_code_msg = err_map(g, &result, &[("invalid_code", "code_invalid_msg"), ("already_redeemed", "code_already_redeemed_msg"), ("unknown_player", "unknown_player_msg")]),
            }
        })));
    }

    pub fn draw_redeem_code(&mut self) -> (Rect, Rect, Rect) {
        self.fill_bg(true);
        let (pw, ph) = (600, 260);
        let (bs, px, py) = self.input_panel(pw, ph, "redeem_code_btn");
        let sf = self.small_font.clone();
        let bx = Rect::new(px + 30, py + 90, pw - 60, 46);
        draw::rect(&bs, self.get_bg_color(), bx, 0, 10);
        draw::rect(&bs, (120, 180, 255), bx, 2, 10);
        let vis = _fit_text_tail(&self._redeem_code_text, &sf, bx.w - 20);
        let inp = sf.render(&format!("{}{}", vis, self.cursor()), true, self.get_text_color());
        bs.blit(&inp, (bx.x + 10, bx.centery() - inp.get_height() / 2));
        if self._redeem_code_busy {
            let m = sf.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, py + 150));
        } else if !self._redeem_code_msg.is_empty() {
            let ok = self._redeem_code_msg.starts_with(&self.get_text("redeem_code_success")) || self._redeem_code_msg == self.get_text("redeem_key_success");
            let m = sf.render(&self._redeem_code_msg, true, if ok { (60, 190, 100) } else { (220, 60, 60) });
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, py + 150));
        }
        let paste = Rect::new(px + 30, py + ph - 62, 170, 44);
        let conf = Rect::new(px + 215, py + ph - 62, 170, 44);
        let back = Rect::new(px + 400, py + ph - 62, 170, 44);
        let mp = self.get_mouse_pos_logical();
        self.draw_ui_button(&bs, paste, &self.get_text("paste_btn"), BtnOpts { font: Some(sf.clone()), hovered: paste.collidepoint(mp), ..Default::default() });
        self.green_confirm(&bs, conf, mp);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), ..Default::default() });
        self.flip();
        (paste, conf, back)
    }

    pub fn redeem_code_loop(&mut self) {
        let mut clock = Clock::new();
        while self.in_redeem_code {
            let (paste, conf, back) = self.draw_redeem_code();
            if let Some((code, coins)) = self._redeem_code_pending.take() {
                let c = self.stats.gi("coins", 0) + coins;
                self.stats.insert("coins".into(), Value::from(c));
                self.save_section("stats", None);
                if !self.redeemed_codes_local.contains(&code) { self.redeemed_codes_local.push(code); self.save_current_profile(); }
            }
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, unicode, mods } => {
                        if key == K_RETURN { self._confirm_redeem_code(); }
                        else if key == K_BACKSPACE { self._redeem_code_text.pop(); }
                        else if key == K_v && (mods & KMOD_CTRL) != 0 { if let Some(p) = _paste_into_field(99) { self._redeem_code_text = p; } }
                        else if self.is_back_key_ex(key, false) { self.play_sound(Snd::ClickBack); self.in_redeem_code = false; self.in_more_menu = true; }
                        else if let Some(ch) = ascii_ch(&unicode) { if char_len(&self._redeem_code_text) < 99 { self._redeem_code_text.push(ch); } }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if paste.collidepoint(pos) { if let Some(p) = _paste_into_field(99) { self._redeem_code_text = p; } self.play_sound(Snd::Click); }
                        else if conf.collidepoint(pos) { self._confirm_redeem_code(); }
                        else if back.collidepoint(pos) { self.play_sound(Snd::ClickBack); self.in_redeem_code = false; self.in_more_menu = true; }
                    }
                    _ => {}
                }
            }
            clock.tick(60);
        }
    }

    // ---------------- Umbenennen ----------------
    pub fn _confirm_rename(&mut self) {
        let nn = self._rename_text.trim().to_string();
        if nn.is_empty() { self._rename_msg = self.get_text("name_empty"); return; }
        if !self.is_name_allowed(&nn) { self._rename_msg = self.get_text("name_filtered"); return; }
        self._rename_busy = true;
        self._rename_msg.clear();
        let n2 = nn.clone();
        self.rename_profile_online(&nn, Some(Box::new(move |g, result| {
            g._rename_busy = false;
            match &result {
                Some(r) if r.truthy("ok") => { g._rename_pending_local_name = Some(r.gos("name").unwrap_or(n2)); g._rename_msg = g.get_text("rename_success"); }
                _ => g._rename_msg = err_map(g, &result, &[("name_taken", "name_taken"), ("name_filtered", "name_filtered"), ("unknown_player", "unknown_player_msg")]),
            }
        })));
    }

    pub fn draw_rename_profile(&mut self) -> (Rect, Rect) {
        self.fill_bg(true);
        let (pw, ph) = (520, 260);
        let (bs, px, py) = self.input_panel(pw, ph, "rename_btn");
        let sf = self.small_font.clone();
        let bx = Rect::new(px + 40, py + 90, pw - 80, 46);
        draw::rect(&bs, self.get_bg_color(), bx, 0, 10);
        draw::rect(&bs, (120, 180, 255), bx, 2, 10);
        let inp = self.menu_font.render(&format!("{}{}", self._rename_text, self.cursor()), true, self.get_text_color());
        bs.blit(&inp, (bx.x + 10, bx.centery() - inp.get_height() / 2));
        if self._rename_busy {
            let m = sf.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, py + 150));
        } else if !self._rename_msg.is_empty() {
            let ok = self._rename_msg == self.get_text("rename_success");
            let m = sf.render(&self._rename_msg, true, if ok { (60, 190, 100) } else { (220, 60, 60) });
            bs.blit(&m, (px + pw / 2 - m.get_width() / 2, py + 150));
        }
        let cr = Rect::new(px + pw / 2 - 200, py + ph - 62, 180, 44);
        let br = Rect::new(px + pw / 2 + 20, py + ph - 62, 180, 44);
        let mp = self.get_mouse_pos_logical();
        self.green_confirm(&bs, cr, mp);
        self.draw_ui_button(&bs, br, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: br.collidepoint(mp), ..Default::default() });
        self.flip();
        (cr, br)
    }

    pub fn rename_profile_loop(&mut self) {
        let mut clock = Clock::new();
        while self.in_rename_profile {
            let (cr, br) = self.draw_rename_profile();
            if let Some(n) = self._rename_pending_local_name.take() { self.rename_current_profile_local(&n); }
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, unicode, .. } => {
                        if key == K_RETURN { self._confirm_rename(); }
                        else if key == K_BACKSPACE { self._rename_text.pop(); }
                        else if self.is_back_key_ex(key, false) { self.play_sound(Snd::ClickBack); self.in_rename_profile = false; self.in_profile_hub = true; }
                        else if let Some(ch) = ascii_ch(&unicode) { if char_len(&self._rename_text) < 20 { self._rename_text.push(ch); } }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if cr.collidepoint(pos) { self._confirm_rename(); }
                        else if br.collidepoint(pos) { self.play_sound(Snd::ClickBack); self.in_rename_profile = false; self.in_profile_hub = true; }
                    }
                    _ => {}
                }
            }
            clock.tick(60);
        }
    }
}

pub type TagSet = HashSet<String>;
