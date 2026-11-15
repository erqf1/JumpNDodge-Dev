//! Replays auf dem Server: Bestläufe pro Spieler und Schwierigkeit (für Freundes-Geister) und
//! geteilte Replays per Code. Läuft über den WebSocket-Port als einfaches HTTP
//! (hinter dem Cloudflare Tunnel: https://<host>/replays/...).
//!
//! GET  /replays/ping
//! POST /replays/best?user=U&diff=D&key=K   (Body: .jnr) - speichert, wenn besser; erster Schlüssel gehört dem Namen
//! GET  /replays/best?user=U&diff=D
//! POST /replays/share                       (Body: .jnr) -> Code
//! GET  /replays/s/CODE

use jnd_core::net::ws::{HttpRequest, HttpResponse};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

const SHARE_MAX_FILES: usize = 5000;
const SHARE_DAYS: u64 = 30;
const CODE_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

/// Kopf eines Replays (JNR1): Schwierigkeit und Punkte; None = kein gültiges Replay
pub fn parse_head(b: &[u8]) -> Option<(String, i64)> {
    if b.len() < 4 || &b[..4] != b"JNR1" { return None; }
    let mut p = 4 + 2 + 2 + 3;
    let dl = *b.get(p)? as usize; p += 1;
    let diff = String::from_utf8_lossy(b.get(p..p + dl)?).to_string(); p += dl;
    let score = i64::from_le_bytes(b.get(p..p + 8)?.try_into().ok()?); p += 8;
    let n = u32::from_le_bytes(b.get(p..p + 4)?.try_into().ok()?);
    if n == 0 || n > 60 * 3600 { return None; }
    Some((diff, score))
}

fn clean(s: &str, max: usize) -> Option<String> {
    let s = s.trim().to_lowercase();
    let ok = !s.is_empty() && s.chars().count() <= max && s.chars().all(|c| c.is_alphanumeric() || "_-.".contains(c));
    ok.then_some(s)
}
fn file_safe(s: &str) -> String { s.chars().map(|c| match c { 'ö' => 'o', 'ä' => 'a', 'ü' => 'u', 'ß' => 's', c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => c, _ => '_' }).collect() }
fn hash(k: &str) -> String { Sha256::digest(k.as_bytes()).iter().map(|b| format!("{:02x}", b)).collect() }

pub struct ReplayStore {
    dir: Mutex<Option<PathBuf>>,
    lock: Mutex<()>,
    shares: Mutex<(Instant, u32, u32)>,
}

impl ReplayStore {
    pub fn new() -> ReplayStore { ReplayStore { dir: Mutex::new(None), lock: Mutex::new(()), shares: Mutex::new((Instant::now(), 0, 0)) } }
    /// Ordner für die Replays (ohne Ordner nimmt der Server keine Replays an)
    pub fn set_dir(&self, d: PathBuf) { *self.dir.lock().unwrap() = Some(d); }

    pub fn handle(&self, r: &HttpRequest) -> HttpResponse {
        let Some(dir) = self.dir.lock().unwrap().clone() else { return HttpResponse::text(404, "Replays sind auf diesem Server aus"); };
        let path = r.path.trim_end_matches('/');
        match (r.method.as_str(), path) {
            ("GET", "/replays/ping") => HttpResponse::text(200, "ok"),
            ("POST", "/replays/best") => self.put_best(&dir, r),
            ("GET", "/replays/best") => {
                let (Some(u), Some(d)) = (r.param("user").and_then(|u| clean(u, 32)), r.param("diff").and_then(|d| clean(d, 32))) else { return HttpResponse::text(400, "user/diff fehlt"); };
                match std::fs::read(dir.join("best").join(file_safe(&u)).join(format!("{}.jnr", file_safe(&d)))) { Ok(b) => HttpResponse::bytes(b), Err(_) => HttpResponse::text(404, "kein Replay") }
            }
            ("POST", "/replays/share") => self.share(&dir, r),
            ("GET", p) if p.starts_with("/replays/s/") => {
                let code = p.trim_start_matches("/replays/s/").to_uppercase();
                if code.len() != 6 || !code.bytes().all(|c| CODE_CHARS.contains(&c)) { return HttpResponse::text(400, "Code ungültig"); }
                match std::fs::read(dir.join("share").join(format!("{}.jnr", code))) { Ok(b) => HttpResponse::bytes(b), Err(_) => HttpResponse::text(404, "Code unbekannt") }
            }
            _ => HttpResponse::text(404, "nicht gefunden"),
        }
    }

    fn put_best(&self, dir: &PathBuf, r: &HttpRequest) -> HttpResponse {
        let (Some(u), Some(d), Some(k)) = (r.param("user").and_then(|u| clean(u, 32)), r.param("diff").and_then(|d| clean(d, 32)), r.param("key")) else { return HttpResponse::text(400, "user/diff/key fehlt"); };
        if k.len() < 16 || k.len() > 128 { return HttpResponse::text(400, "key ungültig"); }
        let Some((rd, score)) = parse_head(&r.body) else { return HttpResponse::text(400, "kein Replay"); };
        if rd.to_lowercase() != d { return HttpResponse::text(400, "Schwierigkeit passt nicht"); }
        let _g = self.lock.lock().unwrap();
        // Name gehört dem ersten Schlüssel
        let owners_p = dir.join("owners.json");
        let mut owners: HashMap<String, String> = std::fs::read_to_string(&owners_p).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        let hk = hash(k);
        match owners.get(&u) {
            Some(h) if *h != hk => return HttpResponse::text(403, "Name gehört einem anderen Spiel"),
            Some(_) => {}
            None => { owners.insert(u.clone(), hk); let _ = std::fs::create_dir_all(dir); let _ = std::fs::write(&owners_p, serde_json::to_string(&owners).unwrap_or_default()); }
        }
        let ud = dir.join("best").join(file_safe(&u));
        let f = ud.join(format!("{}.jnr", file_safe(&d)));
        let old = std::fs::read(&f).ok().and_then(|b| parse_head(&b)).map(|h| h.1).unwrap_or(-1);
        if score <= old { return HttpResponse::text(200, "alt"); }
        let _ = std::fs::create_dir_all(&ud);
        match std::fs::write(&f, &r.body) { Ok(_) => HttpResponse::text(200, "gespeichert"), Err(_) => HttpResponse::text(500, "Speichern fehlgeschlagen") }
    }

    fn share(&self, dir: &PathBuf, r: &HttpRequest) -> HttpResponse {
        if parse_head(&r.body).is_none() { return HttpResponse::text(400, "kein Replay"); }
        {
            let mut s = self.shares.lock().unwrap();
            if s.0.elapsed() > Duration::from_secs(60) { *s = (Instant::now(), 0, s.2); }
            if s.1 >= 30 { return HttpResponse::text(429, "zu viele - kurz warten"); }
            s.1 += 1; s.2 += 1;
            if s.2 % 50 == 1 { self.cleanup(dir); }
        }
        let sd = dir.join("share");
        let _ = std::fs::create_dir_all(&sd);
        let _g = self.lock.lock().unwrap();
        for _ in 0..20 {
            let code: String = (0..6).map(|_| CODE_CHARS[rand::random::<usize>() % CODE_CHARS.len()] as char).collect();
            let f = sd.join(format!("{}.jnr", code));
            if f.exists() { continue; }
            return match std::fs::write(&f, &r.body) { Ok(_) => HttpResponse::text(200, &code), Err(_) => HttpResponse::text(500, "Speichern fehlgeschlagen") };
        }
        HttpResponse::text(500, "kein freier Code")
    }

    /// Alte geteilte Replays weg (älter als 30 Tage, höchstens 5000)
    fn cleanup(&self, dir: &PathBuf) {
        let Ok(rd) = std::fs::read_dir(dir.join("share")) else { return; };
        let mut files: Vec<(SystemTime, PathBuf)> = rd.flatten().filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect();
        let cutoff = SystemTime::now() - Duration::from_secs(SHARE_DAYS * 86400);
        files.sort();
        let extra = files.len().saturating_sub(SHARE_MAX_FILES);
        for (i, (t, p)) in files.iter().enumerate() { if i < extra || *t < cutoff { let _ = std::fs::remove_file(p); } }
    }
}
