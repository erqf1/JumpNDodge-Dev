//! WebSocket-Weg für den Multiplayer. Gedacht für Cloudflare Tunnel und ähnliche Dienste: dort kommt nur
//! HTTP/WebSocket durch, kein UDP. Die Pakete sind dieselben wie über UDP (inkl. Wiederholen/Bestätigen -
//! über TCP unnötig, aber harmlos), nur dass jedes Paket eine WebSocket-Binärnachricht ist.
//!
//! Server: hört zusätzlich auf einem TCP-Port (Standard 47801) auf ws:// - TLS macht der Tunnel.
//! Spiel: `wss://host` oder `ws://host:port` als Adresse, oder automatisch als Rückfall, wenn UDP nicht antwortet.

use super::channel::{decode_packet, encode_packet, Packet, Wire};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use tungstenite::{Message, WebSocket};

/// Eindeutige Schein-Adresse pro WebSocket-Verbindung (der Server schlüsselt Verbindungen nach Adresse;
/// hinter einem Tunnel kommen alle von derselben IP). Bereich fd00:4a4e:44::/48 ist privat und kommt
/// über UDP nicht vor.
pub fn ws_addr(id: u32) -> SocketAddr {
    SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0xfd00, 0x4a4e, 0x44, 0, 0, 0, (id >> 16) as u16, id as u16)), 1)
}
pub fn is_ws_addr(a: &SocketAddr) -> bool {
    matches!(a.ip(), IpAddr::V6(v) if v.segments()[..3] == [0xfd00, 0x4a4e, 0x44])
}

fn would_block(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(io) if io.kind() == ErrorKind::WouldBlock || io.kind() == ErrorKind::TimedOut)
}

/// Alle anstehenden Nachrichten einer Verbindung lesen. false = Verbindung ist zu.
fn pump<S: std::io::Read + std::io::Write>(ws: &mut WebSocket<S>, from: SocketAddr, f: &mut impl FnMut(SocketAddr, Packet)) -> bool {
    let _ = ws.flush();
    for _ in 0..4096 {
        match ws.read() {
            Ok(Message::Binary(b)) => { if let Some(p) = decode_packet(&b[..]) { f(from, p); } }
            Ok(Message::Close(_)) => return false,
            Ok(_) => {}
            Err(e) if would_block(&e) => return true,
            Err(_) => return false,
        }
    }
    true
}

// ---------------------------------------------------------------- Server

/// Server-Seite: nimmt WebSocket-Verbindungen an (Handshake in eigenen Threads, damit der Spieltakt nie wartet)
pub struct WsHub {
    rx: Receiver<WebSocket<TcpStream>>,
    conns: RefCell<HashMap<SocketAddr, WebSocket<TcpStream>>>,
    closed: RefCell<Vec<SocketAddr>>,
    next: Cell<u32>,
    local: SocketAddr,
}

impl WsHub {
    pub fn bind(bind: &str, port: u16) -> std::io::Result<WsHub> {
        let listener = TcpListener::bind((bind, port))?;
        let local = listener.local_addr()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    if let Ok(ws) = tungstenite::accept(stream) {
                        let _ = ws.get_ref().set_read_timeout(None);
                        if ws.get_ref().set_nonblocking(true).is_ok() { let _ = tx.send(ws); }
                    }
                });
            }
        });
        Ok(WsHub { rx, conns: RefCell::new(HashMap::new()), closed: RefCell::new(vec![]), next: Cell::new(1), local })
    }

    pub fn local_addr(&self) -> SocketAddr { self.local }
    pub fn is_ws(&self, a: &SocketAddr) -> bool { self.conns.borrow().contains_key(a) }
    pub fn count(&self) -> usize { self.conns.borrow().len() }

    /// Neue Verbindungen übernehmen und alles Empfangene ausliefern (nicht blockierend)
    pub fn recv_all(&self, mut f: impl FnMut(SocketAddr, Packet)) {
        while let Ok(ws) = self.rx.try_recv() {
            let id = self.next.get();
            self.next.set(id.wrapping_add(1).max(1));
            self.conns.borrow_mut().insert(ws_addr(id), ws);
        }
        let mut gone = vec![];
        for (a, ws) in self.conns.borrow_mut().iter_mut() {
            if !pump(ws, *a, &mut f) { gone.push(*a); }
        }
        let mut c = self.conns.borrow_mut();
        for a in gone { c.remove(&a); self.closed.borrow_mut().push(a); }
    }

    /// Seit dem letzten Aufruf geschlossene Verbindungen (Server trennt die Spieler sofort)
    pub fn take_closed(&self) -> Vec<SocketAddr> { std::mem::take(&mut *self.closed.borrow_mut()) }

    pub fn close(&self, a: &SocketAddr) {
        if let Some(mut ws) = self.conns.borrow_mut().remove(a) { let _ = ws.close(None); let _ = ws.flush(); }
    }
}

impl Wire for WsHub {
    fn send_bytes(&self, to: SocketAddr, b: &[u8]) {
        if let Some(ws) = self.conns.borrow_mut().get_mut(&to) { let _ = ws.send(Message::binary(b.to_vec())); }
    }
}

// ---------------------------------------------------------------- Spiel

type ClientStream = tungstenite::stream::MaybeTlsStream<TcpStream>;

/// Spiel-Seite: eine WebSocket-Verbindung zum Server
pub struct WsLink {
    ws: RefCell<WebSocket<ClientStream>>,
    pub addr: SocketAddr,
    closed: Cell<bool>,
}

/// "wss://host[:port][/pfad]" oder "ws://..." -> Verbindung (Zeitlimit für TCP + Handshake)
pub fn connect(url: &str, timeout: Duration) -> Result<WsLink, String> {
    let rest = url.split_once("://").map(|(_, r)| r).ok_or("Ungültige Adresse")?;
    let tls = url.starts_with("wss://");
    let host_port = rest.split('/').next().unwrap_or("");
    let with_port = if host_port.rsplit_once(':').map(|(_, p)| p.parse::<u16>().is_ok()).unwrap_or(false) { host_port.to_string() }
        else { format!("{}:{}", host_port, if tls { 443 } else { 80 }) };
    let addr = with_port.to_socket_addrs().map_err(|e| format!("Adresse nicht gefunden: {e}"))?.next().ok_or("Adresse nicht gefunden")?;
    let stream = TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("Keine Verbindung: {e}"))?;
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    #[cfg(feature = "tls")]
    let (mut ws, _) = tungstenite::client_tls(url, stream).map_err(|e| format!("WebSocket-Fehler: {e}"))?;
    #[cfg(not(feature = "tls"))]
    let (mut ws, _) = {
        if tls { return Err("wss:// braucht das Feature \"tls\"".into()); }
        tungstenite::client(url, tungstenite::stream::MaybeTlsStream::Plain(stream)).map_err(|e| format!("WebSocket-Fehler: {e}"))?
    };
    let set_nb = |s: &TcpStream| { let _ = s.set_read_timeout(None); let _ = s.set_write_timeout(None); s.set_nonblocking(true) };
    match ws.get_mut() {
        tungstenite::stream::MaybeTlsStream::Plain(s) => set_nb(s),
        #[cfg(feature = "tls")]
        tungstenite::stream::MaybeTlsStream::Rustls(s) => set_nb(s.get_mut()),
        _ => Ok(()),
    }.map_err(|e| e.to_string())?;
    Ok(WsLink { ws: RefCell::new(ws), addr: ws_addr(0), closed: Cell::new(false) })
}

impl WsLink {
    pub fn recv_all(&self, mut f: impl FnMut(SocketAddr, Packet)) {
        if self.closed.get() { return; }
        if !pump(&mut self.ws.borrow_mut(), self.addr, &mut f) { self.closed.set(true); }
    }
    pub fn is_closed(&self) -> bool { self.closed.get() }
    pub fn close(&self) { let mut ws = self.ws.borrow_mut(); let _ = ws.close(None); let _ = ws.flush(); }
}

impl Wire for WsLink {
    fn send_bytes(&self, _to: SocketAddr, b: &[u8]) {
        if self.closed.get() { return; }
        let _ = self.ws.borrow_mut().send(Message::binary(b.to_vec()));
    }
}

/// Ein Paket direkt senden (ohne Peer), z.B. für die Serverinfo
pub fn send_packet(w: &dyn Wire, to: SocketAddr, p: &Packet) { w.send_bytes(to, &encode_packet(p)); }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schein_adressen() {
        assert!(is_ws_addr(&ws_addr(7)));
        assert_ne!(ws_addr(1), ws_addr(2));
        assert!(!is_ws_addr(&"127.0.0.1:47800".parse().unwrap()));
        assert!(!is_ws_addr(&"[::1]:47800".parse().unwrap()));
    }
}
