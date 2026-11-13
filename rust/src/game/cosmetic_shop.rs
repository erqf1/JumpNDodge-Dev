//! Kosmetik-Shop (Python 12373-12826).

use super::core::*;
use super::cosmetics_inv::*;
use super::state::{Snd, G};
use crate::data::d;
use crate::pg::*;
use crate::util::*;
use serde_json::Value;

fn rar_order(r: &str) -> i32 { match r { "Common" => 0, "Rare" => 1, "Epic" => 2, "Legendary" => 3, "Mythic" => 4, _ => 9 } }
fn capitalize(s: &str) -> String { let mut c = s.chars(); match c.next() { Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(), None => String::new() } }
fn sub(c: (i32, i32, i32), n: i32) -> (i32, i32, i32) { ((c.0 - n).max(0), (c.1 - n).max(0), (c.2 - n).max(0)) }
fn add(c: (i32, i32, i32), n: i32) -> (i32, i32, i32) { ((c.0 + n).min(255), (c.1 + n).min(255), (c.2 + n).min(255)) }

impl G {
    /// -> (back_rect, [(key, buy_rect)])
    pub fn draw_cosmetic_shop(&mut self) -> (Rect, Vec<(String, Rect)>) {
        self._trail_anim_t = (self._trail_anim_t + 0.06) % (2.0 * 3.14159265);
        let mp = self.get_mouse_pos_logical();
        let dark = self.is_dark_mode();
        self.ensure_daily_sale();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let deco = if !dark { [(200, 210, 255), (220, 200, 255)] } else { [(60, 80, 140), (80, 50, 120)] };
        for (i, dc) in deco.iter().enumerate() {
            let s = Surface::new_alpha(300, 300);
            draw::circle(&s, (dc.0, dc.1, dc.2, 40), (150, 150), 150, 0);
            bs.blit(&s, (i as i32 * 500 - 100, 50));
        }
        let tt = self.get_text("cosmetic_shop");
        self.draw_screen_title(&bs, &tt, 20);
        let sf = self.small_font.clone();
        let mf = self.menu_font.clone();
        let cb = Rect::new(w - 175, 15, 160, 32);
        self.draw_panel(&bs, cb, 16, (200, 180, 60), 2);
        draw::circle(&bs, GOLD, (cb.x + 18, cb.centery()), 8, 0);
        draw::circle(&bs, (180, 140, 0), (cb.x + 18, cb.centery()), 8, 2);
        let ct = sf.render_fit(&format!("{} Coins", self.stats.gi("coins", 0)), cb.w - 42, self.get_text_color());
        bs.blit(&ct, (cb.x + 34, cb.centery() - ct.get_height() / 2));
        let (tabs, tw, _) = self.get_cosmetic_tab_layout(true);
        let tlf = if tw < 150 { self.tab_font.clone() } else { sf.clone() };
        for (i, (r, k)) in tabs.iter().zip(COSMETIC_TAB_LABEL_KEYS.iter()).enumerate() {
            let hot = r.collidepoint(mp);
            self.draw_tab(&bs, *r, &self.get_text(k), &tlf, !self.cosmetic_shop_showing_sale && self.cosmetic_shop_tab == i, hot);
        }
        let sr = self.get_sale_button_rect();
        let sh = sr.collidepoint(mp);
        // Sale bleibt rot (Bedeutung: Angebot), sonst wie die anderen Reiter
        self.draw_ui_button(&bs, sr, "Sale", BtnOpts { font: Some(sf.clone()), hovered: sh || self.cosmetic_shop_showing_sale, base_color: Some(if self.cosmetic_shop_showing_sale { (210, 45, 60) } else { (160, 32, 44) }), hover_color: Some((220, 55, 70)), text_color: Some((255, 255, 255)), ..Default::default() });
        let content_top = 160;
        let resolved: Option<String>;
        let grid_start_y;
        if self.cosmetic_shop_showing_sale {
            resolved = Some("__sale__".into());
            let s1 = mf.render(&self.get_text("sale_view_title"), true, (255, 90, 90));
            bs.blit(&s1, (w / 2 - s1.get_width() / 2, content_top));
            let tu = self.get_time_until_reset();
            let s2 = sf.render(&format!("{} {}", self.get_text("sale_next_in"), tu), true, self.get_text_color());
            bs.blit(&s2, (w / 2 - s2.get_width() / 2, content_top + s1.get_height() + 4));
            grid_start_y = content_top + s1.get_height() + 4 + s2.get_height() + 16;
        } else {
            let cat = COSMETIC_TAB_CATEGORIES[self.cosmetic_shop_tab];
            if cat == Some("face") {
                let subs = self.get_face_subtab_layout();
                for (i, r) in subs.iter().enumerate() {
                    let active = FACE_SUBCATEGORIES[i] == self.shop_face_subcategory;
                    let hot = r.collidepoint(mp);
                    self.draw_tab(&bs, *r, &self.get_text(FACE_SUBCATEGORY_LABEL_KEYS[i]), &sf, active, hot);
                }
                resolved = Some(self.shop_face_subcategory.clone());
                grid_start_y = subs[0].bottom() + 20;
            } else {
                resolved = cat.map(|c| c.to_string());
                grid_start_y = content_top;
            }
        }
        // kompakte Kacheln: 4 Spalten, damit auch 11 Spuren auf einen Bildschirm passen
        let cols = 4;
        let (tile_w, tile_h, spx, spy) = (172, 96, 12, 12);
        let db = d("COSMETICS_DB").as_object().unwrap();
        let mut items: Vec<(String, Value)> = match resolved.as_deref() {
            Some("__sale__") => self.get_sale_items().into_iter().filter(|k| db.contains_key(k)).map(|k| { let m = db[&k].clone(); (k, m) }).collect(),
            None => vec![],
            Some(cat) => db.iter().filter(|(_, m)| m.gs("category", "") == cat).map(|(k, m)| (k.clone(), m.clone())).collect(),
        };
        items.sort_by_key(|(_, m)| rar_order(&m.gs("rarity", "")));
        let grid_w = cols * tile_w + (cols - 1) * spx;
        let gx = w / 2 - grid_w / 2;
        let owned_keys = self.cosmetics_state.gstrs("owned");
        let total_buyable = items.iter().filter(|(k, _)| !owned_keys.contains(k)).count();
        let mut buy_idx = 0;
        let mut item_rects: Vec<(String, Rect)> = vec![];
        if resolved.is_none() {
            let m = sf.render(&self.get_text("tools_coming_soon"), true, self.get_text_color());
            bs.blit(&m, (w / 2 - m.get_width() / 2, grid_start_y + 60));
        } else if resolved.as_deref() == Some("__sale__") && items.is_empty() {
            let m = sf.render(&self.get_text("sale_no_items"), true, self.get_text_color());
            bs.blit(&m, (w / 2 - m.get_width() / 2, grid_start_y + 60));
        }
        let pos_of = |i: usize, n: usize| -> (i32, i32) {
            let cu = cols as usize;
            let (col, row) = ((i % cu) as i32, (i / cu) as i32);
            let rc = cu.min(n - row as usize * cu) as i32;
            let off = ((cols - rc) * (tile_w + spx)) / 2;
            (gx + off + col * (tile_w + spx), grid_start_y + row * (tile_h + spy))
        };
        for (i, (key, meta)) in items.iter().enumerate() {
            let (x, y) = pos_of(i, items.len());
            let ir = Rect::new(x, y, tile_w, tile_h);
            let name = self.get_cosmetic_name(key);
            let price = self.get_cosmetic_price(key);
            let base_price = self.get_cosmetic_base_price(key);
            let disc = self.get_sale_discount(key);
            let owned = owned_keys.contains(key);
            let rc = get_rarity_colors(&meta.gs("rarity", "Common"));
            let shc: Color = if dark { (0, 0, 0, 60).into() } else { (180, 180, 200).into() };
            draw::rect(&bs, shc, Rect::new(x + 4, y + 4, tile_w, tile_h), 0, 14);
            let cbg = if dark { sub(rc.bg, 40) } else { rc.bg };
            draw::rect(&bs, cbg, ir, 0, 14);
            let foc = !owned && self.nav_is_focused("cosmetic_shop", buy_idx, total_buyable + 1);
            if ir.collidepoint(mp) || foc { draw::rect(&bs, rc.border, ir, 3, 14); } else { draw::rect(&bs, sub(rc.border, 50), ir, 2, 14); }
            let bt = self.badge_font.render(rc.label, true, (255, 255, 255));
            let br = Rect::new(x + 6, y + 6, bt.get_width() + 12, bt.get_height() + 4);
            draw::rect(&bs, rc.badge, br, 0, 6);
            bs.blit(&bt, (br.x + 6, br.y + 2));
            let (icx, icy) = (x + tile_w - 24, y + 24);
            draw::circle(&bs, add(cbg, 40), (icx, icy), 17, 0);
            draw::circle(&bs, rc.border, (icx, icy), 17, 2);
            draw_cosmetic_icon(&bs, key, icx, icy, false);
            let nm = mf.render_fit(&name, tile_w - 52, (255, 255, 255));
            bs.blit(&nm, (x + 8, y + 32));
            let py = y + tile_h - 26;
            draw::circle(&bs, GOLD, (x + 16, py + 9), 6, 0);
            draw::circle(&bs, (200, 160, 0), (x + 16, py + 9), 6, 2);
            if disc != 0 {
                let bps = sf.render(&base_price.to_string(), true, (190, 190, 190));
                bs.blit(&bps, (x + 26, py));
                let sy = py + bps.get_height() / 2;
                draw::line(&bs, (190, 190, 190), (x + 26, sy), (x + 26 + bps.get_width(), sy), 2);
                bs.blit(&sf.render(&price.to_string(), true, (255, 110, 110)), (x + 26 + bps.get_width() + 6, py));
            } else {
                bs.blit(&sf.render(&price.to_string(), true, GOLD), (x + 26, py));
            }
            let br2 = Rect::new(x + tile_w - 78, y + tile_h - 28, 70, 22);
            if owned {
                draw::rect(&bs, (30, 160, 60), br2, 0, 12);
                let os = sf.render_fit(&self.get_text("owned"), br2.w - 6, (255, 255, 255));
                bs.blit(&os, (br2.centerx() - os.get_width() / 2, br2.centery() - os.get_height() / 2));
            } else {
                let hv = br2.collidepoint(mp) || foc;
                draw::rect(&bs, if hv { (255, 200, 20) } else { (200, 160, 10) }, br2, 0, 12);
                let bt2 = sf.render_fit(&self.get_text("buy"), br2.w - 6, (30, 20, 0));
                bs.blit(&bt2, (br2.centerx() - bt2.get_width() / 2, br2.centery() - bt2.get_height() / 2));
                item_rects.push((key.clone(), br2));
                buy_idx += 1;
            }
        }
        let mut hovered: Option<String> = None;
        for (i, (key, _)) in items.iter().enumerate() {
            let (rx, ry) = pos_of(i, items.len());
            if Rect::new(rx, ry, tile_w, tile_h).collidepoint(mp) { hovered = Some(key.clone()); break; }
        }
        if hovered.is_none() {
            if let Some(fi) = self.nav_get_focus("cosmetic_shop", total_buyable + 1) {
                if fi < total_buyable { hovered = item_rects.get(fi).map(|x| x.0.clone()); }
            }
        }
        if let Some(hk) = hovered {
            let pm = db[&hk].clone();
            let pr = pm.gs("rarity", "Common");
            let prc = get_rarity_colors(&pr);
            let pname = self.get_cosmetic_name(&hk);
            let pprice = self.get_cosmetic_price(&hk);
            let powned = owned_keys.contains(&hk);
            let total_rows = (items.len() as i32 + cols - 1) / cols;
            let pv_w = cols * tile_w + (cols - 1) * spx;
            let pv_h = 76;
            let pv_x = gx;
            let pv_y = (grid_start_y + total_rows * (tile_h + spy) + 2).min(h - 58 - pv_h - 6);
            draw::rect(&bs, self.get_panel_color(), Rect::new(pv_x, pv_y, pv_w, pv_h), 0, 12);
            draw::rect(&bs, prc.border, Rect::new(pv_x, pv_y, pv_w, pv_h), 2, 12);
            let (icx2, icy2) = (pv_x + 46, pv_y + pv_h / 2);
            if let Some(slot) = cosmetic_category_to_slot(&pm.gs("category", "")) {
                let mut ov = self.cosmetics_state.clone();
                ov.insert(slot.into(), Value::String(hk.clone()));
                let ar = Rect::new(icx2 - 25, icy2 - 25, 50, 50);
                draw::rect(&bs, (100, 100, 150), Rect::new(ar.x + 3, ar.y + 3, 50, 50), 0, 8);
                self.draw_avatar(&bs, ar, None, true, Some(&ov));
            } else {
                draw::circle(&bs, add(prc.bg, 30), (icx2, icy2), 32, 0);
                draw::circle(&bs, prc.border, (icx2, icy2), 32, 2);
                draw_cosmetic_icon(&bs, &hk, icx2, icy2, true);
            }
            let tx = pv_x + 96;
            bs.blit(&mf.render(&pname, true, self.get_text_color()), (tx, pv_y + 8));
            let bsf = sf.render(&pr, true, (255, 255, 255));
            let bbg = Rect::new(tx, pv_y + 36, bsf.get_width() + 14, 20);
            draw::rect(&bs, prc.badge, bbg, 0, 10);
            bs.blit(&bsf, (bbg.x + 7, bbg.y + 2));
            bs.blit(&sf.render(&capitalize(&pm.gs("type", "")), true, (180, 180, 200)), (tx + bbg.w + 10, pv_y + 37));
            bs.blit(&sf.render(&format!("{} Coins", pprice), true, GOLD), (tx, pv_y + 60));
            let ss = sf.render(&if powned { self.tde("Besessen", "Owned") } else { self.tde("Nicht besessen", "Not owned") }, true, if powned { (80, 200, 120) } else { (200, 120, 100) });
            bs.blit(&ss, (pv_x + pv_w - ss.get_width() - 14, pv_y + pv_h - 22));
        }
        let back = Rect::new(w / 2 - 150, h - 54, 300, 44);
        let bh = back.collidepoint(mp) || self.nav_is_focused("cosmetic_shop", total_buyable, total_buyable + 1);
        self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(mf.clone()), hovered: bh, ..Default::default() });
        self.cleanup_overlays();
        for o in self.overlays.clone() {
            let t = sf.render(&o.text, true, o.color);
            bs.blit(&t, ((w - t.get_width()) / 2, 100));
        }
        self.draw_achievement_popups(&bs);
        self.flip();
        (back, item_rects)
    }

    pub fn cosmetic_shop_loop(&mut self) {
        while self.in_cosmetic_shop {
            let (back, buy) = self.draw_cosmetic_shop();
            let keys: Vec<String> = buy.iter().map(|x| x.0.clone()).collect();
            let mut rects: Vec<Rect> = buy.iter().map(|x| x.1).collect();
            rects.push(back);
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if key == K_m || self.is_back_key(key) { self.in_cosmetic_shop = false; self.in_shop_menu = true; }
                        else if key == K_TAB {
                            self.cosmetic_shop_tab = (self.cosmetic_shop_tab + 1) % 4;
                            self.shop_face_subcategory = "eyes".into();
                            self.cosmetic_shop_showing_sale = false;
                            self.nav_set_focus("cosmetic_shop", Some(0));
                        } else if let Some(dd) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("cosmetic_shop", rects.len());
                            let n = nav_move(&rects, cur, dd);
                            self.nav_set_focus("cosmetic_shop", n);
                        } else if self.is_nav_confirm(key) {
                            if let Some(fi) = self.nav_get_focus("cosmetic_shop", rects.len()) {
                                if fi < keys.len() { let k = keys[fi].clone(); self.buy_cosmetic(&k); }
                                else if fi == keys.len() { self.in_cosmetic_shop = false; self.in_shop_menu = true; }
                            }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let p = self.get_mouse_pos_logical();
                        if self.get_sale_button_rect().collidepoint(p) {
                            self.cosmetic_shop_showing_sale = !self.cosmetic_shop_showing_sale;
                            self.nav_set_focus("cosmetic_shop", Some(0));
                            self.play_sound(Snd::MenuClick);
                            continue;
                        }
                        let (tabs, _, _) = self.get_cosmetic_tab_layout(true);
                        if let Some(i) = tabs.iter().position(|r| r.collidepoint(p)) {
                            self.cosmetic_shop_tab = i;
                            self.shop_face_subcategory = "eyes".into();
                            self.cosmetic_shop_showing_sale = false;
                            self.nav_set_focus("cosmetic_shop", Some(0));
                            continue;
                        }
                        if !self.cosmetic_shop_showing_sale && COSMETIC_TAB_CATEGORIES[self.cosmetic_shop_tab] == Some("face") {
                            if let Some(i) = self.get_face_subtab_layout().iter().position(|r| r.collidepoint(p)) {
                                self.shop_face_subcategory = FACE_SUBCATEGORIES[i].into();
                                self.nav_set_focus("cosmetic_shop", Some(0));
                                continue;
                            }
                        }
                        if back.collidepoint(p) { self.in_cosmetic_shop = false; self.in_shop_menu = true; }
                        else { for (k, r) in buy.iter() { if r.collidepoint(p) { let k = k.clone(); self.buy_cosmetic(&k); break; } } }
                    }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); self.clock.tick(fps); }
        }
    }

    pub fn to_power_roll(&mut self) { self.in_shop_menu = false; self.in_shop = true; self.play_sound(Snd::MenuClick); }
    pub fn to_cosmetic_shop(&mut self) { self.in_shop_menu = false; self.in_cosmetic_shop = true; self.play_sound(Snd::MenuClick); }
}
