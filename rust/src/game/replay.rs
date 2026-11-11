//! Replays und Geister.
//! Jede Runde wird pro Logikschritt (60/s) aufgezeichnet: Spieler, Hindernisse, 1-UPs, Punkte, Leben.
//! Das ist unabhängig von Zufall, Fenstergröße und Bildrate - ein Replay sieht immer genau so aus wie
//! die Runde. `data/replays/last.jnr` = letzte Runde, `best-<Schwierigkeit>.jnr` = Rekordlauf
//! (daraus kommt auch der Geist, gegen den man läuft).

use super::core::*;
use super::state::{Snd, G};
use crate::pg::*;
use std::path::PathBuf;

const MAGIC: &[u8; 4] = b"JNR1";

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub px: i16,
    pub py: i16,
    pub visible: bool,
    pub score: i32,
    pub lives: u8,
    /// (Art 0 = Boden, 1 = von oben, 2 = 1-UP; x, y, w, h)
    pub objs: Vec<(u8, i16, i16, i16, i16)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    pub w: i32,
    pub h: i32,
    pub color: (i32, i32, i32),
    pub difficulty: String,
    pub score: i64,
    pub frames: Vec<Frame>,
}

fn c16(v: i32) -> i16 { v.clamp(i16::MIN as i32, i16::MAX as i32) as i16 }

impl Recording {
    pub fn new(w: i32, h: i32, color: (i32, i32, i32), difficulty: &str) -> Recording {
        Recording { w, h, color, difficulty: difficulty.into(), score: 0, frames: Vec::new() }
    }
    /// Ein Logikschritt. Begrenzung: 1 Stunde.
    pub fn push(&mut self, player: Rect, visible: bool, score: i64, lives: i64, objs: impl Iterator<Item = (u8, Rect)>) {
        if self.frames.len() >= 60 * 3600 { return; }
        self.frames.push(Frame {
            px: c16(player.x), py: c16(player.y), visible, score: score.clamp(0, i32::MAX as i64) as i32, lives: lives.clamp(0, 255) as u8,
            objs: objs.take(255).map(|(k, r)| (k, c16(r.x), c16(r.y), c16(r.w), c16(r.h))).collect(),
        });
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(32 + self.frames.len() * 24);
        b.extend_from_slice(MAGIC);
        b.extend_from_slice(&(self.w as i16).to_le_bytes());
        b.extend_from_slice(&(self.h as i16).to_le_bytes());
        b.extend_from_slice(&[self.color.0 as u8, self.color.1 as u8, self.color.2 as u8]);
        let d = self.difficulty.as_bytes();
        b.push(d.len().min(255) as u8);
        b.extend_from_slice(&d[..d.len().min(255)]);
        b.extend_from_slice(&self.score.to_le_bytes());
        b.extend_from_slice(&(self.frames.len() as u32).to_le_bytes());
        for f in &self.frames {
            b.extend_from_slice(&f.px.to_le_bytes());
            b.extend_from_slice(&f.py.to_le_bytes());
            b.push(f.visible as u8);
            b.extend_from_slice(&f.score.to_le_bytes());
            b.push(f.lives);
            b.push(f.objs.len() as u8);
            for (k, x, y, w, h) in &f.objs {
                b.push(*k);
                for v in [x, y, w, h] { b.extend_from_slice(&v.to_le_bytes()); }
            }
        }
        b
    }
    pub fn decode(b: &[u8]) -> Option<Recording> {
        let mut p = 0usize;
        let mut take = |n: usize| -> Option<&[u8]> { let s = b.get(p..p + n)?; p += n; Some(s) };
        if take(4)? != MAGIC { return None; }
        let i16v = |s: &[u8]| i16::from_le_bytes([s[0], s[1]]);
        let w = i16v(take(2)?) as i32;
        let h = i16v(take(2)?) as i32;
        let c = take(3)?;
        let color = (c[0] as i32, c[1] as i32, c[2] as i32);
        let dl = take(1)?[0] as usize;
        let difficulty = String::from_utf8_lossy(take(dl)?).to_string();
        let score = i64::from_le_bytes(take(8)?.try_into().ok()?);
        let n = u32::from_le_bytes(take(4)?.try_into().ok()?) as usize;
        if n > 60 * 3600 { return None; }
        let mut frames = Vec::with_capacity(n);
        for _ in 0..n {
            let px = i16v(take(2)?);
            let py = i16v(take(2)?);
            let visible = take(1)?[0] != 0;
            let score = i32::from_le_bytes(take(4)?.try_into().ok()?);
            let lives = take(1)?[0];
            let no = take(1)?[0] as usize;
            let mut objs = Vec::with_capacity(no);
            for _ in 0..no {
                let k = take(1)?[0];
                let x = i16v(take(2)?); let y = i16v(take(2)?); let ow = i16v(take(2)?); let oh = i16v(take(2)?);
                objs.push((k, x, y, ow, oh));
            }
            frames.push(Frame { px, py, visible, score, lives, objs });
        }
        Some(Recording { w, h, color, difficulty, score, frames })
    }
    pub fn seconds(&self) -> f64 { self.frames.len() as f64 / LOGIC_HZ }
}

/// Dateiname für den Rekordlauf einer Schwierigkeit (ohne Sonderzeichen)
fn best_name(difficulty: &str) -> String {
    let safe: String = difficulty.chars().map(|c| match c { 'ö' => 'o', 'ä' => 'a', 'ü' => 'u', c if c.is_ascii_alphanumeric() => c, _ => '_' }).collect();
    format!("best-{}.jnr", safe)
}

impl G {
    pub fn replay_dir(&self) -> PathBuf { self._DATA_DIR.join("replays") }

    pub fn save_recording(&mut self, rec: &Recording, best: bool) {
        let dir = self.replay_dir();
        let _ = std::fs::create_dir_all(&dir);
        let data = rec.encode();
        let _ = std::fs::write(dir.join("last.jnr"), &data);
        if best { let _ = std::fs::write(dir.join(best_name(&rec.difficulty)), &data); }
    }

    pub fn load_best_recording(&self, difficulty: &str) -> Option<Recording> {
        let b = std::fs::read(self.replay_dir().join(best_name(difficulty))).ok()?;
        Recording::decode(&b)
    }
    pub fn load_last_recording(&self) -> Option<Recording> {
        let b = std::fs::read(self.replay_dir().join("last.jnr")).ok()?;
        Recording::decode(&b)
    }

    /// Liste der gespeicherten Replays (letzte Runde, Rekordläufe je Schwierigkeit)
    pub fn run_replays_menu(&mut self) {
        let mut clock = Clock::new();
        loop {
            let mut items: Vec<(String, Recording)> = vec![];
            if let Some(r) = self.load_last_recording() { items.push((self.tde("Letzte Runde", "Last round"), r)); }
            for d in crate::game::save::DIFFICULTIES.iter() {
                if let Some(r) = self.load_best_recording(d) { items.push((format!("{} · {}", self.tde("Rekord", "Best"), self.get_difficulty_name(d)), r)); }
            }
            let fps = self.target_fps();
            clock.tick(fps);
            self.sync_window_size();
            self.fill_bg(true);
            let bs = self.base_surface.clone();
            let (w, h) = (self.WIDTH, self.HEIGHT);
            let mp = self.get_mouse_pos_logical();
            self.draw_title_shadow(&bs, "Replays", 20);
            let sf = self.small_font.clone();
            let tc = self.get_text_color();
            let mut rows = vec![];
            if items.is_empty() {
                let t = sf.render(&self.tde("Noch keine Replays - spiel eine Runde!", "No replays yet - play a round!"), true, (150, 150, 170));
                bs.blit(&t, (w / 2 - t.get_width() / 2, 160));
            }
            for (i, (label, r)) in items.iter().enumerate().take(((h - 200) / 50).max(1) as usize) {
                let row = Rect::new(w / 2 - 250, 110 + i as i32 * 50, 500, 42);
                let hov = row.collidepoint(mp) || self.nav_is_focused("replays", i, items.len() + 1);
                draw::rect(&bs, if hov { self.get_button_hover_color() } else { self.get_button_color() }, row, 0, 10);
                draw::rect(&bs, (120, 160, 240), row, if hov { 2 } else { 1 }, 10);
                bs.blit(&sf.render(label, true, tc), (row.x + 14, row.y + 10));
                let secs = r.seconds() as i64;
                let info = format!("{} {} · {}:{:02}", r.score, self.get_text("score"), secs / 60, secs % 60);
                let is = sf.render(&info, true, (150, 160, 190));
                bs.blit(&is, (row.right() - 14 - is.get_width(), row.y + 10));
                rows.push(row);
            }
            let back = Rect::new(w / 2 - 90, h - 64, 180, 44);
            let bh = back.collidepoint(mp) || self.nav_is_focused("replays", rows.len(), rows.len() + 1);
            self.draw_ui_button(&bs, back, &self.get_text("back"), BtnOpts { font: Some(sf.clone()), hovered: bh, ..Default::default() });
            self.flip();
            let mut all = rows.clone();
            all.push(back);
            let mut pick: Option<usize> = None;
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self.is_back_key(key) || key == K_m { self.play_sound(Snd::ClickBack); return; }
                        if let Some(dd) = self.nav_direction(key) {
                            let cur = self.nav_get_focus("replays", all.len());
                            self.nav_set_focus("replays", nav_move(&all, cur, dd));
                        } else if self.is_nav_confirm(key) {
                            match self.nav_get_focus("replays", all.len()) { Some(i) if i < rows.len() => pick = Some(i), _ => { self.play_sound(Snd::ClickBack); return; } }
                        }
                    }
                    Event::MouseButtonDown { button: 1, .. } => {
                        let p = self.get_mouse_pos_logical();
                        if back.collidepoint(p) { self.play_sound(Snd::ClickBack); return; }
                        pick = rows.iter().position(|r| r.collidepoint(p));
                    }
                    _ => {}
                }
            }
            if let Some(i) = pick {
                self.play_sound(Snd::MenuClick);
                let r = items[i].1.clone();
                self.run_replay(&r);
                if self.sound_enabled { self.play_menu_music(); }
            }
        }
    }

    /// Replay ansehen: Leertaste Pause, ←/→ 5 s spulen, 1/2/4 Tempo, ESC zurück
    pub fn run_replay(&mut self, rec: &Recording) {
        if rec.frames.is_empty() { return; }
        let mut clock = Clock::new();
        let mut pos = 0.0f64; // Frame-Index (mit Bruchteil)
        let mut speed = 1.0f64;
        let mut paused = false;
        let last = (rec.frames.len() - 1) as f64;
        self.stop_music();
        loop {
            let fps = self.target_fps();
            let ms = clock.tick_f(fps).min(100.0);
            self.sync_window_size();
            if !paused { pos = (pos + ms / LOGIC_TICK_MS * speed).min(last); }
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if self.is_back_key(key) || key == K_m { self.play_sound(Snd::ClickBack); return; }
                        if key == K_SPACE || key == K_p { paused = !paused; if pos >= last { pos = 0.0; paused = false; } }
                        if key == K_LEFT || key == K_a { pos = (pos - 5.0 * LOGIC_HZ).max(0.0); }
                        if key == K_RIGHT || key == K_d { pos = (pos + 5.0 * LOGIC_HZ).min(last); }
                        if key == K_1 { speed = 1.0; }
                        if key == K_2 { speed = 2.0; }
                        if key == K_4 { speed = 4.0; }
                    }
                    Event::MouseButtonDown { button: 1, .. } => { paused = !paused; }
                    _ => {}
                }
            }
            self.draw_replay_frame(rec, pos, paused, speed);
            self.flip();
        }
    }

    fn draw_replay_frame(&mut self, rec: &Recording, pos: f64, paused: bool, speed: f64) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        // Aufnahme-Welt einheitlich ins Fenster skalieren
        let s = (w as f64 / rec.w.max(1) as f64).min(h as f64 / rec.h.max(1) as f64);
        let (ox, oy) = ((w as f64 - rec.w as f64 * s) / 2.0, (h as f64 - rec.h as f64 * s) / 2.0);
        let tr = |x: i32, y: i32, rw: i32, rh: i32| -> Rect {
            let x0 = (ox + x as f64 * s).round() as i32; let y0 = (oy + y as f64 * s).round() as i32;
            Rect::new(x0, y0, ((x + rw) as f64 * s + ox).round() as i32 - x0, ((y + rh) as f64 * s + oy).round() as i32 - y0)
        };
        let i = pos.floor() as usize;
        let f = &rec.frames[i.min(rec.frames.len() - 1)];
        let nf = rec.frames.get(i + 1).unwrap_or(f);
        let a = pos - pos.floor();
        let dark = self.is_dark_mode();
        let floor = tr(0, rec.h - 48, rec.w, 2);
        super::art::draw_floor(&bs, w, h, floor.y, pos * 8.0 * s, dark);
        let tsec = self.get_ticks() as f64 / 1000.0;
        for (k, x, y, ow, oh) in &f.objs {
            let r = tr(*x as i32, *y as i32, *ow as i32, *oh as i32);
            match k {
                0 => super::art::draw_hazard(&bs, r),
                1 => { super::art::draw_faller(&bs, r); super::art::draw_faller_marker(&bs, r, tsec); }
                _ => super::art::draw_oneup(&bs, r, tsec, &self.badge_font),
            }
        }
        if f.visible {
            let px = f.px as f64 + (nf.px - f.px) as f64 * a;
            let py = f.py as f64 + (nf.py - f.py) as f64 * a;
            super::art::draw_player(&bs, tr(px.round() as i32, py.round() as i32, 50, 50), rec.color);
        }
        // Anzeige
        let sf = self.small_font.clone();
        let tc = self.get_text_color();
        self.hud_text(&format!("{}: {}", self.get_text("score"), f.score), 12, 10);
        self.hud_text(&format!("{}: {}", self.get_text("lives"), f.lives), 12, 32);
        let title = format!("Replay · {}", self.get_difficulty_name(&rec.difficulty));
        let t = self.menu_font.render(&title, true, tc);
        bs.blit(&t, (w / 2 - t.get_width() / 2, 8));
        let cur = pos / LOGIC_HZ;
        let tot = rec.seconds();
        let mmss = |v: f64| format!("{}:{:02}", v as i64 / 60, v as i64 % 60);
        let state = if paused { self.tde("Pause", "Paused") } else if speed > 1.0 { format!("{}×", speed) } else { String::new() };
        let info = sf.render(&format!("{} / {}  {}", mmss(cur), mmss(tot), state), true, (160, 170, 200));
        bs.blit(&info, (w / 2 - info.get_width() / 2, 8 + t.get_height()));
        let bar = Rect::new(40, h - 44, w - 80, 6);
        draw::rect(&bs, (60, 60, 80), bar, 0, 3);
        draw::rect(&bs, (120, 160, 240), Rect::new(bar.x, bar.y, ((bar.w as f64) * (pos / (rec.frames.len().max(2) - 1) as f64)) as i32, bar.h), 0, 3);
        let hint = self.tde("Leertaste Pause · ←/→ 5 s · 1/2/4 Tempo · ESC zurück", "Space pause · ←/→ 5 s · 1/2/4 speed · ESC back");
        let hs = sf.render(&hint, true, (110, 110, 135));
        bs.blit(&hs, (w / 2 - hs.get_width() / 2, h - 32));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hin_und_zurueck() {
        let mut r = Recording::new(1280, 720, (45, 95, 235), "Hölle");
        for i in 0..200 {
            r.push(Rect::new(50 + i, 600, 50, 50), i % 7 != 0, i as i64 / 10, 3, vec![(0u8, Rect::new(1200 - i * 8, 620, 50, 50)), (2u8, Rect::new(900, 600, 50, 50))].into_iter());
        }
        r.score = 20;
        let d = Recording::decode(&r.encode()).expect("lesbar");
        assert_eq!(d, r);
        assert!(Recording::decode(b"kaputt").is_none());
        assert!(Recording::decode(&r.encode()[..40]).is_none());
        assert_eq!(best_name("Hölle"), "best-Holle.jnr");
    }
}
