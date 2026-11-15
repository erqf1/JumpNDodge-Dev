//! Online-Multiplayer (nur Rust-Version): Serverauswahl, Lobby mit Räumen, Raum (Bereit/Countdown),
//! die Runde selbst und das Ergebnis mit Rematch.
//!
//! Netzwerk läuft nie blockierend im Hauptthread: Serverabfragen und Verbindungsaufbau laufen
//! in eigenen Threads, die Runde nutzt `jnd_core::net::netround::NetRound`
//! (eigener Spieler vorhergesagt, Gegner interpoliert). Online gibt es keine Pause.

use super::core::*;
use super::state::{Snd, G};
use crate::pg::*;
use crate::util::*;
use jnd_core::net::client::{fetch_server_list, lan_scan, query_info, Client, ConnectOptions};
use jnd_core::net::netround::{DrawPlayer, NetRound};
use jnd_core::net::proto::*;
use jnd_core::sim2p::{Input, ObstacleKind, SimEvent, Winner};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// Offizieller Server (DNS-Eintrag zeigt auf den Heimserver; nicht immer online)
pub const OFFICIAL_SERVER: &str = "play.mert.win";
/// Wo es den Server zum Selbsthosten gibt
pub const HOST_INFO_URL: &str = "mert.win/jump/server";

/// Farben der Spieler (P1 bis P8). Kein Grün (1-UP ist grün, und nie Orange neben Grün), kein Rot (Hindernisse).
pub const PLAYER_COLORS: [(i32, i32, i32); 8] = [
    (70, 130, 255), (225, 80, 170), (245, 200, 50), (60, 200, 225),
    (150, 105, 240), (245, 140, 55), (225, 225, 240), (185, 135, 95),
];
/// Die ersten zwei (lokaler 2-Spieler-Modus)
pub const MP_COLORS: [(i32, i32, i32); 2] = [PLAYER_COLORS[0], PLAYER_COLORS[1]];
fn pcol(i: usize) -> (i32, i32, i32) { PLAYER_COLORS[i % PLAYER_COLORS.len()] }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen { Browser, Connecting, Lobby, Room, Playing, Result }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field { None, Addr, Code, Chat }

enum InfoState { Checking, Online(ServerInfo, u32), Offline }

/// Eine Chat-Zeile. Lebt nur im Speicher, solange man verbunden ist (kein Verlauf, nichts auf der Platte).
struct ChatLine { from: String, text: String, own: bool, system: bool, at: Instant }
const CHAT_KEEP: usize = 60;

#[derive(Clone)]
struct RoomView { code: String, mode: Mode, difficulty: String, you: usize, players: Vec<Option<LobbyPlayer>> }
impl RoomView {
    fn present(&self) -> usize { self.players.iter().filter(|p| p.is_some()).count() }
    fn all_ready(&self) -> bool { self.present() >= 2 && self.players.iter().flatten().all(|p| p.ready) }
    fn me_ready(&self) -> bool { self.players.get(self.you).and_then(|p| p.as_ref()).map(|p| p.ready).unwrap_or(false) }
}

struct Mp {
    screen: Screen,
    client: Option<Client>,
    connect_rx: Option<Receiver<Result<Client, String>>>,
    connecting_addr: String,
    connect_started: Instant,
    addr_input: String,
    code_input: String,
    focus: Field,
    infos: HashMap<String, InfoState>,
    info_tx: Sender<(String, Option<(ServerInfo, u32)>)>,
    info_rx: Receiver<(String, Option<(ServerInfo, u32)>)>,
    last_info_query: Option<Instant>,
    message: String,
    message_err: bool,
    message_until: Instant,
    rooms: Vec<RoomInfo>,
    last_list: Instant,
    create_mode: Mode,
    create_diff: usize,
    /// Plätze für einen neuen Raum (2 bis 8)
    create_max: usize,
    /// Namen der Spieler in der laufenden Runde (Rundenreihenfolge)
    round_names: Vec<String>,
    /// Raum-Chef (Platz) und privat - kommt mit RoomExtra (alte Server: Platz 0, öffentlich)
    room_host: usize,
    room_private: bool,
    /// Schnell-Nachrichten: (in der Runde?, Spieler, Text-Nr., seit)
    emotes: Vec<(bool, usize, u8, Instant)>,
    /// Schnell spielen gesendet (für den Hinweis bei alten Servern); nach dem Verbinden gleich losspielen
    quick_sent: Option<Instant>,
    auto_quick: bool,
    /// Aussehen der anderen: im Raum nach Platz, in der Runde nach Spielernummer
    room_looks: Vec<PlayerLook>,
    round_looks: Vec<PlayerLook>,
    /// Spur-Partikel je Spieler (Runde)
    trails: HashMap<usize, Vec<super::gameplay::Particle>>,
    /// Test/Dev: dem ersten laufenden Raum zuschauen
    auto_spectate: bool,
    room: Option<RoomView>,
    countdown: Option<u8>,
    round: Option<NetRound>,
    round_started: Instant,
    acc_ms: f64,
    jump_latch: bool,
    /// Sprung vormerken: bis dahin weiter "Sprung" melden (springt dann bei der Landung)
    jump_hold_until: Option<Instant>,
    leave_confirm_until: Option<Instant>,
    result: Option<(Option<Winner>, i64, String, u64)>,
    server_name: String,
    motd: String,
    ping_ms: Option<u32>,
    last_ping_sent: Instant,
    /// Buttons des letzten Bildes: (Rechteck, Aktion)
    buttons: Vec<(Rect, String)>,
    /// Test/Dev: dem ersten offenen Raum beitreten, sobald einer in der Liste steht
    auto_join: Option<Instant>,
    /// Test/Dev: bereit melden, sobald man in einem vollen Raum ist
    auto_ready: bool,
    fx: super::art::Fx,
    client_id: u32,
    chat: VecDeque<ChatLine>,
    chat_input: String,
    /// Zuletzt an den Server gemeldete Fenstergröße (für die Feldbreite)
    sent_screen: (i32, i32),
    /// Server-Browser: öffentliche Liste (vom offiziellen Server) und Server im eigenen Netzwerk
    public_servers: Vec<ListedServer>,
    lan_servers: Vec<(String, ServerInfo, u32)>,
    list_tx: Sender<ListUpd>,
    list_rx: Receiver<ListUpd>,
    servers_at: Option<Instant>,
    servers_busy: u8,
    browser_scroll: i32,
    /// Nach dem Verbinden diesem Raum beitreten (Einladung)
    join_code: Option<String>,
    /// Freunde-einladen-Fenster offen
    invite_open: bool,
    /// Ping je Platz im Raum (ms, 0 = unbekannt)
    room_pings: Vec<u16>,
}

/// Ergebnis der Server-Suche im Hintergrund
enum ListUpd { Public(Vec<ListedServer>), Lan(Vec<(String, ServerInfo, u32)>) }

/// Einladung eines Freundes (aus einer Direktnachricht mit `[jnd-join ADRESSE CODE]`)
#[derive(Clone)]
struct Invite { from_user: String, from_name: String, addr: String, code: String, at: Instant }

thread_local! {
    static INVITES: std::cell::RefCell<Vec<Invite>> = const { std::cell::RefCell::new(Vec::new()) };
    static INVITE_SEEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    static INVITE_POLL: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
    /// Banner im Hauptmenü: (Beitreten, Wegklicken)
    static INVITE_RECTS: std::cell::Cell<Option<(Rect, Rect)>> = const { std::cell::Cell::new(None) };
    /// Freundesliste fürs Einladen: None = lädt, Some(Err) = Fehler
    static FRIENDS: std::cell::RefCell<Option<Result<Vec<(String, String)>, String>>> = const { std::cell::RefCell::new(None) };
    /// Einladung an Benutzername: 1 sendet, 2 gesendet, 3 zu schnell, 4 Fehler
    static INVITE_SENT: std::cell::RefCell<HashMap<String, u8>> = std::cell::RefCell::new(HashMap::new());
}

/// Einladung aus einem Nachrichtentext lesen: (Adresse, Raumcode)
pub fn parse_invite(body: &str) -> Option<(String, String)> {
    let i = body.find("[jnd-join ")?;
    let rest = &body[i + 10..];
    let end = rest.find(']')?;
    let mut it = rest[..end].split_whitespace();
    let (a, c) = (it.next()?, it.next()?);
    if a.len() > 100 || !a.chars().all(|ch| ch.is_ascii_alphanumeric() || ".-:/[]_".contains(ch)) { return None; }
    if c.is_empty() || c.len() > 8 || !c.chars().all(|ch| ch.is_ascii_alphanumeric()) { return None; }
    Some((a.to_string(), c.to_uppercase()))
}

impl Mp {
    fn new() -> Mp {
        let (info_tx, info_rx) = channel();
        let (list_tx, list_rx) = channel();
        let now = Instant::now();
        Mp {
            screen: Screen::Browser, client: None, connect_rx: None, connecting_addr: String::new(), connect_started: now,
            addr_input: String::new(), code_input: String::new(), focus: Field::None, infos: HashMap::new(), info_tx, info_rx,
            last_info_query: None, message: String::new(), message_err: false, message_until: now, rooms: vec![], last_list: now,
            create_mode: Mode::Vs, create_diff: 1, create_max: 2, round_names: vec![], room_host: 0, room_private: false, emotes: vec![], quick_sent: None, auto_quick: false, room_looks: vec![], round_looks: vec![], trails: HashMap::new(), auto_spectate: false, room: None, countdown: None, round: None, round_started: now, acc_ms: 0.0,
            jump_latch: false, jump_hold_until: None, leave_confirm_until: None, result: None, server_name: String::new(), motd: String::new(),
            ping_ms: None, last_ping_sent: now, buttons: vec![], auto_join: None, auto_ready: false, fx: super::art::Fx::new(),
            client_id: 0, chat: VecDeque::new(), chat_input: String::new(), sent_screen: (0, 0),
            public_servers: vec![], lan_servers: vec![], list_tx, list_rx, servers_at: None, servers_busy: 0, browser_scroll: 0,
            join_code: None, invite_open: false, room_pings: vec![],
        }
    }
    fn say(&mut self, text: &str, err: bool) {
        self.message = text.into();
        self.message_err = err;
        self.message_until = Instant::now() + Duration::from_secs(if err { 6 } else { 3 });
    }
    fn send(&mut self, m: ClientMsg) { if let Some(c) = self.client.as_mut() { c.send(&m); } }
    fn chat_push(&mut self, from: String, text: String, own: bool, system: bool) {
        self.chat.push_back(ChatLine { from, text, own, system, at: Instant::now() });
        while self.chat.len() > CHAT_KEEP { self.chat.pop_front(); }
    }
}

/// Text in Zeilen umbrechen (Wörter; überlange Wörter werden hart getrennt). `first_indent` = Platz,
/// den die erste Zeile schon belegt (Name davor).
fn wrap_text(f: &Font, text: &str, max_w: i32, first_indent: i32) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    let mut cur = String::new();
    let avail = |n: usize| if n == 0 { max_w - first_indent } else { max_w };
    for word in text.split(' ') {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{} {}", cur, word) };
        if f.size(&cand).0 <= avail(lines.len()) { cur = cand; continue; }
        if !cur.is_empty() { lines.push(std::mem::take(&mut cur)); }
        // Wort allein zu lang: Zeichen für Zeichen trennen
        let mut piece = String::new();
        for ch in word.chars() {
            let c2 = format!("{}{}", piece, ch);
            if f.size(&c2).0 > avail(lines.len()) && !piece.is_empty() { lines.push(std::mem::take(&mut piece)); piece.push(ch); } else { piece = c2; }
        }
        cur = piece;
    }
    if !cur.is_empty() || lines.is_empty() { lines.push(cur); }
    lines
}

thread_local! {
    /// Spielfeld der laufenden Runde (vom Server beim Start vorgegeben)
    static FIELD: std::cell::Cell<(i32, i32)> = const { std::cell::Cell::new((FIELD_W, FIELD_H)) };
}
fn field() -> (i32, i32) { FIELD.with(|f| f.get()) }

thread_local! {
    /// Läuft gerade eine Online-Runde? Dann gilt wie im Einzelspieler keine GUI-Skalierung.
    static ROUND_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub fn round_active() -> bool { ROUND_ACTIVE.with(|r| r.get()) }

fn now_ms() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0) }

fn mode_name(g: &G, m: Mode) -> String { match m { Mode::Vs => "VS".into(), Mode::Coop => g.tde("Koop", "Co-op") } }

impl G {
    pub fn open_multiplayer(&mut self) {
        self.play_sound(Snd::MenuClick);
        for f in super::devconsole::ALL_SCREEN_FLAGS { self.set_screen_flag(f, false); }
        self.in_multiplayer = true;
    }

    fn mp_player_name(&self) -> String {
        let n = self.current_profile_username.clone().or_else(|| self.current_profile_name.clone()).unwrap_or_default();
        clean_name(&n)
    }

    fn mp_recent(&self) -> Vec<String> { self.settings.gstrs("mp_recent_servers") }

    fn mp_remember(&mut self, addr: &str) {
        let mut l = self.mp_recent();
        l.retain(|a| !a.eq_ignore_ascii_case(addr));
        if !addr.eq_ignore_ascii_case(OFFICIAL_SERVER) { l.insert(0, addr.to_string()); }
        l.truncate(5);
        self.settings.insert("mp_recent_servers".into(), json!(l));
        self.save_section("settings", None);
    }

    fn mp_official(&self) -> String {
        let s = self.settings.gs("mp_official_server", "");
        if s.trim().is_empty() { OFFICIAL_SERVER.to_string() } else { s }
    }

    /// Serverstatus im Hintergrund abfragen (alle 5 s)
    fn mp_query_infos(&mut self, mp: &mut Mp) {
        while let Ok((addr, res)) = mp.info_rx.try_recv() {
            mp.infos.insert(addr, match res { Some((i, p)) => InfoState::Online(i, p), None => InfoState::Offline });
        }
        while let Ok(u) = mp.list_rx.try_recv() {
            mp.servers_busy = mp.servers_busy.saturating_sub(1);
            match u {
                ListUpd::Public(l) => { mp.public_servers = l; mp.last_info_query = None; }
                ListUpd::Lan(l) => {
                    for (a, i, p) in &l { mp.infos.insert(a.clone(), InfoState::Online(i.clone(), *p)); }
                    mp.lan_servers = l;
                }
            }
        }
        if mp.servers_busy == 0 && mp.servers_at.map(|t| t.elapsed() >= Duration::from_secs(30)).unwrap_or(true) { self.mp_refresh_servers(mp); }
        if mp.last_info_query.map(|t| t.elapsed() < Duration::from_secs(5)).unwrap_or(false) { return; }
        mp.last_info_query = Some(Instant::now());
        let mut addrs = vec![self.mp_official()];
        addrs.extend(self.mp_recent());
        for s in mp.public_servers.iter().take(12) { if !addrs.iter().any(|a| a.eq_ignore_ascii_case(&s.address)) { addrs.push(s.address.clone()); } }
        for a in addrs {
            mp.infos.entry(a.clone()).or_insert(InfoState::Checking);
            let tx = mp.info_tx.clone();
            std::thread::spawn(move || {
                let r = query_info(&a, Duration::from_millis(1500)).ok().map(|(i, d)| (i, d.as_millis() as u32));
                let _ = tx.send((a, r));
            });
        }
    }

    fn mp_connect(&mut self, mp: &mut Mp, addr: &str) {
        let addr = addr.trim().to_string();
        if addr.is_empty() { mp.say(&self.tde("Bitte eine Adresse eingeben", "Please enter an address"), true); return; }
        if let Some(c) = mp.client.take() { c.disconnect(); }
        let (tx, rx) = channel();
        let opts = ConnectOptions::new(&self.mp_player_name());
        let a = addr.clone();
        std::thread::spawn(move || { let _ = tx.send(Client::connect(&a, opts)); });
        mp.connect_rx = Some(rx);
        mp.connecting_addr = addr;
        mp.connect_started = Instant::now();
        mp.screen = Screen::Connecting;
        mp.focus = Field::None;
        println!("[MP] verbinde mit {}", mp.connecting_addr);
    }

    fn mp_disconnect(&mut self, mp: &mut Mp, msg: Option<&str>) {
        if let Some(c) = mp.client.take() { c.disconnect(); }
        mp.connect_rx = None;
        mp.room = None;
        mp.round = None;
        mp.countdown = None;
        mp.result = None;
        mp.screen = Screen::Browser;
        mp.last_info_query = None;
        // Chat ist flüchtig: mit der Verbindung ist er weg
        mp.chat.clear();
        mp.chat_input.clear();
        if mp.focus == Field::Chat { mp.focus = Field::None; }
        match msg { Some(m) => mp.say(m, true), None => mp.message.clear() }
        if self.mixer_playing_ingame { self.stop_music(); self.mixer_playing_ingame = false; self.play_menu_music(); }
    }

    fn mp_leave_screen(&mut self, mp: &mut Mp) {
        self.mp_disconnect(mp, None);
        self.in_multiplayer = false;
        self.in_online_hub = true;
    }

    // ------------------------------------------------------------------
    // Hauptschleife
    // ------------------------------------------------------------------
    pub fn multiplayer_loop(&mut self) {
        let mut mp = Mp::new();
        mp.addr_input = self.settings.gs("mp_last_addr", "");
        let mut clock = Clock::new();
        while self.in_multiplayer {
            ROUND_ACTIVE.with(|r| r.set(mp.screen == Screen::Playing));
            let fps = self.target_fps();
            let frame_ms = clock.tick_f(fps).min(250.0);
            self.sync_window_size();
            self.mp_network(&mut mp);
            for c in std::mem::take(&mut self._mp_dev_cmds) { self.mp_dev(&mut mp, &c); }
            if !self.in_multiplayer { break; }
            let evs = self.event_get();
            for ev in evs { self.mp_event(&mut mp, ev); if !self.in_multiplayer { break; } }
            if !self.in_multiplayer { break; }
            if mp.screen == Screen::Playing { self.mp_round_logic(&mut mp, frame_ms); }
            self.mp_draw(&mut mp);
        }
        if let Some(c) = mp.client.take() { c.disconnect(); }
        ROUND_ACTIVE.with(|r| r.set(false));
    }

    /// Netzwerk: Verbindungsaufbau, Nachrichten, Verbindungsverlust
    fn mp_network(&mut self, mp: &mut Mp) {
        if let Some(rx) = &mp.connect_rx {
            match rx.try_recv() {
                Ok(Ok(c)) => {
                    mp.connect_rx = None;
                    mp.server_name = c.server_name.clone();
                    mp.motd = c.motd.clone();
                    mp.client_id = c.client_id;
                    mp.client = Some(c);
                    mp.screen = Screen::Lobby;
                    mp.chat.clear();
                    mp.chat_input.clear();
                    let hint = self.tde("Verbunden. Chat der letzten 24 Stunden:", "Connected. Chat from the last 24 hours:");
                    mp.chat_push(String::new(), hint, false, true);
                    mp.rooms.clear();
                    mp.last_list = Instant::now() - Duration::from_secs(10);
                    let a = mp.connecting_addr.clone();
                    self.settings.insert("mp_last_addr".into(), json!(a));
                    self.mp_remember(&a);
                    println!("[MP] verbunden mit {} ({})", mp.server_name, a);
                    let look = self.mp_my_look();
                    mp.send(ClientMsg::Look(look));
                    if std::mem::take(&mut mp.auto_quick) { mp.send(ClientMsg::QuickPlay); mp.quick_sent = Some(Instant::now()); }
                    if let Some(code) = mp.join_code.take() { mp.send(ClientMsg::JoinRoom { code }); }
                    self.play_sound(Snd::Click);
                }
                Ok(Err(e)) => {
                    mp.connect_rx = None;
                    mp.screen = Screen::Browser;
                    println!("[MP] Verbindung fehlgeschlagen: {}", e);
                    mp.say(&e, true);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => { mp.connect_rx = None; mp.screen = Screen::Browser; }
            }
        }
        let Some(c) = mp.client.as_mut() else { return; };
        let msgs = c.poll();
        if c.is_dead() {
            println!("[MP] Verbindung verloren");
            let t = self.tde("Verbindung zum Server verloren", "Lost connection to the server");
            self.mp_disconnect(mp, Some(&t));
            return;
        }
        // Fenstergröße melden (beim Verbinden und wenn sich das Fenster ändert) - außerhalb einer Runde.
        // Echte Fenstergröße wie im Spiel (dort gilt keine GUI-Skalierung), nicht die Menü-Größe.
        let win = self.display.size();
        if mp.screen != Screen::Playing && mp.sent_screen != win {
            mp.sent_screen = win;
            mp.send(ClientMsg::Screen { w: win.0.max(1) as u32, h: win.1.max(1) as u32 });
        }
        if mp.last_ping_sent.elapsed() >= Duration::from_secs(2) {
            mp.last_ping_sent = Instant::now();
            mp.send(ClientMsg::Ping { t: now_ms() });
        }
        if mp.quick_sent.map(|t| t.elapsed() > Duration::from_secs(5)).unwrap_or(false) {
            mp.quick_sent = None;
            if mp.room.is_none() { mp.say(&self.tde("Der Server kennt „Schnell spielen“ noch nicht (Server-Update nötig).", "This server doesn't support Quick play yet (needs an update)."), true); }
        }
        mp.emotes.retain(|e| e.3.elapsed() < Duration::from_millis(2600));
        if mp.screen == Screen::Lobby && mp.last_list.elapsed() >= Duration::from_secs(3) {
            mp.last_list = Instant::now();
            mp.send(ClientMsg::ListRooms);
        }
        for m in msgs { self.mp_message(mp, m); if mp.client.is_none() { return; } }
    }

    fn mp_message(&mut self, mp: &mut Mp, m: ServerMsg) {
        match m {
            ServerMsg::RoomList(l) if mp.auto_spectate && l.iter().any(|r| r.playing) => {
                mp.auto_spectate = false;
                if let Some(r) = l.iter().find(|r| r.playing) { mp.send(ClientMsg::Spectate { code: r.code.clone() }); }
                mp.rooms = l;
            }
            ServerMsg::RoomList(l) => {
                mp.rooms = l;
                if mp.auto_join.map(|t| t.elapsed() < Duration::from_secs(30)).unwrap_or(false) && mp.room.is_none() {
                    if let Some(r) = mp.rooms.iter().find(|r| r.players >= 1 && r.players < r.max && !r.playing) {
                        let c = r.code.clone();
                        mp.auto_join = None;
                        mp.send(ClientMsg::JoinRoom { code: c });
                    } else {
                        mp.last_list = Instant::now() - Duration::from_millis(2500);
                    }
                }
            }
            ServerMsg::RoomState { code, mode, difficulty, you, players } => {
                mp.quick_sent = None;
                if mp.room.as_ref().map(|r| r.code != code).unwrap_or(true) {
                    println!("[MP] Raum {}", code);
                    mp.room_host = 0;
                    mp.room_private = false;
                    mp.emotes.clear();
                }
                mp.room = Some(RoomView { code, mode, difficulty, you: you as usize, players });
                if matches!(mp.screen, Screen::Lobby | Screen::Connecting) { mp.screen = Screen::Room; }
                if mp.auto_ready {
                    if let Some(r) = &mp.room {
                        if r.players.iter().all(|p| p.is_some()) && !r.me_ready() {
                            mp.auto_ready = false;
                            mp.send(ClientMsg::Ready(true));
                        }
                    }
                }
                let all_ready = mp.room.as_ref().map(|r| r.all_ready()).unwrap_or(false);
                if !all_ready { mp.countdown = None; }
            }
            ServerMsg::LeftRoom { reason } => {
                mp.room = None;
                mp.round = None;
                mp.countdown = None;
                mp.result = None;
                mp.screen = Screen::Lobby;
                mp.last_list = Instant::now() - Duration::from_secs(10);
                mp.say(&reason, false);
                if self.mixer_playing_ingame { self.stop_music(); self.mixer_playing_ingame = false; self.play_menu_music(); }
            }
            ServerMsg::Countdown { seconds } => {
                mp.countdown = Some(seconds);
                mp.screen = Screen::Room;
                self.play_sound(Snd::Click);
            }
            ServerMsg::Start { mode, difficulty, width, height, you, names, .. } => {
                if mp.focus == Field::Chat { mp.focus = Field::None; }
                let spectator = you == SPECTATOR;
                let you = you as usize;
                mp.round_names = names;
                mp.trails.clear();
                FIELD.with(|f| f.set((width.max(100), height.max(100))));
                mp.round = Some(if spectator { NetRound::spectate(mode, &difficulty, width, height) } else { NetRound::new(mode, &difficulty, you, width, height) });
                mp.round_started = Instant::now();
                mp.acc_ms = 0.0;
                mp.countdown = None;
                mp.result = None;
                mp.leave_confirm_until = None;
                mp.screen = Screen::Playing;
                mp.fx.clear();
                self.overlays.clear();
                self.play_ingame_music();
                self.mixer_playing_ingame = true;
                println!("[MP] Runde gestartet ({:?}, {}, du = P{})", mode, difficulty, you + 1);
            }
            ServerMsg::Snapshot(s) => {
                if let Some(r) = mp.round.as_mut() {
                    let you = r.you;
                    let evs = r.on_snapshot(s);
                    for e in evs {
                        self.mp_event_fx(mp, e, you);
                        if self.effects_on() {
                            let Some(r) = mp.round.as_ref() else { continue; };
                            let at = |i: usize| -> Option<Rect> { if i == r.you { Some(r.own_draw(1.0).rect) } else { r.other_draw(i).map(|p| p.rect) } };
                            match e {
                                SimEvent::Hit(i) | SimEvent::Died(i) => {
                                    if let Some(pr) = at(i) { let (x, y) = self.mp_field_point(pr.centerx(), pr.centery()); mp.fx.burst(x as f64, y as f64, super::art::HAZARD, 20); }
                                    if i == you {
                                        self.shake(6.0);
                                        if self.settings.gb("pad_rumble", false) { self.display.rumble(if matches!(e, SimEvent::Died(_)) { 1.0 } else { 0.6 }, if matches!(e, SimEvent::Died(_)) { 380 } else { 180 }); }
                                    }
                                }
                                SimEvent::OneUp(i) | SimEvent::Revived(i) => {
                                    if let Some(pr) = at(i) { let (x, y) = self.mp_field_point(pr.centerx(), pr.centery()); mp.fx.sparkle(x as f64, y as f64, super::art::ONEUP, 16); }
                                }
                            }
                        }
                    }
                }
            }
            ServerMsg::GameOver { winner, score, frames, reason } => {
                println!("[MP] Runde vorbei: Sieger {:?}, Score {}, {} Frames", winner, score, frames);
                let you = mp.round.as_ref().map(|r| r.you).or(mp.room.as_ref().map(|r| r.you)).unwrap_or(0);
                let mode = mp.round.as_ref().map(|r| r.mode).unwrap_or(Mode::Vs);
                if !mp.round.as_ref().map(|r| r.is_spectator()).unwrap_or(false) { self.mp_record_stats(mode, winner, you, frames); }
                mp.result = Some((winner, score, reason, frames));
                mp.screen = Screen::Result;
                if self.mixer_playing_ingame { self.stop_music(); self.mixer_playing_ingame = false; }
                self.play_sound_v(Snd::GameOver, "gameover_volume");
            }
            ServerMsg::Error(e) => {
                if e.contains("beendet") { self.mp_disconnect(mp, Some(&e)); } else { mp.say(&e, true); }
            }
            ServerMsg::Pong { t } => {
                let p = now_ms().saturating_sub(t) as u32;
                mp.ping_ms = Some(p);
                if mp.room.is_some() { mp.send(ClientMsg::MyPing(p.min(9999) as u16)); }
            }
            ServerMsg::Pings(p) => { mp.room_pings = p; }
            ServerMsg::Reject { reason } => self.mp_disconnect(mp, Some(&reason)),
            ServerMsg::Chat { from_id, from, text } => {
                let own = from_id != 0 && from_id == mp.client_id;
                mp.chat_push(from, text, own, from_id == 0);
            }
            ServerMsg::RoomExtra { host, private } => { mp.room_host = host as usize; mp.room_private = private; }
            ServerMsg::Looks { in_round, looks } => { if in_round { mp.round_looks = looks; } else { mp.room_looks = looks; } }
            ServerMsg::Emote { player, id, in_round } => {
                mp.emotes.retain(|e| !(e.0 == in_round && e.1 == player as usize));
                mp.emotes.push((in_round, player as usize, id, Instant::now()));
                self.play_sound(Snd::Click);
            }
            ServerMsg::Welcome { .. } | ServerMsg::Info(_) | ServerMsg::ServerList(_) | ServerMsg::Announced { .. } => {}
        }
    }

    fn mp_record_stats(&mut self, mode: Mode, winner: Option<Winner>, you: usize, frames: u64) {
        let s = &mut self.stats;
        let tr = s.gi("total_rounds", 0) + 1;
        s.insert("total_rounds".into(), json!(tr));
        let r = s.gi("mp_rounds", 0) + 1;
        s.insert("mp_rounds".into(), json!(r));
        let won = mode == Mode::Vs && winner.and_then(|w| w.index()) == Some(you);
        if won { let w = s.gi("mp_wins", 0) + 1; s.insert("mp_wins".into(), json!(w)); }
        let ttp = s.gf("total_time_played", 0.0) + frames as f64 / TICK_HZ as f64;
        s.insert("total_time_played".into(), Value::from(ttp));
        self.save_section("stats", None);
    }

    fn mp_event_fx(&mut self, mp: &Mp, e: SimEvent, you: usize) {
        let du = self.tde("Du", "You");
        let who = |i: usize| if i == you { du.clone() } else { mp.round_names.get(i).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("P{}", i + 1)) };
        let pos = |g: &G, i: usize| -> Option<(i32, i32)> {
            let r = mp.round.as_ref()?;
            let p = if i == r.you { r.own_draw(1.0) } else { r.other_draw(i)? };
            Some(g.mp_field_point(p.rect.centerx(), p.rect.top() - 20))
        };
        // Ab 3 Spielern würden sich die Texte über den Köpfen stapeln: für die anderen nur Effekt, kein Text/Ton
        let crowded = mp.round.as_ref().map(|r| r.player_count() > 2).unwrap_or(false);
        if crowded && matches!(e, SimEvent::Hit(i) | SimEvent::OneUp(i) if i != you) { return; }
        let (snd, text, col, at) = match e {
            SimEvent::Hit(i) => (Snd::Hit, format!("{} {}!", who(i), self.tde("getroffen", "hit")), pcol(i), pos(self, i)),
            SimEvent::Died(i) => (Snd::Hit, format!("{} {}", who(i), self.tde("ausgeschieden", "eliminated")), (220, 70, 70), None),
            SimEvent::OneUp(i) => (Snd::OneUp, format!("1-UP ({})", who(i)), super::art::ONEUP, pos(self, i)),
            SimEvent::Revived(i) => (Snd::OneUp, format!("{} {}", who(i), self.tde("wiederbelebt!", "revived!")), pcol(i), pos(self, i)),
        };
        if snd == Snd::Hit { self.play_sound_v(Snd::Hit, "hit_volume"); } else { self.play_sound(snd); }
        self.push_overlay(&text, col, 1.5, at);
    }

    /// Runde: feste 60 Logikschritte/s, Eingaben senden (keine Pause online)
    fn mp_round_logic(&mut self, mp: &mut Mp, frame_ms: f64) {
        let Some(r) = mp.round.as_mut() else { return; };
        r.advance(frame_ms);
        mp.acc_ms += frame_ms;
        let mut steps = 0;
        while mp.acc_ms >= LOGIC_TICK_MS {
            mp.acc_ms -= LOGIC_TICK_MS;
            steps += 1;
            if steps > 8 { mp.acc_ms = 0.0; break; }
            let pressed = self.any_action_pressed("jump") || mp.jump_latch;
            if mp.jump_latch && self.settings.gb("jump_buffer", true) { mp.jump_hold_until = Some(Instant::now() + Duration::from_millis(120)); }
            let buffered = mp.jump_hold_until.map(|t| Instant::now() < t).unwrap_or(false);
            let raw = Input {
                jump: pressed || buffered,
                left: self.any_action_pressed("left"),
                right: self.any_action_pressed("right"),
            };
            mp.jump_latch = false;
            mp.fx.update(LOGIC_TICK_MS);
            if !r.is_spectator() {
                let (f, inp) = r.tick(raw);
                if let Some(c) = mp.client.as_mut() { c.send_input(f, inp); }
            }
            // Spuren der Spieler (aus ihrem Aussehen): in der Luft Partikel hinten raus
            let n = r.player_count();
            let floor_y = r.floor_y;
            for i in 0..n {
                let trail = if i == r.you { self.mp_my_look().trail } else { mp.round_looks.get(i).map(|l| l.trail.clone()).unwrap_or_default() };
                if trail.is_empty() || super::art::legacy() { continue; }
                let d = if i == r.you { Some(r.own_draw(1.0)) } else { r.other_draw(i) };
                let Some(d) = d else { continue; };
                if !d.alive || d.rect.bottom() >= floor_y - 1 { continue; }
                let fr = self.mp_field_rect(d.rect);
                let list = mp.trails.entry(i).or_default();
                for _ in 0..super::gameplay::trail_particle_count(&trail) {
                    let sx = fr.centerx() as f64 + crate::util::uniform(-(fr.w as f64) * 0.35, fr.w as f64 * 0.2);
                    list.push(super::gameplay::spawn_trail_particle(&trail, sx, crate::util::uniform(-1.2, -0.2), fr.centery() as f64));
                }
                if list.len() > 400 { let cut = list.len() - 400; list.drain(0..cut); }
            }
            for list in mp.trails.values_mut() { super::gameplay::update_particles(list); }
        }
    }

    /// Eigenes Aussehen (für die anderen)
    fn mp_my_look(&self) -> PlayerLook {
        let c = &self.cosmetics_state;
        PlayerLook {
            headwear: c.gos("equipped_headwear").unwrap_or_default(), eyes: c.gos("equipped_eyes").unwrap_or_default(),
            nose: c.gos("equipped_nose").unwrap_or_default(), mouth: c.gos("equipped_mouth").unwrap_or_default(),
            trail: self.equipped_trails().first().cloned().unwrap_or_default(),
        }.clean()
    }

    /// Kosmetik eines anderen Spielers auf ein Rechteck zeichnen
    fn mp_draw_look(&self, bs: &Surface, r: Rect, look: &PlayerLook) {
        if look.headwear.is_empty() && look.eyes.is_empty() && look.nose.is_empty() && look.mouth.is_empty() { return; }
        let mut m = serde_json::Map::new();
        m.insert("equipped_headwear".into(), json!(look.headwear));
        m.insert("equipped_eyes".into(), json!(look.eyes));
        m.insert("equipped_nose".into(), json!(look.nose));
        m.insert("equipped_mouth".into(), json!(look.mouth));
        self.draw_player_cosmetics(bs, r, Some(&m));
    }

    // ------------------------------------------------------------------
    // Eingaben
    // ------------------------------------------------------------------
    fn mp_event(&mut self, mp: &mut Mp, ev: Event) {
        match ev {
            Event::Quit => { if let Some(c) = mp.client.take() { c.disconnect(); } self.quit(); }
            Event::KeyDown { key, unicode, mods } => self.mp_key(mp, key, &unicode, mods),
            Event::MouseWheel { y, .. } if mp.screen == Screen::Browser => { mp.browser_scroll = (mp.browser_scroll - y).max(0); }
            Event::MouseButtonDown { button: 1, .. } => {
                let pos = self.get_mouse_pos_logical();
                let hit = mp.buttons.iter().rev().find(|(r, _)| r.collidepoint(pos)).map(|(_, a)| a.clone());
                match hit {
                    Some(a) => self.mp_action(mp, &a),
                    None => mp.focus = Field::None,
                }
            }
            _ => {}
        }
    }

    fn mp_key(&mut self, mp: &mut Mp, key: i32, unicode: &str, mods: u16) {
        // Schnell-Nachrichten: 1-4 (nicht beim Tippen)
        if mp.focus == Field::None && matches!(mp.screen, Screen::Playing | Screen::Room | Screen::Result) && (49..=52).contains(&key) {
            mp.send(ClientMsg::Emote((key - 49) as u8));
            if mp.screen != Screen::Playing { return; }
        }
        if mp.screen == Screen::Playing {
            if self.action_matches_key(key, "jump") { mp.jump_latch = true; }
            if (key == K_ESCAPE || self.is_back_key(key)) && mp.round.as_ref().map(|r| r.is_spectator()).unwrap_or(false) {
                mp.send(ClientMsg::LeaveRoom);
                return;
            }
            if key == K_ESCAPE || self.action_matches_key(key, "pause") {
                // Online gibt es keine Pause: zweimal ESC = Runde verlassen
                if mp.leave_confirm_until.map(|t| Instant::now() < t).unwrap_or(false) {
                    mp.leave_confirm_until = None;
                    mp.send(ClientMsg::LeaveRoom);
                } else {
                    mp.leave_confirm_until = Some(Instant::now() + Duration::from_secs(3));
                }
            }
            return;
        }
        if mp.focus != Field::None {
            let focus = mp.focus;
            let field = match focus { Field::Addr => &mut mp.addr_input, Field::Chat => &mut mp.chat_input, _ => &mut mp.code_input };
            let max = match focus { Field::Addr => 80, Field::Chat => MAX_CHAT_LEN, _ => 4 };
            if key == K_BACKSPACE { field.pop(); return; }
            if key == K_RETURN || key == K_KP_ENTER {
                let a = match focus { Field::Addr => "connect_input", Field::Chat => "chat_send", _ => "join_code" };
                self.mp_action(mp, a);
                return;
            }
            if key == K_ESCAPE || key == K_TAB { mp.focus = Field::None; return; }
            if key == K_v && mods & KMOD_CTRL != 0 {
                if let Some(t) = clipboard_get() {
                    for ch in t.trim().chars() { if field.chars().count() < max && !ch.is_control() { field.push(ch); } }
                }
                return;
            }
            for ch in unicode.chars() {
                if ch.is_control() || field.chars().count() >= max { continue; }
                match focus {
                    Field::Code => if ch.is_ascii_alphanumeric() { field.push(ch.to_ascii_uppercase()); },
                    Field::Chat => field.push(ch),
                    _ => if !ch.is_whitespace() { field.push(ch); },
                }
            }
            return;
        }
        if (self.is_back_key(key) || key == K_ESCAPE) && mp.invite_open {
            self.play_sound(Snd::ClickBack);
            mp.invite_open = false;
            return;
        }
        if self.is_back_key(key) || key == K_ESCAPE {
            self.play_sound(Snd::ClickBack);
            match mp.screen {
                Screen::Browser => self.mp_leave_screen(mp),
                Screen::Connecting => { mp.connect_rx = None; mp.screen = Screen::Browser; }
                Screen::Lobby => self.mp_disconnect(mp, None),
                Screen::Room | Screen::Result => mp.send(ClientMsg::LeaveRoom),
                Screen::Playing => {}
            }
        } else if key == K_RETURN || key == K_KP_ENTER {
            match mp.screen {
                Screen::Room => self.mp_action(mp, "ready"),
                Screen::Result => self.mp_action(mp, "rematch"),
                _ => {}
            }
        } else if key == K_F5 && mp.screen == Screen::Lobby {
            self.mp_action(mp, "refresh");
        } else if key == K_t && matches!(mp.screen, Screen::Lobby | Screen::Room | Screen::Result) {
            // T = Chat (wie in vielen Spielen); das "t" selbst landet nicht im Feld
            mp.focus = Field::Chat;
        }
    }

    fn mp_action(&mut self, mp: &mut Mp, a: &str) {
        let (cmd, arg) = a.split_once(':').unwrap_or((a, ""));
        match cmd {
            "back" => { self.play_sound(Snd::ClickBack); self.mp_leave_screen(mp); }
            "focus_addr" => mp.focus = Field::Addr,
            "focus_code" => mp.focus = Field::Code,
            "focus_chat" => mp.focus = Field::Chat,
            "chat_send" => {
                let t = clean_chat(&mp.chat_input);
                mp.chat_input.clear();
                if !t.is_empty() { mp.send(ClientMsg::Chat { text: t }); }
            }
            "connect_official" => { self.play_sound(Snd::Click); let o = self.mp_official(); self.mp_connect(mp, &o); }
            "connect_input" => { self.play_sound(Snd::Click); let a = mp.addr_input.clone(); self.mp_connect(mp, &a); }
            "connect" => { self.play_sound(Snd::Click); self.mp_connect(mp, arg); }
            "forget" => {
                let mut l = self.mp_recent();
                l.retain(|x| x != arg);
                self.settings.insert("mp_recent_servers".into(), json!(l));
                self.save_section("settings", None);
                self.play_sound(Snd::ClickBack);
            }
            "cancel" => { self.play_sound(Snd::ClickBack); mp.connect_rx = None; mp.screen = Screen::Browser; }
            "disconnect" => { self.play_sound(Snd::ClickBack); self.mp_disconnect(mp, None); }
            "refresh" => { self.play_sound(Snd::Click); mp.last_list = Instant::now(); mp.send(ClientMsg::ListRooms); }
            "quick" => { self.play_sound(Snd::MenuClick); mp.send(ClientMsg::QuickPlay); mp.quick_sent = Some(Instant::now()); }
            "spectate" => { self.play_sound(Snd::MenuClick); mp.send(ClientMsg::Spectate { code: arg.to_string() }); }
            "refresh_servers" => { self.play_sound(Snd::Click); mp.servers_at = None; if mp.servers_busy == 0 { self.mp_refresh_servers(mp); } }
            "invite_open" => {
                self.play_sound(Snd::MenuClick);
                mp.invite_open = true;
                FRIENDS.with(|f| *f.borrow_mut() = None);
                INVITE_SENT.with(|s| s.borrow_mut().clear());
                let Some(me) = self.current_profile_username.clone() else { FRIENDS.with(|f| *f.borrow_mut() = Some(Ok(vec![]))); return; };
                self.fetch_friends_list(&me, Some(Box::new(|_g, r| {
                    let res = match r {
                        Some(v) if v.truthy("ok") => Ok(v.garr("friends").iter().map(|f| (f.gs("username", ""), f.gs("name", ""))).filter(|(u, _)| !u.is_empty()).collect()),
                        _ => Err(String::new()),
                    };
                    FRIENDS.with(|f| *f.borrow_mut() = Some(res));
                })));
            }
            "invite_close" => { self.play_sound(Snd::ClickBack); mp.invite_open = false; }
            "invite_send" => {
                let Some(room) = &mp.room else { return; };
                let who = arg.to_string();
                if INVITE_SENT.with(|s| s.borrow().get(&who).copied()) == Some(1) { return; }
                self.play_sound(Snd::Click);
                let body = format!("{} {}! [jnd-join {} {}]", self.tde("Komm in meinen JumpNDodge-Raum", "Join my JumpNDodge room"), room.code, mp.connecting_addr.trim(), room.code);
                INVITE_SENT.with(|s| { s.borrow_mut().insert(who.clone(), 1); });
                let w2 = who.clone();
                self.send_chat_message_online("dm", &who, &body, Some(Box::new(move |_g, r| {
                    let st = match &r { Some(v) if v.truthy("ok") => 2, Some(v) if v.gs("error", "") == "rate_limited" => 3, _ => 4 };
                    INVITE_SENT.with(|s| { s.borrow_mut().insert(w2, st); });
                })));
            }
            "invite_join" => { self.play_sound(Snd::MenuClick); self.mp_accept_invite(mp); }
            "invite_dismiss" => { self.play_sound(Snd::ClickBack); self.mp_dismiss_invite(); }
            "quick_official" => { self.play_sound(Snd::MenuClick); mp.auto_quick = true; let o = self.mp_official(); self.mp_connect(mp, &o); }
            "host_start" => { self.play_sound(Snd::MenuClick); mp.send(ClientMsg::HostStart); }
            "kick" => { if let Ok(i) = arg.parse::<u8>() { self.play_sound(Snd::ClickBack); mp.send(ClientMsg::Kick { slot: i }); } }
            "private" => { self.play_sound(Snd::Click); mp.send(ClientMsg::SetPrivate(!mp.room_private)); }
            "emote" => { if let Ok(i) = arg.parse::<u8>() { mp.send(ClientMsg::Emote(i)); } }
            "mode" => { self.play_sound(Snd::Click); mp.create_mode = if arg == "coop" { Mode::Coop } else { Mode::Vs }; }
            "max" => {
                self.play_sound(Snd::Click);
                mp.create_max = if arg == "-" { if mp.create_max <= 2 { 8 } else { mp.create_max - 1 } } else if mp.create_max >= 8 { 2 } else { mp.create_max + 1 };
            }
            "diff" => {
                self.play_sound(Snd::Click);
                let n = DIFFICULTIES.len();
                mp.create_diff = if arg == "-" { (mp.create_diff + n - 1) % n } else { (mp.create_diff + 1) % n };
            }
            "create" => {
                self.play_sound(Snd::MenuClick);
                let d = DIFFICULTIES[mp.create_diff].to_string();
                mp.send(ClientMsg::CreateRoom { mode: mp.create_mode, difficulty: d, max_players: mp.create_max as u8 });
            }
            "join" => { self.play_sound(Snd::MenuClick); mp.send(ClientMsg::JoinRoom { code: arg.to_string() }); }
            "join_code" => {
                let c = mp.code_input.trim().to_uppercase();
                if c.len() == 4 { self.play_sound(Snd::MenuClick); mp.focus = Field::None; mp.send(ClientMsg::JoinRoom { code: c }); }
                else { mp.say(&self.tde("Der Raumcode hat 4 Zeichen", "Room codes have 4 characters"), true); }
            }
            "ready" => {
                self.play_sound(Snd::Click);
                let me_ready = mp.room.as_ref().map(|r| r.me_ready()).unwrap_or(false);
                mp.send(ClientMsg::Ready(!me_ready));
            }
            "rematch" => {
                self.play_sound(Snd::MenuClick);
                mp.result = None;
                mp.round = None;
                mp.screen = Screen::Room;
                mp.send(ClientMsg::Ready(true));
            }
            "leave_room" => { self.play_sound(Snd::ClickBack); mp.send(ClientMsg::LeaveRoom); }
            _ => {}
        }
    }

    /// Dev-Konsole / unsichtbarer Test: `run mp connect <adresse>`, `run mp create vs Hölle`,
    /// `run mp join <code>|first`, `run mp ready`, `run mp rematch`, `run mp leave`
    fn mp_dev(&mut self, mp: &mut Mp, line: &str) {
        let p: Vec<&str> = line.split_whitespace().collect();
        match p.first().copied().unwrap_or("") {
            "connect" => { let a = p.get(1).copied().unwrap_or(""); self.mp_connect(mp, a); }
            "invite" => {
                let (a, c) = (p.get(1).copied().unwrap_or(""), p.get(2).copied().unwrap_or(""));
                self.mp_join_invite(mp, a, c);
            }
            // Test: anderen Listen-/offiziellen Server nutzen
            "official" => {
                self.settings.insert("mp_official_server".into(), json!(p.get(1).copied().unwrap_or("")));
                mp.servers_at = None; mp.last_info_query = None;
            }
            // Test: Einladung anzeigen (ohne Online-Konto)
            "fakeinvite" => {
                let (n, a, c) = (p.get(1).copied().unwrap_or("Anna"), p.get(2).copied().unwrap_or(""), p.get(3).copied().unwrap_or("ABCD"));
                INVITES.with(|v| v.borrow_mut().push(Invite { from_user: n.to_lowercase(), from_name: n.into(), addr: a.into(), code: c.to_uppercase(), at: Instant::now() }));
            }
            // Test: Freundesliste fürs Einladen setzen, z.B. fakefriends anna:Anna,ben:Ben
            "fakefriends" => {
                let l = p.get(1).copied().unwrap_or("").split(',').filter_map(|e| e.split_once(':')).map(|(u, n)| (u.to_string(), n.to_string())).collect();
                FRIENDS.with(|f| *f.borrow_mut() = Some(Ok(l)));
            }
            "act" => { let a = p.get(1).copied().unwrap_or(""); self.mp_action(mp, a); }
            "create" => {
                mp.create_mode = if p.get(1).map(|m| m.eq_ignore_ascii_case("coop")).unwrap_or(false) { Mode::Coop } else { Mode::Vs };
                if let Some(i) = p.get(2).and_then(|d| DIFFICULTIES.iter().position(|x| x.eq_ignore_ascii_case(d))) { mp.create_diff = i; }
                if let Some(n) = p.get(3).and_then(|n| n.parse::<usize>().ok()) { mp.create_max = n.clamp(2, 8); }
                self.mp_action(mp, "create");
            }
            "join" => {
                match p.get(1).copied() {
                    Some("first") | None => { mp.auto_join = Some(Instant::now()); mp.send(ClientMsg::ListRooms); }
                    Some(c) => self.mp_action(mp, &format!("join:{}", c)),
                }
            }
            "ready" => { mp.auto_ready = true; mp.send(ClientMsg::Ready(true)); }
            "rematch" => self.mp_action(mp, "rematch"),
            "leave" => mp.send(ClientMsg::LeaveRoom),
            "disconnect" => self.mp_disconnect(mp, None),
            "list" => mp.send(ClientMsg::ListRooms),
            "chat" => { let t = p[1..].join(" "); mp.send(ClientMsg::Chat { text: t }); }
            "quick" => self.mp_action(mp, "quick"),
            "spectate" => {
                match p.get(1).copied() {
                    Some("first") | None => { mp.auto_spectate = true; mp.send(ClientMsg::ListRooms); }
                    Some(c) => { let a = format!("spectate:{}", c); self.mp_action(mp, &a); }
                }
            }
            "start" => self.mp_action(mp, "host_start"),
            "kick" => { let a = format!("kick:{}", p.get(1).copied().unwrap_or("1")); self.mp_action(mp, &a); }
            "private" => { let on = p.get(1).map(|v| *v != "off").unwrap_or(true); mp.send(ClientMsg::SetPrivate(on)); }
            "emote" => { let a = format!("emote:{}", p.get(1).copied().unwrap_or("0")); self.mp_action(mp, &a); }
            "chat_focus" => mp.focus = Field::Chat,
            "chat_type" => { mp.focus = Field::Chat; mp.chat_input = p[1..].join(" "); }
            "state" => println!("[MP] Bildschirm {:?}, Raum {:?}", mp.screen, mp.room.as_ref().map(|r| r.code.clone())),
            _ => println!("[MP] unbekannt: {}", line),
        }
    }

    // ------------------------------------------------------------------
    // Zeichnen
    // ------------------------------------------------------------------
    fn mp_button(&self, bs: &Surface, mp: &mut Mp, r: Rect, text: &str, action: &str, border: (i32, i32, i32), font: Option<Font>) {
        let hov = r.collidepoint(self.mouse_logical_peek());
        self.draw_ui_button(bs, r, text, BtnOpts { font: Some(font.unwrap_or_else(|| self.small_font.clone())), hovered: hov, border_color: Some(border), hover_border_color: Some((200, 220, 255)), hover_border_width: Some(3), ..Default::default() });
        mp.buttons.push((r, action.to_string()));
    }

    /// Mausposition ohne Nebenwirkungen (Tastatur-Navigation bleibt aktiv)
    fn mouse_logical_peek(&self) -> (i32, i32) {
        let (mx, my) = self.display.mouse_pos();
        (((mx as f64 - self.offset_x) / self.scale_x) as i32, ((my as f64 - self.offset_y) / self.scale_y) as i32)
    }

    fn mp_text_field(&self, bs: &Surface, mp: &mut Mp, r: Rect, text: &str, placeholder: &str, focused: bool, action: &str) {
        draw::rect(bs, self.get_panel_color(), r, 0, 10);
        draw::rect(bs, if focused { (120, 180, 255) } else { (110, 110, 140) }, r, 2, 10);
        let sf = &self.small_font;
        let cursor = if focused && (self.get_ticks() / 500) % 2 == 0 { "|" } else { "" };
        let (t, col) = if text.is_empty() && !focused { (placeholder.to_string(), (130, 130, 150)) } else {
            (format!("{}{}", super::online_screens3::_fit_text_tail(text, sf, r.w - 24), cursor), self.get_text_color())
        };
        let s = sf.render(&t, true, col);
        bs.blit(&s, (r.x + 12, r.centery() - s.get_height() / 2));
        mp.buttons.push((r, action.to_string()));
    }

    fn mp_text(&self, bs: &Surface, f: &Font, text: &str, color: (i32, i32, i32), x: i32, y: i32, center: bool) -> i32 {
        let s = f.render(text, true, color);
        let x = if center { x - s.get_width() / 2 } else { x };
        bs.blit(&s, (x, y));
        s.get_width()
    }

    fn mp_draw(&mut self, mp: &mut Mp) {
        mp.buttons.clear();
        if mp.screen != Screen::Playing { self.fill_bg(true); }
        match mp.screen {
            Screen::Browser => { self.mp_query_infos(mp); self.mp_draw_browser(mp); }
            Screen::Connecting => self.mp_draw_connecting(mp),
            Screen::Lobby => self.mp_draw_lobby(mp),
            Screen::Room => self.mp_draw_room(mp),
            Screen::Playing => self.mp_draw_round(mp),
            Screen::Result => self.mp_draw_result(mp),
        }
        if mp.invite_open && matches!(mp.screen, Screen::Room) { self.mp_draw_invite_window(mp); } else { mp.invite_open = false; }
        if mp.screen != Screen::Playing {
            self.mp_poll_invites();
            let bs = self.base_surface.clone();
            if let Some((j, d)) = self.mp_draw_invite_banner(&bs) {
                mp.buttons.push((j, "invite_join".into()));
                mp.buttons.push((d, "invite_dismiss".into()));
            }
        }
        // Meldung oben
        if !mp.message.is_empty() && Instant::now() < mp.message_until {
            let bs = self.base_surface.clone();
            let s = self.small_font.render(&mp.message, true, WHITE);
            let y = if mp.screen == Screen::Playing { 90 } else { self.HEIGHT - 116 };
            let r = Rect::new(self.WIDTH / 2 - s.get_width() / 2 - 14, y, s.get_width() + 28, s.get_height() + 10);
            draw::rect(&bs, if mp.message_err { (150, 45, 60) } else { (45, 90, 150) }, r, 0, 8);
            bs.blit(&s, (r.x + 14, r.y + 5));
        }
        self.flip();
    }

    fn mp_status_line(&self, st: Option<&InfoState>) -> (String, (i32, i32, i32)) {
        match st {
            None | Some(InfoState::Checking) => (self.tde("prüfe …", "checking …"), (150, 150, 170)),
            Some(InfoState::Offline) => (self.tde("offline", "offline"), (200, 90, 100)),
            Some(InfoState::Online(i, ping)) => {
                // Nur das Protokoll entscheidet, ob man zusammen spielen kann - nicht die Spielversion
                if i.proto != PROTO_VERSION {
                    (format!("{} {} ({} {})", self.tde("online · andere Version", "online · other version"), i.game_version, self.tde("du:", "you:"), GAME_VERSION), (210, 170, 70))
                } else {
                    let pw = if i.password { self.tde(" · Passwort", " · password") } else { String::new() };
                    (format!("online · {} {} · {} ms{}", i.players, self.tde("Spieler", "players"), ping, pw), (90, 190, 140))
                }
            }
        }
    }

    fn mp_draw_browser(&mut self, mp: &mut Mp) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        self.draw_title_shadow(&bs, "Multiplayer", 14);
        let sf = self.small_font.clone();
        let mf = self.menu_font.clone();
        let tc = self.get_text_color();
        let sub = self.tde("Online gegen- oder miteinander – bis zu 8 Spieler", "Play online against or with each other – up to 8 players");
        self.mp_text(&bs, &sf, &sub, (150, 150, 180), w / 2, 82, true);
        let cx = w / 2 - 300;
        // Offizieller Server
        let off = self.mp_official();
        let card = Rect::new(cx, 112, 600, 74);
        self.draw_panel(&bs, card, 12, (100, 160, 220), 2);
        self.mp_text(&bs, &mf, &self.tde("Offizieller Server", "Official server"), tc, card.x + 16, card.y + 8, false);
        let (st, sc) = self.mp_status_line(mp.infos.get(&off));
        draw::circle(&bs, sc, (card.x + 22, card.y + 54), 6, 0);
        self.mp_text(&bs, &sf, &st, sc, card.x + 36, card.y + 45, false);
        let online = matches!(mp.infos.get(&off), Some(InfoState::Online(..)));
        let qb = Rect::new(card.right() - 312, card.y + 15, 152, 44);
        self.mp_button(&bs, mp, qb, &self.tde("Schnell spielen", "Quick play"), "quick_official", (139, 108, 240), None);
        let cb = Rect::new(card.right() - 150, card.y + 15, 136, 44);
        self.mp_button(&bs, mp, cb, &self.tde("Verbinden", "Connect"), "connect_official", if online { (90, 190, 140) } else { (120, 120, 140) }, None);
        // Per IP
        self.mp_text(&bs, &sf, &self.tde("Per IP oder Hostname verbinden", "Connect by IP or hostname"), tc, cx, 200, false);
        let field = Rect::new(cx, 224, 440, 44);
        let ai = mp.addr_input.clone();
        let foc = mp.focus == Field::Addr;
        let ph = self.tde("z.B. 192.168.0.20 oder name.ddns.net:47800", "e.g. 192.168.0.20 or name.ddns.net:47800");
        self.mp_text_field(&bs, mp, field, &ai, &ph, foc, "focus_addr");
        self.mp_button(&bs, mp, Rect::new(cx + 452, 224, 148, 44), &self.tde("Verbinden", "Connect"), "connect_input", (100, 160, 220), None);
        self.mp_text(&bs, &self.badge_font.clone(), &self.tde("Standard-Port 47800 (UDP). Strg+V zum Einfügen.", "Default port 47800 (UDP). Ctrl+V to paste."), (130, 130, 150), cx + 4, 272, false);
        // Server-Liste: öffentlich (vom offiziellen Server), im eigenen Netzwerk, zuletzt genutzt
        self.mp_draw_server_list(&bs, mp, Rect::new(cx, 296, 600, (h - 96 - 296).max(0)));
        let host = format!("{} {}", self.tde("Eigenen Server hosten:", "Host your own server:"), HOST_INFO_URL);
        self.mp_text(&bs, &self.badge_font.clone(), &host, (140, 140, 170), w / 2, h - 82, true);
        self.mp_button(&bs, mp, Rect::new(w / 2 - 90, h - 58, 180, 44), &self.get_text("back"), "back", (150, 150, 150), None);
    }

    fn mp_draw_connecting(&mut self, mp: &mut Mp) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        self.draw_title_shadow(&bs, "Multiplayer", 14);
        let dots = ".".repeat(1 + (mp.connect_started.elapsed().as_millis() / 400 % 3) as usize);
        let t = format!("{} {}{}", self.tde("Verbinde mit", "Connecting to"), mp.connecting_addr, dots);
        self.mp_text(&bs, &self.menu_font.clone(), &t, self.get_text_color(), w / 2, h / 2 - 40, true);
        self.mp_button(&bs, mp, Rect::new(w / 2 - 90, h / 2 + 30, 180, 44), &self.tde("Abbrechen", "Cancel"), "cancel", (150, 150, 150), None);
    }

    fn mp_draw_lobby(&mut self, mp: &mut Mp) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let sf = self.small_font.clone();
        let bf = self.badge_font.clone();
        let tc = self.get_text_color();
        self.draw_title_shadow(&bs, &mp.server_name.clone(), 14);
        let ping = mp.ping_ms.map(|p| format!(" · {} ms", p)).unwrap_or_default();
        self.mp_text(&bs, &sf, &format!("{}{}", mp.motd, ping), (150, 150, 180), w / 2, 82, true);
        // Raumliste
        let lx = w / 2 - 385;
        let list = Rect::new(lx, 110, 430, 190);
        self.draw_panel(&bs, list, 12, (110, 110, 150), 2);
        self.mp_text(&bs, &sf, &self.tde("Räume", "Rooms"), tc, list.x + 14, list.y + 10, false);
        let mut y = list.y + 42;
        if mp.rooms.is_empty() {
            self.mp_text(&bs, &sf, &self.tde("Noch keine Räume - erstell einen!", "No rooms yet - create one!"), (140, 140, 160), list.centerx(), y + 30, true);
        }
        for r in mp.rooms.clone() {
            if y + 40 > list.bottom() - 6 { break; }
            let row = Rect::new(list.x + 10, y, list.w - 20, 34);
            let open = r.players < r.max && !r.playing;
            let watch = r.playing;
            let hov = (open || watch) && row.collidepoint(self.mouse_logical_peek());
            draw::rect(&bs, if hov { self.get_button_hover_color() } else { self.get_button_color() }, row, 0, 8);
            let col = if open { tc } else { (130, 130, 140) };
            self.mp_text(&bs, &sf, &r.code, (120, 180, 255), row.x + 10, row.y + 6, false);
            let info = format!("{} · {} · {}", mode_name(self, r.mode), self.get_difficulty_name(&r.difficulty), r.host);
            self.mp_text(&bs, &bf, &info, col, row.x + 80, row.y + 9, false);
            let st = if r.playing { self.tde("läuft · zuschauen", "playing · watch") } else { format!("{}/{}", r.players, r.max) };
            let sw = bf.size(&st).0;
            self.mp_text(&bs, &bf, &st, col, row.right() - 10 - sw, row.y + 9, false);
            if open { mp.buttons.push((row, format!("join:{}", r.code))); }
            if watch { mp.buttons.push((row, format!("spectate:{}", r.code))); }
            y += 38;
        }
        self.mp_draw_chat(&bs, mp, Rect::new(lx, 312, 430, h - 80 - 312));
        // Raum erstellen
        let rx = w / 2 + 60;
        let panel = Rect::new(rx, 110, 325, 246);
        self.draw_panel(&bs, panel, 12, (110, 110, 150), 2);
        self.mp_text(&bs, &sf, &self.tde("Raum erstellen", "Create room"), tc, panel.x + 14, panel.y + 10, false);
        let vs_sel = mp.create_mode == Mode::Vs;
        let sel = |on: bool| if on { (120, 180, 255) } else { (110, 110, 130) };
        self.mp_button(&bs, mp, Rect::new(rx + 14, panel.y + 42, 143, 38), "VS", "mode:vs", sel(vs_sel), None);
        self.mp_button(&bs, mp, Rect::new(rx + 168, panel.y + 42, 143, 38), &mode_name(self, Mode::Coop), "mode:coop", sel(!vs_sel), None);
        self.mp_button(&bs, mp, Rect::new(rx + 14, panel.y + 92, 40, 38), "<", "diff:-", (110, 110, 130), None);
        self.mp_button(&bs, mp, Rect::new(rx + 271, panel.y + 92, 40, 38), ">", "diff:+", (110, 110, 130), None);
        let dn = self.get_difficulty_name(DIFFICULTIES[mp.create_diff]);
        self.mp_text(&bs, &sf, &dn, tc, rx + 162, panel.y + 101, true);
        self.mp_button(&bs, mp, Rect::new(rx + 14, panel.y + 140, 40, 38), "<", "max:-", (110, 110, 130), None);
        self.mp_button(&bs, mp, Rect::new(rx + 271, panel.y + 140, 40, 38), ">", "max:+", (110, 110, 130), None);
        let mt = format!("{} {}", mp.create_max, self.tde("Spieler", "players"));
        self.mp_text(&bs, &sf, &mt, tc, rx + 162, panel.y + 149, true);
        self.mp_button(&bs, mp, Rect::new(rx + 14, panel.y + 190, 297, 42), &self.tde("Erstellen", "Create"), "create", (90, 190, 140), None);
        // Code
        let jp = Rect::new(rx, 366, 325, 104);
        self.draw_panel(&bs, jp, 12, (110, 110, 150), 2);
        self.mp_text(&bs, &sf, &self.tde("Mit Code beitreten", "Join with code"), tc, jp.x + 14, jp.y + 10, false);
        let ci = mp.code_input.clone();
        let foc = mp.focus == Field::Code;
        self.mp_text_field(&bs, mp, Rect::new(rx + 14, jp.y + 44, 160, 44), &ci, "ABCD", foc, "focus_code");
        self.mp_button(&bs, mp, Rect::new(rx + 184, jp.y + 44, 127, 44), &self.tde("Beitreten", "Join"), "join_code", (100, 160, 220), None);
        // unten
        // Schnell spielen = Hauptaktion (Akzentfarbe), dann Aktualisieren und Trennen
        self.draw_ui_button(&bs, Rect::new(w / 2 - 290, h - 62, 210, 44), &self.tde("Schnell spielen", "Quick play"), BtnOpts { font: Some(sf.clone()), hovered: Rect::new(w / 2 - 290, h - 62, 210, 44).collidepoint(self.mouse_logical_peek()), base_color: Some(super::ui_c::ACCENT), hover_color: Some(super::art::lighten(super::ui_c::ACCENT, 0.12)), text_color: Some((255, 255, 255)), ..Default::default() });
        mp.buttons.push((Rect::new(w / 2 - 290, h - 62, 210, 44), "quick".into()));
        self.mp_button(&bs, mp, Rect::new(w / 2 - 70, h - 62, 170, 44), &self.tde("Aktualisieren", "Refresh"), "refresh", (100, 160, 220), None);
        self.mp_button(&bs, mp, Rect::new(w / 2 + 110, h - 62, 170, 44), &self.tde("Trennen", "Disconnect"), "disconnect", (150, 150, 150), None);
    }

    fn mp_draw_room(&mut self, mp: &mut Mp) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let Some(room) = mp.room.clone() else {
            self.mp_text(&bs, &self.menu_font.clone(), &self.tde("Warte auf Raum …", "Waiting for room …"), self.get_text_color(), w / 2, h / 2, true);
            return;
        };
        let sf = self.small_font.clone();
        let tc = self.get_text_color();
        self.draw_title_shadow(&bs, &format!("{} {}", self.tde("Raum", "Room"), room.code), 14);
        let sub = format!("{} · {}", mode_name(self, room.mode), self.get_difficulty_name(&room.difficulty));
        self.mp_text(&bs, &sf, &sub, (150, 150, 180), w / 2, 82, true);
        let n = room.players.len();
        // 2 Plätze: zwei große Karten; mehr: Raster mit bis zu 4 pro Reihe
        let (cols, cw, chh, gap) = if n <= 2 { (2usize, 290, 130, 20) } else { (4usize, 145, 92, 10) };
        let rows = n.div_ceil(cols).max(1);
        let total_w = cols as i32 * cw + (cols as i32 - 1) * gap;
        for i in 0..n {
            let (cx, cy) = ((i % cols) as i32, (i / cols) as i32);
            let r = Rect::new(w / 2 - total_w / 2 + cx * (cw + gap), 118 + cy * (chh + 10), cw, chh);
            let col = pcol(i);
            draw::rect(&bs, self.get_panel_color(), r, 0, 14);
            draw::rect(&bs, if room.players[i].is_some() { col } else { (90, 90, 110) }, r, 2, 14);
            let big = n <= 2;
            let ps = if big { 40 } else { 26 };
            let icon = Rect::new(r.x + 12, r.y + 12, ps, ps);
            super::art::draw_player(&bs, icon, col);
            if room.players[i].is_some() {
                if i == room.you { self.draw_player_cosmetics(&bs, icon, None); }
                else if let Some(l) = mp.room_looks.get(i) { self.mp_draw_look(&bs, icon, l); }
            }
            let tx = r.x + 20 + ps;
            self.mp_text(&bs, &sf, &format!("P{}", i + 1), col, tx, r.y + if big { 18 } else { 12 }, false);
            if room.players[i].is_some() && i == mp.room_host {
                let cs = self.badge_font.render(&self.tde("Chef", "Host"), true, (255, 255, 255));
                let cr = Rect::new(r.right() - cs.get_width() - 18 - if room.you == mp.room_host { 0 } else { 0 }, r.y + 8, cs.get_width() + 10, cs.get_height() + 4);
                draw::rect(&bs, super::ui_c::ACCENT, cr, 0, 5);
                bs.blit(&cs, (cr.x + 5, cr.y + 2));
            }
            if room.you == mp.room_host && i != room.you && room.players[i].is_some() {
                // Raum-Chef: rauswerfen
                let kr = Rect::new(r.right() - 30, r.bottom() - 30, 22, 22);
                self.mp_button(&bs, mp, kr, "x", &format!("kick:{}", i), (150, 100, 110), Some(self.badge_font.clone()));
            }
            if let Some(e) = mp.emotes.iter().find(|e| !e.0 && e.1 == i) {
                self.mp_draw_emote(&bs, e.2, r.centerx(), r.y - 4);
            }
            match &room.players[i] {
                Some(p) => {
                    let me = if i == room.you { format!(" ({})", self.tde("du", "you")) } else { String::new() };
                    let name = format!("{}{}", p.name, me);
                    let ns = sf.render_fit(&name, r.w - 24, tc);
                    bs.blit(&ns, (if big { tx } else { r.x + 12 }, r.y + if big { 42 } else { 44 }));
                    let (t, c) = if p.ready { (self.tde("Bereit", "Ready"), (90, 190, 140)) } else { (self.tde("Wartet …", "Not ready"), (180, 150, 90)) };
                    self.mp_text(&bs, &sf, &t, c, r.x + 12, r.y + if big { 90 } else { 66 }, false);
                    // Ping des Spielers (eigener: direkt gemessen)
                    let pg = if i == room.you { mp.ping_ms.map(|v| v.min(9999) as u16).unwrap_or(0) } else { mp.room_pings.get(i).copied().unwrap_or(0) };
                    if pg > 0 {
                        let bf = self.badge_font.clone();
                        let pc = if pg < 80 { (90, 190, 140) } else if pg < 160 { (215, 195, 90) } else { (220, 95, 105) };
                        let pt = format!("{} ms", pg);
                        let pw = bf.size(&pt).0;
                        self.mp_text(&bs, &bf, &pt, pc, r.right() - 12 - pw, r.y + if big { 94 } else { 28 }, false);
                    }
                }
                None => {
                    let t = if big { self.tde("frei - warte auf Mitspieler", "free - waiting for player") } else { self.tde("frei", "free") };
                    self.mp_text(&bs, &sf, &t, (140, 140, 160), r.x + 12, r.y + if big { 90 } else { 56 }, false);
                }
            }
        }
        let below = 118 + rows as i32 * (chh + 10);
        if room.players.iter().any(|p| p.is_none()) {
            let t = format!("{} {}", self.tde("Gib deinen Freunden den Code", "Give your friends the code"), room.code);
            self.mp_text(&bs, &sf, &t, (150, 150, 180), w / 2, below + 2, true);
        }
        if self.current_profile_username.is_some() && room.players.iter().any(|p| p.is_none()) {
            let ir = Rect::new(16, 20, 190, 36);
            self.mp_button(&bs, mp, ir, &self.tde("Freunde einladen", "Invite friends"), "invite_open", (100, 160, 220), Some(self.small_font.clone()));
        }
        let chat_top = below + 32;
        let chat_r = Rect::new(w / 2 - 300, chat_top, 600, (h - 132 - chat_top).max(0));
        self.mp_draw_chat(&bs, mp, chat_r);
        if let Some(c) = mp.countdown {
            thread_local! { static BIG: Font = Font::new(140); }
            let big = BIG.with(|f| f.clone());
            let veil = Surface::new_alpha(chat_r.w, chat_r.h);
            veil.fill((0, 0, 0, if self.is_dark_mode() { 150 } else { 90 }));
            bs.blit(&veil, (chat_r.x, chat_r.y));
            let bh = big.size("0").1;
            self.mp_text(&bs, &big, &c.to_string(), (120, 180, 255), w / 2, chat_r.centery() - bh / 2, true);
        }
        let me_ready = room.me_ready();
        let rt = if me_ready { self.tde("Nicht mehr bereit", "Not ready") } else { self.tde("Bereit!", "Ready!") };
        if room.you == mp.room_host {
            // Raum-Chef: Bereit + Starten (auch wenn nicht alle bereit sind) + privat
            self.mp_button(&bs, mp, Rect::new(w / 2 - 190, h - 120, 185, 50), &rt, "ready", if me_ready { (180, 150, 90) } else { (90, 190, 140) }, Some(self.small_font.clone()));
            let can = room.present() >= 2;
            let st = Rect::new(w / 2 + 5, h - 120, 185, 50);
            if can {
                self.draw_ui_button(&bs, st, &self.tde("Starten", "Start"), BtnOpts { font: Some(self.small_font.clone()), hovered: st.collidepoint(self.mouse_logical_peek()), base_color: Some(super::ui_c::ACCENT), hover_color: Some(super::art::lighten(super::ui_c::ACCENT, 0.12)), text_color: Some((255, 255, 255)), ..Default::default() });
                mp.buttons.push((st, "host_start".into()));
            } else {
                self.draw_ui_button(&bs, st, &self.tde("Starten (ab 2)", "Start (2+)"), BtnOpts { font: Some(self.small_font.clone()), hovered: false, ..Default::default() });
            }
            let pr = Rect::new(w - 186, 20, 170, 36);
            let pt = if mp.room_private { self.tde("Privat: an", "Private: on") } else { self.tde("Privat: aus", "Private: off") };
            self.mp_button(&bs, mp, pr, &pt, "private", (110, 110, 130), Some(self.small_font.clone()));
        } else {
            self.mp_button(&bs, mp, Rect::new(w / 2 - 190, h - 120, 380, 50), &rt, "ready", if me_ready { (180, 150, 90) } else { (90, 190, 140) }, Some(self.menu_font.clone()));
            if mp.room_private {
                let ps = self.small_font.render(&self.tde("Privat - nur mit Code", "Private - code only"), true, (150, 150, 180));
                bs.blit(&ps, (w - ps.get_width() - 20, 28));
            }
        }
        self.mp_button(&bs, mp, Rect::new(w / 2 - 90, h - 60, 180, 44), &self.tde("Verlassen", "Leave"), "leave_room", (150, 150, 150), None);
    }

    /// Spielfeld (800x600) in Fensterkoordinaten: (Faktor, Versatz x, Versatz y)
    fn mp_field_tf(&self) -> (f64, f64, f64) {
        let (fw, fh) = field();
        // nie vergrößern: gleiche Größe wie im Einzelspieler; nur verkleinern, falls das Fenster kleiner ist
        let s = (self.WIDTH as f64 / fw as f64).min(self.HEIGHT as f64 / fh as f64).min(1.0);
        (s, (self.WIDTH as f64 - fw as f64 * s) / 2.0, (self.HEIGHT as f64 - fh as f64 * s) / 2.0)
    }
    fn mp_field_rect(&self, r: Rect) -> Rect {
        let (s, ox, oy) = self.mp_field_tf();
        let x0 = (ox + r.x as f64 * s).round() as i32;
        let y0 = (oy + r.y as f64 * s).round() as i32;
        Rect::new(x0, y0, (ox + (r.x + r.w) as f64 * s).round() as i32 - x0, (oy + (r.y + r.h) as f64 * s).round() as i32 - y0)
    }
    fn mp_field_point(&self, x: i32, y: i32) -> (i32, i32) {
        let (s, ox, oy) = self.mp_field_tf();
        ((ox + x as f64 * s) as i32, (oy + y as f64 * s) as i32)
    }

    /// Sprechblase einer Schnell-Nachricht, unten mittig bei (cx, bottom)
    fn mp_draw_emote(&self, bs: &Surface, id: u8, cx: i32, bottom: i32) {
        let t = if self.lang() == "Deutsch" { EMOTES.get(id as usize) } else { EMOTES_EN.get(id as usize) }.copied().unwrap_or("?");
        let f = &self.small_font;
        let ts = f.render(t, true, (30, 26, 50));
        let r = Rect::new(cx - ts.get_width() / 2 - 10, bottom - ts.get_height() - 10, ts.get_width() + 20, ts.get_height() + 8);
        draw::rect(bs, (245, 243, 255), r, 0, 8);
        draw::polygon(bs, (245, 243, 255), &[(cx - 6, r.bottom()), (cx + 6, r.bottom()), (cx, r.bottom() + 7)], 0);
        bs.blit(&ts, (r.x + 10, r.y + 4));
    }

    fn mp_draw_player(&self, bs: &Surface, i: usize, p: &DrawPlayer, label: &str, hat: bool) {
        if !p.alive { return; }
        let r = self.mp_field_rect(p.rect);
        if p.invuln == 0 || (self.get_ticks() / 80) % 2 == 0 {
            super::art::draw_player(bs, r, pcol(i));
        }
        let sf = &self.small_font;
        let l = sf.render(label, true, WHITE);
        let ls = sf.render(label, true, (0, 0, 0));
        let (lx, ly) = (r.centerx() - l.get_width() / 2, r.top() - 18 - if hat { 14 } else { 0 });
        bs.blit(&ls, (lx + 1, ly + 1));
        bs.blit(&l, (lx, ly));
    }

    fn mp_draw_round(&mut self, mp: &mut Mp) {
        self.fill_bg(true);
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let Some(r) = mp.round.as_ref() else { return; };
        let alpha = (mp.acc_ms / LOGIC_TICK_MS).clamp(0.0, 1.0);
        let (s, ox, _) = self.mp_field_tf();
        // Feldränder bei breiten Fenstern andeuten
        if ox > 4.0 {
            let fr = self.mp_field_rect(Rect::new(0, 0, field().0, field().1));
            draw::line(&bs, (70, 60, 110), (fr.left() - 1, fr.top()), (fr.left() - 1, fr.bottom()), 1);
            draw::line(&bs, (70, 60, 110), (fr.right(), fr.top()), (fr.right(), fr.bottom()), 1);
        }
        let floor = self.mp_field_rect(Rect::new(0, r.floor_y + 2, field().0, 2));
        let tsec = self.get_ticks() as f64 / 1000.0;
        let dark = self.is_dark_mode();
        super::art::draw_floor(&bs, w, h, floor.top(), r.server_frame * r.speed * s, dark);
        for p in r.powerups_draw() { super::art::draw_oneup(&bs, self.mp_field_rect(p), tsec, &self.badge_font); }
        for (o, kind) in r.obstacles_draw() {
            let fr = self.mp_field_rect(o);
            match kind {
                ObstacleKind::Ground => super::art::draw_hazard(&bs, fr),
                ObstacleKind::Top => { super::art::draw_faller(&bs, fr); super::art::draw_faller_marker(&bs, fr, tsec); }
            }
        }
        let you = r.you;
        mp.fx.draw(&bs);
        let n = r.player_count().max(2);
        let spectator = r.is_spectator();
        for (i, list) in mp.trails.iter() {
            let t = if *i == you { self.mp_my_look().trail } else { mp.round_looks.get(*i).map(|l| l.trail.clone()).unwrap_or_default() };
            if !t.is_empty() { super::gameplay::draw_trail_particles(&bs, &t, list); }
        }
        for (o, d) in r.others_draw() {
            let label = mp.round_names.get(o).filter(|n| !n.is_empty() && spectator).cloned().unwrap_or_else(|| format!("P{}", o + 1));
            let hat = mp.round_looks.get(o).map(|l| !l.headwear.is_empty()).unwrap_or(false);
            self.mp_draw_player(&bs, o, &d, &label, hat);
            if d.alive { if let Some(l) = mp.round_looks.get(o) { let fr = self.mp_field_rect(d.rect); self.mp_draw_look(&bs, fr, l); } }
        }
        let me = r.own_draw(alpha);
        if !spectator {
            let hat = !self.mp_my_look().headwear.is_empty();
            self.mp_draw_player(&bs, you, &me, &self.tde("Du", "You"), hat);
            if me.alive { let fr = self.mp_field_rect(me.rect); self.draw_player_cosmetics(&bs, fr, None); }
        }
        for (_, pl, id, _) in mp.emotes.iter().filter(|e| e.0) {
            let pr = if *pl == you { Some(me.rect) } else { r.other_draw(*pl).map(|d| d.rect) };
            if let Some(pr) = pr {
                let fr = self.mp_field_rect(pr);
                self.mp_draw_emote(&bs, *id, fr.centerx(), fr.top() - 26);
            }
        }
        // HUD: Leben P1 links, P2 rechts
        let mf = self.menu_font.clone();
        let sf = self.small_font.clone();
        let full = self.mp_field_rect(Rect::new(0, 0, field().0, field().1));
        let (hud_l, hud_r) = (full.left().max(0), full.right().min(w));
        // HUD am Rand des Spielfelds: zu zweit P1 links, P2 rechts; mit mehr Spielern je eine Spalte links und rechts
        let (hl, hr) = (hud_l + 12, hud_r - 12);
        let left_n = n.div_ceil(2);
        for i in 0..n {
            let (alive, lives) = if i == you { (me.alive, me.lives) } else { r.server_player(i).map(|p| (p.alive, p.lives)).unwrap_or((true, 3)) };
            let col = if alive { pcol(i) } else { (70, 70, 70) };
            let left = i < left_n;
            let row = if left { i } else { i - left_n } as i32;
            let cnt = 3.max(lives);
            if n <= 2 {
                let lbl = mf.render(&format!("P{}", i + 1), true, col);
                if left { bs.blit(&lbl, (hl, 10)); } else { bs.blit(&lbl, (hr - lbl.get_width(), 10)); }
                for k in 0..cnt {
                    let c = if k < lives { col } else { (55, 55, 55) };
                    let x = if left { hl + k as i32 * 15 } else { hr - (cnt - k) as i32 * 15 };
                    draw::rect(&bs, c, Rect::new(x, 10 + lbl.get_height() + 3, 11, 11), 0, 3);
                }
            } else {
                // kompakt: "P3 ■■■" pro Zeile
                let lbl = sf.render(&format!("P{}", i + 1), true, col);
                let y = 10 + row * 26;
                let boxes = cnt as i32 * 14;
                if left {
                    bs.blit(&lbl, (hl, y));
                    for k in 0..cnt { draw::rect(&bs, if k < lives { col } else { (55, 55, 55) }, Rect::new(hl + lbl.get_width() + 8 + k as i32 * 14, y + 6, 10, 10), 0, 3); }
                } else {
                    let x0 = hr - boxes;
                    bs.blit(&lbl, (x0 - 8 - lbl.get_width(), y));
                    for k in 0..cnt { draw::rect(&bs, if k < lives { col } else { (55, 55, 55) }, Rect::new(x0 + k as i32 * 14, y + 6, 10, 10), 0, 3); }
                }
            }
        }
        let secs = mp.round_started.elapsed().as_secs();
        let ss = mf.render(&format!("{}: {}", self.get_text("score"), r.score), true, self.get_text_color());
        bs.blit(&ss, (w / 2 - ss.get_width() / 2, 10));
        let ping = mp.ping_ms.map(|p| format!("  ·  {} ms", p)).unwrap_or_default();
        let watch = if spectator { format!("  ·  {}", self.tde("Zuschauer", "Spectating")) } else { String::new() };
        let info = format!("Online-{}  ·  {}s  ·  {}{}{}", mode_name(self, r.mode), secs, self.get_difficulty_name(&r.difficulty), ping, watch);
        let is = sf.render(&info, true, (140, 90, 210));
        bs.blit(&is, (w / 2 - is.get_width() / 2, 12 + ss.get_height()));
        if !me.alive {
            let t = if r.mode == Mode::Coop { self.tde("Ausgeschieden - dein Partner kann dich mit einem 1-UP wiederbeleben", "Eliminated - your partner can revive you with a 1-UP") } else { self.tde("Ausgeschieden", "Eliminated") };
            self.mp_text(&bs, &sf, &t, (220, 90, 100), w / 2, h / 2 - 40, true);
        }
        let hint = if spectator {
            (self.tde("Du schaust zu - nach der Runde spielst du mit, wenn ein Platz frei ist  ·  ESC: zurück", "You're watching - you join after the round if there's room  ·  ESC: back"), (150, 150, 180))
        } else if mp.leave_confirm_until.map(|t| Instant::now() < t).unwrap_or(false) {
            (self.tde("Nochmal ESC = Runde verlassen (online gibt es keine Pause)", "Press ESC again to leave (no pause online)"), (230, 120, 130))
        } else { (self.tde("ESC: Runde verlassen  ·  1-4: Schnell-Nachricht", "ESC: leave round  ·  1-4: quick message"), (90, 90, 110)) };
        self.mp_text(&bs, &sf, &hint.0, hint.1, w / 2, h - 20, true);
        self.mp_draw_chat_toasts(&bs, mp);
        self.draw_overlays_ingame();
    }

    /// Server-Chat-Kasten: Verlauf seit dem Verbinden (nur im Speicher), Eingabe unten
    fn mp_draw_chat(&self, bs: &Surface, mp: &mut Mp, r: Rect) {
        if r.h < 90 { return; }
        let dark = self.is_dark_mode();
        let sf = self.small_font.clone();
        let bf = self.badge_font.clone();
        self.draw_panel(bs, r, 12, (110, 110, 150), 2);
        let tc = self.get_text_color();
        let dim = if dark { (140, 136, 170) } else { (110, 106, 140) };
        self.mp_text(bs, &bf, "Server-Chat", dim, r.x + 14, r.y + 9, false);
        let note = self.tde("Verlauf: letzte 24 Stunden", "History: last 24 hours");
        let nw = bf.size(&note).0;
        self.mp_text(bs, &bf, &note, dim, r.right() - 14 - nw, r.y + 9, false);
        // Eingabe
        let input = Rect::new(r.x + 10, r.bottom() - 46, r.w - 20, 36);
        let ci = mp.chat_input.clone();
        let foc = mp.focus == Field::Chat;
        let ph = self.tde("T oder hier klicken zum Schreiben", "Press T or click to chat");
        self.mp_text_field(bs, mp, input, &ci, &ph, foc, "focus_chat");
        // Zeilen von unten nach oben
        let lh = bf.size("Ag").1 + 3;
        let top = r.y + 30;
        let mut y = input.y - 6;
        let max_w = r.w - 28;
        let own_col = super::ui_c::readable(super::ui_c::ACCENT, dark);
        let other_col = super::ui_c::readable((60, 190, 210), dark);
        'outer: for line in mp.chat.iter().rev() {
            let (prefix, pcol) = if line.system { (String::new(), dim) } else { (format!("{}: ", line.from), if line.own { own_col } else { other_col }) };
            let pw = if prefix.is_empty() { 0 } else { bf.size(&prefix).0 };
            let rows = wrap_text(&bf, &line.text, max_w, pw);
            let block_h = rows.len() as i32 * lh;
            if y - block_h < top { break 'outer; }
            y -= block_h;
            let tcol = if line.system { dim } else { tc };
            for (k, row) in rows.iter().enumerate() {
                let ry = y + k as i32 * lh;
                if k == 0 && pw > 0 {
                    self.mp_text(bs, &bf, &prefix, pcol, r.x + 14, ry, false);
                    self.mp_text(bs, &bf, row, tcol, r.x + 14 + pw, ry, false);
                } else {
                    self.mp_text(bs, &bf, row, tcol, r.x + 14, ry, false);
                }
            }
        }
        let _ = sf;
    }

    /// Während der Runde: neue Nachrichten 6 s unten links, dann ausblenden (Tippen geht erst danach)
    fn mp_draw_chat_toasts(&self, bs: &Surface, mp: &Mp) {
        let bf = self.badge_font.clone();
        let now = Instant::now();
        let recent: Vec<&ChatLine> = mp.chat.iter().rev().filter(|l| !l.system && now.duration_since(l.at) < Duration::from_secs(6)).take(3).collect();
        let lh = bf.size("Ag").1 + 4;
        let mut y = self.HEIGHT - 46 - lh;
        for l in recent {
            let age = now.duration_since(l.at).as_secs_f64();
            let a = if age > 5.0 { ((6.0 - age) * 255.0) as u8 } else { 255 };
            let txt: String = format!("{}: {}", l.from, l.text).chars().take(70).collect();
            let s = bf.render(&txt, true, if l.own { (200, 190, 255) } else { (235, 235, 245) });
            let bg = Surface::new_alpha(s.get_width() + 14, s.get_height() + 4);
            bg.fill((10, 8, 20, (a as f64 * 0.55) as u8));
            s.set_alpha(Some(a as i32));
            bs.blit(&bg, (10, y));
            bs.blit(&s, (17, y + 2));
            y -= lh;
        }
    }

    fn mp_draw_result(&mut self, mp: &mut Mp) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let Some((winner, score, reason, frames)) = mp.result.clone() else { return; };
        let you = mp.round.as_ref().map(|r| r.you).or(mp.room.as_ref().map(|r| r.you)).unwrap_or(0);
        let mode = mp.round.as_ref().map(|r| r.mode).or(mp.room.as_ref().map(|r| r.mode)).unwrap_or(Mode::Vs);
        let n_round = mp.round_names.len().max(2);
        let spectator = mp.round.as_ref().map(|r| r.is_spectator()).unwrap_or(false);
        let (title, col) = match (mode, winner) {
            (Mode::Coop, _) => (self.tde("Runde vorbei", "Round over"), (120, 180, 255)),
            (_, Some(Winner::Draw)) => (self.tde("Unentschieden!", "Draw!"), (200, 200, 220)),
            (_, Some(w)) if w.index() == Some(you) => (self.tde("Gewonnen!", "You win!"), (90, 200, 140)),
            (_, Some(w)) if n_round > 2 || spectator => {
                let i = w.index().unwrap_or(0);
                let name = mp.round_names.get(i).cloned().unwrap_or_else(|| format!("P{}", i + 1));
                (format!("{} {}", name, self.tde("gewinnt!", "wins!")), pcol(i))
            }
            (_, Some(_)) => (self.tde("Verloren", "You lose"), (220, 90, 100)),
            (_, None) => (self.tde("Runde vorbei", "Round over"), (200, 200, 220)),
        };
        let big = self.font.clone();
        self.mp_text(&bs, &big, &title, col, w / 2, 90, true);
        let sf = self.small_font.clone();
        let mf = self.menu_font.clone();
        let tc = self.get_text_color();
        self.mp_text(&bs, &mf, &format!("{}: {}", self.get_text("score"), score), tc, w / 2, 180, true);
        self.mp_text(&bs, &sf, &format!("{} s", frames / TICK_HZ), (150, 150, 180), w / 2, 222, true);
        if !reason.is_empty() { self.mp_text(&bs, &sf, &reason, (200, 170, 110), w / 2, 252, true); }
        if let Some(room) = &mp.room {
            let others: Vec<&LobbyPlayer> = room.players.iter().enumerate().filter(|(i, _)| *i != room.you).filter_map(|(_, p)| p.as_ref()).collect();
            let t = if others.is_empty() {
                self.tde("Alle anderen haben den Raum verlassen", "Everyone else left the room")
            } else if others.len() == 1 {
                let p = others[0];
                if p.ready { format!("{} {}", p.name, self.tde("will nochmal!", "wants a rematch!")) } else { format!("{}: {}", self.tde("Mitspieler", "Opponent"), p.name) }
            } else {
                let r = others.iter().filter(|p| p.ready).count();
                format!("{} {} {} {}", r, self.tde("von", "of"), others.len(), self.tde("wollen nochmal", "want a rematch"))
            };
            self.mp_text(&bs, &sf, &t, (150, 150, 180), w / 2, 282, true);
        }
        self.mp_draw_chat(&bs, mp, Rect::new(w / 2 - 300, 316, 600, h - 142 - 316));
        let can_rematch = mp.room.as_ref().map(|r| r.present() >= 2).unwrap_or(false);
        if can_rematch {
            self.mp_button(&bs, mp, Rect::new(w / 2 - 190, h - 130, 380, 54), &self.tde("Nochmal!", "Rematch!"), "rematch", (90, 190, 140), Some(mf.clone()));
        }
        self.mp_button(&bs, mp, Rect::new(w / 2 - 110, h - 64, 220, 44), &self.tde("Raum verlassen", "Leave room"), "leave_room", (150, 150, 150), None);
    }
}

const DIFFICULTIES: [&str; 8] = jnd_core::net::proto::DIFFICULTIES;

// ----------------------------------------------------------------------
// Server-Browser und Freunde einladen
// ----------------------------------------------------------------------
impl G {
    /// Öffentliche Liste beim offiziellen Server holen und im eigenen Netzwerk suchen (im Hintergrund)
    fn mp_refresh_servers(&mut self, mp: &mut Mp) {
        mp.servers_at = Some(Instant::now());
        mp.servers_busy = 2;
        let reg = self.mp_official();
        let tx = mp.list_tx.clone();
        std::thread::spawn(move || { let _ = tx.send(ListUpd::Public(fetch_server_list(&reg, Duration::from_secs(3)).unwrap_or_default())); });
        let tx = mp.list_tx.clone();
        std::thread::spawn(move || {
            let l = lan_scan(Duration::from_millis(1200)).into_iter().map(|(a, i, d)| (a, i, d.as_millis() as u32)).collect();
            let _ = tx.send(ListUpd::Lan(l));
        });
    }

    fn mp_draw_server_list(&mut self, bs: &Surface, mp: &mut Mp, area: Rect) {
        let sf = self.small_font.clone();
        let bf = self.badge_font.clone();
        let tc = self.get_text_color();
        self.mp_text(bs, &sf, &self.tde("Server", "Servers"), tc, area.x, area.y + 4, false);
        let busy = mp.servers_busy > 0;
        let rb = Rect::new(area.right() - 150, area.y, 150, 30);
        let rt = if busy { self.tde("Suche …", "Searching …") } else { self.tde("Aktualisieren", "Refresh") };
        self.mp_button(bs, mp, rb, &rt, "refresh_servers", (110, 110, 130), Some(bf.clone()));
        // Einträge: (Art, Adresse, Name aus der Liste, vergessen möglich)
        let off = self.mp_official();
        let mut rows: Vec<(u8, String, String, bool)> = vec![];
        let seen = |rows: &Vec<(u8, String, String, bool)>, a: &str| a.eq_ignore_ascii_case(&off) || rows.iter().any(|r| r.1.eq_ignore_ascii_case(a));
        for s in &mp.public_servers { if !seen(&rows, &s.address) { rows.push((0, s.address.clone(), s.info.name.clone(), false)); } }
        for (a, i, _) in &mp.lan_servers { if !seen(&rows, a) { rows.push((1, a.clone(), i.name.clone(), false)); } }
        for a in self.mp_recent() { if !seen(&rows, &a) { rows.push((2, a.clone(), String::new(), true)); } }
        // schnellste erreichbare Server zuerst, dann die noch geprüften, offline ganz unten
        rows.sort_by_key(|r| match mp.infos.get(&r.1) {
            Some(InfoState::Online(i, p)) if i.proto == PROTO_VERSION => (0u8, *p),
            Some(InfoState::Online(..)) => (2, 0),
            Some(InfoState::Checking) | None => (1, 0),
            Some(InfoState::Offline) => (3, 0),
        });
        let top = area.y + 38;
        if rows.is_empty() {
            let t = if busy { self.tde("Suche Server …", "Looking for servers …") } else { self.tde("Keine weiteren Server gefunden", "No other servers found") };
            self.mp_text(bs, &sf, &t, (140, 140, 160), area.centerx(), top + 10, true);
            let hint = self.tde("Eigene Server erscheinen hier mit \"listed\": true in der server.json", "Your own servers show up here with \"listed\": true in server.json");
            self.mp_text(bs, &bf, &hint, (120, 120, 145), area.centerx(), top + 40, true);
            return;
        }
        let row_h = 44;
        let fit = ((area.bottom() - top) / row_h).max(1) as usize;
        let max_scroll = rows.len().saturating_sub(fit) as i32;
        mp.browser_scroll = mp.browser_scroll.min(max_scroll);
        let mouse = self.mouse_logical_peek();
        let mut y = top;
        for (kind, addr, name, forget) in rows.iter().skip(mp.browser_scroll as usize).take(fit) {
            let w = if *forget { area.w - 44 } else { area.w };
            let r = Rect::new(area.x, y, w, row_h - 6);
            let info = mp.infos.get(addr);
            let usable = !matches!(info, Some(InfoState::Online(i, _)) if i.proto != PROTO_VERSION);
            let hov = usable && r.collidepoint(mouse);
            draw::rect(bs, if hov { self.get_button_hover_color() } else { self.get_button_color() }, r, 0, 8);
            let (tag, tcol) = match kind {
                0 => (self.tde("Öffentlich", "Public"), (100, 160, 220)),
                1 => ("LAN".to_string(), (139, 108, 240)),
                _ => (self.tde("Zuletzt", "Recent"), (130, 130, 150)),
            };
            let tw = bf.size(&tag).0 + 14;
            let tr = Rect::new(r.x + 8, r.centery() - 10, tw, 20);
            draw::rect(bs, tcol, tr, 0, 6);
            self.mp_text(bs, &bf, &tag, (255, 255, 255), tr.centerx(), tr.y + 2, true);
            let shown = match info { Some(InfoState::Online(i, _)) if !i.name.is_empty() => i.name.clone(), _ if !name.is_empty() => name.clone(), _ => addr.clone() };
            let nx = tr.right() + 10;
            self.mp_text(bs, &sf, &shown, tc, nx, r.y + 2, false);
            if shown != *addr { self.mp_text(bs, &bf, addr, (130, 130, 150), nx, r.y + 22, false); }
            let (st, sc) = self.mp_status_line(info);
            let stw = bf.size(&st).0.min(280);
            let sts = bf.render(&st, true, sc);
            bs.blit_area(&sts, (r.right() - 12 - stw, r.centery() - sts.get_height() / 2), Some(Rect::new(sts.get_width() - stw, 0, stw, sts.get_height())));
            if usable { mp.buttons.push((r, format!("connect:{}", addr))); }
            if *forget { self.mp_button(bs, mp, Rect::new(area.right() - 38, y, 38, row_h - 6), "x", &format!("forget:{}", addr), (150, 100, 110), None); }
            y += row_h;
        }
        if max_scroll > 0 {
            let t = format!("{}–{} / {} · {}", mp.browser_scroll + 1, (mp.browser_scroll as usize + fit).min(rows.len()), rows.len(), self.tde("Mausrad zum Blättern", "scroll for more"));
            self.mp_text(bs, &bf, &t, (120, 120, 145), area.centerx(), area.bottom() + 2, true);
        }
    }

    /// Einladungen in Direktnachrichten suchen (alle 10 s, nur mit Online-Profil)
    pub fn mp_poll_invites(&mut self) {
        if self.current_profile_username.is_none() { return; }
        let now = self.get_ticks();
        let last = INVITE_POLL.with(|c| c.get());
        if last != 0 && now.saturating_sub(last) < 10_000 { return; }
        INVITE_POLL.with(|c| c.set(now.max(1)));
        // Alte Einladungen (älter als 2 Minuten) verschwinden von selbst
        INVITES.with(|v| v.borrow_mut().retain(|i| i.at.elapsed() < Duration::from_secs(120)));
        self.fetch_conversations_list(Some(Box::new(|_g, r| {
            let Some(r) = r else { return; };
            if !r.truthy("ok") { return; }
            for c in r.garr("conversations") {
                if c.gs("conversation_type", "") != "dm" || !c.truthy("unread") { continue; }
                let Some((addr, code)) = parse_invite(&c.gs("last_body", "")) else { continue; };
                let from_user = c.gs("target", "");
                let key = format!("{}|{}|{}", from_user, addr, code);
                let new = INVITE_SEEN.with(|s| { let mut s = s.borrow_mut(); if s.contains(&key) { false } else { s.push(key); true } });
                if !new { continue; }
                let from_name = { let n = c.gs("label", ""); if n.is_empty() { from_user.clone() } else { n } };
                INVITES.with(|v| v.borrow_mut().push(Invite { from_user, from_name, addr, code, at: Instant::now() }));
            }
        })));
    }

    /// Banner für die neueste Einladung oben in der Mitte; liefert (Beitreten, Wegklicken)
    pub fn mp_draw_invite_banner(&mut self, bs: &Surface) -> Option<(Rect, Rect)> {
        let inv = INVITES.with(|v| v.borrow().last().cloned());
        let Some(inv) = inv else { INVITE_RECTS.with(|c| c.set(None)); return None; };
        let w = self.WIDTH;
        let bw = 600.min(w - 32);
        let r = Rect::new(w / 2 - bw / 2, 8, bw, 52);
        let veil = Surface::new_alpha(r.w, r.h);
        let dark = self.is_dark_mode();
        veil.fill(if dark { (34, 32, 48, 245) } else { (250, 250, 255, 245) });
        bs.blit(&veil, (r.x, r.y));
        draw::rect(bs, super::ui_c::ACCENT, r, 2, 10);
        let sf = self.small_font.clone();
        let bf = self.badge_font.clone();
        let t = format!("{} {}", inv.from_name, self.tde("lädt dich in einen Raum ein", "invites you to a room"));
        self.mp_text(bs, &sf, &t, self.get_text_color(), r.x + 14, r.y + 6, false);
        self.mp_text(bs, &bf, &format!("{} {} · {}", self.tde("Raum", "Room"), inv.code, inv.addr), (130, 130, 160), r.x + 14, r.y + 30, false);
        let jr = Rect::new(r.right() - 182, r.y + 9, 124, 34);
        let dr = Rect::new(r.right() - 50, r.y + 9, 38, 34);
        let mouse = self.mouse_logical_peek();
        self.draw_ui_button(bs, jr, &self.tde("Beitreten", "Join"), BtnOpts { font: Some(bf.clone()), hovered: jr.collidepoint(mouse), base_color: Some(super::ui_c::ACCENT), hover_color: Some(super::art::lighten(super::ui_c::ACCENT, 0.12)), text_color: Some((255, 255, 255)), ..Default::default() });
        self.draw_ui_button(bs, dr, "x", BtnOpts { font: Some(bf), hovered: dr.collidepoint(mouse), ..Default::default() });
        INVITE_RECTS.with(|c| c.set(Some((jr, dr))));
        Some((jr, dr))
    }

    /// Klick im Hauptmenü auf das Einladungs-Banner; true = verbraucht
    pub fn mp_invite_menu_click(&mut self, pos: (i32, i32)) -> bool {
        let Some((jr, dr)) = INVITE_RECTS.with(|c| c.get()) else { return false; };
        if jr.collidepoint(pos) {
            let Some(inv) = INVITES.with(|v| v.borrow_mut().pop()) else { return true; };
            self.mark_chat_read_online("dm", &inv.from_user, None);
            self.open_multiplayer();
            self.in_menu = false;
            self._mp_dev_cmds.push(format!("invite {} {}", inv.addr, inv.code));
            INVITE_RECTS.with(|c| c.set(None));
            return true;
        }
        if dr.collidepoint(pos) { self.play_sound(Snd::ClickBack); self.mp_dismiss_invite(); return true; }
        false
    }

    fn mp_dismiss_invite(&mut self) {
        if let Some(inv) = INVITES.with(|v| v.borrow_mut().pop()) { self.mark_chat_read_online("dm", &inv.from_user, None); }
        INVITE_RECTS.with(|c| c.set(None));
    }

    fn mp_accept_invite(&mut self, mp: &mut Mp) {
        let Some(inv) = INVITES.with(|v| v.borrow_mut().pop()) else { return; };
        self.mark_chat_read_online("dm", &inv.from_user, None);
        self.mp_join_invite(mp, &inv.addr, &inv.code);
    }

    /// Zum Server der Einladung (falls nötig) und in den Raum
    fn mp_join_invite(&mut self, mp: &mut Mp, addr: &str, code: &str) {
        if addr.is_empty() || code.is_empty() { return; }
        let same = mp.client.is_some() && mp.connecting_addr.trim().eq_ignore_ascii_case(addr.trim());
        if same {
            if mp.room.is_some() { mp.send(ClientMsg::LeaveRoom); }
            mp.send(ClientMsg::JoinRoom { code: code.to_uppercase() });
        } else {
            mp.join_code = Some(code.to_uppercase());
            self.mp_connect(mp, addr);
        }
    }

    /// Fenster "Freunde einladen" über dem Raum
    fn mp_draw_invite_window(&mut self, mp: &mut Mp) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        // Alles darunter ist gesperrt, solange das Fenster offen ist
        mp.buttons.clear();
        mp.buttons.push((Rect::new(0, 0, w, h), "noop".into()));
        let veil = Surface::new_alpha(w, h);
        veil.fill((0, 0, 0, 140));
        bs.blit(&veil, (0, 0));
        let pw = 460.min(w - 32);
        let ph = 420.min(h - 40);
        let p = Rect::new(w / 2 - pw / 2, h / 2 - ph / 2, pw, ph);
        self.draw_panel(&bs, p, 14, super::ui_c::ACCENT, 2);
        let sf = self.small_font.clone();
        let bf = self.badge_font.clone();
        let mf = self.menu_font.clone();
        let tc = self.get_text_color();
        self.mp_text(&bs, &mf, &self.tde("Freunde einladen", "Invite friends"), tc, p.centerx(), p.y + 14, true);
        let code = mp.room.as_ref().map(|r| r.code.clone()).unwrap_or_default();
        self.mp_text(&bs, &bf, &format!("{} {} · {}", self.tde("Raum", "Room"), code, self.tde("sie bekommen eine Nachricht", "they get a message")), (130, 130, 160), p.centerx(), p.y + 52, true);
        let list = FRIENDS.with(|f| f.borrow().clone());
        let mut y = p.y + 82;
        match list {
            None => { self.mp_text(&bs, &sf, &self.tde("Lade Freunde …", "Loading friends …"), (140, 140, 160), p.centerx(), y + 20, true); }
            Some(Err(_)) => { self.mp_text(&bs, &sf, &self.tde("Freunde konnten nicht geladen werden", "Could not load friends"), (200, 110, 120), p.centerx(), y + 20, true); }
            Some(Ok(l)) if l.is_empty() => {
                self.mp_text(&bs, &sf, &self.tde("Noch keine Freunde", "No friends yet"), (140, 140, 160), p.centerx(), y + 20, true);
                self.mp_text(&bs, &bf, &self.tde("Freunde fügst du unter Online > Freunde hinzu", "Add friends under Online > Friends"), (120, 120, 145), p.centerx(), y + 50, true);
            }
            Some(Ok(l)) => {
                let fit = ((p.bottom() - 70 - y) / 44).max(1) as usize;
                for (user, name) in l.iter().take(fit) {
                    let r = Rect::new(p.x + 16, y, p.w - 32, 38);
                    draw::rect(&bs, self.get_button_color(), r, 0, 8);
                    let shown = if name.is_empty() { user.clone() } else { name.clone() };
                    self.mp_text(&bs, &sf, &shown, tc, r.x + 12, r.y + 8, false);
                    let st = INVITE_SENT.with(|s| s.borrow().get(user).copied()).unwrap_or(0);
                    let br = Rect::new(r.right() - 132, r.y + 4, 126, 30);
                    match st {
                        0 | 3 | 4 => {
                            let t = match st { 3 => self.tde("Zu schnell – nochmal", "Too fast – retry"), 4 => self.tde("Fehler – nochmal", "Failed – retry"), _ => self.tde("Einladen", "Invite") };
                            self.mp_button(&bs, mp, br, &t, &format!("invite_send:{}", user), if st == 0 { (100, 160, 220) } else { (180, 100, 110) }, Some(bf.clone()));
                        }
                        1 => { self.mp_text(&bs, &bf, &self.tde("sende …", "sending …"), (140, 140, 160), br.centerx(), br.y + 7, true); }
                        _ => { self.mp_text(&bs, &bf, &self.tde("eingeladen ✓", "invited ✓"), (90, 190, 140), br.centerx(), br.y + 7, true); }
                    }
                    y += 44;
                }
            }
        }
        self.mp_button(&bs, mp, Rect::new(p.centerx() - 80, p.bottom() - 56, 160, 42), &self.tde("Schließen", "Close"), "invite_close", (150, 150, 150), None);
    }
}
