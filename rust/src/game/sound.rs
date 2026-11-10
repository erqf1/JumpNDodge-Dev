//! Sound-System (Python: Abschnitt "Sound System").

use super::state::{Snd, G};
use crate::util::*;
use serde_json::json;

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
        let get_path = |f: &str| -> Option<String> { let p = base_path.join(f); if p.exists() { Some(p.to_string_lossy().to_string()) } else { None } };
        self.menu_music_path = get_path("menu_music.mp3");
        self.INGAME_MUSIC_TRACKS = G::_discover_ingame_music_tracks(&base_path);
        self.ingame_music_paths = self.INGAME_MUSIC_TRACKS.iter().map(|(k, _l, f)| (k.clone(), get_path(f))).collect();
        let chosen = self.settings.gs("ingame_music_track", "standard");
        self.ingame_music_path = self.ingame_music_paths.iter().find(|(k, _)| *k == chosen).and_then(|(_, p)| p.clone())
            .or_else(|| self.ingame_music_paths.first().and_then(|(_, p)| p.clone()));
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
        if self.sound_enabled {
            if let Some(p) = self.menu_music_path.clone() {
                self.mixer.music_load(&p);
                self.mixer.music_set_volume(get_pygame_volume(self.settings.gf("menu_music_volume", 0.8)));
                self.mixer.music_play(-1);
            }
        }
    }

    pub fn play_ingame_music(&mut self) {
        let chosen = self.settings.gs("ingame_music_track", "standard");
        self.ingame_music_path = self.ingame_music_paths.iter().find(|(k, _)| *k == chosen).and_then(|(_, p)| p.clone())
            .or_else(|| self.ingame_music_paths.first().and_then(|(_, p)| p.clone()));
        if self.sound_enabled {
            if let Some(p) = self.ingame_music_path.clone() {
                self.mixer.music_load(&p);
                self.mixer.music_set_volume(get_pygame_volume(self.settings.gf("ingame_music_volume", 0.8)));
                self.mixer.music_play(-1);
            }
        }
    }

    pub fn stop_music(&mut self) { self.mixer.music_stop(); }

    pub fn preview_ingame_music_track(&mut self, track_key: &str, duration_ms: i64) {
        let was_same = self._preview_track_key.as_deref() == Some(track_key);
        if let Some(ch) = self._preview_channel.take() {
            ch.stop();
            self._preview_track_key = None;
            if was_same { return; }
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
