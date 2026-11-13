//! Grafik der Rust Edition: Spielfigur, Hindernisse, 1-UP, Boden und Hintergrund.
//! Alles wird prozedural gezeichnet (Verläufe, Kanten, Glanz) und als Sprite je Größe/Farbe/
//! Auflösung zwischengespeichert - die Hitboxen bleiben exakt wie im Original.
//! Farbregel: nie Orange und Grün zusammen (1-UP ist türkis statt grün).
//!
//! Seit 5.3: Standard sind die Texturen aus `assets/textures` (Pixel-Raster im Stil der Python-Version:
//! flach, harte Schatten, Treppen-Ecken). "Legacy-Grafik" (Einstellung `legacy_art`) zeichnet alles exakt
//! wie die ursprüngliche Python-Version (JumpNDodge.py: einfarbige Rechtecke, harte Schatten, rote Blöcke,
//! grünes 1-UP, zwei Bodenlinien, einfarbiger Hintergrund, kein Squash & Stretch).

use crate::pg::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

thread_local! {
    static LEGACY: Cell<bool> = const { Cell::new(false) };
    static ANIM_T: Cell<f64> = const { Cell::new(0.0) };
    static TEX: RefCell<HashMap<String, Option<(Surface, f64, f64)>>> = RefCell::new(HashMap::new());
}
/// Legacy-Grafik an/aus (wird jedes Bild aus den Einstellungen gesetzt)
pub fn set_legacy(v: bool) { LEGACY.with(|c| c.set(v)); crate::pg::surface::set_step_corners(!v); }
pub fn legacy() -> bool { LEGACY.with(|c| c.get()) }
/// Laufende Zeit in Sekunden für Animationen in Texturen (jedes Bild gesetzt)
pub fn set_anim_time(t: f64) { ANIM_T.with(|c| c.set(t)); }
pub fn anim_time() -> f64 { ANIM_T.with(|c| c.get()) }

fn rgb8(c: Col) -> jnd_tex::Rgb { (c.0.clamp(0, 255) as u8, c.1.clamp(0, 255) as u8, c.2.clamp(0, 255) as u8) }
pub fn pal(c: Col) -> Vec<(&'static str, String)> { jnd_tex::palette(rgb8(c)) }

/// Textur so gerendert, dass ihr Body genau w x h logische Pixel groß ist. Liefert Fläche und Versatz
/// zur linken oberen Ecke des Spiel-Rechtecks. Zwischengespeichert pro Größe, Farbe und Render-Faktor.
pub fn tex(name: &str, vars: &[(&str, String)], w: f64, h: f64) -> Option<(Surface, f64, f64)> {
    if w <= 0.5 || h <= 0.5 { return None; }
    let k = surface::render_scale();
    let mut key = format!("{name}|{:.2}x{:.2}@{:.3}", w, h, k);
    for (a, b) in vars { key.push('|'); key.push_str(a); key.push('='); key.push_str(b); }
    TEX.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 1500 { c.clear(); }
        c.entry(key).or_insert_with(|| {
            jnd_tex::render_fit(name, vars, w as f32, h as f32, k)
                .map(|r| (Surface::from_pixmap_k(r.pixmap, k), r.off_x as f64, r.off_y as f64))
        }).clone()
    })
}

/// Textur auf das Spiel-Rechteck legen. false = Textur fehlt (dann zeichnet der Aufrufer Legacy).
pub fn put(bs: &Surface, name: &str, vars: &[(&str, String)], r: Rect, opacity: f32) -> bool {
    match tex(name, vars, r.w as f64, r.h as f64) {
        Some((s, ox, oy)) => { bs.blit_f(&s, r.x as f64 + ox, r.y as f64 + oy, opacity); true }
        None => false,
    }
}

/// Eigene Texturen aus <Spielordner>/textures (überschreiben die eingebauten)
pub fn init_texture_dir(base: &std::path::Path) {
    let d = base.join("textures");
    jnd_tex::set_override_dir(if d.is_dir() { Some(d) } else { None });
}

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

// ---------------- Python-Original (Legacy-Grafik) ----------------
/// Python RED / GREEN (JumpNDodge.py)
pub const PY_RED: Col = (255, 0, 0);
pub const PY_GREEN: Col = (0, 200, 0);
pub const PY_FALLER: Col = (160, 0, 200);

fn py_player(bs: &Surface, r: Rect, color: Col, shadow: bool) {
    if shadow { draw::rect(bs, (100, 100, 150), Rect::new(r.x + 3, r.y + 3, r.w, r.h), 0, 8); }
    draw::rect(bs, color, r, 0, 8);
}

/// Avatar in Menüs (Python: ohne Schatten), sonst wie die Spielfigur
pub fn draw_avatar_body(bs: &Surface, r: Rect, color: Col) {
    if legacy() { py_player(bs, r, color, false); } else { draw_player(bs, r, color); }
}

/// Spielfigur: Verlauf, dunkler Rand, Glanzkante, weicher Schatten. `r` = gezeichnetes Rechteck.
pub fn draw_player(bs: &Surface, r: Rect, color: Col) {
    if r.w <= 0 || r.h <= 0 { return; }
    if legacy() { py_player(bs, r, color, true); return; }
    if !legacy() && put(bs, "player", &pal(color), r, 1.0) { return; }
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

/// Geist (Rekordlauf): halbtransparente Spielfigur
pub fn draw_ghost(bs: &Surface, r: Rect, color: Col) {
    if r.w <= 0 || r.h <= 0 { return; }
    if legacy() {
        let spr = cached(format!("pyghost{}x{}:{:?}", r.w, r.h, color), || {
            let s = Surface::new_alpha(r.w, r.h);
            draw::rect(&s, lighten(color, 0.35), Rect::new(0, 0, r.w, r.h), 0, 8);
            s.set_alpha(Some(95));
            s
        });
        bs.blit(&spr, (r.x, r.y));
        return;
    }
    if !legacy() && put(bs, "player", &pal(lighten(color, 0.25)), r, 0.4) { return; }
    let spr = cached(format!("g{}x{}:{:?}", r.w, r.h, color), || {
        let s = Surface::new_alpha(r.w + PAD * 2, r.h + PAD * 2);
        let b = Rect::new(PAD, PAD, r.w, r.h);
        let c = lighten(color, 0.35);
        draw::rect_vgrad(&s, b, (r.w.min(r.h) / 5).max(4), lighten(c, 0.3), c);
        draw::rect(&s, lighten(c, 0.6), b, 2, (r.w.min(r.h) / 5).max(4));
        s.set_alpha(Some(95));
        s
    });
    bs.blit(&spr, (r.x - PAD, r.y - PAD));
}

/// Bodenhindernis: kantiger Block mit Warnstreifen
pub fn draw_hazard(bs: &Surface, r: Rect) { draw_hazard_c(bs, r, HAZARD) }
pub fn draw_hazard_c(bs: &Surface, r: Rect, base: Col) {
    if r.w <= 0 || r.h <= 0 { return; }
    if legacy() {
        let (fill, shadow) = if base == HAZARD { (PY_RED, (150, 50, 50)) } else { (base, darken(base, 0.4)) };
        draw::rect(bs, shadow, Rect::new(r.x + 2, r.y + 2, r.w, r.h), 0, 6);
        draw::rect(bs, fill, r, 0, 6);
        return;
    }
    if !legacy() {
        draw_speed_streaks(bs, r, base);
        if put(bs, "hazard", &pal(base), r, 1.0) { return; }
    }
    let spr = cached(format!("h{}x{}:{:?}", r.w, r.h, base), || {
        let s = Surface::new_alpha(r.w + PAD * 2, r.h + PAD * 2);
        let b = Rect::new(PAD, PAD, r.w, r.h);
        draw::rect_vgrad(&s, Rect::new(b.x + 2, b.y + 3, b.w, b.h), 7, (60, 10, 25, 120), (60, 10, 25, 50));
        draw::rect_vgrad(&s, b, 7, lighten(base, 0.18), darken(base, 0.32));
        // Warnstreifen (dunkel, schräg) im Inneren
        let inner = Rect::new(b.x + 6, b.y + 6, b.w - 12, b.h - 12);
        if inner.w > 8 && inner.h > 8 {
            s.set_clip(Some(inner));
            let mut x = inner.x - inner.h;
            while x < inner.right() {
                draw::polygon(&s, darken(base, 0.3), &[(x, inner.bottom()), (x + 7, inner.bottom()), (x + 7 + inner.h, inner.y), (x + inner.h, inner.y)], 0);
                x += 16;
            }
            s.set_clip(None);
            draw::rect(&s, darken(base, 0.38), inner, 1, 4);
        }
        draw::rect(&s, darken(base, 0.55), b, 2, 7);
        // helle Oberkante
        draw::line(&s, lighten(base, 0.55), (b.x + 6, b.y + 2), (b.right() - 7, b.y + 2), 2);
        s
    });
    bs.blit(&spr, (r.x - PAD, r.y - PAD));
}

/// Hindernis von oben: Kristallblock mit Zacken nach unten
pub fn draw_faller(bs: &Surface, r: Rect) { draw_faller_c(bs, r, FALLER) }
pub fn draw_faller_c(bs: &Surface, r: Rect, base: Col) {
    if r.w <= 0 || r.h <= 0 { return; }
    if legacy() {
        let (fill, shadow) = if base == FALLER { (PY_FALLER, (80, 0, 110)) } else { (base, darken(base, 0.45)) };
        draw::rect(bs, shadow, Rect::new(r.x + 2, r.y + 2, r.w, r.h), 0, 6);
        draw::rect(bs, fill, r, 0, 6);
        return;
    }
    if !legacy() && put(bs, "faller", &pal(base), r, 1.0) { return; }
    let spr = cached(format!("f{}x{}:{:?}", r.w, r.h, base), || {
        let s = Surface::new_alpha(r.w + PAD * 2, r.h + PAD * 2);
        let b = Rect::new(PAD, PAD, r.w, r.h);
        let gc = lighten(base, 0.3);
        draw::glow(&s, (gc.0, gc.1, gc.2, 70), (b.centerx(), b.centery()), (b.w.max(b.h) as f32) * 0.75);
        let body = Rect::new(b.x, b.y, b.w, b.h * 3 / 4);
        draw::rect_vgrad(&s, body, 6, lighten(base, 0.3), darken(base, 0.15));
        // Zacken
        let n = 4;
        let zw = b.w as f64 / n as f64;
        for i in 0..n {
            let x0 = b.x as f64 + i as f64 * zw;
            draw::polygon(&s, darken(base, 0.1), &[(x0 + 1.0, body.bottom() as f64 - 1.0), (x0 + zw - 1.0, body.bottom() as f64 - 1.0), (x0 + zw / 2.0, b.bottom() as f64)], 0);
        }
        draw::rect(&s, darken(base, 0.45), body, 2, 6);
        // Kristall-Kern (Raute) mit Lichtkante
        let (cx, cy) = (body.centerx() as f64, body.centery() as f64);
        let (dw, dh) = (body.w as f64 * 0.22, body.h as f64 * 0.32);
        draw::polygon(&s, lighten(base, 0.35), &[(cx, cy - dh), (cx + dw, cy), (cx, cy + dh), (cx - dw, cy)], 0);
        draw::polygon(&s, lighten(base, 0.65), &[(cx, cy - dh), (cx + dw * 0.45, cy - dh * 0.1), (cx - dw * 0.45, cy - dh * 0.1)], 0);
        s
    });
    bs.blit(&spr, (r.x - PAD, r.y - PAD));
}

/// Warnpfeil über einem fallenden Hindernis
pub fn draw_faller_marker(bs: &Surface, r: Rect, t: f64) {
    if legacy() {
        // Python: fester Pfeil, kein Blinken
        let (ax, ay) = (r.centerx(), r.top() - 8);
        draw::polygon(bs, (200, 100, 255), &[(ax, ay + 7), (ax - 6, ay), (ax + 6, ay)], 0);
        return;
    }
    let a = 0.6 + 0.4 * (t * 8.0).sin().abs();
    let c = lighten(FALLER, 0.35 * a);
    let (ax, ay) = (r.centerx(), r.top() - 10);
    draw::polygon(bs, c, &[(ax, ay + 7), (ax - 6, ay), (ax + 6, ay)], 0);
}

/// 1-UP: türkise Kachel mit Herz und pulsierendem Glühen
pub fn draw_oneup(bs: &Surface, r: Rect, t: f64, label_font: &Font) {
    if r.w <= 0 || r.h <= 0 { return; }
    if legacy() {
        // Python: hellgrüner Rand, grünes Feld, "1-UP" in Weiß mittig
        draw::rect(bs, (100, 255, 100), Rect::new(r.x - 3, r.y - 3, r.w + 6, r.h + 6), 0, 10);
        draw::rect(bs, PY_GREEN, r, 0, 8);
        let txt = label_font.render("1-UP", true, (255, 255, 255));
        bs.blit(&txt, (r.centerx() - txt.get_width() / 2, r.centery() - txt.get_height() / 2));
        return;
    }
    let pulse = 0.5 + 0.5 * (t * 4.0).sin();
    draw::glow(bs, with_a(ONEUP, (60.0 + 50.0 * pulse) as i32), (r.centerx(), r.centery()), r.w as f32 * (0.85 + 0.12 * pulse as f32));
    if !legacy() {
        // kleines Wippen in ganzen Pixeln statt weichem Schweben (passt zum Pixel-Stil)
        let bob = if (t * 3.0).sin() > 0.0 { -2 } else { 0 };
        if put(bs, "oneup", &pal(ONEUP), Rect::new(r.x, r.y + bob, r.w, r.h), 1.0) { return; }
    }
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

/// Boden. Steht still - die Hindernisse bewegen sich selbst (Nutzerwunsch), `_scroll` wird ignoriert.
pub fn draw_floor(bs: &Surface, w: i32, h: i32, floor_y: i32, _scroll: f64, dark: bool) {
    if legacy() {
        // Python: zwei lila Linien, sonst nichts
        let _ = (h, dark);
        draw::line(bs, (60, 30, 110), (0, floor_y - 1), (w, floor_y - 1), 1);
        draw::line(bs, (140, 80, 220), (0, floor_y), (w, floor_y), 2);
        return;
    }
    if !legacy() {
        let name = if dark { "floor" } else { "floor_light" };
        if let Some((tile, ox, oy)) = tex(name, &[], 96.0, 48.0) {
            let band = if dark { (36, 34, 58) } else { (211, 201, 246) };
            if floor_y + 48 < h { draw::rect(bs, band, Rect::new(0, floor_y + 48, w, h - floor_y - 48), 0, 0); }
            let mut x = 0;
            while x < w { bs.blit_f(&tile, x as f64 + ox, floor_y as f64 + oy, 1.0); x += 96; }
            return;
        }
    }
    let scroll = 0.0f64;
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

/// Fahrtstreifen rechts neben einem Bodenhindernis: zeigt, dass es selbst fährt (Boden steht still).
/// Drei Balken im Pixel-Stil, flackern leicht.
pub fn draw_speed_streaks(bs: &Surface, r: Rect, base: Col) {
    let frame = ((anim_time() * 14.0) as i64 + (r.x as i64 / 7)).rem_euclid(3) as i32;
    let spr = cached(format!("streak{}x{}:{:?}:{}", r.w, r.h, base, frame), || {
        let (w, h) = (r.w * 3 / 4 + 8, r.h);
        let s = Surface::new_alpha(w, h);
        let c = lighten(base, 0.35);
        let u = (r.h / 12).max(2);
        let lens = [[0.55, 0.85, 0.4], [0.7, 0.75, 0.5], [0.5, 1.0, 0.35]][frame as usize];
        for (i, (yf, a)) in [(0.22, 150), (0.5, 110), (0.76, 70)].iter().enumerate() {
            let len = ((w - 4) as f64 * lens[i]) as i32;
            draw::rect(&s, with_a(c, *a), Rect::new(4, (h as f64 * yf) as i32 - u / 2, len, u), 0, 0);
        }
        // Staubkrümel
        draw::rect(&s, with_a((170, 160, 210), 120), Rect::new(2 + frame * 3, h - u * 2, u, u), 0, 0);
        draw::rect(&s, with_a((170, 160, 210), 80), Rect::new(8 + frame * 4, h - u * 3, u, u), 0, 0);
        s
    });
    bs.blit(&spr, (r.right() + 2, r.y));
}

/// Cosmetics der Spielfigur als Texturen. Reihenfolge: Kopf, dann Gesicht (Brille liegt über dem Stirnband).
pub fn draw_cosmetics_tex(bs: &Surface, r: Rect, head: &str, eyes: &str, nose: &str, mouth: &str, ticks: i64) -> bool {
    if legacy() { return false; }
    let none: Vec<(&str, String)> = vec![];
    if !head.is_empty() {
        put(bs, &format!("cos_{head}"), &none, r, 1.0);
        if head == "propeller_hat" {
            // Flügel drehen: Breite folgt |cos|, in 12 Stufen (sonst zu viele Zwischenbilder)
            let a = (ticks as f64 / 60.0).to_radians() * 6.0;
            let spin = ((a.cos().abs() * 12.0).round() / 12.0).max(0.08);
            put(bs, "cos_propeller_blade", &[("spin", format!("{spin:.3}"))], r, 1.0);
        }
    }
    for c in [eyes, nose, mouth] { if !c.is_empty() { put(bs, &format!("cos_{c}"), &none, r, 1.0); } }
    true
}

/// Symbol eines Cosmetics/einer Spur (Shop, Inventar). false = keine Textur.
pub fn draw_icon_tex(bs: &Surface, key: &str, cx: i32, cy: i32, big: bool) -> bool {
    if legacy() { return false; }
    let name = if key.ends_with("_trail") { format!("trail_{key}") } else { format!("cos_{key}") };
    let size = if big { 40.0 } else { 24.0 };
    let k = surface::render_scale();
    let ck = format!("icon:{name}:{size}@{k:.3}");
    let spr = TEX.with(|c| {
        let mut c = c.borrow_mut();
        c.entry(ck).or_insert_with(|| {
            let vars = vec![("hue", "#ff5ac8".to_string()), ("spin", "1".to_string())];
            jnd_tex::render_icon(&name, &vars, size as f32, k).map(|pm| (Surface::from_pixmap_k(pm, k), 0.0, 0.0))
        }).clone()
    });
    match spr {
        Some((s, _, _)) => { bs.blit_f(&s, cx as f64 - size / 2.0, cy as f64 - size / 2.0, 1.0); true }
        None => false,
    }
}

/// Ein Spur-Partikel als Textur. `size` = Kantenlänge, `alpha` 0..1, `hue` nur für die Regenbogenspur.
pub fn draw_trail_tex(bs: &Surface, trail: &str, x: f64, y: f64, size: f64, alpha: f32, hue: Option<Col>) -> bool {
    if legacy() { return false; }
    let sz = size.round().max(3.0);
    let vars = match hue { Some(c) => vec![("hue", jnd_tex::hex(rgb8(c)))], None => vec![] };
    match tex(&format!("trail_{trail}"), &vars, sz, sz) {
        Some((s, ox, oy)) => { bs.blit_f(&s, (x - sz / 2.0).round() + ox, (y - sz / 2.0).round() + oy, alpha); true }
        None => false,
    }
}

/// Squash & Stretch beim Springen/Landen - nur Optik, die Hitbox bleibt
pub fn squash(r: Rect, vy: f64, land_ms: f64) -> Rect {
    if legacy() { return r; } // Python kannte kein Squash & Stretch
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

// =====================================================================
// Effekte: Partikel (Treffer, 1-UP, Landen) - rein optisch
// =====================================================================
#[derive(Clone)]
struct Part { x: f64, y: f64, vx: f64, vy: f64, life: f64, max: f64, col: Col, size: f64, grav: f64 }

#[derive(Default)]
pub struct Fx { parts: Vec<Part> }

impl Fx {
    pub fn new() -> Fx { Fx { parts: Vec::new() } }
    pub fn clear(&mut self) { self.parts.clear(); }
    fn push(&mut self, p: Part) { if self.parts.len() < 600 { self.parts.push(p); } }

    /// Splitter in alle Richtungen (Treffer)
    pub fn burst(&mut self, x: f64, y: f64, col: Col, n: usize) {
        use crate::util::uniform;
        for _ in 0..n {
            let a = uniform(0.0, std::f64::consts::TAU);
            let sp = uniform(2.0, 7.5);
            let life = uniform(350.0, 700.0);
            let c = if uniform(0.0, 1.0) < 0.35 { lighten(col, 0.45) } else { col };
            self.push(Part { x, y, vx: a.cos() * sp, vy: a.sin() * sp - 2.0, life, max: life, col: c, size: uniform(2.5, 5.0), grav: 0.25 });
        }
    }
    /// Funkeln nach oben (1-UP)
    pub fn sparkle(&mut self, x: f64, y: f64, col: Col, n: usize) {
        use crate::util::uniform;
        for _ in 0..n {
            let life = uniform(450.0, 900.0);
            self.push(Part { x: x + uniform(-18.0, 18.0), y: y + uniform(-10.0, 10.0), vx: uniform(-1.2, 1.2), vy: uniform(-4.0, -1.5), life, max: life, col: lighten(col, uniform(0.0, 0.5)), size: uniform(2.0, 4.0), grav: -0.02 });
        }
    }
    /// Staub beim Landen
    pub fn dust(&mut self, x: f64, y: f64, dark: bool) {
        use crate::util::uniform;
        let col = if dark { (150, 140, 190) } else { (170, 165, 200) };
        for i in 0..10 {
            let dir = if i % 2 == 0 { -1.0 } else { 1.0 };
            let life = uniform(220.0, 420.0);
            self.push(Part { x: x + dir * uniform(4.0, 20.0), y, vx: dir * uniform(1.0, 3.2), vy: uniform(-1.6, -0.3), life, max: life, col, size: uniform(2.0, 3.5), grav: 0.05 });
        }
    }
    /// Ein Logikschritt (dt in ms, Bewegung pro 1/60 s)
    pub fn update(&mut self, dt: f64) {
        let k = dt / (1000.0 / 60.0);
        for p in self.parts.iter_mut() {
            p.x += p.vx * k;
            p.y += p.vy * k;
            p.vy += p.grav * k;
            p.vx *= 0.985f64.powf(k);
            p.life -= dt;
        }
        self.parts.retain(|p| p.life > 0.0);
    }
    pub fn draw(&self, bs: &Surface) {
        for p in &self.parts {
            let f = (p.life / p.max).clamp(0.0, 1.0);
            let r = (p.size * (0.4 + 0.6 * f)) as f32;
            if r < 0.6 { continue; }
            draw::aacircle(bs, p.col, (p.x, p.y), r);
        }
    }
    pub fn is_empty(&self) -> bool { self.parts.is_empty() }
}
