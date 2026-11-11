//! Client-Seite: Verbindung zu einem JumpNDodge-Server (vom Spiel und von Tests benutzt).

use super::channel::{recv_all, send_raw, Lossy, Packet, Peer};
use super::proto::*;
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

fn bind_for(addr: &SocketAddr) -> Result<UdpSocket, String> {
    let local = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
    let s = UdpSocket::bind(local).map_err(|e| format!("Netzwerkfehler: {}", e))?;
    s.set_nonblocking(true).map_err(|e| e.to_string())?;
    Ok(s)
}

/// Serverinfo für die Serverliste abfragen (ohne Verbindung)
pub fn query_info(addr: &str, timeout: Duration) -> Result<(ServerInfo, Duration), String> {
    let a = parse_addr(addr)?;
    let sock = bind_for(&a)?;
    let start = Instant::now();
    let mut last = start - Duration::from_secs(1);
    while start.elapsed() < timeout {
        if last.elapsed() >= Duration::from_millis(250) {
            send_raw(&sock, a, &Packet::Unreliable { ack: 0, data: encode(&ClientMsg::Info) }, Lossy::default());
            last = Instant::now();
        }
        let mut res = None;
        recv_all(&sock, |from, p| {
            if from != a { return; }
            if let Packet::Unreliable { data, .. } = p {
                if let Some(ServerMsg::Info(i)) = decode::<ServerMsg>(&data) { res = Some(i); }
            }
        });
        if let Some(i) = res { return Ok((i, start.elapsed())); }
        std::thread::sleep(Duration::from_millis(5));
    }
    Err("Server antwortet nicht".into())
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
    sock: UdpSocket,
    peer: Peer,
    pub client_id: u32,
    pub server_name: String,
    pub motd: String,
    /// Nachrichten, die schon während des Verbindens kamen
    pending: Vec<ServerMsg>,
}

impl Client {
    pub fn connect(addr: &str, opt: ConnectOptions) -> Result<Client, String> {
        let a = parse_addr(addr)?;
        let sock = bind_for(&a)?;
        let mut peer = Peer::new(a);
        peer.loss = opt.loss;
        peer.send_reliable(&sock, encode(&ClientMsg::Hello { proto: opt.proto, game_version: opt.game_version.clone(), name: opt.name.clone(), password: opt.password.clone() }));
        let start = Instant::now();
        let mut pending = vec![];
        while start.elapsed() < opt.timeout {
            let mut got = vec![];
            recv_all(&sock, |from, p| if from == a { got.extend(peer.on_packet(p)); });
            peer.update(&sock);
            for d in got {
                match decode::<ServerMsg>(&d) {
                    Some(ServerMsg::Welcome { client_id, server_name, motd }) => {
                        return Ok(Client { sock, peer, client_id, server_name, motd, pending });
                    }
                    Some(ServerMsg::Reject { reason }) => {
                        // Bestätigung schicken, damit der Server nicht weiter wiederholt
                        peer.update(&sock);
                        return Err(reason);
                    }
                    Some(m) => pending.push(m),
                    None => {}
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err("Server antwortet nicht (Adresse, Port und Portweiterleitung prüfen)".into())
    }

    pub fn server_addr(&self) -> SocketAddr { self.peer.addr }

    /// Zuverlässig senden (Lobby-Aktionen)
    pub fn send(&mut self, m: &ClientMsg) { self.peer.send_reliable(&self.sock, encode(m)); }

    /// Eingabe für einen Frame (unzuverlässig, jeden Frame neu)
    pub fn send_input(&mut self, frame: u64, input: Input) {
        self.peer.send_unreliable(&self.sock, encode(&ClientMsg::Input { frame, input }));
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
        let a = self.peer.addr;
        let peer = &mut self.peer;
        let mut raw = vec![];
        recv_all(&self.sock, |from, p| if from == a { raw.extend(peer.on_packet(p)); });
        for d in raw { if let Some(m) = decode::<ServerMsg>(&d) { out.push(m); } }
        if self.peer.last_sent.elapsed() >= Duration::from_secs(1) {
            let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
            self.peer.send_unreliable(&self.sock, encode(&ClientMsg::Ping { t }));
        }
        self.peer.update(&self.sock);
        self.pending = out;
    }

    /// Verbindung verloren (Server seit 10 s stumm)
    pub fn is_dead(&self) -> bool { self.peer.dead }

    /// Sauber trennen (Server erfährt es sofort statt erst nach dem Timeout)
    pub fn disconnect(mut self) {
        let bye = encode(&ClientMsg::Bye);
        self.peer.send_reliable(&self.sock, bye.clone());
        for _ in 0..3 { self.peer.send_unreliable(&self.sock, bye.clone()); }
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
}
