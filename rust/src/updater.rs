//! Eingebauter Updater (wie bei Vidline/Clipline): sucht auf GitHub nach einer neueren Version,
//! lädt das passende Paket und installiert es nach dem Beenden über ein kleines Hilfsskript:
//! - Windows: Setup (still, gleicher Ordner und Modus wie die bestehende Installation) oder ZIP (portabel)
//! - macOS: .app aus der passenden DMG ersetzen
//! - Linux: .deb (apt) / .pkg.tar.zst (pacman) über pkexec oder tar.gz (portabel)
//! Danach startet das Spiel neu. Ersetzt den alten separaten JumpNDodgeUpdater.
//!
//! Test-Hilfen: UPDATER_FAKE_VERSION=4.0.0 (ältere Version vortäuschen),
//! UPDATER_DRY=1 (Hilfsskript nur schreiben, nicht starten), UPDATER_REPO=owner/repo (anderes Repo),
//! UPDATER_AUTO=now (Meldung sofort bestätigen).

use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub const REPO: &str = "erqf1/JumpNDodge";
/// AppId des Windows-Installers (gleich wie beim bisherigen Installer: Update an Ort und Stelle)
pub const INNO_APP_ID: &str = "B44A592F-4DB5-4419-BCA6-7C5F80AA4472";

pub fn current_version() -> String {
    std::env::var("UPDATER_FAKE_VERSION").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

#[derive(Clone, Debug)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub page: String,
    pub notes: String,
    pub assets: Vec<(String, String, u64)>,
}

pub fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+', ' ']).next()?;
    let mut it = core.split('.').map(|p| p.parse::<u64>().ok());
    let a = it.next()??;
    let b = it.next().flatten().unwrap_or(0);
    let c = it.next().flatten().unwrap_or(0);
    Some((a, b, c))
}

pub fn is_newer(remote: &str, local: &str) -> bool {
    match (parse_version(remote), parse_version(local)) { (Some(r), Some(l)) => r > l, _ => false }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().try_proxy_from_env(true).timeout_connect(Duration::from_secs(10)).timeout_read(Duration::from_secs(30)).build()
}

/// Neueste Version auf GitHub (blockierend - im Hintergrund-Thread aufrufen)
pub fn fetch_latest() -> Result<Release, String> {
    let repo = std::env::var("UPDATER_REPO").ok().filter(|r| !r.is_empty()).unwrap_or_else(|| REPO.to_string());
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let r = agent().get(&url)
        .set("User-Agent", &format!("JumpNDodge/{}", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .call().map_err(|e| format!("{}", e))?;
    let v: Value = r.into_json().map_err(|e| e.to_string())?;
    let tag = v.get("tag_name").and_then(|t| t.as_str()).unwrap_or("").to_string();
    if parse_version(&tag).is_none() { return Err(format!("Keine Spielversion: '{}'", tag)); }
    let assets = v.get("assets").and_then(|a| a.as_array()).map(|a| a.iter().filter_map(|x| {
        Some((x.get("name")?.as_str()?.to_string(), x.get("browser_download_url")?.as_str()?.to_string(), x.get("size").and_then(|s| s.as_u64()).unwrap_or(0)))
    }).collect()).unwrap_or_default();
    Ok(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        tag,
        page: v.get("html_url").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        notes: v.get("body").and_then(|t| t.as_str()).unwrap_or("").to_string(),
        assets,
    })
}

fn exe_path() -> PathBuf { plain_path(&std::env::current_exe().ok().and_then(|p| p.canonicalize().ok().or(Some(p))).unwrap_or_default()) }
fn exe_dir() -> PathBuf { exe_path().parent().map(|p| p.to_path_buf()).unwrap_or_default() }

fn which(cmd: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file())).unwrap_or(false)
}

/// Passendes Paket für dieses System und diese Installationsart: (Name, URL, Größe)
pub fn pick_asset(rel: &Release) -> Option<(String, String, u64)> {
    let suffix: String = if cfg!(windows) {
        if exe_dir().join("unins000.exe").is_file() { "-windows-x64-setup.exe".into() } else { "-windows-x64.zip".into() }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") { "-macos-apple-silicon.dmg".into() } else { "-macos-intel.dmg".into() }
    } else {
        if !cfg!(target_arch = "x86_64") { return None; }
        let exe = exe_path();
        if exe.starts_with("/usr") {
            let deb = which("dpkg") && std::process::Command::new("dpkg").arg("-S").arg(&exe).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
            if !which("pkexec") { return None; }
            if deb { "_amd64.deb".into() } else if which("pacman") { "-x86_64.pkg.tar.zst".into() } else { return None; }
        } else {
            "-linux-x86_64.tar.gz".into()
        }
    };
    let suffix = std::env::var("UPDATER_ASSET_SUFFIX").ok().filter(|s| !s.is_empty()).unwrap_or(suffix);
    // Genau der Spiel-Dateiname ("jumpndodge" + Endung), nicht nur die Endung: bis 5.3.0 endete auch das
    // Server-Zip auf "-windows-x64.zip" und wurde von portablen Windows-Installationen statt des Spiels geladen.
    let want = format!("jumpndodge{}", suffix.to_lowercase());
    rel.assets.iter().find(|(n, _, _)| n.to_lowercase() == want).cloned()
}

/// `\\?\C:\...` (von canonicalize) -> `C:\...`, damit PowerShell und Installer normale Pfade bekommen
fn plain_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") { return PathBuf::from(format!(r"\\{}", rest)); }
    if let Some(rest) = s.strip_prefix(r"\\?\") { return PathBuf::from(rest); }
    p.to_path_buf()
}

/// Download mit Fortschritt (blockierend). `cancel` bricht ab.
pub fn download(url: &str, dest: &Path, got: Arc<AtomicU64>, total: Arc<AtomicU64>, cancel: Arc<AtomicBool>) -> Result<(), String> {
    let r = agent().get(url).set("User-Agent", &format!("JumpNDodge/{}", env!("CARGO_PKG_VERSION"))).call().map_err(|e| e.to_string())?;
    if let Some(len) = r.header("Content-Length").and_then(|l| l.parse::<u64>().ok()) { total.store(len, Ordering::Relaxed); }
    if let Some(d) = dest.parent() { std::fs::create_dir_all(d).map_err(|e| e.to_string())?; }
    let mut f = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut rd = r.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) { drop(f); let _ = std::fs::remove_file(dest); return Err("abgebrochen".into()); }
        let n = rd.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 { break; }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        got.fetch_add(n as u64, Ordering::Relaxed);
    }
    f.flush().map_err(|e| e.to_string())?;
    let size = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    if size < 1024 { let _ = std::fs::remove_file(dest); return Err("Download unvollständig".into()); }
    Ok(())
}

pub fn temp_dir() -> PathBuf { std::env::temp_dir().join("jumpndodge-update") }

fn sq(s: &str) -> String { format!("'{}'", s.replace('\'', "'\\''")) }
#[allow(dead_code)]
fn pq(s: &str) -> String { format!("'{}'", s.replace('\'', "''")) }

#[cfg(windows)]
fn install_scope(app_dir: &Path) -> &'static str {
    // Inno trägt sich unter HKLM (alle Benutzer) oder HKCU (nur ich) ein - mit dem falschen
    // Modus fände das Setup die alte Installation nicht.
    let key = format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{{{}}}_is1", INNO_APP_ID);
    let q = |root: &str, extra: &[&str]| -> bool {
        let mut c = std::process::Command::new("reg");
        c.arg("query").arg(format!(r"{}\{}", root, key)).arg("/v").arg("UninstallString");
        for e in extra { c.arg(e); }
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
    };
    if q("HKLM", &["/reg:64"]) || q("HKLM", &["/reg:32"]) { return "/ALLUSERS"; }
    if q("HKCU", &[]) { return "/CURRENTUSER"; }
    let t = app_dir.join(".update-write-test");
    if std::fs::write(&t, b"x").is_ok() { let _ = std::fs::remove_file(&t); "/CURRENTUSER" } else { "/ALLUSERS" }
}

/// Schreibt das Hilfsskript (wartet auf das Ende des Spiels, installiert, startet neu) und startet es.
/// Gibt den Pfad des Skripts zurück. Danach muss sich das Spiel beenden.
pub fn launch_installer(file: &Path) -> Result<PathBuf, String> {
    // Test-Hilfen nicht an das neu gestartete Spiel vererben (sonst Update-Schleife)
    for v in ["UPDATER_FAKE_VERSION", "UPDATER_AUTO", "UPDATER_REPO", "UPDATER_ASSET_SUFFIX"] { std::env::remove_var(v); }
    let pid = std::process::id();
    let exe = exe_path();
    let app_dir = exe_dir();
    let tmp = file.parent().map(|p| p.to_path_buf()).unwrap_or_else(temp_dir);
    let dry = std::env::var("UPDATER_DRY").map(|v| v == "1").unwrap_or(false);
    let f = file.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        let mut s: Vec<String> = vec![
            "$ErrorActionPreference = 'SilentlyContinue'".into(),
            format!("Wait-Process -Id {} -Timeout 20", pid),
            format!("if (Get-Process -Id {0}) {{ Stop-Process -Id {0} -Force; Start-Sleep -Seconds 1 }}", pid),
        ];
        let log = pq(&tmp.join("update.log").to_string_lossy());
        if f.to_lowercase().ends_with(".exe") {
            let scope = install_scope(&app_dir);
            s.push(format!("$p = Start-Process -Wait -PassThru -FilePath {} -ArgumentList '/SILENT','/SUPPRESSMSGBOXES','/NORESTART','/CLOSEAPPLICATIONS','/FORCECLOSEAPPLICATIONS','{}',{}",
                pq(&f), scope, pq(&format!("/DIR=\"{}\"", app_dir.to_string_lossy()))));
            s.push(format!("\"setup {} exit=$($p.ExitCode)\" | Out-File -LiteralPath {} -Encoding utf8", scope, log));
        } else {
            let out = pq(&tmp.join("unpacked").to_string_lossy());
            s.push(format!("Remove-Item -LiteralPath {} -Recurse -Force", out));
            s.push(format!("Expand-Archive -LiteralPath {} -DestinationPath {} -Force", pq(&f), out));
            s.push(format!("$src = Get-ChildItem -LiteralPath {} -Directory | Select-Object -First 1", out));
            s.push(format!("Copy-Item -Path (Join-Path $src.FullName '*') -Destination {} -Recurse -Force", pq(&app_dir.to_string_lossy())));
        }
        s.push(format!("Start-Process -FilePath {}", pq(&exe.to_string_lossy())));
        let script = tmp.join("update.ps1");
        let mut body = vec![0xEF, 0xBB, 0xBF];
        body.extend_from_slice(s.join("\r\n").as_bytes());
        std::fs::write(&script, body).map_err(|e| e.to_string())?;
        if !dry {
            use std::os::windows::process::CommandExt;
            std::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"]).arg(&script)
                .creation_flags(0x0000_0008 | 0x0000_0200) // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
                .spawn().map_err(|e| e.to_string())?;
        }
        return Ok(script);
    }
    #[cfg(not(windows))]
    {
        let mut s: Vec<String> = vec!["#!/bin/sh".into(), format!("while kill -0 {} 2>/dev/null; do sleep 0.3; done", pid)];
        if cfg!(target_os = "macos") {
            // .../JumpNDodge.app/Contents/MacOS -> .../JumpNDodge.app
            let app = app_dir.parent().and_then(|p| p.parent()).map(|p| p.to_path_buf()).unwrap_or_else(|| app_dir.clone());
            let app = app.to_string_lossy().to_string();
            s.extend([
                format!("APP={}", sq(&app)), format!("DMG={}", sq(&f)), "MNT=$(mktemp -d /tmp/update.XXXXXX)".into(),
                "if hdiutil attach -nobrowse -quiet -mountpoint \"$MNT\" \"$DMG\"; then".into(),
                "  SRC=$(ls -d \"$MNT\"/*.app | head -n 1)".into(),
                "  rm -rf \"$APP.old\"".into(),
                "  if mv \"$APP\" \"$APP.old\" && ditto \"$SRC\" \"$APP\"; then".into(),
                "    rm -rf \"$APP.old\"; hdiutil detach -quiet \"$MNT\"".into(),
                "    xattr -dr com.apple.quarantine \"$APP\" 2>/dev/null".into(),
                "  else".into(),
                "    [ -d \"$APP\" ] || mv \"$APP.old\" \"$APP\"; hdiutil detach -quiet \"$MNT\"; open \"$DMG\"; exit 0".into(),
                "  fi".into(),
                "else open \"$DMG\"; exit 0; fi".into(),
                "open \"$APP\"".into(),
            ]);
        } else {
            if f.ends_with(".deb") { s.push(format!("pkexec apt-get install -y --reinstall {}", sq(&f))); }
            else if f.ends_with(".zst") { s.push(format!("pkexec pacman -U --noconfirm {}", sq(&f))); }
            else { s.push(format!("tar xzf {} --strip-components=1 -C {}", sq(&f), sq(&app_dir.to_string_lossy()))); }
            s.push(format!("nohup {} >/dev/null 2>&1 &", sq(&exe.to_string_lossy())));
        }
        let script = tmp.join("update.sh");
        std::fs::write(&script, s.join("\n") + "\n").map_err(|e| e.to_string())?;
        if !dry {
            std::process::Command::new("/bin/sh").arg(&script)
                .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                .spawn().map_err(|e| e.to_string())?;
        }
        Ok(script)
    }
}

/// Download-Seite im Browser öffnen (wenn automatisch nicht geht)
pub fn open_url(url: &str) {
    let _ = if cfg!(windows) {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versionen() {
        assert!(is_newer("v5.0.1", "5.0.0"));
        assert!(is_newer("5.1", "5.0.9"));
        assert!(!is_newer("v4.9.1", "5.0.0"));
        assert!(!is_newer("installer", "5.0.0"));
        assert!(!is_newer("rust-v4.9.1", "4.0.0"));
        assert_eq!(parse_version("v5.0.0-beta"), Some((5, 0, 0)));
    }
    #[test]
    fn nimmt_nie_das_server_zip() {
        let a = |n: &str| (n.to_string(), format!("https://x/{n}"), 1u64);
        let rel = Release { tag: "v9.0.0".into(), version: "9.0.0".into(), page: String::new(), notes: String::new(), assets: vec![
            a("JumpNDodge-Server-windows-x64.zip"), a("JumpNDodge-Server-win-x64.zip"), a("JumpNDodge-Server-linux-x86_64.zip"),
            a("JumpNDodge-Server-macos-apple-silicon.zip"), a("JumpNDodge-windows-x64-setup.exe"), a("JumpNDodge-windows-x64.zip"),
            a("jumpndodge_amd64.deb"), a("jumpndodge-x86_64.pkg.tar.zst"), a("jumpndodge-linux-x86_64.tar.gz"),
            a("JumpNDodge-macos-apple-silicon.dmg"), a("JumpNDodge-macos-intel.dmg"),
        ] };
        for suf in ["-windows-x64.zip", "-windows-x64-setup.exe", "_amd64.deb", "-x86_64.pkg.tar.zst", "-linux-x86_64.tar.gz", "-macos-apple-silicon.dmg", "-macos-intel.dmg"] {
            std::env::set_var("UPDATER_ASSET_SUFFIX", suf);
            let got = pick_asset(&rel).map(|x| x.0).unwrap_or_default();
            assert!(!got.to_lowercase().contains("server") && got.to_lowercase().ends_with(suf), "{suf} -> {got}");
        }
        std::env::remove_var("UPDATER_ASSET_SUFFIX");
    }
    #[test]
    fn verbatim_pfade() {
        assert_eq!(plain_path(Path::new(r"\\?\C:\Users\x\JumpNDodge.exe")), PathBuf::from(r"C:\Users\x\JumpNDodge.exe"));
        assert_eq!(plain_path(Path::new(r"\\?\UNC\srv\share\a")), PathBuf::from(r"\\srv\share\a"));
        assert_eq!(plain_path(Path::new(r"C:\a")), PathBuf::from(r"C:\a"));
    }
}
