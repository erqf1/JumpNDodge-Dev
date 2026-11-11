//! Oberfläche des eingebauten Updaters: automatische Suche beim Start, Meldung im Hauptmenü
//! (Jetzt aktualisieren / Später / Ignorieren), Download mit Fortschritt, manuelle Suche.

use super::core::*;
use super::state::{Snd, G};
use crate::pg::*;
use crate::updater::{self, Release};
use crate::util::*;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

impl G {
    /// Suche im Hintergrund starten (manual = vom Spieler angestoßen)
    pub fn start_update_check(&mut self, manual: bool) {
        if self._upd_rx.is_some() { self._upd_manual |= manual; return; }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || { let _ = tx.send(updater::fetch_latest()); });
        self._upd_rx = Some(rx);
        self._upd_manual = manual;
        if manual { let t = self.tde("Suche nach Updates …", "Checking for updates …"); self.push_overlay(&t, (120, 170, 230), 1.5, None); }
    }

    /// Beim Start (nicht im unsichtbaren Test, im Debug-Build nur mit UPDATER_FAKE_VERSION)
    pub fn maybe_auto_update_check(&mut self) {
        if self.display.hidden && std::env::var("UPDATER_AUTO").is_err() { return; }
        if cfg!(debug_assertions) && std::env::var("UPDATER_FAKE_VERSION").is_err() { return; }
        if !self.settings.gb("auto_update_check", true) { return; }
        self.start_update_check(false);
    }

    fn poll_update_check(&mut self) {
        let Some(rx) = &self._upd_rx else { return; };
        let Ok(res) = rx.try_recv() else { return; };
        self._upd_rx = None;
        let manual = std::mem::take(&mut self._upd_manual);
        let cur = updater::current_version();
        match res {
            Ok(rel) => {
                println!("[UPDATE] neueste Version {} (installiert {})", rel.version, cur);
                if updater::is_newer(&rel.version, &cur) {
                    let attempted = self.settings.gs("update_attempted", "");
                    if !attempted.is_empty() {
                        self.settings.remove("update_attempted");
                        self.save_section("settings", None);
                        if attempted == rel.tag {
                            // letzter Versuch genau dieser Version hat nicht geklappt: nicht endlos neu anbieten
                            let t = self.tde("Update konnte nicht automatisch installiert werden - die Download-Seite öffnet sich.", "The update couldn't be installed automatically - the download page opens.");
                            self.push_overlay(&t, (220, 120, 120), 5.0, None);
                            updater::open_url(&rel.page);
                            return;
                        }
                    }
                    let ignored = self.settings.gs("update_ignored", "") == rel.tag;
                    let later = self.settings.gs("update_later", "") == rel.tag && now_secs() < self.settings.gf("update_later_until", 0.0);
                    self._upd_show = manual || (!ignored && !later);
                    self._upd_release = Some(rel);
                } else if manual {
                    let t = format!("{} ({})", self.tde("Du hast die neueste Version", "You're using the latest version"), cur);
                    self.push_overlay(&t, (90, 190, 140), 2.5, None);
                }
            }
            Err(e) => {
                println!("[UPDATE] Suche fehlgeschlagen: {}", e);
                if manual {
                    let t = format!("{}: {}", self.tde("Update-Suche fehlgeschlagen", "Update check failed"), e);
                    self.push_overlay(&t, (220, 120, 120), 3.0, None);
                }
            }
        }
    }

    /// Im Hauptmenü aufrufen: zeigt die Meldung, wenn ein Update bereitsteht
    pub fn update_dialog_if_due(&mut self) -> bool {
        self.poll_update_check();
        if !self._upd_show { return false; }
        self._upd_show = false;
        let Some(rel) = self._upd_release.clone() else { return false; };
        self.run_update_dialog(rel);
        true
    }

    fn upd_panel(&mut self) -> (Surface, Rect) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let r = Rect::new(w / 2 - 280, h / 2 - 170, 560, 340);
        draw::rect(&bs, self.get_panel_color(), r, 0, 18);
        draw::rect(&bs, (110, 140, 220), r, 2, 18);
        (bs, r)
    }

    fn run_update_dialog(&mut self, rel: Release) {
        let cur = updater::current_version();
        let auto_now = std::env::var("UPDATER_AUTO").map(|v| v == "now").unwrap_or(false);
        let mut clock = Clock::new();
        let notes: Vec<String> = rel.notes.lines().map(|l| l.trim().trim_start_matches(['-', '*', '#']).trim().to_string())
            .filter(|l| !l.is_empty()).take(5).collect();
        loop {
            let (bs, p) = self.upd_panel();
            let mf = self.menu_font.clone();
            let sf = self.small_font.clone();
            let tc = self.get_text_color();
            let title = mf.render(&self.tde("Update verfügbar", "Update available"), true, tc);
            bs.blit(&title, (p.centerx() - title.get_width() / 2, p.y + 20));
            let line = format!("JumpNDodge {} {} ({} {}).", rel.version, self.tde("ist da", "is available"), self.tde("du hast", "you have"), cur);
            let ls = sf.render(&line, true, (160, 170, 200));
            bs.blit(&ls, (p.centerx() - ls.get_width() / 2, p.y + 66));
            let mut y = p.y + 104;
            for n in &notes {
                let t = super::online_screens3::_fit_text_tail(n, &self.badge_font, p.w - 60);
                let s = self.badge_font.render(&format!("• {}", t), true, tc);
                bs.blit(&s, (p.x + 30, y));
                y += 24;
            }
            let mp = self.get_mouse_pos_logical();
            let now_b = Rect::new(p.x + 24, p.bottom() - 70, 230, 46);
            let later_b = Rect::new(p.x + 266, p.bottom() - 70, 130, 46);
            let ign_b = Rect::new(p.x + 408, p.bottom() - 70, 128, 46);
            self.draw_ui_button(&bs, now_b, &self.tde("Jetzt aktualisieren", "Update now"), BtnOpts { font: Some(sf.clone()), hovered: now_b.collidepoint(mp), border_color: Some((90, 190, 140)), ..Default::default() });
            self.draw_ui_button(&bs, later_b, &self.tde("Später", "Later"), BtnOpts { font: Some(sf.clone()), hovered: later_b.collidepoint(mp), ..Default::default() });
            self.draw_ui_button(&bs, ign_b, &self.tde("Ignorieren", "Skip"), BtnOpts { font: Some(sf.clone()), hovered: ign_b.collidepoint(mp), ..Default::default() });
            self.flip();
            let mut act = if auto_now { Some("now") } else { None };
            for ev in self.event_get() {
                match ev {
                    Event::Quit => self.quit(),
                    Event::KeyDown { key, .. } => {
                        if key == K_RETURN || key == K_KP_ENTER { act = Some("now"); }
                        else if self.is_back_key(key) { act = Some("later"); }
                    }
                    Event::MouseButtonDown { button: 1, .. } => {
                        let m = self.get_mouse_pos_logical();
                        if now_b.collidepoint(m) { act = Some("now"); }
                        else if later_b.collidepoint(m) { act = Some("later"); }
                        else if ign_b.collidepoint(m) { act = Some("ignore"); }
                    }
                    _ => {}
                }
            }
            match act {
                Some("later") => {
                    self.play_sound(Snd::ClickBack);
                    self.settings.insert("update_later".into(), json!(rel.tag));
                    self.settings.insert("update_later_until".into(), json!(now_secs() + 14.0 * 86400.0));
                    self.save_section("settings", None);
                    return;
                }
                Some("ignore") => {
                    self.play_sound(Snd::ClickBack);
                    self.settings.insert("update_ignored".into(), json!(rel.tag));
                    self.save_section("settings", None);
                    return;
                }
                Some("now") => { self.play_sound(Snd::Click); self.run_update_download(&rel); return; }
                _ => {}
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        }
    }

    fn run_update_download(&mut self, rel: &Release) {
        let Some((name, url, size)) = updater::pick_asset(rel) else {
            println!("[UPDATE] kein passendes Paket - öffne {}", rel.page);
            let t = self.tde("Kein passendes Paket für dieses System - die Download-Seite öffnet sich.", "No matching package for this system - the download page opens.");
            self.push_overlay(&t, (220, 160, 110), 4.0, None);
            updater::open_url(&rel.page);
            return;
        };
        println!("[UPDATE] lade {}", name);
        let dest = updater::temp_dir().join(&name);
        let got = Arc::new(AtomicU64::new(0));
        let total = Arc::new(AtomicU64::new(size));
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let (got, total, cancel, dest, url) = (got.clone(), total.clone(), cancel.clone(), dest.clone(), url.clone());
            std::thread::spawn(move || { let _ = tx.send(updater::download(&url, &dest, got, total, cancel)); });
        }
        let mut clock = Clock::new();
        let result = loop {
            if let Ok(r) = rx.try_recv() { break r; }
            let (bs, p) = self.upd_panel();
            let mf = self.menu_font.clone();
            let sf = self.small_font.clone();
            let tc = self.get_text_color();
            let title = mf.render(&self.tde("Update wird geladen …", "Downloading update …"), true, tc);
            bs.blit(&title, (p.centerx() - title.get_width() / 2, p.y + 40));
            let (g, t) = (got.load(Ordering::Relaxed), total.load(Ordering::Relaxed).max(1));
            let frac = (g as f64 / t as f64).clamp(0.0, 1.0);
            let bar = Rect::new(p.x + 40, p.y + 140, p.w - 80, 22);
            draw::rect(&bs, (50, 52, 70), bar, 0, 11);
            draw::rect(&bs, (100, 150, 240), Rect::new(bar.x, bar.y, ((bar.w as f64) * frac) as i32, bar.h), 0, 11);
            let info = format!("{:.1} / {:.1} MB", g as f64 / 1_048_576.0, t as f64 / 1_048_576.0);
            let is = sf.render(&info, true, (160, 170, 200));
            bs.blit(&is, (p.centerx() - is.get_width() / 2, p.y + 176));
            let mp = self.get_mouse_pos_logical();
            let cb = Rect::new(p.centerx() - 80, p.bottom() - 70, 160, 46);
            self.draw_ui_button(&bs, cb, &self.tde("Abbrechen", "Cancel"), BtnOpts { font: Some(sf.clone()), hovered: cb.collidepoint(mp), ..Default::default() });
            self.flip();
            for ev in self.event_get() {
                match ev {
                    Event::Quit => { cancel.store(true, Ordering::Relaxed); self.quit(); }
                    Event::KeyDown { key, .. } if self.is_back_key(key) => cancel.store(true, Ordering::Relaxed),
                    Event::MouseButtonDown { button: 1, .. } => { if cb.collidepoint(self.get_mouse_pos_logical()) { cancel.store(true, Ordering::Relaxed); } }
                    _ => {}
                }
            }
            { let fps = self.target_fps(); clock.tick(fps); }
        };
        match result {
            Ok(()) => {
                self.settings.insert("update_attempted".into(), json!(rel.tag));
                self.save_section("settings", None);
                crate::crypto::flush_saves();
                match updater::launch_installer(&dest) {
                    Ok(script) => {
                        println!("[UPDATE] Hilfsskript {} gestartet - beende das Spiel", script.display());
                        if std::env::var("UPDATER_DRY").map(|v| v == "1").unwrap_or(false) { println!("[UPDATE] Testlauf (UPDATER_DRY) - Spiel läuft weiter"); return; }
                        self.quit();
                    }
                    Err(e) => {
                        println!("[UPDATE] Hilfsskript fehlgeschlagen: {}", e);
                        self.settings.remove("update_attempted");
                        self.save_section("settings", None);
                        updater::open_url(&rel.page);
                    }
                }
            }
            Err(e) => {
                println!("[UPDATE] Download fehlgeschlagen: {}", e);
                if e != "abgebrochen" {
                    let t = format!("{}: {}", self.tde("Download fehlgeschlagen", "Download failed"), e);
                    self.push_overlay(&t, (220, 120, 120), 4.0, None);
                    updater::open_url(&rel.page);
                }
            }
        }
    }
}
