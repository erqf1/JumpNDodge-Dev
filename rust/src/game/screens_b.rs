//! Erfolge, Overlays/Popups, Steuerung, Einstellungen (Python 9649-10390, 11380-11485, 13882-14020).

use super::core::*;
use super::state::{Overlay, Snd, G};
use crate::data::d;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

pub const ACHIEVEMENT_CATS: [(&str, &[&str]); 7] = [
    ("🎯 Score", &["score_10", "score_50", "score_100", "score_200", "score_500", "score_1000"]),
    ("🔁 Runden", &["first_round", "five_rounds", "ten_rounds", "twenty_rounds", "fifty_rounds"]),
    ("🔥 Hardcore", &["hardcore_start", "hardcore_survivor"]),
    ("Unmöglich", &["impossible_start", "impossible_survivor"]),
    ("💚 Pickups", &["pickup_1up", "pickup_3x_1up", "pickup_10x_1up"]),
    ("💰 Coins", &["coins_100", "coins_500", "coins_1000"]),
    ("🏆 Bonus", &["no_damage_run", "speed_demon"]),
];
const ACH_POPUP_DURATION: f64 = 3.5;

pub fn ach_defined(code: &str) -> bool { d("achievement_defs").gv(code).is_some() }
pub fn ach_coins(code: &str) -> i64 { d("achievement_defs").gv(code).and_then(|v| v.as_array()).map(|a| as_i(&a[1]).unwrap_or(0)).unwrap_or(0) }

#[derive(Clone)]
pub struct AchPopup { pub title: String, pub coins: i64, pub start_time: f64 }

impl G {
    pub fn get_achievement_name(&self, code: &str) -> String {
        match d("achievement_defs").gv(code).and_then(|v| v.as_array()) {
            Some(a) => match &a[0] {
                Value::Object(_) => a[0].gos(&self.lang()).or_else(|| a[0].gos("Deutsch")).unwrap_or_else(|| code.into()),
                other => py_str(other),
            },
            None => py_str(&json!({})),
        }
    }
    pub fn get_achievement_description(&self, code: &str) -> String {
        let desc = d("achievement_descriptions").gv(code).cloned().unwrap_or(json!({}));
        desc.gos(&self.lang()).or_else(|| desc.gos("Deutsch")).or_else(|| desc.gos("English")).unwrap_or_default()
    }
    /// check_achievements(context) - in der Python-Version vorübergehend deaktiviert (gibt [] zurück)
    pub fn check_achievements(&mut self, _ctx: &serde_json::Map<String, Value>) -> Vec<(String, String, i64)> { vec![] }

    // ---------------- Overlays ----------------
    pub fn push_overlay(&mut self, text: &str, color: (i32, i32, i32), duration: f64, pos: Option<(i32, i32)>) {
        let lang = self.lang();
        let mut t = text.to_string();
        if self.texts.get(&lang).and_then(|p| p.get(text)).is_some() { t = self.get_text(text); }
        let now = now_secs();
        self.overlays.push(Overlay { text: t, color, until: now + duration, start: now, pos });
    }
    pub fn cleanup_overlays(&mut self) {
        let now = now_secs();
        self.overlays.retain(|o| o.until > now);
    }
    pub fn push_achievement_popup(&mut self, title: &str, coins: i64) {
        self.achievement_popup_queue.push(AchPopup { title: title.into(), coins, start_time: now_secs() });
    }
    pub fn draw_achievement_popups(&mut self, surface: &Surface) {
        let now = now_secs();
        self.achievement_popup_queue.retain(|p| now - p.start_time < ACH_POPUP_DURATION);
        let Some(p) = self.achievement_popup_queue.first().cloned() else { return; };
        let age = now - p.start_time;
        let slide = (age / 0.3).min(1.0);
        let ft = ACH_POPUP_DURATION - 0.5;
        let fade = if age > ft { (1.0 - (age - ft) / 0.5).max(0.0) } else { 1.0 };
        let tf = slide * fade;
        let (pw, ph) = (300, 72);
        let px = (self.WIDTH as f64 - pw as f64 - 12.0 + (1.0 - tf) * (pw as f64 + 20.0)) as i32;
        let py = self.HEIGHT - ph - 60;
        let gv = (200.0 + 55.0 * (now * 5.0).sin()) as i32;
        let gc = (gv, (gv as f64 * 0.78) as i32, 30);
        draw::rect(surface, (22, 18, 40), Rect::new(px, py, pw, ph), 0, 10);
        draw::rect(surface, ((gv as f64 * 0.4) as i32, (gv as f64 * 0.3) as i32, 10), Rect::new(px - 3, py - 3, pw + 6, ph + 6), 2, 13);
        draw::rect(surface, gc, Rect::new(px, py, pw, ph), 2, 10);
        let sf = self.small_font.clone();
        surface.blit(&sf.render("Achievement Unlocked!", true, (255, 195, 50)), (px + 10, py + 9));
        surface.blit(&sf.render(&p.title, true, (220, 220, 255)), (px + 10, py + 38));
        if p.coins > 0 {
            let cs = sf.render(&format!("+{} Coins", p.coins), true, (255, 220, 60));
            surface.blit(&cs, (px + pw - cs.get_width() - 10, py + 38));
        }
    }
    pub fn draw_ingame_achievement_banner(&mut self, surface: &Surface) {
        let now = now_secs();
        let Some(p) = self.achievement_popup_queue.first().cloned() else { return; };
        let age = now - p.start_time;
        if age >= ACH_POPUP_DURATION { return; }
        let slide = (age / 0.25).min(1.0);
        let ft = ACH_POPUP_DURATION - 0.6;
        let fade = if age > ft { (1.0 - (age - ft) / 0.6).max(0.0) } else { 1.0 };
        let alpha = (255.0 * slide * fade) as i32;
        if alpha <= 0 { return; }
        let (bw, bh) = (340, 58);
        let bx = self.WIDTH / 2 - bw / 2;
        let by = (8.0 + (1.0 - slide) * (-(bh as f64) - 12.0)) as i32;
        let gv = (200.0 + 55.0 * (now * 5.0).sin()) as i32;
        let gc = (gv, (gv as f64 * 0.78) as i32, 30);
        let banner = Surface::new_alpha(bw, bh);
        banner.fill((22, 18, 40, alpha.min(230)));
        draw::rect(&banner, (gc.0, gc.1, gc.2, alpha), Rect::new(0, 0, bw, bh), 2, 10);
        let sf = self.small_font.clone();
        let hdr = sf.render("Achievement Unlocked!", true, (255, 195, 50));
        hdr.set_alpha(Some(alpha));
        banner.blit(&hdr, (bw / 2 - hdr.get_width() / 2, 6));
        let ts = sf.render(&p.title, true, (220, 220, 255));
        ts.set_alpha(Some(alpha));
        let mut tx = bw / 2 - ts.get_width() / 2;
        if p.coins > 0 {
            let cs = sf.render(&format!("+{}", p.coins), true, (255, 220, 60));
            cs.set_alpha(Some(alpha));
            tx = 10.max(bw / 2 - ts.get_width() / 2 - cs.get_width() / 2 - 4);
            banner.blit(&ts, (tx, 32));
            banner.blit(&cs, (tx + ts.get_width() + 8, 32));
        } else {
            banner.blit(&ts, (tx, 32));
        }
        surface.blit(&banner, (bx, by));
    }

    pub fn show_1up_text_overlay(&mut self, player_rect: Option<Rect>) {
        let pos = player_rect.map(|r| (r.centerx(), r.top() - 30));
        self.push_overlay("1-UP!", (0, 150, 0), 1.5, pos);
    }

    pub fn compute_coins(&self, score: i64, difficulty: &str, survived: f64) -> i64 {
        if self.klassik_mode { return 0; }
        if survived < MIN_SURVIVE_SECONDS_FOR_COINS as f64 { return 0; }
        match coin_rule(difficulty) {
            None => 0,
            Some(r) if r.fract() != 0.0 => (score as f64 / r).ceil() as i64,
            Some(r) => score.div_euclid(r as i64),
        }
    }

    // ---------------- Erfolge-Bildschirm ----------------
    pub fn draw_achievements_screen(&mut self) -> Rect {
        self._ach_row_rects.clear();
        let mp = self.get_mouse_pos_logical();
        let dark = self.is_dark_mode();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        self.draw_title_shadow(&bs, &self.get_text("achievements"), 20);
        self.draw_coin_display(&bs);
        let total_rows: usize = ACHIEVEMENT_CATS.iter().map(|(_, c)| c.iter().filter(|x| ach_defined(x)).count()).sum();
        let nav_total = total_rows + 1;
        let back = Rect::new(w - 120, 15, 100, 35);
        let hov = back.collidepoint(mp) || self.nav_is_focused("achievements", total_rows, nav_total);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(self.small_font.clone()), hovered: hov, radius: Some(10), ..Default::default() });
        let content_top = 75;
        let fade_top = content_top + 18;
        let fade_bottom = h - 45;
        let unl: usize = ACHIEVEMENT_CATS.iter().map(|(_, c)| c.iter().filter(|x| self.achievements.gb(x, false)).count()).sum();
        let tc: usize = ACHIEVEMENT_CATS.iter().map(|(_, c)| c.len()).sum();
        let sf = self.small_font.clone();
        let ps = sf.render(&format!("{}/{}", unl, tc), true, if dark { (180, 210, 120) } else { (60, 120, 60) });
        bs.blit(&ps, (w / 2 - ps.get_width() / 2, content_top - 20));
        let mut y = content_top - self._ach_scroll;
        bs.set_clip(Some(Rect::new(0, fade_top, w, fade_bottom - fade_top)));
        for (cat, codes) in ACHIEVEMENT_CATS.iter() {
            let uc = codes.iter().filter(|c| self.achievements.gb(c, false)).count();
            let all = uc == codes.len();
            let hc = if all { (255, 210, 50) } else if dark { (160, 160, 210) } else { (80, 80, 140) };
            let hs = sf.render(&format!("{}  {}/{}", cat, uc, codes.len()), true, hc);
            if fade_top <= y + 24 && y < fade_bottom { bs.blit(&hs, (55, y + 3)); }
            y += 28;
            for (ci, code) in codes.iter().enumerate() {
                if !ach_defined(code) { continue; }
                let unlocked = self.achievements.gb(code, false);
                let tt = self.get_achievement_name(code);
                let coins = ach_coins(code);
                let ar = Rect::new(55, y, w - 110, 32);
                let idx = self._ach_row_rects.len();
                self._ach_row_rects.push((ar, code.to_string()));
                if y + 32 > fade_top && y < fade_bottom {
                    let (bg, bc, tcol, prefix) = if unlocked {
                        (if dark { (28, 78, 48) } else { (210, 240, 220) }, (70, 190, 110), if dark { (190, 255, 210) } else { (20, 100, 50) }, "✔ ")
                    } else {
                        (if dark { (38, 38, 52) } else { (220, 220, 228) }, if dark { (65, 65, 88) } else { (180, 180, 200) }, if dark { (95, 95, 125) } else { (140, 140, 160) }, "")
                    };
                    let hr = (ar.collidepoint(mp) || self.nav_is_focused("achievements", idx, nav_total)) && self._ach_detail_code.is_none();
                    draw::rect(&bs, if hr { if dark { (45, 95, 65) } else { (195, 230, 210) } } else { bg }, ar, 0, 7);
                    draw::rect(&bs, bc, ar, 2, 7);
                    if unlocked {
                        let gv = (160.0 + 60.0 * (now_secs() * 3.0 + ci as f64 * 0.8).sin()) as i32;
                        draw::rect(&bs, (gv, (gv as f64 * 0.85) as i32, (gv as f64 * 0.3) as i32), ar, 1, 7);
                    }
                    bs.blit(&sf.render(&format!("{}{}", prefix, tt), true, tcol), (63, y + 7));
                    if coins > 0 {
                        let cl = sf.render(&format!("+{}", coins), true, if unlocked { (255, 210, 50) } else { (120, 100, 40) });
                        bs.blit(&cl, (ar.right() - cl.get_width() - 10, y + 7));
                    }
                }
                y += 36;
            }
            y += 10;
        }
        bs.set_clip(None);
        let total_h = y + self._ach_scroll - content_top;
        let vis_h = fade_bottom - fade_top;
        if total_h > vis_h {
            let sb_h = 30.max(vis_h * vis_h / total_h);
            let sb_y = fade_top + (self._ach_scroll as f64 / 1.max(total_h - vis_h) as f64 * (vis_h - sb_h) as f64) as i32;
            draw::rect(&bs, (120, 120, 150), Rect::new(w - 10, sb_y, 6, sb_h), 0, 3);
        }
        let ht = if self._ach_detail_code.is_none() { "ESC: Zurück" } else { "Klick / ESC: Schließen" };
        let hs = sf.render(ht, true, (120, 120, 140));
        bs.blit(&hs, (w / 2 - hs.get_width() / 2, h - 22));
        if let Some(c) = self._ach_detail_code.clone() { if ach_defined(&c) { self._draw_ach_detail_popup(&c); } }
        self.draw_achievement_popups(&bs);
        self.flip();
        back
    }

    pub fn _draw_ach_detail_popup(&mut self, code: &str) {
        let dark = self.is_dark_mode();
        let unlocked = self.achievements.gb(code, false);
        let tt = self.get_achievement_name(code);
        let desc = self.get_achievement_description(code);
        let coins = ach_coins(code);
        let bs = self.base_surface.clone();
        let (pw, ph) = (360, 160);
        let px = self.WIDTH / 2 - pw / 2;
        let py = self.HEIGHT / 2 - ph / 2;
        let ov = Surface::new_alpha(self.WIDTH, self.HEIGHT);
        ov.fill((0, 0, 0, 140));
        bs.blit(&ov, (0, 0));
        draw::rect(&bs, if dark { (25, 20, 48) } else { (245, 245, 255) }, Rect::new(px, py, pw, ph), 0, 14);
        let gv = (180.0 + 55.0 * (now_secs() * 4.0).sin()) as i32;
        draw::rect(&bs, if unlocked { (gv, (gv as f64 * 0.85) as i32, 40) } else { (80, 80, 110) }, Rect::new(px, py, pw, ph), 2, 14);
        let sf = self.small_font.clone();
        let st = sf.render(if unlocked { "✔ Freigeschaltet" } else { "Gesperrt" }, true, if unlocked { (80, 220, 120) } else { (160, 100, 100) });
        bs.blit(&st, (px + pw / 2 - st.get_width() / 2, py + 10));
        let ts = self.menu_font.render(&tt, true, self.get_text_color());
        bs.blit(&ts, (px + pw / 2 - ts.get_width() / 2, py + 34));
        if !desc.is_empty() {
            let mut lines = vec![];
            let mut line = String::new();
            for wd in desc.split_whitespace() {
                let test = format!("{} {}", line, wd).trim().to_string();
                if sf.size(&test).0 <= pw - 30 { line = test; } else { lines.push(line.clone()); line = wd.to_string(); }
            }
            if !line.is_empty() { lines.push(line); }
            let dc = if dark { (180, 180, 200) } else { (80, 80, 100) };
            for (i, l) in lines.iter().take(3).enumerate() {
                let ls = sf.render(l, true, dc);
                bs.blit(&ls, (px + pw / 2 - ls.get_width() / 2, py + 80 + i as i32 * 20));
            }
        }
        if coins > 0 {
            let cs = sf.render(&format!("Belohnung: +{} Coins", coins), true, if unlocked { (255, 210, 50) } else { (120, 100, 40) });
            bs.blit(&cs, (px + pw / 2 - cs.get_width() / 2, py + ph - 28));
        }
    }

    // ---------------- Steuerung ----------------
    pub fn ctrl_tab_actions(&self, tab: usize) -> Vec<&'static str> {
        match tab { 0 => STANDARD_ACTIONS.to_vec(), 1 => TWO_P_ACTIONS.to_vec(), _ => MENU_NAV_ACTIONS.to_vec() }
    }

    /// -> (key_rects, add_rects, reset, back, tab_rects)
    pub fn draw_controls_menu(&mut self) -> (Vec<(String, i32, Rect)>, Vec<(String, Rect)>, Rect, Rect, Vec<Rect>) {
        let mp = self.get_mouse_pos_logical();
        let dark = self.is_dark_mode();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let ov = Surface::new_alpha(w, h);
        ov.fill((0, 0, 0, 120));
        bs.blit(&ov, (0, 0));
        let tt = self.get_text("controls_title");
        let ts = self.font.render(&tt, true, (255, 255, 255));
        bs.blit(&self.font.render(&tt, true, (80, 80, 80)), (w / 2 - ts.get_width() / 2 + 2, 22));
        bs.blit(&ts, (w / 2 - ts.get_width() / 2, 20));
        let sf = self.small_font.clone();
        let mf = self.menu_font.clone();
        let hint = sf.render(&self.get_text("controls_hint_legend"), true, (180, 180, 200));
        bs.blit(&hint, (w / 2 - hint.get_width() / 2, 72));
        let tab_labels = [self.get_text("controls_tab_standard"), self.get_text("controls_tab_2p"), self.get_text("controls_tab_menu")];
        let (tw, th, tg) = (160, 34, 8);
        let ttw = 3 * tw + 2 * tg;
        let tx0 = w / 2 - ttw / 2;
        let mut tab_rects = vec![];
        for (ti, tl) in tab_labels.iter().enumerate() {
            let tr = Rect::new(tx0 + ti as i32 * (tw + tg), 96, tw, th);
            tab_rects.push(tr);
            let active = ti == self._ctrl_active_tab;
            let bg = if active { (70, 120, 200) } else if tr.collidepoint(mp) { self.get_button_hover_color() } else { self.get_button_color() };
            draw::rect(&bs, bg, tr, 0, 8);
            draw::rect(&bs, if active { (150, 180, 220) } else { (150, 150, 150) }, tr, 2, 8);
            let ls = sf.render(tl, true, if active { (255, 255, 255) } else { self.get_text_color() });
            bs.blit(&ls, (tr.centerx() - ls.get_width() / 2, tr.centery() - ls.get_height() / 2));
        }
        let kb = self.get_keybindings();
        let extras = self.get_menu_nav_extra_keys();
        let actions = self.ctrl_tab_actions(self._ctrl_active_tab);
        let is_menu = self._ctrl_active_tab == 2;
        let mut use_count: std::collections::HashMap<i32, i32> = Default::default();
        if !is_menu {
            for a in &actions { for k in kb.iter().find(|(x, _)| x == a).map(|(_, v)| v.clone()).unwrap_or_default() { *use_count.entry(k).or_insert(0) += 1; } }
        }
        let table_w = 760.min(w - 60);
        let table_x = w / 2 - table_w / 2;
        let row_h = 54;
        let rows_y0 = 142;
        let divider = if dark { (70, 72, 90) } else { (190, 192, 205) };
        let vt = rows_y0;
        let vb = h - 80;
        let vh = row_h.max(vb - vt);
        let content_h = actions.len() as i32 * row_h;
        let max_scroll = 0.max(content_h - vh);
        self._ctrl_scroll = self._ctrl_scroll.clamp(0, max_scroll);
        self.draw_card(&bs, Rect::new(table_x - 16, vt - 8, table_w + 36, vh + 16), 14, false, None, 3, false, None);
        let prev_clip = bs.get_clip();
        bs.set_clip(Some(Rect::new(0, vt, w, vh)));
        let mut key_rects = vec![];
        let mut add_rects = vec![];
        let arrow = |k: i32| match k { K_LEFT => Some("left"), K_RIGHT => Some("right"), K_UP => Some("up"), K_DOWN => Some("down"), _ => None };
        let arrow_pts = |dir: &str, cx: i32, cy: i32| -> Vec<(i32, i32)> {
            let s = 9;
            match dir {
                "left" => vec![(cx + s, cy - s), (cx - s, cy), (cx + s, cy + s)],
                "right" => vec![(cx - s, cy - s), (cx + s, cy), (cx - s, cy + s)],
                "up" => vec![(cx - s, cy + s), (cx, cy - s), (cx + s, cy + s)],
                _ => vec![(cx - s, cy - s), (cx, cy + s), (cx + s, cy - s)],
            }
        };
        for (i, action) in actions.iter().enumerate() {
            let ry = rows_y0 + i as i32 * row_h - self._ctrl_scroll;
            if ry + row_h < vt || ry > vb { continue; }
            let listening = self._ctrl_listening.as_deref() == Some(*action);
            if listening {
                let hl = Surface::new_alpha(table_w, row_h - 2);
                hl.fill((200, 150, 0, 40));
                bs.blit(&hl, (table_x, ry + 1));
            }
            if i > 0 { draw::line(&bs, divider, (table_x, ry), (table_x + table_w, ry), 1); }
            let rcy = ry + row_h / 2;
            let lc = if listening { (240, 200, 30) } else if dark { (255, 255, 255) } else { (20, 20, 30) };
            let lbl = mf.render(&self.get_action_label(action), true, lc);
            bs.blit(&lbl, (table_x + 14, rcy - lbl.get_height() / 2));
            let (default_keys, keys_list): (Vec<i32>, Vec<i32>) = if is_menu {
                (menu_nav_default(action).to_vec(), ints(extras.get(*action).unwrap_or(&json!([]))).iter().map(|x| *x as i32).collect())
            } else {
                (vec![], kb.iter().find(|(x, _)| x == action).map(|(_, v)| v.clone()).unwrap_or_default())
            };
            let btn_h = 36;
            let right = table_x + table_w - 10;
            let plus_w = 36;
            let pr = Rect::new(right - plus_w, rcy - btn_h / 2, plus_w, btn_h);
            let ph = pr.collidepoint(mp) || self.nav_is_focused("controls_menu", i, actions.len() + 2);
            draw::rect(&bs, if ph { (0, 160, 70) } else { (0, 110, 50) }, pr, 0, 6);
            let ps = mf.render("+", true, (255, 255, 255));
            bs.blit(&ps, (pr.centerx() - ps.get_width() / 2, pr.centery() - ps.get_height() / 2));
            add_rects.push((action.to_string(), pr));
            let mut cx = right - plus_w - 8;
            if listening {
                let ws = mf.render(&self.get_text("press_a_key"), true, (240, 200, 30));
                let ww = ws.get_width() + 24;
                let wr = Rect::new(cx - ww, rcy - btn_h / 2, ww, btn_h);
                draw::rect(&bs, (70, 55, 10), wr, 0, 8);
                draw::rect(&bs, (220, 175, 30), wr, 2, 8);
                bs.blit(&ws, (wr.centerx() - ws.get_width() / 2, wr.centery() - ws.get_height() / 2));
            } else {
                for k in keys_list.iter().rev() {
                    let ad = arrow(*k);
                    let kn = key_display_name(*k);
                    let conflict = use_count.get(k).copied().unwrap_or(0) > 1;
                    let bw = if ad.is_some() { 54 } else { 54.max(mf.size(&kn).0 + 22) };
                    let br = Rect::new(cx - bw, rcy - btn_h / 2, bw, btn_h);
                    let hv = br.collidepoint(mp);
                    let bg = if hv { (160, 40, 40) } else if conflict { (90, 65, 15) } else if dark { (55, 58, 80) } else { (195, 198, 215) };
                    let bd = if hv { (220, 80, 80) } else if conflict { (235, 165, 40) } else if dark { (100, 105, 135) } else { (150, 155, 178) };
                    let sc = if hv { (255, 180, 180) } else if conflict { (255, 210, 140) } else if dark { (230, 230, 240) } else { (20, 20, 30) };
                    draw::rect(&bs, bg, br, 0, 7);
                    draw::rect(&bs, bd, br, 2, 7);
                    if conflict && !hv {
                        let wn = sf.render("⚠", true, (255, 200, 60));
                        bs.blit(&wn, (br.right() - wn.get_width() - 1, br.top() - wn.get_height() + 4));
                    }
                    if let Some(dir) = ad { draw::polygon(&bs, sc, &arrow_pts(dir, br.centerx(), br.centery()), 0); }
                    else { let ks = mf.render(&kn, true, sc); bs.blit(&ks, (br.centerx() - ks.get_width() / 2, br.centery() - ks.get_height() / 2)); }
                    key_rects.push((action.to_string(), *k, br));
                    cx -= bw + 6;
                }
                for k in default_keys.iter().rev() {
                    let ad = arrow(*k);
                    let kn = key_display_name(*k);
                    let bw = if ad.is_some() { 54 } else { 54.max(mf.size(&kn).0 + 22) };
                    let br = Rect::new(cx - bw, rcy - btn_h / 2, bw, btn_h);
                    let (bg, bd, sc) = if dark { ((45, 47, 60), (90, 92, 110), (170, 170, 185)) } else { ((225, 225, 235), (190, 190, 200), (90, 90, 100)) };
                    draw::rect(&bs, bg, br, 0, 7);
                    draw::rect(&bs, bd, br, 2, 7);
                    if let Some(dir) = ad { draw::polygon(&bs, sc, &arrow_pts(dir, br.centerx(), br.centery()), 0); }
                    else { let ks = mf.render(&kn, true, sc); bs.blit(&ks, (br.centerx() - ks.get_width() / 2, br.centery() - ks.get_height() / 2)); }
                    cx -= bw + 6;
                }
            }
        }
        bs.set_clip(Some(prev_clip));
        if max_scroll > 0 {
            let track = Rect::new(table_x + table_w + 10, vt, 6, vh);
            draw::rect(&bs, divider, track, 0, 3);
            let sb_h = 24.max(vh * vh / content_h);
            let sb_y = vt + ((vh - sb_h) as f64 * (self._ctrl_scroll as f64 / max_scroll as f64)) as i32;
            draw::rect(&bs, if dark { (140, 145, 170) } else { (110, 115, 140) }, Rect::new(track.x, sb_y, track.w, sb_h), 0, 3);
            let sh = sf.render(&self.get_text("controls_scroll_hint"), true, if dark { (150, 150, 170) } else { (110, 110, 130) });
            bs.blit(&sh, (w / 2 - sh.get_width() / 2, vb + 4));
        }
        let by = h - 58;
        let reset = Rect::new(w / 2 - 222, by, 214, 44);
        let back = Rect::new(w / 2 + 8, by, 214, 44);
        for (bi, (r, label, accent, small)) in [(reset, self.get_text("reset_defaults"), (180, 120, 0), true), (back, self.get_text("back"), (70, 120, 200), false)].iter().enumerate() {
            let hv = r.collidepoint(mp) || self.nav_is_focused("controls_menu", actions.len() + bi, actions.len() + 2);
            let bg = if hv && bi == 0 { (80, 60, 10) } else if hv && bi == 1 { (40, 70, 140) } else if dark { (50, 52, 68) } else { (195, 198, 215) };
            draw::rect(&bs, bg, *r, 0, 8);
            draw::rect(&bs, *accent, *r, 2, 8);
            let f = if *small { &sf } else { &mf };
            let ls = f.render(label, true, if dark || hv { (255, 255, 255) } else { (20, 20, 30) });
            bs.blit(&ls, (r.centerx() - ls.get_width() / 2, r.centery() - ls.get_height() / 2));
        }
        self.flip();
        (key_rects, add_rects, reset, back, tab_rects)
    }

    // ---------------- Einstellungen ----------------
    pub fn settings_bg_toggles(&self) -> Vec<(&'static str, String)> {
        vec![("bg_stars", self.get_text("bg_stars_toggle")), ("bg_rings_enabled", self.get_text("bg_rings_toggle")), ("bg_animated", self.get_text("bg_animated_toggle")),
             ("mono_bg", self.get_text("bg_mono_toggle")), ("show_floor_line", self.get_text("bg_floor_toggle"))]
    }

    pub fn draw_footer(&self, bs: &Surface, text: &str) {
        let (w, h) = (self.WIDTH, self.HEIGHT);
        draw::rect(bs, self.get_panel_color(), Rect::new(0, h - 30, w, 30), 0, 0);
        draw::line(bs, if self.is_dark_mode() { (90, 90, 100) } else { (200, 200, 200) }, (0, h - 30), (w, h - 30), 1);
        let ft = self.small_font.render(text, true, self.get_text_color());
        bs.blit(&ft, (w / 2 - ft.get_width() / 2, h - 25));
    }

    pub fn draw_settings_menu(&mut self) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let t = self.font.render(&self.get_text("settings"), true, self.get_text_color());
        bs.blit(&self.font.render(&self.get_text("settings"), true, (100, 100, 120)), (w / 2 - t.get_width() / 2 + 2, 62));
        bs.blit(&t, (w / 2 - t.get_width() / 2, 60));
        self.draw_coin_display(&bs);
        let sf = self.small_font.clone();
        let mf = self.menu_font.clone();
        let gs = self.settings.gf("gui_scale", 1.0).clamp(0.5, 2.0);
        let (scx, sy, sw) = (w / 2, 125, 220);
        let track = Rect::new(scx - sw / 2, sy - 4, sw, 8);
        draw::rect(&bs, self.get_panel_color(), track, 0, 4);
        draw::rect(&bs, (150, 180, 220), track, 2, 4);
        let hx = (track.x as f64 + (gs - 0.5) / 1.5 * sw as f64) as i32;
        let mx = (track.x as f64 + sw as f64 / 3.0) as i32;
        draw::circle(&bs, (100, 200, 120), (mx, sy), 4, 0);
        let rt = sf.render(&self.get_text("recommended_label"), true, (100, 200, 120));
        bs.blit(&rt, (mx - rt.get_width() / 2, sy + 13));
        draw::circle(&bs, (80, 140, 220), (hx, sy), 10, 0);
        draw::circle(&bs, (200, 220, 255), (hx, sy), 10, 2);
        let lbl = mf.render(&self.get_text("gui_size_label"), true, self.get_text_color());
        bs.blit(&lbl, (scx - sw / 2 - lbl.get_width() - 10, sy - lbl.get_height() / 2));
        let vt = mf.render(&format!("{}%", (gs * 100.0) as i64), true, self.get_text_color());
        bs.blit(&vt, (scx + sw / 2 + 10, sy - vt.get_height() / 2));
        let toggles = self.settings_bg_toggles();
        let tx0 = w / 2 - (5 * 80 + 4 * 8) / 2;
        let ty = 158;
        let nav_total = toggles.len() + 6;
        let bl = mf.render(&self.get_text("background_label"), true, self.get_text_color());
        bs.blit(&bl, (tx0 - bl.get_width() - 10, ty - bl.get_height() / 2 + 13));
        for (ti, (k, l)) in toggles.iter().enumerate() {
            let active = self.settings.gb(k, *k != "mono_bg");
            let tr = Rect::new(tx0 + ti as i32 * 88, ty, 80, 26);
            let mut bc = if active { (100, 200, 120) } else { (160, 160, 160) };
            if self.nav_is_focused("settings_menu", ti, nav_total) { bc = (255, 255, 255); }
            draw::rect(&bs, if active { (70, 160, 90) } else { self.get_button_color() }, tr, 0, 8);
            draw::rect(&bs, bc, tr, 2, 8);
            let ts = sf.render(l, true, if active { (255, 255, 255) } else { self.get_text_color() });
            bs.blit(&ts, (tr.centerx() - ts.get_width() / 2, tr.centery() - ts.get_height() / 2));
        }
        let dmt = if self.is_dark_mode() { self.get_text("light_mode") } else { self.get_text("dark_mode") };
        let texts = [self.get_text("language"), dmt, self.get_text("sound"), self.get_text("controls_menu_btn"), self.get_text("more"), self.get_text("back")];
        let dmc = if self.is_dark_mode() { (0, 150, 0) } else { (150, 100, 200) };
        let colors = [(100, 200, 100), dmc, (100, 150, 200), (80, 180, 200), (200, 150, 80), (150, 150, 150)];
        for (i, (t, c)) in texts.iter().zip(colors.iter()).enumerate() {
            let r = Rect::new(w / 2 - 180, 193 + i as i32 * 60, 360, 48);
            let hv = r.collidepoint(mp) || self.nav_is_focused("settings_menu", toggles.len() + i, nav_total);
            self.draw_ui_button(&bs, r, t, BtnOpts { font: Some(mf.clone()), hovered: hv, border_color: Some(*c), hover_border_color: Some(*c), hover_border_width: Some(3), ..Default::default() });
        }
        if !self.last_export_message.is_empty() {
            let eb = Rect::new(50, h - 80, w - 100, 30);
            draw::rect(&bs, self.get_panel_color(), eb, 0, 8);
            draw::rect(&bs, (180, 180, 180), eb, 1, 8);
            bs.blit(&sf.render(&self.last_export_message, true, self.get_text_color()), (eb.x + 10, eb.y + 5));
        }
        self.cleanup_overlays();
        for o in self.overlays.clone() {
            let t = sf.render(&o.text, true, o.color);
            bs.blit(&t, ((w - t.get_width()) / 2, 100));
        }
        self.draw_achievement_popups(&bs);
        self.draw_footer(&bs, &self.get_text("back_to_main"));
        self.flip();
    }

    pub fn draw_settings_more_menu(&mut self) -> Vec<Rect> {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let w = self.WIDTH;
        let t = self.font.render("Mehr", true, self.get_text_color());
        bs.blit(&self.font.render("Mehr", true, (100, 100, 120)), (w / 2 - t.get_width() / 2 + 2, 62));
        bs.blit(&t, (w / 2 - t.get_width() / 2, 60));
        let (on, off) = (self.tde("AN", "ON"), self.tde("AUS", "OFF"));
        let on_off = |b: bool| if b { on.clone() } else { off.clone() };
        let fs = self.settings.gb("fullscreen", false);
        let lim = self.settings.gi("fps_limit", 0);
        let fps_txt = if lim > 0 { format!("{} FPS", lim) } else { format!("Monitor ({} Hz)", self.target_fps()) };
        let fp = self.settings.gb("pause_on_focus_loss", true);
        let fxo = self.settings.gb("effects", true);
        let gho = self.settings.gb("ghost", true);
        let texts = [
            format!("{}: {}", self.tde("Vollbild (F11)", "Fullscreen (F11)"), on_off(fs)),
            format!("FPS: {}", fps_txt),
            format!("{}: {}", self.tde("Auto-Pause", "Auto pause"), on_off(fp)),
            format!("{}: {}", self.tde("Effekte", "Effects"), on_off(fxo)),
            format!("{}: {}", self.tde("Rekord-Geist", "Best-run ghost"), on_off(gho)),
            format!("{}: {}", self.tde("Legacy-Grafik", "Legacy graphics"), on_off(self.settings.gb("legacy_art", false))),
            format!("{} (v{})", self.tde("Nach Updates suchen", "Check for updates"), env!("CARGO_PKG_VERSION")),
            "Experimental".to_string(), self.get_text("export_language"), self.get_text("back")];
        let colors = [(100, 160, 220), (100, 160, 220), (100, 160, 220), (100, 160, 220), (100, 160, 220), (100, 160, 220), (110, 190, 150), (200, 150, 100), (150, 100, 200), (150, 150, 150)];
        let mut rects = vec![];
        for (i, (t, c)) in texts.iter().zip(colors.iter()).enumerate() {
            let r = Rect::new(w / 2 - 180, 112 + i as i32 * 44, 360, 39);
            rects.push(r);
            let hv = r.collidepoint(mp) || self.nav_is_focused("settings_more_menu", i, texts.len());
            self.draw_ui_button(&bs, r, t, BtnOpts { font: Some(self.menu_font.clone()), hovered: hv, border_color: Some(*c), hover_border_color: Some((200, 220, 255)), hover_border_width: Some(3), ..Default::default() });
        }
        // Meldungen (z.B. Ergebnis der Update-Suche)
        self.cleanup_overlays();
        let mut y = 8;
        for o in self.overlays.clone() {
            let t = self.small_font.render(&o.text, true, o.color);
            bs.blit(&t, ((w - t.get_width()) / 2, y));
            y += t.get_height() + 4;
        }
        self.flip();
        rects
    }

    pub fn draw_experimental_menu(&mut self) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let w = self.WIDTH;
        let t = self.font.render("Experimental", true, self.get_text_color());
        bs.blit(&self.font.render("Experimental", true, (100, 100, 120)), (w / 2 - t.get_width() / 2 + 2, 62));
        bs.blit(&t, (w / 2 - t.get_width() / 2, 60));
        let r = Rect::new(w / 2 - 180, 180, 360, 48);
        let hv = r.collidepoint(mp) || self.nav_is_focused("experimental_menu", 0, 1);
        self.draw_ui_button(&bs, r, &self.get_text("back"), BtnOpts { font: Some(self.menu_font.clone()), hovered: hv, ..Default::default() });
        self.draw_footer(&bs, "ESC/M: Back to Settings");
        self.flip();
    }

    pub fn sound_volume_rows(&self) -> Vec<(String, &'static str)> {
        vec![(self.get_text("menu_click_label"), "menu_click_volume"), (self.get_text("click_label"), "click_volume"), (self.get_text("click_back_label"), "click_back_volume"),
             (self.get_text("hit_label"), "hit_volume"), (self.get_text("gameover_label"), "gameover_volume"), (self.get_text("menu_music_label"), "menu_music_volume"),
             (self.get_text("ingame_music_label"), "ingame_music_volume")]
    }

    /// -> (toggle, back, minus, plus, tracks)
    pub fn draw_settings_sound_menu(&mut self) -> (Rect, Rect, Vec<(Rect, &'static str, f64)>, Vec<(Rect, &'static str, f64)>, Vec<(Rect, String)>) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let w = self.WIDTH;
        let t = self.font.render(&self.get_text("sound"), true, self.get_text_color());
        bs.blit(&self.font.render(&self.get_text("sound"), true, (100, 100, 120)), (w / 2 - t.get_width() / 2 + 2, 62));
        bs.blit(&t, (w / 2 - t.get_width() / 2, 60));
        let (sx, sy, gap) = (w / 2 - 180, 180, 64);
        let et = format!("{}: {}", self.get_text("sound_enabled"), if self.sound_enabled { self.get_text("on") } else { self.get_text("off") });
        let toggle = Rect::new(sx, sy, 360, 48);
        let back = Rect::new(sx, sy + gap, 360, 48);
        let nav_total = 2 + 7 + 7 + self.INGAME_MUSIC_TRACKS.len();
        let mf = self.menu_font.clone();
        let sf = self.small_font.clone();
        let hv = toggle.collidepoint(mp) || self.nav_is_focused("settings_sound_menu", 0, nav_total);
        self.draw_ui_button(&bs, toggle, &et, BtnOpts { font: Some(mf.clone()), hovered: hv, ..Default::default() });
        let hv = back.collidepoint(mp) || self.nav_is_focused("settings_sound_menu", 1, nav_total);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(mf.clone()), hovered: hv, ..Default::default() });
        let mut minus = vec![];
        let mut plus = vec![];
        let mut tracks = vec![];
        let mut y = sy + 2 * gap;
        for (vi, (label, key)) in self.sound_volume_rows().into_iter().enumerate() {
            bs.blit(&sf.render(&format!("{}:", label), true, self.get_text_color()), (sx, y));
            let mr = Rect::new(sx + 150, y, 40, 30);
            let hv = mr.collidepoint(mp) || self.nav_is_focused("settings_sound_menu", 2 + vi, nav_total);
            draw::rect(&bs, if hv { self.get_button_hover_color() } else { self.get_button_color() }, mr, 0, 8);
            let m = mf.render("-", true, self.get_text_color());
            bs.blit(&m, (mr.centerx() - m.get_width() / 2, mr.centery() - m.get_height() / 2));
            minus.push((mr, key, -0.1));
            bs.blit(&sf.render(&format!("{:.1}", self.settings.gf(key, 0.8)), true, self.get_text_color()), (sx + 200, y + 5));
            let pr = Rect::new(sx + 250, y, 40, 30);
            let hv = pr.collidepoint(mp) || self.nav_is_focused("settings_sound_menu", 9 + vi, nav_total);
            draw::rect(&bs, if hv { self.get_button_hover_color() } else { self.get_button_color() }, pr, 0, 8);
            let p = mf.render("+", true, self.get_text_color());
            bs.blit(&p, (pr.centerx() - p.get_width() / 2, pr.centery() - p.get_height() / 2));
            plus.push((pr, key, 0.1));
            if key == "ingame_music_volume" {
                let chosen = self.settings.gs("ingame_music_track", "standard");
                let mut tx = sx + 300;
                for (ti, (tk, tl, _)) in self.INGAME_MUSIC_TRACKS.clone().iter().enumerate() {
                    let tr = Rect::new(tx, y, 72, 30);
                    let sel = *tk == chosen;
                    let prev = self.is_previewing_track(tk);
                    let hv = tr.collidepoint(mp) || self.nav_is_focused("settings_sound_menu", 16 + ti, nav_total);
                    let (bg, bd) = if prev { ((220, 160, 60), (255, 200, 100)) } else if sel { ((70, 160, 90), (100, 200, 120)) }
                        else { (if hv { self.get_button_hover_color() } else { self.get_button_color() }, (180, 180, 180)) };
                    draw::rect(&bs, bg, tr, 0, 8);
                    draw::rect(&bs, bd, tr, 2, 8);
                    let ts = sf.render(tl, true, if sel || prev { (255, 255, 255) } else { self.get_text_color() });
                    bs.blit(&ts, (tr.centerx() - ts.get_width() / 2, tr.centery() - ts.get_height() / 2));
                    tracks.push((tr, tk.clone()));
                    tx += 72 + 6;
                }
            }
            y += 40;
        }
        self.draw_footer(&bs, "ESC/M: Back to Settings");
        self.flip();
        (toggle, back, minus, plus, tracks)
    }

    pub fn open_stats_from_profile_hub(&mut self) { self.in_profile_hub = false; self.showing_stats = true; self.play_sound(Snd::MenuClick); }
    pub fn set_more_to_admin(&mut self) { self.in_more_menu = false; self.in_admin_panel = true; self.play_sound(Snd::MenuClick); }
    pub fn open_classic_game(&mut self) { self.play_sound(Snd::MenuClick); self.in_more_menu = false; self.in_classic_game = true; }
}
