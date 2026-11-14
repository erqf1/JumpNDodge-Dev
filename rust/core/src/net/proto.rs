//! Nachrichten zwischen Spiel (Client) und Multiplayer-Server.
//! Wird per bincode kodiert.
//!
//! Versionen: Spiel und Server dürfen verschiedene Spielversionen haben (ältere/neuere passen zusammen).
//! Gesperrt wird nur über PROTO_VERSION - die von Hand erhöhen, wenn sich Nachrichten oder Spielregeln
//! (sim2p) so ändern, dass alte und neue Versionen nicht mehr zusammen spielen können. Neue Nachrichten
//! immer HINTEN an die enums anhängen und nur senden, wenn die Gegenseite sie kennt.

use crate::sim2p::{Input, ObstacleKind, SimEvent, Sim2p, Winner};
use serde::{Deserialize, Serialize};

pub const PROTO_VERSION: u32 = 2;
/// Spielversion (nur zur Anzeige; ob es passt, entscheidet PROTO_VERSION)
pub const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_PORT: u16 = 47800;
/// Spielfeld online: so groß wie der kleinere Bildschirm der beiden Spieler, in echten Pixeln wie im
/// Einzelspieler (Figuren und Sprünge gleich groß, nichts wird hochskaliert). Ohne Angabe (alte Spiele) 800x600.
pub const FIELD_W: i32 = 800;
pub const FIELD_H: i32 = 600;
pub const FIELD_MIN: (i32, i32) = (800, 600);
pub const FIELD_MAX: (i32, i32) = (3840, 2160);

/// Feldgröße aus den Fenstergrößen der beiden Spieler. None = ein Spieler hat nichts gemeldet -> 800x600.
pub fn field_size_for(a: Option<(u32, u32)>, b: Option<(u32, u32)>) -> (i32, i32) {
    match (a, b) {
        (Some(a), Some(b)) => (
            (a.0.min(b.0) as i32).clamp(FIELD_MIN.0, FIELD_MAX.0),
            (a.1.min(b.1) as i32).clamp(FIELD_MIN.1, FIELD_MAX.1),
        ),
        _ => (FIELD_W, FIELD_H),
    }
}
pub const TICK_HZ: u64 = 60;
/// Snapshot an die Clients jeden n-ten Tick (60 Hz / 2 = 30 Snapshots pro Sekunde)
pub const SNAPSHOT_EVERY: u64 = 2;
/// Ereignisse der letzten n Frames wandern in jedem Snapshot mit (gegen Paketverlust)
pub const EVENT_HISTORY_FRAMES: u64 = 60;
pub const COUNTDOWN_SECS: u8 = 3;
pub const MAX_NAME_LEN: usize = 20;
/// Server-Chat: maximale Länge einer Nachricht (Zeichen)
pub const MAX_CHAT_LEN: usize = 120;
/// Server-Chat: so viele Nachrichten am Stück, danach eine pro CHAT_REFILL_SECS
pub const CHAT_BURST: f64 = 5.0;
pub const CHAT_REFILL_SECS: f64 = 2.0;
pub const DIFFICULTIES: [&str; 8] = ["Leicht", "Mittel", "Schwer", "Hardcore", "Unmöglich", "Albtraum", "Hölle", "Dynamisch"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode { Vs, Coop }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// Unverbundene Abfrage für die Serverliste (Name, Spieler, Version)
    Info,
    Hello { proto: u32, game_version: String, name: String, password: String },
    ListRooms,
    CreateRoom { mode: Mode, difficulty: String },
    JoinRoom { code: String },
    LeaveRoom,
    Ready(bool),
    /// Eingabe des Spielers für einen Frame (unzuverlässig, jeden Frame neu gesendet)
    Input { frame: u64, input: Input },
    Ping { t: u64 },
    Bye,
    /// Chat-Nachricht an alle, die gerade auf dem Server sind (wird nicht gespeichert)
    Chat { text: String },
    /// Bildschirmgröße des Spielers (für die Feldbreite). Alte Server kennen das nicht und ignorieren es.
    Screen { w: u32, h: u32 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub motd: String,
    pub game_version: String,
    pub proto: u32,
    pub rooms: u32,
    pub max_rooms: u32,
    pub players: u32,
    pub password: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoomInfo {
    pub code: String,
    pub mode: Mode,
    pub difficulty: String,
    pub players: u8,
    pub host: String,
    pub playing: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyPlayer { pub name: String, pub ready: bool }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerSnap {
    pub x: i32,
    pub y: i32,
    pub vy: i32,
    pub jumping: bool,
    pub alive: bool,
    pub lives: i64,
    pub invuln: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObstacleSnap { pub x: i32, pub y: i32, pub w: i32, pub h: i32, pub kind: ObstacleKind }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub frame: u64,
    pub score: i64,
    pub speed: f64,
    pub players: [PlayerSnap; 2],
    pub obstacles: Vec<ObstacleSnap>,
    pub powerups: Vec<(i32, i32)>,
    /// (Frame, Ereignis) der letzten EVENT_HISTORY_FRAMES Frames - Client filtert Doppelte
    pub events: Vec<(u64, SimEvent)>,
    /// Letzter Eingabe-Frame, den der Server von DIR bekommen hat
    pub input_ack: u64,
}

impl Snapshot {
    pub fn from_sim(sim: &Sim2p, frame: u64, events: Vec<(u64, SimEvent)>, input_ack: u64) -> Snapshot {
        let ps = |i: usize| {
            let p = &sim.players[i];
            PlayerSnap { x: p.rect.x, y: p.rect.y, vy: p.vy, jumping: p.jumping, alive: p.alive, lives: p.lives, invuln: p.invuln }
        };
        let obstacles = sim.obstacles.iter().chain(sim.top_obstacles.iter())
            .map(|o| ObstacleSnap { x: o.rect.x, y: o.rect.y, w: o.rect.w, h: o.rect.h, kind: o.kind }).collect();
        Snapshot {
            frame, score: sim.score, speed: sim.speed, players: [ps(0), ps(1)], obstacles,
            powerups: sim.powerups.iter().map(|p| (p.x, p.y)).collect(), events, input_ack,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    Info(ServerInfo),
    Welcome { client_id: u32, server_name: String, motd: String },
    /// Verbindung abgelehnt (falsche Version, falsches Passwort, Server voll ...)
    Reject { reason: String },
    RoomList(Vec<RoomInfo>),
    /// Aktueller Raum: `you` = eigener Platz (0 = P1, 1 = P2)
    RoomState { code: String, mode: Mode, difficulty: String, you: u8, players: [Option<LobbyPlayer>; 2] },
    LeftRoom { reason: String },
    Countdown { seconds: u8 },
    Start { seed: u64, mode: Mode, difficulty: String, width: i32, height: i32 },
    Snapshot(Snapshot),
    GameOver { winner: Option<Winner>, score: i64, frames: u64, reason: String },
    Error(String),
    Pong { t: u64 },
    /// Chat live weitergereicht. `from_id` 0 = Hinweis vom Server (beigetreten, zu schnell ...)
    Chat { from_id: u32, from: String, text: String },
}

pub fn encode<T: Serialize>(m: &T) -> Vec<u8> {
    use bincode::Options;
    bincode::DefaultOptions::new().with_fixint_encoding().allow_trailing_bytes().serialize(m).unwrap_or_default()
}
pub fn decode<'a, T: Deserialize<'a>>(b: &'a [u8]) -> Option<T> {
    use bincode::Options;
    // Größenlimit gegen manipulierte Pakete
    bincode::DefaultOptions::new().with_fixint_encoding().allow_trailing_bytes().with_limit(64 * 1024).deserialize(b).ok()
}

/// Chat-Text bereinigen: keine Steuerzeichen, Leerraum zusammengefasst, max. MAX_CHAT_LEN Zeichen.
/// Leer = nicht senden.
pub fn clean_chat(t: &str) -> String {
    let mut out = String::new();
    for w in t.split_whitespace() {
        let w: String = w.chars().filter(|c| !c.is_control() && !matches!(*c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}')).collect();
        if w.is_empty() { continue; }
        if !out.is_empty() { out.push(' '); }
        out.push_str(&w);
    }
    out.chars().take(MAX_CHAT_LEN).collect::<String>().trim_end().to_string()
}

/// Spielername bereinigen: sichtbare ASCII-Zeichen, max. 20, sonst "Spieler"
pub fn clean_name(n: &str) -> String {
    let s: String = n.chars().filter(|c| (' '..='~').contains(c)).take(MAX_NAME_LEN).collect();
    let s = s.trim().to_string();
    if s.is_empty() { "Spieler".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chat_text_bereinigen() {
        assert_eq!(clean_chat("  hallo 
	 welt  "), "hallo welt");
        assert_eq!(clean_chat("\u{202E}böse\u{200B}"), "böse");
        assert_eq!(clean_chat("   "), "");
        assert_eq!(clean_chat(&"x".repeat(500)).chars().count(), MAX_CHAT_LEN);
        assert_eq!(clean_chat("Grüße 👋"), "Grüße 👋");
    }
    #[test]
    fn feldgroesse() {
        assert_eq!(field_size_for(None, Some((1920, 1080))), (800, 600));
        assert_eq!(field_size_for(Some((1920, 1080)), Some((1920, 1080))), (1920, 1080));
        assert_eq!(field_size_for(Some((2560, 1080)), Some((1920, 1200))), (1920, 1080));
        assert_eq!(field_size_for(Some((400, 700)), Some((1920, 1080))), (800, 700));
        assert_eq!(field_size_for(Some((9000, 9000)), Some((8000, 8000))), (3840, 2160));
    }
    #[test]
    fn chat_kodieren() {
        let m = ServerMsg::Chat { from_id: 3, from: "Anna".into(), text: "hi".into() };
        assert_eq!(decode::<ServerMsg>(&encode(&m)), Some(m));
        let c = ClientMsg::Chat { text: "yo".into() };
        assert_eq!(decode::<ClientMsg>(&encode(&c)), Some(c));
    }
}
