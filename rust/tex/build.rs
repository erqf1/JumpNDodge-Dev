// Bettet alle Texturen aus assets/textures/*.svg ins Programm ein.
use std::{env, fs, path::Path};

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/textures");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = fs::read_dir(&dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".svg")).collect())
        .unwrap_or_default();
    names.sort();
    let mut out = String::from("&[\n");
    for n in &names {
        let p = dir.join(n);
        println!("cargo:rerun-if-changed={}", p.display());
        let stem = n.trim_end_matches(".svg");
        out.push_str(&format!("    ({:?}, include_str!({:?})),\n", stem, p.canonicalize().unwrap().display().to_string()));
    }
    out.push_str("]\n");
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("textures.rs"), out).unwrap();
}
