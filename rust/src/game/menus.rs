//! Menü-Funktionen (Python 4450-5820): Bildschirmwechsel, Daily-Reward-Panel, Hauptmenü,
//! Schwierigkeitsauswahl.

use super::core::*;
use super::state::{Snd, G};
use crate::data::d;
use crate::game::save::DIFFICULTIES;
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

/// Button-Klasse (Python class Button): Text + Callback
#[derive(Clone)]
pub struct Button {
    pub rect: Rect,
    pub text: String,
    pub action: MenuAction,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction { StartGame, OpenShop, OpenLoadout, OpenSettings, OpenMore }

impl G {
    pub fn switch_language(&mut self) {
        self.language_index = (self.language_index + 1) % self.languages.len();
        let l = self.lang();
        self.settings.insert("language".into(), json!(l));
        self.save_section("settings", None);
        self.update_button_texts();
        self.play_sound(Snd::Click);
    }

    pub fn switch_difficulty(&mut self) {
        self.difficulty_index = (self.difficulty_index + 1) % DIFFICULTIES.len();
        self.settings.insert("difficulty".into(), json!(DIFFICULTIES[self.difficulty_index]));
        self.save_section("settings", None);
        self.update_button_texts();
        self.play_sound(Snd::Click);
    }

    pub fn start_game(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_menu = false;
        self.in_difficulty_select = true;
        self._ds_slider_edit = None;
    }

    pub fn actually_start_game(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.klassik_mode = self.current_game_mode == "klassik";
        self.request_fade(Box::new(|g| {
            g.in_difficulty_select = false;
            g.game_running = true;
        }));
    }

    pub fn show_statistics(&mut self) { self.in_menu = false; self.showing_stats = true; }
    pub fn open_shop_menu(&mut self) { self.play_sound(Snd::MenuClick); self.in_menu = false; self.in_shop_menu = true; }
    pub fn open_loadout_menu(&mut self) { self.play_sound(Snd::MenuClick); self.in_menu = false; self.in_loadout_menu = true; }
    pub fn open_profile_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_menu = false;
        self.in_profile_hub = true;
        self.ping_server_online(None);
    }
    pub fn back_to_main_from_profile_hub(&mut self) { self.play_sound(Snd::ClickBack); self.in_profile_hub = false; self.in_menu = true; }
    pub fn open_profile_select_from_hub(&mut self) { self.play_sound(Snd::MenuClick); self.in_profile_hub = false; self.in_profile_select = true; }
    pub fn open_online_leaderboard_from_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_online_hub = false;
        if self._gate_blocks_online_access() { return; }
        self.in_online_leaderboard = true;
    }
    pub fn back_to_hub_from_online_leaderboard(&mut self) { self.play_sound(Snd::ClickBack); self.in_online_leaderboard = false; self.in_online_hub = true; }
    pub fn open_player_search(&mut self, return_screen: &str) {
        self.play_sound(Snd::MenuClick);
        self.in_online_leaderboard = false;
        self.in_friends_hub = false;
        self.in_player_search = true;
        self._search_query.clear();
        self._search_results = None;
        self._search_loading = false;
        self._player_search_return_screen = return_screen.to_string();
    }
    pub fn back_from_player_search(&mut self) {
        self.play_sound(Snd::ClickBack);
        self.in_player_search = false;
        if self._player_search_return_screen == "in_friends_hub" { self.in_friends_hub = true; } else { self.in_online_leaderboard = true; }
    }
    pub fn open_public_profile(&mut self, username: &str, return_screen: &str) {
        self.play_sound(Snd::MenuClick);
        self.in_online_leaderboard = false;
        self.in_player_search = false;
        self.in_friends_hub = false;
        self.in_clan_hub = false;
        self.in_public_profile = true;
        self._public_profile_username = Some(username.to_string());
        self._public_profile_data = None;
        self._public_profile_loading = false;
        self._public_profile_return_screen = return_screen.to_string();
        self._friend_action_pending = None;
        self._public_profile_friend_msg.clear();
    }
    pub fn open_redeem_code_from_more(&mut self) {
        self.in_more_menu = false;
        self.in_redeem_code = true;
        self._redeem_code_text.clear();
        self._redeem_code_msg.clear();
        self._redeem_code_busy = false;
    }
    pub fn open_rename_profile_from_hub(&mut self) {
        self.play_sound(Snd::MenuClick);
        self.in_profile_hub = false;
        self.in_rename_profile = true;
        self._rename_text.clear();
        self._rename_msg.clear();
        self._rename_busy = false;
    }
    pub fn back_from_public_profile(&mut self) {
        self.play_sound(Snd::ClickBack);
        self.in_public_profile = false;
        match self._public_profile_return_screen.as_str() {
            "in_player_search" => self.in_player_search = true,
            "in_friends_hub" => self.in_friends_hub = true,
            "in_clan_hub" => self.in_clan_hub = true,
            _ => self.in_online_leaderboard = true,
        }
    }

    pub fn get_daily_reward_status(&mut self) -> String {
        let (can, msg) = self.can_claim_daily_reward();
        if !can {
            if msg.contains("Internet") { return self.get_text("daily_reward_no_internet"); }
            return self.get_text("daily_reward_claimed");
        }
        self.get_text("daily_reward_available")
    }

    pub fn get_time_until_reset(&mut self) -> String {
        let (now, has) = self.get_cached_now();
        if !has { return "Keine Internetverbindung".into(); }
        let next = (now.date() + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap();
        let diff = (next - now).num_milliseconds() as f64 / 1000.0;
        let hours = (diff / 3600.0).floor() as i64;
        let minutes = ((diff % 3600.0) / 60.0).floor() as i64;
        let seconds = (diff % 60.0).floor() as i64;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }

    pub fn claim_daily_reward_from_menu(&mut self) -> bool {
        let ok = self.claim_daily_reward();
        if ok { self.update_button_texts(); }
        ok
    }

    pub fn draw_daily_reward_panel(&mut self) {
        let bs = self.base_surface.clone();
        let panel = Rect::new(self.WIDTH - 320, 60, 300, 200);
        draw::rect(&bs, (245, 248, 255), panel, 0, 12);
        draw::rect(&bs, (60, 80, 140), panel, 2, 12);
        let sf = self.small_font.clone();
        bs.blit(&sf.render(&self.get_text("daily_reward"), true, (30, 30, 60)), (panel.x + 15, panel.y + 15));
        let (can, msg) = self.can_claim_daily_reward();
        let st = if can { sf.render(&self.get_text("daily_reward_available"), true, (0, 150, 0)) }
            else if msg.contains("Internet") { sf.render(&self.get_text("daily_reward_no_internet"), true, (200, 100, 100)) }
            else { sf.render(&self.get_text("daily_reward_claimed"), true, (150, 150, 150)) };
        bs.blit(&st, (panel.x + 15, panel.y + 40));
        let last = self.stats.gobj("last_daily_reward_stats");
        if !last.is_empty() {
            let mut yo = 70;
            bs.blit(&sf.render(&format!("{}:", self.get_text("last_reward")), true, (80, 80, 80)), (panel.x + 15, panel.y + yo));
            yo += 25;
            bs.blit(&sf.render(&format!("{} {}", last.gi("total", 0), self.get_text("coins")), true, (255, 215, 0)), (panel.x + 15, panel.y + yo));
            yo += 20;
            let rarity = last.gs("rarity", "Normal");
            let rc = match rarity.as_str() { "Rare" => (80, 150, 255), "Epic" => (180, 80, 255), _ => (130, 130, 140) };
            let rn = self.get_text(&format!("rarity_{}", rarity.to_lowercase()));
            bs.blit(&sf.render(&format!("{}: {}", self.get_text("rarity_label"), rn), true, rc), (panel.x + 15, panel.y + yo));
            yo += 20;
            let extra = last.gi("extra", 0);
            if extra > 0 {
                let ec = if rarity == "Epic" { (180, 80, 255) } else { (80, 150, 255) };
                bs.blit(&sf.render(&format!("+{} {} Extra", extra, self.get_text("coins")), true, ec), (panel.x + 15, panel.y + yo));
            }
        }
        bs.blit(&sf.render(&self.get_text("reset_time"), true, (120, 120, 120)), (panel.x + 15, panel.bottom() - 25));
    }

    pub fn start_loading_screen(&mut self, to_daily_rewards: bool) {
        self.in_menu = false;
        self.in_loading = true;
        self.loading_progress = 0.0;
        self.loading_start_time = self.get_ticks();
        self.loading_to_daily_rewards = to_daily_rewards;
    }

    pub fn open_daily_rewards_menu(&mut self) { self.play_sound(Snd::MenuClick); self.start_loading_screen(true); }
    pub fn handle_daily_reward(&mut self) { self.open_daily_rewards_menu(); }

    pub fn open_admin_login(&mut self) {
        self.in_menu = false;
        self.in_admin_login = true;
        self.admin_pwd_input.clear();
        self.admin_msg.clear();
        self.play_sound(Snd::MenuClick);
    }

    pub fn open_more_menu(&mut self) { self.play_sound(Snd::MenuClick); self.set_all_menu_flags(false); self.in_more_menu = true; }
    pub fn open_settings_menu(&mut self) { self.play_sound(Snd::MenuClick); self.set_all_menu_flags(false); self.in_settings_menu = true; }

    pub fn update_button_texts(&mut self) {
        let keys = ["start", "shop", "loadout", "settings", "more"];
        for (i, k) in keys.iter().enumerate() {
            if i < self.buttons.len() { self.buttons[i].text = self.get_text(k); }
        }
    }

    pub fn create_main_buttons(&mut self) {
        let sx = self.WIDTH / 2 - 180;
        let (sy, gap) = (155, 56);
        let defs = [("start", MenuAction::StartGame), ("shop", MenuAction::OpenShop), ("loadout", MenuAction::OpenLoadout), ("settings", MenuAction::OpenSettings), ("more", MenuAction::OpenMore)];
        self.buttons = defs.iter().enumerate().map(|(i, (k, a))| Button { rect: Rect::new(sx, sy + i as i32 * gap, 360, 48), text: self.get_text(k), action: *a }).collect();
    }

    pub fn run_menu_action(&mut self, a: MenuAction) {
        match a {
            MenuAction::StartGame => self.start_game(),
            MenuAction::OpenShop => self.open_shop_menu(),
            MenuAction::OpenLoadout => self.open_loadout_menu(),
            MenuAction::OpenSettings => self.open_settings_menu(),
            MenuAction::OpenMore => self.open_more_menu(),
        }
    }

    pub fn _get_profile_hub_btn_rect(&self) -> Rect { Rect::new(15, self.HEIGHT - 118, 220, 56) }

    pub fn draw_menu(&mut self) {
        let mouse_pos = self.get_mouse_pos_logical();
        if self.sound_enabled && self.menu_music_path.is_some() && !self.mixer.music_get_busy() { self.play_menu_music(); }
        let bx = self.WIDTH / 2 - 180;
        for (i, b) in self.buttons.iter_mut().enumerate() { b.rect.x = bx; b.rect.y = 155 + i as i32 * 56; }
        let has_mods = true;
        let has_online = self.WIDTH >= 500;
        let nb = self.buttons.len();
        let nav_total = nb + has_mods as usize + 2 + has_online as usize;
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let title = self.font.render("JumpNDodge", true, self.get_text_color());
        let title_sh = self.font.render("JumpNDodge", true, (100, 100, 120));
        bs.blit(&title_sh, (w / 2 - title.get_width() / 2 + 3, 73));
        bs.blit(&title, (w / 2 - title.get_width() / 2, 70));
        // Wie bei Minecraft "JAVA EDITION": Edition in Großbuchstaben mit Abstand unter dem Logo
        thread_local! { static ED_FONT: Font = Font::sys("arial", 17, true); }
        let ef = ED_FONT.with(|f| f.clone());
        let text = "RUST EDITION";
        let spacing = 5;
        let glyphs: Vec<Surface> = text.chars().map(|c| ef.render(&c.to_string(), true, (226, 140, 96))).collect();
        let shadows: Vec<Surface> = text.chars().map(|c| ef.render(&c.to_string(), true, if self.is_dark_mode() { (60, 34, 24) } else { (190, 150, 130) })).collect();
        let total: i32 = glyphs.iter().map(|g| g.get_width()).sum::<i32>() + spacing * (glyphs.len() as i32 - 1);
        let mut x = w / 2 - total / 2;
        let y = 117;
        for (g, sh) in glyphs.iter().zip(shadows.iter()) {
            bs.blit(sh, (x + 2, y + 2));
            bs.blit(g, (x, y));
            x += g.get_width() + spacing;
        }
        let coin_bg = Rect::new(15, 15, 200, 35);
        draw::rect(&bs, self.get_panel_color(), coin_bg, 0, 18);
        draw::rect(&bs, (200, 200, 200), coin_bg, 2, 18);
        let ct = self.small_font.render(&format!("{}: {}", self.get_text("coins"), self.stats.gi("coins", 0)), true, self.get_text_color());
        bs.blit(&ct, (25, 22));

        let pr = self._get_profile_hub_btn_rect();
        let pidx = nb + has_mods as usize + 1;
        let phov = pr.collidepoint(mouse_pos) || self.nav_is_focused("menu", pidx, nav_total);
        self.draw_ui_button(&bs, pr, "", BtnOpts { hovered: phov, radius: Some(10), ..Default::default() });
        let ar = Rect::new(pr.x + 3, pr.y + 3, 50, 50);
        self.draw_avatar(&bs, ar, None, true, None);
        let name = self.current_profile_name.clone().unwrap_or_else(|| "-".into());
        let ns = self.small_font.render(&name, true, self.get_text_color());
        let ns = fit_surf(&ns, pr.w - ar.w - 20);
        bs.blit(&ns, (ar.right() + 10, pr.centery() - ns.get_height() / 2));
        self._refresh_notifications_if_due();

        if has_online {
            let or = Rect::new(w - 235, h - 118, 220, 56);
            let oidx = nb + has_mods as usize + 2;
            let ohov = or.collidepoint(mouse_pos) || self.nav_is_focused("menu", oidx, nav_total);
            self.draw_ui_button(&bs, or, &self.get_text("online_hub_title"), BtnOpts { font: Some(self.menu_font.clone()), hovered: ohov, radius: Some(10), ..Default::default() });
            let total = self._notif_pending_friend_requests + self._notif_pending_clan_requests + self._notif_unread_conversations;
            if total > 0 { self._draw_notification_badge(&bs, or.right(), or.top(), total); }
        }

        let buttons = self.buttons.clone();
        for (i, b) in buttons.iter().enumerate() {
            let hov = b.rect.collidepoint(mouse_pos) || self.nav_is_focused("menu", i, nav_total);
            self.draw_ui_button(&bs, b.rect, &b.text, BtnOpts { font: Some(self.menu_font.clone()), hovered: hov, ..Default::default() });
        }

        if has_mods {
            // Rust Edition: Lua-Mods (wie in Python unten links)
            let mc = self.mods.active().count();
            let label = if mc > 0 { format!("Mods ({})", mc) } else { "Mods".to_string() };
            let mr = Rect::new(20, h - 56, 130, 36);
            let mhov = mr.collidepoint(mouse_pos) || self.nav_is_focused("menu", nb, nav_total);
            self.draw_ui_button(&bs, mr, &label, BtnOpts { font: Some(self.small_font.clone()), hovered: mhov, radius: Some(8), ..Default::default() });
        }

        let dr = Rect::new(w - 200, 15, 180, 40);
        let didx = nb + has_mods as usize;
        let hov = dr.collidepoint(mouse_pos) || self.nav_is_focused("menu", didx, nav_total);
        self.draw_ui_button(&bs, dr, &self.get_text("daily_rewards"), BtnOpts {
            font: Some(self.small_font.clone()), hovered: hov, base_color: Some((255, 200, 0)), hover_color: Some((255, 215, 0)),
            text_color: Some((0, 0, 0)), border_color: Some((180, 120, 0)), hover_border_color: Some((200, 150, 0)), radius: Some(8), ..Default::default()
        });
        self.flip();
    }

    pub fn get_diff_short(&self, diff: &str) -> String {
        let t = d("_diff_short_translations");
        let de = &t["Deutsch"];
        t.gv(&self.lang()).unwrap_or(de).gos(diff).or_else(|| de.gos(diff)).unwrap_or_else(|| diff.to_string())
    }
    fn hover_pair(&self, table: &str, key: &str) -> Vec<String> {
        let t = d(table);
        let de = &t["Deutsch"];
        let v = t.gv(&self.lang()).unwrap_or(de).gv(key).or_else(|| de.gv(key));
        match v { Some(Value::Array(a)) => a.iter().map(py_str).collect(), _ => vec![String::new(), String::new()] }
    }
    pub fn get_diff_hover(&self, diff: &str) -> Vec<String> { self.hover_pair("_diff_hover_translations", diff) }
    pub fn get_modifier_hover(&self, key: &str) -> Vec<String> { self.hover_pair("_modifier_hover_translations", key) }

    pub fn draw_difficulty_select(&mut self) {
        let mouse_pos = self.get_mouse_pos_logical();
        let dark = self.is_dark_mode();
        let mode = self.current_game_mode.clone();
        let sb_count = 4 + if mode == "2player" { 2 } else { 0 };
        let main_count = if mode == "custom" { 8 } else { DIFFICULTIES.len() };
        let total = sb_count + main_count + 2;
        let start_idx = sb_count + main_count;
        let back_idx = start_idx + 1;
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let dim = Surface::new_alpha(w, h);
        dim.fill((0, 0, 0, 150));
        bs.blit(&dim, (0, 0));
        let ts = self.menu_font.render(&self.get_text("choose_difficulty_title"), true, self.get_text_color());
        bs.blit(&ts, (w / 2 - ts.get_width() / 2, 30));

        let (sbx, sbw) = (24, 180);
        let main_x = sbx + sbw + 24;
        let main_w = 300.max(w - main_x - 24);
        let main_y0 = 118;
        let klassik_accent = (180, 140, 0);
        let tp_accent = (140, 60, 200);
        let custom_accent = (0, 160, 170);
        let mf = self.menu_font.clone();
        let sf = self.small_font.clone();

        let cr = Rect::new(sbx, main_y0, sbw, 56);
        let c_active = mode == "custom";
        let mut c_bg = if c_active { custom_accent } else if dark { (35, 35, 50) } else { (200, 205, 220) };
        let c_brd = if c_active { (100, 230, 230) } else if dark { (80, 80, 100) } else { (150, 155, 175) };
        let c_tc = if c_active { (255, 255, 255) } else if dark { (160, 160, 180) } else { (80, 85, 110) };
        let c_f = self.nav_is_focused("difficulty_select", 0, total);
        if (cr.collidepoint(mouse_pos) || c_f) && !c_active { c_bg = if dark { (0, 70, 75) } else { (100, 190, 195) }; }
        draw::rect(&bs, c_bg, cr, 0, 12);
        draw::rect(&bs, if c_f { (255, 255, 255) } else { c_brd }, cr, 3, 12);
        let cl = mf.render("Custom", true, c_tc);
        bs.blit(&cl, (cr.centerx() - cl.get_width() / 2, cr.top() + 8));
        let cs = sf.render("Slider & Modifier", true, c_tc);
        bs.blit(&cs, (cr.centerx() - cs.get_width() / 2, cr.bottom() - 22));

        let ply = cr.bottom() + 20;
        bs.blit(&sf.render("PRESETS", true, if dark { (140, 140, 160) } else { (120, 120, 140) }), (sbx, ply));
        let pby = ply + 22;
        let std_r = Rect::new(sbx, pby, sbw, 42);
        let kl_r = Rect::new(sbx, std_r.bottom() + 8, sbw, 42);
        let tp_r = Rect::new(sbx, kl_r.bottom() + 8, sbw, 42);
        let presets: [(Rect, &str, bool, Option<(i32, i32, i32)>); 3] = [
            (std_r, "Standard", mode == "standard", None),
            (kl_r, "Klassik", mode == "klassik", Some(klassik_accent)),
            (tp_r, "2 Player", mode == "2player", Some(tp_accent)),
        ];
        for (pi, (prect, label, active, accent)) in presets.iter().enumerate() {
            let pf = self.nav_is_focused("difficulty_select", 1 + pi, total);
            let (mut pbg, pbrd, ptc);
            if *active {
                pbg = accent.unwrap_or(if dark { (50, 80, 160) } else { (60, 100, 200) });
                pbrd = if *accent == Some(klassik_accent) { (255, 220, 80) } else if accent.is_some() { (200, 140, 255) } else if dark { (120, 160, 255) } else { (40, 70, 180) };
                ptc = (255, 255, 255);
            } else {
                pbg = if dark { (35, 35, 50) } else { (200, 205, 220) };
                pbrd = if dark { (80, 80, 100) } else { (150, 155, 175) };
                ptc = if dark { (160, 160, 180) } else { (80, 85, 110) };
            }
            if (prect.collidepoint(mouse_pos) || pf) && !active {
                pbg = if *accent == Some(klassik_accent) { if dark { (100, 75, 0) } else { (200, 165, 40) } }
                    else if *accent == Some(tp_accent) { if dark { (60, 35, 80) } else { (160, 120, 210) } }
                    else if dark { (45, 55, 80) } else { (180, 185, 210) };
            }
            draw::rect(&bs, pbg, *prect, 0, 10);
            draw::rect(&bs, if pf { (255, 255, 255) } else { pbrd }, *prect, 2, 10);
            let ls = sf.render(label, true, ptc);
            bs.blit(&ls, (prect.centerx() - ls.get_width() / 2, prect.centery() - ls.get_height() / 2));
        }
        if mode == "2player" {
            let sub_w = (sbw - 8) / 2;
            let sub_y = tp_r.bottom() + 10;
            let co = Rect::new(sbx, sub_y, sub_w, 32);
            let vs = Rect::new(sbx + sub_w + 8, sub_y, sub_w, 32);
            for (si, (sr, sl, sa)) in [(co, "Co-op", self.two_player_submode == "coop"), (vs, "VS", self.two_player_submode == "vs")].iter().enumerate() {
                let sfc = self.nav_is_focused("difficulty_select", 4 + si, total);
                let (mut sbg, sbrd, stc) = if *sa { ((if dark { (100, 40, 160) } else { (120, 50, 190) }), (200, 140, 255), (255, 255, 255)) }
                    else { ((if dark { (35, 35, 50) } else { (210, 210, 225) }), (if dark { (80, 80, 100) } else { (150, 150, 175) }), (if dark { (150, 150, 170) } else { (70, 70, 100) })) };
                if (sr.collidepoint(mouse_pos) || sfc) && !sa { sbg = if dark { (55, 30, 80) } else { (185, 185, 210) }; }
                draw::rect(&bs, sbg, *sr, 0, 8);
                draw::rect(&bs, if sfc { (255, 255, 255) } else { sbrd }, *sr, 2, 8);
                let ss = sf.render(sl, true, stc);
                bs.blit(&ss, (sr.centerx() - ss.get_width() / 2, sr.centery() - ss.get_height() / 2));
            }
        }

        let (card_w, card_x, cards_y0, card_h, gap) = (main_w, main_x, main_y0, 50, 56);
        if mode == "custom" {
            let speed = self.settings.gf("custom_speed", 10.0);
            let mind = self.settings.gf("custom_min_delay", 900.0);
            let maxd = self.settings.gf("custom_max_delay", 1600.0);
            let labels = ["Geschwindigkeit:", "Min-Pause:", "Max-Pause:"];
            let label_w = labels.iter().map(|l| mf.size(l).0).max().unwrap();
            let slider_x = card_x + label_w + 24;
            let slider_w = card_w - (label_w + 24) - 80;
            let rows = [
                ("Geschwindigkeit:", speed, 4.0, 35.0, cards_y0 + 30, format!("{}", speed as i64)),
                ("Min-Pause:", mind, 100.0, 2000.0, cards_y0 + 100, format!("{:.2}s", mind / 1000.0)),
                ("Max-Pause:", maxd, 200.0, 2500.0, cards_y0 + 170, format!("{:.2}s", maxd / 1000.0)),
            ];
            for (si, (label, value, lo, hi, y, vt)) in rows.iter().enumerate() {
                let slf = self.nav_is_focused("difficulty_select", sb_count + si, total);
                let edit = self._ds_slider_edit == Some(si as i32);
                let row = Rect::new(card_x, y - 20, card_w, 40);
                if slf { draw::rect(&bs, (255, 255, 255), row, 2, 8); }
                let track = Rect::new(slider_x, y - 4, slider_w, 8);
                draw::rect(&bs, self.get_panel_color(), track, 0, 4);
                draw::rect(&bs, if edit { (255, 210, 60) } else { (150, 180, 220) }, track, 2, 4);
                let frac = ((value - lo) / (hi - lo)).clamp(0.0, 1.0);
                let hx = (slider_x as f64 + frac * slider_w as f64) as i32;
                let hr = if edit { 13 } else { 10 };
                draw::circle(&bs, if edit { (255, 200, 40) } else { (0, 160, 170) }, (hx, *y), hr, 0);
                draw::circle(&bs, if edit { (255, 230, 150) } else { (150, 230, 230) }, (hx, *y), hr, 2);
                let ls = mf.render(label, true, self.get_text_color());
                bs.blit(&ls, (card_x, y - ls.get_height() / 2));
                let vs = mf.render(vt, true, self.get_text_color());
                bs.blit(&vs, (card_x + card_w - vs.get_width(), y - vs.get_height() / 2));
            }
            let mod_y = cards_y0 + 210;
            bs.blit(&mf.render("Modifier:", true, self.get_text_color()), (card_x, mod_y));
            let toggles = custom_toggle_defs();
            let (tw, th, tgap, row_gap, cols) = (220, 34, 12, 10, 2usize);
            let toggle_y = mod_y + 34;
            let trows = ((toggles.len() + cols - 1) / cols) as i32;
            let mut hovered_mod: Option<(&str, Option<Rect>)> = None;
            for (ti, (skey, def, tl)) in toggles.iter().enumerate() {
                let active = self.settings.gb(skey, *def);
                let (col, row) = ((ti % cols) as i32, (ti / cols) as i32);
                let tr = Rect::new(card_x + col * (tw + tgap), toggle_y + row * (th + row_gap), tw, th);
                let tf = self.nav_is_focused("difficulty_select", sb_count + 3 + ti, total);
                let mh = tr.collidepoint(mouse_pos);
                let hov = mh || tf;
                if hov { hovered_mod = Some((skey, if mh { None } else { Some(tr) })); }
                let bg = if active { (0, 140, 150) } else { self.get_button_color() };
                let bc = if tf { (255, 255, 255) } else if hov && !active { (120, 240, 240) } else if active { (100, 220, 220) } else { (160, 160, 160) };
                draw::rect(&bs, bg, tr, 0, 8);
                draw::rect(&bs, bc, tr, if hov { 3 } else { 2 }, 8);
                let tsf = sf.render(tl, true, if active { (255, 255, 255) } else { self.get_text_color() });
                bs.blit(&tsf, (tr.centerx() - tsf.get_width() / 2, tr.centery() - tsf.get_height() / 2));
            }
            let hint_y = toggle_y + trows * (th + row_gap) + 4;
            bs.blit(&sf.render("Keine Coins im Custom-Modus. Freie Bewegung: WASD.", true, if dark { (150, 150, 150) } else { (110, 110, 110) }), (card_x, hint_y));
            if let Some((k, anchor)) = hovered_mod {
                if d("_modifier_hover_translations")["Deutsch"].gv(k).is_some() {
                    let content = self.get_modifier_hover(k);
                    self.draw_hover_tooltip(&content, mouse_pos, 600, None, anchor);
                }
            }
        } else {
            let mut hovered: Option<(String, Option<Rect>)> = None;
            for (i, diff) in DIFFICULTIES.iter().enumerate() {
                let cy = cards_y0 + i as i32 * gap;
                let crr = Rect::new(card_x, cy, card_w, card_h);
                let mh = crr.collidepoint(mouse_pos);
                let hov = mh || self.nav_is_focused("difficulty_select", sb_count + i, total);
                let sel = i == self.difficulty_index;
                let col = diff_color(diff);
                let bright = col.0 + col.1 + col.2 > 60;
                let border = if bright { col } else if dark { (200, 200, 200) } else { (80, 80, 80) };
                if hov { hovered = Some((diff.to_string(), if mh { None } else { Some(crr) })); }
                if sel {
                    if dark {
                        draw::rect(&bs, (col.0 / 5, col.1 / 5, col.2 / 5), crr, 0, 12);
                    } else {
                        draw::rect(&bs, (220, 240, 255), crr, 0, 12);
                    }
                    draw::rect(&bs, border, crr, 3, 12);
                } else if hov {
                    draw::rect(&bs, self.get_button_hover_color(), crr, 0, 12);
                    draw::rect(&bs, border, crr, 2, 12);
                } else {
                    draw::rect(&bs, self.get_button_color(), crr, 0, 12);
                    draw::rect(&bs, if dark { (130, 130, 130) } else { (180, 180, 180) }, crr, 1, 12);
                }
                draw::rect(&bs, col, Rect::new(card_x, cy, 7, card_h), 0, 4);
                let sel_tc = if !bright { (210, 210, 210) } else { col };
                let nc = if sel { sel_tc } else { self.get_text_color() };
                let ns = mf.render(&self.get_difficulty_name(diff), true, nc);
                bs.blit(&ns, (card_x + 18, cy + card_h / 2 - ns.get_height() / 2));
                let dc = if hov || sel { self.get_text_color() } else if dark { (150, 150, 150) } else { (110, 110, 110) };
                let ds = sf.render(&self.get_diff_short(diff), true, dc);
                bs.blit(&ds, (card_x + 175, cy + card_h / 2 - ds.get_height() / 2));
                let badge = if sel {
                    sf.render(&self.get_text("selected_badge"), true, if dark { sel_tc } else { (0, 110, 190) })
                } else if self.klassik_mode {
                    sf.render(&self.get_text("no_coins_badge"), true, (120, 120, 120))
                } else if let Some(rule) = coin_rule(diff) {
                    sf.render(&py_format(&self.get_text("pts_per_coin"), &[&fmt_num(rule)]), true, if dark { (200, 155, 0) } else { (160, 115, 0) })
                } else {
                    sf.render(&self.get_text("no_coins_badge"), true, (120, 120, 120))
                };
                bs.blit(&badge, (crr.right() - badge.get_width() - 14, cy + card_h / 2 - badge.get_height() / 2));
            }
            if let Some((hd, anchor)) = hovered {
                let content = self.get_diff_hover(&hd);
                self.draw_hover_tooltip(&content, mouse_pos, 600, Some(diff_color(&hd)), anchor);
            }
        }

        let (btn_y, btn_h) = (h - 54, 44);
        let start_r = Rect::new(w / 2 + 8, btn_y, 200, btn_h);
        let back_r = Rect::new(w / 2 - 208, btn_y, 200, btn_h);
        let sfoc = self.nav_is_focused("difficulty_select", start_idx, total);
        let shov = start_r.collidepoint(mouse_pos) || sfoc;
        draw::rect(&bs, if shov { (0, 140, 60) } else { (0, 110, 50) }, start_r, 0, 10);
        draw::rect(&bs, if sfoc { (255, 255, 255) } else { ui_glow_pulse((0, 200, 80), 3.0, 0.65, None) }, start_r, 2, 10);
        let sl = mf.render(&self.get_text("start"), true, (255, 255, 255));
        bs.blit(&sl, (start_r.centerx() - sl.get_width() / 2, start_r.centery() - sl.get_height() / 2));
        let bfoc = self.nav_is_focused("difficulty_select", back_idx, total);
        let bhov = back_r.collidepoint(mouse_pos) || bfoc;
        draw::rect(&bs, if bhov { self.get_button_hover_color() } else { self.get_button_color() }, back_r, 0, 10);
        draw::rect(&bs, if bfoc { (255, 255, 255) } else { (160, 160, 160) }, back_r, 2, 10);
        let bl = mf.render(&self.get_text("back"), true, self.get_text_color());
        bs.blit(&bl, (back_r.centerx() - bl.get_width() / 2, back_r.centery() - bl.get_height() / 2));
        self.flip();
    }
}

pub fn custom_toggle_defs() -> [(&'static str, bool, &'static str); 5] {
    [
        ("custom_oneups", true, "1-Ups"),
        ("custom_vertical_obstacles", false, "Vert. Hindernisse"),
        ("custom_wind_sides", false, "Wind (Seiten)"),
        ("custom_wind_all", false, "Wind (Allseitig)"),
        ("custom_free_roam", false, "Freie Bewegung (Beta)"),
    ]
}

pub fn diff_color(diff: &str) -> (i32, i32, i32) {
    let a = d("_diff_colors").garr(diff);
    if a.len() >= 3 { (as_i(&a[0]).unwrap() as i32, as_i(&a[1]).unwrap() as i32, as_i(&a[2]).unwrap() as i32) } else { (200, 200, 200) }
}

/// Zahl wie Python str(): 10 -> "10", 1.5 -> "1.5"
pub fn fmt_num(f: f64) -> String { if f == f.trunc() { format!("{}", f as i64) } else { format!("{}", f) } }

/// _fit_surf(surf, max_w)
pub fn fit_surf(s: &Surface, max_w: i32) -> Surface {
    if s.get_width() <= max_w { return s.clone(); }
    let scale = max_w as f64 / s.get_width() as f64;
    transform::smoothscale(s, (max_w, 1.max((s.get_height() as f64 * scale) as i32)))
}
