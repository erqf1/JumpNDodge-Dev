//! Verschlüsselter Spielstand - exakt dasselbe Verfahren wie in der Python-Version
//! (HMAC-SHA256-Zählermodus + HMAC-Tag, Schlüssel per PBKDF2), damit alte Spielstände
//! weiter geladen werden können und die Python-Version neue Speicherstände lesen kann.

use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::OnceLock;

type HmacSha256 = Hmac<Sha256>;

const ENCRYPTION_SECRET: &[u8] = b"JumpNDodge#SaveState#Key#7f3a1c9e-52d6-4b8a-9e11-0c6f2a7d4b3e#v1";
const ENCRYPTION_SALT: &[u8] = b"JumpNDodge-SaveData-KDF-Salt-v1";
const ENC_KDF_ROUNDS: u32 = 200_000;

fn save_key() -> &'static [u8; 32] {
    static KEY: OnceLock<[u8; 32]> = OnceLock::new();
    KEY.get_or_init(|| {
        let mut k = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(ENCRYPTION_SECRET, ENCRYPTION_SALT, ENC_KDF_ROUNDS, &mut k);
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

pub fn encrypt_bytes(plain: &[u8]) -> Vec<u8> {
    let key = save_key();
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

pub fn decrypt_bytes(blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() < 48 { return None; }
    let key = save_key();
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

pub fn load_encrypted_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let raw = std::fs::read(path).ok()?;
    let raw: Vec<u8> = raw.into_iter().filter(|b| !b.is_ascii_whitespace()).collect();
    let blob = base64::engine::general_purpose::STANDARD.decode(raw).ok()?;
    let plain = decrypt_bytes(&blob)?;
    serde_json::from_slice(&plain).ok()
}
