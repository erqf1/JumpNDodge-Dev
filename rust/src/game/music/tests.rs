//! Rendert alle Songs, prüft Länge/Pegel/Übersteuerung/Schleife und druckt eine Mix-Analyse
//! (RMS je Abschnitt von 8 Takten, Frequenzbänder, Stereo-Breite).
//! `JND_MUSIC_OUT=<ordner>` schreibt WAVs zum Anhören, `JND_MUSIC_ONLY=<schlüssel>` nur einen Song.

use super::dsp::*;
use super::*;

fn db(x: f32) -> f32 { 20.0 * x.max(1e-9).log10() }

/// Bänder: Sub <60, Bass 60–250, Mitte 250–2k, Hoch 2k–8k, Luft >8k (RMS je Band)
fn bands(mono: &[f32]) -> [f32; 5] {
    let cuts = [60.0, 250.0, 2000.0, 8000.0];
    let mut lps: Vec<Svf> = cuts.iter().map(|c| Svf::new(*c, 0.707)).collect();
    let mut lps2: Vec<Svf> = cuts.iter().map(|c| Svf::new(*c, 0.707)).collect();
    let mut acc = [0f64; 5];
    for &x in mono {
        let lo: Vec<f32> = (0..4).map(|k| lps2[k].lp(lps[k].lp(x))).collect();
        let b = [lo[0], lo[1] - lo[0], lo[2] - lo[1], lo[3] - lo[2], x - lo[3]];
        for k in 0..5 { acc[k] += (b[k] * b[k]) as f64; }
    }
    let n = mono.len().max(1) as f64;
    let mut o = [0f32; 5];
    for k in 0..5 { o[k] = (acc[k] / n).sqrt() as f32; }
    o
}

#[test]
fn songs_rendern() {
    let only = std::env::var("JND_MUSIC_ONLY").ok();
    let out = std::env::var("JND_MUSIC_OUT").ok();
    let mut keys: Vec<&str> = vec![MENU];
    keys.extend(songs().iter().map(|s| s.key));
    for key in keys {
        if let Some(o) = &only { if o != key { continue; } }
        let t0 = std::time::Instant::now();
        let (s, st) = render_now_stats(key).unwrap();
        let ms = t0.elapsed().as_millis();
        let (title, bpm) = info(key).map(|i| (i.title, i.bpm)).unwrap_or(("Menü", 174.0));
        let frames = s.len() / 2;
        let secs = frames as f32 / SR as f32;
        let f = |i: usize| s[i] as f32 / 32768.0;
        let rms = (s.iter().map(|v| (*v as f32 / 32768.0).powi(2)).sum::<f32>() / s.len() as f32).sqrt();
        let peak = s.iter().map(|v| v.unsigned_abs()).max().unwrap_or(0) as f32 / 32768.0;
        let clip = s.iter().filter(|v| v.unsigned_abs() >= 32700).count();
        // Stereo: Seite/Mitte
        let (mut mid, mut side) = (0f64, 0f64);
        for i in 0..frames { let (l, r) = (f(i * 2), f(i * 2 + 1)); mid += ((l + r) * 0.5).powi(2) as f64; side += ((l - r) * 0.5).powi(2) as f64; }
        let width = (side / mid.max(1e-12)).sqrt();
        // Naht der Schleife: Sprung zwischen letztem und erstem Sample im Vergleich zu typischen Schritten
        let seam = ((f(0) - f(frames * 2 - 2)).abs()).max((f(1) - f(frames * 2 - 1)).abs());
        let typ = (1..frames).step_by(97).map(|i| (f(i * 2) - f(i * 2 - 2)).abs()).fold(0f32, f32::max);
        println!("\n== {title} ({key}): {:.1} s ({}:{:02}), {bpm} BPM, gerendert in {ms} ms", secs, secs as u32 / 60, secs as u32 % 60);
        println!("   RMS {:.1} dBFS, Spitze {:.1} dBFS, Clipping {clip}, Stereo-Breite (S/M) {:.2}, Naht {:.4} (größter Schritt {:.3})", db(rms), db(peak), width, seam, typ);
        let names = ["Klavier", "Glocken", "Fläche", "Lead", "Bass", "Break", "Perc", "FX"];
        println!("   Busse (dB): {}", names.iter().zip(st.iter()).map(|(n, v)| format!("{n} {:.1}", db(*v))).collect::<Vec<_>>().join(" | "));
        let mono: Vec<f32> = (0..frames).map(|i| (f(i * 2) + f(i * 2 + 1)) * 0.5).collect();
        let b = bands(&mono);
        println!("   Bänder gesamt (dB): Sub {:.1} | Bass {:.1} | Mitte {:.1} | Hoch {:.1} | Luft {:.1}", db(b[0]), db(b[1]), db(b[2]), db(b[3]), db(b[4]));
        let bar = SR as f32 * 60.0 / bpm * 4.0;
        let chunk = (bar * 8.0) as usize;
        for (k, c) in mono.chunks(chunk).enumerate().filter(|(_, c)| c.len() as f32 > bar) {
            let r = (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt();
            let b = bands(c);
            println!("   Takt {:>3}–{:<3} RMS {:>6.1} | Sub {:>6.1} Bass {:>6.1} Mitte {:>6.1} Hoch {:>6.1} Luft {:>6.1}", k * 8 + 1, k * 8 + 8, db(r), db(b[0]), db(b[1]), db(b[2]), db(b[3]), db(b[4]));
        }
        assert!(clip == 0, "{key}: Übersteuerung");
        assert!(peak < 0.95, "{key}: Spitze zu hoch");
        assert!(rms > 0.05, "{key}: zu leise");
        let local = (frames - 200..frames - 1).chain(1..200).map(|i| (f(i * 2) - f(i * 2 - 2)).abs()).fold(0f32, f32::max);
        assert!(seam <= local.max(0.01) * 1.2, "{key}: Schleife springt ({seam} > {local})");
        if key != MENU { assert!((85.0..=155.0).contains(&secs), "{key}: Länge {secs}"); }
        if let Some(d) = &out { let _ = write_wav(&std::path::Path::new(d).join(format!("{key}.wav")), &s); }
    }
}

#[test]
fn notenschrift() {
    let (n, len) = arr::parse_mel("c4:4 eb4:2 g4 | r:4 c5+e5:4!");
    assert_eq!(len, 4.0);
    assert_eq!(n[0].m, 60);
    assert_eq!(n[1].m, 63);
    assert_eq!(n[2].d, 0.5);
    assert_eq!(n.len(), 5);
    assert_eq!(n[3].m, 72);
    let c = arr::parse_chord("Bb/D");
    assert_eq!((c.root, c.bass), (10, 2));
    assert_eq!(arr::parse_chord("F#m7").tones, vec![0, 3, 7, 10]);
}

#[test]
fn speicher_und_cache() {
    // unbekannte Schlüssel werden ignoriert
    request("gibtsnicht");
    assert!(get("gibtsnicht").is_none());
    assert!(songs().len() >= 6);
    for s in songs() { assert!(!s.title.is_empty() && (160.0..=185.0).contains(&s.bpm)); }
}

#[test]
#[ignore]
fn profil() {
    let sc = (info(&std::env::var("JND_MUSIC_ONLY").unwrap_or("jnd_banners".into())).unwrap().build)();
    let mut by: std::collections::BTreeMap<String, (usize, f64)> = Default::default();
    for e in &sc.evs {
        let name = format!("{:?}", e.inst).split([' ', '{']).next().unwrap().to_string();
        let t = std::time::Instant::now();
        let mut v = inst::make_voice(&e.inst, e.pan, e.seed, &inst::Ctx { samples: &sc.samples });
        let (mut l, mut r) = (vec![0f32; 256], vec![0f32; 256]);
        let mut n = 0;
        while v.render(&mut l, &mut r) && n < 2000 { n += 1; }
        let x = by.entry(name).or_default();
        x.0 += 1; x.1 += t.elapsed().as_secs_f64();
    }
    for (k, (n, s)) in by { println!("{k:10} {n:6} Ereignisse {:7.0} ms", s * 1000.0); }
    let mut leer = (info("jnd_banners").unwrap().build)();
    leer.evs.clear();
    let t = std::time::Instant::now();
    let _ = engine::render(&leer, 1);
    println!("Engine ohne Stimmen: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    let t = std::time::Instant::now();
    let _ = engine::render(&sc, 1);
    println!("gesamt 1 Thread: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
}
