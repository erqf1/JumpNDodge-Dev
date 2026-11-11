//! Client-Seite einer Online-Runde - ohne Grafik, damit sie testbar ist.
//!
//! - Der **eigene Spieler** wird lokal vorhergesagt (sofortige Reaktion auf Tasten): jeder
//!   Logikschritt bewegt ihn mit derselben Funktion wie der Server (`sim2p::move_player`).
//!   Kommt ein Snapshot, wird auf den Serverstand zurückgesetzt und alle Eingaben, die der
//!   Server noch nicht kannte, werden erneut angewendet. Kleine Abweichungen werden weich
//!   ausgeglichen statt zu springen.
//! - Der **andere Spieler** wird zwischen zwei Snapshots interpoliert (leicht verzögert, dafür flüssig).
//! - **Hindernisse/1-UPs** bewegen sich gleichmäßig und werden ab dem letzten Snapshot
//!   weitergerechnet.

use super::proto::*;
use crate::rect::Rect;
use crate::sim2p::{move_player, Input, ObstacleKind, Player, SimEvent, PLAYER_SIZE, START_X};
use std::collections::VecDeque;

/// Wie viele Frames hinter dem Server der andere Spieler gezeichnet wird
const INTERP_DELAY_FRAMES: f64 = 2.5;
/// Sprungtaste wird mindestens so viele Frames gesendet (ein einzelnes verlorenes Paket
/// darf keinen Sprung verschlucken)
const JUMP_HOLD_FRAMES: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawPlayer { pub rect: Rect, pub alive: bool, pub lives: i64, pub invuln: i32 }

pub struct NetRound {
    pub mode: Mode,
    pub difficulty: String,
    /// Eigener Platz (0 = P1, 1 = P2)
    pub you: usize,
    pub field_w: i32,
    pub field_h: i32,
    pub floor_y: i32,
    history: VecDeque<Snapshot>,
    /// Geschätzte aktuelle Server-Frame (läuft mit 60/s weiter, wird an Snapshots angeglichen)
    pub server_frame: f64,
    /// Zähler der eigenen Eingaben
    pub local_frame: u64,
    inputs: VecDeque<(u64, Input)>,
    pub pred: Player,
    pred_prev: Rect,
    /// Weicher Ausgleich nach einer Korrektur (Pixel)
    corr: (f64, f64),
    jump_hold: u32,
    jump_was: bool,
    last_event_frame: u64,
    pub score: i64,
    pub speed: f64,
    /// Anzahl Korrekturen > 2 px (für Tests/Statistik)
    pub corrections: u32,
}

impl NetRound {
    pub fn new(mode: Mode, difficulty: &str, you: usize, field_w: i32, field_h: i32) -> NetRound {
        let floor_y = field_h - 50;
        let you = you.min(1);
        let rect = Rect::new(START_X[you], floor_y - PLAYER_SIZE, PLAYER_SIZE, PLAYER_SIZE);
        NetRound {
            mode, difficulty: difficulty.into(), you, field_w, field_h, floor_y,
            history: VecDeque::new(), server_frame: 0.0, local_frame: 0, inputs: VecDeque::new(),
            pred: Player { rect, vy: 0, jumping: false, up_was: false, alive: true, lives: crate::sim2p::START_LIVES, invuln: 0 },
            pred_prev: rect, corr: (0.0, 0.0), jump_hold: 0, jump_was: false, last_event_frame: 0,
            score: 0, speed: crate::sim2p::difficulty_speed(difficulty) as f64, corrections: 0,
        }
    }

    pub fn latest(&self) -> Option<&Snapshot> { self.history.back() }

    /// Zeit vergeht (ms) - für die Schätzung der Server-Frame
    pub fn advance(&mut self, ms: f64) {
        if !self.history.is_empty() { self.server_frame += ms * TICK_HZ as f64 / 1000.0; }
    }

    /// Ein Logikschritt (60/s) mit der aktuellen Tastatur-Eingabe.
    /// Gibt (Frame, Eingabe) zurück, die an den Server geschickt werden.
    pub fn tick(&mut self, raw: Input) -> (u64, Input) {
        if raw.jump && !self.jump_was { self.jump_hold = JUMP_HOLD_FRAMES; }
        self.jump_was = raw.jump;
        let mut inp = raw;
        if self.jump_hold > 0 { inp.jump = true; self.jump_hold -= 1; }
        self.local_frame += 1;
        self.inputs.push_back((self.local_frame, inp));
        while self.inputs.len() > 240 { self.inputs.pop_front(); }
        self.pred_prev = self.pred.rect;
        move_player(&mut self.pred, inp, self.field_w, self.floor_y);
        if self.pred.invuln > 0 { self.pred.invuln -= 1; }
        self.corr.0 *= 0.8;
        self.corr.1 *= 0.8;
        if self.corr.0.abs() < 0.3 { self.corr.0 = 0.0; }
        if self.corr.1.abs() < 0.3 { self.corr.1 = 0.0; }
        (self.local_frame, inp)
    }

    /// Snapshot vom Server verarbeiten. Gibt neue Ereignisse (Treffer, 1-UP ...) zurück.
    pub fn on_snapshot(&mut self, s: Snapshot) -> Vec<SimEvent> {
        if let Some(l) = self.history.back() { if s.frame <= l.frame { return vec![]; } }
        let mut ev = vec![];
        let mut maxf = self.last_event_frame;
        for (f, e) in &s.events {
            if *f > self.last_event_frame { ev.push(*e); maxf = maxf.max(*f); }
        }
        self.last_event_frame = maxf;
        self.score = s.score;
        self.speed = s.speed;
        // Server-Uhr angleichen
        let f = s.frame as f64;
        if self.history.is_empty() || (self.server_frame - f).abs() > 20.0 { self.server_frame = f; }
        else { self.server_frame += (f - self.server_frame) * 0.15; }
        if self.server_frame < f { self.server_frame = f; }
        self.reconcile(&s);
        self.history.push_back(s);
        while self.history.len() > 10 { self.history.pop_front(); }
        ev
    }

    fn reconcile(&mut self, s: &Snapshot) {
        let ps = &s.players[self.you];
        let old = (self.pred.rect.x as f64 + self.corr.0, self.pred.rect.y as f64 + self.corr.1);
        let was_alive = self.pred.alive;
        let ack = s.input_ack;
        while self.inputs.front().map(|i| i.0 < ack).unwrap_or(false) { self.inputs.pop_front(); }
        let up_was = self.inputs.front().filter(|i| i.0 == ack).map(|i| i.1.jump).unwrap_or(false);
        let mut p = Player {
            rect: Rect::new(ps.x, ps.y, PLAYER_SIZE, PLAYER_SIZE), vy: ps.vy, jumping: ps.jumping,
            up_was: up_was && ps.jumping, alive: ps.alive, lives: ps.lives, invuln: ps.invuln,
        };
        if p.alive {
            for (fr, inp) in self.inputs.iter() {
                if *fr <= ack { continue; }
                move_player(&mut p, *inp, self.field_w, self.floor_y);
                if p.invuln > 0 { p.invuln -= 1; }
            }
        }
        let dx = old.0 - p.rect.x as f64;
        let dy = old.1 - p.rect.y as f64;
        if dx.abs() > 2.0 || dy.abs() > 2.0 { self.corrections += 1; }
        // große Sprünge (Wiederbelebung, lange Funkstille) nicht glätten
        self.corr = if !was_alive || !p.alive || dx.abs() > 150.0 || dy.abs() > 150.0 { (0.0, 0.0) } else { (dx, dy) };
        if !was_alive && p.alive { self.pred_prev = p.rect; }
        self.pred_prev.x = self.pred_prev.x.clamp(p.rect.x - 60, p.rect.x + 60);
        self.pred_prev.y = self.pred_prev.y.clamp(p.rect.y - 60, p.rect.y + 60);
        self.pred = p;
    }

    /// Eigener Spieler zum Zeichnen (alpha = Anteil bis zum nächsten Logikschritt)
    pub fn own_draw(&self, alpha: f64) -> DrawPlayer {
        let a = alpha.clamp(0.0, 1.0);
        let x = self.pred_prev.x as f64 + (self.pred.rect.x - self.pred_prev.x) as f64 * a + self.corr.0;
        let y = self.pred_prev.y as f64 + (self.pred.rect.y - self.pred_prev.y) as f64 * a + self.corr.1;
        DrawPlayer { rect: Rect::new(x.round() as i32, y.round() as i32, PLAYER_SIZE, PLAYER_SIZE), alive: self.pred.alive, lives: self.pred.lives, invuln: self.pred.invuln }
    }

    /// Anderer Spieler zum Zeichnen (zwischen zwei Snapshots interpoliert)
    pub fn other_draw(&self) -> Option<DrawPlayer> {
        let o = 1 - self.you;
        let latest = self.history.back()?;
        let rf = self.server_frame - INTERP_DELAY_FRAMES;
        let mk = |p: &PlayerSnap, x: f64, y: f64| DrawPlayer { rect: Rect::new(x.round() as i32, y.round() as i32, PLAYER_SIZE, PLAYER_SIZE), alive: p.alive, lives: p.lives, invuln: p.invuln };
        let snaps: Vec<&Snapshot> = self.history.iter().collect();
        for w in snaps.windows(2) {
            let (a, b) = (w[0], w[1]);
            if (a.frame as f64) <= rf && rf <= b.frame as f64 {
                let (pa, pb) = (&a.players[o], &b.players[o]);
                let t = (rf - a.frame as f64) / (b.frame - a.frame).max(1) as f64;
                // Wiederbelebung/Teleport nicht verschmieren
                if !pa.alive || (pa.x - pb.x).abs() > 100 || (pa.y - pb.y).abs() > 150 { return Some(mk(pb, pb.x as f64, pb.y as f64)); }
                return Some(mk(pb, pa.x as f64 + (pb.x - pa.x) as f64 * t, pa.y as f64 + (pb.y - pa.y) as f64 * t));
            }
        }
        let p = &latest.players[o];
        Some(mk(p, p.x as f64, p.y as f64))
    }

    /// Frames seit dem letzten Snapshot (für das Weiterrechnen der Hindernisse)
    fn frames_since_latest(&self) -> f64 {
        match self.history.back() { Some(l) => (self.server_frame - l.frame as f64).clamp(0.0, 15.0), None => 0.0 }
    }

    /// Hindernisse zum Zeichnen (ab dem letzten Snapshot weitergerechnet)
    pub fn obstacles_draw(&self) -> Vec<(Rect, ObstacleKind)> {
        let Some(l) = self.history.back() else { return vec![]; };
        let df = self.frames_since_latest();
        let fall = (self.speed * 0.85).max(2.0);
        l.obstacles.iter().map(|o| {
            let r = match o.kind {
                ObstacleKind::Ground => Rect::new(o.x - (self.speed * df).round() as i32, o.y, o.w, o.h),
                ObstacleKind::Top => Rect::new(o.x, o.y + (fall * df).round() as i32, o.w, o.h),
            };
            (r, o.kind)
        }).collect()
    }

    pub fn powerups_draw(&self) -> Vec<Rect> {
        let Some(l) = self.history.back() else { return vec![]; };
        let df = self.frames_since_latest();
        l.powerups.iter().map(|(x, y)| Rect::new(x - (self.speed * df).round() as i32, *y, 38, 38)).collect()
    }

    /// Spielerstand laut Server (Leben/lebendig) für die Anzeige
    pub fn server_player(&self, i: usize) -> Option<&PlayerSnap> { self.history.back().map(|s| &s.players[i.min(1)]) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim2p::{Config, Sim2p};

    /// Perfektes Netz mit fester Verzögerung: Vorhersage stimmt mit dem Server überein
    fn run(delay_frames: usize, loss_every: u64) -> NetRound {
        let cfg = Config { difficulty: "Leicht".into(), vs: false, obstacle_weights: vec![("normal".into(), 100.0)], seed: 5 };
        let mut sim = Sim2p::new(cfg, FIELD_W, FIELD_H);
        let mut nr = NetRound::new(Mode::Coop, "Leicht", 0, FIELD_W, FIELD_H);
        let mut to_server: VecDeque<(u64, Input)> = VecDeque::new();
        let mut to_client: VecDeque<Snapshot> = VecDeque::new();
        let (mut srv_inp, mut ack) = (Input::default(), 0u64);
        for f in 1..=900u64 {
            let raw = Input { jump: f % 45 == 3, left: (f / 100) % 3 == 1, right: (f / 100) % 3 == 2 };
            let sent = nr.tick(raw);
            if loss_every == 0 || f % loss_every != 0 { to_server.push_back(sent); }
            while to_server.len() > delay_frames { let (fr, i) = to_server.pop_front().unwrap(); if fr > ack { srv_inp = i; ack = fr; } }
            sim.step(16, [srv_inp, Input::default()], FIELD_W, FIELD_H);
            if f % SNAPSHOT_EVERY == 0 { to_client.push_back(Snapshot::from_sim(&sim, f, vec![], ack)); }
            while to_client.len() > delay_frames / 2 { let s = to_client.pop_front().unwrap(); nr.on_snapshot(s); }
            nr.advance(1000.0 / 60.0);
        }
        nr
    }

    #[test]
    fn vorhersage_ohne_verzoegerung_exakt() {
        let nr = run(0, 0);
        assert_eq!(nr.corrections, 0, "ohne Verzögerung darf nie korrigiert werden");
    }

    #[test]
    fn vorhersage_mit_verzoegerung_ohne_korrektur() {
        // 100 ms Verzögerung: die Wiederholung der unbestätigten Eingaben trifft den Serverstand
        let nr = run(6, 0);
        assert!(nr.corrections <= 2, "Korrekturen: {}", nr.corrections);
    }

    #[test]
    fn sprung_trotz_verlorenem_paket() {
        let mut nr = NetRound::new(Mode::Vs, "Leicht", 1, FIELD_W, FIELD_H);
        let (_, a) = nr.tick(Input { jump: true, ..Default::default() });
        let (_, b) = nr.tick(Input::default());
        let (_, c) = nr.tick(Input::default());
        assert!(a.jump && b.jump && c.jump, "Sprung wird mehrere Frames gehalten");
        assert!(nr.pred.rect.bottom() < nr.floor_y);
    }
}
