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
            std::fs::write(path, s).map_err(|e| format!("{} konnte nicht angelegt werden: {}", path.display(), e))?;
            Ok(c)
        }
    }
}
