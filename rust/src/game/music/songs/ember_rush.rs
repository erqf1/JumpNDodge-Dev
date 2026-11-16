//! „Ember Rush“ – feurig und treibend. 180 BPM, a-Moll.
//! Durchlaufendes Klavier-Ostinato in Sechzehnteln, harte Breaks mit Clap-Schicht, verzerrter
//! Reese, Supersaw-Lead, Halftime-Brücke mit Call-and-Response. Eigene Komposition.

use super::common::*;
use crate::game::music::arr::*;
use crate::game::music::engine::*;
use crate::game::music::inst::*;

const THEME: &str = "e6:3 c6:3 a5:2 b5:2 c6:2 e6:4 | f6:3 e6:3 c6:2 a5:4 c6:4 | g6:3 e6:3 c6:2 d6:2 e6:2 g6:4 | d6:6 b5:2 g5:4 b5:4 |
                     a6:3 g6:3 e6:2 c6:2 d6:2 e6:4 | f6:3 e6:3 d6:2 c6:4 a5:4 | b5:3 d6:3 g6:2 f6:2 d6:2 b5:4 | g#5:6 b5:2 e6:4 g#6:4";
const CALL: &str = "a5:2 c6:2 e6:2 a6:2 r:8 | a6:2 g6:2 f6:4 r:8 | e6:2 g6:2 c7:4 r:8 | b6:4 g6:4 r:8 |
                    c7:2 b6:2 a6:2 e6:2 r:8 | f6:2 a6:2 c7:4 r:8 | d7:2 b6:2 g6:4 r:8 | g#6:4 b6:4 r:8";
const RESP: &str = "r:8 e6:2 d6:2 c6:2 b5:2 | r:8 c6:2 d6:2 e6:4 | r:8 g6:2 e6:2 d6:2 c6:2 | r:8 d6:2 e6:2 d6:4 |
                    r:8 e6:2 f6:2 g6:2 a6:2 | r:8 a6:2 g6:2 f6:4 | r:8 g6:2 a6:2 b6:4 | r:8 e6:2 g#6:2 b6:4";
const MAIN: &str = "Am F C G Am F G E";

pub fn build() -> Score {
    let mut a = Arr::new(180.0, 72);
    a.prog(0, 40, MAIN);
    a.prog(40, 4, "Am F C G");
    a.prog(44, 4, "F G E E");
    a.prog(48, 24, MAIN);

    let piano = Tone::Piano { pedal: 0.0 };
    let ped = Tone::Piano { pedal: 0.6 };
    let mut sw = SynthP::supersaw(); sw.drive = 1.3; sw.cut = 4200.0;
    let saw = Tone::Synth(sw);
    let mut pp = SynthP::pad(); pp.cut = 2000.0;
    let pad = Tone::Synth(pp);
    let choir = Tone::Choir { vowel: 0.1 };
    let bell = Tone::Bell { ratio: 3.0, index: 1.8, decay: 0.7 };
    let kit = Kit { kick_tone: 120.0, kick_decay: 0.12, snare_tone: 195.0, snare_decay: 0.085, ride: 0.45, hat: 0.55, crunch: 2.6, layer_kick: 0.9, layer_snare: 0.5, clap: 0.6 };
    let brk = a.make_break(kit);
    let ost = "0 2 1 2 0 2 1 3";

    // ---- Intro (1–8): das Ostinato glüht auf
    a.arp(0, 8, ost, 1.0, 57, Bus::Piano, piano, 0.36, 0.9);
    a.lp(Bus::Piano, 0.0, 31.0, 900.0, 12000.0, true);
    a.pad(4, 4, 64, 3, Bus::Pad, pad, 0.5);
    hats16(&mut a, 4, 4, 0.25, 0);
    a.riser(32.0, 8.0, 0.45, false);

    // ---- Aufbau (9–16)
    a.arp(8, 8, ost, 1.0, 57, Bus::Piano, piano, 0.34, 0.9);
    a.mel(8, 8, THEME, Bus::Piano, piano, 0.85, 0, 0.1);
    a.breaks(8, 8, &brk, &CHOPPY, &[FILLS[0]], 4, 0.8, 0.5);
    a.lp(Bus::Drums, 32.0, 63.0, 600.0, 14000.0, true);
    a.bass(8, 8, "x.x.x.x.x.x.x.x.", 0.7, None);
    a.pad(8, 8, 64, 4, Bus::Pad, pad, 0.55);
    snare_roll(&mut a, 56.0, 8.0, &kit, 0.15, 0.95);
    drop_in(&mut a, 64.0, 8.0, true, 1.0);

    // ---- Drop 1 (17–32)
    a.breaks(16, 16, &brk, &CHOPPY, &FILLS, 4, 0.92, 1.0);
    hats16(&mut a, 16, 16, 0.3, 2);
    a.arp(16, 16, ost, 1.0, 57, Bus::Piano, piano, 0.28, 0.9);
    a.mel(16, 16, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(16, 16, THEME, Bus::Piano, piano, 0.42, -12, -0.1);
    a.mel(16, 16, THEME, Bus::Lead, saw, 0.45, 0, 0.0);
    a.pad(16, 16, 64, 4, Bus::Pad, pad, 0.55);
    a.pad(24, 8, 64, 3, Bus::Pad, choir, 0.5);
    a.bass(16, 16, "x-x--x-x-x--x-o-", 1.0, Some((0.42, 0.5, 200.0, 2600.0, 3.2)));
    a.crash(96.0, 0.6);
    a.stutter(111.0, 1.0, 0.25, true);
    a.stutter(127.0, 1.0, 0.125, true);

    // ---- Brücke (33–40): Halftime, Klavier ruft, Supersaw antwortet
    a.impact(128.0, 0.9);
    a.crash(128.0, 0.6);
    a.breaks(32, 8, &brk, &HALF, &[HALF_FILL], 4, 0.95, 1.0);
    a.mel(32, 8, CALL, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(32, 8, CALL, Bus::Piano, piano, 0.4, -12, -0.1);
    a.mel(32, 8, RESP, Bus::Lead, saw, 0.55, 0, 0.0);
    a.mel(32, 8, RESP, Bus::Keys, bell, 0.4, 12, 0.3);
    a.pad(32, 8, 64, 4, Bus::Pad, choir, 0.55);
    a.bass(32, 8, "x-------x-----o-", 1.0, Some((0.5, 1.0, 160.0, 2000.0, 3.0)));
    a.tape_stop(158.0, 2.0);

    // ---- Breakdown (41–44): kurz Luft holen
    a.mel(40, 4, THEME, Bus::Piano, ped, 0.8, 0, 0.1);
    a.arp(40, 4, "0 1 2 1", 2.0, 57, Bus::Piano, ped, 0.25, 1.2);
    a.pad(40, 4, 64, 4, Bus::Pad, pad, 0.55);
    a.pad(40, 4, 60, 3, Bus::Pad, choir, 0.45);

    // ---- Aufbau 2 (45–48)
    a.arp(44, 4, ost, 1.0, 57, Bus::Piano, piano, 0.36, 0.9);
    a.breaks(44, 4, &brk, &GLITCH, &[], 0, 0.8, 0.6);
    a.hp(Bus::Drums, 176.0, 191.0, 1500.0, 40.0, true);
    a.bass(44, 4, "x.x.x.x.x.x.x.x.", 0.75, None);
    snare_roll(&mut a, 184.0, 8.0, &kit, 0.2, 1.0);
    drop_in(&mut a, 192.0, 16.0, true, 1.0);

    // ---- Drop 2 (49–64): Thema eine Oktave höher, dann Ruf und Antwort in voller Fahrt
    a.breaks(48, 16, &brk, &GLITCH, &FILLS, 4, 0.92, 1.0);
    hats16(&mut a, 48, 16, 0.3, 1);
    a.arp(48, 16, ost, 1.0, 57, Bus::Piano, piano, 0.28, 0.9);
    a.mel(48, 8, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(48, 8, THEME, Bus::Piano, piano, 0.42, -12, -0.1);
    a.mel(48, 8, THEME, Bus::Lead, saw, 0.5, 12, 0.0);
    a.mel(56, 8, CALL, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(56, 8, CALL, Bus::Piano, piano, 0.42, -12, -0.1);
    a.mel(56, 8, RESP, Bus::Lead, saw, 0.55, 0, 0.0);
    a.mel(56, 8, RESP, Bus::Keys, bell, 0.4, 12, 0.3);
    a.pad(48, 16, 64, 4, Bus::Pad, pad, 0.55);
    a.pad(48, 16, 64, 3, Bus::Pad, choir, 0.5);
    a.bass(48, 16, "x-x--x-x-x--x-o-", 1.0, Some((0.45, 0.5, 200.0, 2800.0, 3.4)));
    a.crash(224.0, 0.7);
    a.impact(224.0, 0.5);
    a.stutter(239.0, 1.0, 0.25, true);
    a.stutter(255.0, 1.0, 0.125, true);

    // ---- Outro (65–72): Glut – Ostinato und gefilterte Breaks
    a.arp(64, 8, ost, 1.0, 57, Bus::Piano, piano, 0.34, 0.9);
    a.lp(Bus::Piano, 256.0, 287.9, 12000.0, 900.0, false);
    a.mel(64, 4, THEME, Bus::Piano, ped, 0.7, 0, 0.1);
    a.breaks(64, 6, &brk, &CHOPPY, &[], 0, 0.75, 0.5);
    a.lp(Bus::Drums, 256.0, 280.0, 12000.0, 400.0, true);
    a.bass(64, 4, "x---------------", 0.7, None);
    a.pad(64, 8, 64, 3, Bus::Pad, pad, 0.45);

    a.sc.mix.bus[Bus::Lead as usize].gain = 1.2;
    a.sc.mix.master = 0.8;
    a.finish()
}
