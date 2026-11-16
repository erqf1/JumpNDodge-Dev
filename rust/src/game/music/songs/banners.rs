//! „Banners in the Rain“ – episch-kämpferisch. 172 BPM, c-Moll.
//! Regen und Klavier im Intro, Kriegstrommeln im Aufbau, Drops mit zerhackten Breaks,
//! Chor und Supersaw, Halftime-Teil mit Reese, Breakdown mit Streichern, Call-and-Response im
//! zweiten Drop. Eigene Komposition.

use super::common::*;
use crate::game::music::arr::*;
use crate::game::music::engine::*;
use crate::game::music::inst::*;

/// Hauptthema (8 Takte über Cm Ab Eb Bb Cm Ab Bb G)
const THEME: &str = "g5:4 eb5:2 f5:2 g5:6 c6:2 | c6:3 bb5:3 ab5:2 g5:4 eb5:4 | bb5:4 g5:2 ab5:2 bb5:4 eb6:4 | d6:6 c6:2 bb5:4 f5:4 |
                     g5:4 eb5:2 f5:2 g5:4 c6:2 d6:2 | eb6:6 d6:2 c6:4 ab5:4 | d6:4 c6:2 bb5:2 c6:4 d6:4 | b5:8 g5:4 d6:4";
/// Ruf (Klavier) und Antwort (Glocken/Supersaw) im zweiten Drop
const CALL: &str = "c6:2 d6:2 eb6:4 r:8 | eb6:2 d6:2 c6:4 r:8 | bb5:2 c6:2 eb6:4 r:8 | f6:4 d6:4 r:8 |
                    c6:2 d6:2 eb6:4 r:8 | ab6:4 g6:4 r:8 | f6:2 eb6:2 d6:4 r:8 | d6:4 b5:4 r:8";
const RESP: &str = "r:8 g6:2 f6:2 eb6:2 d6:2 | r:8 c6:2 eb6:2 ab6:4 | r:8 g6:2 f6:2 eb6:2 bb5:2 | r:8 bb5:2 c6:2 d6:4 |
                    r:8 g6:2 ab6:2 g6:2 eb6:2 | r:8 eb6:2 f6:2 g6:4 | r:8 bb6:2 ab6:2 g6:2 f6:2 | r:8 g6:2 f6:2 d6:2 b5:2";
/// Breakdown-Melodie (über Ab Eb Bb Cm Ab Eb Bb G)
const BRK: &str = "eb5:4 ab5:4 c6:6 bb5:2 | g5:4 bb5:4 eb6:6 d6:2 | f5:4 bb5:4 d6:4 f6:4 | eb6:8 d6:4 c6:4 |
                   c6:4 eb6:4 ab6:6 g6:2 | g6:4 f6:2 eb6:2 bb5:8 | d6:4 f6:4 bb6:4 ab6:4 | g6:12 d6:4";

const MAIN: &str = "Cm Ab Eb Bb Cm Ab Bb G";

pub fn build() -> Score {
    let mut a = Arr::new(172.0, 76);
    a.prog(0, 8, "Cm Ab Eb Bb");
    a.prog(8, 8, MAIN);
    a.prog(16, 16, MAIN);
    a.prog(32, 8, "Cm:8 Ab:8 Eb:8 Bb:4 G:4");
    a.prog(40, 8, "Ab Eb Bb Cm Ab Eb Bb G");
    a.prog(48, 4, "Ab Bb Cm G");
    a.prog(52, 16, MAIN);
    a.prog(68, 8, "Cm Ab Eb Bb");

    let piano = Tone::Piano { pedal: 0.0 };
    let ped = Tone::Piano { pedal: 0.8 };
    let choir = Tone::Choir { vowel: 0.15 };
    let ooh = Tone::Choir { vowel: 0.8 };
    let bell = Tone::Bell { ratio: 3.5, index: 1.4, decay: 0.9 };
    let mut padp = SynthP::pad(); padp.cut = 1500.0;
    let pad = Tone::Synth(padp);
    let strings = Tone::Synth(SynthP::strings());
    let mut sawp = SynthP::supersaw(); sawp.cut = 3200.0;
    let saw = Tone::Synth(sawp);
    let kit = Kit { kick_tone: 105.0, kick_decay: 0.13, snare_tone: 185.0, snare_decay: 0.09, ride: 0.55, hat: 0.4, crunch: 1.7, layer_kick: 0.85, layer_snare: 0.5, clap: 0.35 };
    let brk = a.make_break(kit);
    let taiko = |a: &mut Arr, t: f32, v: f32| a.add(t, Bus::Perc, 0.0, Inst::Tom { midi: 36.0, vel: v, decay: 0.38 });
    let low_roots = |a: &mut Arr, bar0: usize, bars: usize, tone: Tone, v: f32| {
        for (t, d, ch) in a.spans(bar0, bars) { a.play(Bus::Piano, tone, t, d, ch.bass_note(36) as f32, v, 0.0); }
    };

    // ---- Intro (Takt 1–8): Regen, Klavier, Chor
    a.air(0.0, 34.0, 0.32, 2600.0, 0.9, 7.0);
    a.arp(0, 8, "0 1 2 3 2 1 2 1", 2.0, 57, Bus::Piano, ped, 0.34, 1.8);
    low_roots(&mut a, 0, 8, ped, 0.45);
    a.pad(0, 8, 62, 3, Bus::Pad, ooh, 0.45);
    a.mel(4, 4, THEME, Bus::Piano, ped, 0.55, 0, 0.1);
    for b in [6usize, 7] { for q in [0.0, 2.0] { taiko(&mut a, b as f32 * 4.0 + q, 0.5); } }

    // ---- Aufbau (9–16): Breaks kommen gefiltert, Trommeln, Thema
    a.breaks(8, 8, &brk, &GROOVE, &[], 0, 0.75, 0.0);
    a.lp(Bus::Drums, 32.0, 63.5, 450.0, 9000.0, true);
    for i in 0..16 { taiko(&mut a, 32.0 + i as f32, 0.55 + 0.02 * i as f32); }
    for i in 0..32 { taiko(&mut a, 48.0 + i as f32 * 0.5, if i % 2 == 0 { 0.75 } else { 0.45 }); }
    a.mel(8, 8, THEME, Bus::Piano, piano, 0.8, 0, 0.1);
    a.arp(8, 8, "0 1 2 1", 2.0, 52, Bus::Piano, piano, 0.26, 0.9);
    a.pad(8, 8, 64, 4, Bus::Pad, pad, 0.55);
    a.pad(12, 4, 62, 3, Bus::Pad, choir, 0.45);
    a.bass(12, 4, "x---------------", 0.75, None);
    snare_roll(&mut a, 60.0, 4.0, &kit, 0.2, 0.85);
    drop_in(&mut a, 64.0, 8.0, true, 1.0);

    // ---- Drop 1 (17–32)
    a.breaks(16, 8, &brk, &GROOVE, &FILLS[..2], 4, 0.9, 1.0);
    a.breaks(24, 8, &brk, &CHOPPY, &FILLS[1..3], 4, 0.9, 1.0);
    a.mel(16, 16, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(16, 16, THEME, Bus::Piano, piano, 0.42, -12, -0.1);
    a.mel(24, 8, THEME, Bus::Lead, saw, 0.5, 0, 0.0);
    a.arp(24, 8, "0 1 2 3 4 3 2 1", 1.0, 79, Bus::Keys, bell, 0.32, 0.8);
    a.arp(16, 16, "0 1 2 1", 2.0, 52, Bus::Piano, piano, 0.24, 0.9);
    a.pad(16, 16, 64, 4, Bus::Pad, pad, 0.6);
    a.pad(24, 8, 62, 3, Bus::Pad, choir, 0.55);
    a.bass(16, 16, "x-----x---x---o-", 1.0, Some((0.34, 2.0, 150.0, 900.0, 1.8)));
    a.crash(96.0, 0.6);
    a.stutter(127.0, 1.0, 0.25, true);

    // ---- Halftime (33–40): Reese, Chor, Thema doppelt so langsam
    a.impact(128.0, 0.8);
    a.crash(128.0, 0.7);
    a.breaks(32, 8, &brk, &HALF, &[HALF_FILL], 4, 0.95, 1.0);
    a.bass(32, 8, "x-------x-----o-", 1.0, Some((0.5, 0.5, 120.0, 1500.0, 2.2)));
    a.pad(32, 8, 62, 4, Bus::Pad, choir, 0.7);
    a.pad(32, 8, 66, 4, Bus::Pad, pad, 0.45);
    a.comp(32, 8, "X.....x.x.......", 60, 4, Bus::Piano, piano, 0.55);
    a.mel_x(32, 8, THEME, 2.0, Bus::Lead, saw, 0.45, 0, 0.0);
    a.mel_x(32, 8, THEME, 2.0, Bus::Piano, piano, 0.6, 12, 0.15);
    a.tape_stop(158.0, 2.0);

    // ---- Breakdown (41–48): Klavier allein mit Streichern, Regen
    a.rev_piano(160.0, 4.0, &[56, 60, 63, 68], 0.7);
    a.air(160.0, 32.0, 0.28, 2400.0, 0.9, 6.0);
    a.mel(40, 8, BRK, Bus::Piano, ped, 0.85, 0, 0.1);
    a.arp(40, 8, "0 1 2 3 4 3 2 1", 1.0, 60, Bus::Piano, ped, 0.18, 1.5);
    low_roots(&mut a, 40, 8, ped, 0.5);
    a.pad(40, 8, 62, 4, Bus::Pad, strings, 0.5);
    a.pad(44, 4, 64, 3, Bus::Pad, ooh, 0.5);
    a.breaks(44, 4, &brk, &STRAIGHT, &[], 0, 0.55, 0.0);
    a.lp(Bus::Drums, 176.0, 192.0, 700.0, 3500.0, false);
    a.bass(44, 4, "x---------------", 0.7, None);

    // ---- Aufbau 2 (49–52)
    a.breaks(48, 4, &brk, &GROOVE, &[], 0, 0.85, 0.6);
    a.lp(Bus::Drums, 192.0, 207.0, 3500.0, 16000.0, true);
    a.arp(48, 4, "0 1 2 3 4 5 6 7", 1.0, 64, Bus::Piano, piano, 0.3, 0.8);
    a.pad(48, 4, 64, 4, Bus::Pad, choir, 0.6);
    a.bass(48, 4, "x-------x-------", 0.85, None);
    snare_roll(&mut a, 200.0, 8.0, &kit, 0.15, 0.9);
    drop_in(&mut a, 208.0, 16.0, true, 1.0);

    // ---- Drop 2 (53–68): Ruf und Antwort, dann Thema eine Oktave höher
    a.breaks(52, 8, &brk, &CHOPPY, &FILLS[2..], 4, 0.9, 1.0);
    a.breaks(60, 8, &brk, &GLITCH, &FILLS[..2], 4, 0.9, 1.0);
    a.mel(52, 8, CALL, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(52, 8, CALL, Bus::Piano, piano, 0.45, -12, -0.1);
    a.mel(52, 8, RESP, Bus::Keys, bell, 0.6, 0, 0.3);
    a.mel(52, 8, RESP, Bus::Lead, saw, 0.4, -12, -0.2);
    a.mel(60, 8, THEME, Bus::Lead, saw, 0.5, 12, 0.0);
    a.mel(60, 8, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(60, 8, THEME, Bus::Piano, piano, 0.42, -12, -0.1);
    a.arp(60, 8, "0 1 2 3 4 3 2 1", 1.0, 79, Bus::Keys, bell, 0.3, 0.8);
    a.arp(52, 16, "0 1 2 1", 2.0, 52, Bus::Piano, piano, 0.24, 0.9);
    a.pad(52, 16, 64, 4, Bus::Pad, pad, 0.6);
    a.pad(52, 16, 62, 3, Bus::Pad, choir, 0.55);
    a.bass(52, 8, "x-----x---x---o-", 1.0, Some((0.36, 1.0, 150.0, 1100.0, 2.0)));
    a.bass(60, 8, "x--x--x---x-x-o-", 1.0, Some((0.38, 0.5, 150.0, 1300.0, 2.2)));
    a.crash(240.0, 0.7);
    a.impact(240.0, 0.5);
    a.stutter(255.0, 1.0, 0.25, true);
    a.stutter(271.0, 1.0, 0.5, false);

    // ---- Outro (69–76): zurück zum Regen
    a.impact(272.0, 0.6);
    a.crash(272.0, 0.6);
    a.breaks(68, 4, &brk, &HALF, &[], 0, 0.7, 0.6);
    a.lp(Bus::Drums, 272.0, 288.0, 12000.0, 500.0, false);
    a.lp(Bus::Drums, 288.01, 304.0, 500.0, 500.0, true);
    a.mel(68, 4, THEME, Bus::Piano, ped, 0.7, 0, 0.1);
    a.arp(68, 8, "0 1 2 3 2 1 2 1", 2.0, 57, Bus::Piano, ped, 0.32, 1.8);
    low_roots(&mut a, 68, 8, ped, 0.45);
    a.pad(68, 8, 62, 3, Bus::Pad, ooh, 0.45);
    a.bass(68, 4, "x---------------", 0.6, None);
    a.air(272.0, 32.0, 0.3, 2600.0, 0.9, 7.0);

    a.sc.mix.master = 0.8;
    a.finish()
}
