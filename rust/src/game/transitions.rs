//! Bildschirm-Übergänge mit Herkunft (nach Apples "Designing Fluid Interfaces"):
//! - Tiefer ins Menü = das Neue kommt von rechts, das Alte rückt etwas nach links.
//!   Zurück = derselbe Weg rückwärts (das Alte fährt nach rechts raus).
//! - Online (unten rechts), Profil (unten links) und Belohnungen (oben rechts) kommen aus
//!   ihrer Ecke und kehren dorthin zurück.
//! - Feder ohne Nachwippen (kritisch gedämpft, Antwort ~0,3 s), unterbrechbar: ein neuer Wechsel startet
//!   vom gerade gezeigten Bild aus. Eingaben werden nie gesperrt.
//! - "Reduziert" = nur Überblenden, "Aus" = sofort. Ins Spiel hinein wird nie animiert.
//!
//! Kosten: Ein Schnappschuss entsteht nur bei Klick/Taste, der Ausgabepuffer wird wiederverwendet.

use super::devconsole::ALL_SCREEN_FLAGS;
use super::state::G;
use crate::pg::*;
use std::cell::RefCell;

#[derive(Clone, Copy, PartialEq)]
enum Kind { Forward, Back, CornerIn(f64, f64), CornerOut(f64, f64), Fade }

struct Snap { surf: Surface, at: f64, screen: &'static str }
struct Active { from: Surface, kind: Kind, start: f64, to_screen: &'static str }

thread_local! {
    static SNAP: RefCell<Option<Snap>> = const { RefCell::new(None) };
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
    static LAST_COMPOSITE: RefCell<Option<Surface>> = const { RefCell::new(None) };
    static LAST_SCREEN: RefCell<&'static str> = const { RefCell::new("") };
    static BUF: RefCell<Option<Surface>> = const { RefCell::new(None) };
}

/// Menü-Tiefe; None = nie animieren (Spiel, Laden, Profilwahl)
fn depth(screen: &str) -> Option<i32> {
    Some(match screen {
        "in_menu" => 0,
        "in_shop_menu" | "in_loadout_menu" | "in_settings_menu" | "in_more_menu" | "in_difficulty_select" | "in_profile_hub"
        | "in_online_hub" | "in_daily_rewards" | "in_mods_menu" | "in_language_select" => 1,
        "in_player_search" | "in_public_profile" | "in_clan_create" | "in_clan_search" | "in_clan_view" | "in_chat" | "in_admin_panel" => 3,
        // Anleitung ist ein Pop-up mit eigener Einblendung über dem vorherigen Bild
        "game_running" | "in_classic_game" | "in_loading" | "in_profile_select" | "in_profile_name_input" | "in_username_gate" | "in_tutorial" | "" => return None,
        _ => 2,
    })
}

/// Ecke, aus der ein Bildschirm vom Hauptmenü aus kommt (Lage seines Knopfs)
fn corner(screen: &str) -> Option<(f64, f64)> {
    match screen {
        "in_online_hub" => Some((1.0, 1.0)),
        "in_profile_hub" => Some((-1.0, 1.0)),
        "in_daily_rewards" => Some((1.0, -1.0)),
        _ => None,
    }
}

fn now() -> f64 { super::art::anim_time() }

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

    /// Bei Klick/Taste aufrufen: merkt sich das zuletzt gezeigte Bild als möglichen Startpunkt
    pub fn transition_snapshot(&mut self) {
        if super::art::motion_level() == 0 || super::art::legacy() { return; }
        let screen = self.current_screen();
        if depth(screen).is_none() { return; }
        // Kopie: der Übergangs-Puffer wird im nächsten Bild wieder beschrieben
        let surf = LAST_COMPOSITE.with(|c| c.borrow().as_ref().map(|s| s.copy())).unwrap_or_else(|| self.base_surface.copy());
        SNAP.with(|s| *s.borrow_mut() = Some(Snap { surf, at: now(), screen }));
    }

    /// Im flip(): bei Bildschirmwechsel den Übergang starten und das zusammengesetzte Bild liefern
    pub fn apply_transition(&mut self, frame: &Surface) -> Option<Surface> {
        let screen = self.current_screen();
        let prev = LAST_SCREEN.with(|l| std::mem::replace(&mut *l.borrow_mut(), screen));
        let level = super::art::motion_level();
        if screen != prev {
            ACTIVE.with(|a| *a.borrow_mut() = None);
            let snap = SNAP.with(|s| s.borrow_mut().take());
            if let (Some(snap), Some(d_to)) = (snap, depth(screen)) {
                if level > 0 && !super::art::legacy() && snap.screen == prev && now() - snap.at < 1.0 {
                    if let Some(d_from) = depth(prev) {
                        let kind = if level == 1 { Kind::Fade }
                            else if prev == "in_menu" && corner(screen).is_some() { let (x, y) = corner(screen).unwrap(); Kind::CornerIn(x, y) }
                            else if screen == "in_menu" && corner(prev).is_some() { let (x, y) = corner(prev).unwrap(); Kind::CornerOut(x, y) }
                            else if d_to > d_from { Kind::Forward }
                            else if d_to < d_from { Kind::Back }
                            else { Kind::Fade };
                        ACTIVE.with(|a| *a.borrow_mut() = Some(Active { from: snap.surf, kind, start: now(), to_screen: screen }));
                    }
                }
            }
        }
        let out = ACTIVE.with(|a| {
            let mut a = a.borrow_mut();
            let act = a.as_ref()?;
            if act.to_screen != screen || !act.from.same_pixels(frame) { *a = None; return None; }
            let t = now() - act.start;
            let (p, done) = if act.kind == Kind::Fade {
                let p = (t / 0.16).clamp(0.0, 1.0);
                (p * p * (3.0 - 2.0 * p), t >= 0.16)
            } else {
                // kritisch gedämpfte Feder, Antwort 0,3 s
                let w = std::f64::consts::TAU / 0.3;
                (1.0 - (1.0 + w * t) * (-w * t).exp(), t >= 0.5)
            };
            if done { *a = None; return None; }
            let (w, h) = frame.get_size();
            let (wf, hf) = (w as f64, h as f64);
            // Puffer wiederverwenden (vorher: jedes Bild eine neue bildschirmgroße Fläche)
            let s = BUF.with(|b| {
                let mut b = b.borrow_mut();
                match b.as_ref() { Some(s) if s.same_pixels(frame) => s.clone(), _ => { let s = Surface::new(w, h); *b = Some(s.clone()); s } }
            });
            match act.kind {
                // Alt und Neu decken zusammen immer das ganze Bild ab -> kein Füllen nötig
                Kind::Forward => {
                    s.blit_f(&act.from, -wf * 0.25 * p, 0.0, 1.0);
                    s.blit_f(frame, wf * (1.0 - p), 0.0, 1.0);
                }
                Kind::Back => {
                    s.blit_f(frame, -wf * 0.25 * (1.0 - p), 0.0, 1.0);
                    s.blit_f(&act.from, wf * p, 0.0, 1.0);
                }
                Kind::CornerIn(cx, cy) => {
                    s.blit(&act.from, (0, 0));
                    s.blit_f(frame, cx * wf * 0.3 * (1.0 - p), cy * hf * 0.3 * (1.0 - p), p.min(1.0) as f32);
                }
                Kind::CornerOut(cx, cy) => {
                    s.blit(frame, (0, 0));
                    s.blit_f(&act.from, cx * wf * 0.3 * p, cy * hf * 0.3 * p, (1.0 - p).max(0.0) as f32);
                }
                Kind::Fade => {
                    s.blit(&act.from, (0, 0));
                    s.blit_f(frame, 0.0, 0.0, p as f32);
                }
            }
            Some(s)
        });
        LAST_COMPOSITE.with(|c| *c.borrow_mut() = out.clone());
        out
    }
}
