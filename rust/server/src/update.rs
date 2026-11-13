//! Auto-Update des Servers: schaut regelmäßig ins neueste GitHub-Release, lädt die passende Server-Zip,
//! prüft sie gegen SHA256SUMS.txt und meldet "bereit". Neu gestartet wird erst, wenn niemand verbunden ist
//! (siehe Server::poll_once). Ältere/neuere Spiele bleiben kompatibel, solange PROTO_VERSION gleich ist.
//!
//! - Docker (JND_SERVER_DOCKER=1): neue Datei nach /data/bin, Prozess endet, Docker startet neu,
//!   `docker-entrypoint.sh` nimmt die neuere von beiden (Image oder /data/bin).
//! - Normal (Windows/Linux/macOS): Programmdatei wird ersetzt und mit denselben Argumenten neu gestartet.

use jnd_core::net::proto::GAME_VERSION;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::time::{Duration, Instant};

pub const REPO: &str = "erqf1/JumpNDodge";

#[derive(Debug, Clone)]
pub struct Ready { pub version: String, pub binary: PathBuf }

pub struct Updater {
    every: Duration,
    last: Option<Instant>,
    rx: Option<Receiver<Result<Option<Ready>, String>>>,
    ready: Option<Ready>,
}

impl Updater {
    pub fn new(minutes: u32) -> Updater {
        Updater { every: Duration::from_secs(60 * minutes.max(5) as u64), last: None, rx: None, ready: None }
    }

    /// Jeden Durchgang aufrufen; liefert Log-Zeilen. Sucht beim Start nach 20 s und dann im Takt.
    pub fn poll(&mut self) -> Vec<String> {
        let mut out = vec![];
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Ok(Some(r))) => { out.push(format!("Update {} geladen und geprüft", r.version)); self.ready = Some(r); self.rx = None; }
                Ok(Ok(None)) => self.rx = None,
                Ok(Err(e)) => { out.push(format!("Update-Suche: {e}")); self.rx = None; }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.rx = None,
            }
        }
        let due = match self.last { None => true, Some(t) => t.elapsed() >= self.every };
        if self.rx.is_none() && self.ready.is_none() && due {
            // erste Suche erst kurz nach dem Start (Netz/Container noch nicht ganz da)
            if self.last.is_none() { self.last = Some(Instant::now() - self.every + Duration::from_secs(20)); return out; }
            self.last = Some(Instant::now());
            let (tx, rx) = channel();
            std::thread::spawn(move || { let _ = tx.send(check_and_download()); });
            self.rx = Some(rx);
        }
        out
    }

    pub fn take_ready(&mut self) -> Option<Ready> { self.ready.take() }

    /// Jemand mit neuerer Spielversion ist da: gleich nachsehen statt erst zur nächsten Runde
    pub fn check_soon(&mut self) {
        if self.rx.is_some() || self.ready.is_some() { return; }
        if self.last.map(|t| t.elapsed() < Duration::from_secs(120)).unwrap_or(false) { return; }
        self.last = Some(Instant::now() - self.every);
    }
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches('v');
    let mut it = v.split(|c: char| c == '.' || c == '-').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}
pub fn is_newer(remote: &str, local: &str) -> bool {
    matches!((parse_version(remote), parse_version(local)), (Some(r), Some(l)) if r > l)
}

/// Name der Server-Zip für diese Plattform im Release
pub fn asset_name() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) { Some("JumpNDodge-Server-win-x64.zip") }
    else if cfg!(all(target_os = "linux", target_arch = "x86_64")) { Some("JumpNDodge-Server-linux-x86_64.zip") }
    else if cfg!(all(target_os = "macos", target_arch = "aarch64")) { Some("JumpNDodge-Server-macos-apple-silicon.zip") }
    else if cfg!(all(target_os = "macos", target_arch = "x86_64")) { Some("JumpNDodge-Server-macos-intel.zip") }
    else { None }
}

fn bin_name() -> &'static str { if cfg!(windows) { "JumpNDodge-Server.exe" } else { "JumpNDodge-Server" } }

fn in_docker() -> bool { std::env::var("JND_SERVER_DOCKER").map(|v| v == "1").unwrap_or(false) }

/// Arbeitsordner für Updates (Docker: /data, sonst neben dem Programm)
fn work_dir() -> PathBuf {
    if let Ok(d) = std::env::var("JND_SERVER_DATA") { if !d.trim().is_empty() { return PathBuf::from(d); } }
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_else(|| PathBuf::from("."))
}

fn agent() -> ureq::Agent { ureq::AgentBuilder::new().timeout(Duration::from_secs(60)).user_agent(&format!("JumpNDodge-Server/{GAME_VERSION}")).build() }

fn get_bytes(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let r = agent().get(url).call().map_err(|e| e.to_string())?;
    let mut v = vec![];
    r.into_reader().take(limit).read_to_end(&mut v).map_err(|e| e.to_string())?;
    Ok(v)
}

fn check_and_download() -> Result<Option<Ready>, String> {
    let Some(asset) = asset_name() else { return Ok(None); };
    let repo = std::env::var("JND_SERVER_UPDATE_REPO").ok().filter(|r| !r.is_empty()).unwrap_or_else(|| REPO.into());
    let rel: serde_json::Value = agent().get(&format!("https://api.github.com/repos/{repo}/releases/latest"))
        .call().map_err(|e| e.to_string())?.into_json().map_err(|e| e.to_string())?;
    let tag = rel["tag_name"].as_str().unwrap_or("").to_string();
    let ver = tag.trim_start_matches('v').to_string();
    if !is_newer(&ver, GAME_VERSION) { return Ok(None); }
    let assets = rel["assets"].as_array().cloned().unwrap_or_default();
    let url_of = |n: &str| assets.iter().find(|a| a["name"].as_str() == Some(n)).and_then(|a| a["browser_download_url"].as_str().map(String::from));
    let zip_url = url_of(asset).ok_or_else(|| format!("{tag}: keine Datei {asset}"))?;
    let sums_url = url_of("SHA256SUMS.txt").ok_or_else(|| format!("{tag}: keine SHA256SUMS.txt - Update ausgelassen"))?;
    let sums = String::from_utf8_lossy(&get_bytes(&sums_url, 1 << 20)?).to_string();
    let want = sums.lines().find_map(|l| {
        let mut p = l.split_whitespace();
        let (h, n) = (p.next()?, p.next()?);
        (n.trim_start_matches('*') == asset).then(|| h.to_lowercase())
    }).ok_or_else(|| format!("{asset} fehlt in SHA256SUMS.txt"))?;
    let data = get_bytes(&zip_url, 200 << 20)?;
    let got = hex(&Sha256::digest(&data));
    if got != want { return Err(format!("Prüfsumme passt nicht ({asset}) - Update verworfen")); }
    // Programmdatei aus der Zip holen
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).map_err(|e| e.to_string())?;
    let mut bin = None;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = f.name().replace('\\', "/");
        if name.rsplit('/').next() == Some(bin_name()) {
            let mut v = vec![];
            f.read_to_end(&mut v).map_err(|e| e.to_string())?;
            bin = Some(v);
            break;
        }
    }
    let bin = bin.ok_or("Programmdatei nicht in der Zip gefunden")?;
    let dir = work_dir().join("update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.neu", bin_name()));
    std::fs::write(&path, bin).map_err(|e| e.to_string())?;
    make_executable(&path);
    Ok(Some(Ready { version: ver, binary: path }))
}

fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

#[cfg(unix)]
fn make_executable(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755));
}
#[cfg(not(unix))]
fn make_executable(_p: &Path) {}

/// Update einspielen. Docker: Datei nach /data/bin, dann soll der Prozess enden (Docker startet neu).
/// Sonst: Programmdatei ersetzen und mit denselben Argumenten neu starten. Danach muss main() beenden.
pub fn apply(r: &Ready, args: &[String]) -> Result<(), String> {
    if in_docker() {
        let dir = work_dir().join("bin");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::rename(&r.binary, dir.join(bin_name())).map_err(|e| e.to_string())?;
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if cfg!(windows) {
        // laufende .exe darf umbenannt, aber nicht überschrieben werden
        let old = exe.with_extension("exe.alt");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(&exe, &old).map_err(|e| e.to_string())?;
        if let Err(e) = std::fs::rename(&r.binary, &exe) { let _ = std::fs::rename(&old, &exe); return Err(e.to_string()); }
    } else {
        std::fs::rename(&r.binary, &exe).map_err(|e| e.to_string())?;
    }
    std::process::Command::new(&exe).args(args.iter().skip(1)).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versionen() {
        assert!(is_newer("v5.4.1", "5.4.0"));
        assert!(is_newer("6.0.0", "5.9.9"));
        assert!(!is_newer("5.4.0", "5.4.0"));
        assert!(!is_newer("v5.3.9", "5.4.0"));
        assert!(!is_newer("kaputt", "5.4.0"));
    }
    #[test]
    fn plattform_datei() {
        if cfg!(all(windows, target_arch = "x86_64")) { assert_eq!(asset_name(), Some("JumpNDodge-Server-win-x64.zip")); }
        assert!(asset_name().map(|n| n.starts_with("JumpNDodge-Server-") && n.ends_with(".zip")).unwrap_or(true));
    }
}
