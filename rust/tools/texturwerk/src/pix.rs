//! Pixel-Raster-Texturen (`assets/textures/pix/*.pix`) -> SVG.
//!
//! Eine .pix-Datei beschreibt eine Textur wie Pixel-Art als Zeichen-Raster. Daraus wird eine SVG-Datei
//! mit glatten, scharfen Flächen (jede Farbe ein einziger Pfad, dadurch keine Haarlinien zwischen den
//! Zellen). So bleibt der Pixel-Look, ohne dass es verpixelt aussieht.
//!
//! ```text
//! # Kommentar
//! zelle 5                     Kantenlänge einer Zelle (Einheiten der Textur, Spielfigur = 100)
//! ursprung 0 0                linke obere Ecke des nächsten Rasters
//! body 0 0 100 100            Bereich, der auf das Spiel-Rechteck gelegt wird (Standard 0 0 100 100)
//! schatten 5 5 {{s}} 1        harter Schatten der ganzen Form: Versatz x y, Farbe, Deckkraft
//! schatten aus
//! transform <svg-transform>   Transform für die folgenden Raster (z.B. für Animationen mit {{spin}})
//! transform aus
//! farbe C {{c}} 1             Zeichen -> Farbe (Platzhalter erlaubt), optional Deckkraft
//! raster                      danach die Zeilen; '.' oder ' ' = leer. Endet bei Leerzeile/nächstem Befehl
//! ```
//! Mehrere `raster`-Blöcke in einer Datei sind erlaubt (z.B. Mütze und Stiel in verschiedenen Größen).

use std::collections::BTreeMap;
use std::fmt::Write as _;

struct Block {
    cell: f32,
    ox: f32,
    oy: f32,
    shadow: Option<(f32, f32, String, f32)>,
    transform: Option<String>,
    rows: Vec<Vec<char>>,
}

fn num(s: &str) -> Result<f32, String> { s.parse::<f32>().map_err(|_| format!("keine Zahl: {s}")) }
fn f(v: f32) -> String {
    let s = format!("{:.3}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_string() }
}

/// Pfad aus allen Zellen eines Zeichens (waagerechte Läufe zusammengefasst)
fn path_for(b: &Block, pick: impl Fn(char) -> bool) -> String {
    let mut d = String::new();
    for (r, row) in b.rows.iter().enumerate() {
        let mut c = 0;
        while c < row.len() {
            if pick(row[c]) {
                let start = c;
                while c < row.len() && pick(row[c]) { c += 1; }
                let (x, y, w) = (b.ox + start as f32 * b.cell, b.oy + r as f32 * b.cell, (c - start) as f32 * b.cell);
                let _ = write!(d, "M{} {}h{}v{}h{}z", f(x), f(y), f(w), f(b.cell), f(-w));
            } else { c += 1; }
        }
    }
    d
}

pub fn to_svg(name: &str, src: &str) -> Result<String, String> {
    let mut cell = 5.0f32;
    let (mut ox, mut oy) = (0.0f32, 0.0f32);
    let mut body = [0.0f32, 0.0, 100.0, 100.0];
    let mut shadow: Option<(f32, f32, String, f32)> = None;
    let mut transform: Option<String> = None;
    let mut colors: BTreeMap<char, (String, f32)> = BTreeMap::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut in_grid = false;
    let mut comment = String::new();

    for (ln, raw) in src.lines().enumerate() {
        let line = raw.trim_end();
        let err = |m: String| format!("{name}.pix Zeile {}: {m}", ln + 1);
        if in_grid {
            let t = line.trim();
            let is_cmd = t.split_whitespace().next().map(|w| ["zelle", "ursprung", "body", "schatten", "transform", "farbe", "raster"].contains(&w)).unwrap_or(false);
            if t.is_empty() || t.starts_with('#') || is_cmd { in_grid = false; } else {
                blocks.last_mut().unwrap().rows.push(t.chars().collect());
                continue;
            }
        }
        let t = line.trim();
        if t.is_empty() { continue; }
        if let Some(c) = t.strip_prefix('#') { if comment.is_empty() { comment = c.trim().to_string(); } continue; }
        let mut it = t.split_whitespace();
        let cmd = it.next().unwrap();
        let rest: Vec<&str> = it.collect();
        match cmd {
            "zelle" => cell = num(rest.first().ok_or_else(|| err("zelle braucht eine Zahl".into()))?).map_err(err)?,
            "ursprung" => {
                if rest.len() != 2 { return Err(err("ursprung braucht x y".into())); }
                ox = num(rest[0]).map_err(err)?; oy = num(rest[1]).map_err(err)?;
            }
            "body" => {
                if rest.len() != 4 { return Err(err("body braucht x y b h".into())); }
                for i in 0..4 { body[i] = num(rest[i]).map_err(err)?; }
            }
            "schatten" => {
                if rest.first() == Some(&"aus") { shadow = None; } else {
                    if rest.len() < 3 { return Err(err("schatten braucht x y farbe [deckkraft]".into())); }
                    let a = rest.get(3).map(|v| num(v)).transpose().map_err(err)?.unwrap_or(1.0);
                    shadow = Some((num(rest[0]).map_err(err)?, num(rest[1]).map_err(err)?, rest[2].to_string(), a));
                }
            }
            "transform" => { transform = if rest.first() == Some(&"aus") { None } else { Some(rest.join(" ")) }; }
            "farbe" => {
                if rest.len() < 2 { return Err(err("farbe braucht Zeichen und Farbe".into())); }
                let ch = rest[0].chars().next().unwrap();
                let a = rest.get(2).map(|v| num(v)).transpose().map_err(err)?.unwrap_or(1.0);
                colors.insert(ch, (rest[1].to_string(), a));
            }
            "raster" => {
                blocks.push(Block { cell, ox, oy, shadow: shadow.clone(), transform: transform.clone(), rows: Vec::new() });
                in_grid = true;
            }
            _ => return Err(err(format!("unbekannter Befehl: {cmd}"))),
        }
    }
    if blocks.is_empty() { return Err(format!("{name}.pix: kein raster")); }

    // Ausdehnung (inkl. Schatten), auf ganze Einheiten gerundet
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for b in &blocks {
        let w = b.rows.iter().map(|r| r.len()).max().unwrap_or(0) as f32 * b.cell;
        let h = b.rows.len() as f32 * b.cell;
        let (sx, sy) = b.shadow.as_ref().map(|s| (s.0, s.1)).unwrap_or((0.0, 0.0));
        x0 = x0.min(b.ox + sx.min(0.0)); y0 = y0.min(b.oy + sy.min(0.0));
        x1 = x1.max(b.ox + w + sx.max(0.0)); y1 = y1.max(b.oy + h + sy.max(0.0));
        for row in &b.rows { for ch in row { if *ch != '.' && *ch != ' ' && !colors.contains_key(ch) { return Err(format!("{name}.pix: Zeichen '{ch}' hat keine farbe")); } } }
    }
    // Animierte Raster können über ihre Fläche hinaus gehen -> etwas Luft
    let pad = if blocks.iter().any(|b| b.transform.is_some()) { 4.0 } else { 1.0 };
    let (x0, y0, x1, y1) = ((x0 - pad).floor(), (y0 - pad).floor(), (x1 + pad).ceil(), (y1 + pad).ceil());
    let (w, h) = (x1 - x0, y1 - y0);

    let mut out = String::new();
    let _ = writeln!(out, r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="{} {} {} {}" data-body="{} {} {} {}">"#,
        f(w), f(h), f(x0), f(y0), f(w), f(h), f(body[0]), f(body[1]), f(body[2]), f(body[3]));
    let _ = writeln!(out, "  <!-- Erzeugt von \"texturwerk pix\" aus pix/{name}.pix - dort bearbeiten, nicht hier. -->");
    if !comment.is_empty() { let _ = writeln!(out, "  <!-- {} -->", comment.replace("--", "-")); }
    for b in &blocks {
        let open = b.transform.as_ref().map(|t| format!("  <g transform=\"{t}\">\n")).unwrap_or_default();
        out.push_str(&open);
        if let Some((sx, sy, col, a)) = &b.shadow {
            let d = path_for(b, |c| c != '.' && c != ' ');
            let op = if *a < 1.0 { format!(" opacity=\"{}\"", f(*a)) } else { String::new() };
            let _ = writeln!(out, "  <path transform=\"translate({} {})\" fill=\"{}\"{} d=\"{}\"/>", f(*sx), f(*sy), col, op, d);
        }
        for (ch, (col, a)) in &colors {
            let d = path_for(b, |c| c == *ch);
            if d.is_empty() { continue; }
            let op = if *a < 1.0 { format!(" opacity=\"{}\"", f(*a)) } else { String::new() };
            let _ = writeln!(out, "  <path fill=\"{}\"{} d=\"{}\"/>", col, op, d);
        }
        if b.transform.is_some() { out.push_str("  </g>\n"); }
    }
    out.push_str("</svg>\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn einfaches_raster() {
        let s = to_svg("t", "zelle 10\nfarbe A #ff0000\nraster\nAA.\n.A.\n").unwrap();
        assert!(s.contains("M0 0h20v10h-20z"), "{s}");
        assert!(s.contains("M10 10h10v10h-10z"));
        assert!(s.contains(r#"viewBox="-1 -1 32 22""#), "{s}");
    }
    #[test]
    fn fehlende_farbe() {
        assert!(to_svg("t", "farbe A #fff\nraster\nAB\n").is_err());
    }
}
