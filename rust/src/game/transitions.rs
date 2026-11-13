//! Bildschirm-Übergänge mit Herkunft (nach Apples "Designing Fluid Interfaces") und Pop-ups.
//! - Tiefer ins Menü = das Neue kommt von rechts, das Alte rückt etwas nach links.
//!   Zurück = derselbe Weg rückwärts (das Alte fährt nach rechts raus).
//! - Pop-ups (Einstellungen, Belohnungen, Profil, Online, Anleitung, Code, Mods, Schwierigkeit):
//!   eine Karte über dem abgedunkelten Menü. Sie gleitet aus der Richtung ihres Knopfs herein und
//!   blendet auf. Klick neben die Karte = schließen (wie Esc).
//! - Feder ohne Nachwippen (kritisch gedämpft, Antwort ~0,3 s), unterbrechbar. Eingaben werden nie gesperrt.
//! - "Reduziert" = nur Überblenden, "Aus" = sofort. Ins Spiel hinein wird nie animiert.
//!
//! Kosten: Pop-ups werden direkt in Kartengröße gezeichnet (Render-Faktor * POPUP_SCALE) und dann nur
//! pixelgenau kopiert - kein Skalieren pro Bild. Alle Puffer werden wiederverwendet.

use super::devconsole::ALL_SCREEN_FLAGS;
use super::state::G;
use crate::pg::*;
use std::cell::{Cell, RefCell};

/// Größe der Pop-up-Karte relativ zum Fenster
pub const POPUP_SCALE: f64 = 0.84;

#[derive(Clone, Copy, PartialEq)]
enum Kind { Forward, Back, Fade }

struct Snap { surf: Surface, at: f64, screen: &'static str }
struct Active { from: Surface, kind: Kind, start: f64, to_screen: &'static str }

thread_local! {
    static SNAP: RefCell<Option<Snap>> = const { RefCell::new(None) };
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
    static LAST_COMPOSITE: RefCell<Option<Surface>> = const { RefCell::new(None) };
    static LAST_SCREEN: RefCell<&'static str> = const { RefCell::new("") };
    static BUF: RefCell<Option<Surface>> = const { RefCell::new(None) };
    /// Pop-up: letztes Bild des Ursprungs (Menü); abgedunkelte Fassung; Ausgabepuffer
    static POPUP_SRC: RefCell<Option<Surface>> = const { RefCell::new(None) };
    static POPUP_BG: RefCell<Option<Surface>> = const { RefCell::new(None) };
    static POPUP_BUF: RefCell<Option<Surface>> = const { RefCell::new(None) };
    /// Start der Einblendung und Richtung, aus der die Karte kommt
    static POPUP_IN: Cell<(f64, f64, f64)> = const { Cell::new((-10.0, 0.0, 1.0)) };
    /// Render-Faktor des ganzen Fensters (Pop-ups zeichnen kleiner)
    static FULL_K: Cell<f32> = const { Cell::new(1.0) };
}

pub fn set_full_k(k: f32) { FULL_K.with(|c| c.set(k)); }
pub fn full_k() -> f32 { FULL_K.with(|c| c.get()) }

/// Menü-Tiefe; None = nie animieren (Spiel, Laden, Profilwahl)
fn depth(screen: &str) -> Option<i32> {
    Some(match screen {
        "in_menu" => 0,
        "in_shop_menu" | "in_loadout_menu" | "in_settings_menu" | "in_more_menu" | "in_difficulty_select" | "in_profile_hub"
        | "in_online_hub" | "in_daily_rewards" | "in_mods_menu" | "in_language_select" => 1,
        "in_player_search" | "in_public_profile" | "in_clan_create" | "in_clan_search" | "in_clan_view" | "in_chat" | "in_admin_panel" => 3,
        "game_running" | "in_classic_game" | "in_loading" | "in_profile_select" | "in_profile_name_input" | "in_username_gate" | "" => return None,
        _ => 2,
    })
}

/// Bildschirme, die als Karte über dem Menü erscheinen
pub fn is_popup_screen(screen: &str) -> bool {
    matches!(screen,
        "in_settings_menu" | "in_settings_more_menu" | "in_settings_sound_menu" | "in_controls_menu" | "in_experimental_menu"
        | "in_daily_rewards" | "in_tutorial" | "in_help" | "in_redeem_code" | "in_mods_menu" | "in_difficulty_select"
        | "in_profile_hub" | "in_online_hub")
}

/// Aus welcher Richtung die Karte kommt (Lage ihres Knopfs im Hauptmenü)
fn popup_dir(screen: &str) -> (f64, f64) {
    match screen {
        "in_online_hub" => (1.0, 1.0),
        "in_profile_hub" | "in_settings_menu" => (-1.0, 1.0),
        "in_daily_rewards" => (1.0, -1.0),
        _ => (0.0, 1.0),
    }
}

/// Darf ein Bildschirm Hintergrund eines Pop-ups sein? (Menüs der ersten Ebene, keine Pop-ups)
fn bg_source(screen: &str) -> bool { matches!(depth(screen), Some(0) | Some(1)) && !is_popup_screen(screen) }

fn now() -> f64 { super::art::anim_time() }

fn spring(t: f64, response: f64) -> f64 {
    let w = std::f64::consts::TAU / response;
    (1.0 - (1.0 + w * t) * (-w * t).exp()).clamp(0.0, 1.0)
}

impl G {
    /// Der Bildschirm, der gerade "vorne" ist (bei mehreren aktiven Flags der tiefste)
    pub fn current_screen(&mut self) -> &'static str {
        let mut best: (&'static str, i32) = ("", -1);
        for f in ALL_SCREEN_FLAGS {
            if self.get_flag(f) {
                let d = depth(f).unwrap_or(99);
                if d > best.1 { best = (f, d); }
            }
        }
        best.0
    }

    /// Wird gerade ein Pop-up gezeigt? (nicht mit Legacy-Grafik: dort wie im Python-Original)
    pub fn popup_active(&mut self) -> bool { !super::art::legacy() && is_popup_screen(self.current_screen()) }

    /// Linke obere Ecke der Pop-up-Karte in logischen Fensterkoordinaten
    pub fn popup_origin(&self) -> (f64, f64) {
        (self.WIDTH as f64 * (1.0 - POPUP_SCALE) / 2.0, self.HEIGHT as f64 * (1.0 - POPUP_SCALE) / 2.0)
    }

    /// Fenster-Position -> Position auf der Karte
    pub fn popup_map(&self, p: (i32, i32)) -> (i32, i32) {
        let (ox, oy) = self.popup_origin();
        (((p.0 as f64 - ox) / POPUP_SCALE).floor() as i32, ((p.1 as f64 - oy) / POPUP_SCALE).floor() as i32)
    }

    /// Liegt eine Fenster-Position außerhalb der Karte?
    pub fn popup_outside(&self, p: (i32, i32)) -> bool {
        let (x, y) = self.popup_map(p);
        x < 0 || y < 0 || x >= self.WIDTH || y >= self.HEIGHT
    }

    /// Bei Klick/Taste aufrufen: merkt sich das zuletzt gezeigte Bild (Übergang und Pop-up-Hintergrund)
    pub fn transition_snapshot(&mut self) {
        if super::art::legacy() { return; }
        let screen = self.current_screen();
        if depth(screen).is_none() { return; }
        let anim = super::art::motion_level() > 0;
        let bg = bg_source(screen);
        if !anim && !bg { return; }
        // Kopie: die Puffer werden im nächsten Bild wieder beschrieben
        let surf = LAST_COMPOSITE.with(|c| c.borrow().as_ref().map(|s| s.copy())).unwrap_or_else(|| self.base_surface.copy());
        if bg { POPUP_SRC.with(|p| *p.borrow_mut() = Some(surf.clone())); }
        if anim { SNAP.with(|s| *s.borrow_mut() = Some(Snap { surf, at: now(), screen })); }
    }

    /// Pop-up-Karte über das abgedunkelte Menü legen
    fn compose_popup(&mut self, card: &Surface) -> Surface {
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let k = full_k();
        let dark = self.is_dark_mode();
        let out = POPUP_BUF.with(|b| {
            let mut b = b.borrow_mut();
            let want = (px_len(w, k), px_len(h, k));
            let ok = b.as_ref().map(|s| s.pixel_size() == want && (s.scale_k() - k).abs() < 1e-4).unwrap_or(false);
            if !ok { *b = Some(Surface::new_scaled(w, h, k)); }
            b.as_ref().unwrap().clone()
        });
        // Hintergrund: abgedunkeltes Menü (einmal vorbereitet)
        match POPUP_BG.with(|b| b.borrow().clone()).filter(|b| b.same_pixels(&out)) {
            Some(b) => out.blit(&b, (0, 0)),
            None => { out.fill(self.get_bg_color()); out.fill_blend((0, 0, 0), if dark { 110 } else { 70 }); }
        }
        let (t0, dx, dy) = POPUP_IN.with(|c| c.get());
        let lvl = super::art::motion_level();
        let p = match lvl { 0 => 1.0, 1 => ((now() - t0) / 0.16).clamp(0.0, 1.0), _ => spring(now() - t0, 0.3) };
        let slide = if lvl == 2 { 1.0 - p } else { 0.0 };
        let (ox, oy) = self.popup_origin();
        let (cx, cy) = (ox + dx * 46.0 * slide, oy + dy * 46.0 * slide);
        let cr = Rect::new(cx.round() as i32, cy.round() as i32, (w as f64 * POPUP_SCALE).round() as i32, (h as f64 * POPUP_SCALE).round() as i32);
        // Schatten nach unten + Rahmen (schmale Streifen, billig)
        let a = (p * 255.0) as i32;
        let shadow = if dark { (8, 7, 14, (170.0 * p) as i32) } else { (120, 112, 160, (90.0 * p) as i32) };
        draw::rect(&out, shadow, Rect::new(cr.x + 3, cr.bottom() + 3, cr.w - 6, 6), 0, 0);
        let border = super::ui_c::mix(super::ui_c::ACCENT, if dark { (40, 36, 60) } else { (255, 255, 255) }, 0.45);
        let bc = (border.0, border.1, border.2, a);
        draw::rect(&out, bc, Rect::new(cr.x, cr.y - 3, cr.w, 3), 0, 0);
        draw::rect(&out, bc, Rect::new(cr.x, cr.bottom(), cr.w, 3), 0, 0);
        draw::rect(&out, bc, Rect::new(cr.x - 3, cr.y, 3, cr.h), 0, 0);
        draw::rect(&out, bc, Rect::new(cr.right(), cr.y, 3, cr.h), 0, 0);
        out.blit_raw(card, (cr.x as f32 * k).round() as i32, (cr.y as f32 * k).round() as i32, p as f32);
        out
    }

    /// Im flip(): bei Bildschirmwechsel den Übergang starten und das zusammengesetzte Bild liefern
    pub fn apply_transition(&mut self, frame: &Surface) -> Option<Surface> {
        let screen = self.current_screen();
        let prev = LAST_SCREEN.with(|l| std::mem::replace(&mut *l.borrow_mut(), screen));
        let level = super::art::motion_level();
        let legacy = super::art::legacy();
        let popup = !legacy && is_popup_screen(screen);
        let was_popup = !legacy && is_popup_screen(prev);
        if screen != prev {
            ACTIVE.with(|a| *a.borrow_mut() = None);
            let snap = SNAP.with(|s| s.borrow_mut().take());
            if popup && !was_popup {
                // Pop-up geht auf: Hintergrund vorbereiten, Karte gleitet aus der Knopf-Richtung herein
                let src = POPUP_SRC.with(|p| p.borrow().clone());
                let dim = if self.is_dark_mode() { 120 } else { 80 };
                let bg = src.map(|s| { let c = s.copy(); c.fill_blend((0, 0, 0), dim); c });
                POPUP_BG.with(|b| *b.borrow_mut() = bg);
                let (dx, dy) = popup_dir(screen);
                POPUP_IN.with(|c| c.set((now(), dx, dy)));
            } else if let (Some(snap), Some(d_to)) = (snap, depth(screen)) {
                if level > 0 && !legacy && snap.screen == prev && now() - snap.at < 1.0 {
                    if let Some(d_from) = depth(prev) {
                        let kind = if level == 1 || popup || was_popup { Kind::Fade }
                            else if d_to > d_from { Kind::Forward }
                            else if d_to < d_from { Kind::Back }
                            else { Kind::Fade };
                        ACTIVE.with(|a| *a.borrow_mut() = Some(Active { from: snap.surf, kind, start: now(), to_screen: screen }));
                    }
                }
            }
            if !popup { POPUP_BG.with(|b| *b.borrow_mut() = None); }
        }
        let composed = if popup { Some(self.compose_popup(frame)) } else { None };
        let frame_c = composed.clone().unwrap_or_else(|| frame.clone());
        let out = ACTIVE.with(|a| {
            let mut a = a.borrow_mut();
            let act = a.as_ref()?;
            if act.to_screen != screen || !act.from.same_pixels(&frame_c) { *a = None; return None; }
            let t = now() - act.start;
            let (p, done) = if act.kind == Kind::Fade {
                let p = (t / 0.16).clamp(0.0, 1.0);
                (p * p * (3.0 - 2.0 * p), t >= 0.16)
            } else {
                (spring(t, 0.3), t >= 0.5)
            };
            if done { *a = None; return None; }
            let (w, h) = frame_c.get_size();
            let wf = w as f64;
            let s = BUF.with(|b| {
                let mut b = b.borrow_mut();
                match b.as_ref() { Some(s) if s.same_pixels(&frame_c) => s.clone(), _ => { let s = Surface::new_scaled(w, h, frame_c.scale_k()); *b = Some(s.clone()); s } }
            });
            match act.kind {
                // Alt und Neu decken zusammen immer das ganze Bild ab -> kein Füllen nötig
                Kind::Forward => {
                    s.blit_f(&act.from, -wf * 0.25 * p, 0.0, 1.0);
                    s.blit_f(&frame_c, wf * (1.0 - p), 0.0, 1.0);
                }
                Kind::Back => {
                    s.blit_f(&frame_c, -wf * 0.25 * (1.0 - p), 0.0, 1.0);
                    s.blit_f(&act.from, wf * p, 0.0, 1.0);
                }
                Kind::Fade => {
                    s.blit(&act.from, (0, 0));
                    s.blit_f(&frame_c, 0.0, 0.0, p as f32);
                }
            }
            Some(s)
        });
        let out = out.or(composed);
        LAST_COMPOSITE.with(|c| *c.borrow_mut() = out.clone());
        out
    }
}

fn px_len(v: i32, k: f32) -> u32 { ((v.max(1) as f32) * k).round().max(1.0) as u32 }
