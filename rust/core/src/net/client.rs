//! Client-Seite: Verbindung zu einem JumpNDodge-Server (vom Spiel und von Tests benutzt).
//!
//! Wege: UDP (Standard, schnellste Variante) oder WebSocket (`wss://host`, `ws://host:port` - z.B. hinter
//! einem Cloudflare Tunnel). Antwortet ein Hostname nicht per UDP, wird automatisch `wss://host` versucht.

use super::channel::{recv_all, send_raw, Lossy, Packet, Peer, Wire};
use super::proto::*;
use super::ws::{self, WsLink};
use crate::sim2p::Input;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

/// "1.2.3.4", "1.2.3.4:47800", "play.example.com", "[::1]:47800" -> Socket-Adresse
pub fn parse_addr(s: &str) -> Result<SocketAddr, String> {
    let s = s.trim();
    if s.is_empty() { return Err("Keine Adresse angegeben".into()); }
    let with_port = if s.parse::<SocketAddr>().is_ok() { s.to_string() }
        else if s.parse::<std::net::Ipv6Addr>().is_ok() { format!("[{}]:{}", s, DEFAULT_PORT) }
        else if s.rsplit_once(':').map(|(_, p)| p.parse::<u16>().is_ok()).unwrap_or(false) && s.matches(':').count() == 1 { s.to_string() }
        else { format!("{}:{}", s, DEFAULT_PORT) };
    with_port.to_socket_addrs().map_err(|e| format!("Adresse nicht gefunden: {}", e))?
        .next().ok_or_else(|| "Adresse nicht gefunden".into())
}

fn is_ws_url(s: &str) -> bool { let s = s.trim(); s.starts_with("ws://") || s.starts_with("wss://") }

/// Reiner Hostname ohne Port (z.B. play.mert.win) -> darf auf wss:// ausweichen
fn ws_fallback(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() || s.contains(':') || s.parse::<std::net::IpAddr>().is_ok() || !s.contains('.') { return None; }
    if s.chars().all(|c| c.is_ascii_digit() || c == '.') { return None; }
    Some(format!("wss://{}", s))
}

fn bind_for(addr: &SocketAddr) -> Result<UdpSocket, String> {
    let local = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
    let s = UdpSocket::bind(local).map_err(|e| format!("Netzwerkfehler: {}", e))?;
    s.set_nonblocking(true).map_err(|e| e.to_string())?;
    Ok(s)
}

/// Verbindungsweg
enum Link { Udp(UdpSocket, SocketAddr), Ws(WsLink) }

impl Link {
    fn peer_addr(&self) -> SocketAddr { match self { Link::Udp(_, a) => *a, Link::Ws(w) => w.addr } }
    fn wire(&self) -> &dyn Wire { match self { Link::Udp(s, _) => s, Link::Ws(w) => w } }
    fn recv(&self, mut f: impl FnMut(Packet)) {
        match self {
            Link::Udp(s, a) => recv_all(s, |from, p| if from == *a { f(p) }),
            Link::Ws(w) => w.recv_all(|_, p| f(p)),
        }
    }
    fn open(addr: &str, timeout: Duration) -> Result<Link, String> {
        if is_ws_url(addr) { return Ok(Link::Ws(ws::connect(addr.trim(), timeout)?)); }
        let a = parse_addr(addr)?;
        Ok(Link::Udp(bind_for(&a)?, a))
    }
}

/// Unverbundene Anfrage schicken (alle 250 ms wiederholt), bis `pick` eine Antwort erkennt
fn ask_over<T>(link: &Link, msg: &[u8], timeout: Duration, pick: &mut dyn FnMut(ServerMsg) -> Option<T>) -> Option<(T, Duration)> {
    let start = Instant::now();
    let mut last = start - Duration::from_secs(1);
    while start.elapsed() < timeout {
        if last.elapsed() >= Duration::from_millis(250) {
            send_raw(link.wire(), link.peer_addr(), &Packet::Unreliable { ack: 0, data: msg.to_vec() }, Lossy::default());
            last = Instant::now();
        }
        let mut res = None;
        link.recv(|p| if let Packet::Unreliable { data, .. } = p {
            if res.is_none() { if let Some(m) = decode::<ServerMsg>(&data) { res = pick(m); } }
        });
        if let Some(v) = res { return Some((v, start.elapsed())); }
        if let Link::Ws(w) = link { if w.is_closed() { return None; } }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

/// Unverbundene Anfrage mit Ausweichweg über wss:// (wie beim Verbinden)
fn ask<T>(addr: &str, msg: &ClientMsg, timeout: Duration, mut pick: impl FnMut(ServerMsg) -> Option<T>) -> Result<(T, Duration), String> {
    let data = encode(msg);
    let link = Link::open(addr, timeout)?;
    let r = ask_over(&link, &data, timeout, &mut pick);
    if let Link::Ws(w) = &link { w.close(); }
    if let Some(r) = r { return Ok(r); }
    if let (Link::Udp(..), Some(url)) = (&link, ws_fallback(addr)) {
        let start = Instant::now();
        if let Ok(l) = Link::open(&url, timeout) {
            let r = ask_over(&l, &data, timeout, &mut pick);
            if let Link::Ws(w) = &l { w.close(); }
            if let Some((v, _)) = r { return Ok((v, start.elapsed())); }
        }
    }
    Err("Server antwortet nicht".into())
}

/// Serverinfo für die Serverliste abfragen (ohne Verbindung)
pub fn query_info(addr: &str, timeout: Duration) -> Result<(ServerInfo, Duration), String> {
    ask(addr, &ClientMsg::Info, timeout, |m| if let ServerMsg::Info(i) = m { Some(i) } else { None })
}

/// Öffentliche Server-Liste beim Listen-Server (normalerweise der offizielle) abfragen
pub fn fetch_server_list(registry: &str, timeout: Duration) -> Result<Vec<ListedServer>, String> {
    ask(registry, &ClientMsg::ListServers, timeout, |m| if let ServerMsg::ServerList(l) = m { Some(l) } else { None }).map(|(l, _)| l)
}

/// Einen Server in der öffentlichen Liste anmelden; Antwort (angenommen?, Text)
pub fn announce(registry: &str, address: &str, timeout: Duration) -> Result<(bool, String), String> {
    ask(registry, &ClientMsg::Announce { address: address.into() }, timeout, |m| if let ServerMsg::Announced { ok, text } = m { Some((ok, text)) } else { None }).map(|(r, _)| r)
}

/// Server im eigenen Netzwerk suchen (Broadcast auf den Standard-Port): (Adresse, Info, Ping)
pub fn lan_scan(timeout: Duration) -> Vec<(String, ServerInfo, Duration)> {
    let Ok(s) = UdpSocket::bind("0.0.0.0:0") else { return vec![]; };
    let _ = s.set_broadcast(true);
    let _ = s.set_nonblocking(true);
    let data = super::channel::encode_packet(&Packet::Unreliable { ack: 0, data: encode(&ClientMsg::Info) });
    let start = Instant::now();
    let mut last = start - Duration::from_secs(1);
    let mut found: Vec<(String, ServerInfo, Duration)> = vec![];
    let mut buf = [0u8; 2048];
    while start.elapsed() < timeout {
        if last.elapsed() >= Duration::from_millis(400) {
            let _ = s.send_to(&data, ("255.255.255.255", DEFAULT_PORT));
            last = Instant::now();
        }
        while let Ok((n, from)) = s.recv_from(&mut buf) {
            let Some(Packet::Unreliable { data, .. }) = super::channel::decode_packet(&buf[..n]) else { continue; };
            let Some(ServerMsg::Info(i)) = decode::<ServerMsg>(&data) else { continue; };
            let a = if from.port() == DEFAULT_PORT { from.ip().to_string() } else { from.to_string() };
            if !found.iter().any(|f| f.0 == a) { found.push((a, i, start.elapsed())); }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    found
}

#[derive(Clone, Debug)]
pub struct ConnectOptions {
    pub name: String,
    pub password: String,
    pub game_version: String,
    pub proto: u32,
    pub timeout: Duration,
    pub loss: Lossy,
}
impl ConnectOptions {
    pub fn new(name: &str) -> ConnectOptions {
        ConnectOptions { name: name.into(), password: String::new(), game_version: GAME_VERSION.into(), proto: PROTO_VERSION, timeout: Duration::from_secs(5), loss: Lossy::default() }
    }
}

pub struct Client {
    link: Link,
    peer: Peer,
    pub client_id: u32,
    pub server_name: String,
    pub motd: String,
    /// Nachrichten, die schon während des Verbindens kamen
    pending: Vec<ServerMsg>,
}

/// Ergebnis eines Verbindungsversuchs: Ok, abgelehnt (Grund) oder keine Antwort
enum Try { Ok(Client), Rejected(String), NoAnswer(String) }

impl Client {
    pub fn connect(addr: &str, opt: ConnectOptions) -> Result<Client, String> {
        match Client::try_connect(addr, &opt) {
            Try::Ok(c) => Ok(c),
            Try::Rejected(r) => Err(r),
            Try::NoAnswer(e) => {
                // Hostname ohne Antwort per UDP: vielleicht nur per WebSocket erreichbar (Cloudflare Tunnel)
                if !is_ws_url(addr) {
                    if let Some(url) = ws_fallback(addr) {
                        match Client::try_connect(&url, &opt) { Try::Ok(c) => return Ok(c), Try::Rejected(r) => return Err(r), Try::NoAnswer(_) => {} }
                    }
                }
                Err(e)
            }
        }
    }

    fn try_connect(addr: &str, opt: &ConnectOptions) -> Try {
        let link = match Link::open(addr, opt.timeout) { Ok(l) => l, Err(e) => return Try::NoAnswer(e) };
        let mut peer = Peer::new(link.peer_addr());
        peer.loss = opt.loss;
        peer.send_reliable(link.wire(), encode(&ClientMsg::Hello { proto: opt.proto, game_version: opt.game_version.clone(), name: opt.name.clone(), password: opt.password.clone() }));
        let start = Instant::now();
        let mut pending = vec![];
        while start.elapsed() < opt.timeout {
            let mut got = vec![];
            link.recv(|p| got.extend(peer.on_packet(p)));
            peer.update(link.wire());
            let mut welcome = None;
            for d in got {
                match decode::<ServerMsg>(&d) {
                    // Was im selben Schwung nach dem Willkommen kam (z.B. der Chat-Verlauf), nicht verwerfen
                    Some(ServerMsg::Welcome { client_id, server_name, motd }) if welcome.is_none() => {
                        welcome = Some((client_id, server_name, motd));
                    }
                    Some(ServerMsg::Reject { reason }) => {
                        // Bestätigung schicken, damit der Server nicht weiter wiederholt
                        peer.update(link.wire());
                        return Try::Rejected(reason);
                    }
                    Some(m) => pending.push(m),
                    None => {}
                }
            }
            if let Some((client_id, server_name, motd)) = welcome {
                return Try::Ok(Client { link, peer, client_id, server_name, motd, pending });
            }
            if let Link::Ws(w) = &link { if w.is_closed() { break; } }
            std::thread::sleep(Duration::from_millis(5));
        }
        Try::NoAnswer("Server antwortet nicht (Adresse, Port und Portweiterleitung prüfen)".into())
    }

    pub fn server_addr(&self) -> SocketAddr { self.link.peer_addr() }
    /// Läuft die Verbindung über WebSocket (z.B. Cloudflare Tunnel)?
    pub fn via_websocket(&self) -> bool { matches!(self.link, Link::Ws(_)) }

    /// Zuverlässig senden (Lobby-Aktionen)
    pub fn send(&mut self, m: &ClientMsg) { self.peer.send_reliable(self.link.wire(), encode(m)); }

    /// Eingabe für einen Frame (unzuverlässig, jeden Frame neu)
    pub fn send_input(&mut self, frame: u64, input: Input) {
        self.peer.send_unreliable(self.link.wire(), encode(&ClientMsg::Input { frame, input }));
    }

    /// Empfangen + Wiederholen + Keepalive. Nicht blockierend, jeden Frame aufrufen.
    pub fn poll(&mut self) -> Vec<ServerMsg> {
        self.pump();
        std::mem::take(&mut self.pending)
    }

    /// Netzwerk bedienen (Bestätigungen, Wiederholungen, Keepalive) und eingehende Nachrichten
    /// nur zwischenspeichern - für Zeiten, in denen niemand die Nachrichten abholt.
    /// Ohne das würden verlorene Pakete nicht wiederholt und der Server trennt nach 10 s.
    pub fn pump(&mut self) {
        let mut out = std::mem::take(&mut self.pending);
        let peer = &mut self.peer;
        let mut raw = vec![];
        self.link.recv(|p| raw.extend(peer.on_packet(p)));
        for d in raw { if let Some(m) = decode::<ServerMsg>(&d) { out.push(m); } }
        if self.peer.last_sent.elapsed() >= Duration::from_secs(1) {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
            self.peer.send_unreliable(self.link.wire(), encode(&ClientMsg::Ping { t }));
        }
        self.peer.update(self.link.wire());
        if let Link::Ws(w) = &self.link { if w.is_closed() { self.peer.dead = true; } }
        self.pending = out;
    }

    /// Verbindung verloren (Server seit 10 s stumm)
    pub fn is_dead(&self) -> bool { self.peer.dead }

    /// Sauber trennen (Server erfährt es sofort statt erst nach dem Timeout)
    pub fn disconnect(mut self) {
        let bye = encode(&ClientMsg::Bye);
        self.peer.send_reliable(self.link.wire(), bye.clone());
        for _ in 0..3 { self.peer.send_unreliable(self.link.wire(), bye.clone()); }
        if let Link::Ws(w) = &self.link { w.close(); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adressen() {
        assert_eq!(parse_addr("127.0.0.1").unwrap().port(), DEFAULT_PORT);
        assert_eq!(parse_addr("127.0.0.1:1234").unwrap().port(), 1234);
        assert_eq!(parse_addr("[::1]:5555").unwrap().port(), 5555);
        assert_eq!(parse_addr("::1").unwrap().port(), DEFAULT_PORT);
        assert_eq!(parse_addr("localhost:777").unwrap().port(), 777);
        assert!(parse_addr("").is_err());
    }
    #[test]
    fn websocket_rueckfall_nur_fuer_hostnamen() {
        assert_eq!(ws_fallback("play.mert.win").as_deref(), Some("wss://play.mert.win"));
        assert_eq!(ws_fallback("play.mert.win:47800"), None);
        assert_eq!(ws_fallback("192.168.0.20"), None);
        assert_eq!(ws_fallback("localhost"), None);
        assert!(is_ws_url("wss://x.y") && is_ws_url("ws://1.2.3.4:47801") && !is_ws_url("x.y"));
    }
}
