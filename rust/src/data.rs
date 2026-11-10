//! Große Datentabellen aus JumpNDodge.py (automatisch extrahiert nach assets/data.json).

use serde_json::Value;
use std::sync::LazyLock;

pub static D: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/data.json")).expect("data.json")
});

/// Eine Tabelle nach Python-Namen, z.B. `d("POWERS_DB")`
pub fn d(name: &str) -> &'static Value { &D[name] }
