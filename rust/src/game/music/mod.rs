//! Eingebaute Musik: eigene Breakcore-Songs, komplett synthetisch erzeugt (keine Dateien).
//!
//! Aufbau:
//! - `dsp`    Wellentabellen, Filter, Hall, Echo, Limiter
//! - `inst`   Instrumente (Klavier, Glocke, Zupfsaite, Synth, Chor, Sub/Reese, Drums, Effekte)
//! - `engine` Busse, Sidechain, Master-Effekte, paralleles Rendern mit nahtloser Schleife
//! - `arr`    Noten-/Akkordschrift, Breakbeat zum Zerhacken, Helfer für Schichten
//! - `songs`  die Songs als Arrangement-Funktionen
//!
//! Gerendert wird nur, was gebraucht wird (Menü-Fassung + gewählter Song), in einem
//! Hintergrund-Thread. Im Speicher bleiben höchstens die Menü-Fassung und zwei Songs.

pub mod arr;
pub mod dsp;
pub mod engine;
pub mod inst;
pub mod songs;

pub use dsp::SR;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

/// Ein eingebauter Song
pub struct SongInfo {
    /// Schlüssel in den Einstellungen (`ingame_music_track`)
    pub key: &'static str,
    pub title: &'static str,
    /// Stil (deutsch / englisch) für die Anzeige in der Musikauswahl
    pub style: &'static str,
    pub style_en: &'static str,
    pub bpm: f32,
    /// Vorschau ab diesem Takt (erster Drop)
    pub preview_bar: f32,
    pub build: fn() -> engine::Score,
}

/// Schlüssel der ruhigen Menü-Fassung
pub const MENU: &str = "menu";

pub fn songs() -> &'static [SongInfo] { songs::LIST }
pub fn info(key: &str) -> Option<&'static SongInfo> { songs().iter().find(|s| s.key == key) }
pub fn is_builtin(key: &str) -> bool { info(key).is_some() }

/// Song sofort rendern (blockiert; für Tests und den Hintergrund-Thread)
pub fn render_now(key: &str) -> Option<Vec<i16>> { render_now_stats(key).map(|r| r.0) }

pub fn render_now_stats(key: &str) -> Option<(Vec<i16>, [f32; engine::NBUS])> {
    let sc = if key == MENU { songs::menu() } else { (info(key)?.build)() };
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2);
    // im Spiel einen Kern frei lassen
    let threads = if cfg!(test) { threads } else { threads.saturating_sub(1) }.clamp(1, 4);
    Some(engine::render_stats(&sc, threads))
}

// ---------------------------------------------------------------- Hintergrund-Rendern + Speicher

#[derive(Default)]
struct St { cache: Vec<(String, Arc<Vec<i16>>)>, queue: Vec<String>, busy: Option<String> }

fn state() -> &'static (Mutex<St>, Condvar) {
    static S: OnceLock<(Mutex<St>, Condvar)> = OnceLock::new();
    S.get_or_init(|| {
        std::thread::Builder::new().name("musik".into()).spawn(worker).ok();
        (Mutex::new(St::default()), Condvar::new())
    })
}

fn worker() {
    loop {
        let key = {
            let (m, cv) = state();
            let mut st = m.lock().unwrap();
            while st.queue.is_empty() { st = cv.wait(st).unwrap(); }
            let k = st.queue.remove(0);
            st.busy = Some(k.clone());
            k
        };
        let data = render_now(&key);
        let (m, _) = state();
        let mut st = m.lock().unwrap();
        st.busy = None;
        if let Some(d) = data {
            st.cache.retain(|c| c.0 != key);
            st.cache.push((key, Arc::new(d)));
            // höchstens zwei Songs + Menü behalten (älteste zuerst raus)
            while st.cache.iter().filter(|c| c.0 != MENU).count() > 2 {
                if let Some(i) = st.cache.iter().position(|c| c.0 != MENU) { st.cache.remove(i); }
            }
        }
    }
}

/// Song anfordern (rendert im Hintergrund, falls noch nicht da)
pub fn request(key: &str) {
    if key != MENU && !is_builtin(key) { return; }
    let (m, cv) = state();
    let mut st = m.lock().unwrap();
    if st.cache.iter().any(|c| c.0 == key) || st.busy.as_deref() == Some(key) || st.queue.iter().any(|q| q == key) { return; }
    // nur der zuletzt gewünschte Song wartet (schnelles Durchklicken rendert nicht alles)
    st.queue.retain(|q| q == MENU);
    st.queue.push(key.to_string());
    cv.notify_one();
}

/// Fertig gerenderter Song (oder None, solange er noch berechnet wird)
pub fn get(key: &str) -> Option<Arc<Vec<i16>>> {
    let (m, _) = state();
    let mut st = m.lock().unwrap();
    let i = st.cache.iter().position(|c| c.0 == key)?;
    // zuletzt benutzt -> nach hinten
    let e = st.cache.remove(i);
    let d = e.1.clone();
    st.cache.push(e);
    Some(d)
}

/// Wird gerade gerendert oder wartet?
pub fn pending(key: &str) -> bool {
    let (m, _) = state();
    let st = m.lock().unwrap();
    st.busy.as_deref() == Some(key) || st.queue.iter().any(|q| q == key)
}

/// Beim Start: Menü-Fassung und gewählten Song im Hintergrund berechnen
pub fn start_render(selected: &str) {
    request(MENU);
    if is_builtin(selected) { request(selected); }
}

/// WAV-Datei schreiben (für Vorschau/Tests)
pub fn write_wav(path: &std::path::Path, samples: &[i16]) -> std::io::Result<()> {
    let mut b = Vec::with_capacity(44 + samples.len() * 2);
    let data_len = (samples.len() * 2) as u32;
    b.extend_from_slice(b"RIFF"); b.extend_from_slice(&(36 + data_len).to_le_bytes()); b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes()); b.extend_from_slice(&1u16.to_le_bytes()); b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&SR.to_le_bytes()); b.extend_from_slice(&(SR * 4).to_le_bytes()); b.extend_from_slice(&4u16.to_le_bytes()); b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data"); b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples { b.extend_from_slice(&s.to_le_bytes()); }
    std::fs::write(path, b)
}

#[cfg(test)]
mod tests;
