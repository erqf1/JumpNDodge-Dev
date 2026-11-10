//! jnd_core: JumpNDodge-Spiellogik ohne Grafik, Fenster oder Ton.
//! Wird vom Spiel (lokaler 2-Spieler-Modus) und später vom Multiplayer-Server benutzt,
//! damit beide garantiert dieselben Regeln haben.

pub mod rect;
pub mod net;
pub mod sim2p;

pub use rect::{addf, rect, Num, Rect};
