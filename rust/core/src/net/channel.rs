//! Schlanker Kanal über UDP: zuverlässige Nachrichten (Reihenfolge garantiert, werden
//! wiederholt bis bestätigt) und unzuverlässige (Eingaben, Snapshots - nur das Neueste zählt).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

const MAGIC: [u8; 4] = *b"JND\x01";
pub const MAX_PACKET: usize = 1400;
/// Unbestätigte zuverlässige Nachricht erneut senden nach
pub const RESEND_AFTER: Duration = Duration::from_millis(120);
/// Gegenstelle gilt als weg, wenn so lange nichts mehr kam
pub const PEER_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Packet {
    /// `ack` = alle zuverlässigen Nachrichten mit seq < ack sind angekommen
    Reliable { seq: u32, ack: u32, data: Vec<u8> },
    Unreliable { ack: u32, data: Vec<u8> },
    Ack { ack: u32 },
}

pub fn encode_packet(p: &Packet) -> Vec<u8> {
    let mut v = MAGIC.to_vec();
    v.extend(super::proto::encode(p));
    v
}
pub fn decode_packet(b: &[u8]) -> Option<Packet> {
    if b.len() < 4 || b[..4] != MAGIC { return None; }
    super::proto::decode(&b[4..])
}

/// Simulierter Paketverlust (nur für Tests): Anteil verworfener ausgehender Pakete
#[derive(Clone, Copy, Debug, Default)]
pub struct Lossy(pub f64);
impl Lossy {
    fn drop_now(&self) -> bool { self.0 > 0.0 && rand::random::<f64>() < self.0 }
}

/// Etwas, worüber Pakete rausgehen: UDP-Socket oder WebSocket (siehe `ws.rs`)
pub trait Wire {
    fn send_bytes(&self, to: SocketAddr, b: &[u8]);
}
impl Wire for UdpSocket {
    fn send_bytes(&self, to: SocketAddr, b: &[u8]) { let _ = self.send_to(b, to); }
}

pub fn send_raw(sock: &dyn Wire, addr: SocketAddr, p: &Packet, loss: Lossy) {
    if loss.drop_now() { return; }
    sock.send_bytes(addr, &encode_packet(p));
}

/// Alles empfangen, was gerade da ist (nicht blockierend). Fehler wie Windows'
/// "Verbindung zurückgesetzt" (ICMP Port unreachable) werden ignoriert.
pub fn recv_all(sock: &UdpSocket, mut f: impl FnMut(SocketAddr, Packet)) {
    let mut buf = [0u8; 65536];
    for _ in 0..4096 {
        match sock.recv_from(&mut buf) {
            Ok((n, from)) => { if let Some(p) = decode_packet(&buf[..n]) { f(from, p); } }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => return,
            Err(_) => continue,
        }
    }
}

struct Pending { seq: u32, data: Vec<u8>, last_sent: Instant, first_sent: Instant }

/// Zustand einer Verbindung (eine Seite)
pub struct Peer {
    pub addr: SocketAddr,
    pub loss: Lossy,
    send_seq: u32,
    unacked: VecDeque<Pending>,
    recv_next: u32,
    recv_buf: BTreeMap<u32, Vec<u8>>,
    pub last_heard: Instant,
    pub last_sent: Instant,
    ack_dirty: bool,
    pub dead: bool,
}

impl Peer {
    pub fn new(addr: SocketAddr) -> Peer {
        let now = Instant::now();
        Peer { addr, loss: Lossy::default(), send_seq: 0, unacked: VecDeque::new(), recv_next: 0, recv_buf: BTreeMap::new(), last_heard: now, last_sent: now, ack_dirty: false, dead: false }
    }

    pub fn recv_next(&self) -> u32 { self.recv_next }
    pub fn has_unacked(&self) -> bool { !self.unacked.is_empty() }

    pub fn send_reliable(&mut self, sock: &dyn Wire, data: Vec<u8>) {
        let seq = self.send_seq;
        self.send_seq = self.send_seq.wrapping_add(1);
        let now = Instant::now();
        send_raw(sock, self.addr, &Packet::Reliable { seq, ack: self.recv_next, data: data.clone() }, self.loss);
        self.unacked.push_back(Pending { seq, data, last_sent: now, first_sent: now });
        self.last_sent = now;
        self.ack_dirty = false;
    }

    pub fn send_unreliable(&mut self, sock: &dyn Wire, data: Vec<u8>) {
        send_raw(sock, self.addr, &Packet::Unreliable { ack: self.recv_next, data }, self.loss);
        self.last_sent = Instant::now();
        self.ack_dirty = false;
    }

    fn handle_ack(&mut self, ack: u32) {
        while let Some(p) = self.unacked.front() {
            if p.seq < ack { self.unacked.pop_front(); } else { break; }
        }
    }

    /// Verarbeitet ein Paket; gibt die Nutzdaten zurück, die jetzt (in Reihenfolge) zugestellt werden
    pub fn on_packet(&mut self, p: Packet) -> Vec<Vec<u8>> {
        self.last_heard = Instant::now();
        let mut out = vec![];
        match p {
            Packet::Ack { ack } => self.handle_ack(ack),
            Packet::Unreliable { ack, data } => { self.handle_ack(ack); out.push(data); }
            Packet::Reliable { seq, ack, data } => {
                self.handle_ack(ack);
                self.ack_dirty = true;
                if seq >= self.recv_next && seq < self.recv_next.saturating_add(1024) { self.recv_buf.insert(seq, data); }
                while let Some(d) = self.recv_buf.remove(&self.recv_next) {
                    out.push(d);
                    self.recv_next += 1;
                }
            }
        }
        out
    }

    /// Regelmäßig aufrufen: wiederholt Unbestätigtes, schickt Bestätigungen, erkennt Timeout
    pub fn update(&mut self, sock: &dyn Wire) {
        let now = Instant::now();
        if now.duration_since(self.last_heard) > PEER_TIMEOUT { self.dead = true; }
        let ack = self.recv_next;
        let mut sent = false;
        for p in self.unacked.iter_mut() {
            if now.duration_since(p.first_sent) > PEER_TIMEOUT { self.dead = true; }
            if now.duration_since(p.last_sent) >= RESEND_AFTER {
                send_raw(sock, self.addr, &Packet::Reliable { seq: p.seq, ack, data: p.data.clone() }, self.loss);
                p.last_sent = now;
                sent = true;
            }
        }
        if sent { self.last_sent = now; self.ack_dirty = false; }
        if self.ack_dirty {
            send_raw(sock, self.addr, &Packet::Ack { ack }, self.loss);
            self.ack_dirty = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> (UdpSocket, UdpSocket) {
        let a = UdpSocket::bind("127.0.0.1:0").unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").unwrap();
        a.set_nonblocking(true).unwrap();
        b.set_nonblocking(true).unwrap();
        (a, b)
    }

    /// 200 zuverlässige Nachrichten kommen trotz 30 % Verlust vollständig und in Reihenfolge an
    #[test]
    fn zuverlaessig_trotz_verlust() {
        let (sa, sb) = pair();
        let mut pa = Peer::new(sb.local_addr().unwrap());
        let mut pb = Peer::new(sa.local_addr().unwrap());
        pa.loss = Lossy(0.3);
        pb.loss = Lossy(0.3);
        for i in 0..200u32 { pa.send_reliable(&sa, i.to_le_bytes().to_vec()); }
        let mut got: Vec<u32> = vec![];
        let start = Instant::now();
        while got.len() < 200 && start.elapsed() < Duration::from_secs(20) {
            recv_all(&sb, |_, p| for d in pb.on_packet(p) { got.push(u32::from_le_bytes([d[0], d[1], d[2], d[3]])); });
            recv_all(&sa, |_, p| { pa.on_packet(p); });
            pa.update(&sa);
            pb.update(&sb);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(got, (0..200).collect::<Vec<_>>());
        // irgendwann ist alles bestätigt
        let start = Instant::now();
        while pa.has_unacked() && start.elapsed() < Duration::from_secs(10) {
            recv_all(&sb, |_, p| { pb.on_packet(p); });
            recv_all(&sa, |_, p| { pa.on_packet(p); });
            pa.update(&sa);
            pb.update(&sb);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!pa.has_unacked());
    }

    #[test]
    fn fremde_pakete_ignoriert() {
        assert!(decode_packet(b"GET / HTTP/1.1").is_none());
        assert!(decode_packet(&[]).is_none());
        assert!(decode_packet(b"JND\x01\xff\xff\xff\xff\xff\xff\xff\xff\xff").is_none());
    }
}
