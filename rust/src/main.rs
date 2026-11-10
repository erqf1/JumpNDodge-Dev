#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod crypto;
mod data;
mod game;
mod pg;
mod util;

use pg::*;

fn main() {
    // Vorläufiger Selbsttest der pygame-Schicht
    if let Some(p) = std::env::args().nth(1).filter(|a| a == "--decrypt-test") {
        let _ = p;
        let path = std::env::args().nth(2).unwrap();
        match crypto::load_encrypted_json(std::path::Path::new(&path)) {
            Some(v) => println!("OK keys: {:?}", v.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>())),
            None => println!("FEHLER"),
        }
        return;
    }
    let mut d = Display::new("JumpNDodge", 800, 600, true, None);
    let base = Surface::new(800, 600);
    let font = Font::new(64);
    let small = Font::new(26);
    let mut clock = Clock::new();
    for frame in 0..30 {
        for e in d.event_get() { if let Event::Quit = e { return; } }
        base.fill((30, 30, 40));
        draw::rect(&base, (70, 70, 80), Rect::new(100, 100, 300, 60), 0, 16);
        draw::rect(&base, (255, 215, 0), Rect::new(100, 100, 300, 60), 3, 16);
        draw::circle(&base, (100, 150, 200), (500, 300), 40, 2);
        draw::polygon(&base, (200, 80, 80), &[(600, 400), (700, 450), (620, 520)], 0);
        let t = font.render("JumpNDodge Größe", true, (220, 220, 220));
        base.blit(&t, (400 - t.get_width() / 2, 30));
        let t2 = small.render(&format!("Frame {frame} – Привет Türkçe", ), true, (255, 255, 255));
        base.blit(&t2, (110, 120));
        d.present(&base);
        clock.tick(60);
    }
    base.save_png(&std::env::var("JND_SHOT").unwrap_or_else(|_| "smoke.png".into()));
}
