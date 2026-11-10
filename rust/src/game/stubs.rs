//! Vorläufige Platzhalter für noch nicht portierte Funktionen (werden nach und nach ersetzt)
use super::state::G;
use crate::pg::Surface;

impl G {
    pub fn debug_log(&mut self, _msg: &str) {}
    pub fn push_overlay(&mut self, text: &str, color: (i32, i32, i32), duration: f64, pos: Option<(i32, i32)>) { let _ = (text, color, duration, pos); }
    pub fn _process_dev_commands(&mut self) {}
    pub fn _debug_track_screen_transitions(&mut self) {}
    pub fn _screenshot_hook(&mut self, _s: &Surface) {}
}

impl G {
    pub fn open_chat(&mut self, _t: &str, _target: &str, _ret: &str, _label: Option<String>) {}
    pub fn open_clan_view(&mut self, _tag: &str, _ret: &str) {}
    pub fn draw_player_cosmetics(&self, _s: &Surface, _r: crate::pg::Rect, _o: Option<&serde_json::Map<String, serde_json::Value>>) {}
    pub fn draw_hover_tooltip(&mut self, _c: &[String], _mp: (i32, i32), _mw: i32, _acc: Option<(i32, i32, i32)>, _anchor: Option<crate::pg::Rect>) {}
}
