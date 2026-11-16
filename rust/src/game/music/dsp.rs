//! Grundbausteine der Klangerzeugung: Wellentabellen (bandbegrenzt), Zufall, Filter, Hall,
//! Echo und Limiter. Alles läuft Sample für Sample (Streaming), damit beim Rendern nur
//! kleine Puffer im Speicher liegen.

use std::f32::consts::{PI, TAU};
use std::sync::OnceLock;

pub const SR: u32 = 44100;
pub const SRF: f32 = SR as f32;

#[inline]
pub fn hz(midi: f32) -> f32 { 440.0 * 2f32.powf((midi - 69.0) / 12.0) }

/// Exponentieller Abfall pro Sample für eine Zeitkonstante in Sekunden
#[inline]
pub fn decay_k(tau_s: f32) -> f32 { if tau_s <= 0.0 { 0.0 } else { (-1.0 / (tau_s * SRF)).exp() } }

/// Gleichmäßige Lautstärke-Panorama-Gewichte (-1 links … 1 rechts)
#[inline]
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * 0.25 * PI;
    (a.cos() * std::f32::consts::SQRT_2, a.sin() * std::f32::consts::SQRT_2)
}

/// Xorshift – schnell und reproduzierbar (jedes Ereignis bekommt einen eigenen Startwert)
#[derive(Clone)]
pub struct Rng(pub u32);
impl Rng {
    pub fn new(seed: u32) -> Rng { Rng(seed.wrapping_mul(2654435761).wrapping_add(0x9E37_79B9) | 1) }
    #[inline]
    pub fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
    #[inline]
    pub fn unit(&mut self) -> f32 { self.next() * 0.5 + 0.5 }
}

// ---------------------------------------------------------------- Wellentabellen

pub const TN: usize = 2048;
const NB: usize = 11;

/// Bandbegrenzte Wellenform: eine Tabelle je Oktave (Band k gilt für Grundtöne bis 40·2^k Hz),
/// damit hohe Töne nicht verzerren (kein Aliasing).
pub struct Wave { bands: Vec<Vec<f32>> }

impl Wave {
    pub fn new(amp: impl Fn(usize) -> f32) -> Wave {
        let mut bands = Vec::with_capacity(NB);
        let mut norm = 0f32;
        for k in 0..NB {
            let fmax = 40.0 * 2f32.powi(k as i32);
            let nh = ((19000.0 / fmax) as usize).clamp(1, 160);
            let coef: Vec<(f32, f32)> = (1..=nh).map(|n| (n as f32, amp(n))).filter(|c| c.1 != 0.0).collect();
            let t: Vec<f32> = (0..TN).map(|i| {
                let p = i as f32 / TN as f32;
                coef.iter().map(|(n, a)| a * (TAU * n * p).sin()).sum()
            }).collect();
            if k == 0 { norm = t.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-6); }
            bands.push(t.into_iter().map(|v| v / norm).collect());
        }
        Wave { bands }
    }
    /// Tabelle passend zur Frequenz
    #[inline]
    pub fn band(&self, f: f32) -> &[f32] {
        let k = if f <= 40.0 { 0 } else { ((f / 40.0).log2().ceil() as usize).min(NB - 1) };
        &self.bands[k]
    }
}

/// Tabellenwert für eine Phase in [0, 1)
#[inline]
pub fn look01(t: &[f32], phase: f32) -> f32 {
    let x = phase * TN as f32;
    let i = x as usize;
    let f = x - i as f32;
    let a = t[i & (TN - 1)];
    let b = t[(i + 1) & (TN - 1)];
    a + (b - a) * f
}

#[inline]
pub fn look(t: &[f32], phase: f32) -> f32 {
    let x = (phase - phase.floor()) * TN as f32;
    let i = x as usize;
    let f = x - i as f32;
    let a = t[i & (TN - 1)];
    let b = t[(i + 1) & (TN - 1)];
    a + (b - a) * f
}

/// Alle Wellenformen, einmal pro Programmlauf berechnet
pub struct Waves {
    pub sine: Wave,
    pub saw: Wave,
    pub square: Wave,
    pub tri: Wave,
    /// Klavier hell / mittel / weich (Hammer-Anschlagpunkt 1/7 → typische Lücken im Spektrum)
    pub piano: [Wave; 3],
    /// weiche Orgel-/Flötenfarbe für ruhige Flächen
    pub soft: Wave,
}

pub fn waves() -> &'static Waves {
    static W: OnceLock<Waves> = OnceLock::new();
    W.get_or_init(|| {
        let strike = |n: usize| (PI * n as f32 / 7.3).sin().abs().max(0.08);
        Waves {
            sine: Wave::new(|n| if n == 1 { 1.0 } else { 0.0 }),
            saw: Wave::new(|n| 1.0 / n as f32),
            square: Wave::new(|n| if n % 2 == 1 { 1.0 / n as f32 } else { 0.0 }),
            tri: Wave::new(|n| if n % 2 == 1 { (if (n / 2) % 2 == 0 { 1.0 } else { -1.0 }) / (n * n) as f32 } else { 0.0 }),
            piano: [
                Wave::new(move |n| if n > 28 { 0.0 } else { strike(n) / (n as f32).powf(1.05) * if n == 2 { 1.25 } else { 1.0 } }),
                Wave::new(move |n| if n > 16 { 0.0 } else { strike(n) / (n as f32).powf(1.7) * if n == 2 { 1.15 } else { 1.0 } }),
                Wave::new(|n| [1.0, 0.38, 0.12, 0.05, 0.02].get(n - 1).copied().unwrap_or(0.0)),
            ],
            soft: Wave::new(|n| [1.0, 0.5, 0.18, 0.1, 0.04, 0.02].get(n - 1).copied().unwrap_or(0.0)),
        }
    })
}

// ---------------------------------------------------------------- Filter

/// Zustandsvariables Filter (TPT/Simper), liefert Tief-, Band- und Hochpass gleichzeitig
#[derive(Clone, Copy, Default)]
pub struct Svf { ic1: f32, ic2: f32, a1: f32, a2: f32, a3: f32, k: f32 }
impl Svf {
    pub fn new(cut: f32, q: f32) -> Svf { let mut s = Svf::default(); s.set(cut, q); s }
    #[inline]
    pub fn set(&mut self, cut: f32, q: f32) {
        let g = (PI * cut.clamp(16.0, SRF * 0.45) / SRF).tan();
        self.k = 1.0 / q.max(0.3);
        self.a1 = 1.0 / (1.0 + g * (g + self.k));
        self.a2 = g * self.a1;
        self.a3 = g * self.a2;
    }
    /// -> (lp, bp, hp)
    #[inline]
    pub fn run(&mut self, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, x - self.k * v1 - v2)
    }
    #[inline] pub fn lp(&mut self, x: f32) -> f32 { self.run(x).0 }
    #[inline] pub fn bp(&mut self, x: f32) -> f32 { self.run(x).1 }
    #[inline] pub fn hp(&mut self, x: f32) -> f32 { self.run(x).2 }
}

/// Einpoliger Tiefpass
#[derive(Clone, Copy, Default)]
pub struct OnePole { z: f32, a: f32 }
impl OnePole {
    pub fn new(cut: f32) -> OnePole { let mut o = OnePole::default(); o.set(cut); o }
    pub fn set(&mut self, cut: f32) { self.a = 1.0 - (-TAU * cut / SRF).exp(); }
    #[inline] pub fn lp(&mut self, x: f32) -> f32 { self.z += (x - self.z) * self.a; self.z }
    #[inline] pub fn hp(&mut self, x: f32) -> f32 { x - self.lp(x) }
}

// ---------------------------------------------------------------- Hall (Freeverb-artig, Stereo)

struct Comb { buf: Vec<f32>, i: usize, filt: f32 }
struct Allpass { buf: Vec<f32>, i: usize }

pub struct Reverb { combs: [Vec<Comb>; 2], alls: [Vec<Allpass>; 2], pre: Vec<f32>, pi: usize, room: f32, damp: f32, hp: [OnePole; 2] }
impl Reverb {
    /// size skaliert die Raumgröße (1 = Freeverb-Standard), predelay in Sekunden
    pub fn new(room: f32, damp: f32, size: f32, predelay: f32) -> Reverb {
        let mk = |spread: usize| -> (Vec<Comb>, Vec<Allpass>) {
            let c = [1116usize, 1188, 1277, 1356, 1422, 1491, 1557, 1617].iter()
                .map(|d| Comb { buf: vec![0.0; ((*d as f32 * size) as usize + spread).max(8)], i: 0, filt: 0.0 }).collect();
            let a = [556usize, 441, 341, 225].iter()
                .map(|d| Allpass { buf: vec![0.0; ((*d as f32 * size) as usize + spread).max(4)], i: 0 }).collect();
            (c, a)
        };
        let (c0, a0) = mk(0);
        let (c1, a1) = mk(23);
        Reverb { combs: [c0, c1], alls: [a0, a1], pre: vec![0.0; ((predelay * SRF) as usize).max(1)], pi: 0, room, damp, hp: [OnePole::new(180.0), OnePole::new(180.0)] }
    }
    #[inline]
    pub fn run(&mut self, l: f32, r: f32) -> (f32, f32) {
        let x = self.pre[self.pi];
        self.pre[self.pi] = (l + r) * 0.5;
        self.pi = (self.pi + 1) % self.pre.len();
        let mut out = [0f32; 2];
        for s in 0..2 {
            let mut acc = 0f32;
            for c in self.combs[s].iter_mut() {
                let y = c.buf[c.i];
                c.filt = y * (1.0 - self.damp) + c.filt * self.damp;
                c.buf[c.i] = x + c.filt * self.room;
                c.i += 1; if c.i == c.buf.len() { c.i = 0; }
                acc += y;
            }
            for a in self.alls[s].iter_mut() {
                let bo = a.buf[a.i];
                let y = -acc + bo;
                a.buf[a.i] = acc + bo * 0.5;
                a.i += 1; if a.i == a.buf.len() { a.i = 0; }
                acc = y;
            }
            // Hall ohne Wummern unten herum
            out[s] = self.hp[s].hp(acc * 0.03);
        }
        (out[0], out[1])
    }
}

// ---------------------------------------------------------------- Echo (Ping-Pong)

pub struct Delay { l: Vec<f32>, r: Vec<f32>, i: usize, d: usize, fb: f32, lp: [OnePole; 2], hp: [OnePole; 2] }
impl Delay {
    pub fn new(delay_s: f32, fb: f32) -> Delay {
        let d = ((delay_s * SRF) as usize).max(1);
        Delay { l: vec![0.0; d], r: vec![0.0; d], i: 0, d, fb, lp: [OnePole::new(5200.0), OnePole::new(5200.0)], hp: [OnePole::new(250.0), OnePole::new(250.0)] }
    }
    #[inline]
    pub fn run(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (yl, yr) = (self.l[self.i], self.r[self.i]);
        // Ping-Pong: links speist rechts und umgekehrt
        let nl = self.hp[0].hp(self.lp[0].lp((l + r) * 0.5 + yr * self.fb));
        let nr = self.hp[1].hp(self.lp[1].lp(yl * self.fb));
        self.l[self.i] = nl;
        self.r[self.i] = nr;
        self.i += 1; if self.i == self.d { self.i = 0; }
        (yl, yr)
    }
}

// ---------------------------------------------------------------- Limiter

/// Begrenzer mit 1,5 ms Vorschau: keine Übersteuerung, kaum hörbar
pub struct Limiter { dl: Vec<f32>, dr: Vec<f32>, i: usize, env: f32, gain: f32, ceil: f32, rel: f32, att: f32 }
impl Limiter {
    pub fn new(ceil: f32) -> Limiter {
        let n = 66;
        Limiter { dl: vec![0.0; n], dr: vec![0.0; n], i: 0, env: 0.0, gain: 1.0, ceil, rel: decay_k(0.12), att: 1.0 - decay_k(0.0003) }
    }
    #[inline]
    pub fn run(&mut self, l: f32, r: f32) -> (f32, f32) {
        let pk = l.abs().max(r.abs());
        self.env = if pk > self.env { pk } else { self.env * self.rel + pk * (1.0 - self.rel) };
        let target = if self.env > self.ceil { self.ceil / self.env } else { 1.0 };
        if target < self.gain { self.gain += (target - self.gain) * self.att; } else { self.gain += (target - self.gain) * (1.0 - self.rel); }
        let (ol, or) = (self.dl[self.i], self.dr[self.i]);
        self.dl[self.i] = l;
        self.dr[self.i] = r;
        self.i += 1; if self.i == self.dl.len() { self.i = 0; }
        let c = self.ceil * 1.02;
        ((ol * self.gain).clamp(-c, c), (or * self.gain).clamp(-c, c))
    }
}

/// Weiche Sättigung: leise Signale unverändert, Spitzen werden rund (Maximum 1/drive)
#[inline]
pub fn sat(x: f32, drive: f32) -> f32 { if drive <= 0.0 { x } else { ftanh(x * drive) / drive } }

/// schnelle tanh-Näherung (Padé), für Sättigung genau genug
#[inline]
pub fn ftanh(x: f32) -> f32 {
    if x > 3.0 { return 1.0; }
    if x < -3.0 { return -1.0; }
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}
