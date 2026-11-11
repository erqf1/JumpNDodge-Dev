//! Grafik der Rust Edition: Spielfigur, Hindernisse, 1-UP, Boden und Hintergrund.
//! Alles wird prozedural gezeichnet (Verläufe, Kanten, Glanz) und als Sprite je Größe/Farbe/
//! Auflösung zwischengespeichert - die Hitboxen bleiben exakt wie im Original.
//! Farbregel: nie Orange und Grün zusammen (1-UP ist türkis statt grün).

use crate::pg::*;
use std::cell::RefCell;
use std::collections::HashMap;

type Col = (i32, i32, i32);

pub fn lighten(c: Col, f: f64) -> Col {
    let l = |v: i32| (v as f64 + (255.0 - v as f64) * f).round() as i32;
    (l(c.0), l(c.1), l(c.2))
}
pub fn darken(c: Col, f: f64) -> Col {
    let d = |v: i32| (v as f64 * (1.0 - f)).round() as i32;
    (d(c.0), d(c.1), d(c.2))
}
fn with_a(c: Col, a: i32) -> (i32, i32, i32, i32) { (c.0, c.1, c.2, a) }

/// Farbe des 1-UP (türkis - verträgt sich mit jeder Spielerfarbe, auch Orange)
pub const ONEUP: Col = (45, 200, 200);
/// Bodenhindernis
pub const HAZARD: Col = (226, 58, 82);
/// Hindernis von oben
pub const FALLER: Col = (150, 70, 230);

thread_local! {
    static CACHE: RefCell<HashMap<String, Surface>> = RefCell::new(HashMap::new());
}

/// Sprite aus dem Zwischenspeicher (pro Render-Faktor neu, damit es scharf bleibt)
fn cached(key: String, make: impl FnOnce() -> Surface) -> Surface {
    let k = surface::render_scale();
    let key = format!("{}@{:.3}", key, k);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 400 { c.clear(); }
        c.entry(key).or_insert_with(make).clone()
    })
}

const PAD: i32 = 6;

/// Spielfigur: Verlauf, dunkler Rand, Glanzkante, weicher Schatten. `r` = gezeichnetes Rechteck.
pub fn draw_player(bs: &Surface, r: Rect, color: Col) {
    if r.w <= 0 || r.h <= 0 { return; }
    let rad = (r.w.min(r.h) / 5).max(4);
    let spr = cached(format!("p{}x{}:{:?}", r.w, r.h, color), || {
        let s = Surface::new_alpha(r.w + PAD * 2, r.h + PAD * 2);
        let b = Rect::new(PAD, PAD, r.w, r.h);
        // Schatten (leicht versetzt, weich)
        draw::rect_vgrad(&s, Rect::new(b.x + 2, b.y + 4, b.w, b.h), rad, with_a(darken(color, 0.7), 110), with_a(darken(color, 0.7), 60));
        // Körper
        draw::rect_vgrad(&s, b, rad, lighten(color, 0.28), darken(color, 0.18));
        // Rand
        draw::rect(&s, darken(color, 0.5), b, 2.max(r.w / 25), rad);
        // Glanzkante oben
        let gw = (b.w - 2 * rad).max(4);
        draw::rect_vgrad(&s, Rect::new(b.x + rad, b.y + 3, gw, (b.h / 7).max(3)), 3, with_a(lighten(color, 0.75), 200), with_a(lighten(color, 0.4), 40));
        s
    });
    bs.blit(&spr, (r.x - PAD, r.y - PAD));
}

/// Bodenhindernis: kantiger Block mit Warnstreifen
pub fn draw_hazard(bs: &Surface, r: Rect) {
    if r.w <= 0 || r.h <= 0 { return; }
    let spr = cached(format!("h{}x{}", r.w, r.h), || {
        let s = Surface::new_alpha(r.w + PAD * 2, r.h + PAD * 2);
        let b = Rect::new(PAD, PAD, r.w, r.h);
        draw::rect_vgrad(&s, Rect::new(b.x + 2, b.y + 3, b.w, b.h), 7, (60, 10, 25, 120), (60, 10, 25, 50));
        draw::rect_vgrad(&s, b, 7, lighten(HAZARD, 0.18), darken(HAZARD, 0.32));
        // Warnstreifen (dunkel, schräg) im Inneren
        let inner = Rect::new(b.x + 6, b.y + 6, b.w - 12, b.h - 12);
        if inner.w > 8 && inner.h > 8 {
            s.set_clip(Some(inner));
            let mut x = inner.x - inner.h;
            while x < inner.right() {
                draw::polygon(&s, darken(HAZARD, 0.3), &[(x, inner.bottom()), (x + 7, inner.bottom()), (x + 7 + inner.h, inner.y), (x + inner.h, inner.y)], 0);
                x += 16;
            }
            s.set_clip(None);
            draw::rect(&s, darken(HAZARD, 0.38), inner, 1, 4);
        }
        draw::rect(&s, darken(HAZARD, 0.55), b, 2, 7);
        // helle Oberkante
        draw::line(&s, lighten(HAZARD, 0.55), (b.x + 6, b.y + 2), (b.right() - 7, b.y + 2), 2);
        s
    });
    bs.blit(&spr, (r.x - PAD, r.y - PAD));
}

/// Hindernis von oben: Kristallblock mit Zacken nach unten
pub fn draw_faller(bs: &Surface, r: Rect) {
    if r.w <= 0 || r.h <= 0 { return; }
    let spr = cached(format!("f{}x{}", r.w, r.h), || {
        let s = Surface::new_alpha(r.w + PAD * 2, r.h + PAD * 2);
        let b = Rect::new(PAD, PAD, r.w, r.h);
        draw::glow(&s, (180, 120, 255, 70), (b.centerx(), b.centery()), (b.w.max(b.h) as f32) * 0.75);
        let body = Rect::new(b.x, b.y, b.w, b.h * 3 / 4);
        draw::rect_vgrad(&s, body, 6, lighten(FALLER, 0.3), darken(FALLER, 0.15));
        // Zacken
        let n = 4;
        let zw = b.w as f64 / n as f64;
        for i in 0..n {
            let x0 = b.x as f64 + i as f64 * zw;
            draw::polygon(&s, darken(FALLER, 0.1), &[(x0 + 1.0, body.bottom() as f64 - 1.0), (x0 + zw - 1.0, body.bottom() as f64 - 1.0), (x0 + zw / 2.0, b.bottom() as f64)], 0);
        }
        draw::rect(&s, darken(FALLER, 0.45), body, 2, 6);
        // Kristall-Kern (Raute) mit Lichtkante
        let (cx, cy) = (body.centerx() as f64, body.centery() as f64);
        let (dw, dh) = (body.w as f64 * 0.22, body.h as f64 * 0.32);
        draw::polygon(&s, lighten(FALLER, 0.35), &[(cx, cy - dh), (cx + dw, cy), (cx, cy + dh), (cx - dw, cy)], 0);
        draw::polygon(&s, lighten(FALLER, 0.65), &[(cx, cy - dh), (cx + dw * 0.45, cy - dh * 0.1), (cx - dw * 0.45, cy - dh * 0.1)], 0);
        s
    });
    bs.blit(&spr, (r.x - PAD, r.y - PAD));
}

/// Warnpfeil über einem fallenden Hindernis
pub fn draw_faller_marker(bs: &Surface, r: Rect, t: f64) {
    let a = 0.6 + 0.4 * (t * 8.0).sin().abs();
    let c = lighten(FALLER, 0.35 * a);
    let (ax, ay) = (r.centerx(), r.top() - 10);
    draw::polygon(bs, c, &[(ax, ay + 7), (ax - 6, ay), (ax + 6, ay)], 0);
}

/// 1-UP: türkise Kachel mit Herz und pulsierendem Glühen
pub fn draw_oneup(bs: &Surface, r: Rect, t: f64, label_font: &Font) {
    if r.w <= 0 || r.h <= 0 { return; }
    let pulse = 0.5 + 0.5 * (t * 4.0).sin();
    draw::glow(bs, with_a(ONEUP, (60.0 + 50.0 * pulse) as i32), (r.centerx(), r.centery()), r.w as f32 * (0.85 + 0.12 * pulse as f32));
    let spr = cached(format!("o{}x{}", r.w, r.h), || {
        let s = Surface::new_alpha(r.w + PAD * 2, r.h + PAD * 2);
        let b = Rect::new(PAD, PAD, r.w, r.h);
        draw::rect_vgrad(&s, b, 10, lighten(ONEUP, 0.25), darken(ONEUP, 0.3));
        draw::rect(&s, darken(ONEUP, 0.5), b, 2, 10);
        // Herz
        let (cx, cy) = (b.centerx() as f64, b.y as f64 + b.h as f64 * 0.30);
        let hs = b.w as f64 * 0.115;
        draw::aacircle(&s, (255, 255, 255), (cx - hs * 0.95, cy - hs * 0.2), hs as f32);
        draw::aacircle(&s, (255, 255, 255), (cx + hs * 0.95, cy - hs * 0.2), hs as f32);
        draw::polygon(&s, (255, 255, 255), &[(cx - hs * 1.9, cy + hs * 0.1), (cx + hs * 1.9, cy + hs * 0.1), (cx, cy + hs * 2.1)], 0);
        let t = label_font.render("1-UP", true, (255, 255, 255));
        s.blit(&t, (b.centerx() - t.get_width() / 2, b.bottom() - t.get_height() - 2));
        s
    });
    bs.blit(&spr, (r.x - PAD, r.y - PAD));
}

/// Boden: Linie mit Verlauf darunter und Markierungen, die mit dem Tempo vorbeiziehen
pub fn draw_floor(bs: &Surface, w: i32, h: i32, floor_y: i32, scroll: f64, dark: bool) {
    let (line_c, glow_c, band_top, band_bot, tick) = if dark {
        ((150, 100, 235), (120, 80, 220, 70), (44, 34, 74), (24, 22, 34), (90, 70, 150))
    } else {
        ((120, 90, 210), (140, 110, 230, 60), (222, 214, 248), (242, 244, 255), (170, 150, 225))
    };
    if floor_y < h {
        draw::rect_vgrad(bs, Rect::new(0, floor_y, w, h - floor_y), 0, band_top, band_bot);
        let off = (scroll.rem_euclid(48.0)) as i32;
        let mut x = -off;
        while x < w {
            draw::line(bs, tick, (x, floor_y + 8), (x + 18, floor_y + 8), 2);
            x += 48;
        }
    }
    let gl = cached(format!("fg{}", w), || {
        let s = Surface::new_alpha(w, 12);
        draw::rect_vgrad(&s, Rect::new(0, 0, w, 6), 0, (glow_c.0, glow_c.1, glow_c.2, 0), glow_c);
        s
    });
    bs.blit(&gl, (0, floor_y - 6));
    draw::line(bs, line_c, (0, floor_y), (w, floor_y), 2);
}

/// Hintergrund-Verlauf (statt einfarbig) - pro Größe einmal gezeichnet
pub fn background(w: i32, h: i32, dark: bool) -> Surface {
    cached(format!("bg{}x{}:{}", w, h, dark), || {
        let s = Surface::new(w, h);
        if dark {
            draw::rect_vgrad(&s, Rect::new(0, 0, w, h), 0, (34, 32, 52), (20, 20, 28));
        } else {
            draw::rect_vgrad(&s, Rect::new(0, 0, w, h), 0, (226, 234, 255), (246, 248, 255));
        }
        s
    })
}

/// Squash & Stretch beim Springen/Landen - nur Optik, die Hitbox bleibt
pub fn squash(r: Rect, vy: f64, land_ms: f64) -> Rect {
    let (sx, sy) = if land_ms < 110.0 {
        let k = 1.0 - land_ms / 110.0;
        (1.0 + 0.14 * k, 1.0 - 0.14 * k)
    } else if vy < -6.0 {
        (0.93, 1.07)
    } else { (1.0, 1.0) };
    if sx == 1.0 && sy == 1.0 { return r; }
    let nw = (r.w as f64 * sx).round() as i32;
    let nh = (r.h as f64 * sy).round() as i32;
    Rect::new(r.centerx() - nw / 2, r.bottom() - nh, nw, nh)
}
