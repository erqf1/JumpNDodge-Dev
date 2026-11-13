//! Verschlüsselter Spielstand - dasselbe Verfahren wie in der Python-Version
//! (HMAC-SHA256-Zählermodus + HMAC-Tag, Schlüssel per PBKDF2).
//!
//! Seit 5.3: Der Schlüssel steht nicht mehr im Quellcode. Er wird beim Bauen aus `JND_SAVE_KEY`
//! (GitHub-Secret) verschleiert eingesetzt (`build.rs`) und kommt so mit jedem Download mit - ohne Internet.
//! Der alte Schlüssel (bis 5.2, öffentlich in der Git-Historie) wird nur noch zum Lesen benutzt:
//! Ein damit gelesener Spielstand wird sofort mit dem neuen Schlüssel neu gespeichert.
//! Python-Version: kann ab 5.3 gespeicherte Stände nicht mehr lesen (sie ist abgelöst).

use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::OnceLock;

type HmacSha256 = Hmac<Sha256>;

/// Beim Bauen eingesetzter Schlüssel (siehe build.rs): MASK XOR DATA = Schlüssel
mod built { include!(concat!(env!("OUT_DIR"), "/savekey.rs")); }

/// Alter Schlüssel bis 5.2 - nur noch zum Lesen alter Spielstände
const LEGACY_SECRET: &[u8] = b"JumpNDodge#SaveState#Key#7f3a1c9e-52d6-4b8a-9e11-0c6f2a7d4b3e#v1";
const ENCRYPTION_SALT: &[u8] = b"JumpNDodge-SaveData-KDF-Salt-v1";
const ENC_KDF_ROUNDS: u32 = 200_000;

fn derive(secret: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(secret, ENCRYPTION_SALT, ENC_KDF_ROUNDS, &mut k);
    k
}

fn legacy_key() -> &'static [u8; 32] {
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(|| derive(LEGACY_SECRET))
}

/// Gibt es einen beim Bauen eingesetzten Schlüssel? (Release-Builds immer, Entwickler-Builds meist nicht)
pub fn has_build_key() -> bool { built::SET }

/// Schlüssel zum Schreiben (und zuerst zum Lesen)
fn save_key() -> &'static [u8; 32] {
    if !built::SET { return legacy_key(); }
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(|| {
        let mut secret: Vec<u8> = built::MASK.iter().zip(built::DATA).map(|(m, d)| m ^ d).collect();
        let k = derive(&secret);
        secret.iter_mut().for_each(|b| *b = 0);
        k
    })
}

fn hmac(key: &[u8], data: &[&[u8]]) -> [u8; 32] {
    let mut m = <HmacSha256 as Mac>::new_from_slice(key).unwrap();
    for d in data { m.update(d); }
    m.finalize().into_bytes().into()
}

fn keystream(key: &[u8], nonce: &[u8], len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len + 32);
    let mut counter: u64 = 0;
    while out.len() < len {
        out.extend_from_slice(&hmac(key, &[nonce, &counter.to_be_bytes()]));
        counter += 1;
    }
    out.truncate(len);
    out
}

pub fn encrypt_bytes(plain: &[u8]) -> Vec<u8> { encrypt_with(save_key(), plain) }

fn encrypt_with(key: &[u8; 32], plain: &[u8]) -> Vec<u8> {
    let nonce: [u8; 16] = rand::random();
    let ks = keystream(key, &nonce, plain.len());
    let ct: Vec<u8> = plain.iter().zip(ks.iter()).map(|(a, b)| a ^ b).collect();
    let tag = hmac(key, &[&nonce, &ct]);
    let mut out = Vec::with_capacity(48 + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&ct);
    out
}

pub fn decrypt_bytes(blob: &[u8]) -> Option<Vec<u8>> { decrypt_any(blob).map(|(p, _)| p) }

/// Entschlüsseln mit dem aktuellen Schlüssel, sonst mit dem alten. `true` = war der alte (neu speichern!)
fn decrypt_any(blob: &[u8]) -> Option<(Vec<u8>, bool)> {
    if let Some(p) = decrypt_with(save_key(), blob) { return Some((p, false)); }
    if built::SET { if let Some(p) = decrypt_with(legacy_key(), blob) { return Some((p, true)); } }
    None
}

fn decrypt_with(key: &[u8; 32], blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() < 48 { return None; }
    let (nonce, rest) = blob.split_at(16);
    let (tag, ct) = rest.split_at(32);
    let mut m = <HmacSha256 as Mac>::new_from_slice(key).unwrap();
    m.update(nonce);
    m.update(ct);
    if m.verify_slice(tag).is_err() { return None; }
    let ks = keystream(key, nonce, ct.len());
    Some(ct.iter().zip(ks.iter()).map(|(a, b)| a ^ b).collect())
}

/// Python json.dumps(data, ensure_ascii=False): ", " und ": " als Trenner
pub fn py_json_dumps(v: &serde_json::Value) -> String {
    let mut out = String::new();
    write_py(v, &mut out);
    out
}

fn write_py(v: &serde_json::Value, out: &mut String) {
    use serde_json::Value;
    match v {
        Value::Object(m) => {
            out.push('{');
            for (i, (k, val)) in m.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(&serde_json::to_string(k).unwrap());
                out.push_str(": ");
                write_py(val, out);
            }
            out.push('}');
        }
        Value::Array(a) => {
            out.push('[');
            for (i, val) in a.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                write_py(val, out);
            }
            out.push(']');
        }
        Value::Number(n) => {
            if let Some(f) = n.as_f64().filter(|_| n.is_f64()) {
                if f.is_finite() && f == f.trunc() && f.abs() < 1e16 {
                    out.push_str(&format!("{:.1}", f));
                } else {
                    out.push_str(&serde_json::to_string(v).unwrap());
                }
            } else {
                out.push_str(&n.to_string());
            }
        }
        _ => out.push_str(&serde_json::to_string(v).unwrap()),
    }
}

pub fn save_encrypted_json(path: &std::path::Path, data: &serde_json::Value) -> std::io::Result<()> {
    let json = py_json_dumps(data);
    let blob = encrypt_bytes(json.as_bytes());
    if let Some(d) = path.parent() { std::fs::create_dir_all(d)?; }
    let b64 = base64::engine::general_purpose::STANDARD.encode(blob);
    // erst in eine Temp-Datei, dann umbenennen - ein Absturz mitten im Schreiben zerstört so nie den Spielstand
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, b64.as_bytes())?;
    std::fs::rename(&tmp, path).or_else(|_| std::fs::write(path, b64.as_bytes()))
}

/// Spielstand laden. War er noch mit dem alten Schlüssel gespeichert, wird er gleich mit dem neuen
/// neu geschrieben (einmalig, ohne Internet).
pub fn load_encrypted_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let (v, legacy) = read_encrypted_json(path)?;
    if legacy {
        if save_encrypted_json(path, &v).is_ok() { println!("[SPEICHER] {} auf neuen Schlüssel umgestellt", path.display()); }
    }
    Some(v)
}

/// Nur lesen, nichts umstellen (für --decrypt-test)
pub fn load_encrypted_json_ro(path: &std::path::Path) -> Option<serde_json::Value> { read_encrypted_json(path).map(|(v, _)| v) }

fn read_encrypted_json(path: &std::path::Path) -> Option<(serde_json::Value, bool)> {
    let raw = std::fs::read(path).ok()?;
    let raw: Vec<u8> = raw.into_iter().filter(|b| !b.is_ascii_whitespace()).collect();
    let blob = base64::engine::general_purpose::STANDARD.decode(raw).ok()?;
    let (plain, legacy) = decrypt_any(&blob)?;
    serde_json::from_slice(&plain).ok().map(|v| (v, legacy))
}

// ---------------- Speichern im Hintergrund ----------------
// Python hat bei jedem save_section() synchron verschlüsselt und geschrieben (auf Windows mit
// Virenscanner oft 20-100 ms) - das ruckelt mitten in Menüs/Runden. Hier übernimmt ein eigener
// Thread das Schreiben; liegen mehrere Stände für dieselbe Datei an, wird nur der neueste
// geschrieben. flush_saves() wartet, bis alles auf der Platte ist (beim Beenden).
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};

struct SaveQueue { pending: HashMap<PathBuf, serde_json::Value>, busy: bool }

fn queue() -> &'static (Mutex<SaveQueue>, Condvar, Condvar) {
    static Q: OnceLock<(Mutex<SaveQueue>, Condvar, Condvar)> = OnceLock::new();
    Q.get_or_init(|| {
        std::thread::Builder::new().name("save-writer".into()).spawn(|| loop {
            let (m, work, done) = queue();
            let job = {
                let mut q = m.lock().unwrap();
                while q.pending.is_empty() { q = work.wait(q).unwrap(); }
                let k = q.pending.keys().next().cloned().unwrap();
                let v = q.pending.remove(&k).unwrap();
                q.busy = true;
                (k, v)
            };
            let _ = save_encrypted_json(&job.0, &job.1);
            let mut q = m.lock().unwrap();
            q.busy = false;
            done.notify_all();
        }).expect("save-writer");
        (Mutex::new(SaveQueue { pending: HashMap::new(), busy: false }), Condvar::new(), Condvar::new())
    })
}

/// Spielstand im Hintergrund schreiben (neuester Stand gewinnt)
pub fn save_encrypted_json_bg(path: &std::path::Path, data: serde_json::Value) {
    let (m, work, _) = queue();
    m.lock().unwrap().pending.insert(path.to_path_buf(), data);
    work.notify_one();
}

/// Wartet, bis alle Hintergrund-Speicherungen geschrieben sind
pub fn flush_saves() {
    let (m, _, done) = queue();
    let mut q = m.lock().unwrap();
    while !q.pending.is_empty() || q.busy {
        let (nq, _) = done.wait_timeout(q, std::time::Duration::from_millis(200)).unwrap();
        q = nq;
    }
}

#[cfg(test)]
mod key_tests {
    use super::*;
    #[test]
    fn alter_schluessel_bleibt_lesbar() {
        let blob = encrypt_with(legacy_key(), b"{\"coins\": 5}");
        let (p, legacy) = decrypt_any(&blob).expect("alter Spielstand muss lesbar sein");
        assert_eq!(p, b"{\"coins\": 5}");
        assert_eq!(legacy, built::SET, "mit eingebautem Schlüssel gilt er als alt (wird neu gespeichert)");
    }
    #[test]
    fn neu_geschrieben_ist_lesbar_und_nicht_mit_altem_schluessel() {
        let blob = encrypt_bytes(b"abc");
        assert_eq!(decrypt_bytes(&blob).unwrap(), b"abc");
        if built::SET { assert!(decrypt_with(legacy_key(), &blob).is_none(), "neue Stände dürfen nicht mit dem alten Schlüssel aufgehen"); }
    }
    #[test]
    fn umstellung_schreibt_mit_neuem_schluessel() {
        let dir = std::env::temp_dir().join(format!("jnd-key-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let f = dir.join("stats.json");
        let blob = encrypt_with(legacy_key(), b"{\"x\": 1}");
        std::fs::write(&f, base64::engine::general_purpose::STANDARD.encode(blob)).unwrap();
        assert_eq!(load_encrypted_json(&f).unwrap()["x"], 1);
        let (_, legacy) = read_encrypted_json(&f).unwrap();
        assert!(!legacy, "nach dem Laden muss der Stand mit dem aktuellen Schlüssel gespeichert sein");
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[test]
    fn manipulierter_stand_wird_abgelehnt() {
        let mut blob = encrypt_bytes(b"{\"coins\": 5}");
        let n = blob.len() - 1;
        blob[n] ^= 1;
        assert!(decrypt_bytes(&blob).is_none());
    }
}
