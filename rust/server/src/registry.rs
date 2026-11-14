//! Öffentliche Server-Liste: Server melden sich beim Listen-Server (dem offiziellen) an,
//! der prüft sie selbst (fragt die Serverinfo unter der angegebenen Adresse ab) und gibt die
//! Liste an Spieler raus. Einträge verfallen, wenn sich ein Server 5 Minuten nicht meldet.

use jnd_core::net::client::{announce, query_info};
use jnd_core::net::proto::*;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

const FRESH: Duration = Duration::from_secs(300);
const MAX_ENTRIES: usize = 200;
const MAX_CHECKING: usize = 16;
/// So oft meldet sich ein gelisteter Server
pub const ANNOUNCE_EVERY: Duration = Duration::from_secs(120);

fn is_public_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(v.is_private() || v.is_loopback() || v.is_link_local() || v.is_unspecified() || v.is_broadcast()
                || v.is_multicast() || (o[0] == 100 && (o[1] & 0xc0) == 64))
        }
        IpAddr::V6(v) => {
            let s0 = v.segments()[0];
            !(v.is_loopback() || v.is_unspecified() || v.is_multicast() || (s0 & 0xfe00) == 0xfc00 || (s0 & 0xffc0) == 0xfe80)
        }
    }
}

/// Erlaubte Adresse: Hostname/IP mit Port oder ws(s)://...; `local` erlaubt auch private Adressen (Tests)
pub fn clean_address(s: &str, local: bool) -> Option<String> {
    let s = s.trim().to_lowercase();
    if s.is_empty() || s.len() > 100 { return None; }
    if !s.chars().all(|c| c.is_ascii_alphanumeric() || ".-:/[]_".contains(c)) { return None; }
    if local { return Some(s); }
    let host = s.trim_start_matches("wss://").trim_start_matches("ws://");
    let host = host.split('/').next().unwrap_or("");
    let ip_part = if let Some(rest) = host.strip_prefix('[') { rest.split(']').next().unwrap_or("") }
        else if host.matches(':').count() == 1 { host.split(':').next().unwrap_or("") }
        else { host };
    match ip_part.parse::<IpAddr>() {
        Ok(ip) => is_public_ip(&ip).then_some(s.clone()),
        Err(_) => (ip_part.contains('.') && ip_part != "localhost" && !ip_part.ends_with(".local")).then_some(s.clone()),
    }
}

pub struct Registry {
    entries: HashMap<String, (ListedServer, Instant)>,
    checking: HashSet<String>,
    tx: Sender<(String, Option<ServerInfo>)>,
    rx: Receiver<(String, Option<ServerInfo>)>,
    /// Private/lokale Adressen zulassen (nur Tests)
    pub allow_local: bool,
}

impl Registry {
    pub fn new() -> Registry {
        let (tx, rx) = channel();
        Registry { entries: HashMap::new(), checking: HashSet::new(), tx, rx, allow_local: false }
    }

    /// Anmeldung eines Servers: (angenommen?, Text)
    pub fn announce(&mut self, address: &str) -> (bool, String) {
        let Some(a) = clean_address(address, self.allow_local) else {
            return (false, "Adresse ungültig oder nicht öffentlich erreichbar".into());
        };
        let listed = self.entries.contains_key(&a);
        if !listed && self.entries.len() >= MAX_ENTRIES { return (false, "Die Liste ist voll".into()); }
        if !self.checking.contains(&a) {
            if self.checking.len() >= MAX_CHECKING { return (false, "Gerade viel los - später nochmal".into()); }
            self.checking.insert(a.clone());
            let tx = self.tx.clone();
            let addr = a.clone();
            std::thread::spawn(move || {
                let info = query_info(&addr, Duration::from_secs(4)).ok().map(|(i, _)| i);
                let _ = tx.send((addr, info));
            });
        }
        (true, if listed { "steht in der Liste".into() } else { "wird geprüft".into() })
    }

    /// Prüfergebnisse übernehmen, alte Einträge entfernen; liefert Log-Zeilen
    pub fn poll(&mut self) -> Vec<String> {
        let mut log = vec![];
        while let Ok((a, info)) = self.rx.try_recv() {
            self.checking.remove(&a);
            match info {
                Some(i) => {
                    if !self.entries.contains_key(&a) { log.push(format!("Server-Liste: {} ({}) aufgenommen", i.name, a)); }
                    self.entries.insert(a.clone(), (ListedServer { address: a, info: i }, Instant::now()));
                }
                None => {
                    if self.entries.remove(&a).is_some() { log.push(format!("Server-Liste: {} nicht erreichbar - entfernt", a)); }
                }
            }
        }
        self.entries.retain(|_, (_, t)| t.elapsed() < FRESH);
        log
    }

    /// Liste für Spieler: volle Server zuerst, höchstens SERVER_LIST_MAX
    pub fn list(&self) -> Vec<ListedServer> {
        let mut l: Vec<ListedServer> = self.entries.values().map(|(s, _)| s.clone()).collect();
        l.sort_by(|a, b| b.info.players.cmp(&a.info.players).then_with(|| a.info.name.cmp(&b.info.name)));
        l.truncate(SERVER_LIST_MAX);
        l
    }
}

/// Meldet den eigenen Server regelmäßig beim Listen-Server an (im Hintergrund)
pub struct Announcer {
    registry: String,
    address: String,
    next: Instant,
    rx: Option<Receiver<Result<(bool, String), String>>>,
    last: String,
}

impl Announcer {
    pub fn new(registry: &str, address: &str) -> Announcer {
        Announcer { registry: registry.trim().to_string(), address: address.trim().to_string(), next: Instant::now(), rx: None, last: String::new() }
    }

    /// Jeden Durchgang aufrufen; liefert eine Log-Zeile, wenn sich das Ergebnis ändert
    pub fn poll(&mut self) -> Option<String> {
        let mut out = None;
        if let Some(rx) = &self.rx {
            if let Ok(r) = rx.try_recv() {
                self.rx = None;
                let line = match r {
                    Ok((true, t)) => format!("Öffentliche Server-Liste ({}): {} als {}", self.registry, t, self.address),
                    Ok((false, t)) => format!("Öffentliche Server-Liste ({}): abgelehnt - {}", self.registry, t),
                    Err(e) => format!("Öffentliche Server-Liste ({}) nicht erreichbar: {}", self.registry, e),
                };
                if line != self.last { self.last = line.clone(); out = Some(line); }
            }
        }
        if self.rx.is_none() && Instant::now() >= self.next {
            // nach der ersten Prüfung schnell nachfragen, ob man drin ist; danach alle 2 Minuten
            self.next = Instant::now() + if self.last.is_empty() { Duration::from_secs(15) } else { ANNOUNCE_EVERY };
            let (tx, rx) = channel();
            let (reg, addr) = (self.registry.clone(), self.address.clone());
            std::thread::spawn(move || { let _ = tx.send(announce(&reg, &addr, Duration::from_secs(5))); });
            self.rx = Some(rx);
        }
        out
    }
}
