//! Der Multiplayer-Server: Verbindungen, Räume (je 2 Spieler), Countdown und die
//! eigentliche Runde - gerechnet mit derselben Simulation wie der lokale 2P-Modus.

use crate::config::Config;
use jnd_core::net::channel::{recv_all, send_raw, Lossy, Packet, Peer, Wire};
use jnd_core::net::ws::WsHub;
use jnd_core::net::proto::*;
use jnd_core::sim2p::{self, Input, SimEvent, Sim2p};
use rand::Rng;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const CODE_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const MAX_ROOMS_IN_LIST: usize = 15;
const MAX_CATCHUP_TICKS: u64 = 5;

struct Conn {
    id: u32,
    peer: Peer,
    name: String,
    welcomed: bool,
    room: Option<String>,
    /// Abgelehnte Verbindungen werden nach kurzer Zeit entfernt
    remove_after: Option<Instant>,
    /// Fenstergröße des Spielers (Pixel), falls gemeldet
    screen: Option<(u32, u32)>,
    /// Chat-Spamschutz (Token-Eimer): verfügbare Nachrichten und letzter Stand
    chat_tokens: f64,
    chat_at: Instant,
}

struct Game {
    sim: Sim2p,
    /// Feldgröße dieser Runde
    fw: i32,
    fh: i32,
    frame: u64,
    /// Platz im Raum für jede Spielernummer der Runde (Runde zählt nur besetzte Plätze)
    order: Vec<usize>,
    inputs: Vec<Input>,
    input_frame: Vec<u64>,
    events: VecDeque<(u64, SimEvent)>,
}

enum Phase { Lobby, Countdown { ends: Instant, shown: u8 }, Playing(Box<Game>) }

struct Room {
    code: String,
    mode: Mode,
    difficulty: String,
    /// Plätze (2 bis 8)
    slots: Vec<Option<SocketAddr>>,
    /// Platz des Raum-Chefs (Ersteller; geht er, übernimmt der nächste)
    host: usize,
    /// Privat: nicht in der Raumliste und nicht bei Schnell spielen
    private: bool,
    ready: Vec<bool>,
    phase: Phase,
}

impl Room {
    fn count(&self) -> u8 { self.slots.iter().filter(|s| s.is_some()).count() as u8 }
    fn slot_of(&self, a: SocketAddr) -> Option<usize> { self.slots.iter().position(|s| *s == Some(a)) }
    fn unready_all(&mut self) { for r in self.ready.iter_mut() { *r = false; } }
    /// Mindestens zwei da und alle Anwesenden bereit
    fn all_ready(&self) -> bool { self.count() >= 2 && self.slots.iter().zip(&self.ready).all(|(s, r)| s.is_none() || *r) }
}

pub type Logger = Box<dyn FnMut(&str) + Send>;

/// Senden über UDP oder - für WebSocket-Verbindungen - über den Hub
struct Wires<'a> { udp: &'a UdpSocket, ws: Option<&'a WsHub> }
impl Wire for Wires<'_> {
    fn send_bytes(&self, to: SocketAddr, b: &[u8]) {
        match self.ws { Some(h) if h.is_ws(&to) => h.send_bytes(to, b), _ => self.udp.send_bytes(to, b) }
    }
}

pub struct Server {
    sock: UdpSocket,
    pub cfg: Config,
    conns: HashMap<SocketAddr, Conn>,
    rooms: BTreeMap<String, Room>,
    next_id: u32,
    start: Instant,
    ticks_done: u64,
    /// Simulierter Paketverlust (nur Tests)
    pub loss: Lossy,
    log: Logger,
    ws: Option<WsHub>,
    updater: Option<crate::update::Updater>,
    /// Ein Update liegt bereit und niemand ist verbunden: run() endet, main.rs startet neu
    pub restart_for_update: Option<crate::update::Ready>,
    /// Chat-Verlauf (Unix-Sekunden, Name, Text) - siehe cfg.chat_history_hours
    chat_log: VecDeque<(u64, String, String)>,
    /// Datei für den Verlauf (übersteht Neustarts/Updates); None = nur im Speicher (Tests)
    chat_store: Option<std::path::PathBuf>,
}

fn unix_now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) }

fn tick_dt_ms(n: u64) -> i64 { ((n * 1000) / TICK_HZ - ((n - 1) * 1000) / TICK_HZ) as i64 }

impl Server {
    pub fn bind(cfg: Config, log: Logger) -> std::io::Result<Server> {
        let sock = UdpSocket::bind((cfg.bind.as_str(), cfg.port))?;
        sock.set_nonblocking(true)?;
        let mut log = log;
        let ws = if cfg.ws_port == 0 { None } else {
            match WsHub::bind(&cfg.bind, cfg.ws_port) {
                Ok(h) => Some(h),
                Err(e) => { log(&format!("WebSocket-Port {} (TCP) nicht verfügbar: {} - nur UDP", cfg.ws_port, e)); None }
            }
        };
        let updater = if cfg.auto_update { Some(crate::update::Updater::new(cfg.update_check_minutes)) } else { None };
        Ok(Server { sock, cfg, conns: HashMap::new(), rooms: BTreeMap::new(), next_id: 1, start: Instant::now(), ticks_done: 0, loss: Lossy::default(), log, ws, updater, restart_for_update: None, chat_log: VecDeque::new(), chat_store: None })
    }

    /// Chat-Verlauf in dieser Datei aufheben (und vorhandenen laden)
    pub fn set_chat_store(&mut self, path: std::path::PathBuf) {
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str::<Vec<(u64, String, String)>>(&s) { self.chat_log = v.into_iter().collect(); }
        }
        self.chat_store = Some(path);
        self.prune_chat();
    }

    fn prune_chat(&mut self) {
        let keep = self.cfg.chat_history_hours as u64 * 3600;
        let now = unix_now();
        while self.chat_log.front().map(|e| keep == 0 || e.0 + keep < now).unwrap_or(false) { self.chat_log.pop_front(); }
        while self.chat_log.len() > CHAT_HISTORY_MAX { self.chat_log.pop_front(); }
    }

    fn save_chat(&self) {
        let Some(p) = &self.chat_store else { return; };
        let v: Vec<&(u64, String, String)> = self.chat_log.iter().collect();
        if let Ok(s) = serde_json::to_string(&v) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() { let _ = std::fs::rename(&tmp, p); }
        }
    }

    /// Adresse des WebSocket-Ports (falls an)
    pub fn ws_addr(&self) -> Option<SocketAddr> { self.ws.as_ref().map(|h| h.local_addr()) }

    pub fn local_addr(&self) -> SocketAddr { self.sock.local_addr().expect("lokale Adresse") }
    pub fn player_count(&self) -> usize { self.conns.values().filter(|c| c.welcomed).count() }
    pub fn room_count(&self) -> usize { self.rooms.len() }

    fn logf(&mut self, s: &str) { (self.log)(s); }

    /// Läuft, bis `stop` gesetzt wird
    pub fn run(&mut self, stop: &AtomicBool) {
        while !stop.load(Ordering::Relaxed) && self.restart_for_update.is_none() {
            self.poll_once();
            std::thread::sleep(Duration::from_millis(1));
        }
        // Allen Bescheid geben
        let addrs: Vec<SocketAddr> = self.conns.keys().copied().collect();
        for a in addrs { self.send(a, &ServerMsg::Error("Server wurde beendet".into())); }
    }

    /// Ein Durchgang: empfangen, Ticks nachholen, Verbindungen pflegen
    pub fn poll_once(&mut self) {
        let mut pkts = vec![];
        recv_all(&self.sock, |from, p| pkts.push((from, p)));
        if let Some(h) = &self.ws {
            h.recv_all(|from, p| pkts.push((from, p)));
            for a in h.take_closed() { self.drop_conn(a, "Verbindung getrennt"); }
        }
        for (from, p) in pkts { self.on_packet(from, p); }
        // Auto-Update: suchen, laden, und erst neu starten, wenn niemand verbunden ist
        if let Some(u) = self.updater.as_mut() {
            for line in u.poll() { (self.log)(&line); }
            if self.conns.values().all(|c| !c.welcomed) {
                if let Some(r) = u.take_ready() {
                    (self.log)(&format!("Update auf {} bereit - niemand verbunden, starte neu", r.version));
                    self.restart_for_update = Some(r);
                }
            }
        }

        let target = self.start.elapsed().as_millis() as u64 * TICK_HZ / 1000;
        if target > self.ticks_done + MAX_CATCHUP_TICKS { self.ticks_done = target - MAX_CATCHUP_TICKS; }
        while self.ticks_done < target {
            self.ticks_done += 1;
            let dt = tick_dt_ms(self.ticks_done.max(1));
            self.tick(dt);
        }

        let now = Instant::now();
        let mut dead = vec![];
        let wires = Wires { udp: &self.sock, ws: self.ws.as_ref() };
        for (a, c) in self.conns.iter_mut() {
            c.peer.update(&wires);
            if c.peer.dead { dead.push((*a, "Zeitüberschreitung")); }
            else if c.remove_after.map(|t| now >= t).unwrap_or(false) { dead.push((*a, "")); }
        }
        for (a, why) in dead { self.drop_conn(a, why); }
    }

    // ---------------- Senden ----------------
    fn send(&mut self, a: SocketAddr, m: &ServerMsg) {
        let wires = Wires { udp: &self.sock, ws: self.ws.as_ref() };
        if let Some(c) = self.conns.get_mut(&a) { c.peer.send_reliable(&wires, encode(m)); }
    }
    fn send_u(&mut self, a: SocketAddr, m: &ServerMsg) {
        let wires = Wires { udp: &self.sock, ws: self.ws.as_ref() };
        if let Some(c) = self.conns.get_mut(&a) { c.peer.send_unreliable(&wires, encode(m)); }
    }

    fn info(&self) -> ServerInfo {
        ServerInfo {
            name: self.cfg.name.clone(), motd: self.cfg.motd.clone(), game_version: GAME_VERSION.into(), proto: PROTO_VERSION,
            rooms: self.rooms.len() as u32, max_rooms: self.cfg.max_rooms, players: self.player_count() as u32, password: !self.cfg.password.is_empty(),
        }
    }

    // ---------------- Empfangen ----------------
    fn on_packet(&mut self, from: SocketAddr, p: Packet) {
        let hello_seq0 = match &p {
            Packet::Reliable { seq: 0, data, .. } => matches!(decode::<ClientMsg>(data), Some(ClientMsg::Hello { .. })),
            _ => false,
        };
        // Serverinfo (auch ohne Verbindung)
        if let Packet::Unreliable { data, .. } = &p {
            if let Some(ClientMsg::Info) = decode::<ClientMsg>(data) {
                let wires = Wires { udp: &self.sock, ws: self.ws.as_ref() };
                send_raw(&wires, from, &Packet::Unreliable { ack: 0, data: encode(&ServerMsg::Info(self.info())) }, self.loss);
                return;
            }
        }
        match self.conns.get(&from) {
            None => {
                if !hello_seq0 { return; }
                if self.conns.len() >= (self.cfg.max_rooms as usize) * 2 + 32 { return; }
                self.new_conn(from);
            }
            Some(c) if hello_seq0 && c.peer.recv_next() > 0 => {
                // Gleicher Absender startet neu (Spiel neu gestartet) -> alte Sitzung beenden
                self.drop_conn(from, "neu verbunden");
                self.new_conn(from);
            }
            _ => {}
        }
        let msgs = match self.conns.get_mut(&from) { Some(c) => c.peer.on_packet(p), None => return };
        for d in msgs {
            if let Some(m) = decode::<ClientMsg>(&d) { self.handle(from, m); }
        }
    }

    fn new_conn(&mut self, a: SocketAddr) {
        let mut peer = Peer::new(a);
        peer.loss = self.loss;
        let id = self.next_id;
        self.next_id += 1;
        self.conns.insert(a, Conn { id, peer, name: String::new(), welcomed: false, room: None, remove_after: None, screen: None, chat_tokens: CHAT_BURST, chat_at: Instant::now() });
    }

    fn reject(&mut self, a: SocketAddr, reason: String) {
        self.logf(&format!("Abgelehnt {}: {}", a, reason));
        self.send(a, &ServerMsg::Reject { reason });
        if let Some(c) = self.conns.get_mut(&a) { c.remove_after = Some(Instant::now() + Duration::from_secs(3)); }
    }

    fn handle(&mut self, a: SocketAddr, m: ClientMsg) {
        let welcomed = self.conns.get(&a).map(|c| c.welcomed).unwrap_or(false);
        if !welcomed {
            if let ClientMsg::Hello { proto, game_version, name, password } = m {
                // Nur die Protokoll-Sperre zählt: verschiedene Spielversionen dürfen zusammen spielen
                if proto != PROTO_VERSION {
                    if proto > PROTO_VERSION { if let Some(u) = self.updater.as_mut() { u.check_soon(); } }
                    let who = if proto < PROTO_VERSION { "Dein Spiel ist zu alt - bitte aktualisieren" } else { "Der Server ist zu alt - er muss aktualisiert werden" };
                    return self.reject(a, format!("{} (Server {}, Protokoll {}; Spiel {}, Protokoll {}).", who, GAME_VERSION, PROTO_VERSION, game_version, proto));
                }
                if !self.cfg.password.is_empty() && password != self.cfg.password {
                    return self.reject(a, "Falsches Passwort".into());
                }
                let name = clean_name(&name);
                // Neuere Spielversion verbunden -> Server ist vermutlich hinterher: sofort nach Update schauen
                if crate::update::is_newer(&game_version, GAME_VERSION) {
                    if let Some(u) = self.updater.as_mut() { u.check_soon(); }
                }
                let (sn, motd) = (self.cfg.name.clone(), self.cfg.motd.clone());
                let id = {
                    let c = self.conns.get_mut(&a).unwrap();
                    c.welcomed = true;
                    c.name = name.clone();
                    c.id
                };
                self.logf(&format!("{} verbunden ({}) - {} Spieler online", name, a, self.player_count()));
                self.send(a, &ServerMsg::Welcome { client_id: id, server_name: sn, motd });
                // Verlauf der letzten Stunden nachschicken
                if self.cfg.chat && self.cfg.chat_history_hours > 0 {
                    self.prune_chat();
                    let hist: Vec<(String, String)> = self.chat_log.iter().map(|e| (e.1.clone(), e.2.clone())).collect();
                    for (from, text) in hist { self.send(a, &ServerMsg::Chat { from_id: CHAT_HISTORY_ID, from, text }); }
                }
                self.chat_notice_others(a, &format!("{} ist da", name));
            }
            return;
        }
        match m {
            ClientMsg::Hello { .. } | ClientMsg::Info => {}
            ClientMsg::ListRooms => {
                let list: Vec<RoomInfo> = self.rooms.values()
                    .filter(|r| !r.private)
                    .map(|r| RoomInfo {
                        code: r.code.clone(), mode: r.mode, difficulty: r.difficulty.clone(), players: r.count(), max: r.slots.len() as u8,
                        host: r.slots.get(r.host).copied().flatten().or_else(|| r.slots.iter().flatten().next().copied()).and_then(|h| self.conns.get(&h)).map(|c| c.name.clone()).unwrap_or_default(),
                        playing: matches!(r.phase, Phase::Playing(_)),
                    })
                    // offene Räume zuerst
                    .collect::<Vec<_>>();
                let mut list = list;
                list.sort_by_key(|r| (r.playing, r.players >= r.max));
                list.truncate(MAX_ROOMS_IN_LIST);
                self.send(a, &ServerMsg::RoomList(list));
            }
            ClientMsg::CreateRoom { mode, difficulty, max_players } => self.create_room(a, mode, difficulty, max_players),
            ClientMsg::QuickPlay => {
                // offener öffentlicher Raum in der Lobby, der vollste zuerst (schneller voll = schneller los)
                let pick = self.rooms.values()
                    .filter(|r| !r.private && matches!(r.phase, Phase::Lobby) && (r.count() as usize) < r.slots.len() && r.slot_of(a).is_none())
                    .max_by_key(|r| (r.count(), std::cmp::Reverse(r.code.clone())))
                    .map(|r| r.code.clone());
                match pick {
                    Some(code) => self.handle(a, ClientMsg::JoinRoom { code }),
                    None => self.create_room(a, Mode::Vs, QUICK_ROOM_DIFFICULTY.into(), QUICK_ROOM_MAX),
                }
            }
            ClientMsg::HostStart => {
                let Some(code) = self.conns[&a].room.clone() else { return; };
                let Some(r) = self.rooms.get_mut(&code) else { return; };
                if r.slot_of(a) != Some(r.host) || !matches!(r.phase, Phase::Lobby) { return; }
                if r.count() < 2 { return self.send(a, &ServerMsg::Error("Mindestens 2 Spieler nötig".into())); }
                r.phase = Phase::Countdown { ends: Instant::now() + Duration::from_secs(COUNTDOWN_SECS as u64), shown: 0 };
                self.broadcast_room(&code);
            }
            ClientMsg::Kick { slot } => {
                let Some(code) = self.conns[&a].room.clone() else { return; };
                let Some(r) = self.rooms.get(&code) else { return; };
                if r.slot_of(a) != Some(r.host) || slot as usize == r.host { return; }
                let Some(target) = r.slots.get(slot as usize).copied().flatten() else { return; };
                self.leave_room(target, "wurde aus dem Raum entfernt");
                self.send(target, &ServerMsg::LeftRoom { reason: "Du wurdest aus dem Raum entfernt".into() });
            }
            ClientMsg::SetPrivate(on) => {
                let Some(code) = self.conns[&a].room.clone() else { return; };
                let Some(r) = self.rooms.get_mut(&code) else { return; };
                if r.slot_of(a) != Some(r.host) { return; }
                r.private = on;
                self.broadcast_room(&code);
            }
            ClientMsg::Emote(id) => {
                if id as usize >= EMOTES.len() { return; }
                let Some(code) = self.conns[&a].room.clone() else { return; };
                let Some(r) = self.rooms.get(&code) else { return; };
                let Some(slot) = r.slot_of(a) else { return; };
                let (player, in_round) = match &r.phase {
                    Phase::Playing(g) => match g.order.iter().position(|&s| s == slot) { Some(k) => (k, true), None => return },
                    _ => (slot, false),
                };
                // kleiner Spamschutz: teilt sich den Chat-Eimer
                let ok = {
                    let Some(c) = self.conns.get_mut(&a) else { return; };
                    let now = Instant::now();
                    c.chat_tokens = (c.chat_tokens + now.duration_since(c.chat_at).as_secs_f64() / CHAT_REFILL_SECS).min(CHAT_BURST);
                    c.chat_at = now;
                    let ok = c.chat_tokens >= 1.0;
                    if ok { c.chat_tokens -= 1.0; }
                    ok
                };
                if !ok { return; }
                let to: Vec<SocketAddr> = r.slots.iter().flatten().copied().collect();
                let m = ServerMsg::Emote { player: player as u8, id, in_round };
                for t in to { self.send(t, &m); }
            }
            ClientMsg::JoinRoom { code } => {
                let code = code.trim().to_uppercase();
                if self.conns[&a].room.as_deref() == Some(code.as_str()) { return self.broadcast_room(&code); }
                let Some(r) = self.rooms.get(&code) else { return self.send(a, &ServerMsg::Error(format!("Raum {} gibt es nicht", code))); };
                if !matches!(r.phase, Phase::Lobby) { return self.send(a, &ServerMsg::Error("In diesem Raum läuft gerade eine Runde".into())); }
                let Some(free) = r.slots.iter().position(|s| s.is_none()) else { return self.send(a, &ServerMsg::Error("Raum ist voll".into())); };
                self.leave_room(a, "hat den Raum verlassen");
                let Some(r) = self.rooms.get_mut(&code) else { return; };
                r.slots[free] = Some(a);
                r.unready_all();
                if let Some(c) = self.conns.get_mut(&a) { c.room = Some(code.clone()); }
                let n = self.conns[&a].name.clone();
                self.logf(&format!("{} ist Raum {} beigetreten", n, code));
                self.broadcast_room(&code);
            }
            ClientMsg::LeaveRoom => {
                if self.conns[&a].room.is_some() {
                    self.leave_room(a, "hat den Raum verlassen");
                    self.send(a, &ServerMsg::LeftRoom { reason: "Du hast den Raum verlassen".into() });
                }
            }
            ClientMsg::Ready(on) => {
                let Some(code) = self.conns[&a].room.clone() else { return; };
                let Some(r) = self.rooms.get_mut(&code) else { return; };
                let Some(i) = r.slot_of(a) else { return; };
                match r.phase {
                    Phase::Playing(_) => return,
                    Phase::Countdown { .. } if !on => r.phase = Phase::Lobby,
                    _ => {}
                }
                r.ready[i] = on;
                let start = matches!(r.phase, Phase::Lobby) && r.all_ready();
                if start { r.phase = Phase::Countdown { ends: Instant::now() + Duration::from_secs(COUNTDOWN_SECS as u64), shown: 0 }; }
                self.broadcast_room(&code);
            }
            ClientMsg::Input { frame, input } => {
                let Some(code) = self.conns[&a].room.clone() else { return; };
                let Some(r) = self.rooms.get_mut(&code) else { return; };
                let Some(i) = r.slot_of(a) else { return; };
                if let Phase::Playing(g) = &mut r.phase {
                    if let Some(k) = g.order.iter().position(|&s| s == i) {
                        if frame > g.input_frame[k] { g.inputs[k] = input; g.input_frame[k] = frame; }
                    }
                }
            }
            ClientMsg::Ping { t } => self.send_u(a, &ServerMsg::Pong { t }),
            ClientMsg::Bye => self.drop_conn(a, "hat das Spiel verlassen"),
            ClientMsg::Chat { text } => self.on_chat(a, &text),
            ClientMsg::Screen { w, h } => {
                if w >= 100 && h >= 100 && w <= 20000 && h <= 20000 {
                    if let Some(c) = self.conns.get_mut(&a) { c.screen = Some((w, h)); }
                }
            }
        }
    }

    // ---------------- Chat ----------------
    /// Weiterreichen und (falls an) für chat_history_hours aufheben; der Inhalt landet nie im Log.
    fn on_chat(&mut self, a: SocketAddr, text: &str) {
        if !self.cfg.chat { return self.chat_notice(a, "Der Chat ist auf diesem Server aus"); }
        let text = clean_chat(text);
        if text.is_empty() { return; }
        let (id, name, ok) = {
            let Some(c) = self.conns.get_mut(&a) else { return; };
            let now = Instant::now();
            c.chat_tokens = (c.chat_tokens + now.duration_since(c.chat_at).as_secs_f64() / CHAT_REFILL_SECS).min(CHAT_BURST);
            c.chat_at = now;
            let ok = c.chat_tokens >= 1.0;
            if ok { c.chat_tokens -= 1.0; }
            (c.id, c.name.clone(), ok)
        };
        if !ok { return self.chat_notice(a, "Nicht so schnell - kurz warten"); }
        if self.cfg.chat_history_hours > 0 {
            self.chat_log.push_back((unix_now(), name.clone(), text.clone()));
            self.prune_chat();
            self.save_chat();
        }
        let m = ServerMsg::Chat { from_id: id, from: name, text };
        let to: Vec<SocketAddr> = self.conns.iter().filter(|(_, c)| c.welcomed && c.remove_after.is_none()).map(|(k, _)| *k).collect();
        for t in to { self.send(t, &m); }
    }

    fn chat_notice(&mut self, a: SocketAddr, text: &str) {
        self.send(a, &ServerMsg::Chat { from_id: 0, from: String::new(), text: text.into() });
    }

    /// Hinweis an alle außer `a` (z.B. "X ist da")
    fn chat_notice_others(&mut self, a: SocketAddr, text: &str) {
        if !self.cfg.chat { return; }
        let to: Vec<SocketAddr> = self.conns.iter().filter(|(k, c)| **k != a && c.welcomed && c.remove_after.is_none()).map(|(k, _)| *k).collect();
        for t in to { self.chat_notice(t, text); }
    }

    fn create_room(&mut self, a: SocketAddr, mode: Mode, difficulty: String, max_players: u8) {
        if !DIFFICULTIES.contains(&difficulty.as_str()) { return self.send(a, &ServerMsg::Error("Unbekannte Schwierigkeit".into())); }
        self.leave_room(a, "hat den Raum verlassen");
        if self.rooms.len() as u32 >= self.cfg.max_rooms { return self.send(a, &ServerMsg::Error("Server ist voll (keine freien Räume)".into())); }
        let code = self.new_code();
        let max = (max_players as usize).clamp(2, sim2p::MAX_PLAYERS);
        let mut slots = vec![None; max];
        slots[0] = Some(a);
        self.rooms.insert(code.clone(), Room { code: code.clone(), mode, difficulty: difficulty.clone(), slots, host: 0, private: false, ready: vec![false; max], phase: Phase::Lobby });
        if let Some(c) = self.conns.get_mut(&a) { c.room = Some(code.clone()); }
        let n = self.conns[&a].name.clone();
        self.logf(&format!("Raum {} erstellt von {} ({:?}, {}, {} Plätze)", code, n, mode, difficulty, max));
        self.broadcast_room(&code);
    }

    fn new_code(&self) -> String {
        let mut rng = rand::thread_rng();
        loop {
            let c: String = (0..4).map(|_| CODE_CHARS[rng.gen_range(0..CODE_CHARS.len())] as char).collect();
            if !self.rooms.contains_key(&c) { return c; }
        }
    }

    fn broadcast_room(&mut self, code: &str) {
        let Some(r) = self.rooms.get(code) else { return; };
        let players: Vec<Option<LobbyPlayer>> = (0..r.slots.len()).map(|i| r.slots[i].and_then(|a| self.conns.get(&a)).map(|c| LobbyPlayer { name: c.name.clone(), ready: r.ready[i] })).collect();
        let msgs: Vec<(SocketAddr, ServerMsg)> = (0..r.slots.len()).filter_map(|i| r.slots[i].map(|a| (a, ServerMsg::RoomState {
            code: r.code.clone(), mode: r.mode, difficulty: r.difficulty.clone(), you: i as u8, players: players.clone(),
        }))).collect();
        let extra = ServerMsg::RoomExtra { host: r.host as u8, private: r.private };
        for (a, m) in msgs { self.send(a, &m); self.send(a, &extra); }
    }

    /// Spieler verlässt seinen Raum (freiwillig oder Verbindung weg)
    fn leave_room(&mut self, a: SocketAddr, why: &str) {
        let Some(code) = self.conns.get_mut(&a).and_then(|c| c.room.take()) else { return; };
        let name = self.conns.get(&a).map(|c| c.name.clone()).unwrap_or_default();
        let Some(r) = self.rooms.get_mut(&code) else { return; };
        let Some(i) = r.slot_of(a) else { return; };
        r.slots[i] = None;
        // Chef geht -> der nächste Anwesende übernimmt
        if r.host == i { if let Some(n) = r.slots.iter().position(|s| s.is_some()) { r.host = n; } }
        let others: Vec<SocketAddr> = r.slots.iter().flatten().copied().collect();
        let mut game_over: Option<ServerMsg> = None;
        let mut end_round = false;
        match &mut r.phase {
            Phase::Countdown { .. } => r.phase = Phase::Lobby,
            Phase::Playing(g) => {
                // Wer geht, scheidet aus. VS: ist nur noch einer übrig, gewinnt er. Koop: weiter, solange jemand lebt.
                if let Some(k) = g.order.iter().position(|&s| s == i) { g.sim.eliminate(k); }
                let winner = g.sim.check_winner(true);
                let all_out = g.sim.players.iter().all(|p| !p.alive) || others.is_empty();
                if winner.is_some() || all_out {
                    game_over = Some(ServerMsg::GameOver { winner, score: g.sim.score, frames: g.frame, reason: format!("{} {}", name, why) });
                    end_round = true;
                }
            }
            Phase::Lobby => {}
        }
        if end_round { r.phase = Phase::Lobby; }
        if !matches!(r.phase, Phase::Playing(_)) { r.unready_all(); }
        let empty = r.count() == 0;
        if empty { self.rooms.remove(&code); self.logf(&format!("Raum {} geschlossen", code)); }
        if let Some(m) = game_over { for o in others { self.send(o, &m); } }
        if !empty { self.broadcast_room(&code); }
    }

    fn drop_conn(&mut self, a: SocketAddr, why: &str) {
        if let Some(h) = &self.ws { h.close(&a); }
        self.leave_room(a, if why.is_empty() { "ist weg" } else { why });
        if let Some(c) = self.conns.remove(&a) {
            if c.welcomed { self.chat_notice_others(a, &format!("{} ist weg", c.name)); }
            if c.welcomed { self.logf(&format!("{} getrennt ({}) - {} Spieler online", c.name, if why.is_empty() { "-" } else { why }, self.player_count())); }
        }
    }

    // ---------------- Spieltakt ----------------
    fn tick(&mut self, dt: i64) {
        let codes: Vec<String> = self.rooms.keys().cloned().collect();
        for code in codes { self.tick_room(&code, dt); }
    }

    fn tick_room(&mut self, code: &str, dt: i64) {
        let mut out: Vec<(SocketAddr, ServerMsg, bool)> = vec![];
        let mut log_line = None;
        let mut back_to_lobby = false;
        // Feldgröße aus den Fenstern aller Spieler (kleinster gemeinsamer) und Namen - vor dem veränderlichen Zugriff
        let (fw, fh, names) = self.rooms.get(code).map(|r| {
            let present: Vec<SocketAddr> = r.slots.iter().flatten().copied().collect();
            let screens: Vec<Option<(u32, u32)>> = present.iter().map(|a| self.conns.get(a).and_then(|c| c.screen)).collect();
            let size = screens.iter().skip(1).fold(screens.first().copied().flatten(), |acc, s| match (acc, *s) {
                (Some(a), Some(b)) => Some((a.0.min(b.0), a.1.min(b.1))),
                _ => None,
            });
            let (w, h) = field_size_for(size, size);
            let names: Vec<String> = present.iter().map(|a| self.conns.get(a).map(|c| c.name.clone()).unwrap_or_default()).collect();
            (w, h, names)
        }).unwrap_or((FIELD_W, FIELD_H, vec![]));
        {
            let Some(r) = self.rooms.get_mut(code) else { return; };
            let slots = r.slots.clone();
            match &mut r.phase {
                Phase::Lobby => {}
                Phase::Countdown { ends, shown } => {
                    let left = ends.saturating_duration_since(Instant::now());
                    let secs = (left.as_millis() as u64).div_ceil(1000) as u8;
                    if secs == 0 {
                        let seed: u64 = rand::random();
                        let cfg = sim2p::Config { difficulty: r.difficulty.clone(), vs: r.mode == Mode::Vs, obstacle_weights: vec![("normal".into(), 100.0)], seed };
                        let order: Vec<usize> = (0..slots.len()).filter(|&i| slots[i].is_some()).collect();
                        let n = order.len();
                        for (k, &slot) in order.iter().enumerate() {
                            if let Some(a) = slots[slot] {
                                out.push((a, ServerMsg::Start { seed, mode: r.mode, difficulty: r.difficulty.clone(), width: fw, height: fh, you: k as u8, names: names.clone() }, true));
                            }
                        }
                        r.phase = Phase::Playing(Box::new(Game { sim: Sim2p::new_n(cfg, n, fw, fh), fw, fh, frame: 0, order, inputs: vec![Input::default(); n], input_frame: vec![0; n], events: VecDeque::new() }));
                        log_line = Some(format!("Raum {}: Runde gestartet ({} Spieler, Feld {}x{})", code, n, fw, fh));
                    } else if secs != *shown {
                        *shown = secs;
                        for a in slots.iter().flatten() { out.push((*a, ServerMsg::Countdown { seconds: secs }, true)); }
                    }
                }
                Phase::Playing(g) => {
                    g.frame += 1;
                    let res = g.sim.step(dt, &g.inputs, g.fw, g.fh);
                    for e in res.events { g.events.push_back((g.frame, e)); }
                    while g.events.front().map(|e| e.0 + EVENT_HISTORY_FRAMES < g.frame).unwrap_or(false) { g.events.pop_front(); }
                    if g.frame % SNAPSHOT_EVERY == 0 || res.game_over {
                        for (k, &slot) in g.order.iter().enumerate() {
                            if let Some(a) = slots[slot] {
                                let s = Snapshot::from_sim(&g.sim, g.frame, g.events.iter().copied().collect(), g.input_frame[k]);
                                out.push((a, ServerMsg::Snapshot(s), false));
                            }
                        }
                    }
                    if res.game_over {
                        for a in slots.iter().flatten() {
                            out.push((*a, ServerMsg::GameOver { winner: res.winner, score: g.sim.score, frames: g.frame, reason: String::new() }, true));
                        }
                        log_line = Some(format!("Raum {}: Runde vorbei (Score {}, Sieger {:?})", code, g.sim.score, res.winner));
                        back_to_lobby = true;
                    }
                }
            }
            if back_to_lobby { r.phase = Phase::Lobby; r.unready_all(); }
        }
        for (a, m, reliable) in out { if reliable { self.send(a, &m) } else { self.send_u(a, &m) } }
        if let Some(l) = log_line { self.logf(&l); }
        if back_to_lobby { self.broadcast_room(code); }
    }
}
