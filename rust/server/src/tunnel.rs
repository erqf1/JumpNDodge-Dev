//! Eingebauter Cloudflare Tunnel: Ist ein Token gesetzt (server.json "tunnel_token" oder TUNNEL_TOKEN),
//! startet der Server cloudflared selbst und hält es am Laufen. Fehlt cloudflared, wird es einmal von
//! GitHub (cloudflare/cloudflared, offizielles Release) nach <Datenordner>/bin geladen.
//! Im Cloudflare-Dashboard zeigt der Hostname dann auf HTTP -> localhost:<ws_port>.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Log-Funktion, die aus Threads benutzt werden kann
pub type SendLog = Arc<dyn Fn(&str) + Send + Sync>;

/// Name der cloudflared-Datei im offiziellen Release für dieses System
pub fn asset_name() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) { Some("cloudflared-windows-amd64.exe") }
    else if cfg!(all(target_os = "linux", target_arch = "x86_64")) { Some("cloudflared-linux-amd64") }
    else if cfg!(all(target_os = "linux", target_arch = "aarch64")) { Some("cloudflared-linux-arm64") }
    else if cfg!(all(target_os = "linux", target_arch = "arm")) { Some("cloudflared-linux-arm") }
    else { None }
}

fn exe_name() -> &'static str { if cfg!(windows) { "cloudflared.exe" } else { "cloudflared" } }

fn works(p: &Path) -> bool {
    let mut c = Command::new(p);
    c.arg("--version").stdout(Stdio::null()).stderr(Stdio::null());
    no_window(&mut c);
    c.status().map(|s| s.success()).unwrap_or(false)
}

#[cfg(windows)]
fn no_window(c: &mut Command) { use std::os::windows::process::CommandExt; c.creation_flags(0x0800_0000); }
#[cfg(not(windows))]
fn no_window(_c: &mut Command) {}

/// cloudflared finden (JND_CLOUDFLARED, Datenordner, PATH, /usr/local/bin) oder laden
fn find_or_download(bin_dir: &Path, log: &SendLog) -> Option<PathBuf> {
    if let Ok(p) = std::env::var("JND_CLOUDFLARED") { let p = PathBuf::from(p); if p.exists() { return Some(p); } }
    let local = bin_dir.join(exe_name());
    for p in [local.clone(), PathBuf::from("/usr/local/bin/cloudflared"), PathBuf::from(exe_name())] {
        if works(&p) { return Some(p); }
    }
    let Some(asset) = asset_name() else {
        log("Tunnel: cloudflared fehlt - auf diesem System bitte selbst installieren (macOS: brew install cloudflared)");
        return None;
    };
    log(&format!("Tunnel: cloudflared fehlt - lade {} von GitHub (cloudflare/cloudflared) …", asset));
    let url = format!("https://github.com/cloudflare/cloudflared/releases/latest/download/{}", asset);
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout(Duration::from_secs(600)).build();
    let resp = match agent.get(&url).call() { Ok(r) => r, Err(e) => { log(&format!("Tunnel: Download fehlgeschlagen: {}", e)); return None; } };
    let _ = std::fs::create_dir_all(bin_dir);
    let tmp = bin_dir.join(format!("{}.part", exe_name()));
    let ok = std::fs::File::create(&tmp).and_then(|mut f| std::io::copy(&mut resp.into_reader().take(300 * 1024 * 1024), &mut f)).map(|n| n > 1_000_000).unwrap_or(false);
    if !ok { let _ = std::fs::remove_file(&tmp); log("Tunnel: Download unvollständig"); return None; }
    #[cfg(unix)]
    { use std::os::unix::fs::PermissionsExt; let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)); }
    if std::fs::rename(&tmp, &local).is_err() { log("Tunnel: cloudflared konnte nicht gespeichert werden"); return None; }
    if !works(&local) { log("Tunnel: geladenes cloudflared startet nicht"); return None; }
    log(&format!("Tunnel: cloudflared gespeichert in {}", local.display()));
    Some(local)
}

use std::io::Read;

/// Wichtige Zeilen von cloudflared (Verbindung steht, Fehler, Warnungen) ins Server-Log
fn interesting(l: &str) -> Option<String> {
    let keep = l.contains("Registered tunnel connection") || l.contains(" ERR ") || l.contains(" WRN ") || l.contains("error") || l.contains("Unauthorized");
    if !keep { return None; }
    // Zeitstempel von cloudflared weglassen (das Server-Log hat einen eigenen)
    let t = match l.split_once(' ') { Some((first, rest)) if first.contains('T') && first.contains(':') => rest, _ => l };
    Some(format!("[Tunnel] {}", t.trim()))
}

pub struct Tunnel { stop: Arc<AtomicBool>, child: Arc<Mutex<Option<Child>>> }

impl Tunnel {
    /// Startet cloudflared im Hintergrund (und immer wieder neu, falls es abbricht)
    pub fn start(token: String, bin_dir: PathBuf, log: SendLog) -> Tunnel {
        let stop = Arc::new(AtomicBool::new(false));
        let child: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let (st, ch) = (stop.clone(), child.clone());
        std::thread::spawn(move || {
            let Some(bin) = find_or_download(&bin_dir, &log) else { return; };
            let mut first = true;
            while !st.load(Ordering::Relaxed) {
                let mut cmd = Command::new(&bin);
                cmd.args(["tunnel", "--no-autoupdate", "run"]).env("TUNNEL_TOKEN", &token).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
                no_window(&mut cmd);
                match cmd.spawn() {
                    Ok(mut c) => {
                        if first { log("Tunnel: cloudflared läuft - verbindet mit Cloudflare …"); first = false; }
                        let out = c.stdout.take();
                        let err = c.stderr.take();
                        *ch.lock().unwrap() = Some(c);
                        let l2 = log.clone();
                        let t_out = out.map(|o| std::thread::spawn(move || for l in BufReader::new(o).lines().map_while(Result::ok) { if let Some(x) = interesting(&l) { l2(&x); } }));
                        if let Some(e) = err { for l in BufReader::new(e).lines().map_while(Result::ok) { if let Some(x) = interesting(&l) { log(&x); } } }
                        if let Some(t) = t_out { let _ = t.join(); }
                        if let Some(mut c) = ch.lock().unwrap().take() { let _ = c.wait(); }
                    }
                    Err(e) => log(&format!("Tunnel: cloudflared startet nicht: {}", e)),
                }
                if st.load(Ordering::Relaxed) { break; }
                log("[Tunnel] beendet - neuer Versuch in 15 s (Token richtig?)");
                for _ in 0..15 { if st.load(Ordering::Relaxed) { break; } std::thread::sleep(Duration::from_secs(1)); }
            }
        });
        Tunnel { stop, child }
    }

    /// cloudflared beenden (z.B. vor einem Update-Neustart)
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(mut c) = self.child.lock().unwrap().take() { let _ = c.kill(); let _ = c.wait(); }
    }
}

impl Drop for Tunnel { fn drop(&mut self) { self.stop(); } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zeilen_filtern() {
        assert_eq!(interesting("2026-11-15T10:00:00Z INF Registered tunnel connection connIndex=0").as_deref(), Some("[Tunnel] INF Registered tunnel connection connIndex=0"));
        assert!(interesting("2026-11-15T10:00:00Z INF Starting metrics server").is_none());
        assert!(interesting("2026-11-15T10:00:00Z ERR Provided Tunnel token is not valid.").is_some());
    }
}
