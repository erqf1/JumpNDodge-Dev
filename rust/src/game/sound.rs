//! Sound-System (Python: Abschnitt "Sound System").

use super::state::{Snd, G};
use crate::util::*;
use serde_json::json;

/// Schlüssel des eingebauten Songs in der Musikauswahl
pub const BUILTIN_TRACK: &str = "jumpndodge";

thread_local! {
    static MENU_BUILTIN_PLAYING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Spiel-Song, der noch gerendert wird und danach starten soll
    static PENDING_GAME: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    /// Vorschau, die noch auf das Rendern wartet (Schlüssel, Dauer in ms)
    static PENDING_PREVIEW: std::cell::RefCell<Option<(String, i64)>> = const { std::cell::RefCell::new(None) };
}

/// Schlüssel des eingebauten Songs, der für die Auswahl wirklich läuft (fehlende Datei -> Falling Pixels)
fn builtin_for(chosen: &str, has_file: bool) -> Option<String> {
    if super::music::is_builtin(chosen) { Some(chosen.to_string()) } else if has_file { None } else { Some(BUILTIN_TRACK.to_string()) }
}

impl G {
    pub fn _discover_ingame_music_tracks(base_path: &std::path::Path) -> Vec<(String, String, String)> {
        let (prefix, suffix) = ("ingame_music_", ".mp3");
        let mut names: Vec<String> = std::fs::read_dir(base_path).map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
        names.sort();
        let mut tracks = vec![];
        for fname in names {
            let low = fname.to_lowercase();
            if low.starts_with(prefix) && low.ends_with(suffix) {
                let name = fname[prefix.len()..fname.len() - suffix.len()].to_string();
                if !name.is_empty() {
                    let display = title_case(&name.replace('_', " "));
                    tracks.push((name, display, fname));
                }
            }
        }
        tracks
    }

    pub fn load_sound_theme(&mut self) {
        let mut base_path = self._DATA_DIR.join("sound");
        if !base_path.is_dir() { base_path = self.BASE_DIR.join("sound"); }
        let load = |g: &G, f: &str| -> Option<crate::pg::Sound> {
            let p = base_path.join(f);
            if p.exists() { g.mixer.load_sound(&p.to_string_lossy()) } else { None }
        };
        self.menu_click_sound = load(self, "menu_click.mp3");
        self.click_sound = load(self, "click.mp3");
        self.click_back_sound = load(self, "click_back.mp3");
        self.gameover_sound = load(self, "gameover.mp3");
        self.hit_sound = load(self, "hit.mp3");
        self.oneup_sound = load(self, "1up.mp3");
        // Rust Edition: fehlen die Dateien (Releases enthalten keine Sounds), eingebaute Klänge nehmen
        if self.settings.gb("builtin_sounds", true) {
            let sr = self.mixer.sample_rate as f64;
            let ch = self.mixer.channels as usize;
            let mk = |g: &G, f: fn(f64) -> Vec<f32>| -> Option<crate::pg::Sound> {
                Some(g.mixer.sound_from_i16(&super::synth::to_i16(&f(sr), ch), ch as u16, sr as u32))
            };
            if self.menu_click_sound.is_none() { self.menu_click_sound = mk(self, super::synth::menu_click); }
            if self.click_sound.is_none() { self.click_sound = mk(self, super::synth::click); }
            if self.click_back_sound.is_none() { self.click_back_sound = mk(self, super::synth::click_back); }
            if self.hit_sound.is_none() { self.hit_sound = mk(self, super::synth::hit); }
            if self.oneup_sound.is_none() { self.oneup_sound = mk(self, super::synth::oneup); }
            if self.gameover_sound.is_none() { self.gameover_sound = mk(self, super::synth::gameover); }
        }
        {
            // Spielgefühl-Klänge gibt es nur eingebaut
            let sr = self.mixer.sample_rate as f64;
            let ch = self.mixer.channels as usize;
            self.combo_sound = Some(self.mixer.sound_from_i16(&super::synth::to_i16(&super::synth::combo(sr), ch), ch as u16, sr as u32));
            self.near_sound = Some(self.mixer.sound_from_i16(&super::synth::to_i16(&super::synth::near_miss(sr), ch), ch as u16, sr as u32));
        }
        let get_path = |f: &str| -> Option<String> { let p = base_path.join(f); if p.exists() { Some(p.to_string_lossy().to_string()) } else { None } };
        self.menu_music_path = get_path("menu_music.mp3");
        // Eingebaute Songs stehen immer zuerst in der Liste
        self.INGAME_MUSIC_TRACKS = G::_discover_ingame_music_tracks(&base_path);
        for (i, song) in super::music::songs().iter().enumerate() {
            self.INGAME_MUSIC_TRACKS.insert(i, (song.key.to_string(), song.title.to_string(), String::new()));
        }
        self.ingame_music_paths = self.INGAME_MUSIC_TRACKS.iter().map(|(k, _l, f)| (k.clone(), if f.is_empty() { None } else { get_path(f) })).collect();
        let chosen = self.settings.gs("ingame_music_track", "standard");
        self.ingame_music_path = self.ingame_music_paths.iter().find(|(k, _)| *k == chosen).and_then(|(_, p)| p.clone());
        // Menü-Fassung und gewählten Song im Hintergrund berechnen (nur die)
        let sel = builtin_for(&chosen, self.ingame_music_path.is_some()).unwrap_or_default();
        super::music::start_render(&sel);
        self.heartbeat_sound = self._generate_heartbeat_sound();
    }

    pub fn _generate_heartbeat_sound(&self) -> Option<crate::pg::Sound> {
        let sr = self.mixer.sample_rate as f64;
        let ch = self.mixer.channels as usize;
        let beat_hz = 52.0;
        let (dur1, gap, dur2, sil) = (0.10, 0.12, 0.08, 0.60);
        let total = (sr * (dur1 + gap + dur2 + sil)) as usize;
        let mut buf = vec![0i16; total * ch];
        let mut thump = |offset: usize, duration: f64, amp: f64| {
            let n = (sr * duration) as usize;
            for i in 0..n.min(total.saturating_sub(offset)) {
                let t = i as f64 / sr;
                let v = (amp * (-t / 0.028).exp() * (2.0 * std::f64::consts::PI * beat_hz * t).sin()) as i64;
                for c in 0..ch { buf[(offset + i) * ch + c] = v.clamp(-32767, 32767) as i16; }
            }
        };
        thump(0, dur1, 28000.0);
        thump((sr * (dur1 + gap)) as usize, dur2, 20000.0);
        Some(self.mixer.sound_from_i16(&buf, ch as u16, sr as u32))
    }

    pub fn snd(&self, s: Snd) -> Option<crate::pg::Sound> {
        match s {
            Snd::MenuClick => self.menu_click_sound.clone(),
            Snd::Click => self.click_sound.clone(),
            Snd::ClickBack => self.click_back_sound.clone(),
            Snd::Hit => self.hit_sound.clone(),
            Snd::GameOver => self.gameover_sound.clone(),
            Snd::Heartbeat => self.heartbeat_sound.clone(),
            Snd::OneUp => self.oneup_sound.clone(),
            Snd::Combo => self.combo_sound.clone(),
            Snd::NearMiss => self.near_sound.clone(),
            Snd::None => None,
        }
    }

    /// play_sound(sound, volume_key="sfx_volume")
    pub fn play_sound(&mut self, s: Snd) { self.play_sound_v(s, "sfx_volume"); }
    pub fn play_sound_v(&mut self, s: Snd, volume_key: &str) {
        if !self.sound_enabled { return; }
        if let Some(sound) = self.snd(s) {
            let user_vol = self.settings.gf(volume_key, 0.8);
            sound.set_volume(get_pygame_volume(user_vol));
            self.mixer.play_detached(&sound);
        }
    }

    pub fn play_menu_music(&mut self) {
        PENDING_GAME.with(|p| *p.borrow_mut() = None);
        if self.sound_enabled {
            self.mixer.music_set_volume(get_pygame_volume(self.settings.gf("menu_music_volume", 0.8)));
            if let Some(p) = self.menu_music_path.clone() {
                self.mixer.music_load(&p);
                self.mixer.music_play(-1);
            } else if let Some(s) = super::music::get(super::music::MENU) {
                // eingebaute ruhige Fassung
                self.mixer.music_play_shared(s, 2, super::music::SR);
                MENU_BUILTIN_PLAYING.with(|c| c.set(true));
            } else {
                super::music::request(super::music::MENU);
                MENU_BUILTIN_PLAYING.with(|c| c.set(false));
            }
        }
    }

    /// Menü: läuft noch keine Musik, weil die Menü-Fassung beim Start noch berechnet wurde -> jetzt starten
    pub fn ensure_menu_music(&mut self) {
        if !self.sound_enabled || self.menu_music_path.is_some() || self.mixer.music_get_busy() { return; }
        if MENU_BUILTIN_PLAYING.with(|c| c.get()) { return; }
        if super::music::get(super::music::MENU).is_some() { self.play_menu_music(); }
    }

    /// Jedes Bild: Songs starten, die inzwischen fertig gerendert sind
    pub fn music_tick(&mut self) {
        if let Some(k) = PENDING_GAME.with(|p| p.borrow().clone()) {
            if let Some(s) = super::music::get(&k) {
                PENDING_GAME.with(|p| *p.borrow_mut() = None);
                if self.sound_enabled {
                    self.mixer.music_set_volume(get_pygame_volume(self.settings.gf("ingame_music_volume", 0.8)));
                    self.mixer.music_play_shared(s, 2, super::music::SR);
                }
            }
        }
        if let Some((k, d)) = PENDING_PREVIEW.with(|p| p.borrow().clone()) {
            if super::music::get(&k).is_some() {
                PENDING_PREVIEW.with(|p| *p.borrow_mut() = None);
                self._preview_track_key = None;
                self.start_builtin_preview(&k, d);
            }
        }
    }

    pub fn play_ingame_music(&mut self) {
        MENU_BUILTIN_PLAYING.with(|c| c.set(false));
        let chosen = self.settings.gs("ingame_music_track", "standard");
        let file = self.ingame_music_paths.iter().find(|(k, _)| *k == chosen).and_then(|(_, p)| p.clone());
        // Eingebauter Song: wenn gewählt, oder wenn die gewählte Datei fehlt (Releases haben keine Musikdateien)
        if let Some(key) = builtin_for(&chosen, file.is_some()) {
            self.ingame_music_path = None;
            if self.sound_enabled {
                if let Some(s) = super::music::get(&key) {
                    self.mixer.music_set_volume(get_pygame_volume(self.settings.gf("ingame_music_volume", 0.8)));
                    self.mixer.music_play_shared(s, 2, super::music::SR);
                } else {
                    // wird noch berechnet: startet von selbst, sobald er fertig ist
                    self.mixer.music_stop();
                    super::music::request(&key);
                    PENDING_GAME.with(|p| *p.borrow_mut() = Some(key));
                }
            }
            return;
        }
        PENDING_GAME.with(|p| *p.borrow_mut() = None);
        self.ingame_music_path = file;
        if self.sound_enabled {
            if let Some(p) = self.ingame_music_path.clone() {
                self.mixer.music_load(&p);
                self.mixer.music_set_volume(get_pygame_volume(self.settings.gf("ingame_music_volume", 0.8)));
                self.mixer.music_play(-1);
            }
        }
    }

    pub fn stop_music(&mut self) {
        PENDING_GAME.with(|p| *p.borrow_mut() = None);
        self.mixer.music_stop();
    }

    /// Wird der Song gerade für die Vorschau berechnet?
    pub fn is_loading_track(&self, track_key: &str) -> bool {
        PENDING_PREVIEW.with(|p| p.borrow().as_ref().map(|x| x.0 == track_key).unwrap_or(false))
    }

    fn start_builtin_preview(&mut self, key: &str, duration_ms: i64) {
        let Some(s) = super::music::get(key) else { return; };
        if !self.sound_enabled { return; }
        let Some(info) = super::music::info(key) else { return; };
        // Vorschau: ab dem ersten Drop
        let start = ((60.0 / info.bpm * 4.0 * info.preview_bar) * super::music::SR as f32) as usize * 2;
        let start = if start >= s.len() { 0 } else { start };
        let end = (start + (duration_ms.max(1000) as usize * super::music::SR as usize / 1000) * 2).min(s.len());
        let sound = self.mixer.sound_from_i16(&s[start..end], 2, super::music::SR);
        sound.set_volume(get_pygame_volume(self.settings.gf("ingame_music_volume", 0.8)));
        self._preview_channel = self.mixer.play(&sound, duration_ms);
        self._preview_track_key = if self._preview_channel.is_some() { Some(key.to_string()) } else { None };
    }

    pub fn preview_ingame_music_track(&mut self, track_key: &str, duration_ms: i64) {
        let was_same = self._preview_track_key.as_deref() == Some(track_key) || self.is_loading_track(track_key);
        let had_pending = PENDING_PREVIEW.with(|p| p.borrow_mut().take()).is_some();
        if let Some(ch) = self._preview_channel.take() {
            ch.stop();
            self._preview_track_key = None;
            if was_same { return; }
        } else if had_pending && was_same { return; }
        if super::music::is_builtin(track_key) {
            if !self.sound_enabled { return; }
            if super::music::get(track_key).is_some() {
                self.start_builtin_preview(track_key, duration_ms);
            } else {
                super::music::request(track_key);
                PENDING_PREVIEW.with(|p| *p.borrow_mut() = Some((track_key.to_string(), duration_ms)));
            }
            return;
        }
        let path = self.ingame_music_paths.iter().find(|(k, _)| k == track_key).and_then(|(_, p)| p.clone());
        let Some(path) = path else { return; };
        if !self.sound_enabled { return; }
        if let Some(sound) = self.mixer.load_sound(&path) {
            sound.set_volume(get_pygame_volume(self.settings.gf("ingame_music_volume", 0.8)));
            self._preview_channel = self.mixer.play(&sound, duration_ms);
            self._preview_track_key = Some(track_key.to_string());
        }
    }

    pub fn is_previewing_track(&mut self, track_key: &str) -> bool {
        if let Some(ch) = &self._preview_channel {
            if !ch.get_busy() {
                self._preview_channel = None;
                self._preview_track_key = None;
            }
        }
        self._preview_track_key.as_deref() == Some(track_key)
    }

    pub fn toggle_sound_enabled(&mut self) {
        self.sound_enabled = !self.sound_enabled;
        self.settings.insert("sound_enabled".into(), json!(self.sound_enabled));
        self.save_section("settings", None);
        if !self.sound_enabled { self.stop_music(); }
    }

    fn adjust_vol(&mut self, key: &str, delta: f64) -> f64 {
        let cur = self.settings.gf(key, 0.8);
        let new = (cur + delta).clamp(0.0, 2.0);
        self.settings.insert(key.into(), jf(new));
        self.save_section("settings", None);
        new
    }
    pub fn adjust_menu_click_volume(&mut self, d: f64) { self.adjust_vol("menu_click_volume", d); }
    pub fn adjust_click_volume(&mut self, d: f64) { self.adjust_vol("click_volume", d); }
    pub fn adjust_click_back_volume(&mut self, d: f64) { self.adjust_vol("click_back_volume", d); }
    pub fn adjust_hit_volume(&mut self, d: f64) { self.adjust_vol("hit_volume", d); }
    pub fn adjust_gameover_volume(&mut self, d: f64) { self.adjust_vol("gameover_volume", d); }
    pub fn adjust_menu_music_volume(&mut self, d: f64) {
        let new = self.adjust_vol("menu_music_volume", d);
        if self.mixer.music_get_busy() { self.mixer.music_set_volume(get_pygame_volume(new)); }
    }
    pub fn adjust_ingame_music_volume(&mut self, d: f64) {
        let new = self.adjust_vol("ingame_music_volume", d);
        if self.mixer.music_get_busy() { self.mixer.music_set_volume(get_pygame_volume(new)); }
    }
}

pub fn get_pygame_volume(user_volume: f64) -> f64 {
    if user_volume <= 1.0 { user_volume * 0.1 } else { 0.1 + (user_volume - 1.0) * 0.9 }
}
