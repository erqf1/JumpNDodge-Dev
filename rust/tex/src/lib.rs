//! Texturen der Rust Edition.
//!
//! Jede Textur ist eine SVG-Datei in `assets/textures/` (beim Bauen eingebettet). Statt Formen im
//! Code zu zeichnen, wird die Datei mit resvg in genau der Größe gerendert, die das Spiel braucht -
//! dadurch bleibt alles bei jeder Auflösung scharf.
//!
//! Regeln für die Dateien (prüft `texturwerk check`):
//! - `viewBox` = gesamte Zeichenfläche inklusive Überstand (Schatten, Glühen, Hüte ...).
//! - `width`/`height` = Breite/Höhe der viewBox.
//! - `data-body="x y w h"` = der Teil, der genau auf das Spiel-Rechteck (Hitbox) gelegt wird.
//!   Fehlt das Attribut, gilt `0 0 100 100`. Alles, was an der Spielfigur hängt (Hüte, Augen ...),
//!   benutzt dieselben Koordinaten wie `player.svg`.
//! - Platzhalter `{{name}}` werden vor dem Rendern ersetzt, z.B. die Spielerfarbe (siehe [`palette`]).
//!
//! Eigene Texturen: Liegt im Ordner aus [`set_override_dir`] eine Datei `<name>.svg` oder `<name>.png`,
//! wird sie statt der eingebauten benutzt (PNG wird auf das Spiel-Rechteck gestreckt).

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub use resvg::tiny_skia::Pixmap;
use resvg::tiny_skia::{self, Transform};
use resvg::usvg;

/// Eingebaute Texturen (Name, SVG-Text)
pub static BUILTIN: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/textures.rs"));

fn override_dir() -> &'static Mutex<Option<PathBuf>> {
    static D: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    D.get_or_init(|| Mutex::new(None))
}

/// Ordner mit eigenen Texturen (wird vor den eingebauten durchsucht). `None` = nur eingebaute.
pub fn set_override_dir(dir: Option<PathBuf>) { *override_dir().lock().unwrap() = dir; }

/// Herkunft einer Textur
pub enum Source {
    Svg(Cow<'static, str>),
    Png(Vec<u8>),
}

/// Textur suchen: erst eigener Ordner (svg, dann png), dann eingebaut.
pub fn source(name: &str) -> Option<Source> {
    if let Some(dir) = override_dir().lock().unwrap().clone() {
        if let Ok(s) = std::fs::read_to_string(dir.join(format!("{name}.svg"))) { return Some(Source::Svg(Cow::Owned(s))); }
        if let Ok(b) = std::fs::read(dir.join(format!("{name}.png"))) { return Some(Source::Png(b)); }
    }
    BUILTIN.iter().find(|(n, _)| *n == name).map(|(_, s)| Source::Svg(Cow::Borrowed(*s)))
}

/// Gibt es diese Textur (eingebaut oder eigene)?
pub fn exists(name: &str) -> bool { source(name).is_some() }

/// Alle bekannten Namen (eingebaut + eigener Ordner), sortiert
pub fn names() -> Vec<String> {
    let mut v: Vec<String> = BUILTIN.iter().map(|(n, _)| n.to_string()).collect();
    if let Some(dir) = override_dir().lock().unwrap().clone() {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if let Some(stem) = n.strip_suffix(".svg").or_else(|| n.strip_suffix(".png")) { v.push(stem.to_string()); }
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

// ------------------------------------------------------------------ Farben

pub type Rgb = (u8, u8, u8);

pub fn hex(c: Rgb) -> String { format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2) }
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}
pub fn lighten(c: Rgb, t: f32) -> Rgb { mix(c, (255, 255, 255), t) }
pub fn darken(c: Rgb, t: f32) -> Rgb { mix(c, (0, 0, 0), t) }

/// Platzhalter für eine Grundfarbe: `{{c}}` selbst, `{{c1}}`..`{{c3}}` heller, `{{d1}}`..`{{d3}}` dunkler,
/// `{{s}}` = gesättigter Schattenton (dunkel, leicht ins Violette, damit Schatten nicht grau wirken).
pub fn palette(c: Rgb) -> Vec<(&'static str, String)> {
    vec![
        ("c", hex(c)),
        ("c1", hex(lighten(c, 0.22))),
        ("c2", hex(lighten(c, 0.48))),
        ("c3", hex(lighten(c, 0.8))),
        ("d1", hex(darken(c, 0.2))),
        ("d2", hex(darken(c, 0.42))),
        ("d3", hex(darken(c, 0.66))),
        ("s", hex(mix(darken(c, 0.72), (40, 20, 70), 0.45))),
    ]
}

/// `{{name}}` durch Werte ersetzen
pub fn fill(svg: &str, vars: &[(&str, String)]) -> String {
    if !svg.contains("{{") { return svg.to_string(); }
    let mut out = String::with_capacity(svg.len());
    let mut rest = svg;
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        if let Some(j) = after.find("}}") {
            let key = after[..j].trim();
            match vars.iter().find(|(k, _)| *k == key) {
                Some((_, v)) => out.push_str(v),
                None => { out.push_str("{{"); out.push_str(&after[..j]); out.push_str("}}"); }
            }
            rest = &after[j + 2..];
        } else {
            out.push_str(&rest[i..]);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

// ------------------------------------------------------------------ Kopf der Datei lesen

/// viewBox und Body-Box aus dem `<svg ...>`-Tag
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Boxes { pub view: [f32; 4], pub body: [f32; 4] }

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let pat = format!("{name}=\"");
    let mut from = 0;
    while let Some(i) = tag[from..].find(&pat) {
        let i = from + i;
        // nur ganze Attributnamen (nicht "data-viewBox" o.ä.)
        let ok = i == 0 || tag.as_bytes()[i - 1].is_ascii_whitespace();
        let start = i + pat.len();
        if ok { return tag[start..].find('"').map(|e| &tag[start..start + e]); }
        from = start;
    }
    None
}
fn four(s: &str) -> Option<[f32; 4]> {
    let v: Vec<f32> = s.split(|c: char| c.is_whitespace() || c == ',').filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect();
    if v.len() == 4 { Some([v[0], v[1], v[2], v[3]]) } else { None }
}

pub fn boxes(svg: &str) -> Option<Boxes> {
    let i = svg.find("<svg")?;
    let e = svg[i..].find('>')? + i;
    let tag = &svg[i..e];
    let view = four(attr(tag, "viewBox")?)?;
    let body = attr(tag, "data-body").and_then(four).unwrap_or([0.0, 0.0, 100.0, 100.0]);
    Some(Boxes { view, body })
}

// ------------------------------------------------------------------ Rendern

/// Fertig gerenderte Textur. `off_*` = Lage der linken oberen Pixmap-Ecke relativ zum Spiel-Rechteck
/// (in logischen Pixeln), `w`/`h` = logische Größe der ganzen Pixmap.
pub struct Rendered { pub pixmap: Pixmap, pub off_x: f32, pub off_y: f32, pub w: f32, pub h: f32 }

fn parse(svg: &str) -> Result<usvg::Tree, String> {
    usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|e| e.to_string())
}

/// Rendert so, dass die Body-Box genau `body_w` x `body_h` logische Pixel groß ist; `k` = echte Pixel pro
/// logischem Pixel.
pub fn render_fit(name: &str, vars: &[(&str, String)], body_w: f32, body_h: f32, k: f32) -> Option<Rendered> {
    match source(name)? {
        Source::Svg(text) => render_svg_fit(&fill(&text, vars), body_w, body_h, k).ok(),
        Source::Png(bytes) => {
            let img = Pixmap::decode_png(&bytes).ok()?;
            let (pw, ph) = (((body_w * k).ceil() as u32).max(1), ((body_h * k).ceil() as u32).max(1));
            let mut pm = Pixmap::new(pw, ph)?;
            let t = Transform::from_scale(pw as f32 / img.width() as f32, ph as f32 / img.height() as f32);
            pm.draw_pixmap(0, 0, img.as_ref(), &tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Bicubic, ..Default::default() }, t, None);
            Some(Rendered { pixmap: pm, off_x: 0.0, off_y: 0.0, w: body_w, h: body_h })
        }
    }
}

/// Wie [`render_fit`], aber direkt mit SVG-Text (Platzhalter schon ersetzt)
pub fn render_svg_fit(svg: &str, body_w: f32, body_h: f32, k: f32) -> Result<Rendered, String> {
    let b = boxes(svg).ok_or("viewBox fehlt")?;
    let tree = parse(svg)?;
    let sx = body_w / b.body[2];
    let sy = body_h / b.body[3];
    let (w, h) = (b.view[2] * sx, b.view[3] * sy);
    let (pw, ph) = (((w * k).ceil() as u32).max(1), ((h * k).ceil() as u32).max(1));
    let mut pm = Pixmap::new(pw, ph).ok_or("zu groß")?;
    // Der Baum ist schon in viewBox-Einheiten mit Ursprung links oben (width/height = viewBox)
    let ts = tree.size();
    let fx = b.view[2] / ts.width();
    let fy = b.view[3] / ts.height();
    resvg::render(&tree, Transform::from_scale(sx * k * fx, sy * k * fy), &mut pm.as_mut());
    Ok(Rendered { pixmap: pm, off_x: (b.view[0] - b.body[0]) * sx, off_y: (b.view[1] - b.body[1]) * sy, w, h })
}

/// Rendert nur den tatsächlich bemalten Bereich der Textur, eingepasst (Seitenverhältnis bleibt) in ein
/// Quadrat von `size` logischen Pixeln - für Symbole im Shop/Inventar.
pub fn render_icon(name: &str, vars: &[(&str, String)], size: f32, k: f32) -> Option<Pixmap> {
    let text = match source(name)? { Source::Svg(t) => fill(&t, vars), Source::Png(_) => {
        let r = render_fit(name, vars, size, size, k)?;
        return Some(r.pixmap);
    } };
    let tree = parse(&text).ok()?;
    let bb = tree.root().abs_stroke_bounding_box();
    let (bw, bh) = (bb.width().max(1.0), bb.height().max(1.0));
    let px = (size * k).ceil() as u32;
    let mut pm = Pixmap::new(px.max(1), px.max(1))?;
    let s = (size * k) / bw.max(bh);
    let (ox, oy) = ((size * k - bw * s) / 2.0, (size * k - bh * s) / 2.0);
    let t = Transform::from_row(s, 0.0, 0.0, s, ox - bb.x() * s, oy - bb.y() * s);
    resvg::render(&tree, t, &mut pm.as_mut());
    Some(pm)
}

/// Rendert eine Fläche, die das Ziel komplett bedeckt (Seitenverhältnis bleibt, Ränder werden
/// abgeschnitten) - für Hintergründe.
pub fn render_cover(name: &str, vars: &[(&str, String)], w: f32, h: f32, k: f32) -> Option<Pixmap> {
    let text = match source(name)? { Source::Svg(t) => fill(&t, vars), Source::Png(_) => return render_fit(name, vars, w, h, k).map(|r| r.pixmap) };
    let b = boxes(&text)?;
    let tree = parse(&text).ok()?;
    let (pw, ph) = (((w * k).ceil() as u32).max(1), ((h * k).ceil() as u32).max(1));
    let mut pm = Pixmap::new(pw, ph)?;
    let s = (pw as f32 / b.view[2]).max(ph as f32 / b.view[3]);
    let ts = tree.size();
    let (fx, fy) = (b.view[2] / ts.width(), b.view[3] / ts.height());
    let (ox, oy) = ((pw as f32 - b.view[2] * s) / 2.0, (ph as f32 - b.view[3] * s) / 2.0);
    resvg::render(&tree, Transform::from_row(s * fx, 0.0, 0.0, s * fy, ox, oy), &mut pm.as_mut());
    Some(pm)
}

/// Prüft eine Textur und liefert Probleme als Text (leer = alles gut)
pub fn check(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Some(src) = source(name) else { return vec!["fehlt".into()] };
    let Source::Svg(text) = src else { return out };
    let mut vars = palette((80, 120, 240));
    vars.push(("spin", "1".into()));
    vars.push(("hue", "#ff4fa0".into()));
    let filled = fill(&text, &vars);
    match boxes(&filled) {
        None => out.push("viewBox fehlt oder ist kaputt".into()),
        Some(b) => {
            let i = filled.find("<svg").unwrap_or(0);
            let e = filled[i..].find('>').map(|e| e + i).unwrap_or(filled.len());
            let tag = &filled[i..e];
            let wh = (attr(tag, "width").and_then(|v| v.parse::<f32>().ok()), attr(tag, "height").and_then(|v| v.parse::<f32>().ok()));
            if wh != (Some(b.view[2]), Some(b.view[3])) { out.push(format!("width/height müssen {} x {} sein (= viewBox)", b.view[2], b.view[3])); }
            if b.body[2] <= 0.0 || b.body[3] <= 0.0 { out.push("data-body hat keine Größe".into()); }
        }
    }
    if filled.contains("{{") { out.push("unbekannter Platzhalter {{...}}".into()); }
    if filled.contains("<text") { out.push("<text> wird nicht unterstützt (keine Schriften eingebaut) - als Pfad zeichnen".into()); }
    if let Err(e) = parse(&filled) { out.push(format!("SVG-Fehler: {e}")); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platzhalter() {
        assert_eq!(fill("a{{c}}b{{x}}", &[("c", "#fff".into())]), "a#fffb{{x}}");
    }
    #[test]
    fn kopf() {
        let b = boxes(r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="140" viewBox="-10 -30 120 140" data-body="0 0 100 100">"#).unwrap();
        assert_eq!(b.view, [-10.0, -30.0, 120.0, 140.0]);
        assert_eq!(b.body, [0.0, 0.0, 100.0, 100.0]);
    }
    #[test]
    fn alle_eingebauten_texturen_sind_gueltig() {
        for (n, _) in BUILTIN { let p = check(n); assert!(p.is_empty(), "{n}: {p:?}"); }
    }
    #[test]
    fn rendern_gibt_richtige_groesse() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120" viewBox="-10 -10 120 120"><rect width="100" height="100" fill="#f00"/></svg>"##;
        let r = render_svg_fit(svg, 50.0, 50.0, 2.0).unwrap();
        assert_eq!((r.pixmap.width(), r.pixmap.height()), (120, 120));
        assert_eq!((r.off_x, r.off_y), (-5.0, -5.0));
        // Mitte rot, Ecke leer
        let px = |x, y| r.pixmap.pixel(x, y).unwrap();
        assert_eq!(px(60, 60).red(), 255);
        assert_eq!(px(2, 2).alpha(), 0);
    }
}
