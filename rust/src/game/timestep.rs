//! Feste Spiellogik-Rate, unabhängig von der Bildrate.
//!
//! Python rechnete pro Bild (60 FPS fest). Die Rust-Version zeichnet so schnell wie der
//! Monitor (z. B. 144/165 Hz), die Spielregeln laufen aber immer mit genau 60 Schritten
//! pro Sekunde – dadurch bleiben Sprunghöhe, Tempo und Highscores identisch.
//! Zwischen zwei Logikschritten wird beim Zeichnen interpoliert (`alpha`).

use super::state::G;

pub const LOGIC_HZ: f64 = 60.0;
pub const STEP_MS: f64 = 1000.0 / LOGIC_HZ;
/// Höchstens so viele Schritte nachholen (sonst "Todesspirale" nach Hängern)
const MAX_STEPS: usize = 6;

#[derive(Default)]
pub struct FixedStep {
    acc: f64,
    n: u64,
}

impl FixedStep {
    pub fn new() -> FixedStep { FixedStep { acc: STEP_MS, n: 0 } }
    /// Fügt die vergangene Zeit hinzu und liefert die ganzzahligen dt-Werte (ms) der fälligen
    /// Logikschritte (abwechselnd 16/17 ms, im Mittel exakt 1000/60).
    pub fn advance(&mut self, frame_ms: f64) -> Vec<i64> {
        self.acc += frame_ms.clamp(0.0, 250.0);
        let mut out = Vec::new();
        while self.acc >= STEP_MS && out.len() < MAX_STEPS {
            self.acc -= STEP_MS;
            let a = (self.n as f64 * STEP_MS).floor() as i64;
            let b = ((self.n + 1) as f64 * STEP_MS).floor() as i64;
            out.push(b - a);
            self.n += 1;
        }
        if out.len() == MAX_STEPS { self.acc = self.acc.min(STEP_MS); }
        out
    }
    /// Anteil (0..1) zwischen dem letzten und dem nächsten Logikschritt
    pub fn alpha(&self) -> f64 { (self.acc / STEP_MS).clamp(0.0, 1.0) }
}

/// Zwischenwert für das Zeichnen
pub fn lerp_i(a: i32, b: i32, t: f64) -> i32 {
    if (b - a).abs() > 200 { return b; } // Sprung (z. B. neu platziert) nicht verschmieren
    (a as f64 + (b - a) as f64 * t).round() as i32
}

impl G {
    /// Ziel-Bildrate im Spiel: Einstellung `fps_limit` (0 = wie der Monitor)
    pub fn render_fps(&self) -> i32 {
        use crate::util::PyGet;
        let lim = self.settings.gi("fps_limit", 0);
        if lim > 0 { return lim.clamp(30, 1000) as i32; }
        self.display.refresh_rate().map(|h| h as i32).unwrap_or(60).max(60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sechzig_schritte_pro_sekunde_bei_jeder_bildrate() {
        for fps in [30.0, 60.0, 144.0, 165.0, 240.0] {
            let mut fs = FixedStep::new();
            fs.acc = 0.0;
            let mut steps = 0; let mut ms = 0;
            for _ in 0..(fps as usize * 10) { let v = fs.advance(1000.0 / fps); steps += v.len(); ms += v.iter().sum::<i64>(); }
            assert!((steps as i64 - 600).abs() <= 1, "fps {fps}: {steps}");
            assert!((ms - 10000).abs() <= 17, "fps {fps}: {ms}");
        }
    }
}
