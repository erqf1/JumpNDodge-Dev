//! Instrumente als Bausteine. Jedes Ereignis (`Inst`) wird zu einer Stimme (`Voice`), die
//! blockweise in ihren Bus rendert und sich selbst beendet, wenn sie ausgeklungen ist.
//! Zeiten in Sekunden, Tonhöhen als MIDI-Noten (60 = c4), `vel` ≈ 0..1.

use super::dsp::*;
use std::f32::consts::TAU;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Wv { Saw, Square, Tri, Sine, Soft }

/// Einstellungen für den subtraktiven Synth (Pad, Supersaw, Pluck, Arp, Streicher)
#[derive(Clone, Copy, Debug)]
pub struct SynthP {
    pub wave: Wv,
    pub uni: usize,
    /// Verstimmung der äußeren Stimmen in Cent
    pub det: f32,
    /// Stereo-Breite der Stimmen
    pub spread: f32,
    pub a: f32, pub d: f32, pub s: f32, pub r: f32,
    /// Grund-Grenzfrequenz, Hüllkurven-Anteil (Hz) und dessen Abfallzeit
    pub cut: f32, pub env_cut: f32, pub cut_tau: f32, pub q: f32,
    /// langsame Filterbewegung (Hz, Anteil)
    pub lfo: f32, pub lfo_amt: f32,
    pub vib: f32,
    pub drive: f32,
    /// Oktave tiefer dazumischen
    pub sub: f32,
}

impl SynthP {
    pub const fn pad() -> SynthP {
        SynthP { wave: Wv::Saw, uni: 5, det: 14.0, spread: 0.9, a: 0.5, d: 1.0, s: 0.9, r: 0.7, cut: 1600.0, env_cut: 600.0, cut_tau: 1.2, q: 0.7, lfo: 0.15, lfo_amt: 0.25, vib: 0.0, drive: 0.0, sub: 0.0 }
    }
    pub const fn supersaw() -> SynthP {
        SynthP { wave: Wv::Saw, uni: 7, det: 22.0, spread: 1.0, a: 0.006, d: 0.4, s: 0.75, r: 0.18, cut: 3800.0, env_cut: 3500.0, cut_tau: 0.25, q: 0.8, lfo: 0.0, lfo_amt: 0.0, vib: 6.0, drive: 0.0, sub: 0.0 }
    }
    pub const fn pluck() -> SynthP {
        SynthP { wave: Wv::Square, uni: 2, det: 8.0, spread: 0.5, a: 0.002, d: 0.16, s: 0.0, r: 0.08, cut: 700.0, env_cut: 5200.0, cut_tau: 0.09, q: 1.1, lfo: 0.0, lfo_amt: 0.0, vib: 0.0, drive: 0.0, sub: 0.0 }
    }
    pub const fn strings() -> SynthP {
        SynthP { wave: Wv::Saw, uni: 3, det: 9.0, spread: 0.8, a: 0.25, d: 0.8, s: 0.85, r: 0.45, cut: 2600.0, env_cut: 500.0, cut_tau: 0.6, q: 0.7, lfo: 0.0, lfo_amt: 0.0, vib: 11.0, drive: 0.0, sub: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub enum Inst {
    Piano { midi: f32, dur: f32, vel: f32, pedal: f32 },
    /// FM-Glocke: ratio = Modulator/Träger, index = Helligkeit
    Bell { midi: f32, dur: f32, vel: f32, ratio: f32, index: f32, decay: f32 },
    /// gezupfte Saite (Karplus-Strong)
    Pluck { midi: f32, vel: f32, bright: f32, decay: f32 },
    Synth { midi: f32, dur: f32, vel: f32, p: SynthP },
    /// Chor „aah“ (vowel 0) bis „ooh“ (1)
    Choir { midi: f32, dur: f32, vel: f32, vowel: f32 },
    Sub { midi: f32, dur: f32, vel: f32, from: f32, glide: f32 },
    Reese { midi: f32, dur: f32, vel: f32, lfo: f32, cmin: f32, cmax: f32, drive: f32 },
    Kick { vel: f32, tone: f32, decay: f32, click: f32 },
    Snare { vel: f32, tone: f32, decay: f32 },
    Clap { vel: f32 },
    Hat { vel: f32, open: bool },
    Cymbal { vel: f32, decay: f32, ride: bool },
    /// tiefe Trommel (Taiko/Tom)
    Tom { midi: f32, vel: f32, decay: f32 },
    Riser { dur: f32, vel: f32, pitched: bool },
    Down { dur: f32, vel: f32 },
    Impact { vel: f32 },
    /// Rauschfläche (Regen, Wind): Bandpass um `cut`, langsame Bewegung
    Air { dur: f32, vel: f32, cut: f32, q: f32, drops: f32 },
    /// vorberechneter Stereo-Puffer (Break, rückwärts …): Start in Sekunden im Puffer,
    /// Länge in Sekunden Ausgabe, rate = Tempo/Tonhöhe
    Sample { id: usize, vel: f32, rate: f32, start: f32, len: f32, reverse: bool },
}

pub trait Voice: Send {
    /// addiert in l/r; false = fertig
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool;
}

pub struct Ctx<'a> { pub samples: &'a [Arc<Vec<f32>>] }

pub fn make_voice(inst: &Inst, pan: f32, seed: u32, ctx: &Ctx) -> Box<dyn Voice> {
    let w = waves();
    let rng = Rng::new(seed);
    match *inst {
        Inst::Piano { midi, dur, vel, pedal } => Box::new(Piano::new(w, midi, dur, vel, pedal, pan, rng)),
        Inst::Bell { midi, dur, vel, ratio, index, decay } => Box::new(Bell::new(w, midi, dur, vel, ratio, index, decay, pan)),
        Inst::Pluck { midi, vel, bright, decay } => Box::new(Pluck::new(midi, vel, bright, decay, pan, rng)),
        Inst::Synth { midi, dur, vel, p } => Box::new(Synth::new(w, midi, dur, vel, p, pan, rng)),
        Inst::Choir { midi, dur, vel, vowel } => Box::new(Choir::new(w, midi, dur, vel, vowel, pan, rng)),
        Inst::Sub { midi, dur, vel, from, glide } => Box::new(Sub::new(w, midi, dur, vel, from, glide)),
        Inst::Reese { midi, dur, vel, lfo, cmin, cmax, drive } => Box::new(Reese::new(w, midi, dur, vel, lfo, cmin, cmax, drive)),
        Inst::Kick { vel, tone, decay, click } => Box::new(Fn1::kick(vel, tone, decay, click, rng)),
        Inst::Snare { vel, tone, decay } => Box::new(Fn1::snare(vel, tone, decay, pan, rng)),
        Inst::Clap { vel } => Box::new(Fn1::clap(vel, pan, rng)),
        Inst::Hat { vel, open } => Box::new(Fn1::hat(vel, open, pan, rng)),
        Inst::Cymbal { vel, decay, ride } => Box::new(Cymbal::new(w, vel, decay, ride, pan, rng)),
        Inst::Tom { midi, vel, decay } => Box::new(Fn1::tom(midi, vel, decay, pan, rng)),
        Inst::Riser { dur, vel, pitched } => Box::new(Sweep::new(w, dur, vel, true, pitched, rng)),
        Inst::Down { dur, vel } => Box::new(Sweep::new(w, dur, vel, false, false, rng)),
        Inst::Impact { vel } => Box::new(Fn1::impact(vel, rng)),
        Inst::Air { dur, vel, cut, q, drops } => Box::new(Air::new(dur, vel, cut, q, drops, rng)),
        Inst::Sample { id, vel, rate, start, len, reverse } => Box::new(Sampler::new(ctx.samples[id].clone(), vel, rate, start, len, reverse, pan)),
    }
}

/// Eine Stimme komplett in einen Stereo-Puffer rendern (für Rückwärts-Effekte und den Break)
pub fn render_offline(inst: &Inst, pan: f32, seed: u32, secs: f32) -> Vec<f32> {
    let n = (secs * SRF) as usize;
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    let mut v = make_voice(inst, pan, seed, &Ctx { samples: &[] });
    let mut i = 0;
    while i < n {
        let e = (i + 256).min(n);
        if !v.render(&mut l[i..e], &mut r[i..e]) { break; }
        i = e;
    }
    l.iter().zip(r.iter()).flat_map(|(a, b)| [*a, *b]).collect()
}

fn wave_of(w: &'static Waves, k: Wv) -> &'static Wave {
    match k { Wv::Saw => &w.saw, Wv::Square => &w.square, Wv::Tri => &w.tri, Wv::Sine => &w.sine, Wv::Soft => &w.soft }
}

// ---------------------------------------------------------------- Klavier

struct Piano {
    t: [&'static [f32]; 3], ph: [f32; 2], dp: [f32; 2],
    e1: f32, e2: f32, k1: f32, k2: f32, b: f32, kb: f32,
    n: usize, dur: usize, rel: f32, krel: f32, gl: f32, gr: f32, rng: Rng, vel: f32, nz: OnePole, att: f32,
}
impl Piano {
    fn new(w: &'static Waves, midi: f32, dur: f32, vel: f32, pedal: f32, pan: f32, rng: Rng) -> Piano {
        let f = hz(midi);
        let low = ((72.0 - midi) / 24.0).clamp(-1.0, 1.5);
        let d1 = 0.16 + 0.1 * low.max(0.0);
        let d2 = (5.0 * 2f32.powf(-(midi - 48.0) / 17.0)).clamp(0.9, 6.0);
        let pn = (((midi - 64.0) / 30.0).clamp(-0.5, 0.5) * 0.6 + pan).clamp(-1.0, 1.0);
        let (gl, gr) = pan_gains(pn);
        let det = 1.0 + 0.0009 + 0.0006 * low.max(0.0);
        Piano {
            t: [w.piano[0].band(f), w.piano[1].band(f), w.piano[2].band(f * 2.0)],
            ph: [0.0, 0.37], dp: [f / SRF, f * det / SRF],
            e1: 0.6, e2: 0.4, k1: decay_k(d1), k2: decay_k(d2),
            b: (0.45 + 0.5 * vel).min(1.0), kb: decay_k(0.3 + 0.25 * low.max(0.0)),
            n: 0, dur: (dur.max(0.02) * SRF) as usize, rel: 1.0, krel: decay_k(if pedal > 0.0 { pedal } else { 0.13 }),
            gl, gr, rng, vel, nz: OnePole::new((f * 6.0).min(9000.0)), att: 0.0,
        }
    }
}
impl Voice for Piano {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let [tb, tm, ts] = self.t;
        for i in 0..l.len() {
            let b = self.b;
            let tone = |p: f32| if b > 0.5 { let x = (b - 0.5) * 2.0; look01(tm, p) + (look01(tb, p) - look01(tm, p)) * x } else { let x = b * 2.0; look01(ts, p) + (look01(tm, p) - look01(ts, p)) * x };
            let mut v = tone(self.ph[0]) * 0.55 + tone(self.ph[1]) * 0.45;
            self.ph[0] += self.dp[0]; self.ph[1] += self.dp[1];
            if self.ph[0] > 1.0 { self.ph[0] -= 1.0; }
            if self.ph[1] > 1.0 { self.ph[1] -= 1.0; }
            if self.n < 400 { v += self.nz.lp(self.rng.next()) * 0.35 * (1.0 - self.n as f32 / 400.0); }
            if self.att < 1.0 { self.att = (self.att + 1.0 / 90.0).min(1.0); }
            let env = (self.e1 + self.e2) * self.att * self.rel;
            let o = v * env * self.vel * 0.5;
            l[i] += o * self.gl;
            r[i] += o * self.gr;
            self.e1 *= self.k1; self.e2 *= self.k2; self.b *= self.kb;
            if self.n >= self.dur { self.rel *= self.krel; }
            self.n += 1;
            if (env < 0.0008 && self.n > 2000) || self.n > (SRF * 6.0) as usize { return false; }
        }
        true
    }
}

// ---------------------------------------------------------------- Glocke (FM)

struct Bell { s: &'static [f32], pc: [f32; 2], pm: f32, dc: [f32; 2], dm: f32, idx: f32, kidx: f32, env: f32, k: f32, n: usize, dur: usize, krel: f32, vel: f32, gl: f32, gr: f32 }
impl Bell {
    #[allow(clippy::too_many_arguments)]
    fn new(w: &'static Waves, midi: f32, dur: f32, vel: f32, ratio: f32, index: f32, decay: f32, pan: f32) -> Bell {
        let f = hz(midi);
        let (gl, gr) = pan_gains(pan);
        Bell { s: &w.sine.band(1.0)[..], pc: [0.0, 0.5], pm: 0.0, dc: [f / SRF, f * 1.0021 / SRF], dm: f * ratio / SRF, idx: index, kidx: decay_k(decay * 0.25), env: 1.0, k: decay_k(decay), n: 0, dur: (dur * SRF) as usize, krel: decay_k(0.35), vel, gl, gr }
    }
}
impl Voice for Bell {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        for i in 0..l.len() {
            let m = look(self.s, self.pm) * (self.idx * 0.75 + 0.25 * self.idx.min(1.0)) / TAU;
            let a = if self.n < 60 { self.n as f32 / 60.0 } else { 1.0 };
            let e = self.env * a * self.vel * 0.32;
            l[i] += look(self.s, self.pc[0] + m) * e * self.gl;
            r[i] += look(self.s, self.pc[1] + m) * e * self.gr;
            self.pc[0] += self.dc[0]; self.pc[1] += self.dc[1]; self.pm += self.dm;
            if self.pc[0] > 1.0 { self.pc[0] -= 1.0; }
            if self.pc[1] > 1.0 { self.pc[1] -= 1.0; }
            if self.pm > 1.0 { self.pm -= 1.0; }
            self.env *= if self.n >= self.dur { self.krel * self.k } else { self.k };
            self.idx *= self.kidx;
            self.n += 1;
            if self.env < 0.0003 { return false; }
        }
        true
    }
}

// ---------------------------------------------------------------- Zupfsaite

struct Pluck { buf: Vec<f32>, i: usize, prev: f32, fb: f32, n: usize, max: usize, gl: f32, gr: f32, vel: f32 }
impl Pluck {
    fn new(midi: f32, vel: f32, bright: f32, decay: f32, pan: f32, mut rng: Rng) -> Pluck {
        let f = hz(midi);
        let len = ((SRF / f - 0.5).round() as usize).max(2);
        let mut lp = OnePole::new(1000.0 + 9000.0 * bright.clamp(0.0, 1.0));
        let buf: Vec<f32> = (0..len).map(|_| lp.lp(rng.next())).collect();
        let fb = (-(1000f32.ln()) / (f * decay.max(0.05))).exp();
        let (gl, gr) = pan_gains(pan);
        Pluck { buf, i: 0, prev: 0.0, fb, n: 0, max: (decay * 1.2 * SRF) as usize, gl, gr, vel }
    }
}
impl Voice for Pluck {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let len = self.buf.len();
        for i in 0..l.len() {
            let y = self.buf[self.i];
            let nv = (y + self.prev) * 0.5 * self.fb;
            self.prev = y;
            self.buf[self.i] = nv;
            self.i += 1; if self.i == len { self.i = 0; }
            let fade = if self.n + 2000 > self.max { (self.max - self.n) as f32 / 2000.0 } else { 1.0 };
            let o = y * self.vel * 0.6 * fade;
            l[i] += o * self.gl;
            r[i] += o * self.gr;
            self.n += 1;
            if self.n >= self.max { return false; }
        }
        true
    }
}

// ---------------------------------------------------------------- Synth

struct Synth {
    t: &'static [f32], ts: &'static [f32], ph: Vec<f32>, dp: Vec<f32>, pg: Vec<(f32, f32)>, sub_ph: f32,
    p: SynthP, f: [Svf; 2], n: usize, dur: usize, env: f32, rel: bool, vel: f32, kd: f32, kr: f32, kc: f32, ce: f32, gl: f32, gr: f32, lfo_ph: f32, norm: f32, vibf: f32,
}
impl Synth {
    fn new(w: &'static Waves, midi: f32, dur: f32, vel: f32, p: SynthP, pan: f32, mut rng: Rng) -> Synth {
        let f = hz(midi);
        let wv = wave_of(w, p.wave);
        let u = p.uni.max(1);
        let mut ph = vec![]; let mut dp = vec![]; let mut pg = vec![];
        for k in 0..u {
            let x = if u == 1 { 0.0 } else { k as f32 / (u - 1) as f32 * 2.0 - 1.0 };
            ph.push(rng.unit());
            dp.push(f * 2f32.powf(x * p.det / 1200.0) / SRF);
            pg.push(pan_gains((x * p.spread + pan).clamp(-1.0, 1.0)));
        }
        let (gl, gr) = (1.0, 1.0);
        Synth {
            t: wv.band(f * 1.02), ts: w.sine.band(1.0), ph, dp, pg, sub_ph: 0.0, p,
            f: [Svf::new(p.cut, p.q), Svf::new(p.cut, p.q)], n: 0, dur: (dur * SRF) as usize, env: 0.0, rel: false, vel,
            kd: decay_k(p.d), kr: decay_k(p.r / 3.0), kc: decay_k(p.cut_tau), ce: 1.0, gl, gr, lfo_ph: rng.unit(), norm: 1.0 / (u as f32).sqrt(), vibf: 1.0,
        }
    }
}
impl Voice for Synth {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let p = self.p;
        let a_n = (p.a * SRF).max(1.0);
        for i in 0..l.len() {
            if self.n % 32 == 0 {
                let lfo = 1.0 + p.lfo_amt * (TAU * self.lfo_ph).sin();
                self.lfo_ph += p.lfo * 32.0 / SRF;
                let c = (p.cut + p.env_cut * self.ce) * lfo;
                self.f[0].set(c, p.q); self.f[1].set(c, p.q);
                if p.vib > 0.0 { self.vibf = 1.0 + (TAU * self.n as f32 / SRF * 5.3).sin() * p.vib / 1731.0 * (self.n as f32 / SRF / 0.4).min(1.0); }
            }
            // Hüllkurve
            if self.n >= self.dur { self.rel = true; }
            if self.rel { self.env *= self.kr; }
            else if (self.n as f32) < a_n { self.env = self.n as f32 / a_n; }
            else { self.env = p.s + (self.env - p.s) * self.kd; }
            self.ce *= self.kc;
            let vib = self.vibf;
            let (mut sl, mut sr) = (0f32, 0f32);
            for k in 0..self.ph.len() {
                let v = look01(self.t, self.ph[k]);
                self.ph[k] += self.dp[k] * vib;
                if self.ph[k] > 1.0 { self.ph[k] -= 1.0; }
                sl += v * self.pg[k].0; sr += v * self.pg[k].1;
            }
            if p.sub > 0.0 {
                let s = look01(self.ts, self.sub_ph) * p.sub;
                self.sub_ph += self.dp[0] * 0.5 * vib;
                if self.sub_ph > 1.0 { self.sub_ph -= 1.0; }
                sl += s; sr += s;
            }
            let mut ol = self.f[0].lp(sl * self.norm);
            let mut or = self.f[1].lp(sr * self.norm);
            if p.drive > 0.0 { ol = sat(ol, p.drive); or = sat(or, p.drive); }
            let e = self.env * self.vel * 0.3;
            l[i] += ol * e * self.gl;
            r[i] += or * e * self.gr;
            self.n += 1;
            if self.rel && self.env < 0.002 { return false; }
        }
        true
    }
}

// ---------------------------------------------------------------- Chor

struct Choir { t: &'static [f32], ph: [f32; 3], dp: [f32; 3], vr: [f32; 3], vf: [f32; 3], fl: [Svf; 3], fr: [Svf; 3], g: [f32; 3], n: usize, dur: usize, env: f32, a: f32, kr: f32, vel: f32, rng: Rng, gl: f32, gr: f32 }
impl Choir {
    fn new(w: &'static Waves, midi: f32, dur: f32, vel: f32, vowel: f32, pan: f32, rng: Rng) -> Choir {
        let f = hz(midi);
        let v = vowel.clamp(0.0, 1.0);
        let fm = [800.0 + (430.0 - 800.0) * v, 1150.0 + (820.0 - 1150.0) * v, 2800.0];
        let qs = [5.0, 7.0, 9.0];
        let mk = || [Svf::new(fm[0], qs[0]), Svf::new(fm[1], qs[1]), Svf::new(fm[2], qs[2])];
        let (gl, gr) = pan_gains(pan);
        Choir {
            t: w.saw.band(f), ph: [0.0, 0.33, 0.66], dp: [f * 0.9945 / SRF, f / SRF, f * 1.0055 / SRF], vr: [4.9, 5.4, 5.8], vf: [1.0; 3],
            fl: mk(), fr: mk(), g: [1.0, 0.55 - 0.25 * v, 0.22 - 0.1 * v], n: 0, dur: (dur * SRF) as usize, env: 0.0,
            a: 1.0 / (0.28 * SRF), kr: decay_k(0.22), vel, rng, gl, gr,
        }
    }
}
impl Voice for Choir {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        for i in 0..l.len() {
            if self.n % 32 == 0 {
                let t = self.n as f32 / SRF;
                for k in 0..3 { self.vf[k] = 1.0 + (TAU * self.vr[k] * t + k as f32).sin() * 0.0035; }
            }
            let mut s = [0f32; 3];
            for k in 0..3 {
                s[k] = look01(self.t, self.ph[k]);
                self.ph[k] += self.dp[k] * self.vf[k];
                if self.ph[k] > 1.0 { self.ph[k] -= 1.0; }
            }
            let breath = self.rng.next() * 0.12;
            let xl = s[0] + s[1] * 0.6 + breath;
            let xr = s[2] + s[1] * 0.6 + breath;
            let (mut ol, mut or) = (0f32, 0f32);
            for k in 0..3 { ol += self.fl[k].bp(xl) * self.g[k]; or += self.fr[k].bp(xr) * self.g[k]; }
            if self.n >= self.dur { self.env *= self.kr; } else { self.env = (self.env + self.a).min(1.0); }
            let e = self.env * self.vel * 0.55;
            l[i] += ol * e * self.gl;
            r[i] += or * e * self.gr;
            self.n += 1;
            if self.n > self.dur && self.env < 0.002 { return false; }
        }
        true
    }
}

// ---------------------------------------------------------------- Bässe

struct Sub { t: &'static [f32], ph: f32, f: f32, f1: f32, kg: f32, n: usize, dur: usize, vel: f32, env: f32 }
impl Sub {
    fn new(w: &'static Waves, midi: f32, dur: f32, vel: f32, from: f32, glide: f32) -> Sub {
        let f1 = hz(midi);
        let f0 = if from > 0.0 { hz(from) } else { f1 };
        Sub { t: w.sine.band(1.0), ph: 0.0, f: f0, f1, kg: if glide > 0.0 { decay_k(glide) } else { 0.0 }, n: 0, dur: (dur * SRF) as usize, vel, env: 0.0 }
    }
}
impl Voice for Sub {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        for i in 0..l.len() {
            self.f = self.f1 + (self.f - self.f1) * self.kg;
            self.ph += self.f / SRF;
            if self.ph > 1.0 { self.ph -= 1.0; }
            if self.n < self.dur { self.env = (self.env + 1.0 / 130.0).min(1.0); } else { self.env -= 1.0 / 1100.0; }
            if self.env <= 0.0 && self.n >= self.dur { return false; }
            let v = sat(look01(self.t, self.ph), 1.4) * 1.25 * self.env.max(0.0) * self.vel * 0.5;
            l[i] += v; r[i] += v;
            self.n += 1;
        }
        true
    }
}

struct Reese { t: &'static [f32], ph: [f32; 3], dp: [f32; 3], fl: Svf, fr: Svf, hl: Svf, hr: Svf, lfo: f32, cmin: f32, cmax: f32, drive: f32, n: usize, dur: usize, vel: f32, env: f32 }
impl Reese {
    #[allow(clippy::too_many_arguments)]
    fn new(w: &'static Waves, midi: f32, dur: f32, vel: f32, lfo: f32, cmin: f32, cmax: f32, drive: f32) -> Reese {
        let f = hz(midi);
        Reese {
            t: w.saw.band(f), ph: [0.0, 0.31, 0.67], dp: [f * 0.9925 / SRF, f / SRF, f * 1.0081 / SRF],
            fl: Svf::new(cmin, 1.3), fr: Svf::new(cmin, 1.3), hl: Svf::new(110.0, 0.7), hr: Svf::new(110.0, 0.7),
            lfo, cmin, cmax, drive, n: 0, dur: (dur * SRF) as usize, vel, env: 0.0,
        }
    }
}
impl Voice for Reese {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        for i in 0..l.len() {
            if self.n % 32 == 0 {
                let x = 0.5 - 0.5 * (TAU * self.lfo * self.n as f32 / SRF).cos();
                let c = self.cmin * (self.cmax / self.cmin).powf(x);
                self.fl.set(c, 1.3); self.fr.set(c * 1.06, 1.3);
            }
            let s: [f32; 3] = [look01(self.t, self.ph[0]), look01(self.t, self.ph[1]), look01(self.t, self.ph[2])];
            for k in 0..3 { self.ph[k] += self.dp[k]; if self.ph[k] > 1.0 { self.ph[k] -= 1.0; } }
            let ol = self.hl.hp(sat(self.fl.lp(s[0] + s[1] * 0.7), self.drive));
            let or = self.hr.hp(sat(self.fr.lp(s[2] + s[1] * 0.7), self.drive));
            if self.n < self.dur { self.env = (self.env + 1.0 / 220.0).min(1.0); } else { self.env -= 1.0 / 1800.0; }
            if self.env <= 0.0 && self.n >= self.dur { return false; }
            let e = self.env.max(0.0) * self.vel * 0.5;
            l[i] += ol * e; r[i] += or * e;
            self.n += 1;
        }
        true
    }
}

// ---------------------------------------------------------------- Schlagzeug (als Funktion der Zeit)

/// Kurze Klänge, deren Sample nur von der Zeit abhängt (plus Zufall/Filterzustand)
struct Fn1 { kind: u8, n: usize, max: usize, vel: f32, p: [f32; 4], ph: f32, rng: Rng, f: [Svf; 2], z: [f32; 2], gl: f32, gr: f32 }
impl Fn1 {
    fn base(kind: u8, secs: f32, vel: f32, pan: f32, rng: Rng) -> Fn1 {
        let (gl, gr) = pan_gains(pan);
        Fn1 { kind, n: 0, max: (secs * SRF) as usize, vel, p: [0.0; 4], ph: 0.0, rng, f: [Svf::new(1000.0, 0.7); 2], z: [0.0; 2], gl, gr }
    }
    fn kick(vel: f32, tone: f32, decay: f32, click: f32, rng: Rng) -> Fn1 { let mut f = Fn1::base(0, decay * 2.2 + 0.05, vel, 0.0, rng); f.p = [tone, decay, click, 0.0]; f }
    fn snare(vel: f32, tone: f32, decay: f32, pan: f32, rng: Rng) -> Fn1 {
        let mut f = Fn1::base(1, decay * 2.6 + 0.04, vel, pan, rng); f.p = [tone, decay, 0.0, 0.0];
        f.f = [Svf::new(3800.0, 0.6), Svf::new(900.0, 0.7)]; f
    }
    fn clap(vel: f32, pan: f32, rng: Rng) -> Fn1 { let mut f = Fn1::base(2, 0.45, vel, pan, rng); f.f = [Svf::new(1350.0, 1.4), Svf::new(1000.0, 0.7)]; f }
    fn hat(vel: f32, open: bool, pan: f32, rng: Rng) -> Fn1 {
        let mut f = Fn1::base(3, if open { 0.42 } else { 0.06 }, vel, pan, rng);
        f.p = [if open { 0.13 } else { 0.014 }, 0.0, 0.0, 0.0]; f.f = [Svf::new(7600.0, 0.8), Svf::new(1000.0, 0.7)]; f
    }
    fn tom(midi: f32, vel: f32, decay: f32, pan: f32, rng: Rng) -> Fn1 { let mut f = Fn1::base(4, decay * 2.5 + 0.05, vel, pan, rng); f.p = [hz(midi), decay, 0.0, 0.0]; f.f = [Svf::new(600.0, 0.8), Svf::new(1000.0, 0.7)]; f }
    fn impact(vel: f32, rng: Rng) -> Fn1 { let mut f = Fn1::base(5, 3.2, vel, 0.0, rng); f.f = [Svf::new(2600.0, 0.7), Svf::new(2600.0, 0.7)]; f }
}
impl Voice for Fn1 {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        for i in 0..l.len() {
            if self.n >= self.max { return false; }
            let t = self.n as f32 / SRF;
            let (vl, vr) = match self.kind {
                0 => {
                    let f = 46.0 + self.p[0] * (-t / 0.03).exp();
                    self.ph += f / SRF;
                    let mut v = (TAU * self.ph).sin() * (-t / self.p[1]).exp();
                    if t < 0.005 { v += self.rng.next() * self.p[2] * (1.0 - t / 0.005); }
                    let v = sat(v * 1.3, 1.6) * 0.95;
                    (v, v)
                }
                1 => {
                    self.ph += (self.p[0] + 70.0 * (-t / 0.012).exp()) / SRF;
                    let tone = ((TAU * self.ph).sin() + 0.45 * (TAU * self.ph * 1.78).sin()) * (-t / (self.p[1] * 0.55)).exp() * 0.5;
                    let nz = self.rng.next();
                    let hp = self.f[1].hp(nz);
                    let nb = self.f[0].lp(hp) * (-t / self.p[1]).exp() * 1.1;
                    let v = tone + nb;
                    (v, v)
                }
                2 => {
                    let burst = |d: f32| if t >= d { (-(t - d) / 0.0045).exp() } else { 0.0 };
                    let env = burst(0.0) * 0.8 + burst(0.009) * 0.9 + burst(0.019) + if t > 0.019 { (-(t - 0.019) / 0.11).exp() * 0.55 } else { 0.0 };
                    let v = self.f[0].bp(self.rng.next()) * env * 2.2;
                    (v, v)
                }
                3 => {
                    let w = self.rng.next();
                    let h1 = w - self.z[0]; self.z[0] = w;
                    let h2 = h1 - self.z[1]; self.z[1] = h1;
                    let v = self.f[0].hp(h2) * 0.8 * (-t / self.p[0]).exp();
                    (v, v)
                }
                4 => {
                    let f = self.p[0] * (1.0 + 0.6 * (-t / 0.04).exp());
                    self.ph += f / SRF;
                    let nz = self.f[0].lp(self.rng.next()) * (-t / 0.03).exp() * 0.5;
                    let v = sat(((TAU * self.ph).sin() * 1.2 + nz) * (-t / self.p[1]).exp(), 1.3);
                    (v, v)
                }
                _ => {
                    let f = 30.0 + 52.0 * (-t / 0.25).exp();
                    self.ph += f / SRF;
                    let boom = (TAU * self.ph).sin() * (-t / 1.1).exp() * 1.2;
                    let nl = self.f[0].lp(self.rng.next()) * (-t / 0.22).exp() * 0.7;
                    let nr = self.f[1].lp(self.rng.next()) * (-t / 0.22).exp() * 0.7;
                    (sat(boom + nl, 1.2), sat(boom + nr, 1.2))
                }
            };
            l[i] += vl * self.vel * self.gl;
            r[i] += vr * self.vel * self.gr;
            self.n += 1;
        }
        true
    }
}

// ---------------------------------------------------------------- Becken

struct Cymbal { t: &'static [f32], ph: [f32; 6], dp: [f32; 6], fl: Svf, fr: Svf, n: usize, max: usize, k: f32, env: f32, vel: f32, ride: bool, rng: Rng, gl: f32, gr: f32 }
impl Cymbal {
    fn new(w: &'static Waves, vel: f32, decay: f32, ride: bool, pan: f32, rng: Rng) -> Cymbal {
        let base = if ride { 520.0 } else { 410.0 };
        let rat = [1.0, 1.4826, 1.8004, 2.5464, 2.6305, 3.8968];
        let mut dp = [0f32; 6];
        for k in 0..6 { dp[k] = base * rat[k] / SRF; }
        let (gl, gr) = pan_gains(pan);
        Cymbal { t: w.square.band(base * 4.0), ph: [0.0, 0.1, 0.2, 0.3, 0.4, 0.5], dp, fl: Svf::new(if ride { 5200.0 } else { 6800.0 }, 0.7), fr: Svf::new(if ride { 5200.0 } else { 7000.0 }, 0.7), n: 0, max: (decay * 5.0 * SRF) as usize, k: decay_k(decay), env: 1.0, vel, ride, rng, gl, gr }
    }
}
impl Voice for Cymbal {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        for i in 0..l.len() {
            let mut m = 0f32;
            for k in 0..6 { m += look01(self.t, self.ph[k]); self.ph[k] += self.dp[k]; if self.ph[k] > 1.0 { self.ph[k] -= 1.0; } }
            let metal = m * if self.ride { 0.16 } else { 0.1 };
            let a = (self.n as f32 / 40.0).min(1.0);
            let ol = self.fl.hp(metal + self.rng.next() * 0.5) * self.env * a;
            let or = self.fr.hp(metal + self.rng.next() * 0.5) * self.env * a;
            l[i] += ol * self.vel * 0.5 * self.gl;
            r[i] += or * self.vel * 0.5 * self.gr;
            self.env *= self.k;
            self.n += 1;
            if self.n >= self.max { return false; }
        }
        true
    }
}

// ---------------------------------------------------------------- Effekte

struct Sweep { n: usize, len: usize, up: bool, pitched: bool, vel: f32, rng: Rng, fl: Svf, fr: Svf, t: &'static [f32], ph: f32 }
impl Sweep {
    fn new(w: &'static Waves, dur: f32, vel: f32, up: bool, pitched: bool, rng: Rng) -> Sweep {
        Sweep { n: 0, len: (dur * SRF) as usize, up, pitched, vel, rng, fl: Svf::new(300.0, 2.0), fr: Svf::new(300.0, 2.0), t: w.saw.band(600.0), ph: 0.0 }
    }
}
impl Voice for Sweep {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        for i in 0..l.len() {
            if self.n >= self.len { return false; }
            let x = self.n as f32 / self.len as f32;
            let p = if self.up { x } else { 1.0 - x };
            if self.n % 32 == 0 {
                let c = 250.0 * 36f32.powf(p);
                self.fl.set(c, 2.2); self.fr.set(c * 1.08, 2.2);
            }
            let amp = if self.up { x.powf(2.2) } else { (1.0 - x).powf(1.6) * (x / 0.02).min(1.0) };
            let mut sl = self.rng.next();
            let mut sr = self.rng.next();
            if self.pitched {
                self.ph += hz(50.0 + 24.0 * x * x) / SRF;
                if self.ph > 1.0 { self.ph -= 1.0; }
                let s = look01(self.t, self.ph) * 0.5;
                sl += s; sr += s;
            }
            let pan = (TAU * x * 3.0).sin() * 0.5;
            let (gl, gr) = pan_gains(pan);
            l[i] += self.fl.bp(sl) * amp * self.vel * 0.7 * gl;
            r[i] += self.fr.bp(sr) * amp * self.vel * 0.7 * gr;
            self.n += 1;
        }
        true
    }
}

struct Air { n: usize, len: usize, vel: f32, cut: f32, q: f32, drops: f32, rng: Rng, fl: Svf, fr: Svf, dl: Svf, dz: f32 }
impl Air {
    fn new(dur: f32, vel: f32, cut: f32, q: f32, drops: f32, rng: Rng) -> Air {
        Air { n: 0, len: (dur * SRF) as usize, vel, cut, q, drops, rng, fl: Svf::new(cut, q), fr: Svf::new(cut, q), dl: Svf::new(3200.0, 3.0), dz: 0.0 }
    }
}
impl Voice for Air {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        for i in 0..l.len() {
            if self.n >= self.len { return false; }
            let t = self.n as f32 / SRF;
            if self.n % 64 == 0 {
                let c = self.cut * (1.0 + 0.35 * (TAU * 0.11 * t).sin() + 0.15 * (TAU * 0.37 * t).sin());
                self.fl.set(c, self.q); self.fr.set(c * 1.1, self.q);
            }
            let x = self.n as f32 / self.len as f32;
            let env = (x / 0.08).min(1.0) * ((1.0 - x) / 0.12).min(1.0);
            let mut dl = 0.0;
            if self.drops > 0.0 {
                // vereinzelte Tropfen
                if self.rng.unit() < self.drops / SRF { self.dz = 1.0; }
                self.dz *= 0.9965;
                dl = self.dl.bp(self.rng.next() * self.dz) * 0.8;
            }
            let ol = self.fl.bp(self.rng.next()) + dl;
            let or = self.fr.bp(self.rng.next()) - dl * 0.6;
            l[i] += ol * env * self.vel * 0.5;
            r[i] += or * env * self.vel * 0.5;
            self.n += 1;
        }
        true
    }
}

struct Sampler { buf: Arc<Vec<f32>>, pos: f64, rate: f64, n: usize, len: usize, vel: f32, rev: bool, gl: f32, gr: f32 }
impl Sampler {
    fn new(buf: Arc<Vec<f32>>, vel: f32, rate: f32, start: f32, len: f32, reverse: bool, pan: f32) -> Sampler {
        let frames = buf.len() / 2;
        let st = (start * SRF) as f64;
        let len_out = (len * SRF) as usize;
        let pos = if reverse { (st + len_out as f64 * rate as f64).min(frames as f64 - 1.0) } else { st };
        let (gl, gr) = pan_gains(pan);
        Sampler { buf, pos, rate: rate as f64, n: 0, len: len_out, vel, rev: reverse, gl, gr }
    }
}
impl Voice for Sampler {
    fn render(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let frames = self.buf.len() / 2;
        for i in 0..l.len() {
            if self.n >= self.len || self.pos < 0.0 || self.pos as usize + 1 >= frames { return false; }
            let p = self.pos as usize;
            let f = (self.pos - p as f64) as f32;
            let b = &self.buf;
            let sl = b[p * 2] + (b[p * 2 + 2] - b[p * 2]) * f;
            let sr = b[p * 2 + 1] + (b[p * 2 + 3] - b[p * 2 + 1]) * f;
            // kurze Blenden gegen Knackser an den Schnittkanten
            let fi = (self.n as f32 / 44.0).min(1.0);
            let fo = ((self.len - self.n) as f32 / 130.0).min(1.0);
            let g = fi * fo * self.vel;
            l[i] += sl * g * self.gl;
            r[i] += sr * g * self.gr;
            if self.rev { self.pos -= self.rate; } else { self.pos += self.rate; }
            self.n += 1;
        }
        true
    }
}
