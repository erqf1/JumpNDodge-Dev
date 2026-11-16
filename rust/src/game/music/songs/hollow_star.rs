//! „Hollow Star“ – dunkel, Leere. 164 BPM, d-Moll mit phrygischem Es.
//! Tiefer Chor, dunkle Gongs, Sub-Drops, schwerer Reese, Halftime-Drop der in volle Breaks
//! kippt, Call-and-Response Klavier ↔ dunkler Lead. Eigene Komposition.

use super::common::*;
use crate::game::music::arr::*;
use crate::game::music::engine::*;
use crate::game::music::inst::*;

const THEME: &str = "a5:6 f5:2 d5:4 e5:4 | f5:6 d5:2 bb4:4 c5:4 | d5:4 bb4:2 c5:2 d5:4 g5:4 | e5:6 c#5:2 a4:8 |
                     a5:4 d6:4 c6:2 a5:2 f5:4 | g5:6 bb5:2 eb6:4 d6:4 | e6:4 c6:2 g5:2 e5:4 g5:4 | c#6:8 a5:4 e5:4";
const CALL: &str = "d5:2 f5:2 a5:4 r:8 | bb5:2 a5:2 f5:4 r:8 | g5:2 bb5:2 d6:4 r:8 | c#6:4 e6:4 r:8 |
                    d6:2 c6:2 a5:4 r:8 | eb6:2 d6:2 bb5:4 r:8 | c6:2 e6:2 g6:4 r:8 | e6:4 c#6:4 r:8";
const RESP: &str = "r:8 a5:2 g5:2 f5:2 e5:2 | r:8 d5:2 f5:2 bb5:4 | r:8 a5:2 g5:2 d5:4 | r:8 a5:2 g5:2 e5:4 |
                    r:8 f5:2 e5:2 d5:2 a4:2 | r:8 g5:2 bb5:2 eb6:4 | r:8 g5:2 e5:2 c5:4 | r:8 c#5:2 e5:2 a5:4";
const MAIN: &str = "Dm Bb Gm A Dm Eb C A";

pub fn build() -> Score {
    let mut a = Arr::new(164.0, 76);
    a.prog(0, 8, "Dm Bb Gm A");
    a.prog(8, 24, MAIN);
    a.prog(32, 8, "Dm:8 Bb:8 Gm:8 A:8");
    a.prog(40, 4, "Gm A Bb A");
    a.prog(44, 24, MAIN);
    a.prog(68, 8, "Dm Bb Gm A");

    let piano = Tone::Piano { pedal: 0.0 };
    let ped = Tone::Piano { pedal: 1.0 };
    let low_choir = Tone::Choir { vowel: 0.9 };
    let choir = Tone::Choir { vowel: 0.3 };
    let gong = Tone::Bell { ratio: 1.41, index: 2.2, decay: 2.6 };
    let mut dp = SynthP::pad(); dp.cut = 650.0; dp.env_cut = 300.0; dp.lfo = 0.07; dp.lfo_amt = 0.4; dp.det = 11.0;
    let dark_pad = Tone::Synth(dp);
    let mut ld = SynthP::supersaw(); ld.cut = 1500.0; ld.env_cut = 2200.0; ld.det = 28.0; ld.drive = 1.4;
    let lead = Tone::Synth(ld);
    let kit = Kit { kick_tone: 90.0, kick_decay: 0.16, snare_tone: 170.0, snare_decay: 0.1, ride: 0.3, hat: 0.25, crunch: 2.1, layer_kick: 0.95, layer_snare: 0.55, clap: 0.25 };
    let brk = a.make_break(kit);
    let gongs = |a: &mut Arr, bar0: usize, bars: usize, v: f32| {
        for (t, _, ch) in a.spans(bar0, bars) {
            if (t as usize / 4) % 2 == 0 { let m = ch.bass_note(38) as f32; a.play(Bus::Keys, gong, t, 4.0, m, v, -0.2); }
        }
    };
    // Sub-Drop: von einer Oktave höher herunter gleiten
    let sub_drop = |a: &mut Arr, t: f32, beats: f32, v: f32| {
        let ch = a.chord(t);
        let m = ch.bass_note(26) as f32;
        let d = a.secs(beats);
        a.add(t, Bus::Bass, 0.0, Inst::Sub { midi: m, dur: d, vel: v, from: m + 12.0, glide: 0.25 });
    };

    // ---- Intro (1–8): Leere, tiefer Chor, Gongs, einzelne Klaviertöne
    a.air(0.0, 34.0, 0.3, 420.0, 2.5, 0.0);
    a.pad(0, 8, 50, 3, Bus::Pad, low_choir, 0.55);
    a.pad(0, 8, 57, 3, Bus::Pad, dark_pad, 0.5);
    gongs(&mut a, 0, 8, 0.45);
    a.mel(4, 4, THEME, Bus::Piano, ped, 0.55, 0, 0.1);
    a.mel(4, 4, THEME, Bus::Piano, ped, 0.3, -12, -0.1);
    for b in [0usize, 2, 4, 6] { sub_drop(&mut a, b as f32 * 4.0, 6.0, 0.7); }

    // ---- Aufbau (9–16): Halftime-Breaks schälen sich aus dem Filter
    a.breaks(8, 8, &brk, &HALF, &[], 0, 0.8, 0.6);
    a.lp(Bus::Drums, 32.0, 63.0, 300.0, 7000.0, true);
    a.mel(8, 8, THEME, Bus::Piano, piano, 0.8, 0, 0.1);
    a.mel(8, 8, THEME, Bus::Piano, piano, 0.4, -12, -0.1);
    a.pad(8, 8, 50, 3, Bus::Pad, low_choir, 0.55);
    a.pad(8, 8, 57, 3, Bus::Pad, dark_pad, 0.5);
    gongs(&mut a, 8, 8, 0.4);
    a.bass(8, 8, "x---------------", 0.8, Some((0.25, 2.0, 90.0, 400.0, 2.0)));
    a.riser(64.0, 16.0, 0.6, true);
    a.rev_crash(64.0, 4.0, 0.6);
    a.tape_stop(62.0, 2.0);

    // ---- Drop 1 (17–32): erst schwerer Halftime, dann volle Breaks
    a.impact(64.0, 1.0);
    a.crash(64.0, 0.7);
    a.breaks(16, 8, &brk, &HALF, &[HALF_FILL], 4, 0.95, 1.0);
    a.bass(16, 8, "x-------x-----o-", 1.0, Some((0.55, 2.0, 90.0, 900.0, 2.6)));
    a.comp(16, 8, "X.......x.....x.", 57, 4, Bus::Piano, piano, 0.55);
    a.mel(16, 16, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(16, 16, THEME, Bus::Piano, piano, 0.45, -12, -0.1);
    a.pad(16, 16, 50, 3, Bus::Pad, low_choir, 0.6);
    a.pad(16, 16, 57, 3, Bus::Pad, dark_pad, 0.5);
    gongs(&mut a, 16, 16, 0.35);
    drop_in(&mut a, 96.0, 4.0, true, 0.8);
    a.breaks(24, 8, &brk, &GROOVE, &FILLS[2..], 4, 0.92, 1.0);
    a.bass(24, 8, "x-----x---x---o-", 1.0, Some((0.45, 1.0, 100.0, 1100.0, 2.6)));
    a.mel(24, 8, THEME, Bus::Lead, lead, 0.4, 0, 0.0);
    a.pad(24, 8, 62, 3, Bus::Pad, choir, 0.5);
    a.stutter(127.0, 1.0, 0.25, true);

    // ---- Breakdown (33–40): Klavier allein in der Leere, Thema halb so schnell
    a.down(128.0, 8.0, 0.6);
    a.impact(128.0, 0.6);
    a.air(128.0, 32.0, 0.3, 420.0, 2.5, 0.0);
    a.mel_x(32, 8, THEME, 2.0, Bus::Piano, ped, 0.85, 0, 0.1);
    a.mel_x(32, 8, THEME, 2.0, Bus::Piano, ped, 0.4, -12, -0.1);
    a.pad(32, 8, 50, 3, Bus::Pad, low_choir, 0.6);
    a.pad(36, 4, 62, 3, Bus::Pad, choir, 0.45);
    gongs(&mut a, 32, 8, 0.4);
    for b in [32usize, 36] { sub_drop(&mut a, b as f32 * 4.0, 12.0, 0.6); }

    // ---- Aufbau 2 (41–44)
    a.breaks(40, 4, &brk, &STRAIGHT, &[], 0, 0.8, 0.5);
    a.hp(Bus::Drums, 160.0, 175.0, 2000.0, 40.0, true);
    a.arp(40, 4, "0 1 2 3 4 5 6 7", 1.0, 62, Bus::Piano, piano, 0.3, 0.8);
    a.pad(40, 4, 57, 3, Bus::Pad, dark_pad, 0.6);
    a.bass(40, 4, "x-------x-------", 0.8, None);
    snare_roll(&mut a, 168.0, 8.0, &kit, 0.15, 0.9);
    drop_in(&mut a, 176.0, 16.0, true, 1.0);

    // ---- Drop 2 (45–60): Ruf (Klavier) und Antwort (dunkler Lead), dann das Thema
    a.breaks(44, 8, &brk, &CHOPPY, &FILLS[..2], 4, 0.92, 1.0);
    a.breaks(52, 8, &brk, &GLITCH, &FILLS[1..3], 4, 0.92, 1.0);
    a.mel(44, 8, CALL, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(44, 8, CALL, Bus::Piano, piano, 0.45, -12, -0.1);
    a.mel(44, 8, RESP, Bus::Lead, lead, 0.55, 0, 0.0);
    a.mel(52, 8, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(52, 8, THEME, Bus::Piano, piano, 0.45, -12, -0.1);
    a.mel(52, 8, THEME, Bus::Lead, lead, 0.45, 12, 0.0);
    a.pad(44, 16, 50, 3, Bus::Pad, low_choir, 0.55);
    a.pad(44, 16, 62, 3, Bus::Pad, choir, 0.5);
    gongs(&mut a, 44, 16, 0.35);
    a.bass(44, 16, "x-----x---x-x-o-", 1.0, Some((0.48, 0.5, 100.0, 1400.0, 2.8)));
    a.crash(208.0, 0.6);
    a.stutter(239.0, 1.0, 0.125, true);

    // ---- Halftime-Ausklang (61–68)
    a.impact(240.0, 0.9);
    a.crash(240.0, 0.6);
    a.breaks(60, 8, &brk, &HALF, &[HALF_FILL], 4, 0.9, 1.0);
    a.bass(60, 8, "x-------x-----o-", 1.0, Some((0.55, 2.0, 90.0, 900.0, 2.6)));
    a.comp(60, 8, "X.......x.....x.", 57, 4, Bus::Piano, piano, 0.55);
    a.mel_x(60, 8, CALL, 2.0, Bus::Lead, lead, 0.4, 0, 0.0);
    a.pad(60, 8, 50, 3, Bus::Pad, low_choir, 0.6);
    a.lp(Bus::Drums, 256.0, 271.0, 16000.0, 600.0, true);
    a.tape_stop(270.0, 2.0);

    // ---- Outro (69–76): zurück in die Leere
    a.air(272.0, 32.0, 0.3, 420.0, 2.5, 0.0);
    a.pad(68, 8, 50, 3, Bus::Pad, low_choir, 0.55);
    a.pad(68, 8, 57, 3, Bus::Pad, dark_pad, 0.5);
    gongs(&mut a, 68, 8, 0.45);
    a.mel(68, 4, THEME, Bus::Piano, ped, 0.6, 0, 0.1);
    for b in [68usize, 72] { sub_drop(&mut a, b as f32 * 4.0, 8.0, 0.65); }

    a.sc.mix.hall = (0.9, 0.4, 1.5, 0.035, 1.1);
    a.sc.mix.bus[Bus::Drums as usize].drive = 1.6;
    a.sc.mix.bus[Bus::Bass as usize].gain = 1.05;
    a.sc.mix.master = 0.8;
    a.finish()
}
