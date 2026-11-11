//! JumpNDodge-Server - Startprogramm.
//!
//! Aufruf: JumpNDodge-Server [--config <datei>] [--port <port>]
//! Ohne Angabe liegt server.json neben dem Programm und wird beim ersten Start angelegt.

use jnd_core::net::proto::{GAME_VERSION, PROTO_VERSION};
use jnd_server::config::Config;
use jnd_server::server::Server;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| PathBuf::from("."))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("JumpNDodge-Server {GAME_VERSION}\n\nAufruf: JumpNDodge-Server [--config <datei>] [--port <port>]\n\nOhne --config wird server.json neben dem Programm benutzt (beim ersten Start angelegt).\nUmgebungsvariablen (überschreiben server.json): JND_SERVER_NAME, JND_SERVER_MOTD,\nJND_SERVER_PASSWORD, JND_SERVER_PORT, JND_SERVER_BIND, JND_SERVER_MAX_ROOMS");
        return;
    }
    let arg = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let cfg_path = arg("--config").map(PathBuf::from).unwrap_or_else(|| exe_dir().join("server.json"));
    let mut cfg = match Config::load_or_create(&cfg_path) {
        Ok(c) => c,
        Err(e) => { eprintln!("Fehler: {e}"); std::process::exit(1); }
    };
    // Umgebungsvariablen (praktisch für Docker/CasaOS) überschreiben server.json
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    if let Some(v) = env("JND_SERVER_NAME") { cfg.name = v; }
    if let Some(v) = env("JND_SERVER_MOTD") { cfg.motd = v; }
    if let Some(v) = env("JND_SERVER_PASSWORD") { cfg.password = v; }
    if let Some(v) = env("JND_SERVER_BIND") { cfg.bind = v; }
    if let Some(v) = env("JND_SERVER_PORT").and_then(|v| v.parse().ok()) { cfg.port = v; }
    if let Some(v) = env("JND_SERVER_MAX_ROOMS").and_then(|v| v.parse().ok()) { cfg.max_rooms = v; }
    if let Some(p) = arg("--port") {
        match p.parse() { Ok(p) => cfg.port = p, Err(_) => { eprintln!("Fehler: ungültiger Port {p}"); std::process::exit(1); } }
    }

    let log_path = if cfg.log_file.is_empty() { None } else {
        let p = PathBuf::from(&cfg.log_file);
        Some(if p.is_absolute() { p } else { cfg_path.parent().map(|d| d.join(&p)).unwrap_or(p) })
    };
    let logger = Box::new(move |s: &str| {
        let line = format!("[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), s);
        println!("{line}");
        if let Some(p) = &log_path {
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) { let _ = writeln!(f, "{line}"); }
        }
    });

    let port = cfg.port;
    let mut server = match Server::bind(cfg, logger) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Fehler: Port {port} (UDP) konnte nicht geöffnet werden: {e}\nLäuft der Server schon? Sonst in server.json einen anderen Port eintragen.");
            std::process::exit(1);
        }
    };
    println!("=====================================================");
    println!(" JumpNDodge-Server {GAME_VERSION} (Protokoll {PROTO_VERSION})");
    println!(" Name:   {}", server.cfg.name);
    println!(" Port:   {port} (UDP)");
    println!(" Config: {}", cfg_path.display());
    println!("-----------------------------------------------------");
    println!(" Spieler im selben Netz verbinden sich mit deiner lokalen IP.");
    println!(" Für Spieler aus dem Internet: im Router UDP-Port {port}");
    println!(" an diesen PC weiterleiten und deine öffentliche IP teilen.");
    println!(" Beenden: Strg+C");
    println!("=====================================================");
    let stop = AtomicBool::new(false);
    server.run(&stop);
}
