#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! JumpNDodge - Rust-Port von JumpNDodge.py (pygame).

mod crypto;
mod data;
mod game;
mod pg;
mod updater;
mod util;

fn main() {
    // Hilfsmodus: verschlüsselten Spielstand prüfen (nur Schlüssel-Liste, keine Inhalte)
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|a| a == "--decrypt-test").unwrap_or(false) {
        let path = args.get(2).cloned().unwrap_or_default();
        match crypto::load_encrypted_json_ro(std::path::Path::new(&path)) {
            Some(v) => println!("OK keys: {:?}", v.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>())),
            None => println!("FEHLER"),
        }
        return;
    }
    // Bei einem Absturz noch ausstehende Spielstände schreiben
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        default_hook(info);
        crypto::flush_saves();
    }));
    let mut g = game::state::G::new();
    g.main_loop();
    crypto::flush_saves();
}
