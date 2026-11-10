//! pygame.font: Schrift laden (Standardschrift wie pygame = freesansbold.ttf), Text rendern.

use super::surface::{Color, Surface};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tiny_skia::{Pixmap, PremultipliedColorU8};

static DEFAULT_FONT: &[u8] = include_bytes!("../../assets/freesansbold.ttf");

struct Glyph {
    m: fontdue::Metrics,
    bmp: Vec<u8>,
}

struct FontData {
    font: fontdue::Font,
    fallbacks: Vec<Rc<fontdue::Font>>,
    px: f32,
    ascent: i32,
    height: i32,
    linesize: i32,
    glyphs: RefCell<HashMap<char, (usize, Rc<Glyph>)>>,
    cache: RefCell<HashMap<(String, u32), Surface>>,
    bold: bool,
}

#[derive(Clone)]
pub struct Font(Rc<FontData>);

thread_local! {
    static FALLBACKS: RefCell<Option<Vec<Rc<fontdue::Font>>>> = const { RefCell::new(None) };
}

fn load_font_bytes(bytes: &[u8]) -> Option<fontdue::Font> {
    fontdue::Font::from_bytes(bytes.to_vec(), fontdue::FontSettings::default()).ok()
}

/// Systemschriften für Zeichen, die in der Hauptschrift fehlen (z.B. Symbole).
fn fallbacks() -> Vec<Rc<fontdue::Font>> {
    FALLBACKS.with(|f| {
        let mut f = f.borrow_mut();
        if f.is_none() {
            let mut v = Vec::new();
            for p in system_font_dirs().iter().flat_map(|d| ["seguisym.ttf", "segoeui.ttf", "DejaVuSans.ttf", "dejavu/DejaVuSans.ttf", "truetype/dejavu/DejaVuSans.ttf", "Arial Unicode.ttf"].iter().map(move |n| d.join(n))) {
                if let Ok(b) = std::fs::read(&p) {
                    if let Some(ft) = load_font_bytes(&b) { v.push(Rc::new(ft)); }
                }
                if v.len() >= 2 { break; }
            }
            *f = Some(v);
        }
        f.as_ref().unwrap().clone()
    })
}

fn system_font_dirs() -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    if cfg!(windows) {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        v.push(std::path::PathBuf::from(windir).join("Fonts"));
    } else if cfg!(target_os = "macos") {
        v.push("/System/Library/Fonts".into());
        v.push("/System/Library/Fonts/Supplemental".into());
        v.push("/Library/Fonts".into());
    } else {
        v.push("/usr/share/fonts".into());
        v.push("/usr/share/fonts/truetype".into());
        v.push("/usr/local/share/fonts".into());
    }
    v
}

fn find_system_font(names: &[&str]) -> Option<Vec<u8>> {
    for d in system_font_dirs() {
        for n in names {
            let p = d.join(n);
            if let Ok(b) = std::fs::read(&p) { return Some(b); }
        }
    }
    None
}

impl Font {
    fn from_font(font: fontdue::Font, px: f32, bold: bool) -> Font {
        let px = px.max(1.0);
        let lm = font.horizontal_line_metrics(px);
        let (ascent, descent, linesize) = match lm {
            Some(l) => (l.ascent.ceil() as i32, l.descent.floor() as i32, l.new_line_size.ceil() as i32),
            None => ((px * 0.8) as i32, -((px * 0.2) as i32), px as i32),
        };
        let height = ascent - descent;
        Font(Rc::new(FontData {
            font, fallbacks: fallbacks(), px, ascent, height, linesize: linesize.max(height),
            glyphs: RefCell::new(HashMap::new()), cache: RefCell::new(HashMap::new()), bold,
        }))
    }

    /// pygame.font.Font(None, size) / pygame.font.SysFont(None, size): Standardschrift,
    /// pygame verkleinert die Standardschrift intern auf 0.6875 * size.
    pub fn new(size: i32) -> Font {
        let px = ((size as f32) * 0.6875).floor().max(1.0);
        Font::from_font(load_font_bytes(DEFAULT_FONT).unwrap(), px, false)
    }

    /// pygame.font.SysFont(name, size, bold) - Systemschrift, sonst Standardschrift
    pub fn sys(name: &str, size: i32, bold: bool) -> Font {
        let lname = name.to_lowercase();
        let cands: Vec<&str> = match (lname.as_str(), bold) {
            ("consolas", _) => vec!["consola.ttf", "DejaVuSansMono.ttf", "dejavu/DejaVuSansMono.ttf", "truetype/dejavu/DejaVuSansMono.ttf", "Menlo.ttc", "liberation/LiberationMono-Regular.ttf"],
            ("arial", true) => vec!["arialbd.ttf", "Arial Bold.ttf", "DejaVuSans-Bold.ttf", "dejavu/DejaVuSans-Bold.ttf", "truetype/dejavu/DejaVuSans-Bold.ttf"],
            ("arial", false) => vec!["arial.ttf", "Arial.ttf", "DejaVuSans.ttf", "dejavu/DejaVuSans.ttf", "truetype/dejavu/DejaVuSans.ttf"],
            ("segoeui", _) | ("segoe ui", _) => vec!["segoeui.ttf"],
            ("segoeuisymbol", _) | ("segoe ui symbol", _) => vec!["seguisym.ttf"],
            _ => vec![],
        };
        if let Some(b) = find_system_font(&cands) {
            if let Some(f) = load_font_bytes(&b) {
                return Font::from_font(f, size as f32, bold);
            }
        }
        Font::new(size)
    }

    pub fn get_height(&self) -> i32 { self.0.height }
    pub fn get_linesize(&self) -> i32 { self.0.linesize }
    pub fn get_ascent(&self) -> i32 { self.0.ascent }

    fn glyph(&self, ch: char) -> (usize, Rc<Glyph>) {
        if let Some(g) = self.0.glyphs.borrow().get(&ch) { return g.clone(); }
        let mut src = 0usize;
        let mut idx = self.0.font.lookup_glyph_index(ch);
        if idx == 0 && !ch.is_whitespace() {
            for (i, fb) in self.0.fallbacks.iter().enumerate() {
                if fb.lookup_glyph_index(ch) != 0 { src = i + 1; idx = 1; break; }
            }
        }
        let _ = idx;
        let f: &fontdue::Font = if src == 0 { &self.0.font } else { &self.0.fallbacks[src - 1] };
        let (m, bmp) = f.rasterize(ch, self.0.px);
        let g = Rc::new(Glyph { m, bmp });
        self.0.glyphs.borrow_mut().insert(ch, (src, g.clone()));
        (src, g)
    }

    /// font.size(text) -> (breite, höhe)
    pub fn size(&self, text: &str) -> (i32, i32) {
        let mut w = 0.0f32;
        for ch in text.chars() {
            let (_, g) = self.glyph(ch);
            w += g.m.advance_width;
        }
        if self.0.bold { w += 1.0; }
        (w.round() as i32, self.0.height)
    }

    /// font.render(text, antialias, color) -> Surface mit Alphakanal
    pub fn render<C: Into<Color>>(&self, text: &str, _aa: bool, color: C) -> Surface {
        let c: Color = color.into();
        let key = (text.to_string(), u32::from_be_bytes([c.0, c.1, c.2, c.3]));
        if let Some(s) = self.0.cache.borrow().get(&key) {
            return s.copy();
        }
        let surf = self.render_raw(text, c);
        let mut cache = self.0.cache.borrow_mut();
        if cache.len() > 600 { cache.clear(); }
        cache.insert(key, surf.copy());
        surf
    }

    /// render mit Hintergrundfarbe (pygame: font.render(text, aa, color, background))
    pub fn render_bg<C: Into<Color>, B: Into<Color>>(&self, text: &str, aa: bool, color: C, bg: B) -> Surface {
        let t = self.render(text, aa, color);
        let (w, h) = t.get_size();
        let out = Surface::new(w, h);
        out.fill(bg);
        out.blit(&t, (0, 0));
        out
    }

    fn render_raw(&self, text: &str, c: Color) -> Surface {
        let (w, h) = self.size(text);
        let surf = Surface::new_alpha(w, h);
        if w <= 0 { return surf; }
        {
            let mut inner = surf.0.borrow_mut();
            let pm: &mut Pixmap = &mut inner.pm;
            let pw = pm.width() as i32;
            let ph = pm.height() as i32;
            let data = pm.pixels_mut();
            let mut pen = 0.0f32;
            let base = self.0.ascent;
            let passes = if self.0.bold { 2 } else { 1 };
            for ch in text.chars() {
                let (_, g) = self.glyph(ch);
                let gx0 = (pen + g.m.xmin as f32).round() as i32;
                let gy0 = base - (g.m.height as i32 + g.m.ymin);
                for pass in 0..passes {
                    for gy in 0..g.m.height as i32 {
                        let y = gy0 + gy;
                        if y < 0 || y >= ph { continue; }
                        for gx in 0..g.m.width as i32 {
                            let x = gx0 + gx + pass;
                            if x < 0 || x >= pw { continue; }
                            let cov = g.bmp[(gy * g.m.width as i32 + gx) as usize] as u32;
                            if cov == 0 { continue; }
                            let a = cov * c.3 as u32 / 255;
                            let idx = (y * pw + x) as usize;
                            let old = data[idx];
                            let oa = old.alpha() as u32;
                            let na = oa.max(a);
                            if na == oa { continue; }
                            let pr = (c.0 as u32 * na / 255) as u8;
                            let pg = (c.1 as u32 * na / 255) as u8;
                            let pb = (c.2 as u32 * na / 255) as u8;
                            data[idx] = PremultipliedColorU8::from_rgba(pr, pg, pb, na as u8).unwrap_or(old);
                        }
                    }
                }
                pen += g.m.advance_width;
            }
        }
        surf
    }
}

impl Default for Font { fn default() -> Font { Font::new(26) } }
