//! Loadout-Menü und Kosmetik-Inventar (Python 11941-12370).

use super::core::*;
use super::state::G;
use crate::data::d;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

pub const COSMETIC_TAB_CATEGORIES: [Option<&str>; 4] = [Some("trail"), Some("headwear"), Some("face"), None];
pub const COSMETIC_TAB_LABEL_KEYS: [&str; 4] = ["trails_tab", "headwear_tab", "face_tab", "tools_tab"];
pub const FACE_SUBCATEGORIES: [&str; 3] = ["eyes", "nose", "mouth"];
pub const FACE_SUBCATEGORY_LABEL_KEYS: [&str; 3] = ["eyes_tab", "nose_tab", "mouth_tab"];
pub const SALE_BUTTON_W: i32 = 100;
pub const SALE_BUTTON_GAP: i32 = 12;

pub fn cosmetic_category(key: &str) -> String { d("COSMETICS_DB").gv(key).map(|m| m.gs("category", "")).unwrap_or_default() }
pub fn cosmetic_rarity(key: &str) -> String { d("COSMETICS_DB").gv(key).map(|m| m.gs("rarity", "Common")).unwrap_or_else(|| "Common".into()) }

impl G {
    pub fn draw_loadout_menu(&mut self) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let w = self.WIDTH;
        let t = self.font.render(&self.get_text("loadout"), true, self.get_text_color());
        bs.blit(&t, (w / 2 - t.get_width() / 2, 70));
        self.draw_coin_display(&bs);
        let (sx, sy, gap) = (w / 2 - 180, 180, 70);
        let labels = [(Rect::new(sx, sy, 360, 48), "loadout"), (Rect::new(sx, sy + gap, 360, 48), "cosmetic_inventory"), (Rect::new(sx, sy + 2 * gap, 360, 48), "back")];
        for (i, (r, k)) in labels.iter().enumerate() {
            let hv = r.collidepoint(mp) || self.nav_is_focused("loadout_menu", i, labels.len());
            draw::rect(&bs, if hv { self.get_button_hover_color() } else { self.get_button_color() }, *r, 0, 12);
            draw::rect(&bs, (180, 180, 180), *r, if hv { 3 } else { 2 }, 12);
            let ts = self.menu_font.render(&self.get_text(k), true, self.get_text_color());
            bs.blit(&ts, (r.centerx() - ts.get_width() / 2, r.centery() - ts.get_height() / 2));
        }
        self.flip();
    }

    pub fn loadout_menu_loop(&mut self) {
        while self.in_loadout_menu {
            let (sx, sy, gap) = (self.WIDTH / 2 - 180, 180, 70);
            let rects = [Rect::new(sx, sy, 360, 48), Rect::new(sx, sy + gap, 360, 48), Rect::new(sx, sy + 2 * gap, 360, 48)];
            let acts = ["power_loadout", "cosmetic_inventory", "back"];
            for ev in self.event_get() {
                let mut act: Option<&str> = None;
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if key == K_m || self.is_back_key(key) { act = Some("back"); }
                        else if let Some(dd) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("loadout_menu", 3);
                            let n = nav_move(&rects, cur, dd);
                            self.nav_set_focus("loadout_menu", n);
                        } else if self.is_nav_confirm(key) {
                            act = self.nav_get_focus("loadout_menu", 3).map(|i| acts[i]);
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let p = self.get_mouse_pos_logical();
                        for (i, r) in rects.iter().enumerate() { if r.collidepoint(p) { act = Some(acts[i]); break; } }
                    }
                    _ => {}
                }
                match act {
                    Some("power_loadout") => self.to_power_loadout(),
                    Some("cosmetic_inventory") => self.to_cosmetic_inventory(),
                    Some("back") => { self.in_loadout_menu = false; self.in_menu = true; }
                    _ => {}
                }
            }
            self.draw_loadout_menu();
            { let fps = self.target_fps(); self.clock.tick(fps); }
        }
    }

    pub fn resolve_cosmetic_category(&self, tab: usize) -> Option<String> {
        match COSMETIC_TAB_CATEGORIES[tab] {
            Some("face") => Some(self.face_subcategory.clone()),
            Some(c) => Some(c.to_string()),
            None => None,
        }
    }

    pub fn get_face_subtab_layout(&self) -> Vec<Rect> {
        let (count, gap, h) = (3, 6, 30);
        let w = 150.min((self.WIDTH - 80 - (count - 1) * gap) / count);
        let xs = self.WIDTH / 2 - (count * w + (count - 1) * gap) / 2;
        (0..count).map(|i| Rect::new(xs + i * (w + gap), 146, w, h)).collect()
    }

    pub fn get_cosmetic_tab_layout(&self, reserve_sale: bool) -> (Vec<Rect>, i32, i32) {
        let tc = 4;
        let (tg, th, margin) = (6, 40, 40);
        let reserved = if reserve_sale { SALE_BUTTON_W + SALE_BUTTON_GAP } else { 0 };
        let tw = 172.min((self.WIDTH - margin - reserved - (tc - 1) * tg) / tc);
        let ttw = tc * tw + (tc - 1) * tg;
        let xs = if reserve_sale { self.WIDTH / 2 - (SALE_BUTTON_W + SALE_BUTTON_GAP + ttw) / 2 + SALE_BUTTON_W + SALE_BUTTON_GAP } else { self.WIDTH / 2 - ttw / 2 };
        ((0..tc).map(|i| Rect::new(xs + i * (tw + tg), 100, tw, th)).collect(), tw, th)
    }

    pub fn get_sale_button_rect(&self) -> Rect {
        let (tabs, _, th) = self.get_cosmetic_tab_layout(true);
        Rect::new(tabs[0].x - SALE_BUTTON_GAP - SALE_BUTTON_W, tabs[0].y, SALE_BUTTON_W, th)
    }

    pub fn get_cosmetics_for_tab(&self, tab: usize) -> (Vec<String>, Option<String>) {
        let Some(cat) = self.resolve_cosmetic_category(tab) else { return (vec![], None); };
        let owned: Vec<String> = self.cosmetics_state.gstrs("owned").into_iter().filter(|c| cosmetic_category(c) == cat).collect();
        let active = if cat == "trail" { self.cosmetics_state.gstrs("equipped").first().cloned() } else { self.cosmetics_state.gos(&format!("equipped_{}", cat)) };
        (owned, active)
    }

    pub fn equip_toggle_cosmetic(&mut self, tab: usize, cosmetic: &str, active: Option<&str>) {
        let cat = self.resolve_cosmetic_category(tab).unwrap_or_default();
        let name = self.get_cosmetic_name(cosmetic);
        if active == Some(cosmetic) {
            if cat == "trail" { self.cosmetics_state.insert("equipped".into(), json!([])); } else { self.cosmetics_state.insert(format!("equipped_{}", cat), Value::Null); }
            self.push_overlay(&format!("{} unequipped", name), (180, 180, 180), 1.5, None);
        } else {
            if cat == "trail" { self.cosmetics_state.insert("equipped".into(), json!([cosmetic])); } else { self.cosmetics_state.insert(format!("equipped_{}", cat), json!(cosmetic)); }
            self.push_overlay(&format!("{} equipped", name), (0, 200, 0), 1.5, None);
        }
        self.save_section("cosmetics", None);
    }

    fn inv_panel_top(&self) -> i32 { if COSMETIC_TAB_CATEGORIES[self.cosmetic_tab] == Some("face") { 186 } else { 150 } }

    pub fn draw_cosmetic_inventory(&mut self) -> Rect {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let t = self.font.render(&self.get_text("cosmetic_inventory"), true, self.get_text_color());
        bs.blit(&t, (w / 2 - t.get_width() / 2, 20));
        let (tabs, tw, _) = self.get_cosmetic_tab_layout(false);
        let tlf = if tw < 150 { self.tab_font.clone() } else { self.small_font.clone() };
        for (i, (r, k)) in tabs.iter().zip(COSMETIC_TAB_LABEL_KEYS.iter()).enumerate() {
            let hot = r.collidepoint(self.get_mouse_pos_logical());
            self.draw_tab(&bs, *r, &self.get_text(k), &tlf, self.cosmetic_tab == i, hot);
        }
        let sf = self.small_font.clone();
        if COSMETIC_TAB_CATEGORIES[self.cosmetic_tab] == Some("face") {
            for (i, sr) in self.get_face_subtab_layout().iter().enumerate() {
                let active = FACE_SUBCATEGORIES[i] == self.face_subcategory;
                let hot = sr.collidepoint(self.get_mouse_pos_logical());
                self.draw_tab(&bs, *sr, &self.get_text(FACE_SUBCATEGORY_LABEL_KEYS[i]), &sf, active, hot);
            }
        }
        let pt = self.inv_panel_top();
        let mut y = pt + 10;
        let panel = Rect::new(50, pt, w - 100, h - pt - 70);
        self.draw_panel(&bs, panel, 16, (60, 80, 140), 3);
        let (owned, active) = self.get_cosmetics_for_tab(self.cosmetic_tab);
        if COSMETIC_TAB_CATEGORIES[self.cosmetic_tab].is_some() {
            let cs = self.tab_font.render(&format!("{} {}", owned.len(), self.get_text("owned_tab")), true, self.get_text_color());
            bs.blit(&cs, (panel.right() - cs.get_width() - 14, panel.bottom() - cs.get_height() - 10));
        }
        if COSMETIC_TAB_CATEGORIES[self.cosmetic_tab].is_none() {
            let m = sf.render(&self.get_text("tools_coming_soon"), true, self.get_text_color());
            bs.blit(&m, (panel.centerx() - m.get_width() / 2, panel.centery() - m.get_height() / 2));
        } else if owned.is_empty() {
            bs.blit(&sf.render(&self.get_text("no_owned"), true, self.get_text_color()), (panel.x + 20, y));
        } else {
            for (i, c) in owned.iter().enumerate() {
                let rc = get_rarity_colors(&cosmetic_rarity(c));
                let eq = active.as_deref() == Some(c.as_str());
                let ir = Rect::new(panel.x + 20, y, panel.w - 40, 50);
                let shc: Color = if self.is_dark_mode() { (0, 0, 0, 40).into() } else { (210, 210, 225).into() };
                draw::rect(&bs, shc, ir.moved(2, 2), 0, 10);
                draw::rect(&bs, self.get_panel_color(), ir, 0, 10);
                draw::rect(&bs, if eq { GREEN } else { rc.border }, ir, 2, 10);
                draw::rect(&bs, rc.border, Rect::new(ir.x, ir.y, 5, ir.h), 0, 6);
                let (icx, icy) = (ir.x + 34, ir.centery());
                draw::circle(&bs, rc.bg, (icx, icy), 19, 0);
                draw::circle(&bs, rc.border, (icx, icy), 19, 2);
                draw_cosmetic_icon(&bs, c, icx, icy, false);
                bs.blit(&sf.render(&self.get_cosmetic_name(c), true, self.get_text_color()), (ir.x + 62, ir.y + 8));
                let bt = self.tab_font.render(rc.label, true, (255, 255, 255));
                let br = Rect::new(ir.x + 62, ir.y + 28, bt.get_width() + 12, 16);
                draw::rect(&bs, rc.badge, br, 0, 8);
                bs.blit(&bt, (br.x + 6, br.y + 1));
                let foc = self.nav_is_focused("cosmetic_inventory", i, owned.len() + 1);
                let btn = Rect::new(ir.right() - 120, ir.y + 10, 100, 30);
                if eq {
                    draw::rect(&bs, if !foc { (30, 170, 90) } else { (40, 200, 110) }, btn, 0, 15);
                    if foc { draw::rect(&bs, (255, 255, 255), btn, 2, 15); }
                    draw::lines(&bs, (255, 255, 255), false, &[(btn.x + 10, btn.centery()), (btn.x + 15, btn.centery() + 5), (btn.x + 24, btn.centery() - 6)], 2);
                    let ut = sf.render(&self.get_text("unequip"), true, (255, 255, 255));
                    bs.blit(&ut, (btn.x + 30, btn.centery() - ut.get_height() / 2));
                } else {
                    draw::rect(&bs, if foc { self.get_button_hover_color() } else { self.get_button_color() }, btn, 0, 15);
                    draw::rect(&bs, if !foc { rc.border } else { (255, 255, 255) }, btn, 2, 15);
                    let et = sf.render(&self.get_text("equip"), true, self.get_text_color());
                    bs.blit(&et, (btn.centerx() - et.get_width() / 2, btn.centery() - et.get_height() / 2));
                }
                y += 60;
                if y > panel.bottom() - 80 { break; }
            }
        }
        // Zurück unter dem Kasten (lag vorher über der Liste)
        let back = Rect::new(w / 2 - 150, h - 58, 300, 46);
        let hv = back.collidepoint(mp) || self.nav_is_focused("cosmetic_inventory", owned.len(), owned.len() + 1);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(self.menu_font.clone()), hovered: hv, ..Default::default() });
        self.flip();
        back
    }

    fn inv_item_buttons(&self) -> Vec<(Rect, String)> {
        let pt = self.inv_panel_top();
        let panel = Rect::new(50, pt, self.WIDTH - 100, self.HEIGHT - pt - 70);
        let mut y = pt + 10;
        let (owned, _) = self.get_cosmetics_for_tab(self.cosmetic_tab);
        let mut out = vec![];
        for c in owned {
            let ir = Rect::new(panel.x + 20, y, panel.w - 40, 50);
            out.push((Rect::new(ir.right() - 120, ir.y + 10, 100, 30), c));
            y += 60;
            if y > panel.bottom() - 80 { break; }
        }
        out
    }

    pub fn cosmetic_inventory_loop(&mut self) {
        while self.in_cosmetic_inventory {
            let back = self.draw_cosmetic_inventory();
            let items = self.inv_item_buttons();
            let (_, active) = self.get_cosmetics_for_tab(self.cosmetic_tab);
            let mut nav: Vec<Rect> = items.iter().map(|x| x.0).collect();
            nav.push(back);
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if key == K_m || self.is_back_key(key) { self._cosmetic_inventory_back(); }
                        else {
                            let dd = self.nav_direction(key);
                            if dd == Some("left") || dd == Some("right") {
                                let step = if dd == Some("right") { 1 } else { 3 };
                                self.cosmetic_tab = (self.cosmetic_tab + step) % 4;
                                self.face_subcategory = "eyes".into();
                                self.nav_set_focus("cosmetic_inventory", Some(0));
                            } else if let Some(dd) = dd {
                                let cur = self.nav_get_focus("cosmetic_inventory", nav.len());
                                let n = nav_move(&nav, cur, dd);
                                self.nav_set_focus("cosmetic_inventory", n);
                            } else if self.is_nav_confirm(key) {
                                let fi = self.nav_get_focus("cosmetic_inventory", nav.len());
                                if let Some(fi) = fi {
                                    if fi < items.len() { let c = items[fi].1.clone(); let t = self.cosmetic_tab; self.equip_toggle_cosmetic(t, &c, active.as_deref()); }
                                    else if fi == items.len() { self._cosmetic_inventory_back(); }
                                }
                            }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let p = self.get_mouse_pos_logical();
                        let (tabs, _, _) = self.get_cosmetic_tab_layout(false);
                        if let Some(i) = tabs.iter().position(|r| r.collidepoint(p)) { self.cosmetic_tab = i; continue; }
                        if COSMETIC_TAB_CATEGORIES[self.cosmetic_tab] == Some("face") {
                            if let Some(i) = self.get_face_subtab_layout().iter().position(|r| r.collidepoint(p)) {
                                self.face_subcategory = FACE_SUBCATEGORIES[i].into();
                                self.nav_set_focus("cosmetic_inventory", Some(0));
                                continue;
                            }
                        }
                        if back.collidepoint(p) { self._cosmetic_inventory_back(); break; }
                        let (_, act2) = self.get_cosmetics_for_tab(self.cosmetic_tab);
                        for (r, c) in self.inv_item_buttons() {
                            if r.collidepoint(p) { let t = self.cosmetic_tab; self.equip_toggle_cosmetic(t, &c, act2.as_deref()); break; }
                        }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); self.clock.tick(fps); }
        }
    }
}
