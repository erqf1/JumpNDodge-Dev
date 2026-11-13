//! Classic-Modus, Menü-Umleitungen, Ladebildschirm, Daily Rewards, Hilfe (Python 10393-10815).

use super::core::*;
use super::online_screens::*;
use super::state::{Snd, G};
use crate::pg::*;
use crate::util::*;

impl G {
    /// JumpNDodge Classic - Original-Logik aus Spiel.py, eingebettet (nicht anfassen)
    pub fn run_classic_game(&mut self) {
        self.stop_music();
        let screen = Surface::new(800, 600);
        let (width, height) = (800i32, 600i32);
        let white = (255, 255, 255);
        let green = (0, 255, 0);
        let blue = (0, 0, 255);
        let red = (255, 0, 0);
        let player_size = 50.0;
        let mut player_x = (width / 2) as f64;
        let mut player_y = (height - 70) as f64 - player_size;
        let player_velocity = 5.0;
        let mut is_jumping = false;
        let gravity = 0.5;
        let jump_speed = 10.0;
        let mut y_velocity = 0.0;
        let mut health = 3;
        let enemy_size = 50.0;
        let enemy_velocity = 2.0;
        let enemy_spawn_interval = 2000;
        let mut last_spawn = self.get_ticks();
        let mut enemies: Vec<(f64, f64, f64)> = vec![];
        let platform_height = 20;
        let platform_y = (height - platform_height - 50) as f64;
        let mut score = 0;
        let mut clock = Clock::new();
        let classic_font = Font::new(36);
        let mut running = true;
        while running {
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => { if key == K_ESCAPE || self.is_back_key(key) { running = false; } }
                    _ => {}
                }
            }
            if self.display.key_pressed(K_a) { player_x -= player_velocity; }
            if self.display.key_pressed(K_d) { player_x += player_velocity; }
            if self.display.key_pressed(K_SPACE) && !is_jumping { is_jumping = true; y_velocity = -jump_speed; }
            if is_jumping {
                player_y += y_velocity;
                y_velocity += gravity;
                if player_y >= platform_y - player_size { player_y = platform_y - player_size; is_jumping = false; y_velocity = 0.0; }
            }
            let now = self.get_ticks();
            if now - last_spawn > enemy_spawn_interval {
                if choice(&["left", "right"]) == "left" { enemies.push((-enemy_size, platform_y - enemy_size, 1.0)); }
                else { enemies.push((width as f64, platform_y - enemy_size, -1.0)); }
                last_spawn = now;
            }
            for e in enemies.iter_mut() { e.0 += e.2 * enemy_velocity; }
            let pr = rect(player_x, player_y, player_size, player_size);
            let mut i = 0;
            while i < enemies.len() {
                let e = enemies[i];
                let er = rect(e.0, e.1, enemy_size, enemy_size);
                if pr.colliderect(&er) {
                    if player_y + player_size - y_velocity <= e.1 {
                        enemies.remove(i);
                        score += 1;
                        continue;
                    } else {
                        health -= 1;
                        player_x = (width / 2) as f64;
                        player_y = platform_y - player_size;
                        if health == 0 { running = false; }
                    }
                }
                i += 1;
            }
            enemies.retain(|e| 0.0 <= e.0 && e.0 <= width as f64);
            screen.fill(white);
            draw::rect(&screen, green, rect(0, platform_y, width, platform_height), 0, 0);
            draw::rect(&screen, blue, rect(player_x, player_y, player_size, player_size), 0, 0);
            for e in &enemies { draw::rect(&screen, red, rect(e.0, e.1, enemy_size, enemy_size), 0, 0); }
            screen.blit(&classic_font.render(&format!("Health: {}", health), true, (0, 0, 0)), (10, 10));
            screen.blit(&classic_font.render(&format!("Score: {}", score), true, (0, 0, 0)), (10, 50));
            let el = self.small_font.render(&self.get_text("esc_back"), true, (0, 0, 0));
            screen.blit(&el, (width / 2 - el.get_width() / 2, 6));
            self.sync_window_size();
            let bs = self.base_surface.clone();
            let (bw, bh) = bs.get_size();
            let fs = (bw as f64 / 800.0).min(bh as f64 / 600.0);
            let (sw, sh) = (1.max((800.0 * fs) as i32), 1.max((600.0 * fs) as i32));
            let scaled = transform::smoothscale(&screen, (sw, sh));
            bs.fill((0, 0, 0));
            bs.blit(&scaled, ((bw - sw) / 2, (bh - sh) / 2));
            self.flip();
            clock.tick(30);
        }
        self.in_classic_game = false;
        self.in_more_menu = true;
        if self.sound_enabled && self.menu_music_path.is_some() { self.play_menu_music(); }
    }

    pub fn back_to_main_from_more(&mut self) { self.play_sound(Snd::ClickBack); self.in_more_menu = false; self.in_menu = true; }
    pub fn open_experimental_menu(&mut self) { self.play_sound(Snd::MenuClick); self.in_settings_menu = false; self.in_experimental_menu = true; }
    pub fn open_settings_sound_menu(&mut self) { self.play_sound(Snd::MenuClick); self.in_settings_menu = false; self.in_settings_sound_menu = true; }
    pub fn open_controls_menu(&mut self) { self.play_sound(Snd::MenuClick); self.in_settings_menu = false; self.in_controls_menu = true; }
    pub fn back_to_main_from_settings(&mut self) { self.play_sound(Snd::ClickBack); self.in_settings_menu = false; self.in_menu = true; }
    pub fn back_to_settings_from_experimental(&mut self) { self.play_sound(Snd::ClickBack); self.in_experimental_menu = false; self.in_settings_more_menu = true; }
    pub fn back_to_settings_from_sound(&mut self) { self.play_sound(Snd::ClickBack); self.in_settings_sound_menu = false; self.in_settings_menu = true; }
    pub fn back_to_main_from_daily_rewards(&mut self) {
        self.in_daily_rewards = false;
        self.play_sound_v(Snd::ClickBack, "click_back_volume");
        self.start_loading_screen(false);
    }
    pub fn finish_loading_to_main(&mut self) { self.in_loading = false; self.in_menu = true; }

    pub fn draw_loading_screen(&mut self) {
        let elapsed = self.get_ticks() - self.loading_start_time;
        self.loading_progress = (elapsed as f64 / self.loading_duration as f64 * 100.0).min(100.0);
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let lt = self.font.render(&self.get_text("loading"), true, self.get_text_color());
        bs.blit(&lt, (w / 2 - lt.get_width() / 2, h / 2 - 50));
        let (bw, bh) = (400, 20);
        let (bx, by) = (w / 2 - bw / 2, h / 2 + 20);
        draw::rect(&bs, self.get_panel_color(), Rect::new(bx, by, bw, bh), 0, 10);
        let pw = (self.loading_progress / 100.0 * bw as f64) as i32;
        draw::rect(&bs, (0, 255, 0), Rect::new(bx, by, pw, bh), 0, 10);
        let pt = self.small_font.render(&format!("{}%", self.loading_progress as i64), true, self.get_text_color());
        bs.blit(&pt, (w / 2 - pt.get_width() / 2, by + 30));
        if self.loading_progress >= 100.0 {
            self.in_loading = false;
            if self.loading_to_daily_rewards { self.in_daily_rewards = true; } else { self.in_menu = true; }
        }
        self.flip();
    }

    pub fn draw_daily_rewards_menu(&mut self) {
        let mp = self.get_mouse_pos_logical();
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let t = self.font.render(&self.get_text("daily_rewards"), true, self.get_text_color());
        bs.blit(&self.font.render(&self.get_text("daily_rewards"), true, (100, 100, 120)), (w / 2 - t.get_width() / 2 + 3, 73));
        bs.blit(&t, (w / 2 - t.get_width() / 2, 70));
        self.draw_coin_display(&bs);
        let mp_ = Rect::new(50, 120, w - 100, h - 200);
        self.draw_panel(&bs, mp_, 16, (60, 80, 140), 3);
        let mf = self.menu_font.clone();
        let sf = self.small_font.clone();
        let mut y = 150;
        let (can, msg) = self.can_claim_daily_reward();
        if can {
            bs.blit(&mf.render(&self.get_text("daily_reward_available"), true, (0, 150, 0)), (70, y));
            y += 50;
            let cr = Rect::new(w / 2 - 100, y, 200, 50);
            let hv = cr.collidepoint(mp) || self.nav_is_focused("daily_rewards", 0, 2);
            self.draw_ui_button(&bs, cr, &self.get_text("claim_reward"), BtnOpts { font: Some(mf.clone()), hovered: hv, base_color: Some((0, 180, 0)), hover_color: Some((0, 210, 40)),
                text_color: Some((255, 255, 255)), border_color: Some((0, 120, 0)), hover_border_color: Some((150, 255, 170)), hover_border_width: Some(3), ..Default::default() });
            y += 80;
        } else if msg.contains("Internet") {
            bs.blit(&mf.render(&self.get_text("daily_reward_no_internet"), true, (200, 100, 100)), (70, y));
            y += 50;
        } else {
            bs.blit(&mf.render(&self.get_text("daily_reward_claimed"), true, (150, 150, 150)), (70, y));
            y += 50;
            bs.blit(&sf.render(&self.get_text("next_reset_in"), true, self.get_text_color()), (70, y));
            y += 30;
            let tu = self.get_time_until_reset();
            bs.blit(&mf.render(&tu, true, (255, 100, 100)), (70, y));
            y += 60;
        }
        let last = self.stats.gobj("last_daily_reward_stats");
        if !last.is_empty() {
            bs.blit(&mf.render(&format!("{}:", self.get_text("last_reward")), true, self.get_text_color()), (70, y));
            y += 40;
            bs.blit(&sf.render(&format!("{} {}", last.gi("total", 0), self.get_text("coins")), true, (255, 215, 0)), (90, y));
            y += 25;
            let rarity = last.gs("rarity", "Normal");
            let rc = match rarity.as_str() { "Rare" => (80, 150, 255), "Epic" => (180, 80, 255), _ => (130, 130, 140) };
            bs.blit(&sf.render(&format!("{}: {}", self.get_text("rarity_label"), self.get_text(&format!("rarity_{}", rarity.to_lowercase()))), true, rc), (90, y));
            y += 25;
            let extra = last.gi("extra", 0);
            if extra > 0 {
                let ec = if rarity == "Epic" { (180, 80, 255) } else { (80, 150, 255) };
                bs.blit(&sf.render(&format!("+{} {} Extra", extra, self.get_text("coins")), true, ec), (90, y));
            }
        }
        let br = Rect::new(w - 180, h - 60, 150, 40);
        let bi = if can { 1 } else { 0 };
        let hv = br.collidepoint(mp) || self.nav_is_focused("daily_rewards", bi, bi + 1);
        self.draw_ui_button(&bs, br, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: hv, radius: Some(10), ..Default::default() });
        self.flip();
    }

    pub fn _load_help_texts(&mut self) {
        self.HELP_TEXTS.clear();
        for (lang, fname) in [("Deutsch", "README_DE.txt"), ("English", "README_EN.txt")] {
            let txt = std::fs::read_to_string(fname).or_else(|_| std::fs::read_to_string(self.BASE_DIR.join(fname)));
            self.HELP_TEXTS.insert(lang.to_string(), txt.unwrap_or_else(|_| format!("Keine Anleitung für {}.", lang)));
        }
    }

    pub fn open_help_menu(&mut self) {
        self.in_more_menu = false;
        self.in_help = true;
        self.help_scroll = 0;
        let lang = self.lang();
        self.help_lang = Some(lang.clone());
        let txt = self.HELP_TEXTS.get(&lang).or_else(|| self.HELP_TEXTS.get("English")).cloned().unwrap_or_default();
        self.help_lines = txt.split('\n').map(|s| s.to_string()).collect();
        self.play_sound(Snd::MenuClick);
    }

    pub fn draw_help_menu(&mut self) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let t = self.font.render(&self.get_text("help"), true, self.get_text_color());
        bs.blit(&t, (w / 2 - t.get_width() / 2, 20));
        let panel = Rect::new(20, 90, w - 40, h - 146);
        self.draw_card(&bs, panel, 14, false, None, 3, false, None);
        let prev = bs.get_clip();
        bs.set_clip(Some(panel.inflate(-16, -16)));
        let mut y0 = panel.y + 14 - self.help_scroll;
        for line in self.help_lines.clone() {
            let ts = self.small_font.render(&line, true, self.get_text_color());
            bs.blit(&ts, (panel.x + 20, y0));
            y0 += 26;
        }
        bs.set_clip(Some(prev));
        let hint = self.small_font.render(&self.get_text("scroll_back_hint"), true, self.get_text_color());
        bs.blit(&hint, (w / 2 - hint.get_width() / 2, h - 40));
        self.flip();
    }

    pub fn switch_stat_tab(&mut self) {
        let vis = if self.mods_installed() { 4 } else { 3 };
        self.current_stat_tab = (self.current_stat_tab + 1) % vis;
    }
    pub fn adjust_stat_scroll(&mut self, delta: i32) { self.stat_scroll = 0.max(self.stat_scroll + delta); }
    pub fn adjust_loadout_scroll(&mut self, delta: i32) { self.loadout_scroll = 0.max(self.loadout_scroll + delta); }
}

#[allow(unused)]
fn _unused() { let _ = fmt_thousands(0); }
