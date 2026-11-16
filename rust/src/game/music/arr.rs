//! Baukasten fürs Arrangieren: Noten- und Akkordschrift, Breakbeat zum Zerhacken und Helfer
//! für Schichten (Melodie, Arpeggio, Flächen, Bass, Breaks, Effekte). Ein Song ist eine
//! Funktion, die damit seine Partitur (`Score`) baut.
//!
//! Notenschrift: `"g5:4 eb5:2 f5:2 | r:4 c6+eb6:4!"` – Tonname mit Oktave (c4 = 60), Dauer in
//! Sechzehnteln nach `:` (ohne Angabe wie die vorige Note), `r` = Pause, `+` = Zusammenklang,
//! `!` = betont, `?` = leise. `|` ist nur zur Übersicht.
//! Akkordschrift: `"Cm Ab Eb:2 Bb/D:2"` – Dauer in Schlägen nach `:` (Standard: ein Takt).

use super::dsp::*;
use super::engine::*;
use super::inst::*;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Chord { pub root: i32, pub tones: Vec<i32>, pub bass: i32 }

impl Chord {
    /// `n` Akkordtöne möglichst nah um `center` (MIDI) – ergibt ruhige Stimmführung
    pub fn voicing(&self, center: i32, n: usize) -> Vec<i32> {
        let mut cand: Vec<i32> = vec![];
        for o in -3..=3 { for t in &self.tones { cand.push(self.root + t + 12 * o + 60); } }
        cand.sort();
        cand.dedup_by_key(|m| *m);
        cand.sort_by_key(|m| ((m - center).abs(), *m));
        let mut v: Vec<i32> = cand.into_iter().take(n).collect();
        v.sort();
        v
    }
    /// Basston in der Oktave ab `low`
    pub fn bass_note(&self, low: i32) -> i32 { low + (self.bass - low).rem_euclid(12) }
}

fn pc(c: char) -> Option<i32> {
    Some(match c.to_ascii_lowercase() { 'c' => 0, 'd' => 2, 'e' => 4, 'f' => 5, 'g' => 7, 'a' => 9, 'b' => 11, _ => return None })
}

pub fn parse_chord(s: &str) -> Chord {
    let (main, slash) = match s.split_once('/') { Some((a, b)) => (a, Some(b)), None => (s, None) };
    let mut ch = main.chars().peekable();
    let mut root = pc(ch.next().unwrap_or('C')).unwrap_or(0);
    match ch.peek() { Some('#') => { root += 1; ch.next(); } Some('b') => { root -= 1; ch.next(); } _ => {} }
    let q: String = ch.collect();
    let tones: Vec<i32> = match q.as_str() {
        "" => vec![0, 4, 7], "m" => vec![0, 3, 7], "5" => vec![0, 7], "7" => vec![0, 4, 7, 10], "m7" => vec![0, 3, 7, 10],
        "maj7" => vec![0, 4, 7, 11], "sus2" => vec![0, 2, 7], "sus4" => vec![0, 5, 7], "add9" => vec![0, 4, 7, 14],
        "madd9" => vec![0, 3, 7, 14], "m9" => vec![0, 3, 7, 10, 14], "maj9" => vec![0, 4, 7, 11, 14], "dim" => vec![0, 3, 6],
        "6" => vec![0, 4, 7, 9], "m6" => vec![0, 3, 7, 9], "7sus4" => vec![0, 5, 7, 10], "aug" => vec![0, 4, 8],
        other => panic!("unbekannter Akkord {other}"),
    };
    let root = root.rem_euclid(12);
    let bass = slash.and_then(|b| {
        let mut c = b.chars();
        let mut p = pc(c.next()?)?;
        match c.next() { Some('#') => p += 1, Some('b') => p -= 1, _ => {} }
        Some(p.rem_euclid(12))
    }).unwrap_or(root);
    Chord { root, tones, bass }
}

#[derive(Clone, Copy, Debug)]
pub struct Note { pub t: f32, pub d: f32, pub m: i32, pub v: f32 }

/// Melodie lesen -> Noten (Zeiten in Schlägen) und Gesamtlänge in Schlägen
pub fn parse_mel(s: &str) -> (Vec<Note>, f32) {
    let mut out = vec![];
    let mut t = 0f32;
    let mut d = 4f32;
    for tok in s.split_whitespace() {
        if tok == "|" { continue; }
        let mut tok = tok;
        let mut v = 1.0;
        if let Some(x) = tok.strip_suffix('!') { tok = x; v = 1.18; }
        if let Some(x) = tok.strip_suffix('?') { tok = x; v = 0.68; }
        let (names, dur) = match tok.split_once(':') { Some((a, b)) => (a, Some(b)), None => (tok, None) };
        if let Some(dd) = dur { d = dd.parse::<f32>().expect("Dauer"); }
        if names != "r" {
            for nm in names.split('+') {
                let mut c = nm.chars().peekable();
                let mut p = pc(c.next().expect("Note")).expect("Notenname");
                match c.peek() { Some('#') => { p += 1; c.next(); } Some('b') => { p -= 1; c.next(); } _ => {} }
                let oct: i32 = c.collect::<String>().parse().expect("Oktave");
                out.push(Note { t, d: d / 4.0, m: 12 * (oct + 1) + p, v });
            }
        }
        t += d / 4.0;
    }
    (out, t)
}

/// Klangfarbe für Melodien/Akkorde
#[derive(Clone, Copy, Debug)]
pub enum Tone {
    Piano { pedal: f32 },
    Bell { ratio: f32, index: f32, decay: f32 },
    Pluck { bright: f32, decay: f32 },
    Synth(SynthP),
    Choir { vowel: f32 },
}

/// Synthetischer Breakbeat (2 Takte), der zerhackt wird
pub struct Brk { pub id: usize, pub kind: [u8; 32], pub kit: Kit }

#[derive(Clone, Copy, Debug)]
pub struct Kit {
    pub kick_tone: f32, pub kick_decay: f32, pub snare_tone: f32, pub snare_decay: f32,
    pub ride: f32, pub hat: f32, pub crunch: f32,
    /// Schicht-Kick/Snare über dem Break (Bus Perc)
    pub layer_kick: f32, pub layer_snare: f32, pub clap: f32,
}
impl Default for Kit {
    fn default() -> Kit { Kit { kick_tone: 110.0, kick_decay: 0.12, snare_tone: 190.0, snare_decay: 0.085, ride: 0.5, hat: 0.4, crunch: 1.6, layer_kick: 0.8, layer_snare: 0.45, clap: 0.0 } }
}

const K: u8 = 1;
const S: u8 = 2;
const G: u8 = 3;

pub struct Arr { pub sc: Score, chords: Vec<(f32, f32, Chord)>, seed: u32 }

impl Arr {
    pub fn new(bpm: f32, bars: usize) -> Arr {
        Arr { sc: Score { bpm, len_beats: bars as f32 * 4.0, evs: vec![], kicks: vec![], samples: vec![], lanes: Lanes::default(), mix: Mix::default() }, chords: vec![], seed: 1 }
    }
    pub fn secs(&self, beats: f32) -> f32 { beats * 60.0 / self.sc.bpm }
    pub fn add(&mut self, t: f32, bus: Bus, pan: f32, inst: Inst) {
        self.seed = self.seed.wrapping_add(1);
        if t < 0.0 || t >= self.sc.len_beats { return; }
        self.sc.evs.push(Ev { t, bus, pan, seed: self.seed, inst });
    }
    pub fn rnd(&mut self) -> f32 { self.seed = self.seed.wrapping_add(7); Rng::new(self.seed).unit() }

    // ---------------------------------------------------------------- Harmonie

    /// Akkordfolge ab Takt `bar0` für `bars` Takte (wiederholt sich)
    pub fn prog(&mut self, bar0: usize, bars: usize, prog: &str) {
        let items: Vec<(Chord, f32)> = prog.split_whitespace().map(|t| match t.split_once(':') {
            Some((c, d)) => (parse_chord(c), d.parse().unwrap()), None => (parse_chord(t), 4.0) }).collect();
        let (mut t, end) = (bar0 as f32 * 4.0, (bar0 + bars) as f32 * 4.0);
        self.chords.retain(|c| c.0 < t || c.0 >= end);
        'o: loop {
            for (c, d) in &items {
                if t >= end - 1e-4 { break 'o; }
                let dd = d.min(end - t);
                self.chords.push((t, dd, c.clone()));
                t += d;
            }
        }
        self.chords.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }
    pub fn chord(&self, beat: f32) -> Chord {
        self.chords.iter().rev().find(|c| c.0 <= beat + 1e-4).or(self.chords.first()).map(|c| c.2.clone()).unwrap_or(parse_chord("C"))
    }
    /// Akkorde in einem Bereich: (Start, Länge, Akkord)
    pub fn spans(&self, bar0: usize, bars: usize) -> Vec<(f32, f32, Chord)> {
        let (a, b) = (bar0 as f32 * 4.0, (bar0 + bars) as f32 * 4.0);
        self.chords.iter().filter(|c| c.0 >= a - 1e-4 && c.0 < b).cloned().collect()
    }

    // ---------------------------------------------------------------- Töne

    #[allow(clippy::too_many_arguments)]
    pub fn play(&mut self, bus: Bus, tone: Tone, t: f32, dur: f32, midi: f32, vel: f32, pan: f32) {
        let d = self.secs(dur);
        let inst = match tone {
            Tone::Piano { pedal } => Inst::Piano { midi, dur: d, vel, pedal },
            Tone::Bell { ratio, index, decay } => Inst::Bell { midi, dur: d, vel, ratio, index, decay },
            Tone::Pluck { bright, decay } => Inst::Pluck { midi, vel, bright, decay },
            Tone::Synth(p) => Inst::Synth { midi, dur: d, vel, p },
            Tone::Choir { vowel } => Inst::Choir { midi, dur: d, vel, vowel },
        };
        self.add(t, bus, pan, inst);
    }

    /// Melodie ab Takt `bar0`, `bars` Takte lang (wird wiederholt, falls kürzer).
    /// `f` darf jede Note verändern oder weglassen (None).
    #[allow(clippy::too_many_arguments)]
    pub fn mel(&mut self, bar0: usize, bars: usize, s: &str, bus: Bus, tone: Tone, vel: f32, tr: i32, pan: f32) {
        self.mel_with(bar0, bars, s, 1.0, |_, n| Some(n), |a, n| a.play(bus, tone, n.t, n.d, n.m as f32 + tr as f32, vel * n.v, pan));
    }
    /// wie `mel`, aber zeitlich gestreckt (2.0 = halbes Tempo, für Halftime-Teile)
    #[allow(clippy::too_many_arguments)]
    pub fn mel_x(&mut self, bar0: usize, bars: usize, s: &str, scale: f32, bus: Bus, tone: Tone, vel: f32, tr: i32, pan: f32) {
        self.mel_with(bar0, bars, s, scale, |_, n| Some(n), |a, n| a.play(bus, tone, n.t, n.d, n.m as f32 + tr as f32, vel * n.v, pan));
    }
    pub fn mel_with(&mut self, bar0: usize, bars: usize, s: &str, scale: f32, mut filt: impl FnMut(&mut Arr, Note) -> Option<Note>, mut play: impl FnMut(&mut Arr, Note)) {
        let (mut notes, len) = parse_mel(s);
        for n in notes.iter_mut() { n.t *= scale; n.d *= scale; }
        let len = len * scale;
        if len <= 0.0 { return; }
        let (a, b) = (bar0 as f32 * 4.0, (bar0 + bars) as f32 * 4.0);
        let mut off = a;
        while off < b - 1e-4 {
            for n in &notes {
                let t = off + n.t;
                if t >= b - 1e-4 { break; }
                let mut nn = *n;
                nn.t = t;
                nn.d = nn.d.min(b - t);
                if let Some(x) = filt(self, nn) { play(self, x); }
            }
            off += len;
        }
    }

    /// Akkord-Arpeggio: `pat` = Indizes in die Akkordtöne um `center` (z.B. "0 1 2 3 2 1"),
    /// `step` in Sechzehnteln, `gate` = Notenlänge relativ zum Schritt
    #[allow(clippy::too_many_arguments)]
    pub fn arp(&mut self, bar0: usize, bars: usize, pat: &str, step: f32, center: i32, bus: Bus, tone: Tone, vel: f32, gate: f32) {
        let idx: Vec<Option<i32>> = pat.split_whitespace().map(|x| x.parse().ok()).collect();
        let st = step / 4.0;
        let (a, b) = (bar0 as f32 * 4.0, (bar0 + bars) as f32 * 4.0);
        let mut t = a;
        let mut k = 0usize;
        while t < b - 1e-4 {
            if let Some(Some(i)) = idx.get(k % idx.len()) {
                let ch = self.chord(t);
                let v = ch.voicing(center, ch.tones.len().max(3));
                let n = v.len() as i32;
                let m = v[i.rem_euclid(n) as usize] + 12 * i.div_euclid(n);
                let acc = if k % 4 == 0 { 1.15 } else if k % 2 == 0 { 0.95 } else { 0.8 };
                let pan = ((k as f32) * 1.7).sin() * 0.45;
                self.play(bus, tone, t, st * gate, m as f32, vel * acc, pan);
            }
            t += st;
            k += 1;
        }
    }

    /// Akkorde als Fläche (je Akkordwechsel neu), `n` Töne um `center`
    #[allow(clippy::too_many_arguments)]
    pub fn pad(&mut self, bar0: usize, bars: usize, center: i32, n: usize, bus: Bus, tone: Tone, vel: f32) {
        for (t, d, ch) in self.spans(bar0, bars) {
            for (j, m) in ch.voicing(center, n).iter().enumerate() {
                let pan = if n > 1 { (j as f32 / (n - 1) as f32 * 2.0 - 1.0) * 0.5 } else { 0.0 };
                self.play(bus, tone, t, d + 0.05, *m as f32, vel, pan);
            }
        }
    }

    /// Akkord-Anschläge nach Rhythmus ("x..x..x." je Sechzehntel, wiederholt)
    #[allow(clippy::too_many_arguments)]
    pub fn comp(&mut self, bar0: usize, bars: usize, rhythm: &str, center: i32, n: usize, bus: Bus, tone: Tone, vel: f32) {
        let r: Vec<char> = rhythm.chars().filter(|c| !c.is_whitespace() && *c != '|').collect();
        let (a, b) = (bar0 as f32 * 4.0, (bar0 + bars) as f32 * 4.0);
        let mut k = 0usize;
        let mut t = a;
        while t < b - 1e-4 {
            let c = r[k % r.len()];
            if c == 'x' || c == 'X' {
                let mut len = 1;
                while len < r.len() && r[(k + len) % r.len()] == '-' { len += 1; }
                let ch = self.chord(t);
                let v = if c == 'X' { vel * 1.2 } else { vel };
                for (j, m) in ch.voicing(center, n).iter().enumerate() {
                    self.play(bus, tone, t + j as f32 * 0.006, len as f32 * 0.25 * 0.9, *m as f32, v, (j as f32 - n as f32 / 2.0) * 0.12);
                }
            }
            t += 0.25;
            k += 1;
        }
    }

    /// Bass nach Rhythmus je Sechzehntel: x = Grundton, o = Oktave, 5 = Quinte, 3 = Terz,
    /// - = halten, . = Pause; Großbuchstaben (X/O) gleiten vom vorigen Ton.
    /// sub = Pegel Sinus-Sub, reese = (Pegel, LFO in Schlägen, Filter min/max, Verzerrung)
    pub fn bass(&mut self, bar0: usize, bars: usize, rhythm: &str, sub: f32, reese: Option<(f32, f32, f32, f32, f32)>) {
        let r: Vec<char> = rhythm.chars().filter(|c| !c.is_whitespace() && *c != '|').collect();
        let (a, b) = (bar0 as f32 * 4.0, (bar0 + bars) as f32 * 4.0);
        let mut k = 0usize;
        let mut t = a;
        let mut prev = 0f32;
        while t < b - 1e-4 {
            let c = r[k % r.len()];
            if c != '-' && c != '.' {
                let mut len = 1;
                while len < r.len() && r[(k + len) % r.len()] == '-' { len += 1; }
                let dur = (len as f32 * 0.25).min(b - t);
                let ch = self.chord(t);
                let root = ch.bass_note(26) as f32;
                let third = if ch.tones.contains(&3) { 3.0 } else { 4.0 };
                let m = match c.to_ascii_lowercase() { 'o' => root + 12.0, '5' => root + 7.0, '3' => root + third, _ => root };
                let glide = c.is_ascii_uppercase() && prev > 0.0;
                let d = self.secs(dur) * 0.97;
                if sub > 0.0 { self.add(t, Bus::Bass, 0.0, Inst::Sub { midi: m, dur: d, vel: sub, from: if glide { prev } else { 0.0 }, glide: 0.05 }); }
                if let Some((rv, lfo, cmin, cmax, drive)) = reese {
                    let hzl = self.sc.bpm / 60.0 / lfo.max(0.05);
                    self.add(t, Bus::Bass, 0.0, Inst::Reese { midi: m + 12.0, dur: d, vel: rv, lfo: hzl, cmin, cmax, drive });
                }
                prev = m;
            }
            t += 0.25;
            k += 1;
        }
    }

    // ---------------------------------------------------------------- Schlagzeug

    /// Schicht-Kick (zählt auch für den Sidechain)
    pub fn kick(&mut self, t: f32, vel: f32, kit: &Kit) {
        self.add(t, Bus::Perc, 0.0, Inst::Kick { vel, tone: kit.kick_tone + 40.0, decay: kit.kick_decay * 1.6, click: 0.5 });
        self.sc.kicks.push(t);
    }
    pub fn snare(&mut self, t: f32, vel: f32, kit: &Kit) {
        self.add(t, Bus::Perc, 0.04, Inst::Snare { vel, tone: kit.snare_tone, decay: kit.snare_decay * 1.3 });
        if kit.clap > 0.0 { self.add(t, Bus::Perc, -0.05, Inst::Clap { vel: vel * kit.clap }); }
    }

    /// Breakbeat (2 Takte) im Songtempo erzeugen. Eigenes Muster, nur das Prinzip ist das klassische:
    /// Kick-Kick-Snare mit Ghost-Notes, Ride in Achteln.
    pub fn make_break(&mut self, kit: Kit) -> Brk {
        let step = self.secs(0.25);
        let n = (step * 32.0 * SRF) as usize;
        let mut buf = vec![0f32; n * 2];
        let mut kind = [0u8; 32];
        let kicks = [0usize, 2, 10, 16, 18, 26];
        let snares = [4usize, 12, 20, 30];
        let ghosts = [7usize, 9, 15, 23, 25, 29, 31];
        let mut put = |at: f32, inst: Inst, pan: f32, seed: u32, buf: &mut Vec<f32>| {
            let s = render_offline(&inst, pan, seed, 1.2);
            let o = (at * SRF) as usize * 2;
            for (i, v) in s.iter().enumerate() {
                // läuft am Ende in den Anfang über (Schleife)
                let j = (o + i) % (n * 2);
                buf[j] += *v;
            }
        };
        let mut seed = 77u32;
        for &k in &kicks { kind[k] = K; put(k as f32 * step, Inst::Kick { vel: if k % 16 == 0 { 1.0 } else { 0.85 }, tone: kit.kick_tone, decay: kit.kick_decay, click: 0.7 }, 0.0, seed, &mut buf); seed += 1; }
        for &s in &snares { kind[s] = S; put(s as f32 * step, Inst::Snare { vel: 0.95, tone: kit.snare_tone, decay: kit.snare_decay }, 0.0, seed, &mut buf); seed += 1; }
        for &g in &ghosts { kind[g] = G; put(g as f32 * step + 0.004, Inst::Snare { vel: 0.3, tone: kit.snare_tone + 15.0, decay: kit.snare_decay * 0.6 }, 0.1, seed, &mut buf); seed += 1; }
        for i in (0..32).step_by(2) {
            let acc = if i % 4 == 0 { 1.0 } else { 0.7 };
            if kit.ride > 0.0 { put(i as f32 * step, Inst::Cymbal { vel: kit.ride * acc * 0.6, decay: 0.32, ride: true }, 0.25, seed, &mut buf); seed += 1; }
        }
        for i in (1..32).step_by(2) {
            if kit.hat > 0.0 && i % 4 != 3 { put(i as f32 * step, Inst::Hat { vel: kit.hat * 0.55, open: false }, -0.25, seed, &mut buf); seed += 1; }
        }
        // „gesampelter“ Charakter: Sättigung, oben etwas dunkler, kleiner Raum
        let mut lp = [OnePole::new(9500.0), OnePole::new(9500.0)];
        let mut rv = Reverb::new(0.45, 0.5, 0.35, 0.002);
        let peak = buf.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-6);
        for i in 0..n {
            let (l, r) = (buf[i * 2] / peak, buf[i * 2 + 1] / peak);
            let (wl, wr) = rv.run(l, r);
            buf[i * 2] = sat(lp[0].lp(l + wl * 3.0), kit.crunch) * 0.9;
            buf[i * 2 + 1] = sat(lp[1].lp(r + wr * 3.0), kit.crunch) * 0.9;
        }
        let peak = buf.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-6);
        for v in buf.iter_mut() { *v /= peak; }
        // als Schleife zweimal hintereinander, damit Scheiben über das Ende hinaus lesen können
        let mut two = buf.clone();
        two.extend_from_slice(&buf);
        self.sc.samples.push(Arc::new(two));
        Brk { id: self.sc.samples.len() - 1, kind, kit }
    }

    /// Zerhackte Breaks: je Takt ein Muster aus 16 Zeichen (Sechzehntel):
    /// Zahl = Scheibe 0–31 des Breaks, `_` = weiterlaufen, `.` = Stille; Endungen:
    /// `r` rückwärts, `s` Stottern (2×32tel), `t` Wirbel (3 pro Schritt), `^`/`v` höher/tiefer.
    /// Alle `fill_every` Takte kommt ein Fill aus `fills`.
    #[allow(clippy::too_many_arguments)]
    pub fn breaks(&mut self, bar0: usize, bars: usize, brk: &Brk, chops: &[&str], fills: &[&str], fill_every: usize, vel: f32, layer: f32) {
        for k in 0..bars {
            let is_fill = fill_every > 0 && !fills.is_empty() && (k + 1) % fill_every == 0;
            let pat = if is_fill { fills[(k / fill_every) % fills.len()] } else { chops[k % chops.len()] };
            self.chop_bar((bar0 + k) as f32 * 4.0, brk, pat, vel, layer);
        }
    }

    pub fn chop_bar(&mut self, t0: f32, brk: &Brk, pat: &str, vel: f32, layer: f32) {
        let toks: Vec<&str> = pat.split_whitespace().collect();
        let step = 0.25f32;
        let step_s = self.secs(step);
        let kit = brk.kit;
        let mut k = 0;
        while k < toks.len() {
            let tk = toks[k];
            let mut len = 1;
            while k + len < toks.len() && toks[k + len] == "_" { len += 1; }
            let t = t0 + k as f32 * step;
            if tk != "." && tk != "_" {
                let digits: String = tk.chars().take_while(|c| c.is_ascii_digit()).collect();
                let mods: String = tk.chars().skip(digits.len()).collect();
                let sl: usize = digits.parse::<usize>().unwrap_or(0) % 32;
                let rev = mods.contains('r');
                let rate = if mods.contains('^') { 1.19 } else if mods.contains('v') { 0.84 } else { 1.0 };
                let reps = if mods.contains('s') { 2 } else if mods.contains('t') { 3 } else { 1 };
                let start = sl as f32 * step_s;
                if reps > 1 {
                    for j in 0..reps {
                        let sub = step / reps as f32;
                        let v = vel * (0.75 + 0.25 * j as f32 / reps as f32);
                        self.add(t + j as f32 * sub, Bus::Drums, 0.0, Inst::Sample { id: brk.id, vel: v, rate, start, len: self.secs(sub) * 0.98, reverse: rev });
                        if brk.kind[sl] == S && layer > 0.0 { self.snare(t + j as f32 * sub, kit.layer_snare * layer * 0.5, &kit); }
                    }
                } else {
                    let dur = len as f32 * step;
                    self.add(t, Bus::Drums, 0.0, Inst::Sample { id: brk.id, vel, rate, start, len: self.secs(dur), reverse: rev });
                    if !rev && layer > 0.0 {
                        for j in 0..len {
                            let kd = brk.kind[(sl + j) % 32];
                            let tt = t + j as f32 * step / rate;
                            if kd == K { self.kick(tt, kit.layer_kick * layer, &kit); }
                            if kd == S { self.snare(tt, kit.layer_snare * layer, &kit); }
                        }
                    }
                    if rev && brk.kind[sl] == K { self.sc.kicks.push(t + dur * 0.7); }
                }
            }
            k += len;
        }
    }

    // ---------------------------------------------------------------- Effekte

    pub fn riser(&mut self, end: f32, beats: f32, vel: f32, pitched: bool) {
        let d = self.secs(beats);
        self.add(end - beats, Bus::Fx, 0.0, Inst::Riser { dur: d, vel, pitched });
    }
    pub fn down(&mut self, t: f32, beats: f32, vel: f32) { let d = self.secs(beats); self.add(t, Bus::Fx, 0.0, Inst::Down { dur: d, vel }); }
    pub fn impact(&mut self, t: f32, vel: f32) { self.add(t, Bus::Fx, 0.0, Inst::Impact { vel }); self.sc.kicks.push(t); }
    pub fn crash(&mut self, t: f32, vel: f32) { self.add(t, Bus::Drums, 0.15, Inst::Cymbal { vel, decay: 0.9, ride: false }); }
    pub fn air(&mut self, t: f32, beats: f32, vel: f32, cut: f32, q: f32, drops: f32) {
        let d = self.secs(beats);
        self.add(t, Bus::Fx, 0.0, Inst::Air { dur: d, vel, cut, q, drops });
    }
    /// Rückwärts-Becken, das genau auf `target` endet
    pub fn rev_crash(&mut self, target: f32, beats: f32, vel: f32) {
        let secs = self.secs(beats);
        let s = render_offline(&Inst::Cymbal { vel: 1.0, decay: secs * 0.45, ride: false }, 0.0, self.seed, secs + 0.05);
        let mut rev: Vec<f32> = Vec::with_capacity(s.len());
        for f in (0..s.len() / 2).rev() { rev.push(s[f * 2]); rev.push(s[f * 2 + 1]); }
        self.sc.samples.push(Arc::new(rev));
        let id = self.sc.samples.len() - 1;
        self.add(target - beats, Bus::Fx, 0.0, Inst::Sample { id, vel, rate: 1.0, start: 0.04, len: secs, reverse: false });
    }
    /// Rückwärts-Klavierakkord als Ansauger in `target`
    pub fn rev_piano(&mut self, target: f32, beats: f32, notes: &[i32], vel: f32) {
        let secs = self.secs(beats);
        let n = ((secs + 0.1) * SRF) as usize;
        let mut mix = vec![0f32; n * 2];
        for (j, m) in notes.iter().enumerate() {
            let s = render_offline(&Inst::Piano { midi: *m as f32, dur: secs, vel: 0.8, pedal: 0.6 }, (j as f32 - 1.0) * 0.3, 900 + j as u32, secs + 0.1);
            for (i, v) in s.iter().enumerate().take(n * 2) { mix[i] += v; }
        }
        let mut rev: Vec<f32> = Vec::with_capacity(mix.len());
        for f in (0..n).rev() { rev.push(mix[f * 2]); rev.push(mix[f * 2 + 1]); }
        self.sc.samples.push(Arc::new(rev));
        let id = self.sc.samples.len() - 1;
        self.add(target - beats, Bus::Fx, 0.0, Inst::Sample { id, vel, rate: 1.0, start: 0.1, len: secs - 0.005, reverse: false });
    }
    /// Kurze Stille (z.B. vor dem Drop)
    pub fn gap(&mut self, t: f32, beats: f32) {
        let g = &mut self.sc.lanes.gain;
        g.push((t - 0.02, 1.0)); g.push((t, 0.0)); g.push((t + beats - 0.01, 0.0)); g.push((t + beats, 1.0));
    }
    pub fn stutter(&mut self, t: f32, beats: f32, slice: f32, shrink: bool) { self.sc.lanes.fx.push(MasterFx::Repeat { t, len: beats, slice, shrink }); }
    pub fn tape_stop(&mut self, t: f32, beats: f32) { self.sc.lanes.fx.push(MasterFx::TapeStop { t, len: beats }); }
    /// Filterfahrt auf einem Bus (Tiefpass), Werte in Hz; danach wieder offen, wenn `reopen`
    pub fn lp(&mut self, bus: Bus, a: f32, b: f32, fa: f32, fb: f32, reopen: bool) {
        let l = &mut self.sc.lanes.lp[bus as usize];
        if a > 0.0 && l.last().map(|p| p.1 >= 19999.0).unwrap_or(true) { l.push((a - 0.01, 20000.0)); }
        l.push((a, fa)); l.push((b, fb));
        if reopen { l.push((b + 0.01, 20000.0)); }
    }
    pub fn hp(&mut self, bus: Bus, a: f32, b: f32, fa: f32, fb: f32, reopen: bool) {
        let l = &mut self.sc.lanes.hp[bus as usize];
        if a > 0.0 && l.last().map(|p| p.1 <= 20.0).unwrap_or(true) { l.push((a - 0.01, 20.0)); }
        l.push((a, fa)); l.push((b, fb));
        if reopen { l.push((b + 0.01, 20.0)); }
    }
    pub fn master_lp(&mut self, a: f32, b: f32, fa: f32, fb: f32, reopen: bool) {
        let l = &mut self.sc.lanes.master_lp;
        if a > 0.0 && l.last().map(|p| p.1 >= 19999.0).unwrap_or(true) { l.push((a - 0.01, 20000.0)); }
        l.push((a, fa)); l.push((b, fb));
        if reopen { l.push((b + 0.01, 20000.0)); }
    }

    pub fn finish(mut self) -> Score {
        let srt = |v: &mut Vec<(f32, f32)>| v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for l in self.sc.lanes.lp.iter_mut() { srt(l); }
        for l in self.sc.lanes.hp.iter_mut() { srt(l); }
        srt(&mut self.sc.lanes.master_lp);
        srt(&mut self.sc.lanes.master_hp);
        srt(&mut self.sc.lanes.gain);
        self.sc.kicks.sort_by(|a, b| a.partial_cmp(b).unwrap());
        self.sc
    }
}
