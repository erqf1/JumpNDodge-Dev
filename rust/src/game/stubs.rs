//! Vorläufige Platzhalter für noch nicht portierte Funktionen (werden nach und nach ersetzt)
use super::state::G;
use crate::pg::Surface;

impl G {
    pub fn debug_log(&mut self, _msg: &str) {}
    pub fn _process_dev_commands(&mut self) {}
    pub fn _debug_track_screen_transitions(&mut self) {}
    pub fn _screenshot_hook(&mut self, _s: &Surface) {}
}

impl G {
    pub fn draw_player_cosmetics(&self, _s: &Surface, _r: crate::pg::Rect, _o: Option<&serde_json::Map<String, serde_json::Value>>) {}
    pub fn draw_hover_tooltip(&mut self, _c: &[String], _mp: (i32, i32), _mw: i32, _acc: Option<(i32, i32, i32)>, _anchor: Option<crate::pg::Rect>) {}
}
impl G {
}
