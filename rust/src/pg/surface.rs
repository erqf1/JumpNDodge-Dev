//! pygame.Surface, pygame.draw und pygame.transform auf Basis von tiny-skia.
//!
//! Surfaces sind wie in Python Referenzen (Rc<RefCell<..>>): `let s2 = s.clone()` zeigt auf
//! dieselbe Fläche, `s.copy()` macht eine echte Kopie. Gezeichnet wird ohne Kantenglättung
//! wie bei pygame.draw. Auf Flächen ohne SRCALPHA wird der Alphawert einer Farbe ignoriert.

use super::rect::{Num, Rect};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tiny_skia::{
    BlendMode, FillRule, FilterQuality, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

fn cl(v: i32) -> u8 { v.clamp(0, 255) as u8 }
fn clf(v: f64) -> u8 { (v as i32).clamp(0, 255) as u8 }

impl From<(i32, i32, i32)> for Color { fn from(t: (i32, i32, i32)) -> Color { Color(cl(t.0), cl(t.1), cl(t.2), 255) } }
impl From<(i32, i32, i32, i32)> for Color { fn from(t: (i32, i32, i32, i32)) -> Color { Color(cl(t.0), cl(t.1), cl(t.2), cl(t.3)) } }
impl From<(u8, u8, u8)> for Color { fn from(t: (u8, u8, u8)) -> Color { Color(t.0, t.1, t.2, 255) } }
impl From<(u8, u8, u8, u8)> for Color { fn from(t: (u8, u8, u8, u8)) -> Color { Color(t.0, t.1, t.2, t.3) } }
impl From<(f64, f64, f64)> for Color { fn from(t: (f64, f64, f64)) -> Color { Color(clf(t.0), clf(t.1), clf(t.2), 255) } }
impl From<(f64, f64, f64, f64)> for Color { fn from(t: (f64, f64, f64, f64)) -> Color { Color(clf(t.0), clf(t.1), clf(t.2), clf(t.3)) } }
impl From<[i32; 3]> for Color { fn from(t: [i32; 3]) -> Color { Color(cl(t[0]), cl(t[1]), cl(t[2]), 255) } }
impl From<[i32; 4]> for Color { fn from(t: [i32; 4]) -> Color { Color(cl(t[0]), cl(t[1]), cl(t[2]), cl(t[3])) } }
impl From<&Color> for Color { fn from(c: &Color) -> Color { *c } }

impl Color {
    pub fn rgb(&self) -> (i32, i32, i32) { (self.0 as i32, self.1 as i32, self.2 as i32) }
    pub fn rgba(&self) -> (i32, i32, i32, i32) { (self.0 as i32, self.1 as i32, self.2 as i32, self.3 as i32) }
    pub fn with_alpha(&self, a: i32) -> Color { Color(self.0, self.1, self.2, cl(a)) }
    pub fn arr(&self) -> [i32; 3] { [self.0 as i32, self.1 as i32, self.2 as i32] }
    fn skia(&self) -> tiny_skia::Color { tiny_skia::Color::from_rgba8(self.0, self.1, self.2, self.3) }
}

// =============================
// Render-Skalierung (scharfe Darstellung bei Windows-Skalierung / Vollbild)
// =============================
// pygame zeichnete immer in "logischen" Pixeln und Windows hat das Bild dann unscharf
// hochgezogen. Hier hat jede Fläche einen Faktor k (echte Pixel pro logischem Pixel).
// Der Spielcode rechnet weiter in logischen Koordinaten, gezeichnet wird in voller Auflösung.
thread_local! {
    static RENDER_SCALE: Cell<f32> = const { Cell::new(1.0) };
}
/// Faktor für neu angelegte Flächen setzen (1.0 = wie pygame)
pub fn set_render_scale(k: f32) { RENDER_SCALE.with(|c| c.set(if k.is_finite() && k > 0.05 { k } else { 1.0 })); }
pub fn render_scale() -> f32 { RENDER_SCALE.with(|c| c.get()) }

#[inline]
fn px(v: i32, k: f32) -> i32 { if k == 1.0 { v } else { (v as f32 * k).round() as i32 } }
/// logisches Rechteck -> Pixelrechteck
fn prect(r: Rect, k: f32) -> Rect {
    if k == 1.0 { return r; }
    let x0 = px(r.x, k); let y0 = px(r.y, k);
    Rect::new(x0, y0, px(r.x + r.w, k) - x0, px(r.y + r.h, k) - y0)
}
fn tf(k: f32) -> Transform { if k == 1.0 { Transform::identity() } else { Transform::from_scale(k, k) } }

/// Positionen akzeptieren wie in pygame ints und floats.
pub trait Pos { fn xy(&self) -> (f64, f64); }
impl<A: Num, B: Num> Pos for (A, B) { fn xy(&self) -> (f64, f64) { (self.0.to_f(), self.1.to_f()) } }

pub struct SurfInner {
    pub pm: Pixmap,
    pub w: i32,
    pub h: i32,
    pub srcalpha: bool,
    pub alpha: Option<u8>,
    pub clip: Option<Rect>,
    mask: Option<Mask>,
    /// echte Pixel pro logischem Pixel
    pub k: f32,
}

#[derive(Clone)]
pub struct Surface(pub Rc<RefCell<SurfInner>>);

impl Surface {
    fn make(w: i32, h: i32, srcalpha: bool) -> Surface { Surface::make_k(w, h, srcalpha, render_scale()) }
    fn make_k(w: i32, h: i32, srcalpha: bool, k: f32) -> Surface {
        let pw = px(w.max(1), k).max(1) as u32;
        let ph = px(h.max(1), k).max(1) as u32;
        let mut pm = Pixmap::new(pw, ph).unwrap_or_else(|| Pixmap::new(1, 1).unwrap());
        if !srcalpha {
            pm.fill(tiny_skia::Color::BLACK);
        }
        Surface(Rc::new(RefCell::new(SurfInner { pm, w: w.max(0), h: h.max(0), srcalpha, alpha: None, clip: None, mask: None, k })))
    }
    /// Fläche mit fester Pixelgröße (z.B. genau Fenstergröße) und logischer Größe w x h
    pub fn new_scaled(w: i32, h: i32, k: f32) -> Surface { Surface::make_k(w, h, false, k) }
    pub fn scale_k(&self) -> f32 { self.0.borrow().k }
    /// pygame.Surface((w, h)) - undurchsichtig, schwarz
    pub fn new<A: Num, B: Num>(w: A, h: B) -> Surface { Surface::make(w.to_i(), h.to_i(), false) }
    /// pygame.Surface((w, h), pygame.SRCALPHA) - durchsichtig
    pub fn new_alpha<A: Num, B: Num>(w: A, h: B) -> Surface { Surface::make(w.to_i(), h.to_i(), true) }
    pub fn from_pixmap(pm: Pixmap, srcalpha: bool) -> Surface {
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        Surface(Rc::new(RefCell::new(SurfInner { pm, w, h, srcalpha, alpha: None, clip: None, mask: None, k: 1.0 })))
    }

    /// Fertig gerenderte Pixmap mit Faktor k (echte Pixel pro logischem Pixel), z.B. eine Textur
    pub fn from_pixmap_k(pm: Pixmap, k: f32) -> Surface {
        let w = (pm.width() as f32 / k).ceil() as i32;
        let h = (pm.height() as f32 / k).ceil() as i32;
        Surface(Rc::new(RefCell::new(SurfInner { pm, w, h, srcalpha: true, alpha: None, clip: None, mask: None, k })))
    }

    /// Blit an einer Kommaposition (auf echte Pixel gerundet) mit eigener Deckkraft - für Texturen
    pub fn blit_f(&self, src: &Surface, x: f64, y: f64, opacity: f32) {
        if opacity <= 0.0 || Rc::ptr_eq(&self.0, &src.0) { return; }
        let sb = src.0.borrow();
        let mut d = self.0.borrow_mut();
        let k = d.k;
        let r = k / sb.k;
        let (tx, ty) = ((x as f32 * k).round(), (y as f32 * k).round());
        let quality = if (r - 1.0).abs() < 1e-4 { FilterQuality::Nearest } else { FilterQuality::Bilinear };
        let paint = PixmapPaint { opacity: opacity.min(1.0) * sb.alpha.map(|a| a as f32 / 255.0).unwrap_or(1.0), blend_mode: BlendMode::SourceOver, quality };
        let mask = d.mask.take();
        d.pm.draw_pixmap(0, 0, sb.pm.as_ref(), &paint, Transform::from_row(r, 0.0, 0.0, r, tx, ty), mask.as_ref());
        d.mask = mask;
    }

    pub fn get_width(&self) -> i32 { self.0.borrow().w }
    pub fn get_height(&self) -> i32 { self.0.borrow().h }
    pub fn get_size(&self) -> (i32, i32) { let s = self.0.borrow(); (s.w, s.h) }
    pub fn get_rect(&self) -> Rect { let s = self.0.borrow(); Rect::new(0, 0, s.w, s.h) }
    pub fn is_srcalpha(&self) -> bool { self.0.borrow().srcalpha }
    pub fn same(&self, o: &Surface) -> bool { Rc::ptr_eq(&self.0, &o.0) }

    pub fn set_alpha(&self, a: Option<i32>) { self.0.borrow_mut().alpha = a.map(|v| v.clamp(0, 255) as u8); }
    pub fn get_alpha(&self) -> Option<i32> { self.0.borrow().alpha.map(|v| v as i32) }

    pub fn copy(&self) -> Surface {
        let s = self.0.borrow();
        Surface(Rc::new(RefCell::new(SurfInner {
            pm: s.pm.clone(), w: s.w, h: s.h, srcalpha: s.srcalpha, alpha: s.alpha, clip: None, mask: None, k: s.k,
        })))
    }

    /// Wie pygame `surface.convert_alpha()` - hier nur eine Kopie mit Alphakanal.
    pub fn convert_alpha(&self) -> Surface { let c = self.copy(); c.0.borrow_mut().srcalpha = true; c }

    pub fn set_clip(&self, r: Option<Rect>) {
        let mut s = self.0.borrow_mut();
        s.clip = r;
        s.mask = None;
        if let Some(r) = r {
            let r = prect(r.clip(&Rect::new(0, 0, s.w, s.h)), s.k);
            let mut m = Mask::new(s.pm.width(), s.pm.height()).unwrap();
            if r.w > 0 && r.h > 0 {
                if let Some(rr) = tiny_skia::Rect::from_xywh(r.x as f32, r.y as f32, r.w as f32, r.h as f32) {
                    let p = PathBuilder::from_rect(rr);
                    m.fill_path(&p, FillRule::Winding, false, Transform::identity());
                }
            }
            s.mask = Some(m);
        }
    }
    pub fn get_clip(&self) -> Rect {
        let s = self.0.borrow();
        s.clip.unwrap_or(Rect::new(0, 0, s.w, s.h))
    }

    pub fn fill<C: Into<Color>>(&self, c: C) {
        let mut c = c.into();
        let mut s = self.0.borrow_mut();
        if !s.srcalpha { c.3 = 255; }
        if let Some(clip) = s.clip {
            drop(s);
            self.fill_rect(c, clip);
            return;
        }
        s.pm.fill(c.skia());
    }
    /// surface.fill(color, rect) - ersetzt die Pixel (kein Mischen), wie pygame.
    pub fn fill_rect<C: Into<Color>>(&self, c: C, r: Rect) {
        let mut c = c.into();
        let mut s = self.0.borrow_mut();
        if !s.srcalpha { c.3 = 255; }
        let mut r = r.normalized().clip(&Rect::new(0, 0, s.w, s.h));
        if let Some(clip) = s.clip { r = r.clip(&clip); }
        if r.w <= 0 || r.h <= 0 { return; }
        let r = prect(r, s.k).clip(&Rect::new(0, 0, s.pm.width() as i32, s.pm.height() as i32));
        let pw = s.pm.width() as usize;
        let pc = c.skia().premultiply().to_color_u8();
        let data = s.pm.pixels_mut();
        for y in r.y..r.y + r.h {
            let row = y as usize * pw;
            for x in r.x..r.x + r.w {
                data[row + x as usize] = pc;
            }
        }
    }

    pub fn get_at<P: Pos>(&self, p: P) -> Color {
        let (x, y) = p.xy();
        let s = self.0.borrow();
        if x < 0.0 || y < 0.0 { return Color(0, 0, 0, 0); }
        let (x, y) = if s.k == 1.0 { (x, y) } else { ((x.floor() + 0.5) * s.k as f64, (y.floor() + 0.5) * s.k as f64) };
        match s.pm.pixel(x as u32, y as u32) {
            Some(px) => { let c = px.demultiply(); Color(c.red(), c.green(), c.blue(), c.alpha()) }
            None => Color(0, 0, 0, 0),
        }
    }

    /// surface.blit(src, pos)
    pub fn blit<P: Pos>(&self, src: &Surface, pos: P) { self.blit_area(src, pos, None); }

    /// surface.blit(src, pos, area)
    pub fn blit_area<P: Pos>(&self, src: &Surface, pos: P, area: Option<Rect>) {
        let (x, y) = pos.xy();
        let (x, y) = (x as i32, y as i32);
        if Rc::ptr_eq(&self.0, &src.0) {
            let c = src.copy();
            self.blit_area(&c, (x, y), area);
            return;
        }
        let sb = src.0.borrow();
        if sb.w <= 0 || sb.h <= 0 { return; }
        let opacity = sb.alpha.map(|a| a as f32 / 255.0).unwrap_or(1.0);
        if opacity <= 0.0 { return; }
        let mut d = self.0.borrow_mut();
        let same_k = (d.k - sb.k).abs() < 1e-4;
        let quality = if same_k { FilterQuality::Nearest } else { FilterQuality::Bilinear };
        let paint = PixmapPaint { opacity, blend_mode: BlendMode::SourceOver, quality };
        let src_pm_owned;
        let (src_pm, ax, ay) = if let Some(a) = area {
            let a = a.clip(&Rect::new(0, 0, sb.w, sb.h));
            if a.w <= 0 || a.h <= 0 { return; }
            let pa = prect(a, sb.k);
            if pa.w <= 0 || pa.h <= 0 { return; }
            let mut sub = Pixmap::new(pa.w as u32, pa.h as u32).unwrap();
            sub.draw_pixmap(-pa.x, -pa.y, sb.pm.as_ref(), &PixmapPaint { blend_mode: BlendMode::Source, ..Default::default() }, Transform::identity(), None);
            src_pm_owned = sub;
            (&src_pm_owned, 0, 0)
        } else {
            (&sb.pm, 0, 0)
        };
        let _ = (ax, ay);
        let mask = d.mask.take();
        if same_k {
            let k = d.k;
            d.pm.draw_pixmap(px(x, k), px(y, k), src_pm.as_ref(), &paint, Transform::identity(), mask.as_ref());
        } else {
            let r = d.k / sb.k;
            let t = Transform::from_row(r, 0.0, 0.0, r, px(x, d.k) as f32, px(y, d.k) as f32);
            d.pm.draw_pixmap(0, 0, src_pm.as_ref(), &paint, t, mask.as_ref());
        }
        d.mask = mask;
    }

    /// Ganzes Pixmap (für Ausgabe/Screenshots)
    pub fn with_pixmap<R>(&self, f: impl FnOnce(&Pixmap) -> R) -> R { f(&self.0.borrow().pm) }

    pub fn save_png(&self, path: &str) -> bool { self.0.borrow().pm.save_png(path).is_ok() }

    pub fn subsurface(&self, r: Rect) -> Surface {
        // pygame teilt die Pixel - hier reicht eine Kopie (wird nur gelesen)
        let s = self.0.borrow();
        let r = r.clip(&Rect::new(0, 0, s.w, s.h));
        let out = Surface::make_k(r.w, r.h, s.srcalpha, s.k);
        let r = prect(r, s.k);
        if r.w > 0 && r.h > 0 {
            out.0.borrow_mut().pm.draw_pixmap(-r.x, -r.y, s.pm.as_ref(), &PixmapPaint { blend_mode: BlendMode::Source, ..Default::default() }, Transform::identity(), None);
        }
        out
    }
}

// =============================
// pygame.draw
// =============================

fn paint_for(dst: &SurfInner, c: Color) -> Paint<'static> {
    let mut c = c;
    if !dst.srcalpha { c.3 = 255; }
    let mut p = Paint::default();
    p.set_color(c.skia());
    // pygame zeichnet ohne Kantenglättung; in hoher Auflösung sieht geglättet besser aus
    p.anti_alias = dst.k > 1.01;
    p.blend_mode = if dst.srcalpha && c.3 < 255 { BlendMode::Source } else { BlendMode::SourceOver };
    p
}

fn aa_paint_for(dst: &SurfInner, c: Color) -> Paint<'static> {
    let mut p = paint_for(dst, c);
    p.anti_alias = true;
    p.blend_mode = BlendMode::SourceOver;
    p
}

fn rrect_path(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, radii: [f32; 4]) {
    // radii: tl, tr, br, bl
    let [tl, tr, br, bl] = radii;
    const K: f32 = 0.552_284_8;
    pb.move_to(x + tl, y);
    pb.line_to(x + w - tr, y);
    if tr > 0.0 { pb.cubic_to(x + w - tr + tr * K, y, x + w, y + tr - tr * K, x + w, y + tr); }
    pb.line_to(x + w, y + h - br);
    if br > 0.0 { pb.cubic_to(x + w, y + h - br + br * K, x + w - br + br * K, y + h, x + w - br, y + h); }
    pb.line_to(x + bl, y + h);
    if bl > 0.0 { pb.cubic_to(x + bl - bl * K, y + h, x, y + h - bl + bl * K, x, y + h - bl); }
    pb.line_to(x, y + tl);
    if tl > 0.0 { pb.cubic_to(x, y + tl - tl * K, x + tl - tl * K, y, x + tl, y); }
    pb.close();
}

/// Optionen für draw.rect (pygame: width, border_radius, border_top_left_radius, ...)
#[derive(Clone, Copy, Default)]
pub struct RectOpts {
    pub width: i32,
    pub radius: i32,
    pub tl: i32,
    pub tr: i32,
    pub bl: i32,
    pub br: i32,
}

pub mod draw {
    use super::*;

    fn fill_path_on(s: &mut SurfInner, path: &tiny_skia::Path, paint: &Paint, rule: FillRule) {
        let mask = s.mask.take();
        let t = tf(s.k);
        s.pm.fill_path(path, paint, rule, t, mask.as_ref());
        s.mask = mask;
    }
    fn stroke_path_on(s: &mut SurfInner, path: &tiny_skia::Path, paint: &Paint, stroke: &Stroke) {
        let mask = s.mask.take();
        let t = tf(s.k);
        s.pm.stroke_path(path, paint, stroke, t, mask.as_ref());
        s.mask = mask;
    }

    /// pygame.draw.rect(surface, color, rect, width=0, border_radius=0)
    pub fn rect<C: Into<Color>>(surf: &Surface, color: C, r: Rect, width: i32, radius: i32) -> Rect {
        rect_ex(surf, color, r, RectOpts { width, radius, tl: -1, tr: -1, bl: -1, br: -1 })
    }

    /// Volle Variante mit Eckradien einzeln (-1 = border_radius verwenden)
    pub fn rect_ex<C: Into<Color>>(surf: &Surface, color: C, r: Rect, o: RectOpts) -> Rect {
        let c = color.into();
        let r = r.normalized();
        if r.w <= 0 || r.h <= 0 || o.width < 0 { return r; }
        let mut s = surf.0.borrow_mut();
        let paint = paint_for(&s, c);
        let maxr = r.w.min(r.h) / 2;
        let pick = |v: i32| -> f32 { let v = if v < 0 { o.radius } else { v }; v.clamp(0, maxr) as f32 };
        let radii = [pick(o.tl), pick(o.tr), pick(o.br), pick(o.bl)];
        let rounded = radii.iter().any(|&v| v > 0.0);
        let (x, y, w, h) = (r.x as f32, r.y as f32, r.w as f32, r.h as f32);
        let width = o.width;
        if !rounded {
            drop(s);
            if width == 0 || width * 2 >= r.w.min(r.h) {
                fill_rect_blend(surf, c, r);
            } else {
                // Rahmen innen: oben, unten, links, rechts (wie pygame)
                fill_rect_blend(surf, c, Rect::new(r.x, r.y, r.w, width));
                fill_rect_blend(surf, c, Rect::new(r.x, r.bottom() - width, r.w, width));
                fill_rect_blend(surf, c, Rect::new(r.x, r.y + width, width, r.h - 2 * width));
                fill_rect_blend(surf, c, Rect::new(r.right() - width, r.y + width, width, r.h - 2 * width));
            }
            return r;
        }
        let mut pb = PathBuilder::new();
        if rounded {
            rrect_path(&mut pb, x, y, w, h, radii);
        } else {
            pb.push_rect(tiny_skia::Rect::from_xywh(x, y, w, h).unwrap());
        }
        if width > 0 && width * 2 < r.w.min(r.h) {
            let wf = width as f32;
            let inner = [ (radii[0] - wf).max(0.0), (radii[1] - wf).max(0.0), (radii[2] - wf).max(0.0), (radii[3] - wf).max(0.0) ];
            if rounded {
                rrect_path(&mut pb, x + wf, y + wf, w - 2.0 * wf, h - 2.0 * wf, inner);
            } else {
                pb.push_rect(tiny_skia::Rect::from_xywh(x + wf, y + wf, w - 2.0 * wf, h - 2.0 * wf).unwrap());
            }
            if let Some(p) = pb.finish() { fill_path_on(&mut s, &p, &paint, FillRule::EvenOdd); }
        } else if let Some(p) = pb.finish() {
            fill_path_on(&mut s, &p, &paint, FillRule::Winding);
        }
        r
    }

    fn fill_rect_blend(surf: &Surface, c: Color, r: Rect) {
        let mut s = surf.0.borrow_mut();
        let mut paint = paint_for(&s, c);
        paint.anti_alias = false;
        let r = prect(r, s.k);
        if let Some(rr) = tiny_skia::Rect::from_xywh(r.x as f32, r.y as f32, r.w as f32, r.h as f32) {
            let mask = s.mask.take();
            s.pm.fill_rect(rr, &paint, Transform::identity(), mask.as_ref());
            s.mask = mask;
        }
    }

    /// pygame.draw.circle(surface, color, center, radius, width=0)
    pub fn circle<C: Into<Color>, P: Pos, R: Num>(surf: &Surface, color: C, center: P, radius: R, width: i32) -> Rect {
        let c = color.into();
        let (cx, cy) = center.xy();
        let (cx, cy) = (cx as i32 as f32, cy as i32 as f32);
        let rad = radius.to_i() as f32;
        if rad < 1.0 { return Rect::new(cx as i32, cy as i32, 0, 0); }
        let mut s = surf.0.borrow_mut();
        let paint = paint_for(&s, c);
        let mut pb = PathBuilder::new();
        // pygame-Kreise sind um den Pixelmittelpunkt zentriert
        let (pcx, pcy) = (cx + 0.5, cy + 0.5);
        pb.push_circle(pcx, pcy, rad);
        if width > 0 && (width as f32) < rad {
            pb.push_circle(pcx, pcy, rad - width as f32);
            if let Some(p) = pb.finish() { fill_path_on(&mut s, &p, &paint, FillRule::EvenOdd); }
        } else if let Some(p) = pb.finish() {
            fill_path_on(&mut s, &p, &paint, FillRule::Winding);
        }
        Rect::new((cx - rad) as i32, (cy - rad) as i32, (rad * 2.0) as i32, (rad * 2.0) as i32)
    }

    /// Abgerundetes Rechteck mit senkrechtem Farbverlauf (oben -> unten), geglättet.
    /// Auf Flächen mit Alphakanal werden die Alphawerte der Farben beachtet.
    pub fn rect_vgrad<C: Into<Color>, D: Into<Color>>(surf: &Surface, r: Rect, radius: i32, top: C, bottom: D) {
        let (top, bottom) = (top.into(), bottom.into());
        let r = r.normalized();
        if r.w <= 0 || r.h <= 0 { return; }
        let mut s = surf.0.borrow_mut();
        let fix = |c: Color| if s.srcalpha { c } else { Color(c.0, c.1, c.2, 255) };
        let (top, bottom) = (fix(top), fix(bottom));
        let (x, y, w, h) = (r.x as f32, r.y as f32, r.w as f32, r.h as f32);
        let Some(shader) = tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(0.0, y), tiny_skia::Point::from_xy(0.0, y + h),
            vec![tiny_skia::GradientStop::new(0.0, top.skia()), tiny_skia::GradientStop::new(1.0, bottom.skia())],
            tiny_skia::SpreadMode::Pad, Transform::identity()) else { return; };
        let mut p = Paint::default();
        p.shader = shader;
        p.anti_alias = true;
        let rad = (radius.max(0) as f32).min(w.min(h) / 2.0);
        let mut pb = PathBuilder::new();
        rrect_path(&mut pb, x, y, w, h, [rad; 4]);
        if let Some(path) = pb.finish() { fill_path_on(&mut s, &path, &p, FillRule::Winding); }
    }

    /// Weicher Kreis mit radialem Verlauf (innen -> außen transparent), z.B. für Glühen
    pub fn glow<C: Into<Color>, P: Pos>(surf: &Surface, color: C, center: P, radius: f32) {
        let c = color.into();
        let (cx, cy) = center.xy();
        if radius <= 0.5 { return; }
        let mut s = surf.0.borrow_mut();
        let Some(shader) = tiny_skia::RadialGradient::new(
            tiny_skia::Point::from_xy(cx as f32, cy as f32), tiny_skia::Point::from_xy(cx as f32, cy as f32), radius,
            vec![tiny_skia::GradientStop::new(0.0, c.skia()), tiny_skia::GradientStop::new(1.0, Color(c.0, c.1, c.2, 0).skia())],
            tiny_skia::SpreadMode::Pad, Transform::identity()) else { return; };
        let mut p = Paint::default();
        p.shader = shader;
        p.anti_alias = true;
        let mut pb = PathBuilder::new();
        pb.push_circle(cx as f32, cy as f32, radius);
        if let Some(path) = pb.finish() { fill_path_on(&mut s, &path, &p, FillRule::Winding); }
    }

    /// Kreis mit weichen Kanten (für Stellen, die pygame.gfxdraw.aacircle o.ä. nachbilden)
    pub fn aacircle<C: Into<Color>, P: Pos>(surf: &Surface, color: C, center: P, radius: f32) {
        let c = color.into();
        let (cx, cy) = center.xy();
        if radius <= 0.0 { return; }
        let mut s = surf.0.borrow_mut();
        let paint = aa_paint_for(&s, c);
        let mut pb = PathBuilder::new();
        pb.push_circle(cx as f32, cy as f32, radius);
        if let Some(p) = pb.finish() { fill_path_on(&mut s, &p, &paint, FillRule::Winding); }
    }

    /// pygame.draw.ellipse(surface, color, rect, width=0)
    pub fn ellipse<C: Into<Color>>(surf: &Surface, color: C, r: Rect, width: i32) -> Rect {
        let c = color.into();
        let r = r.normalized();
        if r.w <= 0 || r.h <= 0 { return r; }
        let mut s = surf.0.borrow_mut();
        let paint = paint_for(&s, c);
        let mut pb = PathBuilder::new();
        if let Some(rr) = tiny_skia::Rect::from_xywh(r.x as f32, r.y as f32, r.w as f32, r.h as f32) {
            pb.push_oval(rr);
        }
        let wf = width as f32;
        if width > 0 && width * 2 < r.w.min(r.h) {
            if let Some(rr) = tiny_skia::Rect::from_xywh(r.x as f32 + wf, r.y as f32 + wf, r.w as f32 - 2.0 * wf, r.h as f32 - 2.0 * wf) {
                pb.push_oval(rr);
            }
            if let Some(p) = pb.finish() { fill_path_on(&mut s, &p, &paint, FillRule::EvenOdd); }
        } else if let Some(p) = pb.finish() {
            fill_path_on(&mut s, &p, &paint, FillRule::Winding);
        }
        r
    }

    /// pygame.draw.line(surface, color, start, end, width=1)
    pub fn line<C: Into<Color>, P: Pos, Q: Pos>(surf: &Surface, color: C, start: P, end: Q, width: i32) -> Rect {
        let c = color.into();
        if width < 1 { return Rect::default(); }
        let (x1, y1) = start.xy();
        let (x2, y2) = end.xy();
        let (x1, y1, x2, y2) = (x1 as i32 as f32 + 0.5, y1 as i32 as f32 + 0.5, x2 as i32 as f32 + 0.5, y2 as i32 as f32 + 0.5);
        let mut s = surf.0.borrow_mut();
        let paint = paint_for(&s, c);
        let mut pb = PathBuilder::new();
        pb.move_to(x1, y1);
        pb.line_to(x2, y2);
        if let Some(p) = pb.finish() {
            let st = Stroke { width: width as f32, line_cap: tiny_skia::LineCap::Square, ..Default::default() };
            let st = if width == 1 { Stroke { width: 1.0, line_cap: tiny_skia::LineCap::Square, ..Default::default() } } else { st };
            stroke_path_on(&mut s, &p, &paint, &st);
        }
        Rect::new(x1.min(x2) as i32, y1.min(y2) as i32, (x1 - x2).abs() as i32 + 1, (y1 - y2).abs() as i32 + 1)
    }

    /// pygame.draw.aaline - weiche Linie
    pub fn aaline<C: Into<Color>, P: Pos, Q: Pos>(surf: &Surface, color: C, start: P, end: Q) {
        let c = color.into();
        let (x1, y1) = start.xy();
        let (x2, y2) = end.xy();
        let mut s = surf.0.borrow_mut();
        let paint = aa_paint_for(&s, c);
        let mut pb = PathBuilder::new();
        pb.move_to(x1 as f32 + 0.5, y1 as f32 + 0.5);
        pb.line_to(x2 as f32 + 0.5, y2 as f32 + 0.5);
        if let Some(p) = pb.finish() {
            stroke_path_on(&mut s, &p, &paint, &Stroke { width: 1.0, ..Default::default() });
        }
    }

    /// pygame.draw.lines(surface, color, closed, points, width=1)
    pub fn lines<C: Into<Color>, P: Pos>(surf: &Surface, color: C, closed: bool, points: &[P], width: i32) {
        let c = color.into();
        if points.len() < 2 || width < 1 { return; }
        let mut s = surf.0.borrow_mut();
        let paint = paint_for(&s, c);
        let mut pb = PathBuilder::new();
        for (i, p) in points.iter().enumerate() {
            let (x, y) = p.xy();
            let (x, y) = (x as i32 as f32 + 0.5, y as i32 as f32 + 0.5);
            if i == 0 { pb.move_to(x, y); } else { pb.line_to(x, y); }
        }
        if closed { pb.close(); }
        if let Some(p) = pb.finish() {
            let st = Stroke { width: width as f32, line_cap: tiny_skia::LineCap::Square, line_join: tiny_skia::LineJoin::Miter, ..Default::default() };
            stroke_path_on(&mut s, &p, &paint, &st);
        }
    }

    /// pygame.draw.polygon(surface, color, points, width=0)
    pub fn polygon<C: Into<Color>, P: Pos>(surf: &Surface, color: C, points: &[P], width: i32) {
        let c = color.into();
        if points.len() < 3 {
            if points.len() == 2 { line(surf, c, (points[0].xy().0, points[0].xy().1), (points[1].xy().0, points[1].xy().1), width.max(1)); }
            return;
        }
        if width > 0 {
            lines(surf, c, true, points, width);
            return;
        }
        let mut s = surf.0.borrow_mut();
        let paint = paint_for(&s, c);
        let mut pb = PathBuilder::new();
        for (i, p) in points.iter().enumerate() {
            let (x, y) = p.xy();
            let (x, y) = (x as i32 as f32 + 0.5, y as i32 as f32 + 0.5);
            if i == 0 { pb.move_to(x, y); } else { pb.line_to(x, y); }
        }
        pb.close();
        if let Some(p) = pb.finish() {
            fill_path_on(&mut s, &p, &paint, FillRule::Winding);
            // pygame füllt die Kanten mit - schmale Kontur dazu
            stroke_path_on(&mut s, &p, &paint, &Stroke { width: 1.0, ..Default::default() });
        }
    }

    /// pygame.draw.arc(surface, color, rect, start_angle, stop_angle, width=1) - Winkel in Bogenmaß,
    /// 0 = rechts, gegen den Uhrzeigersinn.
    pub fn arc<C: Into<Color>>(surf: &Surface, color: C, r: Rect, start: f64, stop: f64, width: i32) {
        let c = color.into();
        if width < 1 || r.w <= 0 || r.h <= 0 { return; }
        let mut stop = stop;
        while stop < start { stop += std::f64::consts::TAU; }
        let mut s = surf.0.borrow_mut();
        let paint = paint_for(&s, c);
        let rx = (r.w as f64) / 2.0 - width as f64 / 2.0;
        let ry = (r.h as f64) / 2.0 - width as f64 / 2.0;
        let cx = r.x as f64 + r.w as f64 / 2.0;
        let cy = r.y as f64 + r.h as f64 / 2.0;
        let steps = (((stop - start) * rx.max(ry) / 2.0).ceil() as i32).clamp(4, 720);
        let mut pb = PathBuilder::new();
        for i in 0..=steps {
            let a = start + (stop - start) * i as f64 / steps as f64;
            let x = cx + rx * a.cos();
            let y = cy - ry * a.sin();
            if i == 0 { pb.move_to(x as f32, y as f32); } else { pb.line_to(x as f32, y as f32); }
        }
        if let Some(p) = pb.finish() {
            stroke_path_on(&mut s, &p, &paint, &Stroke { width: width as f32, ..Default::default() });
        }
    }
}

// =============================
// pygame.transform
// =============================
pub mod transform {
    use super::*;

    /// pygame.transform.smoothscale / scale
    pub fn smoothscale(src: &Surface, size: (i32, i32)) -> Surface { scale_q(src, size, FilterQuality::Bilinear) }
    pub fn scale(src: &Surface, size: (i32, i32)) -> Surface { scale_q(src, size, FilterQuality::Nearest) }

    fn scale_q(src: &Surface, size: (i32, i32), q: FilterQuality) -> Surface {
        let s = src.0.borrow();
        let (w, h) = (size.0.max(0), size.1.max(0));
        let out = Surface::make(w, h, s.srcalpha);
        if w > 0 && h > 0 && s.w > 0 && s.h > 0 {
            let mut o = out.0.borrow_mut();
            let sx = o.pm.width() as f32 / s.pm.width() as f32;
            let sy = o.pm.height() as f32 / s.pm.height() as f32;
            if !s.srcalpha { o.pm.fill(tiny_skia::Color::TRANSPARENT); }
            o.pm.draw_pixmap(0, 0, s.pm.as_ref(), &PixmapPaint { opacity: 1.0, blend_mode: BlendMode::Source, quality: q }, Transform::from_scale(sx, sy), None);
        }
        out.0.borrow_mut().alpha = s.alpha;
        out
    }

    /// pygame.transform.flip(surface, flip_x, flip_y)
    pub fn flip(src: &Surface, fx: bool, fy: bool) -> Surface {
        let s = src.0.borrow();
        let out = Surface::make_k(s.w, s.h, s.srcalpha, s.k);
        {
            let mut o = out.0.borrow_mut();
            let t = Transform::from_row(if fx { -1.0 } else { 1.0 }, 0.0, 0.0, if fy { -1.0 } else { 1.0 },
                if fx { s.pm.width() as f32 } else { 0.0 }, if fy { s.pm.height() as f32 } else { 0.0 });
            o.pm.draw_pixmap(0, 0, s.pm.as_ref(), &PixmapPaint { blend_mode: BlendMode::Source, ..Default::default() }, t, None);
            o.alpha = s.alpha;
        }
        out
    }

    /// pygame.transform.rotate(surface, angle) - Grad, gegen den Uhrzeigersinn, Fläche wächst mit
    pub fn rotate(src: &Surface, angle: f64) -> Surface {
        let s = src.0.borrow();
        let a = angle.to_radians();
        let (w, h) = (s.w as f64, s.h as f64);
        let nw = (w * a.cos().abs() + h * a.sin().abs()).ceil() as i32;
        let nh = (w * a.sin().abs() + h * a.cos().abs()).ceil() as i32;
        let out = Surface::make_k(nw, nh, true, s.k);
        {
            let mut o = out.0.borrow_mut();
            let (opw, oph) = (o.pm.width() as f32, o.pm.height() as f32);
            let t = Transform::from_translate(opw / 2.0, oph / 2.0)
                .pre_rotate(-angle as f32)
                .pre_translate(-(s.pm.width() as f32) / 2.0, -(s.pm.height() as f32) / 2.0);
            let q = if s.k > 1.01 { FilterQuality::Bilinear } else { FilterQuality::Nearest };
            o.pm.draw_pixmap(0, 0, s.pm.as_ref(), &PixmapPaint { quality: q, ..Default::default() }, t, None);
            o.alpha = s.alpha;
        }
        out
    }
}

impl Default for Surface { fn default() -> Surface { Surface::new(800, 600) } }
