//! „Gearbreaker“ – chaotisch-mechanisch. 182 BPM, e-Moll (harmonisch).
//! Tickendes Uhrwerk, Pluck-Ostinato in Sechzehnteln, zerhackte Breaks mit Stottern und
//! Bandstopps, schneller Reese, Spieluhr im Breakdown, Call-and-Response Klavier ↔ Square-Lead.
//! Eigene Komposition.

use super::common::*;
use crate::game::music::arr::*;
use crate::game::music::engine::*;
use crate::game::music::inst::*;

const THEME: &str = "e5:2 g5:2 b5:2 e6:2 d6:2 b5:2 g5:2 b5:2 | c6:3 b5:1 g5:4 e5:2 g5:2 c6:4 | a5:2 c6:2 e6:2 a6:2 g6:2 e6:2 c6:2 e6:2 | d#6:6 f#6:2 b5:4 d#6:4 |
                     e6:2 d6:2 b5:2 g5:2 a5:2 b5:2 d6:2 e6:2 | g6:6 e6:2 c6:4 e6:4 | c6:2 e6:2 a6:4 g6:2 e6:2 c6:4 | b5:4 d#6:4 f#6:4 a6:4";
const CALL: &str = "e6:2 e6:2 g6:2 b6:2 r:8 | c7:4 b6:2 g6:2 r:8 | a6:2 a6:2 c7:2 e7:2 r:8 | d#7:4 b6:4 r:8 |
                    e6:2 g6:2 b6:2 e7:2 r:8 | e7:4 d7:2 c7:2 r:8 | c7:2 b6:2 a6:2 e6:2 r:8 | f#6:4 a6:4 r:8";
const RESP: &str = "r:8 b6:2 a6:2 g6:2 e6:2 | r:8 e6:2 g6:2 c7:4 | r:8 e7:2 c7:2 a6:2 e6:2 | r:8 f#6:2 a6:2 b6:4 |
                    r:8 d7:2 b6:2 g6:2 e6:2 | r:8 g6:2 a6:2 b6:2 c7:2 | r:8 a6:2 c7:2 e7:4 | r:8 d#7:2 b6:2 f#6:2 d#6:2";
const MAIN: &str = "Em C Am B";

pub fn build() -> Score {
    let mut a = Arr::new(182.0, 80);
    a.prog(0, 40, MAIN);
    a.prog(40, 8, "Em:8 C:8 Am:8 B:8");
    a.prog(48, 32, MAIN);

    let piano = Tone::Piano { pedal: 0.0 };
    let ped = Tone::Piano { pedal: 0.5 };
    let mut pl = SynthP::pluck(); pl.cut = 900.0; pl.env_cut = 4200.0; pl.cut_tau = 0.06; pl.q = 1.6;
    let pluck = Tone::Synth(pl);
    let mut sq = SynthP::supersaw(); sq.wave = Wv::Square; sq.uni = 3; sq.det = 12.0; sq.cut = 1800.0; sq.env_cut = 4500.0; sq.cut_tau = 0.12; sq.s = 0.55; sq.vib = 0.0;
    let square = Tone::Synth(sq);
    let musicbox = Tone::Bell { ratio: 4.0, index: 1.1, decay: 1.4 };
    let tick = Tone::Bell { ratio: 2.76, index: 3.0, decay: 0.05 };
    let mut padp = SynthP::pad(); padp.cut = 1100.0; padp.uni = 4;
    let pad = Tone::Synth(padp);
    let choir = Tone::Choir { vowel: 0.4 };
    let kit = Kit { kick_tone: 125.0, kick_decay: 0.1, snare_tone: 205.0, snare_decay: 0.075, ride: 0.35, hat: 0.6, crunch: 2.4, layer_kick: 0.85, layer_snare: 0.55, clap: 0.0 };
    let brk = a.make_break(kit);
    // Uhrwerk: Tick-Tack in Achteln, abwechselnd links/rechts
    let clock = |a: &mut Arr, bar0: usize, bars: usize, v: f32| {
        for i in 0..bars * 8 {
            let t = bar0 as f32 * 4.0 + i as f32 * 0.5;
            let (m, p) = if i % 2 == 0 { (95.0, -0.55) } else { (88.0, 0.55) };
            a.play(Bus::Keys, tick, t, 0.1, m, v * if i % 8 == 0 { 1.2 } else { 1.0 }, p);
        }
    };
    let gears = "0 2 1 3 0 2 1 4";

    // ---- Intro (1–8): Uhrwerk, Ostinato, Klaviermotiv, gefilterte Glitch-Breaks
    clock(&mut a, 0, 8, 0.3);
    a.arp(0, 8, gears, 1.0, 64, Bus::Keys, pluck, 0.42, 0.55);
    a.lp(Bus::Keys, 0.0, 31.5, 600.0, 6000.0, true);
    a.mel(4, 4, THEME, Bus::Piano, ped, 0.6, 0, 0.1);
    a.pad(0, 8, 60, 3, Bus::Pad, pad, 0.45);
    a.breaks(4, 4, &brk, &GLITCH, &[], 0, 0.6, 0.0);
    a.hp(Bus::Drums, 16.0, 31.5, 2500.0, 600.0, true);

    // ---- Aufbau (9–16)
    clock(&mut a, 8, 8, 0.32);
    a.arp(8, 8, gears, 1.0, 64, Bus::Keys, pluck, 0.48, 0.55);
    a.mel(8, 8, THEME, Bus::Piano, piano, 0.8, 0, 0.1);
    a.breaks(8, 8, &brk, &CHOPPY, &[FILLS[3]], 4, 0.8, 0.5);
    a.lp(Bus::Drums, 32.0, 63.0, 900.0, 12000.0, true);
    a.bass(8, 8, "x-------x-------", 0.8, Some((0.25, 1.0, 140.0, 700.0, 2.0)));
    a.pad(8, 8, 60, 3, Bus::Pad, pad, 0.5);
    a.stutter(47.0, 1.0, 0.25, true);
    snare_roll(&mut a, 60.0, 4.0, &kit, 0.2, 0.9);
    drop_in(&mut a, 64.0, 8.0, true, 1.0);

    // ---- Drop 1 (17–32)
    a.breaks(16, 16, &brk, &CHOPPY, &FILLS, 4, 0.92, 1.0);
    hats16(&mut a, 16, 16, 0.32, 2);
    clock(&mut a, 16, 16, 0.22);
    a.mel(16, 16, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(16, 16, THEME, Bus::Piano, piano, 0.42, -12, -0.1);
    a.mel(24, 8, THEME, Bus::Lead, square, 0.45, 12, 0.0);
    a.arp(16, 16, gears, 1.0, 64, Bus::Keys, pluck, 0.38, 0.5);
    a.pad(16, 16, 60, 3, Bus::Pad, pad, 0.5);
    a.bass(16, 16, "x--x--x-x--x-xo-", 1.0, Some((0.38, 0.5, 180.0, 2400.0, 3.0)));
    a.crash(96.0, 0.6);
    for b in [19usize, 23, 27] { a.stutter(b as f32 * 4.0 + 3.0, 1.0, 0.125, true); }
    a.stutter(127.0, 1.0, 0.25, false);

    // ---- Glitch-Halftime (33–40)
    a.impact(128.0, 0.8);
    a.crash(128.0, 0.6);
    a.breaks(32, 8, &brk, &HALF, &[HALF_FILL], 4, 0.95, 1.0);
    a.bass(32, 8, "x-------x---x-o-", 1.0, Some((0.5, 0.25, 160.0, 2800.0, 3.2)));
    a.comp(32, 8, "X..x..x.....x...", 60, 3, Bus::Piano, piano, 0.6);
    a.arp(32, 8, gears, 0.5, 76, Bus::Keys, pluck, 0.3, 0.5);
    a.pad(32, 8, 60, 3, Bus::Pad, choir, 0.45);
    a.stutter(137.0, 1.0, 0.5, false);
    a.stutter(145.0, 2.0, 0.25, true);
    a.tape_stop(156.0, 4.0);

    // ---- Breakdown (41–48): Spieluhr, das Uhrwerk läuft langsam weiter
    a.mel_x(40, 8, THEME, 2.0, Bus::Keys, musicbox, 0.75, 12, 0.2);
    a.mel_x(40, 8, THEME, 2.0, Bus::Piano, ped, 0.55, 0, -0.1);
    a.pad(40, 8, 62, 4, Bus::Pad, pad, 0.55);
    a.pad(44, 4, 60, 3, Bus::Pad, choir, 0.45);
    for i in 0..16 { a.play(Bus::Keys, tick, 160.0 + i as f32 * 2.0, 0.1, if i % 2 == 0 { 95.0 } else { 88.0 }, 0.22, if i % 2 == 0 { -0.5 } else { 0.5 }); }
    a.bass(44, 4, "x---------------", 0.6, None);

    // ---- Aufbau 2 (49–56): Zahnräder ziehen an, Stottern wird immer schneller
    clock(&mut a, 48, 8, 0.3);
    a.arp(48, 8, gears, 1.0, 64, Bus::Keys, pluck, 0.5, 0.55);
    a.lp(Bus::Keys, 192.0, 223.0, 700.0, 9000.0, true);
    a.breaks(48, 8, &brk, &GLITCH, &[FILLS[1]], 4, 0.8, 0.6);
    a.hp(Bus::Drums, 192.0, 222.0, 1800.0, 30.0, true);
    a.mel(48, 8, THEME, Bus::Piano, piano, 0.75, 0, 0.1);
    a.bass(52, 4, "x-x-x-x-x-x-x-x-", 0.7, None);
    a.stutter(219.0, 1.0, 0.25, true);
    drop_in(&mut a, 224.0, 16.0, true, 1.0);

    // ---- Drop 2 (57–72): Ruf (Klavier) und Antwort (Square), dann alles
    a.breaks(56, 8, &brk, &GLITCH, &FILLS[1..], 4, 0.92, 1.0);
    a.breaks(64, 8, &brk, &CHOPPY, &FILLS, 4, 0.92, 1.0);
    hats16(&mut a, 56, 16, 0.32, 1);
    clock(&mut a, 56, 16, 0.2);
    a.mel(56, 8, CALL, Bus::Piano, piano, 1.0, -12, 0.1);
    a.mel(56, 8, CALL, Bus::Piano, piano, 0.4, -24, -0.1);
    a.mel(56, 8, RESP, Bus::Lead, square, 0.55, -12, 0.0);
    a.mel(64, 8, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(64, 8, THEME, Bus::Piano, piano, 0.42, -12, -0.1);
    a.mel(64, 8, THEME, Bus::Lead, square, 0.45, 12, 0.0);
    a.arp(56, 16, gears, 1.0, 64, Bus::Keys, pluck, 0.38, 0.5);
    a.pad(56, 16, 60, 3, Bus::Pad, pad, 0.5);
    a.pad(64, 8, 64, 3, Bus::Pad, choir, 0.5);
    a.bass(56, 16, "x--x--x-x--x-xo-", 1.0, Some((0.4, 0.5, 180.0, 2600.0, 3.0)));
    a.crash(256.0, 0.7);
    a.impact(256.0, 0.5);
    for b in [59usize, 63, 67] { a.stutter(b as f32 * 4.0 + 3.0, 1.0, 0.125, true); }
    a.tape_stop(286.0, 2.0);

    // ---- Outro (73–80): nur noch Uhrwerk und Ostinato, wie am Anfang
    clock(&mut a, 72, 8, 0.3);
    a.arp(72, 8, gears, 1.0, 64, Bus::Keys, pluck, 0.42, 0.55);
    a.lp(Bus::Keys, 288.0, 319.9, 6000.0, 600.0, false);
    a.mel(72, 4, THEME, Bus::Piano, ped, 0.65, 0, 0.1);
    a.pad(72, 8, 60, 3, Bus::Pad, pad, 0.45);
    a.breaks(72, 4, &brk, &HALF, &[], 0, 0.6, 0.4);
    a.bass(72, 4, "x---------------", 0.6, None);

    a.sc.mix.delay = (0.75, 0.32, 0.45);
    a.sc.mix.bus[Bus::Keys as usize].delay = 0.22;
    a.sc.mix.master = 0.8;
    a.finish()
}
