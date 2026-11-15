//! Integrationstests: echter Server + echte Clients über UDP auf localhost.

use jnd_core::net::channel::Lossy;
use jnd_core::net::client::{announce, fetch_server_list, query_info, Client, ConnectOptions};
use jnd_core::net::proto::*;
use jnd_core::sim2p::{Input, Winner};
use jnd_server::config::Config;
use jnd_server::server::Server;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::io::Read;
use std::time::{Duration, Instant};

struct TestServer { addr: String, ws: Option<String>, stop: Arc<AtomicBool>, handle: Option<std::thread::JoinHandle<()>> }
impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() { let _ = h.join(); }
    }
}

fn start_server(password: &str, max_rooms: u32, loss: f64) -> TestServer { start_server_ws(password, max_rooms, loss, 0) }

/// Freien TCP-Port suchen (für den WebSocket-Port des Testservers)
fn free_tcp_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).expect("freier Port")
}

/// Wie start_server, zusätzlich mit WebSocket-Port (0 = aus)
fn start_server_ws(password: &str, max_rooms: u32, loss: f64, ws_port: u16) -> TestServer {
    let cfg = Config { port: 0, ws_port, auto_update: false, bind: "127.0.0.1".into(), password: password.into(), max_rooms, log_file: String::new(), name: "Testserver".into(), ..Config::default() };
    start_with(cfg, loss, false)
}

fn start_with(cfg: Config, loss: f64, allow_local: bool) -> TestServer {
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel();
    let st = stop.clone();
    let handle = std::thread::spawn(move || {
        let mut s = Server::bind(cfg, Box::new(|l: &str| println!("[server] {l}"))).expect("bind");
        s.loss = Lossy(loss);
        s.registry.allow_local = allow_local;
        s.replays.set_dir(std::env::temp_dir().join(format!("jnd-replays-test-{}-{}", std::process::id(), rand_suffix())));
        tx.send((s.local_addr().to_string(), s.ws_addr().map(|a| format!("ws://127.0.0.1:{}", a.port())))).unwrap();
        s.run(&st);
    });
    let (addr, ws) = rx.recv().unwrap();
    TestServer { addr, ws, stop, handle: Some(handle) }
}

fn opts(name: &str, loss: f64) -> ConnectOptions {
    let mut o = ConnectOptions::new(name);
    o.loss = Lossy(loss);
    o
}

/// Pollt, bis `f` für eine Nachricht Some liefert
fn wait_for<T>(c: &mut Client, secs: u64, f: impl FnMut(&ServerMsg) -> Option<T>) -> T { wait_pump(c, None, secs, f) }

/// Wie wait_for, hält dabei aber einen zweiten Client am Leben (Wiederholungen/Bestätigungen),
/// ohne dessen Nachrichten wegzuwerfen - wie im Spiel, wo jeder Client jeden Frame läuft.
fn wait_pump<T>(c: &mut Client, mut other: Option<&mut Client>, secs: u64, mut f: impl FnMut(&ServerMsg) -> Option<T>) -> T {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        if let Some(o) = other.as_deref_mut() { o.pump(); }
        for m in c.poll() { if let Some(v) = f(&m) { return v; } }
        std::thread::sleep(Duration::from_millis(3));
    }
    panic!("Zeitüberschreitung beim Warten");
}

/// Zwei Spieler in einem Raum, beide bereit, bis zum Start
fn setup_round(srv: &TestServer, mode: Mode, diff: &str, loss: f64) -> (Client, Client, u64) {
    let mut a = Client::connect(&srv.addr, opts("Alice", loss)).expect("Alice verbindet");
    let mut b = Client::connect(&srv.addr, opts("Bob", loss)).expect("Bob verbindet");
    a.send(&ClientMsg::CreateRoom { mode, difficulty: diff.into(), max_players: 2 });
    let code = wait_pump(&mut a, Some(&mut b), 10, |m| if let ServerMsg::RoomState { code, .. } = m { Some(code.clone()) } else { None });
    b.send(&ClientMsg::ListRooms);
    let list = wait_pump(&mut b, Some(&mut a), 10, |m| if let ServerMsg::RoomList(l) = m { Some(l.clone()) } else { None });
    assert!(list.iter().any(|r| r.code == code && r.players == 1 && r.host == "Alice"), "Raum steht in der Liste");
    b.send(&ClientMsg::JoinRoom { code: code.to_lowercase() });
    wait_pump(&mut b, Some(&mut a), 10, |m| if let ServerMsg::RoomState { players, you, .. } = m { (players[0].is_some() && players[1].is_some() && *you == 1).then_some(()) } else { None });
    a.send(&ClientMsg::Ready(true));
    b.send(&ClientMsg::Ready(true));
    let sa = wait_pump(&mut a, Some(&mut b), 15, |m| if let ServerMsg::Start { seed, .. } = m { Some(*seed) } else { None });
    let sb = wait_pump(&mut b, Some(&mut a), 15, |m| if let ServerMsg::Start { seed, .. } = m { Some(*seed) } else { None });
    assert_eq!(sa, sb, "beide bekommen denselben Seed");
    (a, b, sa)
}

#[test]
fn serverinfo() {
    let srv = start_server("", 4, 0.0);
    let (info, ping) = query_info(&srv.addr, Duration::from_secs(3)).expect("Info");
    assert_eq!(info.name, "Testserver");
    assert_eq!(info.game_version, GAME_VERSION);
    assert!(!info.password);
    assert!(ping < Duration::from_secs(1));
}

#[test]
fn falsche_version_und_passwort() {
    let srv = start_server("geheim", 4, 0.0);
    // andere Spielversion, gleiches Protokoll: darf mitspielen
    let mut o = opts("Alt", 0.0);
    o.game_version = "5.0.0".into();
    o.password = "geheim".into();
    assert!(Client::connect(&srv.addr, o).is_ok(), "ältere Spielversion mit gleichem Protokoll passt");
    // anderes Protokoll: gesperrt, mit Hinweis wer aktualisieren muss
    let mut o = opts("Eve", 0.0);
    o.proto = PROTO_VERSION - 1;
    o.password = "geheim".into();
    let e = Client::connect(&srv.addr, o).err().expect("altes Protokoll wird abgelehnt");
    assert!(e.contains("zu alt"), "{e}");
    let e = Client::connect(&srv.addr, opts("Eve", 0.0)).err().expect("Passwort wird abgelehnt");
    assert!(e.contains("Passwort"), "{e}");
    let mut o = opts("Eve", 0.0);
    o.password = "geheim".into();
    assert!(Client::connect(&srv.addr, o).is_ok());
    // kein Server -> sinnvolle Meldung statt Hängen
    let mut o = opts("X", 0.0);
    o.timeout = Duration::from_millis(600);
    assert!(Client::connect("127.0.0.1:1", o).is_err());
}

/// Komplette VS-Runde: Eingaben -> Snapshots -> GameOver bei beiden gleich, danach zurück in der Lobby
fn play_vs(loss: f64) {
    let srv = start_server("", 4, loss);
    let (mut a, mut b, _) = setup_round(&srv, Mode::Vs, "Hölle", loss);
    let start = Instant::now();
    let (mut frame, mut snaps) = (0u64, 0u32);
    let mut over: [Option<(Option<Winner>, i64)>; 2] = [None, None];
    let mut last_send = Instant::now();
    // Raumstatus, der NACH dem GameOver kam (zurück in der Lobby)
    let mut lobby_after: [Option<Vec<bool>>; 2] = [None, None];
    while (over.iter().any(|o| o.is_none()) || lobby_after.iter().any(|l| l.is_none())) && start.elapsed() < Duration::from_secs(60) {
        if last_send.elapsed() >= Duration::from_millis(16) {
            frame += 1;
            last_send = Instant::now();
            // Alice springt regelmäßig, Bob steht nur herum
            a.send_input(frame, Input { jump: frame % 30 < 2, ..Default::default() });
            b.send_input(frame, Input::default());
        }
        for (i, c) in [&mut a, &mut b].into_iter().enumerate() {
            for m in c.poll() {
                match m {
                    ServerMsg::Snapshot(s) => { snaps += 1; assert_eq!(s.players.len(), 2); }
                    ServerMsg::GameOver { winner, score, .. } => over[i] = Some((winner, score)),
                    ServerMsg::RoomState { players, .. } if over[i].is_some() => {
                        lobby_after[i] = Some(players.iter().flatten().map(|p| p.ready).collect());
                    }
                    _ => {}
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let (oa, ob) = (over[0].expect("Alice bekommt GameOver"), over[1].expect("Bob bekommt GameOver"));
    assert_eq!(oa, ob, "beide sehen dasselbe Ergebnis");
    assert!(oa.0.is_some(), "VS hat einen Sieger");
    assert!(snaps > 20, "Snapshots kommen an ({snaps})");
    // nach der Runde: beide wieder in der Lobby, keiner bereit (Rematch möglich)
    assert_eq!(lobby_after[0].as_deref(), Some(&[false, false][..]));
    assert_eq!(lobby_after[1].as_deref(), Some(&[false, false][..]));
}

#[test]
fn vs_runde() { play_vs(0.0); }

#[test]
fn vs_runde_mit_paketverlust() { play_vs(0.15); }

#[test]
fn abbruch_im_vs_gegner_gewinnt() {
    let srv = start_server("", 4, 0.0);
    let (mut a, b, _) = setup_round(&srv, Mode::Vs, "Leicht", 0.0);
    wait_for(&mut a, 10, |m| matches!(m, ServerMsg::Snapshot(_)).then_some(()));
    b.disconnect();
    let (w, reason) = wait_for(&mut a, 10, |m| if let ServerMsg::GameOver { winner, reason, .. } = m { Some((*winner, reason.clone())) } else { None });
    assert_eq!(w, Some(Winner::P1));
    assert!(reason.contains("Bob"), "{reason}");
}

#[test]
fn abbruch_im_koop_partner_spielt_weiter() {
    let srv = start_server("", 4, 0.0);
    let (mut a, b, _) = setup_round(&srv, Mode::Coop, "Leicht", 0.0);
    wait_for(&mut a, 10, |m| matches!(m, ServerMsg::Snapshot(_)).then_some(()));
    b.disconnect();
    // Alice bekommt weiter Snapshots, Bob ist ausgeschieden, Alice lebt
    let s = wait_for(&mut a, 10, |m| if let ServerMsg::Snapshot(s) = m { (!s.players[1].alive).then(|| s.clone()) } else { None });
    assert!(s.players[0].alive);
    let later = wait_for(&mut a, 10, |m| if let ServerMsg::Snapshot(x) = m { (x.frame > s.frame + 30).then_some(x.frame) } else { None });
    assert!(later > s.frame);
}

#[test]
fn raum_voll_und_unbekannt() {
    let srv = start_server("", 1, 0.0);
    let (_a, _b, _) = setup_round(&srv, Mode::Vs, "Leicht", 0.0);
    let mut c = Client::connect(&srv.addr, opts("Carl", 0.0)).unwrap();
    c.send(&ClientMsg::JoinRoom { code: "ZZZZ".into() });
    let e = wait_for(&mut c, 10, |m| if let ServerMsg::Error(e) = m { Some(e.clone()) } else { None });
    assert!(e.contains("gibt es nicht"), "{e}");
    c.send(&ClientMsg::CreateRoom { mode: Mode::Vs, difficulty: "Leicht".into(), max_players: 2 });
    let e = wait_for(&mut c, 10, |m| if let ServerMsg::Error(e) = m { Some(e.clone()) } else { None });
    assert!(e.contains("voll"), "{e}");
}

#[test]
fn chat_mit_verlauf_und_spamschutz() {
    let srv = start_server("", 4, 0.0);
    let mut a = Client::connect(&srv.addr, opts("Alice", 0.0)).expect("Alice verbindet");
    // Nachricht, bevor Bob da ist: Bob bekommt sie beim Beitreten aus dem Verlauf (24 Stunden)
    a.send(&ClientMsg::Chat { text: "vorher".into() });
    wait_for(&mut a, 5, |m| matches!(m, ServerMsg::Chat { text, .. } if text == "vorher").then_some(()));
    let mut b = Client::connect(&srv.addr, opts("Bob", 0.0)).expect("Bob verbindet");
    let hist = wait_pump(&mut b, Some(&mut a), 5, |m| match m {
        ServerMsg::Chat { from_id, from, text } if *from_id == CHAT_HISTORY_ID => Some((from.clone(), text.clone())),
        _ => None,
    });
    assert_eq!((hist.0.as_str(), hist.1.as_str()), ("Alice", "vorher"), "Verlauf beim Beitreten");
    // Alice bekommt den Hinweis "Bob ist da"
    wait_pump(&mut a, Some(&mut b), 5, |m| matches!(m, ServerMsg::Chat { from_id: 0, text, .. } if text.contains("Bob")).then_some(()));
    a.send(&ClientMsg::Chat { text: "  hallo
  Bob  ".into() });
    let (from, text) = wait_pump(&mut b, Some(&mut a), 5, |m| match m {
        ServerMsg::Chat { from_id, from, text } if *from_id != 0 && *from_id != CHAT_HISTORY_ID => Some((from.clone(), text.clone())),
        _ => None,
    });
    assert_eq!((from.as_str(), text.as_str()), ("Alice", "hallo Bob"), "bereinigt und mit Absender");
    // Spam: nach CHAT_BURST Nachrichten kommt der Hinweis an den Absender
    for i in 0..8 { b.send(&ClientMsg::Chat { text: format!("spam {i}") }); }
    wait_pump(&mut b, Some(&mut a), 5, |m| matches!(m, ServerMsg::Chat { from_id: 0, text, .. } if text.contains("schnell")).then_some(()));
}

/// WebSocket-Weg (wie hinter einem Cloudflare Tunnel): Serverinfo, Verbinden, Raum und Chat über ws://
#[test]
fn websocket_raum_und_chat() {
    let srv = start_server_ws("", 4, 0.0, free_tcp_port());
    let url = srv.ws.clone().expect("WebSocket-Port offen");
    let (info, _) = query_info(&url, Duration::from_secs(3)).expect("Info per WebSocket");
    assert_eq!(info.name, "Testserver");
    assert_eq!(info.proto, PROTO_VERSION);
    let mut a = Client::connect(&url, opts("Alice", 0.0)).expect("Alice per WebSocket");
    assert!(a.via_websocket());
    // gemischt: Bob kommt per UDP, beide landen auf demselben Server
    let mut b = Client::connect(&srv.addr, opts("Bob", 0.0)).expect("Bob per UDP");
    assert!(!b.via_websocket());
    a.send(&ClientMsg::CreateRoom { mode: Mode::Vs, difficulty: "Leicht".into(), max_players: 2 });
    let code = wait_pump(&mut a, Some(&mut b), 10, |m| if let ServerMsg::RoomState { code, .. } = m { Some(code.clone()) } else { None });
    b.send(&ClientMsg::JoinRoom { code: code.clone() });
    wait_pump(&mut a, Some(&mut b), 10, |m| if let ServerMsg::RoomState { players, .. } = m { (players[0].is_some() && players[1].is_some()).then_some(()) } else { None });
    b.send(&ClientMsg::Chat { text: "hallo per udp".into() });
    // (eigene Nachrichten kommen als Echo auch zurück - nach Absender filtern)
    let text = wait_pump(&mut a, Some(&mut b), 5, |m| match m {
        ServerMsg::Chat { from_id, from, text } if *from_id != 0 && from == "Bob" => Some(text.clone()),
        _ => None,
    });
    assert_eq!(text, "hallo per udp");
    a.send(&ClientMsg::Chat { text: "hallo per ws".into() });
    let text = wait_pump(&mut b, Some(&mut a), 5, |m| match m {
        ServerMsg::Chat { from_id, from, text } if *from_id != 0 && from == "Alice" => Some(text.clone()),
        _ => None,
    });
    assert_eq!(text, "hallo per ws");
    a.disconnect();
    b.disconnect();
}

#[test]
fn feldbreite_nach_bildschirmen() {
    let srv = start_server("", 4, 0.0);
    let mut a = Client::connect(&srv.addr, opts("Breit", 0.0)).expect("A verbindet");
    let mut b = Client::connect(&srv.addr, opts("Normal", 0.0)).expect("B verbindet");
    a.send(&ClientMsg::Screen { w: 2560, h: 1080 });
    b.send(&ClientMsg::Screen { w: 1920, h: 1080 });
    a.send(&ClientMsg::CreateRoom { mode: Mode::Vs, difficulty: "Leicht".into(), max_players: 2 });
    let code = wait_pump(&mut a, Some(&mut b), 10, |m| if let ServerMsg::RoomState { code, .. } = m { Some(code.clone()) } else { None });
    b.send(&ClientMsg::JoinRoom { code });
    wait_pump(&mut b, Some(&mut a), 10, |m| if let ServerMsg::RoomState { players, .. } = m { (players[0].is_some() && players[1].is_some()).then_some(()) } else { None });
    a.send(&ClientMsg::Ready(true));
    b.send(&ClientMsg::Ready(true));
    let (w, h) = wait_pump(&mut a, Some(&mut b), 15, |m| if let ServerMsg::Start { width, height, .. } = m { Some((*width, *height)) } else { None });
    // kleinster gemeinsamer Bildschirm, in echten Pixeln
    assert_eq!((w, h), (1920, 1080));
}


#[test]
fn vier_spieler_ein_raum() {
    let srv = start_server("", 4, 0.0);
    let mut cs: Vec<Client> = ["Anna", "Ben", "Cem", "Dana"].iter().map(|n| Client::connect(&srv.addr, opts(n, 0.0)).expect("verbindet")).collect();
    cs[0].send(&ClientMsg::CreateRoom { mode: Mode::Vs, difficulty: "Leicht".into(), max_players: 4 });
    let code = {
        let (a, rest) = cs.split_at_mut(1);
        wait_pump(&mut a[0], Some(&mut rest[0]), 10, |m| if let ServerMsg::RoomState { code, players, .. } = m { (players.len() == 4).then(|| code.clone()) } else { None })
    };
    for i in 1..4 { cs[i].send(&ClientMsg::JoinRoom { code: code.clone() }); }
    // warten, bis alle vier drin sind (alle Clients am Leben halten)
    let start = Instant::now();
    let mut full = false;
    while start.elapsed() < Duration::from_secs(10) && !full {
        for c in cs.iter_mut() {
            for m in c.poll() { if let ServerMsg::RoomState { players, .. } = m { if players.iter().all(|p| p.is_some()) { full = true; } } }
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(full, "alle vier im Raum");
    for c in cs.iter_mut() { c.send(&ClientMsg::Ready(true)); }
    let mut starts: Vec<Option<(u8, usize)>> = vec![None; 4];
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(15) && starts.iter().any(|s| s.is_none()) {
        for (i, c) in cs.iter_mut().enumerate() {
            for m in c.poll() { if let ServerMsg::Start { you, names, .. } = m { starts[i] = Some((you, names.len())); } }
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    let mut yous: Vec<u8> = starts.iter().map(|s| s.expect("Start bekommen").0).collect();
    assert!(starts.iter().all(|s| s.unwrap().1 == 4), "vier Namen");
    yous.sort();
    assert_eq!(yous, vec![0, 1, 2, 3], "jeder hat seine eigene Nummer");
}

/// Mehrere Clients mit Puffer: nichts geht verloren, wenn in einem Schwung mehrere Nachrichten kommen
struct Pool { cs: Vec<Client>, buf: Vec<std::collections::VecDeque<ServerMsg>> }
impl Pool {
    fn new(cs: Vec<Client>) -> Pool { let n = cs.len(); Pool { cs, buf: (0..n).map(|_| Default::default()).collect() } }
    fn send(&mut self, i: usize, m: ClientMsg) { self.cs[i].send(&m); }
    /// Alle pumpen, bis `f` für Client `i` etwas liefert (übersprungene Nachrichten werden verworfen)
    fn wait<T>(&mut self, i: usize, secs: u64, mut f: impl FnMut(&ServerMsg) -> Option<T>) -> T {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(secs) {
            for (k, c) in self.cs.iter_mut().enumerate() { self.buf[k].extend(c.poll()); }
            while let Some(m) = self.buf[i].pop_front() { if let Some(v) = f(&m) { return v; } }
            std::thread::sleep(Duration::from_millis(3));
        }
        panic!("Zeitüberschreitung beim Warten (Client {i})");
    }
}

#[test]
fn schnell_spielen_chef_und_nachrichten() {
    let srv = start_server("", 8, 0.0);
    let mut p = Pool::new(["Anna", "Ben", "Cem"].iter().map(|n| Client::connect(&srv.addr, opts(n, 0.0)).expect("verbindet")).collect());
    // Anna: Schnell spielen -> neuer Raum, sie ist Chef (Platz 0)
    p.send(0, ClientMsg::QuickPlay);
    let code = p.wait(0, 10, |m| if let ServerMsg::RoomState { code, players, .. } = m { (players.len() == QUICK_ROOM_MAX as usize).then(|| code.clone()) } else { None });
    p.wait(0, 10, |m| matches!(m, ServerMsg::RoomExtra { host: 0, private: false }).then_some(()));
    // Ben und Cem: Schnell spielen -> derselbe Raum
    p.send(1, ClientMsg::QuickPlay);
    let c1 = p.wait(1, 10, |m| if let ServerMsg::RoomState { code, .. } = m { Some(code.clone()) } else { None });
    p.send(2, ClientMsg::QuickPlay);
    let c2 = p.wait(2, 10, |m| if let ServerMsg::RoomState { code, .. } = m { Some(code.clone()) } else { None });
    assert_eq!((c1.as_str(), c2.as_str()), (code.as_str(), code.as_str()), "Schnell spielen landet im offenen Raum");
    // Nur der Chef darf rauswerfen: Ben versucht Cem rauszuwerfen -> nichts; Anna wirft Cem raus
    p.send(1, ClientMsg::Kick { slot: 2 });
    p.send(0, ClientMsg::Kick { slot: 2 });
    let r = p.wait(2, 10, |m| if let ServerMsg::LeftRoom { reason } = m { Some(reason.clone()) } else { None });
    assert!(r.contains("entfernt"), "{r}");
    // Privat: Raum verschwindet aus der Liste
    p.send(0, ClientMsg::SetPrivate(true));
    p.wait(0, 10, |m| matches!(m, ServerMsg::RoomExtra { private: true, .. }).then_some(()));
    p.send(2, ClientMsg::ListRooms);
    let list = p.wait(2, 10, |m| if let ServerMsg::RoomList(l) = m { Some(l.clone()) } else { None });
    assert!(list.iter().all(|r| r.code != code), "privater Raum nicht in der Liste");
    // Schnell-Nachricht kommt bei Ben an
    p.send(0, ClientMsg::Emote(1));
    p.wait(1, 10, |m| matches!(m, ServerMsg::Emote { player: 0, id: 1, in_round: false }).then_some(()));
    // Chef startet, obwohl Ben nicht bereit ist
    p.send(1, ClientMsg::HostStart); // Ben ist nicht Chef -> darf nicht
    p.send(0, ClientMsg::HostStart);
    let (you, names) = p.wait(1, 15, |m| if let ServerMsg::Start { you, names, .. } = m { Some((*you, names.clone())) } else { None });
    assert_eq!((you, names.len()), (1, 2), "Runde mit Anna und Ben");
}

#[test]
fn aussehen_und_zuschauen() {
    let srv = start_server("", 8, 0.0);
    let mut p = Pool::new(["Anna", "Ben", "Cem"].iter().map(|n| Client::connect(&srv.addr, opts(n, 0.0)).expect("verbindet")).collect());
    p.send(0, ClientMsg::CreateRoom { mode: Mode::Vs, difficulty: "Hölle".into(), max_players: 3 });
    let code = p.wait(0, 10, |m| if let ServerMsg::RoomState { code, .. } = m { Some(code.clone()) } else { None });
    // Aussehen wird gesäubert und an alle im Raum geschickt
    p.send(0, ClientMsg::Look(PlayerLook { headwear: "Yellow_Crown!<>".into(), trail: "fire_trail".into(), ..Default::default() }));
    p.send(1, ClientMsg::JoinRoom { code: code.clone() });
    let looks = p.wait(1, 10, |m| if let ServerMsg::Looks { in_round: false, looks } = m { (looks.first().map(|l| l.headwear.as_str()) == Some("yellow_crown")).then(|| looks.clone()) } else { None });
    assert_eq!(looks.len(), 3, "ein Aussehen pro Platz");
    assert_eq!(looks[0].trail, "fire_trail");
    p.send(0, ClientMsg::Ready(true));
    p.send(1, ClientMsg::Ready(true));
    p.wait(0, 15, |m| matches!(m, ServerMsg::Start { .. }).then_some(()));
    // Cem schaut der laufenden Runde zu
    p.send(2, ClientMsg::Spectate { code: code.clone() });
    let (you, names) = p.wait(2, 10, |m| if let ServerMsg::Start { you, names, .. } = m { Some((*you, names.clone())) } else { None });
    assert_eq!((you, names.len()), (SPECTATOR, 2), "Zuschauer bekommt Start mit beiden Namen");
    let rl = p.wait(2, 10, |m| if let ServerMsg::Looks { in_round: true, looks } = m { Some(looks.clone()) } else { None });
    assert_eq!(rl[0].headwear, "yellow_crown");
    p.wait(2, 10, |m| if let ServerMsg::Snapshot(s) = m { (s.players.len() == 2).then_some(()) } else { None });
    // Nach der Runde rückt Cem auf den freien dritten Platz
    p.wait(2, 90, |m| matches!(m, ServerMsg::GameOver { .. }).then_some(()));
    let me = p.wait(2, 15, |m| if let ServerMsg::RoomState { you, players, .. } = m { (players.iter().flatten().count() == 3).then_some(*you) } else { None });
    assert_eq!(me, 2, "Zuschauer sitzt jetzt auf Platz 3");
}

#[test]
fn oeffentliche_server_liste() {
    use jnd_server::registry::clean_address;
    // Nur öffentlich erreichbare Adressen kommen in die echte Liste
    for (a, ok) in [("play.example.com", true), ("wss://jnd.example.com", true), ("8.8.8.8:47800", true), ("192.168.0.20", false),
                    ("10.0.0.1:47800", false), ("127.0.0.1", false), ("localhost", false), ("[fd00::1]:47800", false), ("bad name!", false)] {
        assert_eq!(clean_address(a, false).is_some(), ok, "{a}");
    }
    let free_udp = || std::net::UdpSocket::bind("127.0.0.1:0").and_then(|s| s.local_addr()).map(|a| a.port()).expect("freier Port");
    let base = Config { port: 0, ws_port: 0, auto_update: false, bind: "127.0.0.1".into(), log_file: String::new(), ..Config::default() };
    let reg = start_with(Config { name: "Liste".into(), ..base.clone() }, 0.0, true);
    let port = free_udp();
    let _srv = start_with(Config { name: "Gelistet".into(), port, listed: true, public_address: format!("127.0.0.1:{port}"), registry: reg.addr.clone(), ..base.clone() }, 0.0, false);
    let start = Instant::now();
    loop {
        let l = fetch_server_list(&reg.addr, Duration::from_secs(2)).expect("Liste kommt");
        if let Some(s) = l.iter().find(|s| s.info.name == "Gelistet") { assert_eq!(s.address, format!("127.0.0.1:{port}")); break; }
        assert!(start.elapsed() < Duration::from_secs(20), "Server erscheint nicht in der Liste");
        std::thread::sleep(Duration::from_millis(200));
    }
    // Eine Adresse ohne Server wird geprüft, aber nicht aufgenommen
    let (ok, _) = announce(&reg.addr, &format!("127.0.0.1:{}", free_udp()), Duration::from_secs(2)).expect("Antwort");
    assert!(ok, "Anmeldung wird zur Prüfung angenommen");
    std::thread::sleep(Duration::from_millis(4800));
    let l = fetch_server_list(&reg.addr, Duration::from_secs(2)).expect("Liste kommt");
    assert_eq!(l.len(), 1, "nur der echte Server steht in der Liste: {l:?}");
}

fn rand_suffix() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0) }

/// Minimales Replay (JNR1) mit Punktzahl
fn fake_replay(diff: &str, score: i64) -> Vec<u8> {
    let mut b = b"JNR1".to_vec();
    b.extend_from_slice(&800i16.to_le_bytes()); b.extend_from_slice(&600i16.to_le_bytes());
    b.extend_from_slice(&[10, 20, 30]);
    b.push(diff.len() as u8); b.extend_from_slice(diff.as_bytes());
    b.extend_from_slice(&score.to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&[0, 0, 0, 0, 1, 0, 0, 0, 0, 3, 0]);
    b
}

#[test]
fn replays_bestlauf_und_teilen() {
    let srv = start_server_ws("", 4, 0.0, free_tcp_port());
    let base = srv.ws.clone().expect("ws").replace("ws://", "http://");
    let get = |p: &str| ureq::get(&format!("{base}{p}")).call();
    let post = |p: &str, b: &[u8]| ureq::post(&format!("{base}{p}")).send_bytes(b);
    assert_eq!(get("/replays/ping").expect("ping").into_string().unwrap(), "ok");
    // Bestlauf speichern, schlechterer wird ignoriert, fremder Schlüssel abgelehnt
    let r = post("/replays/best?user=Anna&diff=Mittel&key=0123456789abcdef0123", &fake_replay("Mittel", 50)).expect("upload");
    assert_eq!(r.into_string().unwrap(), "gespeichert");
    assert_eq!(post("/replays/best?user=anna&diff=Mittel&key=0123456789abcdef0123", &fake_replay("Mittel", 40)).unwrap().into_string().unwrap(), "alt");
    match post("/replays/best?user=anna&diff=Mittel&key=ffffffffffffffffffff", &fake_replay("Mittel", 99)) { Err(ureq::Error::Status(403, _)) => {}, x => panic!("fremder Schlüssel: {:?}", x.map(|r| r.status())) }
    let mut got = vec![];
    get("/replays/best?user=ANNA&diff=mittel").expect("download").into_reader().read_to_end(&mut got).unwrap();
    assert_eq!(got, fake_replay("Mittel", 50));
    assert!(matches!(get("/replays/best?user=ben&diff=mittel"), Err(ureq::Error::Status(404, _))));
    assert!(matches!(post("/replays/share", b"kaputt"), Err(ureq::Error::Status(400, _))));
    // Teilen per Code
    let code = post("/replays/share", &fake_replay("Hölle", 7)).expect("teilen").into_string().unwrap();
    assert_eq!(code.len(), 6, "{code}");
    let mut got = vec![];
    get(&format!("/replays/s/{}", code.to_lowercase())).expect("abholen").into_reader().read_to_end(&mut got).unwrap();
    assert_eq!(got, fake_replay("Hölle", 7));
    // WebSocket-Spiel geht weiter über denselben Port
    let (i, _) = query_info(srv.ws.as_ref().unwrap(), Duration::from_secs(3)).expect("ws info");
    assert_eq!(i.name, "Testserver");
}

#[test]
fn ping_im_raum() {
    let srv = start_server("", 4, 0.0);
    let mut p = Pool::new(["Anna", "Ben"].iter().map(|n| Client::connect(&srv.addr, opts(n, 0.0)).expect("verbindet")).collect());
    p.send(0, ClientMsg::CreateRoom { mode: Mode::Vs, difficulty: "Mittel".into(), max_players: 4 });
    let code = p.wait(0, 10, |m| if let ServerMsg::RoomState { code, .. } = m { Some(code.clone()) } else { None });
    p.send(1, ClientMsg::JoinRoom { code });
    p.wait(1, 10, |m| matches!(m, ServerMsg::RoomState { .. }).then_some(()));
    p.send(0, ClientMsg::MyPing(42));
    let pings = p.wait(1, 10, |m| if let ServerMsg::Pings(v) = m { (v.first() == Some(&42)).then(|| v.clone()) } else { None });
    assert_eq!(pings.len(), 4, "ein Wert pro Platz");
}
