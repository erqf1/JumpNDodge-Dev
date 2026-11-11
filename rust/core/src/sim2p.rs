//! 2-Spieler-Simulation (Koop/VS) - die Regeln aus run_game_2p() (Python 15739-16153),
//! ohne Zeichnen. Ein Aufruf von `step()` = ein Frame.

use crate::rect::{addf, Rect};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};

pub const START_LIVES: i64 = 3;
pub const HIT_INVULN: i32 = 90;
pub const REVIVE_INVULN: i32 = 45;
pub const GRAVITY: i32 = 1;
pub const JUMP_VELOCITY: i32 = -20;
pub const MOVE_SPEED: i32 = 8;
pub const POWERUP_CHANCE: f64 = 0.015;
/// Startpositionen (x) von P1 und P2
pub const START_X: [i32; 2] = [60, 140];

/// Schwierigkeiten, bei denen zusätzlich Hindernisse von oben fallen
const TOP_DIFFICULTIES: [&str; 4] = ["Hardcore", "Unmöglich", "Albtraum", "Hölle"];

pub fn difficulty_speed(d: &str) -> i64 {
    match d { "Leicht" => 6, "Mittel" => 8, "Schwer" => 11, "Hardcore" => 15, "Unmöglich" => 18, "Albtraum" => 24, "Hölle" => 32, "Dynamisch" => 7, _ => 8 }
}
pub fn difficulty_obstacle_interval(d: &str) -> (i64, i64) {
    match d {
        "Leicht" => (1400, 2200), "Mittel" => (1000, 1800), "Schwer" => (600, 1300), "Hardcore" => (400, 1000),
        "Unmöglich" => (500, 1000), "Albtraum" => (260, 500), "Hölle" => (120, 300), "Dynamisch" => (1400, 2400),
        _ => (1000, 1800),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input { pub jump: bool, pub left: bool, pub right: bool }

#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub rect: Rect,
    pub vy: i32,
    pub jumping: bool,
    /// Sprungtaste war im letzten Frame schon gedrückt (kein Dauerspringen)
    pub up_was: bool,
    pub alive: bool,
    pub lives: i64,
    /// Unverwundbarkeit in Frames
    pub invuln: i32,
}

impl Player {
    fn new(x: i32, floor_y: i32) -> Player {
        Player { rect: Rect::new(x, floor_y - PLAYER_SIZE, PLAYER_SIZE, PLAYER_SIZE), vy: 0, jumping: false, up_was: false, alive: true, lives: START_LIVES, invuln: 0 }
    }
    fn revive(&mut self, x: i32, floor_y: i32) {
        self.alive = true;
        self.lives = 1;
        self.rect.x = x;
        self.rect.set_bottom(floor_y);
        self.vy = 0;
        self.jumping = false;
        self.up_was = false;
        self.invuln = REVIVE_INVULN;
    }
}

/// Bewegung eines Spielers für einen Frame (auch für die Vorhersage beim Online-Client).
/// Gibt true zurück, wenn der Spieler in diesem Frame abgesprungen ist.
pub fn move_player(p: &mut Player, inp: Input, width: i32, floor_y: i32) -> bool {
    if !p.alive { return false; }
    let mut jumped = false;
    if inp.jump && !p.up_was && !p.jumping { p.vy = JUMP_VELOCITY; p.jumping = true; jumped = true; }
    p.up_was = inp.jump;
    if inp.left { p.rect.x -= MOVE_SPEED; }
    if inp.right { p.rect.x += MOVE_SPEED; }
    p.rect.set_left(0.max(p.rect.left()));
    p.rect.set_right(width.min(p.rect.right()));
    p.vy += GRAVITY;
    p.rect.y += p.vy;
    if p.rect.bottom() >= floor_y {
        p.rect.set_bottom(floor_y);
        p.vy = 0;
        if p.jumping { p.jumping = false; p.up_was = false; }
    }
    jumped
}

/// Seitenlänge der Spieler im 2P-/Online-Modus
pub const PLAYER_SIZE: i32 = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObstacleKind { Ground, Top }

#[derive(Clone, Debug, PartialEq)]
pub struct Obstacle { pub rect: Rect, pub kind: ObstacleKind }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Winner { P1, P2, Draw }
impl Winner {
    pub fn as_str(self) -> &'static str { match self { Winner::P1 => "p1", Winner::P2 => "p2", Winner::Draw => "draw" } }
}

/// Was in einem Frame passiert ist (für Overlays/Sounds/Statistik beim Client)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimEvent {
    /// Spieler i hat den toten Partner wiederbelebt (Index = wiederbelebter Spieler)
    Revived(usize),
    /// Spieler i hat ein 1-UP eingesammelt (+1 Leben)
    OneUp(usize),
    /// Spieler i wurde getroffen und lebt noch
    Hit(usize),
    /// Spieler i wurde getroffen und ist ausgeschieden
    Died(usize),
}

#[derive(Clone, Debug, Default)]
pub struct StepResult {
    pub events: Vec<SimEvent>,
    /// Anzahl neuer Sprünge in diesem Frame (Statistik total_jumps)
    pub jumps: i64,
    pub game_over: bool,
    pub winner: Option<Winner>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub difficulty: String,
    pub vs: bool,
    /// Gewichte der Hindernistypen (difficulty_obstacle_types[difficulty]); leer = nur "normal"
    pub obstacle_weights: Vec<(String, f64)>,
    pub seed: u64,
}

pub struct Sim2p {
    pub cfg: Config,
    pub floor_y: i32,
    pub players: [Player; 2],
    pub obstacles: Vec<Obstacle>,
    pub top_obstacles: Vec<Obstacle>,
    pub powerups: Vec<Rect>,
    pub score: i64,
    pub speed: f64,
    pub min_delay: i64,
    pub max_delay: i64,
    pub obs_timer: i64,
    pub obs_delay: i64,
    pub top_timer: i64,
    pub top_delay: i64,
    pub dynamic_timer: i64,
    pub frame: u64,
    rng: StdRng,
}

impl Sim2p {
    /// Neue Runde auf einem Feld der Größe (width, height)
    pub fn new(cfg: Config, width: i32, height: i32) -> Sim2p {
        let floor_y = height - 50;
        let (min_delay, max_delay) = difficulty_obstacle_interval(&cfg.difficulty);
        let speed = difficulty_speed(&cfg.difficulty) as f64;
        let rng = StdRng::seed_from_u64(cfg.seed);
        let mut s = Sim2p {
            floor_y,
            players: [Player::new(START_X[0], floor_y), Player::new(START_X[1], floor_y)],
            obstacles: vec![], top_obstacles: vec![], powerups: vec![],
            score: 0, speed, min_delay, max_delay,
            obs_timer: 0, obs_delay: 0, top_timer: 0, top_delay: 0, dynamic_timer: 0, frame: 0,
            rng, cfg,
        };
        s.obs_delay = s.randint(min_delay, max_delay);
        s.top_delay = s.new_top_delay(width);
        s
    }

    fn randint(&mut self, a: i64, b: i64) -> i64 { if b < a { a } else { self.rng.gen_range(a..=b) } }
    fn random(&mut self) -> f64 { self.rng.gen::<f64>() }
    fn new_top_delay(&mut self, width: i32) -> i64 {
        (self.randint(900, 1600) as f64 * (800.0 / 400.max(width) as f64)) as i64
    }

    pub fn dynamic_mode(&self) -> bool { self.cfg.difficulty == "Dynamisch" }
    pub fn top_active(&self) -> bool {
        TOP_DIFFICULTIES.contains(&self.cfg.difficulty.as_str()) || (self.dynamic_mode() && self.speed >= 14.0)
    }

    /// pick_obstacle_type (2P nie im Klassik-Modus)
    fn pick_obstacle_type(&mut self) -> String {
        let weights: Vec<(String, f64)> = if self.dynamic_mode() || self.cfg.obstacle_weights.is_empty() {
            vec![("normal".into(), 100.0)]
        } else { self.cfg.obstacle_weights.clone() };
        let total: f64 = weights.iter().map(|w| w.1).sum();
        let mut r = self.random() * total;
        for (t, w) in &weights {
            if r < *w { return t.clone(); }
            r -= w;
        }
        weights.last().map(|w| w.0.clone()).unwrap_or_else(|| "normal".into())
    }

    /// make_obstacle(spawn_x, type)
    fn make_obstacle(&mut self, spawn_x: i32, obs_type: &str, height: i32) -> Vec<Obstacle> {
        let g = |x, s| Obstacle { rect: Rect::new(x, height - 50 - s, s, s), kind: ObstacleKind::Ground };
        if obs_type == "double" {
            let gap = self.randint(80, 120) as i32;
            vec![g(spawn_x, 50), g(spawn_x + 50 + gap, 50)]
        } else {
            vec![g(spawn_x, 50)]
        }
    }

    fn make_top_obstacle(&mut self, width: i32) -> Obstacle {
        let rx = self.randint(60, (width - 110) as i64) as i32;
        Obstacle { rect: Rect::new(rx, -55, 50, 50), kind: ObstacleKind::Top }
    }

    /// Spieler i scheidet sofort aus (z.B. Verbindung im Online-Koop verloren)
    pub fn eliminate(&mut self, i: usize) {
        let p = &mut self.players[i];
        p.alive = false;
        p.lives = 0;
    }

    /// Ein Frame. `dt_ms` = vergangene Zeit, `width`/`height` = aktuelle Spielfeldgröße.
    pub fn step(&mut self, dt_ms: i64, inputs: [Input; 2], width: i32, height: i32) -> StepResult {
        let mut out = StepResult::default();
        self.frame += 1;
        let floor_y = self.floor_y;

        // Bewegung
        for (p, inp) in self.players.iter_mut().zip(inputs.iter()) {
            if move_player(p, *inp, width, floor_y) { out.jumps += 1; }
        }
        for p in self.players.iter_mut() { if p.invuln > 0 { p.invuln -= 1; } }

        // Spawnen
        self.obs_timer += dt_ms;
        if self.obs_timer >= self.obs_delay {
            self.obs_timer = 0;
            self.obs_delay = self.randint(self.min_delay, self.max_delay);
            if self.random() < POWERUP_CHANCE {
                self.powerups.push(Rect::new(width + 50, floor_y - 50, 38, 38));
            } else {
                let t = self.pick_obstacle_type();
                let o = self.make_obstacle(width + 50, &t, height);
                self.obstacles.extend(o);
            }
        }
        if self.top_active() {
            self.top_timer += dt_ms;
            if self.top_timer >= self.top_delay {
                self.top_timer = 0;
                let o = self.make_top_obstacle(width);
                self.top_obstacles.push(o);
                self.top_delay = self.new_top_delay(width);
            }
        }

        // Hindernisse bewegen
        let cs = self.speed;
        let fall = (cs * 0.85).max(2.0);
        let before = self.obstacles.len();
        for o in self.obstacles.iter_mut() { o.rect.x = addf(o.rect.x, -cs); }
        self.obstacles.retain(|o| o.rect.right() >= 0);
        self.score += (before - self.obstacles.len()) as i64;
        for o in self.top_obstacles.iter_mut() { o.rect.y = addf(o.rect.y, fall); }
        self.top_obstacles.retain(|o| o.rect.top() <= height);
        for p in self.powerups.iter_mut() { p.x = addf(p.x, -cs); }
        self.powerups.retain(|p| p.right() >= 0);

        // 1-UP einsammeln (höchstens eins pro Frame)
        for i in 0..self.powerups.len() {
            let pu = self.powerups[i];
            let picker = (0..2).find(|&k| self.players[k].alive && self.players[k].rect.colliderect(&pu));
            if let Some(k) = picker {
                let other = 1 - k;
                if !self.players[other].alive {
                    self.players[other].revive(START_X[other], floor_y);
                    out.events.push(SimEvent::Revived(other));
                } else {
                    self.players[k].lives += 1;
                    out.events.push(SimEvent::OneUp(k));
                }
                self.powerups.remove(i);
                break;
            }
        }

        // Treffer: pro Spieler höchstens ein Boden- und ein Top-Hindernis
        let mut died = [false, false];
        for pl in 0..2 {
            for top in [false, true] {
                let p = &self.players[pl];
                let (alive, inv, pr) = (p.alive, p.invuln, p.rect);
                let list = if top { &mut self.top_obstacles } else { &mut self.obstacles };
                if let Some(idx) = list.iter().position(|o| alive && inv == 0 && pr.colliderect(&o.rect)) {
                    list.remove(idx);
                    let p = &mut self.players[pl];
                    p.lives -= 1;
                    if p.lives <= 0 {
                        p.alive = false;
                        died[pl] = true;
                        out.events.push(SimEvent::Died(pl));
                    } else {
                        p.invuln = HIT_INVULN;
                        out.events.push(SimEvent::Hit(pl));
                    }
                }
            }
        }

        // Dynamisch: alle 5 s schneller
        if self.dynamic_mode() {
            self.dynamic_timer += dt_ms;
            if self.dynamic_timer >= 5000 {
                self.dynamic_timer = 0;
                self.speed += 0.5;
                self.min_delay = 20.max(self.min_delay - 50);
                self.max_delay = (self.min_delay + 20).max(self.max_delay - 100);
            }
        }

        // Rundenende
        if self.cfg.vs {
            out.winner = match (died[0], died[1]) {
                (true, true) => Some(Winner::Draw),
                (true, false) => Some(Winner::P2),
                (false, true) => Some(Winner::P1),
                _ => None,
            };
            out.game_over = out.winner.is_some();
        } else {
            out.game_over = !self.players[0].alive && !self.players[1].alive;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(d: &str, vs: bool, seed: u64) -> Config {
        Config { difficulty: d.into(), vs, obstacle_weights: vec![("normal".into(), 100.0)], seed }
    }

    /// Gleicher Seed + gleiche Eingaben => exakt gleicher Verlauf (Grundlage für den Server)
    #[test]
    fn deterministisch() {
        let run = || {
            let mut s = Sim2p::new(cfg("Hölle", false, 42), 800, 600);
            let mut log = vec![];
            for f in 0..3000u64 {
                let a = Input { jump: f % 37 < 3, left: f % 200 < 50, right: f % 200 > 120 };
                let b = Input { jump: f % 23 < 2, left: false, right: f % 90 < 30 };
                let r = s.step(16, [a, b], 800, 600);
                log.push((s.score, s.players[0].rect, s.players[1].rect, s.obstacles.len(), r.events.clone()));
                if r.game_over { break; }
            }
            log
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn springen_und_landen() {
        let mut s = Sim2p::new(cfg("Leicht", false, 1), 800, 600);
        let r = s.step(16, [Input { jump: true, ..Default::default() }, Input::default()], 800, 600);
        assert_eq!(r.jumps, 1);
        assert!(s.players[0].rect.bottom() < s.floor_y);
        // Taste weiter gedrückt: in der Luft kein zweiter Sprung
        let r2 = s.step(16, [Input { jump: true, ..Default::default() }, Input::default()], 800, 600);
        assert_eq!(r2.jumps, 0);
        // loslassen -> landet wieder auf dem Boden
        for _ in 0..60 { s.step(16, [Input::default(); 2], 800, 600); }
        assert_eq!(s.players[0].rect.bottom(), s.floor_y);
        assert!(!s.players[0].jumping);
    }

    #[test]
    fn vs_runde_endet_mit_sieger() {
        let mut s = Sim2p::new(cfg("Hölle", true, 7), 800, 600);
        // P1 springt dauernd im Takt, P2 steht still -> irgendwann scheidet jemand aus
        for f in 0..20000u64 {
            let r = s.step(16, [Input { jump: f % 40 == 0, ..Default::default() }, Input::default()], 800, 600);
            if r.game_over { assert!(r.winner.is_some()); return; }
        }
        panic!("Runde ist nie zu Ende gegangen");
    }

    #[test]
    fn koop_wiederbeleben() {
        let mut s = Sim2p::new(cfg("Leicht", false, 3), 800, 600);
        s.players[1].alive = false;
        s.players[1].lives = 0;
        let p1 = s.players[0].rect;
        s.powerups.push(Rect::new(p1.x + 4, p1.y, 38, 38));
        let r = s.step(16, [Input::default(); 2], 800, 600);
        assert!(r.events.contains(&SimEvent::Revived(1)));
        assert!(s.players[1].alive);
        assert_eq!(s.players[1].lives, 1);
        assert_eq!(s.players[1].invuln, REVIVE_INVULN);
    }
}
