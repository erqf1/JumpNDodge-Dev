//! Replays online (über den offiziellen Spiel-Server, HTTP auf seinem WebSocket-Port):
//! eigene Bestläufe hochladen, Bestläufe von Freunden als Geister holen, Replays per Code teilen.

use super::replay::Recording;
use super::state::G;
use crate::util::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

thread_local! {
    /// Bestläufe von Freunden je Schwierigkeit: (Name, Replay)
    static FRIEND_GHOSTS: RefCell<HashMap<String, Vec<(String, Recording)>>> = RefCell::new(HashMap::new());
    static FRIEND_AT: RefCell<HashMap<String, Instant>> = RefCell::new(HashMap::new());
    static FRIEND_RX: RefCell<Vec<Receiver<(String, Vec<(String, Vec<u8>)>)>>> = const { RefCell::new(Vec::new()) };
}

/// Für URLs: alles außer a-z, 0-9, -_. prozent-kodieren
pub fn url_enc(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{:02X}", b) }).collect()
}

/// Spieler-Adresse des Servers -> Basis-URL für Replays
pub fn replay_url_for(addr: &str) -> Option<String> {
    let a = addr.trim().trim_end_matches('/');
    if a.is_empty() { return None; }
    if let Some(r) = a.strip_prefix("wss://") { return Some(format!("https://{}", r)); }
    if let Some(r) = a.strip_prefix("ws://") { return Some(format!("http://{}", r)); }
    if a.starts_with("http://") || a.starts_with("https://") { return Some(a.to_string()); }
    // IP oder host:port (UDP) -> WebSocket-Port 47801 per http; reiner Hostname -> https (Tunnel)
    let host = match a.rsplit_once(':') { Some((h, p)) if p.parse::<u16>().is_ok() && !h.contains(':') => h, _ => a };
    let is_ip = host.trim_matches(['[', ']']).parse::<std::net::IpAddr>().is_ok();
    if is_ip || a.contains(':') || !host.contains('.') { Some(format!("http://{}:47801", host)) } else { Some(format!("https://{}", host)) }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(6)).timeout(Duration::from_secs(20)).build()
}

fn http_get(url: &str) -> Result<Vec<u8>, String> {
    let r = agent().get(url).call().map_err(|e| match e { ureq::Error::Status(404, _) => "nicht gefunden".to_string(), e => e.to_string() })?;
    let mut b = vec![];
    std::io::Read::read_to_end(&mut r.into_reader().take_limit(), &mut b).map_err(|e| e.to_string())?;
    Ok(b)
}
trait TakeLimit: std::io::Read + Sized { fn take_limit(self) -> std::io::Take<Self> { self.take(8 * 1024 * 1024) } }
impl<R: std::io::Read> TakeLimit for R {}

fn http_post(url: &str, body: &[u8]) -> Result<String, String> {
    match agent().post(url).set("Content-Type", "application/octet-stream").send_bytes(body) {
        Ok(r) => r.into_string().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(c, r)) => Err(format!("{} {}", c, r.into_string().unwrap_or_default())),
        Err(e) => Err(e.to_string()),
    }
}

impl G {
    /// Basis-URL des Replay-Servers (offizieller Server; Test: JND_REPLAY_URL)
    pub fn replay_base(&self) -> Option<String> {
        if let Ok(u) = std::env::var("JND_REPLAY_URL") { return Some(u.trim_end_matches('/').to_string()); }
        let s = self.settings.gs("mp_official_server", "");
        replay_url_for(if s.trim().is_empty() { super::multiplayer::OFFICIAL_SERVER } else { s.trim() })
    }

    /// Geheimer Schlüssel dieses Spiels (der Name auf dem Server gehört dem ersten Schlüssel)
    fn replay_key(&mut self) -> String {
        let k = self.settings.gs("replay_key", "");
        if k.len() >= 16 { return k; }
        let k: String = (0..32).map(|_| format!("{:x}", rand::random::<u8>() % 16)).collect();
        self.settings.insert("replay_key".into(), serde_json::json!(k));
        self.save_section("settings", None);
        k
    }

    /// Neuen Bestlauf hochladen (für die Geister der Freunde), im Hintergrund
    pub fn upload_best_replay(&mut self, rec: &Recording) {
        if !self.settings.gb("ghost_share", true) { return; }
        let Some(user) = self.current_profile_username.clone() else { return; };
        let Some(base) = self.replay_base() else { return; };
        let key = self.replay_key();
        let url = format!("{}/replays/best?user={}&diff={}&key={}", base, url_enc(&user.to_lowercase()), url_enc(&rec.difficulty.to_lowercase()), key);
        let data = rec.encode();
        std::thread::spawn(move || {
            match http_post(&url, &data) { Ok(t) => println!("[Replay] Bestlauf hochgeladen: {}", t), Err(e) => println!("[Replay] Hochladen fehlgeschlagen: {}", e) }
        });
    }

    /// Bestläufe der Freunde für eine Schwierigkeit im Hintergrund holen (höchstens alle 5 Minuten)
    pub fn prefetch_friend_ghosts(&mut self, diff: &str) {
        self.poll_friend_ghosts();
        if !self.settings.gb("friend_ghosts", true) || diff == "Custom" { return; }
        let Some(me) = self.current_profile_username.clone() else { return; };
        let fresh = FRIEND_AT.with(|m| m.borrow().get(diff).map(|t| t.elapsed() < Duration::from_secs(300)).unwrap_or(false));
        if fresh { return; }
        FRIEND_AT.with(|m| { m.borrow_mut().insert(diff.to_string(), Instant::now()); });
        let d = diff.to_string();
        self.fetch_friends_list(&me, Some(Box::new(move |g, r| {
            let Some(r) = r else { return; };
            if !r.truthy("ok") { return; }
            let friends: Vec<(String, String)> = r.garr("friends").iter().map(|f| (f.gs("username", ""), f.gs("name", ""))).filter(|(u, _)| !u.is_empty()).take(12).collect();
            if friends.is_empty() { return; }
            let Some(base) = g.replay_base() else { return; };
            let (tx, rx) = channel();
            FRIEND_RX.with(|v| v.borrow_mut().push(rx));
            std::thread::spawn(move || {
                let mut got = vec![];
                for (u, n) in friends {
                    let url = format!("{}/replays/best?user={}&diff={}", base, url_enc(&u.to_lowercase()), url_enc(&d.to_lowercase()));
                    if let Ok(b) = http_get(&url) { got.push((if n.is_empty() { u } else { n }, b)); }
                }
                let _ = tx.send((d, got));
            });
        })));
    }

    fn poll_friend_ghosts(&mut self) {
        FRIEND_RX.with(|v| {
            v.borrow_mut().retain(|rx| match rx.try_recv() {
                Ok((d, list)) => {
                    let recs: Vec<(String, Recording)> = list.into_iter().filter_map(|(n, b)| Recording::decode(&b).map(|r| (n, r))).collect();
                    FRIEND_GHOSTS.with(|m| { m.borrow_mut().insert(d, recs); });
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => true,
                Err(_) => false,
            });
        });
    }

    /// Die besten (bis zu 3) Freundes-Geister für eine Schwierigkeit
    pub fn friend_ghosts_for(&mut self, diff: &str) -> Vec<(String, Recording)> {
        self.poll_friend_ghosts();
        if !self.settings.gb("friend_ghosts", true) { return vec![]; }
        let mut l = FRIEND_GHOSTS.with(|m| m.borrow().get(diff).cloned().unwrap_or_default());
        l.sort_by(|a, b| b.1.score.cmp(&a.1.score));
        l.truncate(3);
        l
    }

    /// Test: ein Replay als Freundes-Geist einsetzen
    pub fn dev_fake_friend_ghost(&mut self, name: &str, rec: Recording) {
        FRIEND_GHOSTS.with(|m| m.borrow_mut().entry(rec.difficulty.clone()).or_default().push((name.to_string(), rec)));
        FRIEND_AT.with(|m| { m.borrow_mut().insert(String::new(), Instant::now()); });
    }

    /// Alle geladenen Freundes-Bestläufe (für die Replay-Liste)
    pub fn all_friend_replays(&mut self) -> Vec<(String, Recording)> {
        self.poll_friend_ghosts();
        let mut out = vec![];
        FRIEND_GHOSTS.with(|m| for l in m.borrow().values() { out.extend(l.iter().cloned()); });
        out.sort_by(|a, b| a.1.difficulty.cmp(&b.1.difficulty).then(b.1.score.cmp(&a.1.score)));
        out
    }
}

/// Replay teilen: hochladen und Code holen (blockierend - im Hintergrund aufrufen)
pub fn share_replay(base: &str, rec: &Recording) -> Result<String, String> {
    let code = http_post(&format!("{}/replays/share", base), &rec.encode())?;
    let code = code.trim().to_string();
    if code.len() == 6 { Ok(code) } else { Err(code) }
}

/// Geteiltes Replay per Code laden (blockierend)
pub fn load_shared_replay(base: &str, code: &str) -> Result<Recording, String> {
    let c: String = code.trim().to_uppercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if c.len() != 6 { return Err("Der Code hat 6 Zeichen".into()); }
    let b = http_get(&format!("{}/replays/s/{}", base, c))?;
    Recording::decode(&b).ok_or_else(|| "Replay beschädigt".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adressen() {
        assert_eq!(replay_url_for("play.mert.win").as_deref(), Some("https://play.mert.win"));
        assert_eq!(replay_url_for("wss://a.b").as_deref(), Some("https://a.b"));
        assert_eq!(replay_url_for("ws://127.0.0.1:5000").as_deref(), Some("http://127.0.0.1:5000"));
        assert_eq!(replay_url_for("127.0.0.1:47800").as_deref(), Some("http://127.0.0.1:47801"));
        assert_eq!(replay_url_for("jnd.example.com:47800").as_deref(), Some("http://jnd.example.com:47801"));
        assert_eq!(url_enc("Hölle"), "H%C3%B6lle");
    }
}
