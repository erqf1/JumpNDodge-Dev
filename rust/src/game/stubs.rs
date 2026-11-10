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
}
impl G {
}
impl G {
    pub fn _drain_dev_round_actions(&mut self) -> Vec<(String, Option<i64>)> { vec![] }
    pub fn show_game_over(&mut self, _score: i64, _gained: i64, _old: i64, _new_record: bool, _secs: i64) {}
}
