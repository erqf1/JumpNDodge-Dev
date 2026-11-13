//! Integrationstests: echter Server + echte Clients über UDP auf localhost.

use jnd_core::net::channel::Lossy;
use jnd_core::net::client::{query_info, Client, ConnectOptions};
use jnd_core::net::proto::*;
use jnd_core::sim2p::{Input, Winner};
use jnd_server::config::Config;
use jnd_server::server::Server;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct TestServer { addr: String, stop: Arc<AtomicBool>, handle: Option<std::thread::JoinHandle<()>> }
impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() { let _ = h.join(); }
    }
}

fn start_server(password: &str, max_rooms: u32, loss: f64) -> TestServer {
    let cfg = Config { port: 0, bind: "127.0.0.1".into(), password: password.into(), max_rooms, log_file: String::new(), name: "Testserver".into(), ..Config::default() };
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel();
    let st = stop.clone();
    let handle = std::thread::spawn(move || {
        let mut s = Server::bind(cfg, Box::new(|l: &str| println!("[server] {l}"))).expect("bind");
        s.loss = Lossy(loss);
        tx.send(s.local_addr().to_string()).unwrap();
        s.run(&st);
    });
    let addr = rx.recv().unwrap();
    TestServer { addr, stop, handle: Some(handle) }
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
    a.send(&ClientMsg::CreateRoom { mode, difficulty: diff.into() });
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
    c.send(&ClientMsg::CreateRoom { mode: Mode::Vs, difficulty: "Leicht".into() });
    let e = wait_for(&mut c, 10, |m| if let ServerMsg::Error(e) = m { Some(e.clone()) } else { None });
    assert!(e.contains("voll"), "{e}");
}

#[test]
fn chat_nur_live_und_mit_spamschutz() {
    let srv = start_server("", 4, 0.0);
    let mut a = Client::connect(&srv.addr, opts("Alice", 0.0)).expect("Alice verbindet");
    // Nachricht, bevor Bob da ist: Bob darf sie nie sehen (kein Verlauf)
    a.send(&ClientMsg::Chat { text: "vorher".into() });
    wait_for(&mut a, 5, |m| matches!(m, ServerMsg::Chat { text, .. } if text == "vorher").then_some(()));
    let mut b = Client::connect(&srv.addr, opts("Bob", 0.0)).expect("Bob verbindet");
    // Alice bekommt den Hinweis "Bob ist da"
    wait_pump(&mut a, Some(&mut b), 5, |m| matches!(m, ServerMsg::Chat { from_id: 0, text, .. } if text.contains("Bob")).then_some(()));
    a.send(&ClientMsg::Chat { text: "  hallo\n  Bob  ".into() });
    let (from, text) = wait_pump(&mut b, Some(&mut a), 5, |m| match m {
        ServerMsg::Chat { from_id, from, text } if *from_id != 0 => Some((from.clone(), text.clone())),
        _ => None,
    });
    assert_eq!((from.as_str(), text.as_str()), ("Alice", "hallo Bob"), "bereinigt und mit Absender");
    // Spam: nach CHAT_BURST Nachrichten kommt der Hinweis an den Absender
    for i in 0..8 { b.send(&ClientMsg::Chat { text: format!("spam {i}") }); }
    wait_pump(&mut b, Some(&mut a), 5, |m| matches!(m, ServerMsg::Chat { from_id: 0, text, .. } if text.contains("schnell")).then_some(()));
    // Bob hat "vorher" nie bekommen
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(300) {
        a.pump();
        for m in b.poll() { assert!(!matches!(m, ServerMsg::Chat { ref text, .. } if text == "vorher"), "kein Verlauf für Neue"); }
        std::thread::sleep(Duration::from_millis(3));
    }
}
