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
use jnd_core::net::client::{query_info, Client, ConnectOptions};
use jnd_core::net::netround::{DrawPlayer, NetRound};
use jnd_core::net::proto::*;
use jnd_core::sim2p::{Input, ObstacleKind, SimEvent, Winner};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// Offizieller Server (DNS-Eintrag zeigt auf den Heimserver; nicht immer online)
pub const OFFICIAL_SERVER: &str = "play.jumpndodge.download";
/// Wo es den Server zum Selbsthosten gibt
pub const HOST_INFO_URL: &str = "mert.win/jump/server";

/// Farben der Spieler online (kein Orange neben dem grünen 1-UP)
pub const MP_COLORS: [(i32, i32, i32); 2] = [(70, 130, 255), (225, 80, 170)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Screen { Browser, Connecting, Lobby, Room, Playing, Result }

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field { None, Addr, Code }

enum InfoState { Checking, Online(ServerInfo, u32), Offline }

#[derive(Clone)]
struct RoomView { code: String, mode: Mode, difficulty: String, you: usize, players: [Option<LobbyPlayer>; 2] }

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
    room: Option<RoomView>,
    countdown: Option<u8>,
    round: Option<NetRound>,
    round_started: Instant,
    acc_ms: f64,
    jump_latch: bool,
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
}

impl Mp {
    fn new() -> Mp {
        let (info_tx, info_rx) = channel();
        let now = Instant::now();
        Mp {
            screen: Screen::Browser, client: None, connect_rx: None, connecting_addr: String::new(), connect_started: now,
            addr_input: String::new(), code_input: String::new(), focus: Field::None, infos: HashMap::new(), info_tx, info_rx,
            last_info_query: None, message: String::new(), message_err: false, message_until: now, rooms: vec![], last_list: now,
            create_mode: Mode::Vs, create_diff: 1, room: None, countdown: None, round: None, round_started: now, acc_ms: 0.0,
            jump_latch: false, leave_confirm_until: None, result: None, server_name: String::new(), motd: String::new(),
            ping_ms: None, last_ping_sent: now, buttons: vec![], auto_join: None, auto_ready: false, fx: super::art::Fx::new(),
        }
    }
    fn say(&mut self, text: &str, err: bool) {
        self.message = text.into();
        self.message_err = err;
        self.message_until = Instant::now() + Duration::from_secs(if err { 6 } else { 3 });
    }
    fn send(&mut self, m: ClientMsg) { if let Some(c) = self.client.as_mut() { c.send(&m); } }
}

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
        if mp.last_info_query.map(|t| t.elapsed() < Duration::from_secs(5)).unwrap_or(false) { return; }
        mp.last_info_query = Some(Instant::now());
        let mut addrs = vec![self.mp_official()];
        addrs.extend(self.mp_recent());
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
    }

    /// Netzwerk: Verbindungsaufbau, Nachrichten, Verbindungsverlust
    fn mp_network(&mut self, mp: &mut Mp) {
        if let Some(rx) = &mp.connect_rx {
            match rx.try_recv() {
                Ok(Ok(c)) => {
                    mp.connect_rx = None;
                    mp.server_name = c.server_name.clone();
                    mp.motd = c.motd.clone();
                    mp.client = Some(c);
                    mp.screen = Screen::Lobby;
                    mp.rooms.clear();
                    mp.last_list = Instant::now() - Duration::from_secs(10);
                    let a = mp.connecting_addr.clone();
                    self.settings.insert("mp_last_addr".into(), json!(a));
                    self.mp_remember(&a);
                    println!("[MP] verbunden mit {} ({})", mp.server_name, a);
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
        if mp.last_ping_sent.elapsed() >= Duration::from_secs(2) {
            mp.last_ping_sent = Instant::now();
            mp.send(ClientMsg::Ping { t: now_ms() });
        }
        if mp.screen == Screen::Lobby && mp.last_list.elapsed() >= Duration::from_secs(3) {
            mp.last_list = Instant::now();
            mp.send(ClientMsg::ListRooms);
        }
        for m in msgs { self.mp_message(mp, m); if mp.client.is_none() { return; } }
    }

    fn mp_message(&mut self, mp: &mut Mp, m: ServerMsg) {
        match m {
            ServerMsg::RoomList(l) => {
                mp.rooms = l;
                if mp.auto_join.map(|t| t.elapsed() < Duration::from_secs(30)).unwrap_or(false) && mp.room.is_none() {
                    if let Some(r) = mp.rooms.iter().find(|r| r.players == 1 && !r.playing) {
                        let c = r.code.clone();
                        mp.auto_join = None;
                        mp.send(ClientMsg::JoinRoom { code: c });
                    } else {
                        mp.last_list = Instant::now() - Duration::from_millis(2500);
                    }
                }
            }
            ServerMsg::RoomState { code, mode, difficulty, you, players } => {
                if mp.room.as_ref().map(|r| r.code != code).unwrap_or(true) { println!("[MP] Raum {}", code); }
                mp.room = Some(RoomView { code, mode, difficulty, you: you as usize, players });
                if matches!(mp.screen, Screen::Lobby | Screen::Connecting) { mp.screen = Screen::Room; }
                if mp.auto_ready {
                    if let Some(r) = &mp.room {
                        if r.players.iter().all(|p| p.is_some()) && !r.players[r.you].as_ref().map(|p| p.ready).unwrap_or(false) {
                            mp.auto_ready = false;
                            mp.send(ClientMsg::Ready(true));
                        }
                    }
                }
                let both_ready = mp.room.as_ref().map(|r| r.players.iter().all(|p| p.as_ref().map(|p| p.ready).unwrap_or(false))).unwrap_or(false);
                if !both_ready { mp.countdown = None; }
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
            ServerMsg::Start { mode, difficulty, width, height, .. } => {
                let you = mp.room.as_ref().map(|r| r.you).unwrap_or(0);
                mp.round = Some(NetRound::new(mode, &difficulty, you, width, height));
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
                            let at = |i: usize| -> Option<Rect> { if i == r.you { Some(r.own_draw(1.0).rect) } else { r.other_draw().map(|p| p.rect) } };
                            match e {
                                SimEvent::Hit(i) | SimEvent::Died(i) => {
                                    if let Some(pr) = at(i) { let (x, y) = self.mp_field_point(pr.centerx(), pr.centery()); mp.fx.burst(x as f64, y as f64, super::art::HAZARD, 20); }
                                    if i == you { self.shake(6.0); }
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
                self.mp_record_stats(mode, winner, you, frames);
                mp.result = Some((winner, score, reason, frames));
                mp.screen = Screen::Result;
                if self.mixer_playing_ingame { self.stop_music(); self.mixer_playing_ingame = false; }
                self.play_sound_v(Snd::GameOver, "gameover_volume");
            }
            ServerMsg::Error(e) => {
                if e.contains("beendet") { self.mp_disconnect(mp, Some(&e)); } else { mp.say(&e, true); }
            }
            ServerMsg::Pong { t } => { mp.ping_ms = Some(now_ms().saturating_sub(t) as u32); }
            ServerMsg::Reject { reason } => self.mp_disconnect(mp, Some(&reason)),
            ServerMsg::Welcome { .. } | ServerMsg::Info(_) => {}
        }
    }

    fn mp_record_stats(&mut self, mode: Mode, winner: Option<Winner>, you: usize, frames: u64) {
        let s = &mut self.stats;
        let tr = s.gi("total_rounds", 0) + 1;
        s.insert("total_rounds".into(), json!(tr));
        let r = s.gi("mp_rounds", 0) + 1;
        s.insert("mp_rounds".into(), json!(r));
        let won = mode == Mode::Vs && matches!((winner, you), (Some(Winner::P1), 0) | (Some(Winner::P2), 1));
        if won { let w = s.gi("mp_wins", 0) + 1; s.insert("mp_wins".into(), json!(w)); }
        let ttp = s.gf("total_time_played", 0.0) + frames as f64 / TICK_HZ as f64;
        s.insert("total_time_played".into(), Value::from(ttp));
        self.save_section("stats", None);
    }

    fn mp_event_fx(&mut self, mp: &Mp, e: SimEvent, you: usize) {
        let du = self.tde("Du", "You");
        let who = |i: usize| if i == you { du.clone() } else { format!("P{}", i + 1) };
        let pos = |g: &G, i: usize| -> Option<(i32, i32)> {
            let r = mp.round.as_ref()?;
            let p = if i == r.you { r.own_draw(1.0) } else { r.other_draw()? };
            Some(g.mp_field_point(p.rect.centerx(), p.rect.top() - 20))
        };
        let (snd, text, col, at) = match e {
            SimEvent::Hit(i) => (Snd::Hit, format!("{} {}!", who(i), self.tde("getroffen", "hit")), MP_COLORS[i], pos(self, i)),
            SimEvent::Died(i) => (Snd::Hit, format!("{} {}", who(i), self.tde("ausgeschieden", "eliminated")), (220, 70, 70), None),
            SimEvent::OneUp(i) => (Snd::OneUp, format!("1-UP ({})", who(i)), super::art::ONEUP, pos(self, i)),
            SimEvent::Revived(i) => (Snd::OneUp, format!("{} {}", who(i), self.tde("wiederbelebt!", "revived!")), MP_COLORS[i], pos(self, i)),
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
            let raw = Input {
                jump: self.any_action_pressed("jump") || mp.jump_latch,
                left: self.any_action_pressed("left"),
                right: self.any_action_pressed("right"),
            };
            mp.jump_latch = false;
            mp.fx.update(LOGIC_TICK_MS);
            let (f, inp) = r.tick(raw);
            if let Some(c) = mp.client.as_mut() { c.send_input(f, inp); }
        }
    }

    // ------------------------------------------------------------------
    // Eingaben
    // ------------------------------------------------------------------
    fn mp_event(&mut self, mp: &mut Mp, ev: Event) {
        match ev {
            Event::Quit => { if let Some(c) = mp.client.take() { c.disconnect(); } self.quit(); }
            Event::KeyDown { key, unicode, mods } => self.mp_key(mp, key, &unicode, mods),
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
        if mp.screen == Screen::Playing {
            if self.action_matches_key(key, "jump") { mp.jump_latch = true; }
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
            let field = if mp.focus == Field::Addr { &mut mp.addr_input } else { &mut mp.code_input };
            let max = if mp.focus == Field::Addr { 80 } else { 4 };
            if key == K_BACKSPACE { field.pop(); return; }
            if key == K_RETURN || key == K_KP_ENTER {
                let a = if mp.focus == Field::Addr { "connect_input" } else { "join_code" };
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
                if mp.focus == Field::Code { if ch.is_ascii_alphanumeric() { field.push(ch.to_ascii_uppercase()); } }
                else if !ch.is_whitespace() { field.push(ch); }
            }
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
        }
    }

    fn mp_action(&mut self, mp: &mut Mp, a: &str) {
        let (cmd, arg) = a.split_once(':').unwrap_or((a, ""));
        match cmd {
            "back" => { self.play_sound(Snd::ClickBack); self.mp_leave_screen(mp); }
            "focus_addr" => mp.focus = Field::Addr,
            "focus_code" => mp.focus = Field::Code,
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
            "mode" => { self.play_sound(Snd::Click); mp.create_mode = if arg == "coop" { Mode::Coop } else { Mode::Vs }; }
            "diff" => {
                self.play_sound(Snd::Click);
                let n = DIFFICULTIES.len();
                mp.create_diff = if arg == "-" { (mp.create_diff + n - 1) % n } else { (mp.create_diff + 1) % n };
            }
            "create" => {
                self.play_sound(Snd::MenuClick);
                let d = DIFFICULTIES[mp.create_diff].to_string();
                mp.send(ClientMsg::CreateRoom { mode: mp.create_mode, difficulty: d });
            }
            "join" => { self.play_sound(Snd::MenuClick); mp.send(ClientMsg::JoinRoom { code: arg.to_string() }); }
            "join_code" => {
                let c = mp.code_input.trim().to_uppercase();
                if c.len() == 4 { self.play_sound(Snd::MenuClick); mp.focus = Field::None; mp.send(ClientMsg::JoinRoom { code: c }); }
                else { mp.say(&self.tde("Der Raumcode hat 4 Zeichen", "Room codes have 4 characters"), true); }
            }
            "ready" => {
                self.play_sound(Snd::Click);
                let me_ready = mp.room.as_ref().and_then(|r| r.players[r.you].as_ref().map(|p| p.ready)).unwrap_or(false);
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
            "create" => {
                mp.create_mode = if p.get(1).map(|m| m.eq_ignore_ascii_case("coop")).unwrap_or(false) { Mode::Coop } else { Mode::Vs };
                if let Some(i) = p.get(2).and_then(|d| DIFFICULTIES.iter().position(|x| x.eq_ignore_ascii_case(d))) { mp.create_diff = i; }
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
                if i.game_version != GAME_VERSION || i.proto != PROTO_VERSION {
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
        let sub = self.tde("Online gegen oder mit einem Freund (2 Spieler)", "Play online against or with a friend (2 players)");
        self.mp_text(&bs, &sf, &sub, (150, 150, 180), w / 2, 82, true);
        let cx = w / 2 - 300;
        // Offizieller Server
        let off = self.mp_official();
        let card = Rect::new(cx, 112, 600, 74);
        draw::rect(&bs, self.get_panel_color(), card, 0, 12);
        draw::rect(&bs, (100, 160, 220), card, 2, 12);
        self.mp_text(&bs, &mf, &self.tde("Offizieller Server", "Official server"), tc, card.x + 16, card.y + 8, false);
        let (st, sc) = self.mp_status_line(mp.infos.get(&off));
        draw::circle(&bs, sc, (card.x + 22, card.y + 54), 6, 0);
        self.mp_text(&bs, &sf, &st, sc, card.x + 36, card.y + 45, false);
        let online = matches!(mp.infos.get(&off), Some(InfoState::Online(..)));
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
        // Zuletzt genutzt
        let recent = self.mp_recent();
        let mut y = 300;
        if !recent.is_empty() {
            self.mp_text(&bs, &sf, &self.tde("Zuletzt genutzt", "Recent servers"), tc, cx, y, false);
            y += 26;
            for a in recent.iter().take(((h - 90 - y) / 40).max(0) as usize) {
                let r = Rect::new(cx, y, 556, 36);
                let hov = r.collidepoint(self.mouse_logical_peek());
                draw::rect(&bs, if hov { self.get_button_hover_color() } else { self.get_button_color() }, r, 0, 8);
                self.mp_text(&bs, &sf, a, tc, r.x + 12, r.y + 8, false);
                let (st, sc) = self.mp_status_line(mp.infos.get(a));
                let stw = sf.size(&st).0.min(300);
                let sts = sf.render(&st, true, sc);
                bs.blit_area(&sts, (r.right() - 12 - stw, r.y + 8), Some(Rect::new(sts.get_width() - stw, 0, stw, sts.get_height())));
                mp.buttons.push((r, format!("connect:{}", a)));
                self.mp_button(&bs, mp, Rect::new(cx + 564, y, 36, 36), "x", &format!("forget:{}", a), (150, 100, 110), None);
                y += 40;
            }
        }
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
        let list = Rect::new(lx, 110, 430, h - 190);
        draw::rect(&bs, self.get_panel_color(), list, 0, 12);
        draw::rect(&bs, (110, 110, 150), list, 2, 12);
        self.mp_text(&bs, &sf, &self.tde("Räume", "Rooms"), tc, list.x + 14, list.y + 10, false);
        let mut y = list.y + 42;
        if mp.rooms.is_empty() {
            self.mp_text(&bs, &sf, &self.tde("Noch keine Räume - erstell einen!", "No rooms yet - create one!"), (140, 140, 160), list.centerx(), y + 30, true);
        }
        for r in mp.rooms.clone() {
            if y + 40 > list.bottom() - 6 { break; }
            let row = Rect::new(list.x + 10, y, list.w - 20, 38);
            let open = r.players < 2 && !r.playing;
            let hov = open && row.collidepoint(self.mouse_logical_peek());
            draw::rect(&bs, if hov { self.get_button_hover_color() } else { self.get_button_color() }, row, 0, 8);
            let col = if open { tc } else { (130, 130, 140) };
            self.mp_text(&bs, &sf, &r.code, (120, 180, 255), row.x + 10, row.y + 9, false);
            let info = format!("{} · {} · {}", mode_name(self, r.mode), self.get_difficulty_name(&r.difficulty), r.host);
            self.mp_text(&bs, &bf, &info, col, row.x + 80, row.y + 12, false);
            let st = if r.playing { self.tde("läuft", "playing") } else { format!("{}/2", r.players) };
            let sw = bf.size(&st).0;
            self.mp_text(&bs, &bf, &st, col, row.right() - 10 - sw, row.y + 12, false);
            if open { mp.buttons.push((row, format!("join:{}", r.code))); }
            y += 44;
        }
        // Raum erstellen
        let rx = w / 2 + 60;
        let panel = Rect::new(rx, 110, 325, 200);
        draw::rect(&bs, self.get_panel_color(), panel, 0, 12);
        draw::rect(&bs, (110, 110, 150), panel, 2, 12);
        self.mp_text(&bs, &sf, &self.tde("Raum erstellen", "Create room"), tc, panel.x + 14, panel.y + 10, false);
        let vs_sel = mp.create_mode == Mode::Vs;
        let sel = |on: bool| if on { (120, 180, 255) } else { (110, 110, 130) };
        self.mp_button(&bs, mp, Rect::new(rx + 14, panel.y + 42, 143, 38), "VS", "mode:vs", sel(vs_sel), None);
        self.mp_button(&bs, mp, Rect::new(rx + 168, panel.y + 42, 143, 38), &mode_name(self, Mode::Coop), "mode:coop", sel(!vs_sel), None);
        self.mp_button(&bs, mp, Rect::new(rx + 14, panel.y + 92, 40, 38), "<", "diff:-", (110, 110, 130), None);
        self.mp_button(&bs, mp, Rect::new(rx + 271, panel.y + 92, 40, 38), ">", "diff:+", (110, 110, 130), None);
        let dn = self.get_difficulty_name(DIFFICULTIES[mp.create_diff]);
        self.mp_text(&bs, &sf, &dn, tc, rx + 162, panel.y + 101, true);
        self.mp_button(&bs, mp, Rect::new(rx + 14, panel.y + 144, 297, 42), &self.tde("Erstellen", "Create"), "create", (90, 190, 140), None);
        // Code
        let jp = Rect::new(rx, 322, 325, 104);
        draw::rect(&bs, self.get_panel_color(), jp, 0, 12);
        draw::rect(&bs, (110, 110, 150), jp, 2, 12);
        self.mp_text(&bs, &sf, &self.tde("Mit Code beitreten", "Join with code"), tc, jp.x + 14, jp.y + 10, false);
        let ci = mp.code_input.clone();
        let foc = mp.focus == Field::Code;
        self.mp_text_field(&bs, mp, Rect::new(rx + 14, jp.y + 44, 160, 44), &ci, "ABCD", foc, "focus_code");
        self.mp_button(&bs, mp, Rect::new(rx + 184, jp.y + 44, 127, 44), &self.tde("Beitreten", "Join"), "join_code", (100, 160, 220), None);
        // unten
        self.mp_button(&bs, mp, Rect::new(w / 2 - 190, h - 62, 180, 44), &self.tde("Aktualisieren", "Refresh"), "refresh", (100, 160, 220), None);
        self.mp_button(&bs, mp, Rect::new(w / 2 + 10, h - 62, 180, 44), &self.tde("Trennen", "Disconnect"), "disconnect", (150, 150, 150), None);
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
        for i in 0..2 {
            let r = Rect::new(w / 2 - 300 + i as i32 * 310, 120, 290, 130);
            draw::rect(&bs, self.get_panel_color(), r, 0, 14);
            draw::rect(&bs, MP_COLORS[i], r, 2, 14);
            super::art::draw_player(&bs, Rect::new(r.x + 20, r.y + 20, 40, 40), MP_COLORS[i]);
            self.mp_text(&bs, &sf, &format!("P{}", i + 1), MP_COLORS[i], r.x + 74, r.y + 18, false);
            match &room.players[i] {
                Some(p) => {
                    let me = if i == room.you { format!(" ({})", self.tde("du", "you")) } else { String::new() };
                    self.mp_text(&bs, &sf, &format!("{}{}", p.name, me), tc, r.x + 74, r.y + 42, false);
                    let (t, c) = if p.ready { (self.tde("Bereit", "Ready"), (90, 190, 140)) } else { (self.tde("Wartet …", "Not ready"), (180, 150, 90)) };
                    self.mp_text(&bs, &sf, &t, c, r.x + 20, r.y + 90, false);
                }
                None => {
                    self.mp_text(&bs, &sf, &self.tde("frei - warte auf Mitspieler", "free - waiting for player"), (140, 140, 160), r.x + 20, r.y + 90, false);
                }
            }
        }
        if room.players.iter().any(|p| p.is_none()) {
            let t = format!("{} {}", self.tde("Gib deinem Freund den Code", "Give your friend the code"), room.code);
            self.mp_text(&bs, &sf, &t, (150, 150, 180), w / 2, 270, true);
        }
        if let Some(c) = mp.countdown {
            thread_local! { static BIG: Font = Font::new(140); }
            let big = BIG.with(|f| f.clone());
            self.mp_text(&bs, &big, &c.to_string(), (120, 180, 255), w / 2, 290, true);
        }
        let me_ready = room.players[room.you].as_ref().map(|p| p.ready).unwrap_or(false);
        let rt = if me_ready { self.tde("Nicht mehr bereit", "Not ready") } else { self.tde("Bereit!", "Ready!") };
        self.mp_button(&bs, mp, Rect::new(w / 2 - 190, h - 120, 380, 50), &rt, "ready", if me_ready { (180, 150, 90) } else { (90, 190, 140) }, Some(self.menu_font.clone()));
        self.mp_button(&bs, mp, Rect::new(w / 2 - 90, h - 60, 180, 44), &self.tde("Verlassen", "Leave"), "leave_room", (150, 150, 150), None);
    }

    /// Spielfeld (800x600) in Fensterkoordinaten: (Faktor, Versatz x, Versatz y)
    fn mp_field_tf(&self) -> (f64, f64, f64) {
        let s = (self.WIDTH as f64 / FIELD_W as f64).min(self.HEIGHT as f64 / FIELD_H as f64);
        (s, (self.WIDTH as f64 - FIELD_W as f64 * s) / 2.0, (self.HEIGHT as f64 - FIELD_H as f64 * s) / 2.0)
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

    fn mp_draw_player(&self, bs: &Surface, i: usize, p: &DrawPlayer, label: &str) {
        if !p.alive { return; }
        let r = self.mp_field_rect(p.rect);
        if p.invuln == 0 || (self.get_ticks() / 80) % 2 == 0 {
            super::art::draw_player(bs, r, MP_COLORS[i]);
        }
        let sf = &self.small_font;
        let l = sf.render(label, true, WHITE);
        let ls = sf.render(label, true, (0, 0, 0));
        let (lx, ly) = (r.centerx() - l.get_width() / 2, r.top() - 18);
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
            let fr = self.mp_field_rect(Rect::new(0, 0, FIELD_W, FIELD_H));
            draw::line(&bs, (70, 60, 110), (fr.left() - 1, fr.top()), (fr.left() - 1, fr.bottom()), 1);
            draw::line(&bs, (70, 60, 110), (fr.right(), fr.top()), (fr.right(), fr.bottom()), 1);
        }
        let floor = self.mp_field_rect(Rect::new(0, r.floor_y + 2, FIELD_W, 2));
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
        let other = 1 - you;
        mp.fx.draw(&bs);
        if let Some(o) = r.other_draw() { self.mp_draw_player(&bs, other, &o, &format!("P{}", other + 1)); }
        let me = r.own_draw(alpha);
        self.mp_draw_player(&bs, you, &me, &self.tde("Du", "You"));
        // HUD: Leben P1 links, P2 rechts
        let mf = self.menu_font.clone();
        let sf = self.small_font.clone();
        for i in 0..2 {
            let (alive, lives) = if i == you { (me.alive, me.lives) } else { r.server_player(i).map(|p| (p.alive, p.lives)).unwrap_or((true, 3)) };
            let col = if alive { MP_COLORS[i] } else { (70, 70, 70) };
            let lbl = mf.render(&format!("P{}", i + 1), true, col);
            let cnt = 3.max(lives);
            if i == 0 { bs.blit(&lbl, (12, 10)); } else { bs.blit(&lbl, (w - 12 - lbl.get_width(), 10)); }
            for k in 0..cnt {
                let c = if k < lives { col } else { (55, 55, 55) };
                let x = if i == 0 { 12 + k as i32 * 15 } else { w - 12 - (cnt - k) as i32 * 15 };
                draw::rect(&bs, c, Rect::new(x, 10 + lbl.get_height() + 3, 11, 11), 0, 3);
            }
        }
        let secs = mp.round_started.elapsed().as_secs();
        let ss = mf.render(&format!("{}: {}", self.get_text("score"), r.score), true, self.get_text_color());
        bs.blit(&ss, (w / 2 - ss.get_width() / 2, 10));
        let ping = mp.ping_ms.map(|p| format!("  ·  {} ms", p)).unwrap_or_default();
        let info = format!("Online-{}  ·  {}s  ·  {}{}", mode_name(self, r.mode), secs, self.get_difficulty_name(&r.difficulty), ping);
        let is = sf.render(&info, true, (140, 90, 210));
        bs.blit(&is, (w / 2 - is.get_width() / 2, 12 + ss.get_height()));
        if !me.alive {
            let t = if r.mode == Mode::Coop { self.tde("Ausgeschieden - dein Partner kann dich mit einem 1-UP wiederbeleben", "Eliminated - your partner can revive you with a 1-UP") } else { self.tde("Ausgeschieden", "Eliminated") };
            self.mp_text(&bs, &sf, &t, (220, 90, 100), w / 2, h / 2 - 40, true);
        }
        let hint = if mp.leave_confirm_until.map(|t| Instant::now() < t).unwrap_or(false) {
            (self.tde("Nochmal ESC = Runde verlassen (online gibt es keine Pause)", "Press ESC again to leave (no pause online)"), (230, 120, 130))
        } else { (self.tde("ESC: Runde verlassen", "ESC: leave round"), (90, 90, 110)) };
        self.mp_text(&bs, &sf, &hint.0, hint.1, w / 2, h - 20, true);
        self.draw_overlays_ingame();
    }

    fn mp_draw_result(&mut self, mp: &mut Mp) {
        let bs = self.base_surface.clone();
        let (w, h) = (self.WIDTH, self.HEIGHT);
        let Some((winner, score, reason, frames)) = mp.result.clone() else { return; };
        let you = mp.round.as_ref().map(|r| r.you).or(mp.room.as_ref().map(|r| r.you)).unwrap_or(0);
        let mode = mp.round.as_ref().map(|r| r.mode).or(mp.room.as_ref().map(|r| r.mode)).unwrap_or(Mode::Vs);
        let (title, col) = match (mode, winner) {
            (Mode::Coop, _) => (self.tde("Runde vorbei", "Round over"), (120, 180, 255)),
            (_, Some(Winner::Draw)) => (self.tde("Unentschieden!", "Draw!"), (200, 200, 220)),
            (_, Some(Winner::P1)) if you == 0 => (self.tde("Gewonnen!", "You win!"), (90, 200, 140)),
            (_, Some(Winner::P2)) if you == 1 => (self.tde("Gewonnen!", "You win!"), (90, 200, 140)),
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
            let other = &room.players[1 - room.you];
            let t = match other {
                Some(p) if p.ready => format!("{} {}", p.name, self.tde("will nochmal!", "wants a rematch!")),
                Some(p) => format!("{}: {}", self.tde("Mitspieler", "Opponent"), p.name),
                None => self.tde("Dein Mitspieler hat den Raum verlassen", "The other player left the room"),
            };
            self.mp_text(&bs, &sf, &t, (150, 150, 180), w / 2, 290, true);
        }
        let can_rematch = mp.room.as_ref().map(|r| r.players.iter().all(|p| p.is_some())).unwrap_or(false);
        if can_rematch {
            self.mp_button(&bs, mp, Rect::new(w / 2 - 190, h - 130, 380, 54), &self.tde("Nochmal!", "Rematch!"), "rematch", (90, 190, 140), Some(mf.clone()));
        }
        self.mp_button(&bs, mp, Rect::new(w / 2 - 110, h - 64, 220, 44), &self.tde("Raum verlassen", "Leave room"), "leave_room", (150, 150, 150), None);
    }
}

const DIFFICULTIES: [&str; 8] = jnd_core::net::proto::DIFFICULTIES;
