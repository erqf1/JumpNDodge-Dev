//! Shop-Menü, Hover-Tooltip, Power-Loadout, Admin-Login und Admin-Panel (Python 12829-13546).

use super::core::*;
use super::menus::fit_surf;
use super::shop::{power_rarity, powers_list};
use super::state::{Snd, G};
use crate::data::d;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

fn rar_order(r: &str) -> i32 { match r { "Common" => 0, "Rare" => 1, "Epic" => 2, "Legendary" => 3, "Mythic" => 4, _ => 9 } }

impl G {
    pub fn draw_shop_menu(&mut self) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let w = self.WIDTH;
        let t = self.font.render(&self.get_text("shop"), true, self.get_text_color());
        bs.blit(&t, (w / 2 - t.get_width() / 2, 70));
        self.draw_coin_display(&bs);
        let (sx, sy, gap) = (w / 2 - 180, 180, 70);
        let labels = [(Rect::new(sx, sy, 360, 48), "power_roll"), (Rect::new(sx, sy + gap, 360, 48), "cosmetic_shop"), (Rect::new(sx, sy + 2 * gap, 360, 48), "back")];
        for (i, (r, k)) in labels.iter().enumerate() {
            let hv = r.collidepoint(mp) || self.nav_is_focused("shop_menu", i, 3);
            draw::rect(&bs, if hv { self.get_button_hover_color() } else { self.get_button_color() }, *r, 0, 12);
            draw::rect(&bs, (180, 180, 180), *r, if hv { 3 } else { 2 }, 12);
            let ts = self.menu_font.render(&self.get_text(k), true, self.get_text_color());
            bs.blit(&ts, (r.centerx() - ts.get_width() / 2, r.centery() - ts.get_height() / 2));
        }
        self.flip();
    }

    pub fn shop_menu_loop(&mut self) {
        while self.in_shop_menu {
            let (sx, sy, gap) = (self.WIDTH / 2 - 180, 180, 70);
            let rects = [Rect::new(sx, sy, 360, 48), Rect::new(sx, sy + gap, 360, 48), Rect::new(sx, sy + 2 * gap, 360, 48)];
            let acts = ["power_roll", "cosmetic_shop", "back"];
            for ev in self.event_get() {
                let mut act: Option<&str> = None;
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if key == K_m || self.is_back_key(key) { act = Some("back"); }
                        else if let Some(dd) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("shop_menu", 3);
                            let n = nav_move(&rects, cur, dd);
                            self.nav_set_focus("shop_menu", n);
                        } else if self.is_nav_confirm(key) { act = self.nav_get_focus("shop_menu", 3).map(|i| acts[i]); }
                    }
                    Event::MouseButtonDown { .. } => {
                        let p = self.get_mouse_pos_logical();
                        for (i, r) in rects.iter().enumerate() { if r.collidepoint(p) { act = Some(acts[i]); break; } }
                    }
                    _ => {}
                }
                match act {
                    Some("power_roll") => { self.play_sound(Snd::MenuClick); self.to_power_roll(); }
                    Some("cosmetic_shop") => { self.play_sound(Snd::MenuClick); self.to_cosmetic_shop(); }
                    Some("back") => { self.play_sound(Snd::ClickBack); self.in_shop_menu = false; self.in_menu = true; }
                    _ => {}
                }
            }
            self.draw_shop_menu();
            self.clock.tick(60);
        }
    }

    /// draw_hover_tooltip(content, mouse_pos, max_width, accent_color, anchor_rect)
    /// content: eine Zeile = Text mit automatischem Umbruch, mehrere = fertige Zeilen
    pub fn draw_hover_tooltip(&mut self, content: &[String], mp: (i32, i32), max_width: i32, accent: Option<(i32, i32, i32)>, anchor: Option<Rect>) {
        if content.is_empty() || (content.len() == 1 && content[0].is_empty()) { return; }
        let tf = self.tooltip_font.clone();
        let tc = (240, 240, 245);
        let raw: Vec<String> = if content.len() == 1 {
            let mut lines = vec![];
            let mut line = String::new();
            for wd in content[0].split(' ') {
                let test = format!("{} {}", line, wd).trim().to_string();
                if !line.is_empty() && tf.size(&test).0 > max_width - 16 { lines.push(line.clone()); line = wd.to_string(); } else { line = test; }
            }
            if !line.is_empty() { lines.push(line); }
            lines
        } else { content.to_vec() };
        let surfs: Vec<Surface> = raw.iter().map(|l| { let s = tf.render(l, true, tc); if s.get_width() > max_width - 16 { fit_surf(&s, max_width - 16) } else { s } }).collect();
        if surfs.is_empty() { return; }
        let lh = tf.get_height() + 2;
        let bw = max_width.min(surfs.iter().map(|s| s.get_width()).max().unwrap() + 16);
        let bh = surfs.len() as i32 * lh + 12;
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let (mut x, mut y);
        if let Some(a) = anchor {
            x = a.right() + 12;
            y = a.centery() - bh / 2;
            if x + bw > w { x = a.left() - bw - 12; }
        } else {
            x = mp.0 + 16;
            y = mp.1 + 16;
            if x + bw > w { x = mp.0 - bw - 16; }
            if y + bh > h { y = mp.1 - bh - 16; }
        }
        x = 4.max(x.min(w - bw - 4));
        y = 4.max(y.min(h - bh - 4));
        let tip = Surface::new_alpha(bw, bh);
        draw::rect(&tip, (25, 25, 35, 235), tip.get_rect(), 0, 8);
        let bc = match accent { Some(c) => (c.0, c.1, c.2, 255), None => (150, 180, 220, 255) };
        draw::rect(&tip, bc, tip.get_rect(), if accent.is_some() { 2 } else { 1 }, 8);
        for (i, s) in surfs.iter().enumerate() { tip.blit(s, (8, 6 + i as i32 * lh)); }
        self.base_surface.clone().blit(&tip, (x, y));
    }

    pub fn draw_loadout(&mut self) {
        self.fill_bg(true);
        let mp = self.get_mouse_pos_logical();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let t = self.font.render(&self.get_text("loadout"), true, self.get_text_color());
        bs.blit(&t, (w / 2 - t.get_width() / 2, 20));
        let sf = self.small_font.clone();
        let eq = self.powers_state.gstrs("equipped");
        let owned = self.powers_state.gstrs("owned");
        bs.blit(&sf.render(&format!("Slots: {}/{}", eq.len(), self.get_max_slots()), true, self.get_text_color()), (20, 20));
        let (tw, th) = (150, 40);
        let xs = w / 2 - (2 * tw + 8) / 2;
        let or_ = Rect::new(xs, 80, tw, th);
        let ar = Rect::new(xs + tw + 8, 80, tw, th);
        let all = powers_list();
        for (i, (r, label)) in [(or_, self.get_text("owned_tab")), (ar, self.get_text("all_tab"))].iter().enumerate() {
            let active = self.loadout_tab == i;
            let hv = r.collidepoint(mp);
            let (bg, bc) = if active { (self.get_button_color(), self.get_button_hover_color()) } else { (if hv { self.get_button_hover_color() } else { self.get_panel_color() }, if hv { (150, 180, 220) } else { (180, 180, 180) }) };
            draw::rect(&bs, bg, *r, 0, 12);
            draw::rect(&bs, bc, *r, 2, 12);
            let ts = sf.render(label, true, self.get_text_color());
            bs.blit(&ts, (r.centerx() - ts.get_width() / 2, r.centery() - ts.get_height() / 2));
            let cnt = if i == 0 { owned.len() } else { all.len() };
            let ct = sf.render(&format!("({})", cnt), true, self.get_text_color());
            bs.blit(&ct, (r.right() - ct.get_width() - 8, r.y + 4));
        }
        let info = sf.render(&self.get_text("click_equip"), true, self.get_text_color());
        bs.blit(&info, (w / 2 - info.get_width() / 2, h - 40));
        let mut y = 140 - self.loadout_scroll;
        self.loadout_item_rects.clear();
        let items: Vec<String> = if self.loadout_tab == 0 { owned.clone() } else { let mut a = all.clone(); a.sort_by_key(|n| rar_order(&power_rarity(n))); a };
        if items.is_empty() { bs.blit(&sf.render(&self.get_text("no_owned"), true, DARK_GRAY), (100, y)); }
        let mut hovered: Option<(String, Option<Rect>)> = None;
        for (idx, name) in items.iter().enumerate() {
            let rar = d("POWERS_DB").gv(name).map(|p| p.gs("rarity", "?")).unwrap_or_else(|| "?".into());
            let color = if ["Common", "Rare", "Epic", "Legendary", "Mythic"].contains(&rar.as_str()) { rarity_color(&rar) } else { GRAY };
            let r = Rect::new(100, y, 600, 40);
            let mh = r.collidepoint(mp);
            let rh = mh || self.nav_is_focused("loadout", idx, items.len());
            if rh { hovered = Some((name.clone(), if mh { None } else { Some(r) })); }
            let ow = owned.contains(name);
            let eqd = eq.contains(name);
            let (bg, mut bc) = if self.loadout_tab == 1 && !ow { (self.get_panel_color(), (200, 200, 200)) } else if eqd { (self.get_button_hover_color(), (100, 200, 100)) } else { (self.get_panel_color(), (200, 200, 200)) };
            if rh { bc = (150, 180, 220); }
            draw::rect(&bs, bg, r, 0, 8);
            draw::rect(&bs, bc, r, if rh { 2 } else { 1 }, 8);
            let rr = Rect::new(105, y + 5, 30, 30);
            draw::rect(&bs, color, rr, 0, 6);
            draw::rect(&bs, (0, 0, 0), rr, 1, 6);
            let mut label = self.get_power_name(name);
            if self.loadout_tab == 1 && !ow { label += &format!(" - {}", self.get_text("locked")); } else if eqd { label += &format!(" - {}", self.get_text("equipped")); }
            bs.blit(&sf.render(&label, true, self.get_text_color()), (145, y + 8));
            bs.blit(&sf.render(&format!("[{}]", rar), true, self.get_text_color()), (145, y + 22));
            let desc = self.get_power_desc(name, false);
            if !desc.is_empty() && self.loadout_tab == 1 { bs.blit(&sf.render(&desc, true, self.get_text_color()), (300, y + 15)); }
            self.loadout_item_rects.push((r, name.clone()));
            y += 45;
        }
        if let Some((hn, anchor)) = hovered {
            if !(self.loadout_tab == 1 && !self.get_power_desc(&hn, false).is_empty()) {
                let dsc = self.get_power_desc(&hn, false);
                self.draw_hover_tooltip(&[dsc], mp, 280, None, anchor);
            }
        }
        if let Some(name) = self.loadout_overlay.clone() {
            let desc = self.get_power_desc(&name, true);
            let pd = d("POWERS_DB").gv(&name).cloned().unwrap_or(json!({}));
            let rar = pd.gs("rarity", "?");
            let s = Surface::new(w, h);
            s.set_alpha(Some(200));
            s.fill(OVERLAY_BG);
            bs.blit(&s, (0, 0));
            let panel = Rect::new(60, 60, w - 120, h - 120);
            draw::rect(&bs, self.get_panel_color(), panel, 0, 16);
            draw::rect(&bs, (60, 80, 140), panel, 3, 16);
            let rc = if ["Common", "Rare", "Epic", "Legendary", "Mythic"].contains(&rar.as_str()) { rarity_color(&rar) } else { GRAY };
            let mf = self.menu_font.clone();
            let t1 = mf.render(&self.get_power_name(&name), true, self.get_text_color());
            let t2 = mf.render(&format!("[{}]", rar), true, rc);
            bs.blit(&t1, (panel.x + 20, panel.y + 20));
            bs.blit(&t2, (panel.x + 20 + t1.get_width() + 10, panel.y + 20));
            let rr = Rect::new(panel.x + 20, panel.y + 50, 30, 30);
            draw::rect(&bs, rc, rr, 0, 8);
            draw::rect(&bs, (0, 0, 0), rr, 2, 8);
            bs.blit(&sf.render(&format!("{}:", self.get_text("description")), true, self.get_text_color()), (panel.x + 60, panel.y + 55));
            let mut lines = vec![];
            let mut line = String::new();
            for wd in desc.split(' ') {
                if char_len(&format!("{} {}", line, wd)) > 70 { if !line.is_empty() { lines.push(line.clone()); } line = wd.to_string(); }
                else { line = format!("{} {}", line, wd).trim().to_string(); }
            }
            if !line.is_empty() { lines.push(line); }
            let mut y0 = panel.y + 85;
            for ln in &lines { bs.blit(&sf.render(ln, true, self.get_text_color()), (panel.x + 20, y0)); y0 += 24; }
            y0 += 10;
            if let Some(cd) = pd.gv("cd") { bs.blit(&sf.render(&format!("Cooldown: {}s", py_str(cd)), true, self.get_text_color()), (panel.x + 20, y0)); y0 += 24; }
            if let Some(du) = pd.gv("duration") {
                let t = match du { Value::Array(a) if a.len() >= 2 => format!("Duration: {}-{}s", py_str(&a[0]), py_str(&a[1])), other => format!("Duration: {}s", py_str(other)) };
                bs.blit(&sf.render(&t, true, self.get_text_color()), (panel.x + 20, y0));
                y0 += 24;
            }
            let ow = owned.contains(&name);
            let st = if ow { sf.render(&format!("✅ {}", self.get_text("owned_tab")), true, (0, 150, 0)) } else { sf.render(&format!("❌ {}", self.get_text("not_owned")), true, (150, 0, 0)) };
            bs.blit(&st, (panel.x + 20, y0));
            y0 += 24;
            if eq.contains(&name) { bs.blit(&sf.render(&self.get_text("equipped_tab"), true, (0, 100, 200)), (panel.x + 20, y0)); }
            let hint = sf.render(&self.get_text("enter_click_esc_close_hint"), true, self.get_text_color());
            bs.blit(&hint, (panel.centerx() - hint.get_width() / 2, panel.bottom() - 40));
        }
        self.flip();
    }

    fn equip_from_loadout(&mut self, name: &str) {
        if name == "Extra Slot" { self.push_overlay("Extra Slot ist passiv und kann nicht ausgerüstet werden", (180, 80, 80), 1.5, None); return; }
        let mut eq = self.powers_state.gstrs("equipped");
        if eq.iter().any(|e| e == name) {
            self.push_overlay(&format!("{} bereits ausgerüstet", name), (120, 120, 120), 1.5, None);
        } else if eq.len() < self.get_max_slots() {
            eq.push(name.into());
            let t = format!("Equipped: {}", self.get_power_name(name));
            self.push_overlay(&t, (80, 160, 80), 1.5, None);
        } else if !eq.is_empty() {
            let dropped = eq.remove(0);
            eq.push(name.into());
            let t = format!("Auto-Swap: {} → {}", self.get_power_name(&dropped), self.get_power_name(name));
            self.push_overlay(&t, (180, 140, 20), 1.5, None);
        } else {
            eq.push(name.into());
            let t = format!("Equipped: {}", self.get_power_name(name));
            self.push_overlay(&t, (80, 160, 80), 1.5, None);
        }
        self.powers_state.insert("equipped".into(), json!(eq));
        self.save_section("powers", None);
    }

    pub fn loadout_loop(&mut self) {
        self.loadout_overlay = None;
        while self.in_loadout {
            self.draw_loadout();
            let rects: Vec<Rect> = self.loadout_item_rects.iter().map(|x| x.0).collect();
            let (ft, fb) = (140, self.HEIGHT - 50);
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if key == K_m || self.is_back_key(key) {
                            if self.loadout_overlay.is_some() { self.loadout_overlay = None; } else { self.in_loadout = false; self.in_menu = true; }
                        } else if key == K_TAB { self.loadout_tab = (self.loadout_tab + 1) % 2; }
                        else if self.loadout_overlay.is_some() { if self.is_nav_confirm(key) { self.loadout_overlay = None; } }
                        else if let Some(dd) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("loadout", rects.len());
                            let ni = nav_move(&rects, cur, dd);
                            if ni != cur {
                                self.nav_set_focus("loadout", ni);
                                if let Some(i) = ni { let dl = nav_reveal_delta(rects[i], ft, fb); self.adjust_loadout_scroll(dl); }
                            }
                        } else if self.is_nav_confirm(key) {
                            if let Some(fi) = self.nav_get_focus("loadout", rects.len()) {
                                if fi < self.loadout_item_rects.len() {
                                    let name = self.loadout_item_rects[fi].1.clone();
                                    self.play_sound(Snd::Click);
                                    if self.loadout_tab == 1 { self.loadout_overlay = Some(name); } else { self.equip_from_loadout(&name); }
                                }
                            }
                        }
                    }
                    Event::MouseWheel { y, .. } => { self.adjust_loadout_scroll(-y * 30); }
                    Event::MouseButtonDown { button: 1, .. } => {
                        let p = self.get_mouse_pos_logical();
                        let (tw, th) = (150, 40);
                        let xs = self.WIDTH / 2 - (2 * tw + 8) / 2;
                        if Rect::new(xs, 80, tw, th).collidepoint(p) { self.play_sound_v(Snd::Click, "click_volume"); self.loadout_tab = 0; self.loadout_overlay = None; continue; }
                        if Rect::new(xs + tw + 8, 80, tw, th).collidepoint(p) { self.play_sound(Snd::Click); self.loadout_tab = 1; self.loadout_overlay = None; continue; }
                        if self.loadout_overlay.is_some() { self.loadout_overlay = None; continue; }
                        for (r, name) in self.loadout_item_rects.clone() {
                            if r.collidepoint(p) {
                                self.play_sound(Snd::Click);
                                if self.loadout_tab == 1 { self.loadout_overlay = Some(name); break; }
                                if name == "Extra Slot" { self.push_overlay("Extra Slot ist passiv und kann nicht ausgerüstet werden", (180, 80, 80), 1.5, None); continue; }
                                self.equip_from_loadout(&name);
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // ---------------- Admin ----------------
    pub fn admin_password() -> String { std::env::var("JND_ADMIN_PW").unwrap_or_else(|_| "letin123".into()) }

    pub fn draw_admin_login(&mut self) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let w = self.WIDTH;
        let t = self.font.render(&self.get_text("admin"), true, self.get_text_color());
        bs.blit(&t, (w / 2 - t.get_width() / 2, 40));
        self.draw_card(&bs, Rect::new(70, 110, 460, 220), 14, true, None, 3, false, None);
        let sf = self.small_font.clone();
        bs.blit(&sf.render(&self.get_text("enter_password"), true, self.get_text_color()), (100, 150));
        let bx = Rect::new(100, 182, 420, 40);
        draw::rect(&bs, self.get_bg_color(), bx, 0, 8);
        draw::rect(&bs, (90, 130, 210), bx, 2, 8);
        bs.blit(&sf.render(&"*".repeat(char_len(&self.admin_pwd_input)), true, self.get_text_color()), (bx.x + 10, bx.y + 8));
        bs.blit(&sf.render(&self.get_text("enter_login_menu_hint"), true, self.get_text_color()), (100, 240));
        if !self.admin_msg.is_empty() { bs.blit(&sf.render(&self.admin_msg, true, (220, 60, 60)), (100, 280)); }
        self.flip();
    }

    pub fn admin_login_loop(&mut self) {
        while self.in_admin_login {
            self.draw_admin_login();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, unicode, .. } => {
                        if key == K_ESCAPE || key == K_m { self.in_admin_login = false; self.in_menu = true; }
                        else if key == K_BACKSPACE { self.admin_pwd_input.pop(); }
                        else if key == K_RETURN {
                            if self.admin_pwd_input == G::admin_password() {
                                self.admin_pwd_input.clear();
                                self.admin_msg.clear();
                                self.in_admin_login = false;
                                self.in_admin_panel = true;
                            } else { self.admin_msg = self.get_text("wrong_password"); }
                        } else if let Some(ch) = super::online_screens3::ascii_ch(&unicode) {
                            if char_len(&self.admin_pwd_input) < 64 { self.admin_pwd_input.push(ch); }
                        }
                    }
                    _ => {}
                }
            }
            self.clock.tick(60);
        }
    }

    pub fn draw_admin_panel(&mut self) -> (Rect, Rect, Rect, Rect) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let t = self.font.render(&self.get_text("admin"), true, self.get_text_color());
        bs.blit(&t, (w / 2 - t.get_width() / 2, 40));
        self.draw_card(&bs, Rect::new(60, 95, w - 120, h - 110), 16, true, None, 3, false, None);
        let sf = self.small_font.clone();
        bs.blit(&sf.render(&format!("{}: {}", self.get_text("coins"), self.stats.gi("coins", 0)), true, self.get_text_color()), (100, 120));
        bs.blit(&sf.render(&format!("{}:", self.get_text("amount")), true, self.get_text_color()), (100, 164));
        let bx = Rect::new(200, 156, 140, 40);
        draw::rect(&bs, self.get_bg_color(), bx, 0, 8);
        draw::rect(&bs, (90, 130, 210), bx, 2, 8);
        bs.blit(&sf.render(&self.admin_amount_input, true, self.get_text_color()), (bx.x + 10, bx.y + 8));
        let add = Rect::new(360, 156, 200, 40);
        let set = Rect::new(360, 206, 200, 40);
        let logout = Rect::new(w - 160, h - 60, 140, 40);
        let mp = self.get_mouse_pos_logical();
        for (r, k) in [(add, "add_coins"), (set, "set_coins"), (logout, "logout")] {
            self.draw_ui_button(&bs, r, &self.get_text(k), BtnOpts { font: Some(sf.clone()), hovered: r.collidepoint(mp), ..Default::default() });
        }
        bs.blit(&sf.render(&format!("{}:", self.get_text("give_power")), true, self.get_text_color()), (100, 260));
        self.admin_power_rects.clear();
        let mut y = 290;
        for (i, pn) in powers_list().iter().take(8).enumerate() {
            if i % 2 == 0 && i > 0 { y += 35; }
            let x = 100 + (i as i32 % 2) * 300;
            let r = Rect::new(x, y, 280, 30);
            self.admin_power_rects.push((r, pn.clone()));
            let rar = power_rarity(pn);
            let hv = r.collidepoint(mp);
            draw::rect(&bs, rarity_color(&rar), r, 0, 6);
            draw::rect(&bs, if hv { (255, 255, 255) } else { DARK_GRAY }, r, if hv { 2 } else { 1 }, 6);
            bs.blit(&sf.render(&self.get_power_name(pn), true, self.get_text_color()), (r.x + 8, r.y + 6));
        }
        bs.blit(&sf.render(&self.get_text("enter_confirm_menu_hint"), true, self.get_text_color()), (100, h - 40));
        self.flip();
        (add, set, logout, bx)
    }

    pub fn admin_panel_loop(&mut self) {
        while self.in_admin_panel {
            let (add, set, logout, _) = self.draw_admin_panel();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, unicode, .. } => {
                        if key == K_ESCAPE || key == K_m { self.in_admin_panel = false; self.in_menu = true; }
                        else if key == K_BACKSPACE { self.admin_amount_input.pop(); }
                        else if key == K_RETURN { self.apply_admin_add(); }
                        else if let Some(ch) = unicode.chars().next() {
                            if ch.is_ascii_digit() && self.admin_amount_input.len() < 9 { self.admin_amount_input.push(ch); }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let p = self.get_mouse_pos_logical();
                        if logout.collidepoint(p) { self.play_sound(Snd::ClickBack); self.in_admin_panel = false; self.in_menu = true; }
                        else if add.collidepoint(p) { self.play_sound(Snd::Click); self.apply_admin_add(); }
                        else if set.collidepoint(p) { self.play_sound(Snd::Click); self.apply_admin_set(); }
                        else {
                            for (r, n) in self.admin_power_rects.clone() { if r.collidepoint(p) { self.play_sound(Snd::Click); self.apply_admin_give_power(&n); break; } }
                        }
                    }
                    _ => {}
                }
            }
            self.clock.tick(60);
        }
    }

    pub fn apply_admin_add(&mut self) {
        let n: i64 = if self.admin_amount_input.is_empty() { 0 } else { match self.admin_amount_input.parse() { Ok(v) => v, Err(_) => return } };
        if n <= 0 { return; }
        let c = self.stats.gi("coins", 0) + n;
        self.stats.insert("coins".into(), json!(c));
        self.save_section("stats", None);
    }
    pub fn apply_admin_set(&mut self) {
        let mut n: i64 = if self.admin_amount_input.is_empty() { 0 } else { match self.admin_amount_input.parse() { Ok(v) => v, Err(_) => return } };
        if n < 0 { n = 0; }
        self.stats.insert("coins".into(), json!(n));
        self.save_section("stats", None);
    }
    pub fn apply_admin_give_power(&mut self, pn: &str) {
        let mut owned = self.powers_state.gstrs("owned");
        if !owned.iter().any(|o| o == pn) {
            owned.push(pn.into());
            self.powers_state.insert("owned".into(), json!(owned));
            self.save_section("powers", None);
            let t = format!("Power '{}' hinzugefügt!", self.get_power_name(pn));
            self.push_overlay(&t, GOLD, 1.5, None);
        } else {
            let t = format!("Power '{}' bereits besessen", self.get_power_name(pn));
            self.push_overlay(&t, (200, 200, 0), 1.5, None);
        }
    }
}
