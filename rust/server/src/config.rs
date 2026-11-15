//! server.json - wird beim ersten Start mit Standardwerten angelegt.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Name in der Serverliste
    pub name: String,
    /// Begrüßungstext für Spieler
    pub motd: String,
    /// UDP-Port (im Router weiterleiten!)
    pub port: u16,
    /// Netzwerk-Adresse zum Lauschen ("0.0.0.0" = alle IPv4, "::" = alle IPv6)
    pub bind: String,
    /// Leer = jeder darf rein
    pub password: String,
    /// Maximale Anzahl gleichzeitiger Räume (je 2 Spieler)
    pub max_rooms: u32,
    /// Log-Datei (leer = nur Konsole)
    pub log_file: String,
    /// Server-Chat an/aus
    pub chat: bool,
    /// Wie lange Chat-Nachrichten aufgehoben werden (Stunden). Neue Spieler sehen diesen Verlauf.
    /// 0 = nur live, nichts wird gespeichert. Gespeichert in chat.json neben der server.json.
    pub chat_history_hours: u32,
    /// Zusätzlicher WebSocket-Port (TCP) für Cloudflare Tunnel u.ä. (0 = aus). Das Spiel nutzt ihn mit wss://<domain>.
    pub ws_port: u16,
    /// Neue Server-Versionen von GitHub holen und neu starten, sobald niemand verbunden ist
    pub auto_update: bool,
    /// Wie oft nach Updates gesucht wird (Minuten)
    pub update_check_minutes: u32,
    /// In der öffentlichen Server-Liste im Spiel erscheinen (braucht public_address)
    pub listed: bool,
    /// Adresse, unter der Spieler den Server erreichen (z.B. "jnd.example.com" oder "wss://jnd.example.com")
    pub public_address: String,
    /// Wo die Liste geführt wird (der offizielle Server)
    pub registry: String,
    /// Selbst eine Server-Liste führen (andere Server dürfen sich hier anmelden)
    pub server_list: bool,
    /// Replays annehmen (Bestläufe für Freundes-Geister, geteilte Replays) - über den WebSocket-Port
    pub replays: bool,
    /// Cloudflare Tunnel (eingebaut): Token aus dem Cloudflare-Dashboard, leer = aus.
    /// Im Dashboard zeigt der Hostname auf HTTP -> localhost:<ws_port>. Geheim halten!
    pub tunnel_token: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            name: "JumpNDodge Server".into(),
            motd: "Viel Spaß!".into(),
            port: jnd_core::net::proto::DEFAULT_PORT,
            bind: "0.0.0.0".into(),
            password: String::new(),
            max_rooms: 16,
            log_file: "server.log".into(),
            chat: true,
            chat_history_hours: 24,
            ws_port: 47801,
            auto_update: true,
            update_check_minutes: 60,
            listed: false,
            public_address: String::new(),
            registry: "play.mert.win".into(),
            server_list: true,
            replays: true,
            tunnel_token: String::new(),
        }
    }
}

impl Config {
    /// Lädt die Config; fehlt sie, wird sie mit Standardwerten angelegt.
    pub fn load_or_create(path: &Path) -> Result<Config, String> {
        if path.exists() {
            let s = std::fs::read_to_string(path).map_err(|e| format!("{} nicht lesbar: {}", path.display(), e))?;
            serde_json::from_str(&s).map_err(|e| format!("{} fehlerhaft: {}", path.display(), e))
        } else {
            let c = Config::default();
            let s = serde_json::to_string_pretty(&c).unwrap_or_default();
            // Nicht beschreibbar (z.B. Docker-Ordner ohne Rechte): mit Standardwerten weiterlaufen statt abzustürzen
            if let Err(e) = std::fs::write(path, s) {
                eprintln!("Hinweis: {} konnte nicht angelegt werden ({}) - Server läuft mit Standardwerten.", path.display(), e);
            }
            Ok(c)
        }
    }
}
