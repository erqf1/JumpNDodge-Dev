//! „Falling Pixels“ (5.12) – emotionaler Breakcore, 174 BPM, d-Moll. Dieselbe Komposition wie
//! vorher, jetzt mit den gemeinsamen Instrumenten der Engine. Dazu die ruhige Menü-Fassung.

use crate::game::music::arr::*;
use crate::game::music::engine::*;
use crate::game::music::inst::*;

/// Akkorde (je 2 Takte): Dm – B♭ – F – C (Grundton Oktave 3, Terz klein/groß)
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

const PIANO: Tone = Tone::Piano { pedal: 0.0 };

pub fn build(calm: bool) -> Score {
    let parts: Vec<(Part, usize)> = if calm {
        vec![(Part::Calm, 16)]
    } else {
        vec![(Part::Intro, 8), (Part::Build, 8), (Part::DropA, 16), (Part::Break, 8), (Part::DropB, 16), (Part::Outro, 8)]
    };
    let bars: usize = parts.iter().map(|p| p.1).sum();
    let mut a = Arr::new(174.0, bars);
    let kit = Kit { kick_tone: 120.0, kick_decay: 0.16, ..Kit::default() };
    let mut pad = SynthP::pad();
    pad.uni = 3; pad.det = 9.0; pad.cut = 2400.0; pad.a = 0.45;
    let mut bar0 = 0usize;
    for (part, count) in &parts {
        for k in 0..*count {
            let bar = bar0 + k;
            let tb = bar as f32 * 4.0;
            let (root, third) = CHORDS[(bar / 2) % 4];
            let in_part = k as f32 / *count as f32;
            let full = matches!(part, Part::DropA | Part::DropB);
            if bar % 2 == 0 {
                let pa = match part { Part::Intro | Part::Calm => 0.55, Part::Build => 0.7, Part::Break => 0.75, Part::Outro => 0.6 * (1.0 - in_part * 0.5), _ => 0.75 };
                for (j, m) in [root + 12.0, root + 12.0 + third, root + 19.0, root + 24.0].iter().enumerate() {
                    a.play(Bus::Pad, Tone::Synth(pad), tb, 8.0, *m, pa, j as f32 * 0.3 - 0.45);
                }
            }
            let arp = [0.0, 7.0, 12.0, 12.0 + third, 19.0, 12.0 + third, 12.0, 7.0];
            let av = match part { Part::Intro => 0.5 + 0.2 * in_part, Part::Calm => 0.55, Part::Build => 0.62, Part::Break => 0.55, Part::Outro => 0.58 * (1.0 - in_part * 0.6), _ => 0.4 };
            for (j, off) in arp.iter().enumerate() {
                let acc = if j == 0 { 1.25 } else if j % 2 == 0 { 1.0 } else { 0.82 };
                a.play(Bus::Piano, PIANO, tb + j as f32 * 0.5, 0.5, root + off, av * acc, (j as f32 * 0.9).sin() * 0.35);
            }
            if bar % 2 == 0 && !full { a.play(Bus::Piano, PIANO, tb, 7.2, root - 12.0, av * 1.1, 0.0); }
            let mel: Option<(&[(f32, f32, f32)], f32)> = match part {
                Part::Build => Some((MEL_A, 0.85)),
                Part::DropA => Some((MEL_A, 1.0)),
                Part::Break => Some((MEL_B, 0.85)),
                Part::DropB => Some((MEL_B, 1.0)),
                Part::Outro if k < 4 => Some((MEL_A, 0.8)),
                Part::Calm if k >= 8 => Some((MEL_A, 0.75)),
                _ => None,
            };
            if let Some((m, vel)) = mel {
                let cyc = (bar % 8) as f32 * 4.0;
                for &(st, du, note) in m.iter().filter(|(st, _, _)| *st >= cyc && *st < cyc + 4.0) {
                    let t = tb + (st - cyc);
                    a.play(Bus::Piano, PIANO, t, du, note, vel, 0.12);
                    if full { a.play(Bus::Piano, PIANO, t, du, note - 12.0, vel * 0.5, -0.12); }
                }
            }
            match part {
                Part::DropA | Part::DropB => {
                    let rb = root - 24.0;
                    let r = (0.35, 1.0, 160.0, 700.0, 1.5);
                    let (d1, d2) = (a.secs(2.4), a.secs(1.4));
                    a.add(tb, Bus::Bass, 0.0, Inst::Sub { midi: rb, dur: d1, vel: 1.0, from: 0.0, glide: 0.0 });
                    a.add(tb, Bus::Bass, 0.0, Inst::Reese { midi: rb + 12.0, dur: d1, vel: r.0, lfo: 174.0 / 60.0 / 2.0, cmin: r.2, cmax: r.3, drive: r.4 });
                    a.add(tb + 2.5, Bus::Bass, 0.0, Inst::Sub { midi: rb, dur: d2, vel: 0.95, from: 0.0, glide: 0.0 });
                    a.add(tb + 2.5, Bus::Bass, 0.0, Inst::Reese { midi: rb + 12.0, dur: d2, vel: r.0, lfo: 174.0 / 60.0 / 2.0, cmin: r.2, cmax: r.3, drive: r.4 });
                }
                Part::Break => { let d = a.secs(3.8); a.add(tb, Bus::Bass, 0.0, Inst::Sub { midi: root - 24.0, dur: d, vel: 0.85, from: 0.0, glide: 0.0 }); }
                Part::Build if k >= 4 => { let d = a.secs(3.6); a.add(tb, Bus::Bass, 0.0, Inst::Sub { midi: root - 24.0, dur: d, vel: 0.6 + 0.3 * in_part, from: 0.0, glide: 0.0 }); }
                _ => {}
            }
            let step = 0.25;
            let hat = |a: &mut Arr, t: f32, v: f32, open: bool, pan: f32| a.add(t, Bus::Perc, pan, Inst::Hat { vel: v, open });
            let snr = |a: &mut Arr, t: f32, v: f32, pan: f32| a.add(t, Bus::Perc, pan, Inst::Snare { vel: v, tone: 190.0, decay: 0.09 });
            match part {
                Part::Build => {
                    if k >= 2 { for b4 in [0.0, 2.0] { a.kick(tb + b4, 0.55 + 0.3 * in_part, &kit); } }
                    if k >= 4 { for s in (2..16).step_by(4) { hat(&mut a, tb + s as f32 * step, 0.5, false, 0.3); } }
                    if k + 1 == *count {
                        for s in 0..8 { snr(&mut a, tb + s as f32 * step, 0.25 + s as f32 * 0.04, 0.0); }
                        for s in 0..16 { snr(&mut a, tb + 2.0 + s as f32 * step * 0.5, 0.45 + s as f32 * 0.035, 0.0); }
                    }
                }
                Part::DropA | Part::DropB => {
                    let dense = *part == Part::DropB;
                    let fill = k % 4 == 3;
                    let r1 = a.rnd();
                    let pat = if fill { 2 + (r1 * 2.0) as usize % 2 } else { (r1 * 2.0) as usize % 2 + if dense && k % 2 == 1 { 2 } else { 0 } };
                    let pat = BREAKS[pat.min(3)];
                    for (s, v) in pat.iter().enumerate() {
                        let t = tb + s as f32 * step;
                        let stutter = dense && a.rnd() < 0.12;
                        match v {
                            2 => a.kick(t, 1.0, &kit),
                            1 => { snr(&mut a, t, 0.9, 0.05); if stutter { snr(&mut a, t + step * 0.5, 0.6, 0.05); } }
                            3 => snr(&mut a, t, 0.32, -0.15),
                            _ => {}
                        }
                    }
                    let hats = if dense { 16 } else { 8 };
                    for h in 0..hats {
                        let t = tb + h as f32 * 4.0 / hats as f32;
                        hat(&mut a, t, if h % 2 == 0 { 0.55 } else { 0.35 }, h == hats - 2 && !fill, if h % 2 == 0 { 0.35 } else { -0.35 });
                    }
                    if fill { for s in 0..8 { snr(&mut a, tb + 3.0 + s as f32 * step * 0.5, 0.4 + s as f32 * 0.06, ((s as f32) * 0.8).sin() * 0.4); } }
                }
                Part::Break => {
                    a.kick(tb, 0.7, &kit);
                    snr(&mut a, tb + 2.0, 0.55, 0.0);
                    for h in 0..8 { hat(&mut a, tb + h as f32 * 0.5, 0.28, false, 0.3); }
                    if k + 1 == *count { a.riser(tb + 4.0, 4.0, 0.9, false); }
                }
                Part::Outro => { if k < 4 { a.kick(tb, 0.6 * (1.0 - in_part), &kit); } }
                Part::Intro => { if k >= 6 { a.riser(tb + 4.0, 4.0, 0.35 * (k as f32 - 5.0), false); } }
                Part::Calm => {}
            }
            if *part == Part::Build && k >= 4 { a.riser(tb + 4.0, 4.0, 0.25 * (k as f32 - 3.0), false); }
        }
        bar0 += count;
    }
    a.sc.mix.bus[Bus::Perc as usize].gain = 0.85;
    a.sc.mix.master = if calm { 1.0 } else { 0.85 };
    a.finish()
}
