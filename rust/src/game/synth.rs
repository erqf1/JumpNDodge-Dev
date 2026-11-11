//! Eingebaute Standard-Sounds (synthetisch erzeugt). Die Releases enthalten keine Sounddateien -
//! liegen eigene in `data/sound`, werden die benutzt, sonst diese hier.

use std::f64::consts::TAU;

/// Ein Ton: Frequenz von f0 nach f1 (gleitend), Länge, Lautstärke, Wellenform-Mischung (0 = Sinus, 1 = Dreieck)
struct Tone { start: f64, dur: f64, f0: f64, f1: f64, amp: f64, tri: f64, attack: f64, decay: f64 }

fn render(sr: f64, total: f64, tones: &[Tone], noise: Option<(f64, f64, f64)>) -> Vec<f32> {
    let n = (sr * total) as usize;
    let mut out = vec![0f32; n];
    for t in tones {
        let s0 = (t.start * sr) as usize;
        let len = (t.dur * sr) as usize;
        let mut phase = 0.0f64;
        for i in 0..len {
            let k = s0 + i;
            if k >= n { break; }
            let x = i as f64 / sr;
            let f = t.f0 + (t.f1 - t.f0) * (x / t.dur);
            phase += f / sr;
            let p = phase.fract();
            let sine = (TAU * p).sin();
            let tri = 4.0 * (p - 0.5).abs() - 1.0;
            let wave = sine * (1.0 - t.tri) + tri * t.tri;
            let env = (x / t.attack).min(1.0) * (-x / t.decay).exp() * (1.0 - (x / t.dur).powi(4)).max(0.0);
            out[k] += (wave * env * t.amp) as f32;
        }
    }
    if let Some((amp, decay, len)) = noise {
        // gefiltertes Rauschen (für "Treffer"), deterministisch
        let mut seed: u32 = 0x2468_ace1;
        let mut lp = 0.0f64;
        for i in 0..((len * sr) as usize).min(n) {
            seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
            let r = (seed as f64 / u32::MAX as f64) * 2.0 - 1.0;
            lp += (r - lp) * 0.18;
            let x = i as f64 / sr;
            out[i] += (lp * amp * (-x / decay).exp()) as f32;
        }
    }
    out
}

fn note(n: i32) -> f64 { 440.0 * 2f64.powf((n - 69) as f64 / 12.0) }

pub fn menu_click(sr: f64) -> Vec<f32> {
    render(sr, 0.06, &[Tone { start: 0.0, dur: 0.05, f0: 1500.0, f1: 1300.0, amp: 0.30, tri: 0.2, attack: 0.002, decay: 0.012 }], None)
}
pub fn click(sr: f64) -> Vec<f32> {
    render(sr, 0.08, &[
        Tone { start: 0.0, dur: 0.07, f0: 980.0, f1: 1180.0, amp: 0.28, tri: 0.3, attack: 0.002, decay: 0.02 },
    ], None)
}
pub fn click_back(sr: f64) -> Vec<f32> {
    render(sr, 0.12, &[
        Tone { start: 0.0, dur: 0.05, f0: 880.0, f1: 860.0, amp: 0.24, tri: 0.3, attack: 0.002, decay: 0.02 },
        Tone { start: 0.045, dur: 0.07, f0: 640.0, f1: 600.0, amp: 0.24, tri: 0.3, attack: 0.002, decay: 0.025 },
    ], None)
}
pub fn hit(sr: f64) -> Vec<f32> {
    render(sr, 0.28, &[
        Tone { start: 0.0, dur: 0.25, f0: 180.0, f1: 55.0, amp: 0.55, tri: 0.0, attack: 0.003, decay: 0.07 },
        Tone { start: 0.0, dur: 0.08, f0: 420.0, f1: 200.0, amp: 0.18, tri: 0.6, attack: 0.001, decay: 0.02 },
    ], Some((0.35, 0.035, 0.12)))
}
pub fn oneup(sr: f64) -> Vec<f32> {
    // aufsteigendes Arpeggio (C-Dur), weich
    let ns = [84, 88, 91, 96];
    let tones: Vec<Tone> = ns.iter().enumerate().map(|(i, n)| Tone {
        start: i as f64 * 0.065, dur: if i == 3 { 0.32 } else { 0.11 }, f0: note(*n), f1: note(*n), amp: 0.20, tri: 0.5, attack: 0.004, decay: if i == 3 { 0.12 } else { 0.05 },
    }).collect();
    render(sr, 0.55, &tones, None)
}
pub fn gameover(sr: f64) -> Vec<f32> {
    // drei absteigende Töne (G - E - C), ruhig ausklingend
    let ns = [67, 64, 60];
    let tones: Vec<Tone> = ns.iter().enumerate().map(|(i, n)| Tone {
        start: i as f64 * 0.22, dur: if i == 2 { 0.9 } else { 0.24 }, f0: note(*n), f1: note(*n) * if i == 2 { 0.985 } else { 1.0 },
        amp: 0.26, tri: 0.65, attack: 0.006, decay: if i == 2 { 0.35 } else { 0.12 },
    }).collect();
    render(sr, 1.4, &tones, None)
}

/// Mono-Samples (-1..1) als 16-Bit für den Mixer (auf `ch` Kanäle verteilt)
pub fn to_i16(samples: &[f32], ch: usize) -> Vec<i16> {
    let mut out = Vec::with_capacity(samples.len() * ch);
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 30000.0) as i16;
        for _ in 0..ch { out.push(v); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sounds_sind_sauber() {
        for (name, s) in [("menu_click", menu_click(44100.0)), ("click", click(44100.0)), ("click_back", click_back(44100.0)),
                          ("hit", hit(44100.0)), ("oneup", oneup(44100.0)), ("gameover", gameover(44100.0))] {
            let peak = s.iter().fold(0f32, |m, v| m.max(v.abs()));
            assert!(peak > 0.05 && peak <= 1.0, "{name}: Spitze {peak}");
            // kein Knacken am Ende
            assert!(s.last().unwrap().abs() < 0.02, "{name} endet nicht leise");
        }
    }
}
