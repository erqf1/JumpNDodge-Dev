//! Mods-Menü der Rust Edition (Lua-Mods): an/aus, Fehler und Log ansehen, eigene Modi spielen.

use super::core::*;
use super::state::{Snd, G};
use crate::pg::*;
use crate::util::*;
use serde_json::{json, Value};

impl G {
    pub fn mods_dir(&self) -> std::path::PathBuf { self.BASE_DIR.join("mods") }
    /// Liegt mindestens ein Mod im Mods-Ordner? (Knopf im Hauptmenü, Modded-Statistik)
    pub fn mods_installed(&self) -> bool { !self.mods.list.is_empty() }
    /// Ist mindestens ein Mod aktiv? Dann ist Online gesperrt.
    pub fn modded(&self) -> bool { self.mods.any_active() }

    pub fn reload_mods(&mut self) {
        let enabled = self.settings.gobj("lua_mods_enabled");
        let dir = self.mods_dir();
        self.mods = super::mods::Mods::load(&dir, &enabled);
        println!("[MOD] {} Lua-Mod(s) geladen, {} aktiv", self.mods.list.len(), self.mods.active().count());
    }

    fn save_mod_enabled(&mut self) {
        let mut m = serde_json::Map::new();
        for x in &self.mods.list { m.insert(x.file.clone(), json!(x.enabled)); }
        self.settings.insert("lua_mods_enabled".into(), Value::Object(m));
        self.save_section("settings", None);
    }

    pub fn mods_menu_loop(&mut self) {
        let mut clock = Clock::new();
        let mut selected: usize = 0;
        while self.in_mods_menu {
            let fps = self.target_fps();
            clock.tick(fps);
            self.sync_window_size();
            let mut buttons: Vec<(Rect, String)> = vec![];
            self.fill_bg(true);
            let bs = self.base_surface.clone();
            let (w, h) = (self.WIDTH, self.HEIGHT);
            let mp = self.get_mouse_pos_logical();
            let tc = self.get_text_color();
            let sf = self.small_font.clone();
            let bf = self.badge_font.clone();
            self.draw_title_shadow(&bs, "Mods", 14);
            let sub = self.tde("Lua-Mods aus dem Ordner „mods“ · laufen sicher in einer Sandbox · Runden mit Mods zählen getrennt",
                               "Lua mods from the \"mods\" folder · sandboxed · rounds with mods are counted separately");
            let ss = bf.render(&sub, true, (150, 150, 180));
            bs.blit(&ss, (w / 2 - ss.get_width() / 2, 78));
            // Liste links
            let list = Rect::new(w / 2 - 385, 104, 400, h - 180);
            draw::rect(&bs, self.get_panel_color(), list, 0, 12);
            draw::rect(&bs, (110, 110, 150), list, 2, 12);
            if self.mods.list.is_empty() {
                let t = self.tde("Noch keine Mods. Leg eine Beispiel-Mod an!", "No mods yet. Create an example mod!");
                let s = sf.render(&t, true, (140, 140, 160));
                bs.blit(&s, (list.centerx() - s.get_width() / 2, list.y + 30));
            }
            selected = selected.min(self.mods.list.len().saturating_sub(1));
            let mut y = list.y + 10;
            for (i, m) in self.mods.list.iter().enumerate() {
                if y + 46 > list.bottom() - 4 { break; }
                let row = Rect::new(list.x + 10, y, list.w - 20, 44);
                let sel = i == selected;
                draw::rect(&bs, if sel { self.get_button_hover_color() } else { self.get_button_color() }, row, 0, 8);
                if sel { draw::rect(&bs, (120, 160, 240), row, 2, 8); }
                let (st, sc) = if m.error.is_some() { (self.tde("Fehler", "Error"), (220, 90, 100)) }
                    else if m.enabled { (self.tde("an", "on"), (90, 190, 140)) } else { (self.tde("aus", "off"), (150, 150, 160)) };
                draw::circle(&bs, sc, (row.x + 16, row.centery()), 6, 0);
                let name = super::online_screens3::_fit_text_tail(&m.name, &sf, row.w - 110);
                bs.blit(&sf.render(&name, true, tc), (row.x + 30, row.y + 4));
                let meta = format!("{}{}", m.file, if m.version.is_empty() { String::new() } else { format!(" · v{}", m.version) });
                bs.blit(&bf.render(&meta, true, (140, 140, 160)), (row.x + 30, row.y + 25));
                let sts = sf.render(&st, true, sc);
                bs.blit(&sts, (row.right() - 10 - sts.get_width(), row.y + 12));
                buttons.push((row, format!("sel:{}", i)));
                y += 50;
            }
            // Details rechts
            let det = Rect::new(w / 2 + 25, 104, 360, h - 180);
            draw::rect(&bs, self.get_panel_color(), det, 0, 12);
            draw::rect(&bs, (110, 110, 150), det, 2, 12);
            let mut dy = det.y + 12;
            let mut line = |text: &str, col: (i32, i32, i32), f: &Font, dy: &mut i32| {
                for l in super::screens_a::wrap_words(f, text, det.w - 28) {
                    if *dy + 20 > det.bottom() - 96 { break; }
                    bs.blit(&f.render(&l, true, col), (det.x + 14, *dy));
                    *dy += f.get_height() + 2;
                }
            };
            if let Some(m) = self.mods.list.get(selected) {
                line(&m.name, tc, &sf, &mut dy);
                if !m.author.is_empty() { line(&format!("{} {}", self.tde("von", "by"), m.author), (150, 150, 180), &bf, &mut dy); }
                dy += 4;
                if !m.description.is_empty() { line(&m.description, (180, 180, 200), &bf, &mut dy); dy += 4; }
                if let Some(e) = &m.error { line(&format!("{}: {}", self.tde("Fehler", "Error"), e), (230, 110, 120), &bf, &mut dy); }
                let parts = format!("{} {} · {} {}{}",
                    m.obstacles.len(), self.tde("Hindernis(se)", "obstacle(s)"), m.modes.len(), self.tde("Modus/Modi", "mode(s)"),
                    if m.modifier.is_some() { self.tde(" · ändert Spielwerte", " · changes game values") } else { String::new() });
                line(&parts, (150, 150, 180), &bf, &mut dy);
                for l in m.log.iter().rev().take(4) { line(&format!("> {}", l), (130, 140, 160), &bf, &mut dy); }
                // Knöpfe der Mod
                let tb = Rect::new(det.x + 14, det.bottom() - 90, det.w - 28, 38);
                let tt = if m.enabled { self.tde("Ausschalten", "Disable") } else { self.tde("Einschalten", "Enable") };
                self.draw_ui_button(&bs, tb, &tt, BtnOpts { font: Some(sf.clone()), hovered: tb.collidepoint(mp), border_color: Some(if m.enabled { (180, 150, 90) } else { (90, 190, 140) }), ..Default::default() });
                buttons.push((tb, format!("toggle:{}", selected)));
                let modes: Vec<(usize, String)> = m.modes.iter().enumerate().map(|(i, x)| (i, x.name.clone())).collect();
                if m.enabled && m.error.is_none() {
                    for (k, (mi, mn)) in modes.iter().enumerate().take(1) {
                        let pb = Rect::new(det.x + 14, det.bottom() - 46, det.w - 28, 36);
                        let _ = k;
                        self.draw_ui_button(&bs, pb, &format!("{}: {}", self.tde("Spielen", "Play"), mn), BtnOpts { font: Some(sf.clone()), hovered: pb.collidepoint(mp), border_color: Some((120, 160, 240)), ..Default::default() });
                        buttons.push((pb, format!("play:{}:{}", selected, mi)));
                    }
                }
            }
            // unten
            let labels = [(self.tde("Beispiel-Mod anlegen", "Create example mod"), "example"), (self.tde("Ordner öffnen", "Open folder"), "open"),
                          (self.tde("Neu laden", "Reload"), "reload"), (self.get_text("back"), "back")];
            let bw = 180;
            let total = labels.len() as i32 * bw + (labels.len() as i32 - 1) * 10;
            for (i, (t, a)) in labels.iter().enumerate() {
                let r = Rect::new(w / 2 - total / 2 + i as i32 * (bw + 10), h - 62, bw, 44);
                self.draw_ui_button(&bs, r, t, BtnOpts { font: Some(bf.clone()), hovered: r.collidepoint(mp), ..Default::default() });
                buttons.push((r, a.to_string()));
            }
            self.cleanup_overlays();
            let mut oy = 8;
            for o in self.overlays.clone() {
                let t = sf.render(&o.text, true, o.color);
                bs.blit(&t, ((w - t.get_width()) / 2, oy));
                oy += t.get_height() + 4;
            }
            self.flip();
            let mut act: Option<String> = None;
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self.is_back_key(key) || key == K_m { act = Some("back".into()); }
                        else if key == K_UP || key == K_w { selected = selected.saturating_sub(1); }
                        else if key == K_DOWN || key == K_s { selected = (selected + 1).min(self.mods.list.len().saturating_sub(1)); }
                        else if self.is_nav_confirm(key) { act = Some(format!("toggle:{}", selected)); }
                        else if key == K_F5 { act = Some("reload".into()); }
                    }
                    Event::MouseButtonDown { button: 1, .. } => {
                        let p = self.get_mouse_pos_logical();
                        act = buttons.iter().rev().find(|(r, _)| r.collidepoint(p)).map(|(_, a)| a.clone());
                    }
                    _ => {}
                }
            }
            let Some(a) = act else { continue; };
            let (cmd, arg) = a.split_once(':').unwrap_or((a.as_str(), ""));
            match cmd {
                "back" => { self.play_sound(Snd::ClickBack); self.in_mods_menu = false; self.in_menu = true; }
                "sel" => { selected = arg.parse().unwrap_or(0); self.play_sound(Snd::Click); }
                "toggle" => {
                    if let Some(m) = self.mods.list.get_mut(arg.parse::<usize>().unwrap_or(usize::MAX)) {
                        m.enabled = !m.enabled;
                        self.play_sound(Snd::Click);
                        self.save_mod_enabled();
                    }
                }
                "play" => {
                    let mut it = arg.split(':');
                    let (mi, xi) = (it.next().and_then(|v| v.parse::<usize>().ok()), it.next().and_then(|v| v.parse::<usize>().ok()));
                    if let (Some(mi), Some(xi)) = (mi, xi) {
                        if let Some(mode) = self.mods.list.get(mi).and_then(|m| m.modes.get(xi)).cloned() {
                            self.play_sound(Snd::MenuClick);
                            self._mod_mode = Some(mode);
                            self.current_game_mode = "mod".into();
                            self.in_mods_menu = false;
                            self.game_running = true;
                        }
                    }
                }
                "reload" => { self.play_sound(Snd::Click); self.reload_mods(); let t = format!("{} Mod(s)", self.mods.list.len()); self.push_overlay(&t, (120, 170, 230), 1.5, None); }
                "open" => {
                    let d = self.mods_dir();
                    let _ = std::fs::create_dir_all(&d);
                    crate::updater::open_url(&d.to_string_lossy());
                }
                "example" => {
                    let d = self.mods_dir();
                    let _ = std::fs::create_dir_all(&d);
                    let p = d.join("beispiel.lua");
                    if !p.exists() { let _ = std::fs::write(&p, super::mods::EXAMPLE_MOD); }
                    self.reload_mods();
                    let t = format!("{}: mods/beispiel.lua", self.tde("Angelegt", "Created"));
                    self.push_overlay(&t, (90, 190, 140), 2.0, None);
                    self.play_sound(Snd::Click);
                }
                _ => {}
            }
        }
    }
}
