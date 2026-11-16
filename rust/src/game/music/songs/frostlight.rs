//! „Frostlight“ – kühl und eisig. 168 BPM, h-Moll mit add9/maj7-Farben.
//! Wind, glitzernde Glocken-Arpeggios mit Ping-Pong-Echo, hohe Flächen und „ooh“-Chor,
//! luftige Breaks mit Ride, sauberer Sub. Eigene Komposition.

use super::common::*;
use crate::game::music::arr::*;
use crate::game::music::engine::*;
use crate::game::music::inst::*;

const THEME: &str = "f#5:4 b5:4 c#6:2 d6:6 | d6:2 c#6:2 b5:4 f#5:4 g5:4 | a5:6 f#5:2 a5:4 d6:4 | c#6:6 e6:2 c#6:4 a5:4 |
                     b5:4 g5:2 a5:2 b5:4 e6:4 | f#6:6 e6:2 d6:4 b5:4 | c#6:4 e6:4 a6:4 e6:4 | a#5:8 c#6:4 f#5:4";
const THEME_B: &str = "d6:2 c#6:2 b5:2 c#6:2 d6:4 f#6:4 | g6:2 f#6:2 d6:2 b5:2 d6:8 | a6:4 f#6:2 e6:2 d6:4 a5:4 | e6:2 c#6:2 a5:2 c#6:2 e6:8 |
                       g6:2 f#6:2 e6:2 d6:2 e6:4 b5:4 | d6:2 e6:2 f#6:4 b6:8 | a6:4 g6:2 f#6:2 e6:4 c#6:4 | c#6:4 e6:4 f#6:4 a#6:4";
/// Glocken-Antworten in den Lücken der Melodie
const SPARKLE: &str = "r:8 f#6:2 a6:2 b6:4 | r:8 d7:2 c#7:2 b6:4 | r:8 a6:2 f#6:2 d6:4 | r:8 e6:2 a6:2 c#7:4 |
                       r:8 b6:2 g6:2 e6:4 | r:8 f#6:2 g6:2 b6:4 | r:8 c#7:2 e7:2 a6:4 | r:8 a#6:2 c#7:2 f#7:4";
const MAIN: &str = "Bmadd9 Gmaj7 D A Em7 Gmaj7 A F#7";

pub fn build() -> Score {
    let mut a = Arr::new(168.0, 76);
    a.prog(0, 8, "Bmadd9 Gmaj7 D A");
    a.prog(8, 32, MAIN);
    a.prog(40, 8, "Bmadd9:8 Gmaj7:8 D:8 A:8");
    a.prog(48, 4, "Em7 Gmaj7 A F#7");
    a.prog(52, 16, MAIN);
    a.prog(68, 8, "Bmadd9 Gmaj7 D A");

    let piano = Tone::Piano { pedal: 0.0 };
    let ped = Tone::Piano { pedal: 0.9 };
    let glass = Tone::Bell { ratio: 3.5, index: 2.2, decay: 1.6 };
    let chime = Tone::Bell { ratio: 5.19, index: 1.3, decay: 2.4 };
    let harp = Tone::Pluck { bright: 0.85, decay: 1.4 };
    let mut hp = SynthP::pad(); hp.cut = 4200.0; hp.env_cut = 0.0; hp.lfo = 0.09; hp.lfo_amt = 0.35; hp.a = 0.9; hp.det = 18.0; hp.uni = 5;
    let high_pad = Tone::Synth(hp);
    let ooh = Tone::Choir { vowel: 0.85 };
    let mut ld = SynthP::supersaw(); ld.cut = 2600.0; ld.env_cut = 2500.0; ld.wave = Wv::Tri; ld.uni = 5; ld.det = 16.0;
    let lead = Tone::Synth(ld);
    let kit = Kit { kick_tone: 100.0, kick_decay: 0.12, snare_tone: 200.0, snare_decay: 0.08, ride: 0.85, hat: 0.5, crunch: 1.3, layer_kick: 0.75, layer_snare: 0.42, clap: 0.0 };
    let brk = a.make_break(kit);
    let sparkle_arp = "0 1 2 3 4 5 4 3 2 1 2 3";

    // ---- Intro (1–8): Wind, Glocken, hohe Fläche
    a.air(0.0, 34.0, 0.22, 3200.0, 1.4, 0.0);
    a.arp(0, 8, sparkle_arp, 1.0, 86, Bus::Keys, glass, 0.3, 0.6);
    a.pad(0, 8, 74, 4, Bus::Pad, high_pad, 0.6);
    a.pad(4, 4, 66, 3, Bus::Pad, ooh, 0.45);
    a.mel(4, 4, THEME, Bus::Piano, ped, 0.5, 0, 0.1);

    // ---- Aufbau (9–16)
    a.arp(8, 8, sparkle_arp, 1.0, 86, Bus::Keys, glass, 0.3, 0.6);
    a.arp(8, 8, "0 1 2 1", 2.0, 59, Bus::Piano, ped, 0.24, 1.0);
    a.mel(8, 8, THEME, Bus::Piano, piano, 0.8, 0, 0.1);
    a.pad(8, 8, 74, 4, Bus::Pad, high_pad, 0.6);
    a.breaks(8, 8, &brk, &STRAIGHT, &[], 0, 0.7, 0.0);
    a.hp(Bus::Drums, 32.0, 63.0, 3500.0, 40.0, true);
    a.bass(12, 4, "x---------------", 0.75, None);
    snare_roll(&mut a, 60.0, 4.0, &kit, 0.15, 0.8);
    drop_in(&mut a, 64.0, 8.0, true, 0.9);

    // ---- Drop 1 (17–32)
    a.breaks(16, 16, &brk, &GROOVE, &FILLS[..2], 4, 0.88, 0.9);
    a.mel(16, 16, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(16, 16, THEME, Bus::Piano, piano, 0.4, 12, -0.15);
    a.mel(24, 8, SPARKLE, Bus::Keys, chime, 0.55, 0, 0.35);
    a.arp(16, 16, sparkle_arp, 1.0, 86, Bus::Keys, glass, 0.24, 0.6);
    a.arp(16, 16, "0 1 2 1", 2.0, 59, Bus::Piano, piano, 0.22, 0.9);
    a.pad(16, 16, 74, 4, Bus::Pad, high_pad, 0.65);
    a.pad(24, 8, 66, 3, Bus::Pad, ooh, 0.55);
    a.bass(16, 16, "x-----x---x-----", 1.0, Some((0.22, 4.0, 200.0, 900.0, 1.4)));
    a.crash(96.0, 0.55);
    a.stutter(127.0, 1.0, 0.25, true);

    // ---- Breakdown (33–40): Klavier, Harfe, Chor – und Stille
    a.down(128.0, 8.0, 0.5);
    a.air(128.0, 32.0, 0.2, 3000.0, 1.4, 0.0);
    a.mel(32, 8, THEME_B, Bus::Piano, ped, 0.8, 0, 0.1);
    a.arp(32, 8, "0 1 2 3 4 3", 1.0, 71, Bus::Keys, harp, 0.32, 1.0);
    a.pad(32, 8, 66, 4, Bus::Pad, ooh, 0.55);
    a.pad(32, 8, 76, 3, Bus::Pad, high_pad, 0.5);
    a.bass(36, 4, "x---------------", 0.6, None);

    // ---- Halftime (41–48): Eisglas über schwerem Halftime-Beat
    a.rev_piano(160.0, 4.0, &[67, 71, 74, 78], 0.7);
    a.impact(160.0, 0.7);
    a.breaks(40, 8, &brk, &HALF, &[HALF_FILL], 4, 0.9, 1.0);
    a.mel_x(40, 8, THEME, 2.0, Bus::Lead, lead, 0.5, 0, 0.0);
    a.arp(40, 8, "0 2 4 2", 2.0, 86, Bus::Keys, chime, 0.3, 1.0);
    a.comp(40, 8, "X.......x...x...", 66, 4, Bus::Piano, piano, 0.5);
    a.pad(40, 8, 74, 4, Bus::Pad, high_pad, 0.6);
    a.bass(40, 8, "x-------x-----x-", 1.0, Some((0.35, 1.0, 160.0, 1100.0, 1.8)));
    a.stutter(191.0, 1.0, 0.5, false);

    // ---- Aufbau 2 (49–52)
    a.breaks(48, 4, &brk, &GROOVE, &[], 0, 0.85, 0.6);
    a.lp(Bus::Drums, 192.0, 207.0, 1200.0, 14000.0, true);
    a.arp(48, 4, "0 1 2 3 4 5 6 7", 1.0, 74, Bus::Keys, glass, 0.35, 0.6);
    a.pad(48, 4, 66, 3, Bus::Pad, ooh, 0.6);
    a.bass(48, 4, "x-------x-------", 0.8, None);
    snare_roll(&mut a, 200.0, 8.0, &kit, 0.15, 0.85);
    drop_in(&mut a, 208.0, 16.0, true, 1.0);

    // ---- Drop 2 (53–68): Thema B, Glocken antworten
    a.breaks(52, 16, &brk, &CHOPPY, &FILLS[1..], 4, 0.88, 1.0);
    a.mel(52, 8, THEME_B, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(52, 8, THEME_B, Bus::Lead, lead, 0.4, 12, 0.0);
    a.mel(52, 8, SPARKLE, Bus::Keys, chime, 0.55, 0, 0.35);
    a.mel(60, 8, THEME, Bus::Piano, piano, 1.0, 0, 0.1);
    a.mel(60, 8, THEME, Bus::Piano, piano, 0.42, 12, -0.15);
    a.mel(60, 8, THEME, Bus::Lead, lead, 0.45, 12, 0.0);
    a.arp(52, 16, sparkle_arp, 1.0, 86, Bus::Keys, glass, 0.24, 0.6);
    a.pad(52, 16, 74, 4, Bus::Pad, high_pad, 0.65);
    a.pad(52, 16, 66, 3, Bus::Pad, ooh, 0.5);
    a.bass(52, 16, "x-----x---x---o-", 1.0, Some((0.25, 2.0, 200.0, 1100.0, 1.6)));
    a.crash(240.0, 0.6);
    a.stutter(255.0, 1.0, 0.25, true);
    a.stutter(271.0, 1.0, 0.125, true);

    // ---- Outro (69–76): Glocken und Wind wie am Anfang
    a.down(272.0, 8.0, 0.4);
    a.air(272.0, 32.0, 0.22, 3200.0, 1.4, 0.0);
    a.arp(68, 8, sparkle_arp, 1.0, 86, Bus::Keys, glass, 0.3, 0.6);
    a.mel(68, 4, THEME, Bus::Piano, ped, 0.6, 0, 0.1);
    a.pad(68, 8, 74, 4, Bus::Pad, high_pad, 0.6);
    a.breaks(68, 2, &brk, &HALF, &[], 0, 0.55, 0.4);
    a.bass(68, 2, "x---------------", 0.6, None);

    a.sc.mix.hall = (0.9, 0.25, 1.45, 0.03, 1.15);
    a.sc.mix.delay = (0.75, 0.42, 0.55);
    a.sc.mix.bus[Bus::Keys as usize].delay = 0.4;
    a.sc.mix.bus[Bus::Piano as usize].delay = 0.12;
    a.sc.mix.master = 0.82;
    a.finish()
}
