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
use std::sync::Arc;
use std::io::{Read, Write};
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
    pub fn bind(bind: &str, port: u16) -> std::io::Result<WsHub> { WsHub::bind_with_http(bind, port, None) }

    /// Wie bind; Anfragen ohne WebSocket-Upgrade gehen an `http` (z.B. Replays), sonst 404
    pub fn bind_with_http(bind: &str, port: u16, http: Option<HttpHandler>) -> std::io::Result<WsHub> {
        let listener = TcpListener::bind((bind, port))?;
        let local = listener.local_addr()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                let http = http.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    if !is_upgrade(&stream) { serve_http(stream, http.as_ref()); return; }
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

// ---------------------------------------------------------------- einfache HTTP-Anfragen

/// HTTP-Anfrage (nur was der Server braucht)
pub struct HttpRequest { pub method: String, pub path: String, pub query: Vec<(String, String)>, pub body: Vec<u8> }
impl HttpRequest {
    pub fn param(&self, k: &str) -> Option<&str> { self.query.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str()) }
}
pub struct HttpResponse { pub status: u16, pub content_type: &'static str, pub body: Vec<u8> }
impl HttpResponse {
    pub fn text(status: u16, t: &str) -> HttpResponse { HttpResponse { status, content_type: "text/plain; charset=utf-8", body: t.as_bytes().to_vec() } }
    pub fn bytes(b: Vec<u8>) -> HttpResponse { HttpResponse { status: 200, content_type: "application/octet-stream", body: b } }
}
pub type HttpHandler = Arc<dyn Fn(&HttpRequest) -> HttpResponse + Send + Sync>;
/// Größte erlaubte Anfrage (Replays)
pub const HTTP_MAX_BODY: usize = 6 * 1024 * 1024;

/// Kopf der Anfrage ansehen, ohne etwas zu verbrauchen: WebSocket-Upgrade?
fn is_upgrade(s: &TcpStream) -> bool {
    let mut buf = [0u8; 4096];
    for _ in 0..200 {
        let n = match s.peek(&mut buf) { Ok(n) => n, Err(_) => return true };
        if n == 0 { return true; }
        let head = String::from_utf8_lossy(&buf[..n]).to_lowercase();
        if head.contains("\r\n\r\n") || n == buf.len() { return head.contains("upgrade: websocket"); }
        std::thread::sleep(Duration::from_millis(5));
    }
    true
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => { out.push(b' '); i += 1; }
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) { Ok(v) => { out.push(v); i += 3; } Err(_) => { out.push(b'%'); i += 1; } }
            }
            c => { out.push(c); i += 1; }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn serve_http(mut s: TcpStream, h: Option<&HttpHandler>) {
    let mut data: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    let head_end = loop {
        if let Some(p) = data.windows(4).position(|w| w == b"\r\n\r\n") { break p; }
        if data.len() > 16 * 1024 { return; }
        match s.read(&mut buf) { Ok(0) | Err(_) => return, Ok(n) => data.extend_from_slice(&buf[..n]) }
    };
    let head = String::from_utf8_lossy(&data[..head_end]).to_string();
    let mut lines = head.split("\r\n");
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let (method, target) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("/").to_string());
    let mut len = 0usize;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') { if k.trim().eq_ignore_ascii_case("content-length") { len = v.trim().parse().unwrap_or(0); } }
    }
    let resp = if len > HTTP_MAX_BODY { HttpResponse::text(413, "zu groß") } else {
        let mut body = data[head_end + 4..].to_vec();
        let _ = s.set_read_timeout(Some(Duration::from_secs(20)));
        while body.len() < len { match s.read(&mut buf) { Ok(0) | Err(_) => break, Ok(n) => body.extend_from_slice(&buf[..n]) } }
        body.truncate(len);
        let (path, q) = target.split_once('?').unwrap_or((target.as_str(), ""));
        let query = q.split('&').filter(|p| !p.is_empty()).map(|p| { let (k, v) = p.split_once('=').unwrap_or((p, "")); (pct_decode(k), pct_decode(v)) }).collect();
        let req = HttpRequest { method, path: path.to_string(), query, body };
        match h { Some(h) => h(&req), None => HttpResponse::text(404, "nicht gefunden") }
    };
    let reason = match resp.status { 200 => "OK", 400 => "Bad Request", 403 => "Forbidden", 404 => "Not Found", 413 => "Payload Too Large", 429 => "Too Many Requests", _ => "Error" };
    let hdr = format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n", resp.status, reason, resp.content_type, resp.body.len());
    let _ = s.write_all(hdr.as_bytes());
    let _ = s.write_all(&resp.body);
    let _ = s.flush();
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
