//! Der Multiplayer-Server: Verbindungen, Räume (je 2 Spieler), Countdown und die
//! eigentliche Runde - gerechnet mit derselben Simulation wie der lokale 2P-Modus.

use crate::config::Config;
use jnd_core::net::channel::{recv_all, send_raw, Lossy, Packet, Peer};
use jnd_core::net::proto::*;
use jnd_core::sim2p::{self, Input, SimEvent, Sim2p, Winner};
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
    /// Chat-Spamschutz (Token-Eimer): verfügbare Nachrichten und letzter Stand
    chat_tokens: f64,
    chat_at: Instant,
}

struct Game {
    sim: Sim2p,
    frame: u64,
    inputs: [Input; 2],
    input_frame: [u64; 2],
    events: VecDeque<(u64, SimEvent)>,
}

enum Phase { Lobby, Countdown { ends: Instant, shown: u8 }, Playing(Box<Game>) }

struct Room {
    code: String,
    mode: Mode,
    difficulty: String,
    slots: [Option<SocketAddr>; 2],
    ready: [bool; 2],
    phase: Phase,
}

impl Room {
    fn count(&self) -> u8 { self.slots.iter().filter(|s| s.is_some()).count() as u8 }
    fn slot_of(&self, a: SocketAddr) -> Option<usize> { self.slots.iter().position(|s| *s == Some(a)) }
}

pub type Logger = Box<dyn FnMut(&str) + Send>;

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
}

fn tick_dt_ms(n: u64) -> i64 { ((n * 1000) / TICK_HZ - ((n - 1) * 1000) / TICK_HZ) as i64 }

impl Server {
    pub fn bind(cfg: Config, log: Logger) -> std::io::Result<Server> {
        let sock = UdpSocket::bind((cfg.bind.as_str(), cfg.port))?;
        sock.set_nonblocking(true)?;
        Ok(Server { sock, cfg, conns: HashMap::new(), rooms: BTreeMap::new(), next_id: 1, start: Instant::now(), ticks_done: 0, loss: Lossy::default(), log })
    }

    pub fn local_addr(&self) -> SocketAddr { self.sock.local_addr().expect("lokale Adresse") }
    pub fn player_count(&self) -> usize { self.conns.values().filter(|c| c.welcomed).count() }
    pub fn room_count(&self) -> usize { self.rooms.len() }

    fn logf(&mut self, s: &str) { (self.log)(s); }

    /// Läuft, bis `stop` gesetzt wird
    pub fn run(&mut self, stop: &AtomicBool) {
        while !stop.load(Ordering::Relaxed) {
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
        for (from, p) in pkts { self.on_packet(from, p); }

        let target = self.start.elapsed().as_millis() as u64 * TICK_HZ / 1000;
        if target > self.ticks_done + MAX_CATCHUP_TICKS { self.ticks_done = target - MAX_CATCHUP_TICKS; }
        while self.ticks_done < target {
            self.ticks_done += 1;
            let dt = tick_dt_ms(self.ticks_done.max(1));
            self.tick(dt);
        }

        let now = Instant::now();
        let mut dead = vec![];
        for (a, c) in self.conns.iter_mut() {
            c.peer.update(&self.sock);
            if c.peer.dead { dead.push((*a, "Zeitüberschreitung")); }
            else if c.remove_after.map(|t| now >= t).unwrap_or(false) { dead.push((*a, "")); }
        }
        for (a, why) in dead { self.drop_conn(a, why); }
    }

    // ---------------- Senden ----------------
    fn send(&mut self, a: SocketAddr, m: &ServerMsg) {
        if let Some(c) = self.conns.get_mut(&a) { c.peer.send_reliable(&self.sock, encode(m)); }
    }
    fn send_u(&mut self, a: SocketAddr, m: &ServerMsg) {
        if let Some(c) = self.conns.get_mut(&a) { c.peer.send_unreliable(&self.sock, encode(m)); }
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
                send_raw(&self.sock, from, &Packet::Unreliable { ack: 0, data: encode(&ServerMsg::Info(self.info())) }, self.loss);
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
        self.conns.insert(a, Conn { id, peer, name: String::new(), welcomed: false, room: None, remove_after: None, chat_tokens: CHAT_BURST, chat_at: Instant::now() });
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
                if game_version == GAME_VERSION && proto != PROTO_VERSION {
                    return self.reject(a, format!("Server und Spiel passen nicht zusammen (Protokoll {} statt {}). Der Server muss aktualisiert werden.", PROTO_VERSION, proto));
                }
                if proto != PROTO_VERSION || game_version != GAME_VERSION {
                    return self.reject(a, format!("Andere Version: Server hat {}, du hast {}. Bitte dieselbe Version benutzen.", GAME_VERSION, game_version));
                }
                if !self.cfg.password.is_empty() && password != self.cfg.password {
                    return self.reject(a, "Falsches Passwort".into());
                }
                let name = clean_name(&name);
                let (sn, motd) = (self.cfg.name.clone(), self.cfg.motd.clone());
                let id = {
                    let c = self.conns.get_mut(&a).unwrap();
                    c.welcomed = true;
                    c.name = name.clone();
                    c.id
                };
                self.logf(&format!("{} verbunden ({}) - {} Spieler online", name, a, self.player_count()));
                self.send(a, &ServerMsg::Welcome { client_id: id, server_name: sn, motd });
                self.chat_notice_others(a, &format!("{} ist da", name));
            }
            return;
        }
        match m {
            ClientMsg::Hello { .. } | ClientMsg::Info => {}
            ClientMsg::ListRooms => {
                let list: Vec<RoomInfo> = self.rooms.values()
                    .map(|r| RoomInfo {
                        code: r.code.clone(), mode: r.mode, difficulty: r.difficulty.clone(), players: r.count(),
                        host: r.slots.iter().flatten().next().and_then(|h| self.conns.get(h)).map(|c| c.name.clone()).unwrap_or_default(),
                        playing: matches!(r.phase, Phase::Playing(_)),
                    })
                    // offene Räume zuerst
                    .collect::<Vec<_>>();
                let mut list = list;
                list.sort_by_key(|r| (r.playing, r.players != 1));
                list.truncate(MAX_ROOMS_IN_LIST);
                self.send(a, &ServerMsg::RoomList(list));
            }
            ClientMsg::CreateRoom { mode, difficulty } => {
                if !DIFFICULTIES.contains(&difficulty.as_str()) { return self.send(a, &ServerMsg::Error("Unbekannte Schwierigkeit".into())); }
                self.leave_room(a, "hat den Raum verlassen");
                if self.rooms.len() as u32 >= self.cfg.max_rooms { return self.send(a, &ServerMsg::Error("Server ist voll (keine freien Räume)".into())); }
                let code = self.new_code();
                self.rooms.insert(code.clone(), Room { code: code.clone(), mode, difficulty: difficulty.clone(), slots: [Some(a), None], ready: [false, false], phase: Phase::Lobby });
                if let Some(c) = self.conns.get_mut(&a) { c.room = Some(code.clone()); }
                let n = self.conns[&a].name.clone();
                self.logf(&format!("Raum {} erstellt von {} ({:?}, {})", code, n, mode, difficulty));
                self.broadcast_room(&code);
            }
            ClientMsg::JoinRoom { code } => {
                let code = code.trim().to_uppercase();
                if self.conns[&a].room.as_deref() == Some(code.as_str()) { return self.broadcast_room(&code); }
                let Some(r) = self.rooms.get(&code) else { return self.send(a, &ServerMsg::Error(format!("Raum {} gibt es nicht", code))); };
                if matches!(r.phase, Phase::Playing(_)) { return self.send(a, &ServerMsg::Error("In diesem Raum läuft gerade eine Runde".into())); }
                let Some(free) = r.slots.iter().position(|s| s.is_none()) else { return self.send(a, &ServerMsg::Error("Raum ist voll".into())); };
                self.leave_room(a, "hat den Raum verlassen");
                let Some(r) = self.rooms.get_mut(&code) else { return; };
                r.slots[free] = Some(a);
                r.ready = [false, false];
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
                let start = matches!(r.phase, Phase::Lobby) && r.count() == 2 && r.ready == [true, true];
                if start { r.phase = Phase::Countdown { ends: Instant::now() + Duration::from_secs(COUNTDOWN_SECS as u64), shown: 0 }; }
                self.broadcast_room(&code);
            }
            ClientMsg::Input { frame, input } => {
                let Some(code) = self.conns[&a].room.clone() else { return; };
                let Some(r) = self.rooms.get_mut(&code) else { return; };
                let Some(i) = r.slot_of(a) else { return; };
                if let Phase::Playing(g) = &mut r.phase {
                    if frame > g.input_frame[i] { g.inputs[i] = input; g.input_frame[i] = frame; }
                }
            }
            ClientMsg::Ping { t } => self.send_u(a, &ServerMsg::Pong { t }),
            ClientMsg::Bye => self.drop_conn(a, "hat das Spiel verlassen"),
            ClientMsg::Chat { text } => self.on_chat(a, &text),
        }
    }

    // ---------------- Chat ----------------
    /// Nur live weiterreichen: kein Verlauf, kein Log des Inhalts. Wer später kommt, sieht nichts davon.
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

    fn new_code(&self) -> String {
        let mut rng = rand::thread_rng();
        loop {
            let c: String = (0..4).map(|_| CODE_CHARS[rng.gen_range(0..CODE_CHARS.len())] as char).collect();
            if !self.rooms.contains_key(&c) { return c; }
        }
    }

    fn broadcast_room(&mut self, code: &str) {
        let Some(r) = self.rooms.get(code) else { return; };
        let players: [Option<LobbyPlayer>; 2] = [0, 1].map(|i| r.slots[i].and_then(|a| self.conns.get(&a)).map(|c| LobbyPlayer { name: c.name.clone(), ready: r.ready[i] }));
        let msgs: Vec<(SocketAddr, ServerMsg)> = (0..2).filter_map(|i| r.slots[i].map(|a| (a, ServerMsg::RoomState {
            code: r.code.clone(), mode: r.mode, difficulty: r.difficulty.clone(), you: i as u8, players: players.clone(),
        }))).collect();
        for (a, m) in msgs { self.send(a, &m); }
    }

    /// Spieler verlässt seinen Raum (freiwillig oder Verbindung weg)
    fn leave_room(&mut self, a: SocketAddr, why: &str) {
        let Some(code) = self.conns.get_mut(&a).and_then(|c| c.room.take()) else { return; };
        let name = self.conns.get(&a).map(|c| c.name.clone()).unwrap_or_default();
        let Some(r) = self.rooms.get_mut(&code) else { return; };
        let Some(i) = r.slot_of(a) else { return; };
        r.slots[i] = None;
        r.ready = [false, false];
        let other = r.slots[1 - i];
        let mut game_over: Option<ServerMsg> = None;
        match &mut r.phase {
            Phase::Countdown { .. } => r.phase = Phase::Lobby,
            Phase::Playing(g) => {
                if r.mode == Mode::Vs || other.is_none() {
                    let winner = if i == 0 { Winner::P2 } else { Winner::P1 };
                    game_over = Some(ServerMsg::GameOver { winner: Some(winner), score: g.sim.score, frames: g.frame, reason: format!("{} {}", name, why) });
                    r.phase = Phase::Lobby;
                } else {
                    // Koop: der Partner spielt allein weiter
                    g.sim.eliminate(i);
                }
            }
            Phase::Lobby => {}
        }
        let empty = r.count() == 0;
        if empty { self.rooms.remove(&code); self.logf(&format!("Raum {} geschlossen", code)); }
        if let (Some(o), Some(m)) = (other, game_over) { self.send(o, &m); }
        if !empty { self.broadcast_room(&code); }
    }

    fn drop_conn(&mut self, a: SocketAddr, why: &str) {
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
        {
            let Some(r) = self.rooms.get_mut(code) else { return; };
            let slots = r.slots;
            match &mut r.phase {
                Phase::Lobby => {}
                Phase::Countdown { ends, shown } => {
                    let left = ends.saturating_duration_since(Instant::now());
                    let secs = (left.as_millis() as u64).div_ceil(1000) as u8;
                    if secs == 0 {
                        let seed: u64 = rand::random();
                        let cfg = sim2p::Config { difficulty: r.difficulty.clone(), vs: r.mode == Mode::Vs, obstacle_weights: vec![("normal".into(), 100.0)], seed };
                        for a in slots.iter().flatten() {
                            out.push((*a, ServerMsg::Start { seed, mode: r.mode, difficulty: r.difficulty.clone(), width: FIELD_W, height: FIELD_H }, true));
                        }
                        r.phase = Phase::Playing(Box::new(Game { sim: Sim2p::new(cfg, FIELD_W, FIELD_H), frame: 0, inputs: [Input::default(); 2], input_frame: [0, 0], events: VecDeque::new() }));
                        log_line = Some(format!("Raum {}: Runde gestartet", code));
                    } else if secs != *shown {
                        *shown = secs;
                        for a in slots.iter().flatten() { out.push((*a, ServerMsg::Countdown { seconds: secs }, true)); }
                    }
                }
                Phase::Playing(g) => {
                    g.frame += 1;
                    let res = g.sim.step(dt, g.inputs, FIELD_W, FIELD_H);
                    for e in res.events { g.events.push_back((g.frame, e)); }
                    while g.events.front().map(|e| e.0 + EVENT_HISTORY_FRAMES < g.frame).unwrap_or(false) { g.events.pop_front(); }
                    if g.frame % SNAPSHOT_EVERY == 0 || res.game_over {
                        for (i, a) in slots.iter().enumerate() {
                            if let Some(a) = a {
                                let s = Snapshot::from_sim(&g.sim, g.frame, g.events.iter().copied().collect(), g.input_frame[i]);
                                out.push((*a, ServerMsg::Snapshot(s), false));
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
            if back_to_lobby { r.phase = Phase::Lobby; r.ready = [false, false]; }
        }
        for (a, m, reliable) in out { if reliable { self.send(a, &m) } else { self.send_u(a, &m) } }
        if let Some(l) = log_line { self.logf(&l); }
        if back_to_lobby { self.broadcast_room(code); }
    }
}
