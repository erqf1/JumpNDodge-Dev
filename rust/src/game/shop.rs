//! Glücksrad-Shop (Ribbon) + Roll x10 (Python 11339-11916).

use super::core::*;
use super::menus::fit_surf;
use super::state::{Snd, G};
use crate::data::d;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

pub const SHOP_COST_SINGLE: i64 = 50;
pub const SHOP_COST_X10: i64 = 500;
pub const DUPLICATE_REFUND: i64 = 10;
pub const TILE_W: i32 = 150;
pub const TILE_H: i32 = 130;
pub const RIBBON_Y: i32 = 300;
pub const SPIN_DURATION_MS: i64 = 2200;
pub const RARITY_ORDER: [&str; 5] = ["Mythic", "Legendary", "Epic", "Rare", "Common"];

pub struct PowCols { pub bg: (i32, i32, i32), pub border: (i32, i32, i32), pub badge: (i32, i32, i32) }
pub fn power_rarity_colors(r: &str) -> PowCols {
    match r {
        "Rare" => PowCols { bg: (30, 70, 160), border: (90, 150, 255), badge: (60, 120, 220) },
        "Epic" => PowCols { bg: (90, 30, 150), border: (190, 90, 255), badge: (150, 60, 230) },
        "Legendary" => PowCols { bg: (150, 100, 0), border: (255, 210, 60), badge: (220, 170, 20) },
        "Mythic" => PowCols { bg: (170, 70, 0), border: (255, 150, 40), badge: (230, 110, 10) },
        _ => PowCols { bg: (40, 110, 40), border: (100, 220, 100), badge: (70, 180, 70) },
    }
}
pub fn rarity_icon_shape(r: &str) -> &'static str {
    match r { "Rare" => "diamond", "Epic" => "hexagon", "Legendary" | "Mythic" => "star4", _ => "circle" }
}

pub fn powers_list() -> Vec<String> { d("POWERS_DB").as_object().unwrap().keys().cloned().collect() }
pub fn power_rarity(name: &str) -> String { d("POWERS_DB").gv(name).map(|p| p.gs("rarity", "Common")).unwrap_or_else(|| "Common".into()) }

fn sub(c: (i32, i32, i32), n: i32) -> (i32, i32, i32) { ((c.0 - n).max(0), (c.1 - n).max(0), (c.2 - n).max(0)) }
fn add(c: (i32, i32, i32), n: i32) -> (i32, i32, i32) { ((c.0 + n).min(255), (c.1 + n).min(255), (c.2 + n).min(255)) }

pub fn _draw_rarity_icon(s: &Surface, shape: &str, c: (i32, i32, i32), icx: i32, icy: i32, r: i32) {
    match shape {
        "circle" => { draw::circle(s, c, (icx, icy), r, 0); }
        "diamond" => draw::polygon(s, c, &[(icx, icy - r), (icx + r, icy), (icx, icy + r), (icx - r, icy)], 0),
        "hexagon" => {
            let pts: Vec<(i32, i32)> = (0..6).map(|n| { let a = ((60 * n - 30) as f64).to_radians(); (icx + (r as f64 * a.cos()) as i32, icy + (r as f64 * a.sin()) as i32) }).collect();
            draw::polygon(s, c, &pts, 0);
        }
        "star4" => {
            let rr = r as f64 * 1.25;
            let (x, y) = (icx as f64, icy as f64);
            let pts = [(x, y - rr), (x + rr * 0.36, y - rr * 0.36), (x + rr, y), (x + rr * 0.36, y + rr * 0.36), (x, y + rr), (x - rr * 0.36, y + rr * 0.36), (x - rr, y), (x - rr * 0.36, y - rr * 0.36)];
            draw::polygon(s, c, &pts, 0);
        }
        _ => {}
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShopAct { Single, X10 }

impl G {
    pub fn roll_power(&self) -> Option<String> {
        let total: i64 = RARITY_PROBS.iter().map(|x| x.1).sum();
        let r = randint(1, total);
        let mut upto = 0;
        let mut chosen = "Common";
        for (rar, p) in RARITY_PROBS.iter() { upto += p; if r <= upto { chosen = rar; break; } }
        let cands: Vec<String> = powers_list().into_iter().filter(|n| power_rarity(n) == chosen).collect();
        if cands.is_empty() { None } else { Some(choice(&cands)) }
    }

    pub fn make_ribbon_with_win(&mut self, power_name: &str, base_tiles: usize) -> usize {
        let mut rarities: Vec<&str> = vec![];
        for (rar, p) in RARITY_PROBS.iter() { for _ in 0..*p { rarities.push(rar); } }
        let all = powers_list();
        let mut tiles = vec![];
        for _ in 0..base_tiles {
            let rar = choice(&rarities);
            let c: Vec<String> = all.iter().filter(|n| power_rarity(n) == rar).cloned().collect();
            tiles.push(if c.is_empty() { choice(&all) } else { choice(&c) });
        }
        let idx = randint(6, 6.max(base_tiles as i64 - 6)) as usize;
        tiles[idx] = power_name.to_string();
        self.ribbon_tiles = tiles;
        idx
    }

    pub fn draw_ribbon_shop(&mut self) {
        let dark = self.is_dark_mode();
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let deco = if !dark { [(200, 210, 255), (220, 200, 255)] } else { [(60, 80, 140), (80, 50, 120)] };
        for (i, dc) in deco.iter().enumerate() {
            let s = Surface::new_alpha(300, 300);
            draw::circle(&s, (dc.0, dc.1, dc.2, 40), (150, 150), 150, 0);
            bs.blit(&s, (i as i32 * 500 - 100, 40));
        }
        let tt = format!("{} - {}", self.get_text("shop"), self.get_text("rollen"));
        let ts = self.font.render(&tt, true, self.get_text_color());
        bs.blit(&self.font.render(&tt, true, (80, 80, 100)), (w / 2 - ts.get_width() / 2 + 2, 22));
        bs.blit(&ts, (w / 2 - ts.get_width() / 2, 20));
        let sf = self.small_font.clone();
        let cb = Rect::new(20, 15, 150, 32);
        self.draw_panel(&bs, cb, 16, (200, 180, 60), 2);
        draw::circle(&bs, GOLD, (cb.x + 18, cb.centery()), 8, 0);
        draw::circle(&bs, (180, 140, 0), (cb.x + 18, cb.centery()), 8, 2);
        bs.blit(&sf.render(&self.stats.gi("coins", 0).to_string(), true, GOLD), (cb.x + 32, cb.y + 7));
        let back = Rect::new(w - 130, 15, 110, 32);
        let nb = self.shop_btns.len();
        let bh = back.collidepoint(mp) || self.nav_is_focused("shop", nb, nb + 1);
        draw::rect(&bs, if bh { self.get_button_hover_color() } else { self.get_button_color() }, back, 0, 16);
        draw::rect(&bs, (160, 160, 160), back, if bh { 3 } else { 2 }, 16);
        let bl = sf.render(&self.get_text("back"), true, self.get_text_color());
        bs.blit(&bl, (back.centerx() - bl.get_width() / 2, back.centery() - bl.get_height() / 2));
        let hint = sf.render(&format!("SPACE: {}  |  X: {}  |  M/ESC: Menü", self.get_text("spin"), self.get_text("spin10")), true, if dark { (150, 150, 150) } else { (110, 110, 110) });
        bs.blit(&hint, (w / 2 - hint.get_width() / 2, h - 30));
        let win = Rect::new(40, RIBBON_Y - TILE_H / 2, w - 80, TILE_H);
        let sh: Color = if dark { (0, 0, 0, 60).into() } else { (180, 180, 200).into() };
        draw::rect(&bs, sh, Rect::new(win.x + 4, win.y + 4, win.w, win.h), 0, 14);
        self.draw_panel(&bs, win, 14, (60, 80, 140), 2);
        if self.ribbon_tiles.is_empty() { return; }
        let total_base = self.ribbon_tiles.len() as i32;
        let total_width = (total_base * TILE_W) as f64;
        let base_offset = self.scroll_offset.rem_euclid(total_width);
        let x0 = win.x - base_offset as i32;
        let now = now_secs();
        let gv = (200.0 + 55.0 * (now * 5.0).sin()) as i32;
        let gc = (gv, (gv as f64 * 0.78) as i32, 30);
        let cx = w / 2;
        let mf = self.menu_font.clone();
        for i in -total_base..total_base * 2 {
            let idx = i.rem_euclid(total_base) as usize;
            let name = self.ribbon_tiles[idx].clone();
            let x = x0 + i * TILE_W;
            let r = Rect::new(x, win.y + 4, TILE_W - 8, TILE_H - 8);
            if r.right() < win.left() || r.left() > win.right() { continue; }
            let rar = power_rarity(&name);
            let rc = power_rarity_colors(&rar);
            let center = r.left() <= cx && cx <= r.right();
            let cbg = if dark { sub(rc.bg, 40) } else { rc.bg };
            draw::rect(&bs, cbg, r, 0, 10);
            if center && (rar == "Legendary" || rar == "Mythic") { draw::rect(&bs, gc, r, 3, 10); }
            else if center { draw::rect(&bs, rc.border, r, 3, 10); }
            else { draw::rect(&bs, sub(rc.border, 60), r, 1, 10); }
            let (icx, icy) = (r.centerx(), r.y + 28);
            draw::circle(&bs, add(cbg, 40), (icx, icy), 18, 0);
            draw::circle(&bs, rc.border, (icx, icy), 18, 2);
            _draw_rarity_icon(&bs, rarity_icon_shape(&rar), rc.border, icx, icy, 9);
            let lab = fit_surf(&mf.render(&self.get_power_name(&name), true, (245, 245, 250)), r.w - 8);
            bs.blit(&lab, (r.centerx() - lab.get_width() / 2, r.y + 58));
            let rs = sf.render(&rar, true, add(rc.border, 60));
            bs.blit(&rs, (r.centerx() - rs.get_width() / 2, r.bottom() - 26));
        }
        draw::polygon(&bs, gc, &[(cx, win.bottom() + 18), (cx - 10, win.bottom() - 2), (cx + 10, win.bottom() - 2)], 0);
        draw::line(&bs, gc, (cx, win.top() - 6), (cx, win.bottom() + 18), 2);
        let coins = self.stats.gi("coins", 0);
        for (bi, (r, label, cost, _)) in self.shop_btns.clone().iter().enumerate() {
            let aff = coins >= *cost;
            let hv = (r.collidepoint(mp) || self.nav_is_focused("shop", bi, nb + 1)) && aff;
            let bg = if !aff { if dark { (50, 50, 55) } else { (210, 210, 210) } } else if hv { (0, 150, 90) } else { (0, 115, 70) };
            draw::rect(&bs, bg, *r, 0, 12);
            draw::rect(&bs, if aff && hv { (0, 200, 120) } else { (150, 150, 150) }, *r, if hv { 3 } else { 2 }, 12);
            let l = sf.render(label, true, if aff { (255, 255, 255) } else { (140, 140, 140) });
            bs.blit(&l, (r.centerx() - l.get_width() / 2, r.y + 8));
            let c = sf.render(&format!("{} Coins", cost), true, if aff { (255, 220, 100) } else { (200, 90, 90) });
            bs.blit(&c, (r.centerx() - c.get_width() / 2, r.y + 32));
        }
        if let Some(items) = self.results_overlay.clone() { self.draw_results_overlay(&items); }
        self.flip();
    }

    pub fn draw_results_overlay(&mut self, items: &[(String, String, bool, i64)]) {
        let dark = self.is_dark_mode();
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let s = Surface::new(w, h);
        s.set_alpha(Some(200));
        s.fill(OVERLAY_BG);
        bs.blit(&s, (0, 0));
        let panel = Rect::new(80, 80, w - 160, h - 160);
        let sh: Color = if dark { (0, 0, 0, 80).into() } else { (170, 170, 190).into() };
        draw::rect(&bs, sh, Rect::new(panel.x + 5, panel.y + 5, panel.w, panel.h), 0, 16);
        self.draw_panel(&bs, panel, 16, (60, 80, 140), 3);
        let tt = self.get_text("results");
        let t = self.menu_font.render(&tt, true, self.get_text_color());
        bs.blit(&self.menu_font.render(&tt, true, (80, 80, 100)), (panel.centerx() - t.get_width() / 2 + 2, panel.y + 18));
        bs.blit(&t, (panel.centerx() - t.get_width() / 2, panel.y + 16));
        let mut sorted = items.to_vec();
        sorted.sort_by_key(|it| RARITY_ORDER.iter().position(|r| *r == it.1).unwrap_or(RARITY_ORDER.len()));
        let row_h = 34;
        let mut y = panel.y + 60;
        let sf = self.small_font.clone();
        for (name, rarity, dup, delta) in sorted {
            let rc = power_rarity_colors(&rarity);
            let rr = Rect::new(panel.x + 16, y, panel.w - 32, row_h - 6);
            let rbg = if dark { sub(rc.bg, 65) } else { add(rc.bg, 130) };
            draw::rect(&bs, rbg, rr, 0, 8);
            draw::rect(&bs, rc.border, rr, 1, 8);
            let (icx, icy) = (rr.x + 20, rr.centery());
            draw::circle(&bs, rc.bg, (icx, icy), 11, 0);
            draw::circle(&bs, rc.border, (icx, icy), 11, 2);
            _draw_rarity_icon(&bs, rarity_icon_shape(&rarity), rc.border, icx, icy, 5);
            let bt = sf.render(&rarity, true, (255, 255, 255));
            let br = Rect::new(rr.x + 40, rr.centery() - 10, bt.get_width() + 12, 20);
            draw::rect(&bs, rc.badge, br, 0, 10);
            bs.blit(&bt, (br.x + 6, br.y + 2));
            let mut label = self.get_power_name(&name);
            if dup { label += &format!("  (Duplikat +{} Coins)", delta); }
            let txt = fit_surf(&sf.render(&label, true, self.get_text_color()), rr.right() - br.right() - 16);
            bs.blit(&txt, (br.right() + 10, rr.centery() - txt.get_height() / 2));
            y += row_h;
        }
        let cr = Rect::new(panel.centerx() - 110, panel.bottom() - 44, 220, 30);
        let ch = cr.collidepoint(self.get_mouse_pos_logical());
        draw::rect(&bs, if ch { self.get_button_hover_color() } else { self.get_button_color() }, cr, 0, 10);
        draw::rect(&bs, (150, 150, 150), cr, 2, 10);
        let hs = sf.render(&self.get_text("enter_click_close_hint"), true, self.get_text_color());
        bs.blit(&hs, (cr.centerx() - hs.get_width() / 2, cr.centery() - hs.get_height() / 2));
    }

    fn powers_vec(&self, k: &str) -> Vec<String> { self.powers_state.gstrs(k) }
    fn set_powers_vec(&mut self, k: &str, v: Vec<String>) { self.powers_state.insert(k.into(), json!(v)); }

    pub fn start_single_spin(&mut self) {
        if self.spin_animating || self.results_overlay.is_some() { return; }
        if self.stats.gi("coins", 0) < SHOP_COST_SINGLE { self.push_overlay("insufficient_coins", (200, 0, 0), 1.5, None); return; }
        let c = self.stats.gi("coins", 0) - SHOP_COST_SINGLE;
        self.stats.insert("coins".into(), json!(c));
        self.save_section("stats", None);
        let power = self.roll_power();
        self.spin_target_power = power.clone();
        let base_index = self.make_ribbon_with_win(power.as_deref().unwrap_or(""), 40);
        let total_width = (self.ribbon_tiles.len() as i32 * TILE_W) as f64;
        let extra_loops = randint(2, 4) as f64;
        let center_x = (self.WIDTH / 2 - TILE_W / 2) as f64;
        let target = extra_loops * total_width + (base_index as i32 * TILE_W) as f64;
        self.spin_final_offset = target - center_x;
        self.scroll_offset = self.scroll_offset.rem_euclid(total_width);
        self.spin_animating = true;
        self.spin_start_time = self.get_ticks();
    }

    pub fn grant_spin_reward(&mut self, name: &str) {
        let mut owned = self.powers_vec("owned");
        if owned.iter().any(|o| o == name) {
            let c = self.stats.gi("coins", 0) + DUPLICATE_REFUND;
            self.stats.insert("coins".into(), json!(c));
            self.save_section("stats", None);
            let t = format!("{}: +{} {}", self.get_text("duplicate"), DUPLICATE_REFUND, self.get_text("coins"));
            self.push_overlay(&t, (60, 60, 60), 1.5, None);
        } else {
            owned.push(name.to_string());
            self.set_powers_vec("owned", owned);
            if name == "Extra Slot" {
                self.push_overlay("extra_slot_unlocked", GOLD, 1.5, None);
            } else {
                let mut eq = self.powers_vec("equipped");
                if eq.len() < self.get_max_slots() { eq.push(name.into()); }
                else if !eq.is_empty() {
                    let dropped = eq.remove(0);
                    eq.push(name.into());
                    let t = format!("{}: {} → {}", self.get_text("auto_swap"), dropped, name);
                    self.push_overlay(&t, (180, 140, 20), 1.5, None);
                } else { eq.push(name.into()); }
                self.set_powers_vec("equipped", eq);
            }
            self.save_section("powers", None);
            let t = format!("{}: {}", self.get_text("new_power"), self.get_power_name(name));
            self.push_overlay(&t, GOLD, 1.5, None);
        }
    }

    pub fn do_roll_x10(&mut self) {
        if self.spin_animating { return; }
        if self.stats.gi("coins", 0) < SHOP_COST_X10 { self.push_overlay("Zu wenig Coins!", (200, 0, 0), 1.5, None); return; }
        let c = self.stats.gi("coins", 0) - SHOP_COST_X10;
        self.stats.insert("coins".into(), json!(c));
        self.save_section("stats", None);
        let mut items = vec![];
        let mut coin_back = 0;
        for _ in 0..10 {
            let Some(p) = self.roll_power() else { continue; };
            let rarity = power_rarity(&p);
            let mut owned = self.powers_vec("owned");
            if owned.contains(&p) { coin_back += DUPLICATE_REFUND; items.push((p, rarity, true, DUPLICATE_REFUND)); continue; }
            owned.push(p.clone());
            self.set_powers_vec("owned", owned);
            if p == "Extra Slot" {
                items.push((p, rarity, false, 0));
                self.push_overlay("Extra Slot freigeschaltet! (passiv)", GOLD, 1.5, None);
                continue;
            }
            let mut eq = self.powers_vec("equipped");
            if eq.len() < self.get_max_slots() { eq.push(p.clone()); }
            else if !eq.is_empty() { eq.remove(0); eq.push(p.clone()); }
            self.set_powers_vec("equipped", eq);
            items.push((p, rarity, false, 0));
        }
        if coin_back > 0 { let c = self.stats.gi("coins", 0) + coin_back; self.stats.insert("coins".into(), json!(c)); }
        self.save_section("stats", None);
        self.save_section("powers", None);
        self.results_overlay = Some(items);
        let t = if coin_back > 0 { format!("Roll x10: +{} Coins zurück", coin_back) } else { "Roll x10 abgeschlossen".into() };
        self.push_overlay(&t, (80, 160, 80), 1.5, None);
    }

    fn run_shop_act(&mut self, a: ShopAct) { match a { ShopAct::Single => self.start_single_spin(), ShopAct::X10 => self.do_roll_x10() } }

    pub fn shop_loop(&mut self) {
        let w = self.WIDTH;
        self.shop_btns = vec![
            (Rect::new(w / 2 - 220, 460, 200, 56), self.get_text("spin"), SHOP_COST_SINGLE, ShopAct::Single),
            (Rect::new(w / 2 + 20, 460, 200, 56), self.get_text("spin10"), SHOP_COST_X10, ShopAct::X10),
        ];
        if self.ribbon_tiles.is_empty() { let p = choice(&powers_list()); self.make_ribbon_with_win(&p, 40); }
        while self.in_shop {
            if self.spin_animating {
                let t = self.get_ticks() - self.spin_start_time;
                if t >= SPIN_DURATION_MS {
                    self.spin_animating = false;
                    self.scroll_offset = self.spin_final_offset;
                    if let Some(p) = self.spin_target_power.take() { self.grant_spin_reward(&p); }
                } else {
                    let k = t as f64 / SPIN_DURATION_MS as f64;
                    let ease = 1.0 - (1.0 - k).powi(3);
                    self.scroll_offset = ease * self.spin_final_offset;
                }
            }
            self.cleanup_overlays();
            self.draw_ribbon_shop();
            let mut rects: Vec<Rect> = self.shop_btns.iter().map(|b| b.0).collect();
            rects.push(Rect::new(self.WIDTH - 130, 15, 110, 32));
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self.results_overlay.is_some() {
                            if self.is_nav_confirm(key) || self.is_back_key(key) { self.results_overlay = None; }
                        } else if key == K_m || self.is_back_key(key) { self.in_shop = false; self.in_menu = true; }
                        else if key == K_x { self.do_roll_x10(); }
                        else if let Some(d) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("shop", rects.len());
                            let n = nav_move(&rects, cur, d);
                            self.nav_set_focus("shop", n);
                        } else if self.is_nav_confirm(key) {
                            let fi = self.nav_get_focus("shop", rects.len());
                            if let Some(fi) = fi {
                                if fi < self.shop_btns.len() { let a = self.shop_btns[fi].3; self.run_shop_act(a); }
                                else if fi == self.shop_btns.len() { self.in_shop = false; self.in_menu = true; }
                            }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let pos = self.get_mouse_pos_logical();
                        if self.results_overlay.is_some() { self.results_overlay = None; } else {
                            let back = Rect::new(self.WIDTH - 130, 15, 110, 32);
                            if back.collidepoint(pos) { self.play_sound(Snd::ClickBack); self.in_shop = false; self.in_menu = true; } else {
                                for (r, _, _, a) in self.shop_btns.clone() { if r.collidepoint(pos) { self.run_shop_act(a); break; } }
                            }
                        }
                    }
                    _ => {}
                }
            }
            let bs = self.base_surface.clone();
            let mut y = 12;
            for o in self.overlays.clone() {
                let t = self.small_font.render(&o.text, true, o.color);
                bs.blit(&t, ((self.WIDTH - t.get_width()) / 2, y));
                y += t.get_height() + 4;
            }
            self.draw_achievement_popups(&bs);
            self.flip();
            { let fps = self.target_fps(); self.clock.tick(fps); }
        }
    }

    pub fn to_power_loadout(&mut self) { self.in_loadout_menu = false; self.in_loadout = true; self.play_sound_v(Snd::MenuClick, "menu_click_volume"); }

    pub fn set_flag(&mut self, name: &str, v: bool) {
        match name {
            "in_loadout_menu" => self.in_loadout_menu = v,
            "in_profile_hub" => self.in_profile_hub = v,
            "in_menu" => self.in_menu = v,
            "in_cosmetic_shop" => self.in_cosmetic_shop = v,
            "in_cosmetic_inventory" => self.in_cosmetic_inventory = v,
            "in_shop_menu" => self.in_shop_menu = v,
            _ => {}
        }
    }

    pub fn _cosmetic_inventory_back(&mut self) {
        self.in_cosmetic_inventory = false;
        let s = self._cosmetic_inventory_return_screen.clone();
        self.set_flag(&s, true);
    }
    pub fn to_cosmetic_inventory(&mut self) {
        self.in_loadout_menu = false;
        self.in_cosmetic_inventory = true;
        self._cosmetic_inventory_return_screen = "in_loadout_menu".into();
    }
}

#[allow(unused)]
fn _u(_: Value) {}
