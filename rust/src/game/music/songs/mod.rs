//! Die eingebauten Songs. Jeder Song ist eine Funktion, die mit dem Baukasten (`arr`) seine
//! Partitur baut. Alles eigene Kompositionen im Stil von emotionalem Breakcore mit Klavier.

use super::engine::Score;
use super::SongInfo;

mod banners;
mod common;
mod ember_rush;
mod falling_pixels;
mod frostlight;
mod gearbreaker;
mod hollow_star;

pub static LIST: &[SongInfo] = &[
    SongInfo { key: "jumpndodge", title: "Falling Pixels", style: "emotional", style_en: "emotional", bpm: 174.0, preview_bar: 16.0, build: || falling_pixels::build(false) },
    SongInfo { key: "jnd_banners", title: "Banners in the Rain", style: "episch-kämpferisch", style_en: "epic, battle", bpm: 172.0, preview_bar: 16.0, build: banners::build },
    SongInfo { key: "jnd_gearbreaker", title: "Gearbreaker", style: "chaotisch-mechanisch", style_en: "chaotic, mechanical", bpm: 182.0, preview_bar: 16.0, build: gearbreaker::build },
    SongInfo { key: "jnd_frostlight", title: "Frostlight", style: "kühl-eisig", style_en: "cold, icy", bpm: 168.0, preview_bar: 16.0, build: frostlight::build },
    SongInfo { key: "jnd_hollow_star", title: "Hollow Star", style: "dunkel", style_en: "dark, void", bpm: 164.0, preview_bar: 24.0, build: hollow_star::build },
    SongInfo { key: "jnd_ember_rush", title: "Ember Rush", style: "feurig-treibend", style_en: "fiery, driving", bpm: 180.0, preview_bar: 16.0, build: ember_rush::build },
];

/// Ruhige Menü-Fassung
pub fn menu() -> Score { falling_pixels::build(true) }
