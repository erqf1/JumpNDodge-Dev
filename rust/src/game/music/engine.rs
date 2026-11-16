//! Misch-Engine: Ereignisse → Stimmen → Busse (Filter, Sidechain, Sends) → Hall/Raum/Echo →
//! Master (Stotter-/Bandstopp-Effekte, Filterfahrten, Limiter) → i16 Stereo.
//!
//! Gerendert wird in Blöcken und in mehreren Abschnitten parallel. Jeder Abschnitt startet
//! einige Sekunden früher (Vorlauf), damit Hall, Echo und ausklingende Töne schon „laufen“ –
//! dadurch sind die Nahtstellen unhörbar. Der erste Abschnitt holt sich seinen Vorlauf vom
//! Song-Ende: so ist die Schleife nahtlos.

use super::dsp::*;
use super::inst::*;
use std::sync::Arc;

pub const NBUS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bus { Piano = 0, Keys = 1, Pad = 2, Lead = 3, Bass = 4, Drums = 5, Perc = 6, Fx = 7 }

#[derive(Clone, Copy, Debug)]
pub struct BusCfg { pub gain: f32, pub hall: f32, pub room: f32, pub delay: f32, pub duck: f32, pub drive: f32, pub width: f32 }
impl BusCfg {
    pub const fn new(gain: f32, hall: f32, room: f32, delay: f32, duck: f32) -> BusCfg { BusCfg { gain, hall, room, delay, duck, drive: 0.0, width: 1.0 } }
}

#[derive(Clone, Debug)]
pub struct Mix {
    pub bus: [BusCfg; NBUS],
    /// Hall: Raum, Dämpfung, Größe, Vorverzögerung (s), Pegel
    pub hall: (f32, f32, f32, f32, f32),
    pub room: (f32, f32, f32, f32, f32),
    /// Echo: Zeit in Schlägen, Rückkopplung, Pegel
    pub delay: (f32, f32, f32),
    pub master: f32,
    pub drive: f32,
    /// Sidechain-Abklingzeit (s)
    pub duck_tau: f32,
}

impl Default for Mix {
    fn default() -> Mix {
        Mix {
            bus: [
                BusCfg::new(1.8, 0.42, 0.0, 0.08, 0.22),  // Klavier
                BusCfg::new(1.1, 0.5, 0.0, 0.3, 0.25),    // Glocken/Pluck/Arp
                BusCfg::new(0.22, 0.55, 0.0, 0.0, 0.5),   // Flächen/Chor
                BusCfg::new(1.4, 0.35, 0.0, 0.12, 0.4),   // Lead-Synth
                BusCfg { gain: 1.0, hall: 0.0, room: 0.0, delay: 0.0, duck: 0.6, drive: 1.2, width: 0.8 }, // Bass
                BusCfg { gain: 0.9, hall: 0.0, room: 0.22, delay: 0.0, duck: 0.0, drive: 1.3, width: 1.0 }, // Break
                BusCfg { gain: 0.8, hall: 0.0, room: 0.12, delay: 0.0, duck: 0.0, drive: 1.1, width: 1.0 }, // Kick/Snare-Schicht
                BusCfg::new(0.7, 0.5, 0.0, 0.0, 0.0),     // Effekte
            ],
            hall: (0.86, 0.32, 1.25, 0.022, 1.0),
            room: (0.55, 0.45, 0.6, 0.004, 0.8),
            delay: (0.75, 0.38, 0.5),
            master: 0.8,
            drive: 1.0,
            duck_tau: 0.1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Ev { pub t: f32, pub bus: Bus, pub pan: f32, pub seed: u32, pub inst: Inst }

#[derive(Clone, Copy, Debug)]
pub enum MasterFx {
    /// Stottern: die erste `slice` (Schläge) wird wiederholt; shrink = immer kürzer (Beschleunigung)
    Repeat { t: f32, len: f32, slice: f32, shrink: bool },
    /// Bandstopp: alles wird langsamer bis zum Stillstand
    TapeStop { t: f32, len: f32 },
}

/// Automationsspuren: Punkte (Schlag, Wert), dazwischen linear
#[derive(Clone, Default, Debug)]
pub struct Lanes {
    pub lp: [Vec<(f32, f32)>; NBUS],
    pub hp: [Vec<(f32, f32)>; NBUS],
    pub master_lp: Vec<(f32, f32)>,
    pub master_hp: Vec<(f32, f32)>,
    pub gain: Vec<(f32, f32)>,
    pub fx: Vec<MasterFx>,
}

pub struct Score {
    pub bpm: f32,
    pub len_beats: f32,
    pub evs: Vec<Ev>,
    /// Sidechain-Auslöser (Schläge)
    pub kicks: Vec<f32>,
    pub samples: Vec<Arc<Vec<f32>>>,
    pub lanes: Lanes,
    pub mix: Mix,
}

impl Score {
    pub fn spb(&self) -> f64 { SR as f64 * 60.0 / self.bpm as f64 }
    pub fn len_samples(&self) -> usize { (self.len_beats as f64 * self.spb()).round() as usize }
}

fn lane_at(l: &[(f32, f32)], b: f32) -> f32 {
    if l.is_empty() { return 0.0; }
    if b <= l[0].0 { return l[0].1; }
    for w in l.windows(2) {
        if b < w[1].0 {
            let (a, c) = (w[0], w[1]);
            if c.0 - a.0 <= 1e-6 { return c.1; }
            return a.1 + (c.1 - a.1) * (b - a.0) / (c.0 - a.0);
        }
    }
    l[l.len() - 1].1
}
/// Frequenz-Spuren logarithmisch interpolieren (klingt gleichmäßiger)
fn lane_hz(l: &[(f32, f32)], b: f32) -> f32 {
    if l.is_empty() { return 0.0; }
    if b <= l[0].0 { return l[0].1; }
    for w in l.windows(2) {
        if b < w[1].0 {
            let (a, c) = (w[0], w[1]);
            if c.0 - a.0 <= 1e-6 { return c.1; }
            let x = (b - a.0) / (c.0 - a.0);
            return a.1.max(1.0) * (c.1.max(1.0) / a.1.max(1.0)).powf(x);
        }
    }
    l[l.len() - 1].1
}

const BLOCK: usize = 256;
const PREROLL_S: f32 = 7.0;

/// Song rendern (Stereo i16, verschachtelt). `threads` Abschnitte parallel.
pub fn render(sc: &Score, threads: usize) -> Vec<i16> { render_stats(sc, threads).0 }

/// Wie `render`, zusätzlich RMS je Bus (nach Pegel/Sidechain, vor Hall) – für die Mix-Analyse
pub fn render_stats(sc: &Score, threads: usize) -> (Vec<i16>, [f32; NBUS]) {
    let len = sc.len_samples();
    let stats = std::sync::Mutex::new([0f64; NBUS]);
    let mut out = vec![0i16; len * 2];
    let threads = threads.clamp(1, 8);
    let seg = (len / threads / BLOCK + 1) * BLOCK;
    let mut ev_s: Vec<(i64, usize)> = sc.evs.iter().enumerate().map(|(i, e)| ((e.t as f64 * sc.spb()).round() as i64, i)).collect();
    ev_s.sort_by_key(|e| e.0);
    let mut kicks: Vec<i64> = sc.kicks.iter().map(|k| (*k as f64 * sc.spb()).round() as i64).collect();
    kicks.sort();
    let mut kk: Vec<i64> = kicks.iter().map(|k| k - len as i64).collect();
    kk.extend(kicks.iter());
    std::thread::scope(|s| {
        for (k, chunk) in out.chunks_mut(seg * 2).enumerate() {
            let ev_s = &ev_s;
            let kk = &kk;
            let st = &stats;
            s.spawn(move || {
                let r = render_seg(sc, ev_s, kk, k * seg, chunk);
                let mut g = st.lock().unwrap();
                for i in 0..NBUS { g[i] += r[i]; }
            });
        }
    });
    let g = stats.into_inner().unwrap();
    let mut rms = [0f32; NBUS];
    for i in 0..NBUS { rms[i] = (g[i] / (len.max(1) as f64 * 2.0)).sqrt() as f32; }
    (out, rms)
}

struct MfxState { idx: usize, cap: Vec<f32>, rp: f64 }

fn render_seg(sc: &Score, ev_s: &[(i64, usize)], kicks: &[i64], a: usize, out: &mut [i16]) -> [f64; NBUS] {
    let mut acc = [0f64; NBUS];
    let len = sc.len_samples() as i64;
    let b = a as i64 + (out.len() / 2) as i64;
    let start = a as i64 - (PREROLL_S * SRF) as i64;
    let spb = sc.spb();
    // Ereignisse dieses Abschnitts (vor dem Songanfang: vom Ende her)
    let mut evs: Vec<(i64, usize)> = ev_s.iter().filter(|(s, _)| *s >= start && *s < b).copied().collect();
    if start < 0 {
        let mut wrap: Vec<(i64, usize)> = ev_s.iter().filter(|(s, _)| *s - len >= start).map(|(s, i)| (s - len, *i)).collect();
        wrap.append(&mut evs);
        evs = wrap;
    }
    let ctx = Ctx { samples: &sc.samples };
    let m = &sc.mix;
    let mut voices: Vec<(Box<dyn Voice>, usize)> = Vec::new();
    let mut ei = 0usize;
    let mut bl = vec![[0f32; BLOCK]; NBUS];
    let mut br = vec![[0f32; BLOCK]; NBUS];
    let mut used = [false; NBUS];
    let mut lpf = [[Svf::new(20000.0, 0.7); 2]; NBUS];
    let mut hpf = [[Svf::new(20.0, 0.7); 2]; NBUS];
    let mut mlp = [Svf::new(20000.0, 0.7); 2];
    let mut mhp = [Svf::new(20.0, 0.7); 2];
    let mut hall = Reverb::new(m.hall.0, m.hall.1, m.hall.2, m.hall.3);
    let mut room = Reverb::new(m.room.0, m.room.1, m.room.2, m.room.3);
    let mut delay = Delay::new(m.delay.0 * 60.0 / sc.bpm, m.delay.1);
    let mut lim = Limiter::new(0.89);
    let mut dc = [OnePole::new(18.0), OnePole::new(18.0)];
    let mut ki = kicks.partition_point(|k| *k <= start);
    let mut last_kick = if ki > 0 { kicks[ki - 1] } else { i64::MIN / 2 };
    let mut mfx: Option<MfxState> = None;
    let wrapb = |t: i64| -> f32 { (t.rem_euclid(len) as f64 / spb) as f32 };
    let ducks: Vec<f32> = m.bus.iter().map(|c| c.duck).collect();
    let duck_k = 1.0 / (m.duck_tau * SRF);

    let mut t0 = start;
    while t0 < b {
        let n = ((b - t0) as usize).min(BLOCK);
        for bi in 0..NBUS { bl[bi][..n].fill(0.0); br[bi][..n].fill(0.0); used[bi] = false; }
        // laufende Stimmen
        voices.retain_mut(|(v, bi)| { used[*bi] = true; v.render(&mut bl[*bi][..n], &mut br[*bi][..n]) });
        // neue Stimmen (beginnen mitten im Block)
        while ei < evs.len() && evs[ei].0 < t0 + n as i64 {
            let (s, idx) = evs[ei];
            let e = &sc.evs[idx];
            let mut v = make_voice(&e.inst, e.pan, e.seed, &ctx);
            let off = (s - t0).max(0) as usize;
            let bi = e.bus as usize;
            used[bi] = true;
            if v.render(&mut bl[bi][off..n], &mut br[bi][off..n]) { voices.push((v, bi)); }
            ei += 1;
        }
        // Automation (pro Block)
        let beat = wrapb(t0);
        let mut lp_on = [false; NBUS];
        let mut hp_on = [false; NBUS];
        for bi in 0..NBUS {
            if !sc.lanes.lp[bi].is_empty() { let c = lane_hz(&sc.lanes.lp[bi], beat); lp_on[bi] = c < 19000.0; lpf[bi][0].set(c, 0.8); lpf[bi][1].set(c, 0.8); }
            if !sc.lanes.hp[bi].is_empty() { let c = lane_hz(&sc.lanes.hp[bi], beat); hp_on[bi] = c > 22.0; hpf[bi][0].set(c, 0.75); hpf[bi][1].set(c, 0.75); }
        }
        let mlp_on = !sc.lanes.master_lp.is_empty() && { let c = lane_hz(&sc.lanes.master_lp, beat); mlp[0].set(c, 0.75); mlp[1].set(c, 0.75); c < 19000.0 };
        let mhp_on = !sc.lanes.master_hp.is_empty() && { let c = lane_hz(&sc.lanes.master_hp, beat); mhp[0].set(c, 0.75); mhp[1].set(c, 0.75); c > 22.0 };
        let g0 = if sc.lanes.gain.is_empty() { 1.0 } else { lane_at(&sc.lanes.gain, beat) };
        let g1 = if sc.lanes.gain.is_empty() { 1.0 } else { lane_at(&sc.lanes.gain, wrapb(t0 + n as i64)) };
        // Master-Effekt dieses Blocks
        let fx_now = sc.lanes.fx.iter().enumerate().find(|(_, f)| {
            let (ft, fl) = match f { MasterFx::Repeat { t, len, .. } | MasterFx::TapeStop { t, len } => (*t, *len) };
            beat >= ft && beat < ft + fl
        });
        match (fx_now, &mfx) {
            (Some((i, _)), Some(st)) if st.idx == i => {}
            (Some((i, _)), _) => mfx = Some(MfxState { idx: i, cap: Vec::new(), rp: 0.0 }),
            (None, _) => mfx = None,
        }

        // Sidechain-Form für den Block
        let mut shape = [0f32; BLOCK];
        for (i, sh) in shape.iter_mut().enumerate().take(n) {
            let t = t0 + i as i64;
            while ki < kicks.len() && kicks[ki] <= t { last_kick = kicks[ki]; ki += 1; }
            let since = (t - last_kick) as f32;
            *sh = if since > 2.0 * SRF { 0.0 } else { (since / 130.0).min(1.0) * (-since * duck_k).exp() };
        }
        // Busse blockweise verarbeiten und in Summe + Sends mischen
        let mut sum = [[0f32; BLOCK]; 8];
        let first = (a as i64 - t0).clamp(0, n as i64) as usize;
        for bi in 0..NBUS {
            if !used[bi] && !lp_on[bi] && !hp_on[bi] { continue; }
            let c = m.bus[bi];
            let (xl, xr) = (&mut bl[bi], &mut br[bi]);
            if lp_on[bi] { let f = &mut lpf[bi]; for i in 0..n { xl[i] = f[0].lp(xl[i]); xr[i] = f[1].lp(xr[i]); } }
            if hp_on[bi] { let f = &mut hpf[bi]; for i in 0..n { xl[i] = f[0].hp(xl[i]); xr[i] = f[1].hp(xr[i]); } }
            let mut e = 0f32;
            for i in 0..n {
                let g = c.gain * (1.0 - ducks[bi] * shape[i]);
                let (mut l, mut r) = (xl[i] * g, xr[i] * g);
                if c.width != 1.0 { let mid = (l + r) * 0.5; let side = (l - r) * 0.5 * c.width; l = mid + side; r = mid - side; }
                if c.drive > 0.0 { l = sat(l, c.drive); r = sat(r, c.drive); }
                if i >= first { e += l * l + r * r; }
                sum[0][i] += l; sum[1][i] += r;
                sum[2][i] += l * c.hall; sum[3][i] += r * c.hall;
                sum[4][i] += l * c.room; sum[5][i] += r * c.room;
                sum[6][i] += l * c.delay; sum[7][i] += r * c.delay;
            }
            acc[bi] += e as f64;
        }

        for i in 0..n {
            let t = t0 + i as i64;
            let (dl, dr, hl, hr, rl, rr, el, er) = (sum[0][i], sum[1][i], sum[2][i], sum[3][i], sum[4][i], sum[5][i], sum[6][i], sum[7][i]);
            let (a1, a2) = hall.run(hl, hr);
            let (b1, b2) = room.run(rl, rr);
            let (c1, c2) = delay.run(el, er);
            let mut ol = dl + a1 * m.hall.4 + b1 * m.room.4 + c1 * m.delay.2;
            let mut or = dr + a2 * m.hall.4 + b2 * m.room.4 + c2 * m.delay.2;
            // Stottern / Bandstopp
            if let Some(st) = mfx.as_mut() {
                let f = sc.lanes.fx[st.idx];
                let p = st.cap.len() / 2;
                match f {
                    MasterFx::Repeat { slice, shrink, len: fl, .. } => {
                        let sl = ((slice as f64 * spb) as usize).max(64);
                        if p < sl { st.cap.push(ol); st.cap.push(or); }
                        else {
                            st.cap.push(0.0); st.cap.push(0.0);
                            let mut cur = sl;
                            if shrink { let q = (p as f64 / (fl as f64 * spb) * 4.0) as u32; cur = (sl >> q.min(4)).max(64); }
                            let j = p % cur;
                            let edge = (j as f32 / 48.0).min(1.0).min((cur - j) as f32 / 48.0);
                            ol = st.cap[j * 2] * edge; or = st.cap[j * 2 + 1] * edge;
                        }
                    }
                    MasterFx::TapeStop { len: fl, .. } => {
                        st.cap.push(ol); st.cap.push(or);
                        let q = (p as f64 / (fl as f64 * spb)).min(1.0);
                        st.rp += (1.0 - q).powi(2);
                        let j = (st.rp as usize).min(p);
                        let fade = (1.0 - q as f32).sqrt();
                        ol = st.cap[j * 2] * fade; or = st.cap[j * 2 + 1] * fade;
                    }
                }
            }
            if mlp_on { ol = mlp[0].lp(ol); or = mlp[1].lp(or); }
            if mhp_on { ol = mhp[0].hp(ol); or = mhp[1].hp(or); }
            let g = (g0 + (g1 - g0) * i as f32 / n as f32) * m.master;
            ol = dc[0].hp(ol) * g; or = dc[1].hp(or) * g;
            if m.drive > 0.0 { ol = sat(ol, m.drive); or = sat(or, m.drive); }
            let (fl, fr) = lim.run(ol, or);
            if t >= a as i64 {
                let o = ((t - a as i64) * 2) as usize;
                out[o] = (fl * 32767.0) as i16;
                out[o + 1] = (fr * 32767.0) as i16;
            }
        }
        t0 += n as i64;
    }
    acc
}
