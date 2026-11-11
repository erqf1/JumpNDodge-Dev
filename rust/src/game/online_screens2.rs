//! Online-Bildschirme Teil 2: Freunde-Hub, Online-Hub, Nachrichten-Hub, Squad/Clan-Hub
//! (Python 7700-8580).

use super::core::*;
use super::menus::fit_surf;
use super::state::{Snd, G};
use crate::pg::*;
use crate::util::*;
use serde_json::Value;

impl G {
    /// Titel mit Schatten (Muster aus fast allen Online-Screens)
    pub fn draw_title_shadow(&self, bs: &Surface, text: &str, y: i32) {
        let w = self.WIDTH;
        let ts = self.font.render(text, true, self.get_text_color());
        bs.blit(&self.font.render(text, true, (100, 100, 120)), (w / 2 - ts.get_width() / 2 + 2, y + 2));
        bs.blit(&ts, (w / 2 - ts.get_width() / 2, y));
    }

    /// Zwei Tab-Knöpfe nebeneinander (Alle/Freunde, Roster/Requests ...)
    pub fn draw_tab_pair(&self, bs: &Surface, mp: (i32, i32), tabs: &[(Rect, bool, String)]) {
        for (r, active, label) in tabs {
            let hov = r.collidepoint(mp);
            if *active {
                draw::rect(bs, self.get_button_color(), *r, 0, 10);
                draw::rect(bs, (100, 160, 255), *r, 2, 10);
            } else {
                draw::rect(bs, if hov { self.get_button_hover_color() } else { self.get_panel_color() }, *r, 0, 10);
                draw::rect(bs, (180, 180, 180), *r, 1, 10);
            }
            let t = self.small_font.render(label, true, self.get_text_color());
            bs.blit(&t, (r.centerx() - t.get_width() / 2, r.centery() - t.get_height() / 2));
        }
    }

    fn centered_msg(&self, bs: &Surface, key: &str, y: i32) {
        let s = self.small_font.render(&self.get_text(key), true, self.get_text_color());
        bs.blit(&s, (self.WIDTH / 2 - s.get_width() / 2, y));
    }

    fn accept_decline(&self, bs: &Surface, mp: (i32, i32), row: Rect) -> (Rect, Rect) {
        let dr = Rect::new(row.right() - 96, row.y + 4, 88, row.h - 8);
        let ar = Rect::new(dr.x - 96, row.y + 4, 88, row.h - 8);
        draw::rect(bs, if ar.collidepoint(mp) { (120, 200, 140) } else { (100, 180, 120) }, ar, 0, 8);
        draw::rect(bs, if dr.collidepoint(mp) { (220, 120, 120) } else { (200, 100, 100) }, dr, 0, 8);
        let al = self.small_font.render(&self.get_text("accept_btn"), true, (255, 255, 255));
        let dl = self.small_font.render(&self.get_text("decline_btn"), true, (255, 255, 255));
        bs.blit(&al, (ar.centerx() - al.get_width() / 2, ar.centery() - al.get_height() / 2));
        bs.blit(&dl, (dr.centerx() - dl.get_width() / 2, dr.centery() - dl.get_height() / 2));
        (ar, dr)
    }

    // ---------------- Freunde-Hub ----------------
    pub fn _friends_hub_refresh_requests(&mut self) {
        self._friends_hub_requests_loading = true;
        self.fetch_incoming_friend_requests(Some(Box::new(|g, result| {
            g._friends_hub_requests_loading = false;
            g._friends_hub_requests = match &result { Some(r) if r.truthy("ok") => r.garr("requests"), _ => vec![] };
        })));
    }

    pub fn _respond_to_request(&mut self, from_username: &str, from_name: &str, accept: bool) {
        self.play_sound(Snd::Click);
        self._friends_hub_requests.retain(|r| r.gos("from_username").as_deref() != Some(from_username));
        if accept { self._add_friend_local(from_username, from_name); }
        self.respond_friend_request_online(from_username, accept, None);
    }

    pub fn draw_friends_hub(&mut self) -> (Rect, Rect, Rect, Rect) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        self._friends_hub_row_rects.clear();
        self._friends_hub_request_rects.clear();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        self.draw_title_shadow(&bs, &self.get_text("friends_hub_title"), 20);
        let sf = self.small_font.clone();
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), radius: Some(10), ..Default::default() });
        let search = Rect::new(20, 15, 110, 35);
        self.draw_ui_button(&bs, search, &self.get_text("search"), BtnOpts { font: Some(sf.clone()), hovered: search.collidepoint(mp), radius: Some(10), ..Default::default() });
        let roster_tab = Rect::new(60, 65, 160, 36);
        let req_tab = Rect::new(226, 65, 160, 36);
        let mut rl = self.get_text("friends_tab_requests");
        if !self._friends_hub_requests.is_empty() { rl += &format!(" ({})", self._friends_hub_requests.len()); }
        self.draw_tab_pair(&bs, mp, &[(roster_tab, self._friends_hub_tab == "roster", self.get_text("friends_tab_roster")), (req_tab, self._friends_hub_tab == "requests", rl)]);
        let content_top = 130;
        let fade_bottom = h - 45;
        if self._friends_hub_tab == "roster" {
            if self.friends_local.is_empty() { self.centered_msg(&bs, "no_friends_yet", content_top + 20); } else {
                let mut y = content_top;
                let row_h = 44;
                let mut fl = self.friends_local.clone();
                fl.sort_by_key(|f| f.gs("name", "").to_lowercase());
                for f in fl {
                    if y + row_h > fade_bottom { break; }
                    let username = f.gos("username").filter(|s| !s.is_empty());
                    let name = f.gs("name", "");
                    let rr = Rect::new(w / 2 - 220, y, 440, row_h - 6);
                    if let Some(u) = username {
                        self._friends_hub_row_rects.push((rr, Some(u)));
                        draw::rect(&bs, if rr.collidepoint(mp) { self.get_button_hover_color() } else { self.get_panel_color() }, rr, 0, 8);
                        draw::rect(&bs, (180, 180, 180), rr, 1, 8);
                        let ns = self.menu_font.render(&name, true, self.get_text_color());
                        bs.blit(&ns, (rr.x + 14, rr.centery() - ns.get_height() / 2));
                    } else {
                        draw::rect(&bs, self.get_panel_color(), rr, 0, 8);
                        draw::rect(&bs, (110, 110, 120), rr, 1, 8);
                        let ns = self.menu_font.render(&name, true, (130, 130, 145));
                        bs.blit(&ns, (rr.x + 14, rr.centery() - ns.get_height() / 2));
                        let hs = sf.render(&self.get_text("not_migrated_hint"), true, (110, 110, 125));
                        bs.blit(&hs, (rr.right() - hs.get_width() - 14, rr.centery() - hs.get_height() / 2));
                    }
                    y += row_h;
                }
            }
        } else if self._friends_hub_requests_loading {
            self.centered_msg(&bs, "loading", content_top + 20);
        } else if self._friends_hub_requests.is_empty() {
            self.centered_msg(&bs, "no_requests_yet", content_top + 20);
        } else {
            let mut y = content_top;
            let row_h = 48;
            for req in self._friends_hub_requests.clone() {
                if y + row_h > fade_bottom { break; }
                let fname = req.gs("from_name", "");
                let funame = req.gos("from_username");
                let rr = Rect::new(w / 2 - 260, y, 520, row_h - 6);
                draw::rect(&bs, self.get_panel_color(), rr, 0, 8);
                draw::rect(&bs, (180, 180, 180), rr, 1, 8);
                let ns = self.menu_font.render(&fname, true, self.get_text_color());
                bs.blit(&ns, (rr.x + 14, rr.centery() - ns.get_height() / 2));
                let (ar, dr) = self.accept_decline(&bs, mp, rr);
                self._friends_hub_request_rects.push((ar, dr, funame.unwrap_or_default(), fname));
                y += row_h;
            }
        }
        self.flip();
        (back, roster_tab, req_tab, search)
    }

    pub fn friends_hub_loop(&mut self) {
        let mut clock = Clock::new();
        self._refresh_friends_local();
        self._friends_hub_refresh_requests();
        while self.in_friends_hub {
            let (back, roster, req, search) = self.draw_friends_hub();
            self._apply_friends_local_pending();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => { if key == K_m || self.is_back_key(key) { self.back_to_hub_from_friends_hub(); } }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if back.collidepoint(pos) { self.back_to_hub_from_friends_hub(); }
                        else if search.collidepoint(pos) { self.open_player_search("in_friends_hub"); }
                        else if roster.collidepoint(pos) { self._friends_hub_tab = "roster".into(); self.play_sound(Snd::Click); }
                        else if req.collidepoint(pos) { self._friends_hub_tab = "requests".into(); self.play_sound(Snd::Click); }
                        else if self._friends_hub_tab == "roster" {
                            for (r, u) in self._friends_hub_row_rects.clone() {
                                if r.collidepoint(pos) { if let Some(u) = u { self.open_public_profile(&u, "in_friends_hub"); } break; }
                            }
                        } else {
                            for (ar, dr, fu, fname) in self._friends_hub_request_rects.clone() {
                                if ar.collidepoint(pos) { self._respond_to_request(&fu, &fname, true); break; }
                                else if dr.collidepoint(pos) { self._respond_to_request(&fu, &fname, false); break; }
                            }
                        }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    pub fn open_friends_hub_from_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_online_hub = false;
        if self._gate_blocks_online_access() { return; }
        self.in_friends_hub = true;
        self._friends_hub_tab = "roster".into();
    }
    pub fn back_to_hub_from_friends_hub(&mut self) { self.play_sound(Snd::ClickBack); self.in_friends_hub = false; self.in_online_hub = true; }

    // ---------------- Online-Hub ----------------
    pub fn draw_online_hub(&mut self) -> Vec<Rect> {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let w = self.WIDTH;
        let tt = self.get_text("online_hub_title");
        let ts = self.font.render(&tt, true, self.get_text_color());
        bs.blit(&self.font.render(&tt, true, (100, 100, 120)), (w / 2 - ts.get_width() / 2 + 2, 22));
        bs.blit(&ts, (w / 2 - ts.get_width() / 2, 20));
        self.draw_coin_display(&bs);
        if self._server_reachable == Some(false) {
            let o = self.small_font.render(&self.get_text("offline_hint"), true, (210, 150, 70));
            bs.blit(&o, (w / 2 - o.get_width() / 2, 60));
        }
        let (sx, sy, gap) = (w / 2 - 180, 140, 56);
        let texts = [self.get_text("online_leaderboard_btn"), self.get_text("friends_hub_title"), self.get_text("clan_hub_title"), self.get_text("messages_hub_title"), self.get_text("back")];
        let colors = [(100, 180, 160), (100, 190, 130), (210, 175, 80), (100, 150, 200), (150, 150, 150)];
        let badges = [0, self._notif_pending_friend_requests, self._notif_pending_clan_requests, self._notif_unread_conversations, 0];
        let mut rects = vec![];
        for (i, (t, c)) in texts.iter().zip(colors.iter()).enumerate() {
            let r = Rect::new(sx, sy + i as i32 * gap, 360, 48);
            rects.push(r);
            let hov = r.collidepoint(mp) || self.nav_is_focused("online_hub", i, texts.len());
            self.draw_ui_button(&bs, r, t, BtnOpts { font: Some(self.menu_font.clone()), hovered: hov, border_color: Some(*c), hover_border_color: Some(*c), hover_border_width: Some(3), ..Default::default() });
            if badges[i] > 0 { self._draw_notification_badge(&bs, r.right(), r.top(), badges[i]); }
        }
        self.flip();
        rects
    }

    fn online_hub_act(&mut self, i: usize) {
        match i {
            0 => self.open_online_leaderboard_from_hub(),
            1 => self.open_friends_hub_from_hub(),
            2 => self.open_clan_hub_from_hub(),
            3 => self.open_messages_hub(),
            _ => self.back_from_online_hub(),
        }
    }

    pub fn online_hub_loop(&mut self) {
        let mut clock = Clock::new();
        while self.in_online_hub {
            self._refresh_notifications_if_due();
            let rects = self.draw_online_hub();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self.is_back_key(key) { self.back_from_online_hub(); }
                        else if let Some(d) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("online_hub", rects.len());
                            let n = nav_move(&rects, cur, d);
                            self.nav_set_focus("online_hub", n);
                        } else if self.is_nav_confirm(key) {
                            if let Some(i) = self.nav_get_focus("online_hub", rects.len()) { if i < 5 { self.online_hub_act(i); } }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        for (i, r) in rects.iter().enumerate() { if r.collidepoint(pos) { self.online_hub_act(i); break; } }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    pub fn open_online_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_menu = false;
        if self._gate_blocks_online_access() { return; }
        self.in_online_hub = true;
    }
    pub fn back_from_online_hub(&mut self) { self.play_sound(Snd::ClickBack); self.in_online_hub = false; self.in_menu = true; }

    // ---------------- Nachrichten-Hub ----------------
    pub fn _messages_hub_refresh(&mut self) {
        self._messages_hub_loading = true;
        self.fetch_conversations_list(Some(Box::new(|g, result| {
            g._messages_hub_loading = false;
            g._messages_hub_conversations = match &result { Some(r) if r.truthy("ok") => r.garr("conversations"), _ => vec![] };
        })));
    }

    pub fn draw_messages_hub(&mut self) -> Rect {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        self._messages_hub_row_rects.clear();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        self.draw_title_shadow(&bs, &self.get_text("messages_hub_title"), 20);
        let sf = self.small_font.clone();
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), radius: Some(10), ..Default::default() });
        let content_top = 80;
        let fade_bottom = h - 45;
        if self._messages_hub_loading { self.centered_msg(&bs, "loading", content_top + 20); }
        else if self._messages_hub_conversations.is_empty() { self.centered_msg(&bs, "no_conversations_yet", content_top + 20); }
        else {
            let mut y = content_top;
            let row_h = 58;
            for conv in self._messages_hub_conversations.clone() {
                if y + row_h > fade_bottom { break; }
                let label = conv.gos("label").unwrap_or_default();
                let mut preview = conv.gos("last_body").unwrap_or_default();
                if char_len(&preview) > 50 { preview = take_chars(&preview, 50) + "…"; }
                let unread = conv.truthy("unread");
                let rr = Rect::new(w / 2 - 260, y, 520, row_h - 10);
                draw::rect(&bs, if rr.collidepoint(mp) { self.get_button_hover_color() } else { self.get_panel_color() }, rr, 0, 8);
                draw::rect(&bs, if unread { (100, 160, 255) } else { (180, 180, 180) }, rr, if unread { 2 } else { 1 }, 8);
                let ns = self.menu_font.render(&label, true, self.get_text_color());
                bs.blit(&ns, (rr.x + 14, rr.y + 4));
                let pc = if unread { self.get_text_color() } else { (140, 140, 155) };
                let ps = sf.render(if preview.is_empty() { "-" } else { &preview }, true, pc);
                bs.blit(&ps, (rr.x + 14, rr.y + 6 + ns.get_height()));
                if unread { draw::circle(&bs, (220, 60, 60), (rr.right() - 16, rr.centery()), 7, 0); }
                self._messages_hub_row_rects.push((rr, conv.gs("conversation_type", ""), conv.gs("target", ""), label));
                y += row_h;
            }
        }
        self.flip();
        back
    }

    pub fn messages_hub_loop(&mut self) {
        let mut clock = Clock::new();
        self._messages_hub_refresh();
        while self.in_messages_hub {
            let back = self.draw_messages_hub();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => { if self.is_back_key(key) { self.back_from_messages_hub(); } }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if back.collidepoint(pos) { self.back_from_messages_hub(); } else {
                            for (r, ct, t, l) in self._messages_hub_row_rects.clone() {
                                if r.collidepoint(pos) { self.open_chat(&ct, &t, "in_messages_hub", Some(l)); break; }
                            }
                        }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    pub fn open_messages_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_online_hub = false;
        if self._gate_blocks_online_access() { return; }
        self.in_messages_hub = true;
    }
    pub fn back_from_messages_hub(&mut self) { self.play_sound(Snd::ClickBack); self.in_messages_hub = false; self.in_online_hub = true; }

    // ---------------- Squad/Clan-Hub ----------------
    pub fn _clan_hub_refresh(&mut self) {
        self._clan_mine_loading = true;
        self._clan_mine_data = None;
        self.fetch_my_clan(Some(Box::new(|g, result| {
            g._clan_mine_loading = false;
            g._clan_mine_data = Some(match result { Some(r) if r.truthy("ok") => r, _ => Value::Bool(false) });
        })));
    }
    pub fn _clan_hub_refresh_incoming_requests(&mut self) {
        self._clan_hub_incoming_loading = true;
        self.fetch_clan_incoming_join_requests(Some(Box::new(|g, result| {
            g._clan_hub_incoming_loading = false;
            g._clan_hub_incoming_requests = match &result { Some(r) if r.truthy("ok") => r.garr("requests"), _ => vec![] };
        })));
    }
    pub fn _clan_hub_refresh_my_pending(&mut self) {
        self.fetch_my_clan_join_requests(Some(Box::new(|g, result| {
            if let Some(r) = result { if r.truthy("ok") { g._clan_hub_my_pending_requests = r.garr("requests"); } }
        })));
    }
    pub fn _respond_to_join_request(&mut self, from_username: &str, accept: bool) {
        self.play_sound(Snd::Click);
        self._clan_hub_incoming_requests.retain(|r| r.gos("from_username").as_deref() != Some(from_username));
        self.respond_clan_join_request_online(from_username, accept, Some(Box::new(move |g, result| {
            match result {
                Some(r) if r.truthy("ok") => { if accept { g._clan_hub_refresh(); } }
                _ => g._clan_hub_msg = g.get_text("generic_error_msg"),
            }
        })));
    }
    pub fn _cancel_my_join_request(&mut self, tag: &str) {
        self.play_sound(Snd::Click);
        self._clan_hub_my_pending_requests.retain(|r| r.gos("tag").as_deref() != Some(tag));
        self.cancel_join_clan_online(tag, None);
    }
    pub fn _leave_current_clan(&mut self) {
        self._clan_hub_confirm_leave = false;
        self.play_sound(Snd::Click);
        self._clan_hub_msg.clear();
        self.leave_clan_online(Some(Box::new(|g, result| {
            match result { Some(r) if r.truthy("ok") => g._clan_hub_refresh(), _ => g._clan_hub_msg = g.get_text("generic_error_msg") }
        })));
    }
    pub fn _kick_clan_member(&mut self, username: &str) {
        self.play_sound(Snd::Click);
        self._clan_hub_msg.clear();
        self.kick_clan_member_online(username, Some(Box::new(|g, result| {
            match result { Some(r) if r.truthy("ok") => g._clan_hub_refresh(), _ => g._clan_hub_msg = g.get_text("generic_error_msg") }
        })));
    }

    pub fn _draw_confirm_modal(&mut self, message: &str) -> (Rect, Rect) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let ov = Surface::new_alpha(w, h);
        ov.fill((0, 0, 0, 160));
        bs.blit(&ov, (0, 0));
        let (pw, ph) = (440, 180);
        let (px, py) = (w / 2 - pw / 2, h / 2 - ph / 2);
        let panel = Rect::new(px, py, pw, ph);
        draw::rect(&bs, self.get_panel_color(), panel, 0, 16);
        draw::rect(&bs, (200, 100, 100), panel, 2, 16);
        let ms = self.menu_font.render(message, true, self.get_text_color());
        bs.blit(&ms, (panel.centerx() - ms.get_width() / 2, py + 40));
        let mp = self.get_mouse_pos_logical();
        let yes = Rect::new(panel.centerx() - 170, py + ph - 62, 160, 44);
        let no = Rect::new(panel.centerx() + 10, py + ph - 62, 160, 44);
        draw::rect(&bs, if yes.collidepoint(mp) { (220, 120, 120) } else { (200, 100, 100) }, yes, 0, 10);
        draw::rect(&bs, if no.collidepoint(mp) { (140, 140, 155) } else { (120, 120, 135) }, no, 0, 10);
        let yl = self.small_font.render(&self.get_text("confirm_yes_btn"), true, (255, 255, 255));
        let nl = self.small_font.render(&self.get_text("confirm_no_btn"), true, (255, 255, 255));
        bs.blit(&yl, (yes.centerx() - yl.get_width() / 2, yes.centery() - yl.get_height() / 2));
        bs.blit(&nl, (no.centerx() - nl.get_width() / 2, no.centery() - nl.get_height() / 2));
        (yes, no)
    }

    /// (back, create, join, leave, chat, roster_tab, requests_tab)
    pub fn draw_clan_hub(&mut self) -> [Option<Rect>; 7] {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        self._clan_hub_row_rects.clear();
        self._clan_hub_incoming_rects.clear();
        self._clan_hub_my_pending_rects.clear();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        self.draw_title_shadow(&bs, &self.get_text("clan_hub_title"), 20);
        let sf = self.small_font.clone();
        let back = Rect::new(w - 120, 15, 100, 35);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: back.collidepoint(mp), radius: Some(10), ..Default::default() });
        let (mut create, mut join, mut leave, mut chat, mut roster_tab, mut req_tab) = (None, None, None, None, None, None);
        if self._clan_mine_loading || self._clan_mine_data.is_none() {
            let s = self.font.render(&self.get_text("loading"), true, self.get_text_color());
            bs.blit(&s, (w / 2 - s.get_width() / 2, h / 2 - s.get_height() / 2));
            self.flip();
            return [Some(back), create, join, leave, chat, roster_tab, req_tab];
        }
        let data = self._clan_mine_data.clone().unwrap();
        let in_clan = is_truthy(&data) && data.truthy("in_clan");
        if !in_clan {
            self.centered_msg(&bs, "clan_not_in_one", 130);
            let cr = Rect::new(w / 2 - 180, 190, 360, 48);
            let jr = Rect::new(w / 2 - 180, 250, 360, 48);
            for (r, l, c) in [(cr, self.get_text("create_clan_btn"), (100, 180, 120)), (jr, self.get_text("join_clan_btn"), (100, 150, 220))] {
                self.draw_ui_button(&bs, r, &l, BtnOpts { font: Some(self.menu_font.clone()), hovered: r.collidepoint(mp), border_color: Some(c), hover_border_color: Some(c), hover_border_width: Some(3), ..Default::default() });
            }
            create = Some(cr);
            join = Some(jr);
            if !self._clan_hub_my_pending_requests.is_empty() {
                bs.blit(&sf.render(&self.get_text("clan_my_requests_label"), true, (150, 150, 170)), (w / 2 - 180, 318));
                let mut y = 344;
                let row_h = 40;
                for req in self._clan_hub_my_pending_requests.clone() {
                    if y + row_h > h - 20 { break; }
                    let tag = req.gs("tag", "");
                    let rr = Rect::new(w / 2 - 180, y, 360, row_h - 6);
                    draw::rect(&bs, self.get_panel_color(), rr, 0, 8);
                    draw::rect(&bs, (180, 180, 180), rr, 1, 8);
                    let ns = sf.render(&format!("[{}] {}", tag, req.gs("name", "")), true, self.get_text_color());
                    bs.blit(&ns, (rr.x + 10, rr.centery() - ns.get_height() / 2));
                    let cc = Rect::new(rr.right() - 76, rr.y + 3, 70, rr.h - 6);
                    draw::rect(&bs, if cc.collidepoint(mp) { (220, 120, 120) } else { (200, 100, 100) }, cc, 0, 6);
                    let cl = sf.render(&self.get_text("cancel_btn"), true, (255, 255, 255));
                    bs.blit(&cl, (cc.centerx() - cl.get_width() / 2, cc.centery() - cl.get_height() / 2));
                    self._clan_hub_my_pending_rects.push((cc, tag));
                    y += row_h;
                }
            }
        } else {
            let ts = self.menu_font.render(&format!("[{}] {}", data.gs("tag", ""), data.gs("name", "")), true, self.get_text_color());
            bs.blit(&ts, (w / 2 - ts.get_width() / 2, 100));
            let members = data.garr("members");
            let owner = data.gos("owner_username");
            let cs = sf.render(&format!("{}: {}", self.get_text("clan_members_label"), members.len()), true, (150, 150, 170));
            bs.blit(&cs, (w / 2 - cs.get_width() / 2, 138));
            let is_owner = data.gb("is_owner", false);
            let content_top;
            if is_owner {
                let rt = Rect::new(w / 2 - 164, 160, 160, 34);
                let qt = Rect::new(w / 2 + 4, 160, 160, 34);
                let mut rl = self.get_text("friends_tab_requests");
                if !self._clan_hub_incoming_requests.is_empty() { rl += &format!(" ({})", self._clan_hub_incoming_requests.len()); }
                self.draw_tab_pair(&bs, mp, &[(rt, self._clan_hub_tab == "roster", self.get_text("clan_members_label")), (qt, self._clan_hub_tab == "requests", rl)]);
                roster_tab = Some(rt);
                req_tab = Some(qt);
                content_top = 202;
            } else { content_top = 172; }
            let fade_bottom = h - 120;
            if is_owner && self._clan_hub_tab == "requests" {
                if self._clan_hub_incoming_loading { self.centered_msg(&bs, "loading", content_top + 20); }
                else if self._clan_hub_incoming_requests.is_empty() { self.centered_msg(&bs, "no_requests_yet", content_top + 20); }
                else {
                    let mut y = content_top;
                    let row_h = 48;
                    for req in self._clan_hub_incoming_requests.clone() {
                        if y + row_h > fade_bottom { break; }
                        let fu = req.gos("from_username").filter(|s| !s.is_empty());
                        let fname = req.gs("from_name", "");
                        let rr = Rect::new(w / 2 - 260, y, 520, row_h - 6);
                        draw::rect(&bs, self.get_panel_color(), rr, 0, 8);
                        draw::rect(&bs, (180, 180, 180), rr, 1, 8);
                        let ns = self.menu_font.render(&fname, true, self.get_text_color());
                        bs.blit(&ns, (rr.x + 14, rr.centery() - ns.get_height() / 2));
                        let dr0 = Rect::new(rr.right() - 96, rr.y + 4, 88, rr.h - 8);
                        let ar0 = Rect::new(dr0.x - 96, rr.y + 4, 88, rr.h - 8);
                        if let Some(u) = &fu {
                            let avail = ar0.x - 10 - (rr.x + 14 + ns.get_width() + 10);
                            if avail > 24 {
                                let us = fit_surf(&self.badge_font.render(&format!("@{}", u), true, (140, 140, 165)), avail);
                                bs.blit(&us, (rr.x + 14 + ns.get_width() + 10, rr.centery() - us.get_height() / 2));
                            }
                        }
                        let (ar, dr) = self.accept_decline(&bs, mp, rr);
                        self._clan_hub_incoming_rects.push((ar, dr, fu.unwrap_or_default(), fname));
                        y += row_h;
                    }
                }
            } else {
                let mut y = content_top;
                let row_h = 42;
                for m in members {
                    if y + row_h > fade_bottom { break; }
                    let name = m.gs("name", "");
                    let un = m.gos("username").filter(|s| !s.is_empty());
                    let rr = Rect::new(w / 2 - 240, y, 480, row_h - 6);
                    let hov = rr.collidepoint(mp) && un.is_some();
                    let tc = if un.is_some() { self.get_text_color() } else { (130, 130, 145) };
                    draw::rect(&bs, if hov { self.get_button_hover_color() } else { self.get_panel_color() }, rr, 0, 8);
                    draw::rect(&bs, if un.is_some() { (180, 180, 180) } else { (110, 110, 120) }, rr, 1, 8);
                    let lt = if un.is_some() && un == owner { format!("{}  ({})", name, self.get_text("clan_owner_label")) } else { name.clone() };
                    let ns = sf.render(&lt, true, tc);
                    bs.blit(&ns, (rr.x + 12, rr.centery() - ns.get_height() / 2));
                    let mut kick = None;
                    if is_owner && un.is_some() && un != self.current_profile_username {
                        let k = Rect::new(rr.right() - 82, rr.y + 3, 74, rr.h - 6);
                        draw::rect(&bs, if k.collidepoint(mp) { (220, 120, 120) } else { (200, 100, 100) }, k, 0, 6);
                        let kl = sf.render(&self.get_text("kick_btn"), true, (255, 255, 255));
                        bs.blit(&kl, (k.centerx() - kl.get_width() / 2, k.centery() - kl.get_height() / 2));
                        kick = Some(k);
                    }
                    if let Some(u) = &un {
                        let edge = kick.map(|k| k.x - 10).unwrap_or(rr.right() - 10);
                        let avail = edge - (rr.x + 12 + ns.get_width() + 10);
                        if avail > 24 {
                            let us = fit_surf(&self.badge_font.render(&format!("@{}", u), true, (140, 140, 165)), avail);
                            bs.blit(&us, (rr.x + 12 + ns.get_width() + 10, rr.centery() - us.get_height() / 2));
                        }
                    }
                    self._clan_hub_row_rects.push((rr, un, kick));
                    y += row_h;
                }
            }
            let chr = Rect::new(w / 2 - 240, h - 108, 220, 44);
            let lvr = Rect::new(w / 2 + 20, h - 108, 220, 44);
            self.draw_ui_button(&bs, chr, &self.get_text("chat_title"), BtnOpts { font: Some(sf.clone()), hovered: chr.collidepoint(mp), radius: Some(10), border_color: Some((120, 160, 220)), hover_border_color: Some((120, 160, 220)), hover_border_width: Some(2), ..Default::default() });
            self.draw_ui_button(&bs, lvr, &self.get_text("leave_clan_btn"), BtnOpts { font: Some(sf.clone()), hovered: lvr.collidepoint(mp), radius: Some(10), border_color: Some((200, 100, 100)), hover_border_color: Some((200, 100, 100)), hover_border_width: Some(2), ..Default::default() });
            chat = Some(chr);
            leave = Some(lvr);
        }
        if !self._clan_hub_msg.is_empty() {
            let ms = sf.render(&self._clan_hub_msg, true, (220, 60, 60));
            bs.blit(&ms, (w / 2 - ms.get_width() / 2, h - 34));
        }
        self._clan_hub_confirm_rects = if self._clan_hub_confirm_leave {
            let (y, n) = self._draw_confirm_modal(&self.get_text("confirm_leave_clan_msg"));
            (Some(y), Some(n))
        } else { (None, None) };
        self.flip();
        [Some(back), create, join, leave, chat, roster_tab, req_tab]
    }

    pub fn clan_hub_loop(&mut self) {
        let mut clock = Clock::new();
        self._clan_hub_refresh();
        self._clan_hub_refresh_my_pending();
        let hit = |r: Option<Rect>, p: (i32, i32)| r.map(|r| r.collidepoint(p)).unwrap_or(false);
        while self.in_clan_hub {
            let [back, create, join, leave, chat, rtab, qtab] = self.draw_clan_hub();
            let (cy, cn) = self._clan_hub_confirm_rects;
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self._clan_hub_confirm_leave {
                            if key == K_RETURN { self._leave_current_clan(); } else if self.is_back_key(key) { self._clan_hub_confirm_leave = false; }
                        } else if key == K_m || self.is_back_key(key) { self.back_to_hub_from_clan_hub(); }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if self._clan_hub_confirm_leave {
                            if hit(cy, pos) { self._leave_current_clan(); } else if hit(cn, pos) { self._clan_hub_confirm_leave = false; }
                            continue;
                        }
                        if hit(back, pos) { self.back_to_hub_from_clan_hub(); }
                        else if hit(create, pos) { self.open_clan_create_from_hub(); }
                        else if hit(join, pos) { self.open_clan_search_from_hub(); }
                        else if hit(chat, pos) {
                            let tag = self._clan_mine_data.as_ref().and_then(|d| d.gos("tag")).filter(|s| !s.is_empty());
                            if let Some(t) = tag { self.open_chat("clan", &t, "in_clan_hub", None); }
                        } else if hit(leave, pos) { self.play_sound(Snd::Click); self._clan_hub_confirm_leave = true; }
                        else if hit(rtab, pos) { self._clan_hub_tab = "roster".into(); self.play_sound(Snd::Click); }
                        else if hit(qtab, pos) { self._clan_hub_tab = "requests".into(); self.play_sound(Snd::Click); self._clan_hub_refresh_incoming_requests(); }
                        else if self._clan_hub_my_pending_rects.iter().any(|(r, _)| r.collidepoint(pos)) {
                            for (r, t) in self._clan_hub_my_pending_rects.clone() { if r.collidepoint(pos) { self._cancel_my_join_request(&t); break; } }
                        } else if self._clan_hub_tab == "requests" && !self._clan_hub_incoming_rects.is_empty() {
                            for (ar, dr, fu, _) in self._clan_hub_incoming_rects.clone() {
                                if ar.collidepoint(pos) { self._respond_to_join_request(&fu, true); break; }
                                else if dr.collidepoint(pos) { self._respond_to_join_request(&fu, false); break; }
                            }
                        } else {
                            for (r, u, k) in self._clan_hub_row_rects.clone() {
                                if k.map(|k| k.collidepoint(pos)).unwrap_or(false) { if let Some(u) = &u { self._kick_clan_member(u); } break; }
                                else if r.collidepoint(pos) && u.is_some() { self.open_public_profile(u.as_ref().unwrap(), "in_clan_hub"); break; }
                            }
                        }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    pub fn open_clan_hub_from_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_online_hub = false;
        if self._gate_blocks_online_access() { return; }
        self.in_clan_hub = true;
        self._clan_hub_msg.clear();
        self._clan_hub_tab = "roster".into();
        self._clan_hub_confirm_leave = false;
    }
    pub fn back_to_hub_from_clan_hub(&mut self) { self.play_sound(Snd::ClickBack); self.in_clan_hub = false; self.in_online_hub = true; }
}
