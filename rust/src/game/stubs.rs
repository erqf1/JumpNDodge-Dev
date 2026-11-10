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
}
impl G {
}
