//! Eingebaute Musik: der eigene JumpNDodge-Song „Falling Pixels“ – emotionaler Breakcore
//! (Klavier, zerhackte Breakbeats, Sub-Bass, viel Hall), 174 BPM, d-Moll.
//! Komplett synthetisch erzeugt (eigene Komposition, keine Dateien). Wird beim Start einmal im
//! Hintergrund berechnet. Dazu eine ruhige Klavier-Fassung fürs Menü.

use std::f32::consts::TAU;
use std::sync::OnceLock;

pub const SR: u32 = 44100;
const BPM: f32 = 174.0;
const BEAT: f32 = 60.0 / BPM;
const BAR: f32 = BEAT * 4.0;

static THEME: OnceLock<Vec<i16>> = OnceLock::new();
static CALM: OnceLock<Vec<i16>> = OnceLock::new();

/// Im Hintergrund berechnen (einmal pro Programmstart)
pub fn start_render() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            CALM.get_or_init(|| render(true));
            THEME.get_or_init(|| render(false));
        });
    });
}
/// Spiel-Song (Stereo, 44,1 kHz, i16), sobald fertig
pub fn theme() -> Option<&'static Vec<i16>> { THEME.get() }
/// Menü-Fassung
pub fn calm() -> Option<&'static Vec<i16>> { CALM.get() }

// ---------------------------------------------------------------- Klangbausteine

const TN: usize = 4096;

fn table(harm: &[(f32, f32)]) -> Vec<f32> {
    let mut t: Vec<f32> = (0..TN).map(|i| {
        let p = i as f32 / TN as f32;
        harm.iter().map(|(n, a)| a * (TAU * n * p).sin()).sum()
    }).collect();
    let peak = t.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-6);
    for v in t.iter_mut() { *v /= peak; }
    t
}

#[inline]
fn look(t: &[f32], phase: f32) -> f32 {
    let x = phase.fract() * TN as f32;
    let i = x as usize;
    let f = x - i as f32;
    t[i % TN] * (1.0 - f) + t[(i + 1) % TN] * f
}

fn hz(midi: f32) -> f32 { 440.0 * 2f32.powf((midi - 69.0) / 12.0) }

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> f32 { let mut x = self.0; x ^= x << 13; x ^= x >> 17; x ^= x << 5; self.0 = x; (x as f32 / u32::MAX as f32) * 2.0 - 1.0 }
    fn unit(&mut self) -> f32 { self.next() * 0.5 + 0.5 }
}

struct Tabs { piano_bright: Vec<f32>, piano_soft: Vec<f32>, saw: Vec<f32>, sine: Vec<f32> }
impl Tabs {
    fn new() -> Tabs {
        // Klavier: kräftige 2./3. Obertöne, nach oben weicher; die helle Fassung klingt nach dem Anschlag ab
        let bright: Vec<(f32, f32)> = (1..=12).map(|n| { let n = n as f32; (n, (1.0 / n.powf(1.15)) * if n == 2.0 { 1.3 } else if n == 3.0 { 1.1 } else { 1.0 }) }).collect();
        let soft = vec![(1.0, 1.0), (2.0, 0.42), (3.0, 0.16), (4.0, 0.06)];
        let saw: Vec<(f32, f32)> = (1..=18).map(|n| { let n = n as f32; (n, (1.0 / n) * (-n / 7.0).exp()) }).collect();
        Tabs { piano_bright: table(&bright), piano_soft: table(&soft), saw: table(&saw), sine: table(&[(1.0, 1.0)]) }
    }
}

struct Bus { l: Vec<f32>, r: Vec<f32> }
impl Bus {
    fn new(n: usize) -> Bus { Bus { l: vec![0.0; n], r: vec![0.0; n] } }
    #[inline]
    fn add(&mut self, i: usize, v: f32, pan: f32) {
        if i >= self.l.len() { return; }
        let a = (pan + 1.0) * 0.25 * std::f32::consts::PI;
        self.l[i] += v * a.cos();
        self.r[i] += v * a.sin();
    }
}

/// Klavierton (zwei leicht verstimmte Saiten, Anschlaggeräusch, Ausklang)
fn piano(b: &mut Bus, tb: &Tabs, t0: f32, dur: f32, midi: f32, vel: f32, pan: f32, rng: &mut Rng) {
    let f = hz(midi);
    let decay = 0.55 + ((96.0 - midi) / 30.0).max(0.0);
    let tail = (dur + 1.6).min(5.0);
    let s0 = (t0 * SR as f32) as usize;
    let n = (tail * SR as f32) as usize;
    let (mut p1, mut p2) = (0f32, 0.25f32);
    let (d1, d2) = (f / SR as f32, f * 1.0016 / SR as f32);
    for i in 0..n {
        let t = i as f32 / SR as f32;
        let mut env = (t / 0.003).min(1.0) * (-t / decay).exp();
        if t > dur { env *= (-(t - dur) / 0.22).exp(); }
        if env < 0.0004 && t > 0.05 { break; }
        let bright = (-t / 0.28).exp();
        let s = |p: f32| look(&tb.piano_bright, p) * bright + look(&tb.piano_soft, p) * (1.0 - bright);
        let mut v = s(p1) * 0.6 + s(p2) * 0.4;
        if t < 0.012 { v += rng.next() * 0.25 * (1.0 - t / 0.012); }
        p1 += d1; p2 += d2;
        b.add(s0 + i, v * env * vel, pan);
    }
}

/// Fläche: drei verstimmte Sägezahn-Stimmen, langsamer Einsatz
fn pad(b: &mut Bus, tb: &Tabs, t0: f32, dur: f32, midi: f32, amp: f32) {
    let s0 = (t0 * SR as f32) as usize;
    let n = ((dur + 0.8) * SR as f32) as usize;
    for (k, (cents, pan)) in [(-9.0f32, -0.7f32), (0.0, 0.0), (9.0, 0.7)].iter().enumerate() {
        let f = hz(midi) * 2f32.powf(cents / 1200.0);
        let mut p = k as f32 * 0.31;
        for i in 0..n {
            let t = i as f32 / SR as f32;
            let mut env = (t / 0.45).min(1.0);
            if t > dur { env *= (-(t - dur) / 0.3).exp(); }
            p += f / SR as f32;
            b.add(s0 + i, look(&tb.saw, p) * env * amp, *pan);
        }
    }
}

/// Sub-Bass (Sinus, leicht angezerrt) plus optional „Reese“ (zwei verstimmte Sägen eine Oktave höher)
fn bass(b: &mut Bus, tb: &Tabs, t0: f32, dur: f32, midi: f32, amp: f32, reese: f32) {
    let s0 = (t0 * SR as f32) as usize;
    let n = ((dur + 0.06) * SR as f32) as usize;
    let f = hz(midi);
    let (mut p, mut r1, mut r2, mut lp) = (0f32, 0f32, 0.5f32, 0f32);
    for i in 0..n {
        let t = i as f32 / SR as f32;
        let mut env = (t / 0.006).min(1.0);
        if t > dur { env *= (1.0 - (t - dur) / 0.06).max(0.0); }
        p += f / SR as f32;
        let sub = (look(&tb.sine, p) * 1.6).tanh() / 1.6f32.tanh();
        let mut v = sub * amp;
        if reese > 0.0 {
            r1 += f * 2.0 * 1.007 / SR as f32; r2 += f * 2.0 * 0.993 / SR as f32;
            let raw = (look(&tb.saw, r1) + look(&tb.saw, r2)) * 0.5;
            // Tiefpass mit langsamer Bewegung
            let cut = 0.05 + 0.04 * (TAU * t * 0.7).sin().abs();
            lp += (raw - lp) * cut;
            v += (lp * 2.2).tanh() * reese;
        }
        b.add(s0 + i, v * env, 0.0);
    }
}

fn kick(b: &mut Bus, t0: f32, vel: f32, rng: &mut Rng) {
    let s0 = (t0 * SR as f32) as usize;
    let n = (0.42 * SR as f32) as usize;
    let mut p = 0f32;
    for i in 0..n {
        let t = i as f32 / SR as f32;
        let f = 44.0 + 120.0 * (-t / 0.028).exp();
        p += f / SR as f32;
        let mut v = (TAU * p).sin() * (-t / 0.26).exp();
        if t < 0.004 { v += rng.next() * 0.5 * (1.0 - t / 0.004); }
        b.add(s0 + i, (v * 1.4).tanh() * 0.95 * vel, 0.0);
    }
}

fn snare(b: &mut Bus, t0: f32, vel: f32, pan: f32, rng: &mut Rng) {
    let s0 = (t0 * SR as f32) as usize;
    let n = (0.24 * SR as f32) as usize;
    let (mut p, mut prev, mut bp) = (0f32, 0f32, 0f32);
    for i in 0..n {
        let t = i as f32 / SR as f32;
        p += (180.0 + 60.0 * (-t / 0.01).exp()) / SR as f32;
        let tone = (TAU * p).sin() * (-t / 0.05).exp() * 0.55;
        let w = rng.next();
        let hp = w - prev; prev = w;
        bp += (hp - bp) * 0.78;
        let noise = bp * (-t / 0.09).exp() * 0.9;
        b.add(s0 + i, (tone + noise) * vel, pan);
    }
}

fn hat(b: &mut Bus, t0: f32, vel: f32, open: bool, pan: f32, rng: &mut Rng) {
    let s0 = (t0 * SR as f32) as usize;
    let len = if open { 0.16 } else { 0.035 };
    let n = (len * SR as f32) as usize;
    let (mut a, mut c) = (0f32, 0f32);
    for i in 0..n {
        let t = i as f32 / SR as f32;
        let w = rng.next();
        let h1 = w - a; a = w;
        let h2 = h1 - c; c = h1;
        b.add(s0 + i, h2 * 0.75 * (-t / (len * 0.35)).exp() * vel, pan);
    }
}

/// Rauschen, das über `dur` lauter und heller wird (Übergang zum Drop)
fn riser(b: &mut Bus, t0: f32, dur: f32, amp: f32, rng: &mut Rng) {
    let s0 = (t0 * SR as f32) as usize;
    let n = (dur * SR as f32) as usize;
    let mut lp = 0f32;
    for i in 0..n {
        let x = i as f32 / n as f32;
        lp += (rng.next() - lp) * (0.02 + 0.5 * x * x);
        b.add(s0 + i, lp * x * x * amp, (x * 9.0).sin() * 0.6);
    }
}

/// Hall (Freeverb-artig: 4 Kammfilter + 2 Allpässe je Seite)
fn reverb(inp: &Bus, room: f32, damp: f32) -> Bus {
    let n = inp.l.len();
    let mut out = Bus::new(n);
    for (side, spread) in [(0usize, 0usize), (1, 23)] {
        let src: Vec<f32> = (0..n).map(|i| (inp.l[i] + inp.r[i]) * 0.5).collect();
        let mut acc = vec![0f32; n];
        for &d in &[1557usize, 1617, 1491, 1422, 1277, 1356] {
            let d = d + spread;
            let mut buf = vec![0f32; d];
            let (mut idx, mut filt) = (0usize, 0f32);
            for i in 0..n {
                let y = buf[idx];
                filt = y * (1.0 - damp) + filt * damp;
                buf[idx] = src[i] + filt * room;
                idx = (idx + 1) % d;
                acc[i] += y;
            }
        }
        for &d in &[556usize, 441, 341, 225] {
            let d = d + spread;
            let mut buf = vec![0f32; d];
            let mut idx = 0usize;
            for i in 0..n {
                let bo = buf[idx];
                let y = -acc[i] + bo;
                buf[idx] = acc[i] + bo * 0.5;
                idx = (idx + 1) % d;
                acc[i] = y;
            }
        }
        let o = if side == 0 { &mut out.l } else { &mut out.r };
        for i in 0..n { o[i] = acc[i] * 0.05; }
    }
    out
}

// ---------------------------------------------------------------- Komposition

/// Akkorde (je 2 Takte): Dm – B♭ – F – C. (Grundton auf Oktave 3, Terz klein/groß)
const CHORDS: [(f32, f32); 4] = [(50.0, 3.0), (46.0, 4.0), (53.0, 4.0), (48.0, 4.0)];

/// Melodie A (8 Takte = 32 Schläge): (Start in Schlägen, Länge, MIDI)
const MEL_A: &[(f32, f32, f32)] = &[
    (0.0, 1.5, 81.0), (1.5, 0.5, 79.0), (2.0, 1.0, 77.0), (3.0, 0.5, 76.0), (3.5, 0.5, 77.0),
    (4.0, 2.0, 74.0), (6.0, 1.0, 69.0), (7.0, 1.0, 74.0),
    (8.0, 1.5, 77.0), (9.5, 0.5, 76.0), (10.0, 1.0, 74.0), (11.0, 1.0, 77.0),
    (12.0, 2.0, 82.0), (14.0, 1.0, 81.0), (15.0, 1.0, 79.0),
    (16.0, 1.5, 81.0), (17.5, 0.5, 84.0), (18.0, 1.0, 81.0), (19.0, 1.0, 79.0),
    (20.0, 1.5, 77.0), (21.5, 0.5, 79.0), (22.0, 2.0, 81.0),
    (24.0, 1.0, 79.0), (25.0, 1.0, 76.0), (26.0, 1.0, 72.0), (27.0, 1.0, 76.0),
    (28.0, 1.5, 79.0), (29.5, 0.5, 77.0), (30.0, 2.0, 76.0),
];
/// Melodie B (höher, bewegter) für den zweiten Drop
const MEL_B: &[(f32, f32, f32)] = &[
    (0.0, 1.0, 86.0), (1.0, 0.5, 84.0), (1.5, 0.5, 81.0), (2.0, 1.0, 84.0), (3.0, 1.0, 86.0),
    (4.0, 1.5, 89.0), (5.5, 0.5, 88.0), (6.0, 2.0, 86.0),
    (8.0, 1.0, 86.0), (9.0, 0.5, 84.0), (9.5, 0.5, 82.0), (10.0, 1.0, 81.0), (11.0, 1.0, 82.0),
    (12.0, 2.0, 86.0), (14.0, 1.0, 84.0), (15.0, 1.0, 82.0),
    (16.0, 1.0, 84.0), (17.0, 0.5, 81.0), (17.5, 0.5, 84.0), (18.0, 1.0, 89.0), (19.0, 1.0, 88.0),
    (20.0, 2.0, 84.0), (22.0, 1.0, 81.0), (23.0, 1.0, 84.0),
    (24.0, 1.5, 88.0), (25.5, 0.5, 86.0), (26.0, 1.0, 84.0), (27.0, 1.0, 79.0),
    (28.0, 1.0, 88.0), (29.0, 1.0, 86.0), (30.0, 1.0, 88.0), (31.0, 1.0, 81.0),
];

/// Break-Muster (16 Sechzehntel): 2 = Kick, 1 = Snare, 3 = Ghost-Snare, 0 = nichts
const BREAKS: [[u8; 16]; 4] = [
    [2, 0, 0, 0, 1, 0, 0, 2, 0, 2, 0, 0, 1, 0, 0, 3],
    [2, 0, 2, 0, 1, 0, 0, 3, 0, 3, 2, 0, 1, 0, 3, 0],
    [2, 0, 0, 2, 1, 0, 2, 0, 1, 0, 2, 0, 3, 2, 1, 1],
    [2, 0, 3, 0, 1, 3, 0, 2, 0, 2, 1, 0, 3, 1, 0, 3],
];

#[derive(Clone, Copy, PartialEq)]
enum Part { Intro, Build, DropA, Break, DropB, Outro, Calm }

fn render(calm_version: bool) -> Vec<i16> {
    let parts: Vec<(Part, usize)> = if calm_version {
        vec![(Part::Calm, 16)]
    } else {
        vec![(Part::Intro, 8), (Part::Build, 8), (Part::DropA, 16), (Part::Break, 8), (Part::DropB, 16), (Part::Outro, 8)]
    };
    let bars: usize = parts.iter().map(|p| p.1).sum();
    let len_s = bars as f32 * BAR;
    let tail_s = 4.0;
    let n = ((len_s + tail_s) * SR as f32) as usize;
    let tb = Tabs::new();
    let mut rng = Rng(0x1234_5678);
    let (mut keys, mut drums, mut low) = (Bus::new(n), Bus::new(n), Bus::new(n));
    let mut kicks: Vec<f32> = vec![];

    let mut bar0 = 0usize;
    for (part, count) in &parts {
        for k in 0..*count {
            let bar = bar0 + k;
            let t_bar = bar as f32 * BAR;
            let (root, third) = CHORDS[(bar / 2) % 4];
            let in_part = k as f32 / *count as f32;
            let full = matches!(part, Part::DropA | Part::DropB);

            // Fläche (alle 2 Takte neu)
            if bar % 2 == 0 {
                let pa = match part { Part::Intro | Part::Calm => 0.022, Part::Build => 0.028, Part::Break => 0.03, Part::Outro => 0.024 * (1.0 - in_part * 0.5), _ => 0.03 };
                for m in [root + 12.0, root + 12.0 + third, root + 19.0, root + 24.0] { pad(&mut keys, &tb, t_bar, BAR * 2.0, m, pa); }
            }

            // Klavier-Arpeggio in Achteln
            let arp = [0.0, 7.0, 12.0, 12.0 + third, 19.0, 12.0 + third, 12.0, 7.0];
            let av = match part { Part::Intro => 0.16 + 0.06 * in_part, Part::Calm => 0.17, Part::Build => 0.2, Part::Break => 0.17, Part::Outro => 0.18 * (1.0 - in_part * 0.6), _ => 0.12 };
            for (j, off) in arp.iter().enumerate() {
                let acc = if j == 0 { 1.25 } else if j % 2 == 0 { 1.0 } else { 0.82 };
                let pan = ((j as f32 * 0.9).sin()) * 0.35;
                piano(&mut keys, &tb, t_bar + j as f32 * BEAT * 0.5, BEAT * 0.5, root + off, av * acc, pan, &mut rng);
            }
            // tiefer Bass-Ton des Klaviers auf der Eins
            if bar % 2 == 0 && !full { piano(&mut keys, &tb, t_bar, BAR * 1.8, root - 12.0, av * 1.1, 0.0, &mut rng); }

            // Melodie
            let mel: Option<(&[(f32, f32, f32)], f32)> = match part {
                Part::Build => Some((MEL_A, 0.30)),
                Part::DropA => Some((MEL_A, 0.42)),
                Part::Break => Some((MEL_B, 0.30)),
                Part::DropB => Some((MEL_B, 0.42)),
                Part::Outro if k < 4 => Some((MEL_A, 0.26)),
                Part::Calm if k >= 8 => Some((MEL_A, 0.26)),
                _ => None,
            };
            if let Some((m, vel)) = mel {
                let cyc = (bar % 8) as f32 * 4.0;
                for &(st, du, note) in m.iter().filter(|(st, _, _)| *st >= cyc && *st < cyc + 4.0) {
                    let t = t_bar + (st - cyc) * BEAT;
                    piano(&mut keys, &tb, t, du * BEAT, note, vel, 0.12, &mut rng);
                    // im Drop eine Oktave tiefer gedoppelt - fetter
                    if full { piano(&mut keys, &tb, t, du * BEAT, note - 12.0, vel * 0.45, -0.12, &mut rng); }
                }
            }

            // Bass
            match part {
                Part::DropA | Part::DropB => {
                    let rb = root - 24.0;
                    bass(&mut low, &tb, t_bar, BEAT * 2.4, rb, 0.34, 0.11);
                    bass(&mut low, &tb, t_bar + BEAT * 2.5, BEAT * 1.4, rb, 0.32, 0.11);
                }
                Part::Break => { bass(&mut low, &tb, t_bar, BAR * 0.95, root - 24.0, 0.30, 0.0); }
                Part::Build if k >= 4 => { bass(&mut low, &tb, t_bar, BAR * 0.9, root - 24.0, 0.22 + 0.1 * in_part, 0.0); }
                _ => {}
            }

            // Drums
            let step = BEAT / 4.0;
            match part {
                Part::Build => {
                    if k >= 2 { for b4 in [0.0, 2.0] { let t = t_bar + b4 * BEAT; kick(&mut drums, t, 0.55 + 0.3 * in_part, &mut rng); kicks.push(t); } }
                    if k >= 4 { for s in (2..16).step_by(4) { hat(&mut drums, t_bar + s as f32 * step, 0.5, false, 0.3, &mut rng); } }
                    if k + 1 == *count {
                        // Snare-Wirbel in den Drop: Sechzehntel, dann Zweiunddreißigstel, immer lauter
                        for s in 0..8 { snare(&mut drums, t_bar + s as f32 * step, 0.25 + s as f32 * 0.04, 0.0, &mut rng); }
                        for s in 0..16 { snare(&mut drums, t_bar + BAR * 0.5 + s as f32 * step * 0.5, 0.45 + s as f32 * 0.035, 0.0, &mut rng); }
                    }
                }
                Part::DropA | Part::DropB => {
                    let dense = *part == Part::DropB;
                    let fill = k % 4 == 3;
                    let pat = if fill { 2 + (rng.unit() * 2.0) as usize % 2 } else { (rng.unit() * 2.0) as usize % 2 + if dense && k % 2 == 1 { 2 } else { 0 } };
                    let pat = BREAKS[pat.min(3)];
                    for (s, v) in pat.iter().enumerate() {
                        let t = t_bar + s as f32 * step;
                        // Breakcore: ab und zu ein Schlag als Zweiunddreißigstel doppelt (Stottern)
                        let stutter = dense && rng.unit() < 0.12;
                        match v {
                            2 => { kick(&mut drums, t, 1.0, &mut rng); kicks.push(t); }
                            1 => { snare(&mut drums, t, 0.9, 0.05, &mut rng); if stutter { snare(&mut drums, t + step * 0.5, 0.6, 0.05, &mut rng); } }
                            3 => snare(&mut drums, t, 0.32, -0.15, &mut rng),
                            _ => {}
                        }
                    }
                    let hats = if dense { 16 } else { 8 };
                    for h in 0..hats {
                        let t = t_bar + h as f32 * BAR / hats as f32;
                        let open = h == hats - 2 && !fill;
                        hat(&mut drums, t, if h % 2 == 0 { 0.55 } else { 0.35 }, open, if h % 2 == 0 { 0.35 } else { -0.35 }, &mut rng);
                    }
                    if fill {
                        for s in 0..8 { snare(&mut drums, t_bar + BAR * 0.75 + s as f32 * step * 0.5, 0.4 + s as f32 * 0.06, ((s as f32) * 0.8).sin() * 0.4, &mut rng); }
                    }
                }
                Part::Break => {
                    // halbes Tempo, luftig
                    kick(&mut drums, t_bar, 0.7, &mut rng); kicks.push(t_bar);
                    snare(&mut drums, t_bar + BEAT * 2.0, 0.55, 0.0, &mut rng);
                    for h in 0..8 { hat(&mut drums, t_bar + h as f32 * BEAT * 0.5, 0.28, false, 0.3, &mut rng); }
                    if k + 1 == *count { riser(&mut drums, t_bar, BAR, 0.5, &mut rng); }
                }
                Part::Outro => {
                    if k < 4 { kick(&mut drums, t_bar, 0.6 * (1.0 - in_part), &mut rng); kicks.push(t_bar); }
                }
                Part::Intro => { if k >= 6 { riser(&mut drums, t_bar, BAR, 0.18 * (k as f32 - 5.0), &mut rng); } }
                Part::Calm => {}
            }
            if *part == Part::Build && k >= 4 { riser(&mut drums, t_bar, BAR, 0.12 * (k as f32 - 3.0), &mut rng); }
        }
        bar0 += count;
    }

    // Hall auf Klavier/Fläche (mehr Hall in den ruhigen Teilen ist durch die lauteren Drops überdeckt)
    let wet = reverb(&keys, 0.86, 0.3);
    let drum_room = reverb(&drums, 0.6, 0.45);

    // Sidechain: Bass/Fläche/Klavier ducken kurz bei jeder Kick
    kicks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut ki = 0usize;
    let mut last_kick = -10.0f32;
    let mut out_l = vec![0f32; n];
    let mut out_r = vec![0f32; n];
    for i in 0..n {
        let t = i as f32 / SR as f32;
        while ki < kicks.len() && kicks[ki] <= t { last_kick = kicks[ki]; ki += 1; }
        let since = t - last_kick;
        let duck = 1.0 - 0.55 * (-since / 0.09).exp();
        let duck_keys = 1.0 - 0.25 * (-since / 0.12).exp();
        let k_l = (keys.l[i] + wet.l[i] * 1.1) * duck_keys;
        let k_r = (keys.r[i] + wet.r[i] * 1.1) * duck_keys;
        out_l[i] = k_l + low.l[i] * duck + drums.l[i] * 0.85 + drum_room.l[i] * 0.25;
        out_r[i] = k_r + low.r[i] * duck + drums.r[i] * 0.85 + drum_room.r[i] * 0.25;
    }

    // Nahtlose Schleife: Hallfahne vom Ende an den Anfang falten
    let len = (len_s * SR as f32) as usize;
    for i in 0..(n - len) {
        out_l[i] += out_l[len + i];
        out_r[i] += out_r[len + i];
    }
    out_l.truncate(len);
    out_r.truncate(len);

    // Master: weich begrenzen und auf -1 dB normalisieren
    let drive = 1.3f32;
    for v in out_l.iter_mut().chain(out_r.iter_mut()) { *v = (*v * drive).tanh() / drive.tanh(); }
    let peak = out_l.iter().chain(out_r.iter()).fold(0f32, |m, v| m.max(v.abs())).max(1e-6);
    let g = 0.89 / peak;
    let mut out = Vec::with_capacity(len * 2);
    for i in 0..len {
        out.push((out_l[i] * g * i16::MAX as f32) as i16);
        out.push((out_r[i] * g * i16::MAX as f32) as i16);
    }
    out
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
mod tests {
    use super::*;
    /// Rendert beide Fassungen und prüft Länge/Pegel; JND_MUSIC_OUT=<ordner> schreibt WAVs zum Anhören
    #[test]
    fn song_rendern() {
        for calm in [true, false] {
            let s = render(calm);
            let secs = s.len() as f32 / 2.0 / SR as f32;
            let rms = (s.iter().map(|v| (*v as f32 / 32768.0).powi(2)).sum::<f32>() / s.len() as f32).sqrt();
            let clip = s.iter().filter(|v| v.unsigned_abs() >= 32700).count();
            println!("{}: {:.1} s, RMS {:.3}, Clipping {}", if calm { "Menü" } else { "Spiel" }, secs, rms, clip);
            assert!(secs > 20.0 && rms > 0.03 && clip == 0);
            if let Ok(d) = std::env::var("JND_MUSIC_OUT") {
                let _ = write_wav(&std::path::Path::new(&d).join(if calm { "menue.wav" } else { "spiel.wav" }), &s);
            }
        }
    }
}
