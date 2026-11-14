//! Menü-Look ab 5.3: dunkle, ruhige Flächen mit einer Pixel-Stufe pro Ecke, Schatten nur nach unten
//! (symmetrisch), Unterkante im Ton des Knopfs. Farbe nur, wo sie etwas bedeutet (z.B. Start, Belohnungen);
//! Hover und Fokus zeigen sich mit einem lila Rahmen und 2 px Anheben, weich überblendet.
//! Klickflächen bleiben unverändert, es verschiebt sich nur die Optik.

use super::art::{anim_time, darken, lighten};
use crate::pg::*;
use std::cell::RefCell;
use std::collections::HashMap;

type Col = (i32, i32, i32);

/// Die eine Akzentfarbe der Oberfläche (Hover, Fokus, Hauptaktion)
pub const ACCENT: Col = (139, 108, 240);

struct Anim { hover: f64, seen: f64 }

thread_local! {
    static ANIMS: RefCell<HashMap<(i32, i32, i32, i32), Anim>> = RefCell::new(HashMap::new());
}

/// Hover 0..1, weich nachgeführt (unterbrechbar: läuft immer vom aktuellen Wert aus)
fn hover_amount(r: Rect, hovered: bool) -> f64 {
    let now = anim_time();
    ANIMS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 600 { m.retain(|_, a| now - a.seen < 2.0); }
        let a = m.entry((r.x, r.y, r.w, r.h)).or_insert(Anim { hover: if hovered { 1.0 } else { 0.0 }, seen: now });
        let dt = (now - a.seen).clamp(0.0, 0.1);
        a.seen = now;
        let target = if hovered { 1.0 } else { 0.0 };
        if super::art::motion_level() == 0 { a.hover = target; return a.hover; }
        a.hover += (target - a.hover) * (1.0 - (-dt * 22.0).exp());
        a.hover
    })
}

/// Rechteck mit einer Pixel-Stufe (Größe n) an jeder Ecke
pub fn step_rect<C: Into<Color> + Copy>(s: &Surface, c: C, r: Rect, n: i32) {
    if n <= 0 || r.w <= 2 * n || r.h <= 2 * n { draw::rect(s, c, r, 0, 0); return; }
    draw::rect(s, c, Rect::new(r.x, r.y + n, r.w, r.h - 2 * n), 0, 0);
    draw::rect(s, c, Rect::new(r.x + n, r.y, r.w - 2 * n, r.h), 0, 0);
}

pub fn luma(c: Col) -> f64 { 0.299 * c.0 as f64 + 0.587 * c.1 as f64 + 0.114 * c.2 as f64 }

/// Farbige Schrift lesbar machen: auf Dunkel zu dunkle Farben aufhellen, auf Hell zu helle abdunkeln
pub fn readable(c: Col, dark: bool) -> Col {
    let mut c = c;
    if dark { let mut i = 0; while luma(c) < 125.0 && i < 8 { c = mix(c, (255, 255, 255), 0.22); i += 1; } }
    else { let mut i = 0; while luma(c) > 160.0 && i < 8 { c = mix(c, (0, 0, 0), 0.2); i += 1; } }
    c
}

/// Lesbare Schriftfarbe für eine Fläche (dunkel auf hell, hell auf dunkel)
pub fn text_on(bg: Col) -> Col { if luma(bg) > 150.0 { (28, 24, 44) } else { (245, 243, 255) } }

pub fn mix(a: Col, b: Col, t: f64) -> Col {
    let m = |x: i32, y: i32| (x as f64 + (y - x) as f64 * t).round() as i32;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Knopf. `base`/`hover_base` = Flächenfarben, `ring` = Rahmenfarbe bei Hover/Fokus.
pub fn button(s: &Surface, r: Rect, text: &str, font: &Font, base: Col, hover_base: Col, ring: Col, text_col: Col, hovered: bool, dark: bool) {
    let h = hover_amount(r, hovered);
    let lift = (h * 3.0).round() as i32;
    let n = 3;
    let shadow = if dark { (12, 11, 20) } else { (176, 172, 204) };
    let body_r = Rect::new(r.x, r.y - lift, r.w, r.h);
    // Schatten genau darunter (symmetrisch), wird beim Anheben etwas länger
    step_rect(s, shadow, Rect::new(r.x, r.y + 4 - lift, r.w, r.h), n);
    // Hover hebt sich deutlich ab: heller (dunkel) bzw. leicht lila getönt (hell)
    let lifted = if dark { lighten(hover_base, 0.10) } else { mix(hover_base, ACCENT, 0.10) };
    let body = mix(base, lifted, h);
    let ring_on = h > 0.02;
    if ring_on {
        // Rahmen bei Hover/Fokus: weich vom Knopfton zur Akzentfarbe
        step_rect(s, mix(body, ring, h), body_r, n);
        step_rect(s, body, body_r.inflate(-4, -4), n - 1);
    } else {
        step_rect(s, body, body_r, n);
    }
    // Unterkante im selben Ton, nur dunkler; feine Lichtkante oben
    let inset = if ring_on { 2 } else { 0 };
    let iw = r.w - 2 * n - 2 * inset;
    draw::rect(s, darken(body, 0.24), Rect::new(body_r.x + n + inset, body_r.bottom() - 4 - inset, iw, 4), 0, 0);
    draw::rect(s, lighten(body, 0.10), Rect::new(body_r.x + n + inset, body_r.y + 1 + inset, iw, 1), 0, 0);
    // Streifen links (wächst beim Hover von der Mitte aus), nur bei breiten Knöpfen
    if h > 0.02 && r.w >= 110 && r.h >= 30 {
        let full = r.h - 18;
        let sh = ((full as f64) * h).round() as i32;
        if sh > 0 {
            let sc = if luma(body) > 150.0 { darken(ring, 0.15) } else { lighten(ring, 0.25) };
            draw::rect(s, sc, Rect::new(body_r.x + 7, body_r.centery() - 2 - sh / 2, 5, sh), 0, 0);
        }
    }
    if !text.is_empty() {
        let max_w = r.w - 22;
        let ts = font.render_fit(text, max_w, text_col);
        let (tx, ty) = (body_r.centerx() - ts.get_width() / 2, body_r.centery() - ts.get_height() / 2 - 2);
        // dezenter Schatten direkt darunter (kein seitlicher Versatz), nur auf dunklen Flächen
        if luma(body) < 150.0 {
            let sh = font.render_fit(text, max_w, darken(body, 0.55));
            s.blit(&sh, (tx, ty + 2));
        }
        s.blit(&ts, (tx, ty));
    }
}

/// Panel/Karte: eine Pixel-Stufe pro Ecke, Schatten nur nach unten, optional Rahmen
pub fn panel(s: &Surface, r: Rect, fill: Col, border: Col, bw: i32, shadow: bool, dark: bool) {
    let n = 3;
    if shadow { step_rect(s, if dark { (12, 11, 20) } else { (186, 182, 212) }, Rect::new(r.x, r.y + 5, r.w, r.h), n); }
    let bw = bw.max(0);
    if bw > 0 {
        step_rect(s, border, r, n);
        step_rect(s, fill, r.inflate(-2 * bw, -2 * bw), (n - 1).max(1));
    } else {
        step_rect(s, fill, r, n);
    }
    draw::rect(s, lighten(fill, 0.06), Rect::new(r.x + n + bw, r.y + bw + 1, (r.w - 2 * n - 2 * bw).max(0), 1), 0, 0);
}
