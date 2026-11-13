//! Texturwerk - Werkzeug für die JumpNDodge-Texturen.
//!
//!   texturwerk pix                        alle Pixel-Raster (assets/textures/pix/*.pix) in SVG umwandeln
//!   texturwerk check                      alle Texturen prüfen (Pflicht-Liste, viewBox, Platzhalter, SVG-Fehler)
//!   texturwerk bogen  [datei.png]         Vorschau-Bogen: jede Textur groß, Spielfigur mit jedem Cosmetic, Farben
//!   texturwerk szene  [datei.png] [hell]  Probe-Szene wie im Spiel (800x600, doppelt scharf)
//!   texturwerk export <name> <b> <h> <datei.png> [r,g,b]   eine Textur als PNG
//!
//! Gelesen wird direkt aus `assets/textures/` (Änderungen sind sofort sichtbar, ohne neu zu bauen).
//! Mit `--ordner <pfad>` lässt sich ein anderer Ordner (z.B. ein Texturpaket) ansehen.

mod pix;

use jnd_tex::{palette, Pixmap, Rgb};
use std::path::PathBuf;
use tiny_skia::{Color, Paint, PixmapPaint, Rect, Transform};

/// Texturen, die das Spiel erwartet
const PFLICHT: &[&str] = &[
    "player", "hazard", "faller", "oneup", "floor", "floor_light",
    "cos_yellow_crown", "cos_top_hat", "cos_devil_horns", "cos_halo", "cos_propeller_hat", "cos_propeller_blade", "cos_ninja_headband",
    "cos_pixel_eyes", "cos_sunglasses", "cos_3d_glasses", "cos_robot_visor",
    "cos_clown_nose", "cos_pig_nose",
    "cos_big_grin", "cos_vampire_fangs", "cos_nerd_teeth", "cos_bird_beak",
    "trail_fire_trail", "trail_ice_trail", "trail_shadow_trail", "trail_sparkle_trail", "trail_toxic_trail", "trail_electric_trail",
    "trail_rainbow_trail", "trail_star_trail", "trail_smoke_trail", "trail_blood_trail", "trail_heart_trail",
];

const HEAD: &[&str] = &["yellow_crown", "top_hat", "devil_horns", "halo", "propeller_hat", "ninja_headband"];
const EYES: &[&str] = &["pixel_eyes", "sunglasses", "3d_glasses", "robot_visor"];
const NOSE: &[&str] = &["clown_nose", "pig_nose"];
const MOUTH: &[&str] = &["big_grin", "vampire_fangs", "nerd_teeth", "bird_beak"];
const FARBEN: &[Rgb] = &[(64, 110, 235), (225, 70, 70), (250, 180, 40), (150, 80, 230), (40, 190, 200), (235, 235, 240), (40, 40, 48)];

struct Schrift(fontdue::Font);
impl Schrift {
    fn laden() -> Option<Schrift> {
        for p in ["assets/freesansbold.ttf", "../assets/freesansbold.ttf", "rust/assets/freesansbold.ttf"] {
            if let Ok(b) = std::fs::read(p) { return fontdue::Font::from_bytes(b, Default::default()).ok().map(Schrift); }
        }
        None
    }
    fn text(&self, pm: &mut Pixmap, s: &str, x: f32, y: f32, px: f32, c: (u8, u8, u8)) {
        let mut cx = x;
        for ch in s.chars() {
            let (m, bmp) = self.0.rasterize(ch, px);
            let gx = cx as i32 + m.xmin;
            let gy = (y + px) as i32 - m.height as i32 - m.ymin;
            for row in 0..m.height {
                for col in 0..m.width {
                    let a = bmp[row * m.width + col];
                    if a == 0 { continue; }
                    let (px, py) = (gx + col as i32, gy + row as i32);
                    if px < 0 || py < 0 || px >= pm.width() as i32 || py >= pm.height() as i32 { continue; }
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(c.0, c.1, c.2, a);
                    pm.fill_rect(Rect::from_xywh(px as f32, py as f32, 1.0, 1.0).unwrap(), &paint, Transform::identity(), None);
                }
            }
            cx += m.advance_width;
        }
    }
}

fn rechteck(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, c: (u8, u8, u8, u8)) {
    let mut p = Paint::default();
    p.set_color_rgba8(c.0, c.1, c.2, c.3);
    if let Some(r) = Rect::from_xywh(x, y, w, h) { pm.fill_rect(r, &p, Transform::identity(), None); }
}

/// Textur so zeichnen, dass ihr Body-Bereich bei (x, y) mit Größe w x h liegt
fn setze(pm: &mut Pixmap, name: &str, vars: &[(&str, String)], x: f32, y: f32, w: f32, h: f32, k: f32) -> bool {
    match jnd_tex::render_fit(name, vars, w, h, k) {
        Some(r) => {
            pm.draw_pixmap(((x + r.off_x) * k).round() as i32, ((y + r.off_y) * k).round() as i32, r.pixmap.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
            true
        }
        None => false,
    }
}

fn spielfigur(pm: &mut Pixmap, x: f32, y: f32, s: f32, k: f32, farbe: Rgb, cos: &[&str]) {
    let vars = palette(farbe);
    setze(pm, "player", &vars, x, y, s, s, k);
    for c in cos {
        setze(pm, &format!("cos_{c}"), &vars, x, y, s, s, k);
        if *c == "propeller_hat" {
            let mut v = vars.clone();
            v.push(("spin", "0.7".into()));
            setze(pm, "cos_propeller_blade", &v, x, y, s, s, k);
        }
    }
}

fn hintergrund(pm: &mut Pixmap, dunkel: bool) {
    let name = if dunkel { "bg_dark" } else { "bg_light" };
    match jnd_tex::render_cover(name, &[], pm.width() as f32, pm.height() as f32, 1.0) {
        Some(bg) => { pm.draw_pixmap(0, 0, bg.as_ref(), &PixmapPaint::default(), Transform::identity(), None); }
        None => pm.fill(if dunkel { Color::from_rgba8(24, 22, 36, 255) } else { Color::from_rgba8(236, 240, 255, 255) }),
    }
}

fn bogen(out: &str) {
    let k = 2.0;
    let (w, h) = (1500.0, 1180.0);
    let mut pm = Pixmap::new((w * k) as u32, (h * k) as u32).unwrap();
    hintergrund(&mut pm, true);
    rechteck(&mut pm, 0.0, 0.0, w * k, h * k, (10, 8, 20, 120));
    let f = Schrift::laden();
    let label = |pm: &mut Pixmap, s: &str, x: f32, y: f32| if let Some(f) = &f { f.text(pm, s, x * k, y * k, 13.0 * k, (210, 205, 235)); };
    let titel = |pm: &mut Pixmap, s: &str, x: f32, y: f32| if let Some(f) = &f { f.text(pm, s, x * k, y * k, 20.0 * k, (255, 255, 255)); };

    // 1) Spielwelt groß
    titel(&mut pm, "Spielwelt (Body = 100 px)", 30.0, 20.0);
    let welt: &[(&str, Rgb)] = &[("player", (64, 110, 235)), ("hazard", (226, 58, 82)), ("faller", (150, 70, 230)), ("oneup", (45, 200, 200))];
    for (i, (n, c)) in welt.iter().enumerate() {
        let x = 40.0 + i as f32 * 170.0;
        rechteck(&mut pm, x * k, 70.0 * k, 100.0 * k, 100.0 * k, (255, 255, 255, 18));
        setze(&mut pm, n, &palette(*c), x, 70.0, 100.0, 100.0, k);
        label(&mut pm, n, x, 195.0);
    }
    // Boden-Kachel dreimal nebeneinander
    for i in 0..4 { setze(&mut pm, "floor", &[], 740.0 + i as f32 * 96.0, 90.0, 96.0, 48.0, k); }
    label(&mut pm, "floor (4x gekachelt)", 740.0, 195.0);

    // 2) Spielerfarben, in Spielgröße 50
    titel(&mut pm, "Spielerfarben (Spielgröße 50 px)", 30.0, 230.0);
    for (i, c) in FARBEN.iter().enumerate() {
        spielfigur(&mut pm, 40.0 + i as f32 * 80.0, 280.0, 50.0, k, *c, &[]);
    }
    for (i, c) in FARBEN.iter().take(4).enumerate() {
        let x = 640.0 + i as f32 * 80.0;
        setze(&mut pm, "hazard", &palette(match i { 0 => (226, 58, 82), 1 => (240, 120, 40), 2 => (200, 40, 160), _ => (*c).clone() }), x, 280.0, 50.0, 50.0, k);
    }

    // 3) Cosmetics einzeln auf blauer Figur, groß
    titel(&mut pm, "Cosmetics (Figur 90 px)", 30.0, 370.0);
    let alle: Vec<&str> = HEAD.iter().chain(EYES).chain(NOSE).chain(MOUTH).copied().collect();
    for (i, c) in alle.iter().enumerate() {
        let (col, row) = (i % 8, i / 8);
        let x = 50.0 + col as f32 * 180.0;
        let y = 450.0 + row as f32 * 190.0;
        spielfigur(&mut pm, x, y, 90.0, k, FARBEN[i % 5], &[c]);
        label(&mut pm, c, x - 10.0, y + 105.0);
    }

    // 4) Spuren
    titel(&mut pm, "Spuren (Partikel, 40 px und 14 px)", 30.0, 1010.0);
    let spuren = ["fire_trail", "ice_trail", "shadow_trail", "sparkle_trail", "toxic_trail", "electric_trail", "rainbow_trail", "star_trail", "smoke_trail", "blood_trail", "heart_trail"];
    for (i, s) in spuren.iter().enumerate() {
        let x = 40.0 + i as f32 * 130.0;
        let v = vec![("hue", "#ff4fa0".to_string())];
        setze(&mut pm, &format!("trail_{s}"), &v, x, 1050.0, 40.0, 40.0, k);
        setze(&mut pm, &format!("trail_{s}"), &v, x + 60.0, 1063.0, 14.0, 14.0, k);
        label(&mut pm, s.trim_end_matches("_trail"), x, 1105.0);
    }
    pm.save_png(out).expect("PNG speichern");
    println!("Bogen gespeichert: {out}");
}

fn szene(out: &str, dunkel: bool) {
    let k = 2.0;
    let (w, h) = (800.0, 600.0);
    let mut pm = Pixmap::new((w * k) as u32, (h * k) as u32).unwrap();
    hintergrund(&mut pm, dunkel);
    let floor_y = h - 48.0;
    let mut x = -30.0;
    while x < w { setze(&mut pm, "floor", &[], x, floor_y, 96.0, 48.0, k); x += 96.0; }
    // Spur hinter der Figur
    for i in 0..9 {
        let t = i as f32 / 9.0;
        let s = 16.0 * (1.0 - t) + 4.0;
        setze(&mut pm, "trail_fire_trail", &[], 80.0 - i as f32 * 11.0 - s / 2.0, 470.0 + (i as f32 * 1.7).sin() * 6.0 - s / 2.0 + 20.0, s, s, k);
    }
    spielfigur(&mut pm, 90.0, 400.0, 50.0, k, (64, 110, 235), &["yellow_crown", "sunglasses", "big_grin"]);
    setze(&mut pm, "hazard", &palette((226, 58, 82)), 420.0, floor_y - 50.0, 50.0, 50.0, k);
    setze(&mut pm, "hazard", &palette((226, 58, 82)), 570.0, floor_y - 50.0, 50.0, 50.0, k);
    setze(&mut pm, "hazard", &palette((226, 58, 82)), 670.0, floor_y - 50.0, 50.0, 50.0, k);
    setze(&mut pm, "faller", &palette((150, 70, 230)), 300.0, 150.0, 50.0, 50.0, k);
    setze(&mut pm, "oneup", &palette((45, 200, 200)), 520.0, 330.0, 50.0, 50.0, k);
    spielfigur(&mut pm, 230.0, floor_y - 50.0, 50.0, k, (225, 70, 70), &["propeller_hat", "robot_visor", "vampire_fangs"]);
    pm.save_png(out).expect("PNG speichern");
    println!("Szene gespeichert: {out}");
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut ordner: Option<PathBuf> = None;
    if let Some(i) = args.iter().position(|a| a == "--ordner") {
        ordner = args.get(i + 1).map(PathBuf::from);
        args.drain(i..(i + 2).min(args.len()));
    }
    if ordner.is_none() {
        for p in ["assets/textures", "rust/assets/textures", "../assets/textures"] {
            if std::path::Path::new(p).is_dir() { ordner = Some(PathBuf::from(p)); break; }
        }
    }
    jnd_tex::set_override_dir(ordner.clone());
    let a = |i: usize| args.get(i).cloned();
    match a(0).as_deref() {
        Some("pix") => {
            let dir = ordner.clone().unwrap_or_else(|| PathBuf::from("assets/textures"));
            let mut n = 0;
            let mut fehler = 0;
            let mut liste: Vec<_> = std::fs::read_dir(dir.join("pix")).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().map(|x| x == "pix").unwrap_or(false)).collect()).unwrap_or_default();
            liste.sort();
            for p in liste {
                let name = p.file_stem().unwrap().to_string_lossy().to_string();
                let src = std::fs::read_to_string(&p).unwrap_or_default();
                match pix::to_svg(&name, &src) {
                    Ok(svg) => { std::fs::write(dir.join(format!("{name}.svg")), svg).expect("SVG schreiben"); n += 1; }
                    Err(e) => { println!("FEHLER {e}"); fehler += 1; }
                }
            }
            println!("{n} Raster umgewandelt, {fehler} Fehler");
            if fehler > 0 { std::process::exit(1); }
        }
        Some("check") => {
            let mut fehler = 0;
            for n in PFLICHT { if !jnd_tex::exists(n) { println!("FEHLT  {n}"); fehler += 1; } }
            for n in jnd_tex::names() {
                let p = jnd_tex::check(&n);
                if p.is_empty() { println!("ok     {n}"); } else { fehler += 1; println!("FEHLER {n}: {}", p.join("; ")); }
            }
            println!("{} Texturen, {} Probleme", jnd_tex::names().len(), fehler);
            if fehler > 0 { std::process::exit(1); }
        }
        Some("bogen") => bogen(&a(1).unwrap_or("bogen.png".into())),
        Some("szene") => szene(&a(1).unwrap_or("szene.png".into()), a(2).as_deref() != Some("hell")),
        Some("figur") => {
            // figur <datei.png> <r,g,b> [cosmetic ...] [--groesse N]: Spielfigur mit Cosmetics, durchsichtig
            let out = a(1).unwrap_or("figur.png".into());
            let farbe: Rgb = a(2).and_then(|s| { let v: Vec<u8> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect(); (v.len() == 3).then(|| (v[0], v[1], v[2])) }).unwrap_or((64, 110, 235));
            let mut n = 100.0f32;
            let mut cos: Vec<String> = Vec::new();
            let mut i = 3;
            while let Some(x) = a(i) {
                if x == "--groesse" { n = a(i + 1).and_then(|v| v.parse().ok()).unwrap_or(100.0); i += 2; continue; }
                cos.push(x); i += 1;
            }
            let (w, h) = ((n * 1.7).ceil(), (n * 1.85).ceil());
            let mut pm = Pixmap::new(w as u32, h as u32).unwrap();
            let refs: Vec<&str> = cos.iter().map(|s| s.as_str()).collect();
            spielfigur(&mut pm, (w - n) / 2.0 - n * 0.05, h - n * 1.12, n, 1.0, farbe, &refs);
            pm.save_png(&out).expect("PNG speichern");
            println!("{out}");
        }
        Some("export") => {
            let (Some(n), Some(w), Some(h), Some(out)) = (a(1), a(2).and_then(|v| v.parse::<f32>().ok()), a(3).and_then(|v| v.parse::<f32>().ok()), a(4)) else {
                eprintln!("export <name> <breite> <höhe> <datei.png> [r,g,b]"); std::process::exit(2);
            };
            let farbe: Rgb = a(5).and_then(|s| { let v: Vec<u8> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect(); (v.len() == 3).then(|| (v[0], v[1], v[2])) }).unwrap_or((64, 110, 235));
            match jnd_tex::render_fit(&n, &palette(farbe), w, h, 1.0) {
                Some(r) => { r.pixmap.save_png(&out).expect("PNG speichern"); println!("{out}: {} x {} Pixel", r.pixmap.width(), r.pixmap.height()); }
                None => { eprintln!("Textur {n} nicht gefunden"); std::process::exit(1); }
            }
        }
        _ => {
            println!("Texturwerk - JumpNDodge-Texturen\n");
            println!("  pix                                   Pixel-Raster (pix/*.pix) in SVG umwandeln");
            println!("  check                                 alle Texturen prüfen");
            println!("  bogen  [datei.png]                    Vorschau-Bogen aller Texturen");
            println!("  szene  [datei.png] [hell]             Probe-Szene wie im Spiel");
            println!("  export <name> <b> <h> <datei.png> [r,g,b]");
            println!("\nOrdner: {}", ordner.map(|p| p.display().to_string()).unwrap_or("nur eingebaute".into()));
        }
    }
}
