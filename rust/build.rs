//! Windows: Icon und Versionsinfos in JumpNDodge.exe einbetten (Explorer, Taskleiste, "Apps & Features").
//! Außerdem: Spielstand-Schlüssel aus der Umgebung einsetzen (siehe `src/crypto.rs`).
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

fn main() {
    println!("cargo:rerun-if-changed=../icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut r = winresource::WindowsResource::new();
        r.set_icon("../icon.ico");
        r.set("ProductName", "JumpNDodge");
        r.set("FileDescription", "JumpNDodge");
        r.set("CompanyName", "W Software");
        r.set("LegalCopyright", "Apache-2.0");
        if let Err(e) = r.compile() { println!("cargo:warning=Icon konnte nicht eingebettet werden: {e}"); }
    }
    save_key();
}

/// Der Spielstand-Schlüssel steht nicht im Quellcode. Er kommt beim Bauen aus `JND_SAVE_KEY`
/// (in GitHub ein Secret) und landet nur verschleiert im Programm: zufällige Maske + Schlüssel XOR Maske,
/// so findet man ihn nicht einfach mit `strings`. Fehlt er, benutzt das Programm den alten Schlüssel
/// (Entwickler-Builds). Mit `JND_REQUIRE_SAVE_KEY=1` (Release-Workflow) bricht der Build dann ab.
fn save_key() {
    println!("cargo:rerun-if-env-changed=JND_SAVE_KEY");
    println!("cargo:rerun-if-env-changed=JND_REQUIRE_SAVE_KEY");
    let key = std::env::var("JND_SAVE_KEY").ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty());
    if key.is_none() && std::env::var("JND_REQUIRE_SAVE_KEY").as_deref() == Ok("1") {
        panic!("JND_SAVE_KEY fehlt: Release-Builds brauchen den Spielstand-Schlüssel (GitHub-Secret JND_SAVE_KEY).");
    }
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("savekey.rs");
    let code = match key {
        None => "pub const SET: bool = false;\npub const MASK: &[u8] = &[];\npub const DATA: &[u8] = &[];\n".to_string(),
        Some(k) => {
            if k.len() < 32 { panic!("JND_SAVE_KEY ist zu kurz (mindestens 32 Zeichen)"); }
            let bytes = k.as_bytes();
            // Zufällige Maske ohne extra Abhängigkeit: RandomState ist pro Build-Lauf zufällig geseedet
            let mut mask = Vec::with_capacity(bytes.len());
            let s = RandomState::new();
            for i in 0..bytes.len() {
                let mut h = s.build_hasher();
                h.write_usize(i);
                mask.push((h.finish() & 0xff) as u8);
            }
            let data: Vec<u8> = bytes.iter().zip(&mask).map(|(a, b)| a ^ b).collect();
            format!("pub const SET: bool = true;\npub const MASK: &[u8] = &{:?};\npub const DATA: &[u8] = &{:?};\n", mask, data)
        }
    };
    std::fs::write(out, code).unwrap();
}
