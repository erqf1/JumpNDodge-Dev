//! Gemeinsames für die Songs: Bibliothek an Break-Chops und typische Übergänge.

use crate::game::music::arr::*;
use crate::game::music::engine::*;

// Scheiben des Breaks: 0 K, 2 K, 4 S, 7 g, 9 g, 10 K, 12 S, 15 g | 16 K, 18 K, 20 S, 23 g, 25 g, 26 K, 29 g, 30 S, 31 g
pub const STRAIGHT: [&str; 2] = ["0 _ _ _ _ _ _ _ _ _ _ _ _ _ _ _", "16 _ _ _ _ _ _ _ _ _ _ _ _ _ _ _"];
/// leicht umgestellt – Kopfnicker
pub const GROOVE: [&str; 4] = [
    "0 _ _ _ _ _ _ _ _ _ _ _ _ _ _ _",
    "16 _ 18 _ 20 _ 6 7 8 9 26 _ 12 _ 30 31",
    "0 _ 2 _ 4 _ _ 7 0 _ 10 _ 12 _ 28 29",
    "16 _ _ _ 20 _ _ 23 24 _ 26 _ 4 _ 30 31",
];
/// dichter, mit Doppel-Snares und Stottern
pub const CHOPPY: [&str; 4] = [
    "0 _ 2 _ 4 _ 4 _ 9 _ 10 _ 12 13 14 15",
    "16 _ 26 _ 20 _ _ 23 0 _ 2 _ 30 _ 12s 31",
    "0 _ 2 _ 4 _ _ 7 10 _ 10 _ 12 _ 4s 15",
    "16 _ 18 18s 20 _ 23 _ 26 _ 4 _ 12 _ 30 31",
];
/// Breakcore: rückwärts, Wirbel, Tonhöhe
pub const GLITCH: [&str; 4] = [
    "0 0s 2 _ 4r _ 4 7 10 _ 10 _ 12t 12 30 _",
    "16 _ 18 18s 20 _ 23 23 26 _ 4 _ 12 _ 30t 31^",
    "0 _ 26 _ 4 4s 7 _ 0r _ 2 _ 12 _ 20t^ _",
    "16 16s 18 _ 20 _ 4r _ 26 _ 10 10s 12 _ 30s 30t^",
];
pub const FILLS: [&str; 4] = [
    "0 _ 2 _ 4 _ 4 _ 12 12 12 12 20s 20s 30t 30t",
    "0 _ 2 _ 4 _ 7 _ 4s 4s 12s 12s 4t^ 4t^ 12t^ 12t^",
    "16 _ 18 _ 20 _ _ 23 4r _ _ _ 12t 12t 30t^ 30t^",
    "0 _ 2 _ 4 _ . . 12 _ . 12 12 . 30^ 30^",
];
/// Halftime: Kick auf 1, Snare auf 3
pub const HALF: [&str; 2] = ["0 _ . . . . 2 _ 4 _ _ _ . . 9 _", "0 _ . . 9 . 26 _ 4 _ _ _ . . 4s 15"];
pub const HALF_FILL: &str = "0 _ . . . . 2 _ 4 _ 4 _ 12s 12s 4t^ 4t^";

/// Übergang in einen Drop auf Schlag `t`: Riser, Rückwärts-Becken, kurze Stille, Einschlag + Becken
pub fn drop_in(a: &mut Arr, t: f32, riser_beats: f32, gap: bool, vel: f32) {
    let end = if gap { t - 0.5 } else { t };
    a.riser(end, riser_beats, 0.55 * vel, true);
    a.rev_crash(end, 2.0, 0.6 * vel);
    if gap { a.gap(t - 0.5, 0.5); }
    a.impact(t, 0.9 * vel);
    a.crash(t, 0.75 * vel);
}

/// Snare-Wirbel in Sechzehnteln, in der zweiten Hälfte Zweiunddreißigstel, immer lauter
pub fn snare_roll(a: &mut Arr, t0: f32, beats: f32, kit: &Kit, v0: f32, v1: f32) {
    let half = beats / 2.0;
    let n1 = (half * 4.0) as usize;
    let n2 = (half * 8.0) as usize;
    for i in 0..n1 { let x = i as f32 / (n1 + n2) as f32; a.snare(t0 + i as f32 * 0.25, v0 + (v1 - v0) * x, kit); }
    for i in 0..n2 { let x = (n1 + i) as f32 / (n1 + n2) as f32; a.snare(t0 + half + i as f32 * 0.125, v0 + (v1 - v0) * x, kit); }
}

/// Zusätzliche Hi-Hats in Sechzehnteln (treibend)
pub fn hats16(a: &mut Arr, bar0: usize, bars: usize, vel: f32, open_every: usize) {
    for i in 0..bars * 16 {
        let t = bar0 as f32 * 4.0 + i as f32 * 0.25;
        let acc = if i % 4 == 2 { 1.0 } else if i % 2 == 0 { 0.6 } else { 0.42 };
        let open = open_every > 0 && i % (open_every * 16) == open_every * 16 - 2;
        a.add(t, Bus::Perc, if i % 2 == 0 { 0.3 } else { -0.3 }, crate::game::music::inst::Inst::Hat { vel: vel * acc, open });
    }
}
