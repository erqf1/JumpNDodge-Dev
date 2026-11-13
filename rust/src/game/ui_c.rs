//! Menü-Look ab 5.3 (Stil C wie die Texturen): flache Flächen, Treppen-Ecken, harte Schatten.
//! Dazu zwei kleine Animationen, ohne dass die Bildschirme etwas davon wissen müssen:
//! - Reinfahren: Ein Knopf, der eine Weile nicht gezeichnet wurde (neuer Bildschirm), rutscht von unten rein,
//!   gestaffelt nach seiner Höhe auf dem Bildschirm.
//! - Hover: Knopf hebt sich weich an (Schatten wird länger), mit Akzent-Leiste links.
//! Klickflächen bleiben unverändert, es verschiebt sich nur die Optik.

use super::art::{anim_time, darken, lighten};
use crate::pg::*;
use std::cell::RefCell;
use std::collections::HashMap;

type Col = (i32, i32, i32);

struct Anim { hover: f64, seen: f64, born: f64 }

thread_local! {
    static ANIMS: RefCell<HashMap<(i32, i32, i32, i32), Anim>> = RefCell::new(HashMap::new());
}

/// (Hover 0..1, Reinfahr-Versatz in Pixeln) für ein Rechteck
fn anim(r: Rect, hovered: bool) -> (f64, i32) {
    let now = anim_time();
    ANIMS.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 600 { m.retain(|_, a| now - a.seen < 2.0); }
        let a = m.entry((r.x, r.y, r.w, r.h)).or_insert(Anim { hover: 0.0, seen: -10.0, born: now });
        let dt = (now - a.seen).clamp(0.0, 0.1);
        if now - a.seen > 0.35 { a.born = now; a.hover = 0.0; }
        a.seen = now;
        let target = if hovered { 1.0 } else { 0.0 };
        let k = 1.0 - (-dt * 18.0).exp();
        a.hover += (target - a.hover) * k;
        // gestaffelt: weiter unten = etwas später, alles in ~0.3 s fertig
        let delay = (r.y.max(0) as f64 / 600.0).min(1.0) * 0.12;
        let p = ((now - a.born - delay) / 0.2).clamp(0.0, 1.0);
        let ease = 1.0 - (1.0 - p).powi(3);
        (a.hover, ((1.0 - ease) * 18.0).round() as i32)
    })
}

/// Rechteck mit Treppen-Ecken (zwei Stufen à n)
pub fn step_rect<C: Into<Color> + Copy>(s: &Surface, c: C, r: Rect, n: i32) {
    if r.w <= 4 * n || r.h <= 4 * n { draw::rect(s, c, r, 0, 0); return; }
    draw::rect(s, c, Rect::new(r.x, r.y + 2 * n, r.w, r.h - 4 * n), 0, 0);
    draw::rect(s, c, Rect::new(r.x + n, r.y + n, r.w - 2 * n, r.h - 2 * n), 0, 0);
    draw::rect(s, c, Rect::new(r.x + 2 * n, r.y, r.w - 4 * n, r.h), 0, 0);
}

fn mix(a: Col, b: Col, t: f64) -> Col {
    let m = |x: i32, y: i32| (x as f64 + (y - x) as f64 * t).round() as i32;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Knopf im Stil C. `accent` = Farbe für Unterkante/Leiste (bisher die Randfarbe).
pub fn button(s: &Surface, r: Rect, text: &str, font: &Font, base: Col, hover_base: Col, accent: Col, text_col: Col, hovered: bool, dark: bool) {
    let (h, slide) = anim(r, hovered);
    let lift = (h * 3.0).round() as i32;
    let n = 3;
    let shadow = if dark { (10, 9, 16) } else { (150, 145, 175) };
    let body_r = Rect::new(r.x, r.y - lift + slide, r.w, r.h);
    // Schatten wird beim Anheben länger
    step_rect(s, shadow, Rect::new(body_r.x + 3, body_r.y + 4 + lift, r.w, r.h), n);
    let body = mix(base, hover_base, h);
    step_rect(s, body, body_r, n);
    // Unterkante in Akzentfarbe (dunkel) und helle Oberkante
    let band = darken(mix(body, accent, 0.55), 0.25);
    draw::rect(s, band, Rect::new(body_r.x + n, body_r.bottom() - n - 4, r.w - 2 * n, 4), 0, 0);
    draw::rect(s, band, Rect::new(body_r.x + 2 * n, body_r.bottom() - n, r.w - 4 * n, n), 0, 0);
    draw::rect(s, lighten(body, 0.18), Rect::new(body_r.x + 2 * n, body_r.y + n, r.w - 4 * n, 2), 0, 0);
    // Akzent-Leiste links fährt beim Hover rein
    if h > 0.02 {
        let lh = ((r.h - 4 * n) as f64 * h).round() as i32;
        draw::rect(s, accent, Rect::new(body_r.x + n + 2, body_r.centery() - lh / 2, 4, lh), 0, 0);
    }
    if !text.is_empty() {
        let ts = font.render(text, true, text_col);
        let (tx, ty) = (body_r.centerx() - ts.get_width() / 2, body_r.centery() - ts.get_height() / 2 - 1);
        let sh = font.render(text, true, if dark { (12, 10, 20) } else { darken(body, 0.35) });
        s.blit(&sh, (tx + 2, ty + 2));
        s.blit(&ts, (tx, ty));
    }
}

/// Panel/Karte im Stil C (harte Schattenkante, gestufte Ecken, Rahmen in `border`)
pub fn panel(s: &Surface, r: Rect, fill: Col, border: Col, bw: i32, shadow: bool, dark: bool) {
    let n = 3;
    if shadow { step_rect(s, if dark { (10, 9, 16) } else { (160, 155, 185) }, Rect::new(r.x + 4, r.y + 5, r.w, r.h), n); }
    let bw = bw.max(0);
    if bw > 0 {
        step_rect(s, border, r, n);
        step_rect(s, fill, r.inflate(-2 * bw, -2 * bw), n);
    } else {
        step_rect(s, fill, r, n);
    }
    draw::rect(s, lighten(fill, 0.08), Rect::new(r.x + 2 * n + bw, r.y + bw + 1, (r.w - 4 * n - 2 * bw).max(0), 2), 0, 0);
}
