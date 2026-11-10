//! Game-Over-Screens: show_game_over_2p (Python 16156-16257), show_game_over (16268-16384).

use super::core::*;
use super::state::{Snd, G};
use crate::game::save::DIFFICULTIES;
use crate::pg::*;

impl G {
    pub fn show_game_over_2p(&mut self, score: i64, winner: Option<&str>, survived_seconds: i64) {
        self.play_sound_v(Snd::GameOver, "gameover_volume");
        self.ping_server_online(None);
        loop {
            self.fill_bg(true);
            let bs = self.base_surface.clone();
            let (w, h) = (self.WIDTH, self.HEIGHT);
            let cx = w / 2;
            let (title_text, title_color) = match winner {
                Some("p1") => ("Spieler 1 gewinnt!".to_string(), BLUE),
                Some("p2") => ("Spieler 2 gewinnt!".to_string(), (255, 120, 0)),
                Some("draw") => ("Unentschieden!".to_string(), (220, 200, 0)),
                _ => ("Game Over!".to_string(), self.get_text_color()),
            };
            let panel = Rect::new(cx - 240, h / 2 - 200, 480, 320);
            self.draw_card(&bs, panel, 20, true, Some(title_color), 3, false, None);
            let ts = self.font.render(&title_text, true, title_color);
            let tsh = self.font.render(&title_text, true, (0, 0, 0));
            bs.blit(&tsh, (cx - ts.get_width() / 2 + 3, h / 2 - 148));
            bs.blit(&ts, (cx - ts.get_width() / 2, h / 2 - 150));
            let sc = self.menu_font.render(&format!("Score: {}   ·   {}s", score, survived_seconds), true, self.get_text_color());
            bs.blit(&sc, (cx - sc.get_width() / 2, h / 2 - 75));
            let mode_str = if self.two_player_submode == "vs" { self.get_text("vs_mode_label") } else { self.get_text("coop_mode_label") };
            let diff = DIFFICULTIES[self.difficulty_index];
            let sub = self.small_font.render(&format!("{}  ·  {}", mode_str, self.get_difficulty_name(diff)), true, (140, 60, 200));
            bs.blit(&sub, (cx - sub.get_width() / 2, h / 2 - 45));
            let btn_y = h / 2 + 20;
            let menu_rect = Rect::new(cx - 215, btn_y, 195, 46);
            let rematch_rect = Rect::new(cx + 20, btn_y, 195, 46);
            let rects = [menu_rect, rematch_rect];
            let mp = self.get_mouse_pos_logical();
            let menu_hl = menu_rect.collidepoint(mp) || self.nav_is_focused_d("game_over_2p", 0, 2, 1);
            let rematch_hl = rematch_rect.collidepoint(mp) || self.nav_is_focused_d("game_over_2p", 1, 2, 1);
            let mf = self.menu_font.clone();
            self.draw_ui_button(&bs, menu_rect, &self.get_text("back"), BtnOpts { font: Some(mf.clone()), hovered: menu_hl, ..Default::default() });
            self.draw_ui_button(&bs, rematch_rect, "Rematch ↺", BtnOpts {
                font: Some(mf.clone()), hovered: rematch_hl, base_color: Some((0, 130, 60)), hover_color: Some((30, 175, 90)),
                text_color: Some((255, 255, 255)), border_color: Some((60, 90, 70)), hover_border_color: Some((190, 255, 210)),
                hover_border_width: Some(3), ..Default::default()
            });
            if self._server_reachable == Some(false) {
                let off = self.small_font.render(&self.get_text("offline_hint"), true, (210, 150, 70));
                bs.blit(&off, (cx - off.get_width() / 2, btn_y + 58));
            }
            self.flip();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self.is_back_key(key) { self.game_running = false; self.in_menu = true; return; }
                        if let Some(d) = self.nav_direction(key) {
                            let cur = self.nav_get_focus_d("game_over_2p", 2, 1);
                            self.nav_set_focus("game_over_2p", nav_move(&rects, cur, d));
                        } else if self.is_nav_confirm(key) {
                            match self.nav_get_focus_d("game_over_2p", 2, 1) {
                                Some(0) => { self.game_running = false; self.in_menu = true; return; }
                                Some(1) => { self.game_running = true; return; }
                                _ => {}
                            }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let mp2 = self.get_mouse_pos_logical();
                        if menu_rect.collidepoint(mp2) { self.play_sound(Snd::ClickBack); self.game_running = false; self.in_menu = true; return; }
                        if rematch_rect.collidepoint(mp2) { self.play_sound(Snd::MenuClick); self.game_running = true; return; }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn show_game_over(&mut self, score_display: i64, coins_gained: i64, old_highscore: i64, new_record: bool, survived_seconds: i64) {
        self.play_sound_v(Snd::GameOver, "gameover_volume");
        let new_hs = old_highscore.max(score_display);
        self.ping_server_online(None);
        loop {
            self.fill_bg(true);
            let bs = self.base_surface.clone();
            let (w, h) = (self.WIDTH, self.HEIGHT);
            let cx = w / 2;
            let panel = Rect::new(cx - 240, h / 2 - 300, 480, 480);
            self.draw_card(&bs, panel, 20, true, None, 3, new_record, if new_record { Some((255, 215, 0)) } else { None });
            let av = Rect::new(cx - 25, h / 2 - 265, 50, 50);
            self.draw_avatar(&bs, av, None, true, None);
            let name = self.current_profile_name.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| "-".into());
            let ns = self.small_font.render(&name, true, self.get_text_color());
            bs.blit(&ns, (cx - ns.get_width() / 2, av.bottom() + 4));
            let go = self.get_text("game_over");
            let over = self.font.render(&go, true, self.get_text_color());
            let osh = self.font.render(&go, true, if self.is_dark_mode() { (60, 60, 75) } else { (170, 170, 190) });
            bs.blit(&osh, (cx - over.get_width() / 2 + 2, h / 2 - 168));
            bs.blit(&over, (cx - over.get_width() / 2, h / 2 - 170));
            let sl = self.menu_font.render(&format!("{}:  {}", self.get_text("score"), score_display), true, self.get_text_color());
            let tl = self.menu_font.render(&format!("{}s", survived_seconds), true, (140, 180, 220));
            let tw = sl.get_width() + 20 + tl.get_width();
            bs.blit(&sl, (cx - tw / 2, h / 2 - 100));
            bs.blit(&tl, (cx - tw / 2 + sl.get_width() + 20, h / 2 - 100));
            if new_record {
                let col = ui_glow_pulse((255, 215, 0), 4.0, 0.75, None);
                let l1 = self.menu_font.render(&format!("Neuer Highscore:  {}", new_hs), true, col);
                let l2 = self.small_font.render(&format!("Alter Highscore:  {}", old_highscore), true, (170, 170, 170));
                bs.blit(&l1, (cx - l1.get_width() / 2, h / 2 - 52));
                bs.blit(&l2, (cx - l2.get_width() / 2, h / 2 - 20));
            } else {
                let l = self.menu_font.render(&format!("Highscore:  {}", new_hs), true, (180, 180, 210));
                bs.blit(&l, (cx - l.get_width() / 2, h / 2 - 44));
            }
            let gs = self.small_font.render(&format!("+{} Coins", coins_gained), true, GOLD);
            bs.blit(&gs, (cx - gs.get_width() / 2, h / 2 + 20));
            let btn_y = h / 2 + 60;
            let retry_rect = Rect::new(cx - 215, btn_y, 195, 46);
            let menu_rect = Rect::new(cx + 20, btn_y, 195, 46);
            let rects = [retry_rect, menu_rect];
            let mp = self.get_mouse_pos_logical();
            let retry_hl = retry_rect.collidepoint(mp) || self.nav_is_focused("game_over_1p", 0, 2);
            let menu_hl = menu_rect.collidepoint(mp) || self.nav_is_focused("game_over_1p", 1, 2);
            let mf = self.menu_font.clone();
            self.draw_ui_button(&bs, retry_rect, &self.get_text("retry"), BtnOpts {
                font: Some(mf.clone()), hovered: retry_hl, base_color: Some((0, 130, 60)), hover_color: Some((30, 175, 90)),
                text_color: Some((255, 255, 255)), border_color: Some((60, 90, 70)), hover_border_color: Some((190, 255, 210)),
                hover_border_width: Some(3), ..Default::default()
            });
            self.draw_ui_button(&bs, menu_rect, &self.get_text("back"), BtnOpts { font: Some(mf.clone()), hovered: menu_hl, ..Default::default() });
            let rs = self.small_font.render(&format!("R: {}   ·   M: {}", self.get_text("retry"), self.get_text("back")), true, (130, 130, 150));
            bs.blit(&rs, (cx - rs.get_width() / 2, btn_y + 58));
            if self._server_reachable == Some(false) {
                let off = self.small_font.render(&self.get_text("offline_hint"), true, (210, 150, 70));
                bs.blit(&off, (cx - off.get_width() / 2, btn_y + 58 + rs.get_height() + 6));
            }
            self.flip();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if key == K_r { self.game_running = true; self.in_menu = false; return; }
                        if key == K_m || self.is_back_key(key) { self.in_menu = true; self.game_running = false; return; }
                        if let Some(d) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("game_over_1p", 2);
                            self.nav_set_focus("game_over_1p", nav_move(&rects, cur, d));
                        } else if self.is_nav_confirm(key) {
                            match self.nav_get_focus("game_over_1p", 2) {
                                Some(0) => { self.game_running = true; self.in_menu = false; return; }
                                Some(1) => { self.in_menu = true; self.game_running = false; return; }
                                _ => {}
                            }
                        }
                    }
                    Event::MouseButtonDown { .. } => {
                        let mp2 = self.get_mouse_pos_logical();
                        if retry_rect.collidepoint(mp2) { self.play_sound(Snd::MenuClick); self.game_running = true; self.in_menu = false; return; }
                        if menu_rect.collidepoint(mp2) { self.play_sound(Snd::ClickBack); self.in_menu = true; self.game_running = false; return; }
                    }
                    _ => {}
                }
            }
        }
    }
}
