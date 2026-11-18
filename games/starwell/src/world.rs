//! The game: the canyon layout, flight, gates and crystals, scoring, modes,
//! and what each frame shows (instances, lights, particles, HUD).
//!
//! Coordinates are relative to the glider's forward position: the glider
//! sits at z = 0 and flies toward -z; something `d` units further down the
//! canyon than the glider is at z = -(d - dist). x is across the canyon,
//! y is up.

use core::f32::consts::PI;

use crate::font::{rgba, Hud};
use crate::gfx::{FrameData, Inst, SceneU, SimU, CANYON, CRYSTAL, GLIDER, RING};
use crate::math::*;

pub const SEG_LEN: f32 = 8.0;
pub const WALL_X: f32 = 11.0;
pub const FLOOR_Y: f32 = -5.0;
pub const FLY_X: f32 = 6.5;
pub const FLY_Y: f32 = 3.3;
pub const SECTOR_LEN: f32 = 900.0;
const ROW_GAP: f32 = 16.0;
const VIEW_AHEAD: f32 = 240.0;
const GATE_R: f32 = 1.6;
const GATE_HIT_R: f32 = 1.55;
const GLIDER_HALF_W: f32 = 0.5;
const GLIDER_HALF_H: f32 = 0.22;
const ENERGY_START: f32 = 30.0;
const ENERGY_MAX: f32 = 40.0;
const ENERGY_GATE: f32 = 3.5;
const ENERGY_HIT: f32 = 6.0;
const LATERAL: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Title = 0,
    Play = 1,
    Paused = 2,
    Over = 3,
}

struct Pillar {
    d: f32,
    x: f32,
    r: f32,
    rot: f32,
    broken: bool,
}
struct Beam {
    d: f32,
    y: f32,
    broken: bool,
}
#[derive(Clone, Copy, PartialEq)]
enum GateState {
    Ahead,
    Passed,
    Missed,
}
struct Gate {
    d: f32,
    x: f32,
    y: f32,
    state: GateState,
    /// Seconds since the state changed.
    t: f32,
}
struct Popup {
    text: String,
    age: f32,
    color: [f32; 3],
}

/// A sector's look, all colours linear.
#[derive(Clone, Copy)]
struct Palette {
    fog: V3,
    sky: V3,
    crystal: V3,
    accent: V3,
    rock: V3,
    sun: V3,
    mote: V3,
    gate: V3,
}

const PALETTES: [Palette; 4] = [
    Palette {
        fog: [0.016, 0.05, 0.08],
        sky: [0.002, 0.008, 0.025],
        crystal: [0.2, 0.75, 1.0],
        accent: [0.06, 0.45, 0.8],
        rock: [0.09, 0.11, 0.15],
        sun: [0.45, 0.55, 0.75],
        mote: [0.55, 0.9, 1.0],
        gate: [1.0, 0.72, 0.22],
    },
    Palette {
        fog: [0.06, 0.018, 0.08],
        sky: [0.015, 0.0, 0.025],
        crystal: [0.85, 0.3, 1.0],
        accent: [0.6, 0.12, 0.85],
        rock: [0.12, 0.09, 0.14],
        sun: [0.65, 0.5, 0.75],
        mote: [1.0, 0.6, 1.0],
        gate: [1.0, 0.8, 0.3],
    },
    Palette {
        fog: [0.09, 0.035, 0.015],
        sky: [0.025, 0.006, 0.0],
        crystal: [1.0, 0.42, 0.12],
        accent: [0.9, 0.3, 0.05],
        rock: [0.14, 0.1, 0.08],
        sun: [0.8, 0.55, 0.4],
        mote: [1.0, 0.75, 0.45],
        gate: [0.85, 0.95, 1.0],
    },
    Palette {
        fog: [0.015, 0.07, 0.045],
        sky: [0.0, 0.02, 0.01],
        crystal: [0.3, 1.0, 0.5],
        accent: [0.1, 0.75, 0.35],
        rock: [0.08, 0.12, 0.1],
        sun: [0.5, 0.75, 0.55],
        mote: [0.6, 1.0, 0.7],
        gate: [1.0, 0.75, 0.25],
    },
];

fn mix_pal(a: &Palette, b: &Palette, t: f32) -> Palette {
    Palette {
        fog: lerp3(a.fog, b.fog, t),
        sky: lerp3(a.sky, b.sky, t),
        crystal: lerp3(a.crystal, b.crystal, t),
        accent: lerp3(a.accent, b.accent, t),
        rock: lerp3(a.rock, b.rock, t),
        sun: lerp3(a.sun, b.sun, t),
        mote: lerp3(a.mote, b.mote, t),
        gate: lerp3(a.gate, b.gate, t),
    }
}

/// The canyon wall's distance from the centre line at height y (matches
/// the cross-section in meshes::canyon).
fn wall_x(y: f32) -> f32 {
    if y < -2.0 {
        WALL_X - 1.6 + 1.6 * ((y - FLOOR_Y - 0.4) / 2.6).clamp(0.0, 1.0)
    } else if y < 4.05 {
        WALL_X + 0.6 * (y + 2.0) / 6.05
    } else {
        WALL_X + 0.6 + 1.9 * (y - 4.05) / 4.95
    }
}

/// Stateless per-segment hash for the scenery.
fn hash(mut x: u32) -> u32 {
    x = x.wrapping_mul(0x9E37_79B1) ^ 0x85eb_ca6b;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297a_2d39);
    x ^ (x >> 15)
}

/// Sounds the game asks for this frame (indices into the cart's mixer).
#[derive(Clone, Copy, PartialEq)]
pub enum Sfx {
    Gate,
    Hit,
    Start,
    Sector,
    Over,
    Pause,
}

/// The merged controller state of every connected pad (and keyboard).
#[derive(Clone, Copy, Default)]
pub struct Input {
    pub x: f32,
    pub y: f32,
    pub a: bool,
    pub start: bool,
}

pub struct Game {
    pub mode: Mode,
    pub dist: f32,
    pub speed: f32,
    pub px: f32,
    pub py: f32,
    vx: f32,
    vy: f32,
    pub energy: f32,
    pub score: u32,
    pub best: u32,
    pub new_best: bool,
    pub sector: u32,
    pub goal: bool,
    pub combo: u32,
    pub gates: u32,
    pub hits: u32,
    invuln: f32,
    hit_glow: f32,
    hit_target: f32,
    slow: f32,
    time: f32,
    mode_t: f32,
    pal_from: Palette,
    pal_t: f32,
    pal_idx: usize,
    banner: Option<(String, String, f32)>,
    popups: Vec<Popup>,
    rng: Rng,
    gen_d: f32,
    row: u32,
    pillars: Vec<Pillar>,
    beams: Vec<Beam>,
    gate_list: Vec<Gate>,
    bursts: [[f32; 8]; 8],
    burst_seq: u32,
    burst_next: usize,
    cam: V3,
    cam_look: V3,
    prev: Input,
    roll: f32,
    pitch: f32,
    pub sfx: Vec<(Sfx, f32)>,
    pub rumble: Option<(f32, f32, u32)>,
    /// Forward shift applied to the particles this frame.
    shift: f32,
    sim_dt: f32,
    pub frame: u32,
    pub hint_t: f32,
    /// Steer by the autopilot during play (set through the debug field).
    pub autoplay: bool,
}

impl Game {
    pub fn new(best: u32, seed: u32) -> Game {
        let mut g = Game {
            mode: Mode::Title,
            dist: 0.0,
            speed: 18.0,
            px: 0.0,
            py: 0.0,
            vx: 0.0,
            vy: 0.0,
            energy: ENERGY_START,
            score: 0,
            best,
            new_best: false,
            sector: 1,
            goal: false,
            combo: 0,
            gates: 0,
            hits: 0,
            invuln: 0.0,
            hit_glow: 0.0,
            hit_target: 0.0,
            slow: 0.0,
            time: 0.0,
            mode_t: 0.0,
            pal_from: PALETTES[0],
            pal_t: 1.0,
            pal_idx: 0,
            banner: None,
            popups: Vec::new(),
            rng: Rng(seed | 1),
            gen_d: 0.0,
            row: 0,
            pillars: Vec::new(),
            beams: Vec::new(),
            gate_list: Vec::new(),
            bursts: [[0.0; 8]; 8],
            burst_seq: 0,
            burst_next: 0,
            cam: [0.0, 1.6, 6.0],
            cam_look: [0.0, 0.3, -12.0],
            prev: Input::default(),
            roll: 0.0,
            pitch: 0.0,
            sfx: Vec::new(),
            rumble: None,
            shift: 0.0,
            sim_dt: 0.0,
            frame: 0,
            hint_t: 0.0,
            autoplay: false,
        };
        g.reset_world();
        g
    }

    fn reset_world(&mut self) {
        self.dist = 0.0;
        self.gen_d = 40.0;
        self.row = 0;
        self.pillars.clear();
        self.beams.clear();
        self.gate_list.clear();
        self.px = 0.0;
        self.py = 0.0;
        self.vx = 0.0;
        self.vy = 0.0;
        self.sector = 1;
        if self.pal_idx != 0 {
            self.pal_from = self.palette();
            self.pal_idx = 0;
            self.pal_t = 0.0;
        }
    }

    fn start(&mut self) {
        // A new canyon each flight, seeded by how long the menu waited.
        self.rng = Rng(hash(self.frame.wrapping_mul(2654435761) ^ self.rng.0) | 1);
        self.reset_world();
        self.mode = Mode::Play;
        self.mode_t = 0.0;
        self.speed = 14.0;
        self.energy = ENERGY_START;
        self.score = 0;
        self.new_best = false;
        self.goal = false;
        self.combo = 0;
        self.gates = 0;
        self.hits = 0;
        self.invuln = 0.0;
        self.slow = 0.0;
        self.popups.clear();
        self.banner = Some(("SECTOR 1".into(), "REACH SECTOR 2".into(), 0.0));
        self.hint_t = 0.0;
        self.sfx.push((Sfx::Start, 0.8));
    }

    fn palette(&self) -> Palette {
        mix_pal(&self.pal_from, &PALETTES[self.pal_idx], smooth(self.pal_t))
    }

    fn sector_at(d: f32) -> u32 {
        1 + (d / SECTOR_LEN).max(0.0) as u32
    }

    /// Lay out canyon rows until `VIEW_AHEAD` past the glider.
    fn generate(&mut self) {
        let attract = self.mode == Mode::Title;
        while self.gen_d < self.dist + VIEW_AHEAD {
            let d = self.gen_d;
            let s = Self::sector_at(d);
            let r = &mut self.rng;
            let gate_row = self.row % 3 == 0;
            let lead_in = d < 70.0;
            let mut gx = 0.0;
            if gate_row {
                // The first rings sit near the middle to teach the idea.
                let spread = if d < 120.0 { 0.45 } else { 1.0 };
                gx = r.range(-5.0, 5.0) * spread;
                let gy = r.range(-2.2, 2.2) * spread;
                self.gate_list.push(Gate { d, x: gx, y: gy, state: GateState::Ahead, t: 0.0 });
            }
            if !attract && !lead_in {
                let (p1, p2, pb) = match s {
                    1 => (0.55, 0.12, 0.0),
                    2 => (0.65, 0.3, 0.1),
                    3 => (0.72, 0.42, 0.16),
                    _ => (0.8, 0.55, 0.2),
                };
                if !gate_row && s >= 2 && r.f() < pb {
                    let y = r.range(-2.0, 2.0);
                    self.beams.push(Beam { d, y, broken: false });
                } else {
                    let n = if r.f() < p1 { 1 } else { 0 } + if r.f() < p2 { 1 } else { 0 };
                    let mut placed: Vec<f32> = Vec::new();
                    for _ in 0..n {
                        for _try in 0..8 {
                            let x = r.range(-6.0, 6.0);
                            let clear_gate = !gate_row || (x - gx).abs() > 2.8;
                            let clear_other = placed.iter().all(|&o| (o - x).abs() > 3.6);
                            if clear_gate && clear_other {
                                placed.push(x);
                                let rr = r.range(0.75, 1.05);
                                let rot = r.range(0.0, PI);
                                self.pillars.push(Pillar { d: d + r.range(-2.0, 2.0), x, r: rr, rot, broken: false });
                                break;
                            }
                        }
                    }
                }
            }
            self.gen_d += ROW_GAP;
            self.row += 1;
        }
        let behind = self.dist - 20.0;
        self.pillars.retain(|p| p.d > behind);
        self.beams.retain(|b| b.d > behind);
        self.gate_list.retain(|g| g.d > behind);
    }

    fn burst(&mut self, pos: V3, color: V3, kind: f32) {
        self.burst_seq += 1;
        let k = self.burst_next;
        self.burst_next = (self.burst_next + 1) % 8;
        self.bursts[k] = [pos[0], pos[1], pos[2], self.burst_seq as f32, color[0], color[1], color[2], kind];
    }

    fn popup(&mut self, text: String, color: [f32; 3]) {
        self.popups.push(Popup { text, age: 0.0, color });
    }

    fn add_score(&mut self, n: u32) {
        self.score += n;
        if self.score > self.best {
            self.best = self.score;
            self.new_best = true;
        }
    }

    /// The next gate ahead of the glider, if any.
    pub fn next_gate(&self) -> Option<(f32, f32, f32)> {
        self.gate_list
            .iter()
            .filter(|g| g.state == GateState::Ahead && g.d > self.dist)
            .min_by(|a, b| a.d.partial_cmp(&b.d).unwrap())
            .map(|g| (g.x, g.y, g.d - self.dist))
    }

    /// Up to four intact pillars ahead: (x, distance ahead).
    pub fn pillars_ahead(&self) -> [f32; 8] {
        let mut out = [0.0, 999.0, 0.0, 999.0, 0.0, 999.0, 0.0, 999.0];
        let mut v: Vec<(f32, f32)> = self
            .pillars
            .iter()
            .filter(|p| !p.broken && p.d + p.r > self.dist)
            .map(|p| (p.x, p.d - self.dist))
            .collect();
        v.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        for (k, (x, dz)) in v.iter().take(4).enumerate() {
            out[k * 2] = *x;
            out[k * 2 + 1] = *dz;
        }
        out
    }

    /// The next intact beam ahead: (y, distance ahead), or (0, 999).
    pub fn beam_ahead(&self) -> (f32, f32) {
        self.beams
            .iter()
            .filter(|b| !b.broken && b.d + 0.6 > self.dist)
            .min_by(|a, b| a.d.partial_cmp(&b.d).unwrap())
            .map(|b| (b.y, b.d - self.dist))
            .unwrap_or((0.0, 999.0))
    }

    pub fn update(&mut self, dt: f32, inp: Input) {
        self.frame += 1;
        self.sfx.clear();
        self.rumble = None;
        let a_pressed = inp.a && !self.prev.a;
        let start_pressed = inp.start && !self.prev.start;
        self.prev = inp;
        self.mode_t += dt;
        self.pal_t = (self.pal_t + dt / 3.0).min(1.0);
        self.hit_glow += (self.hit_target - self.hit_glow) * (1.0 - (-dt * 14.0).exp());
        self.hit_target = (self.hit_target - dt * 1.4).max(0.0);

        match self.mode {
            Mode::Title => {
                if (a_pressed || start_pressed) && self.mode_t > 0.3 {
                    self.start();
                }
            }
            Mode::Play => {
                if start_pressed {
                    self.mode = Mode::Paused;
                    self.sfx.push((Sfx::Pause, 0.6));
                    self.sim_dt = 0.0;
                    self.shift = 0.0;
                    return;
                }
            }
            Mode::Paused => {
                self.sim_dt = 0.0;
                self.shift = 0.0;
                if start_pressed || a_pressed {
                    self.mode = Mode::Play;
                    self.sfx.push((Sfx::Pause, 0.6));
                }
                return;
            }
            Mode::Over => {
                if a_pressed && self.mode_t > 1.2 {
                    self.start();
                }
            }
        }
        self.time += dt;
        self.sim_dt = dt;

        // Steering.
        let (ix, iy) = match self.mode {
            Mode::Play if self.autoplay => self.autopilot(),
            Mode::Play => (inp.x, inp.y),
            Mode::Title => self.autopilot(),
            _ => (0.0, 0.0),
        };
        let k = 1.0 - (-dt * 7.0).exp();
        self.vx += (ix * LATERAL - self.vx) * k;
        self.vy += (iy * LATERAL * 0.8 - self.vy) * k;
        self.px += self.vx * dt;
        self.py += self.vy * dt;
        if self.px.abs() > FLY_X {
            self.px = self.px.clamp(-FLY_X, FLY_X);
            self.vx = 0.0;
        }
        if self.py.abs() > FLY_Y {
            self.py = self.py.clamp(-FLY_Y, FLY_Y);
            self.vy = 0.0;
        }
        self.roll += (-self.vx * 0.035 - self.roll) * k;
        self.pitch += (self.vy * 0.045 - self.pitch) * k;

        // Forward speed.
        let target = match self.mode {
            Mode::Title => 16.0,
            Mode::Over => 0.0,
            _ => {
                let s = self.sector as f32 - 1.0;
                let within = (self.dist % SECTOR_LEN) / SECTOR_LEN;
                (22.0 + s * 5.0 + within * 3.0).min(44.0) * (1.0 - 0.45 * self.slow)
            }
        };
        let acc = if self.mode == Mode::Over { 1.5 } else { 1.2 };
        self.speed += (target - self.speed) * (1.0 - (-dt * acc).exp());
        self.slow = (self.slow - dt * 0.7).max(0.0);
        let prev_d = self.dist;
        self.dist += self.speed * dt;
        self.shift = self.speed * dt;
        self.invuln = (self.invuln - dt).max(0.0);
        self.generate();

        let playing = self.mode == Mode::Play;
        if playing {
            self.hint_t += dt;
            self.energy -= dt;
            // Distance points: one per unit flown.
            let whole = self.dist as u32 - prev_d as u32;
            self.add_score(whole);
        }

        // Gates.
        let mut passed: Vec<(f32, f32, f32)> = Vec::new();
        let mut missed = false;
        for g in self.gate_list.iter_mut() {
            g.t += dt;
            if g.state == GateState::Ahead && prev_d < g.d && g.d <= self.dist {
                let dx = self.px - g.x;
                let dy = self.py - g.y;
                g.t = 0.0;
                if (dx * dx + dy * dy).sqrt() < GATE_HIT_R {
                    g.state = GateState::Passed;
                    passed.push((g.x, g.y, -(g.d - self.dist)));
                } else {
                    g.state = GateState::Missed;
                    missed = true;
                }
            }
        }
        let pal = self.palette();
        for (x, y, z) in passed {
            self.burst([x, y, z], pal.gate, 0.0);
            if playing {
                self.combo = (self.combo + 1).min(5);
                self.gates += 1;
                let pts = 100 * self.combo * self.sector;
                self.add_score(pts);
                self.energy = (self.energy + ENERGY_GATE).min(ENERGY_MAX);
                let label = if self.combo > 1 { format!("+{} x{}", pts, self.combo) } else { format!("+{}", pts) };
                self.popup(label, [1.0, 0.85, 0.4]);
                self.sfx.push((Sfx::Gate, 0.5 + 0.1 * self.combo as f32));
                self.rumble = Some((0.0, 0.25, 70));
            }
        }
        if missed && playing {
            self.combo = 0;
        }

        // Crystals.
        if playing && self.invuln <= 0.0 {
            let mut hit: Option<V3> = None;
            for p in self.pillars.iter_mut() {
                if p.broken || (p.d - self.dist).abs() > p.r * 0.85 + 0.3 {
                    continue;
                }
                if (self.px - p.x).abs() < p.r * 0.82 + GLIDER_HALF_W {
                    p.broken = true;
                    hit = Some([p.x, self.py, -(p.d - self.dist)]);
                    break;
                }
            }
            if hit.is_none() {
                for b in self.beams.iter_mut() {
                    if b.broken || (b.d - self.dist).abs() > 0.8 {
                        continue;
                    }
                    if (self.py - b.y).abs() < 0.5 + GLIDER_HALF_H {
                        b.broken = true;
                        hit = Some([self.px, b.y, -(b.d - self.dist)]);
                        break;
                    }
                }
            }
            if let Some(pos) = hit {
                self.hits += 1;
                self.combo = 0;
                self.energy -= ENERGY_HIT;
                self.invuln = 1.0;
                self.slow = 1.0;
                self.hit_target = 1.0;
                self.burst(pos, pal.crystal, 1.0);
                self.popup(format!("-{} ENERGY", ENERGY_HIT as u32), [1.0, 0.4, 0.3]);
                self.sfx.push((Sfx::Hit, 1.0));
                self.rumble = Some((0.9, 0.6, 380));
            }
        }

        // Sectors.
        if playing {
            let s = Self::sector_at(self.dist);
            if s > self.sector {
                self.sector = s;
                self.pal_from = self.palette();
                self.pal_idx = ((s - 1) % 4) as usize;
                self.pal_t = 0.0;
                self.add_score(1000);
                self.energy = (self.energy + 8.0).min(ENERGY_MAX);
                self.sfx.push((Sfx::Sector, 0.9));
                if s == 2 && !self.goal {
                    self.goal = true;
                    self.banner = Some(("SECTOR 2 REACHED".into(), "GOAL CLEARED  +1000".into(), 0.0));
                } else {
                    self.banner = Some((format!("SECTOR {}", s), "+1000  FASTER NOW".into(), 0.0));
                }
            }
            if self.energy <= 0.0 {
                self.energy = 0.0;
                self.mode = Mode::Over;
                self.mode_t = 0.0;
                self.sfx.push((Sfx::Over, 1.0));
            }
        }

        if let Some(b) = self.banner.as_mut() {
            b.2 += dt;
            if b.2 > 3.2 {
                self.banner = None;
            }
        }
        for p in self.popups.iter_mut() {
            p.age += dt;
        }
        self.popups.retain(|p| p.age < 1.2);
    }

    /// Title-screen flight: weave toward the next ring.
    /// Flies by itself: toward the next ring, around crystals ahead. The
    /// title screen uses it, and so does a test or tool that sets the
    /// "autoplay" debug field.
    pub fn autopilot(&self) -> (f32, f32) {
        let (mut tx, mut ty) = match self.next_gate() {
            Some((x, y, dz)) if dz < 80.0 => (x, y),
            _ => ((self.time * 0.4).sin() * 3.0, (self.time * 0.55).sin() * 1.2),
        };
        let p = self.pillars_ahead();
        for k in 0..4 {
            let (x, dz) = (p[k * 2], p[k * 2 + 1]);
            if dz > 0.0 && dz < 16.0 && (x - tx).abs() < 2.1 {
                tx = x + if tx >= x { 2.3 } else { -2.3 };
            }
        }
        let (by, bdz) = self.beam_ahead();
        if bdz > 0.0 && bdz < 16.0 && (by - ty).abs() < 1.3 {
            ty = by + if ty >= by { 1.5 } else { -1.5 };
        }
        let tx = tx.clamp(-FLY_X + 0.2, FLY_X - 0.2);
        let ty = ty.clamp(-FLY_Y + 0.2, FLY_Y - 0.2);
        (((tx - self.px) * 0.9).clamp(-1.0, 1.0), ((ty - self.py) * 0.9).clamp(-1.0, 1.0))
    }

    /// Build the frame: camera, lights, instances, particles.
    pub fn frame_data(&mut self, dt: f32) -> FrameData {
        let pal = self.palette();
        let t = self.time;

        // Camera: behind and above the glider, easing after it (no shake).
        let want = [self.px * 0.55, self.py * 0.5 + 2.3, 6.4];
        let look = [self.px * 0.8, self.py * 0.7 - 0.6, -14.0];
        let k = 1.0 - (-dt.max(1.0 / 120.0) * 6.0).exp();
        self.cam = lerp3(self.cam, want, k);
        self.cam_look = lerp3(self.cam_look, look, k);
        let view = look_at(self.cam, self.cam_look, [0.0, 1.0, 0.0]);
        let proj = perspective(62f32.to_radians(), crate::W as f32 / crate::H as f32, 0.1, 320.0);
        let vp = mul(&proj, &view);

        let mut scene = SceneU {
            view_proj: vp,
            cam: [self.cam[0], self.cam[1], self.cam[2], t],
            sun_dir: [0.3, 0.85, 0.35, 0.32],
            sun_col: [pal.sun[0] * 0.55, pal.sun[1] * 0.55, pal.sun[2] * 0.55, self.dist % 800.0],
            fog: [pal.fog[0], pal.fog[1], pal.fog[2], 125.0],
            sky_top: [pal.sky[0], pal.sky[1], pal.sky[2], 0.0],
            accent: [pal.accent[0], pal.accent[1], pal.accent[2], 0.55],
            cam_right: [view[0], view[4], view[8], 0.0],
            cam_up: [view[1], view[5], view[9], 0.0],
            lights: [[0.0; 4]; 16],
        };

        let mut inst: [Vec<Inst>; 4] = Default::default();
        let rel = |d: f32| -(d - self.dist);

        // Canyon segments and their scenery.
        let first = ((self.dist - 24.0) / SEG_LEN).floor() as i32;
        let last = ((self.dist + VIEW_AHEAD) / SEG_LEN).ceil() as i32;
        for s in first..=last {
            let d = s as f32 * SEG_LEN;
            let z = rel(d);
            inst[CANYON].push(Inst {
                m: trs([0.0, 0.0, z], [0.0; 3], [1.0; 3]),
                color: [pal.rock[0], pal.rock[1], pal.rock[2], 1.0],
                params: [0.0, 1.0, 0.0, 0.0],
            });
            let h = hash(s as u32);
            let mut hr = Rng(h | 1);
            // Wall crystals leaning in from both sides.
            for _ in 0..3 {
                let side = if hr.f() < 0.5 { -1.0 } else { 1.0 };
                let y = hr.range(-4.2, 3.2);
                let len = hr.range(1.4, 3.8);
                let rad = hr.range(0.35, 0.8);
                let lean = side * hr.range(0.6, 1.1);
                let dz = hr.range(0.0, SEG_LEN);
                // Rooted inside the rock so no crystal hangs in the air.
                inst[CRYSTAL].push(Inst {
                    m: trs([side * (wall_x(y) + 0.9), y, z - dz], [hr.range(-0.2, 0.2), hr.range(0.0, 3.0), lean], [rad, len + 1.0, rad]),
                    color: [pal.crystal[0] * 0.6, pal.crystal[1] * 0.6, pal.crystal[2] * 0.6, 1.0],
                    params: [0.2, 0.0, hr.range(0.0, 6.0), 0.0],
                });
            }
            // Shards on the floor.
            for _ in 0..2 {
                let x = hr.range(-10.0, 10.0);
                inst[CRYSTAL].push(Inst {
                    m: trs(
                        [x, FLOOR_Y - 0.1, z - hr.range(0.0, SEG_LEN)],
                        [hr.range(-0.4, 0.4), hr.range(0.0, 3.0), hr.range(-0.4, 0.4)],
                        [hr.range(0.15, 0.35), hr.range(0.4, 1.1), hr.range(0.15, 0.35)],
                    ),
                    color: [pal.crystal[0] * 0.7, pal.crystal[1] * 0.7, pal.crystal[2] * 0.7, 1.0],
                    params: [0.25, 0.0, hr.range(0.0, 6.0), 0.0],
                });
            }
            // A crystal arch over the canyon every sixth segment.
            if s.rem_euclid(6) == 0 {
                let n = 13;
                let rad = 12.6;
                for k in 0..n {
                    let a0 = PI * (0.04 + 0.92 * k as f32 / n as f32);
                    let seg = PI * 0.92 / n as f32 * rad * 1.45;
                    let pos = [a0.cos() * rad, FLOOR_Y + a0.sin() * rad, z - 3.0];
                    inst[CRYSTAL].push(Inst {
                        m: trs(pos, [0.0, 0.0, a0 - 0.03], [0.62, seg, 0.62]),
                        color: [pal.crystal[0], pal.crystal[1], pal.crystal[2], 1.0],
                        params: [0.55, 0.0, k as f32 * 0.5 + s as f32, 0.0],
                    });
                }
            }
        }

        // Obstacle crystals: bright, tall, the thing to avoid.
        for p in self.pillars.iter().filter(|p| !p.broken) {
            let z = rel(p.d);
            if z < -VIEW_AHEAD || z > 12.0 {
                continue;
            }
            let rise = smooth((VIEW_AHEAD - (p.d - self.dist)) / 40.0);
            inst[CRYSTAL].push(Inst {
                m: trs([p.x, FLOOR_Y - 0.3, z], [0.0, p.rot + t * 0.25, 0.0], [p.r, 12.2 * rise, p.r]),
                color: [pal.crystal[0], pal.crystal[1], pal.crystal[2], 1.0],
                params: [1.5, 0.0, p.rot * 3.0, 0.0],
            });
        }
        for b in self.beams.iter().filter(|b| !b.broken) {
            let z = rel(b.d);
            inst[CRYSTAL].push(Inst {
                m: trs([-WALL_X - 1.0, b.y, z], [0.0, 0.0, -PI / 2.0], [0.5, 2.0 * WALL_X + 2.0, 0.5]),
                color: [pal.crystal[0], pal.crystal[1], pal.crystal[2], 1.0],
                params: [1.0, 0.0, b.d, 0.0],
            });
        }

        // Gates, and the brightest ones ahead become point lights.
        let mut light_n = 0usize;
        let mut lights = [[0.0f32; 4]; 16];
        let put_light = |lights: &mut [[f32; 4]; 16], n: &mut usize, p: V3, radius: f32, c: V3, i: f32| {
            if *n < 8 {
                lights[*n * 2] = [p[0], p[1], p[2], radius];
                lights[*n * 2 + 1] = [c[0], c[1], c[2], i];
                *n += 1;
            }
        };
        // The glider's engine light first.
        let gp = [self.px, self.py, 0.0];
        put_light(&mut lights, &mut light_n, [gp[0], gp[1] + 0.3, 1.2], 9.0, pal.mote, 1.1);
        if self.hit_glow > 0.02 {
            put_light(&mut lights, &mut light_n, [gp[0], gp[1], -2.0], 14.0, [1.0, 0.25, 0.15], 2.2 * self.hit_glow);
        }
        let mut gates: Vec<&Gate> = self.gate_list.iter().collect();
        gates.sort_by(|a, b| a.d.partial_cmp(&b.d).unwrap());
        for g in gates {
            let z = rel(g.d);
            // Rings behind the glider would sit between it and the camera and
            // fill the screen: they go once crossed (the burst shows the pass).
            if z < -VIEW_AHEAD || z > 0.2 {
                continue;
            }
            let (scale, glow) = match g.state {
                GateState::Ahead => (1.0, 2.2),
                GateState::Passed => (1.0 + g.t * 2.5, (2.6 * (1.0 - g.t * 2.0)).max(0.0)),
                GateState::Missed => (1.0, 0.35),
            };
            if g.state == GateState::Passed && g.t > 0.5 {
                continue;
            }
            let c = pal.gate;
            inst[RING].push(Inst {
                m: trs([g.x, g.y, z], [0.0, 0.0, t * 0.6 + g.d], [GATE_R * scale, GATE_R * scale, GATE_R]),
                color: [c[0], c[1], c[2], 1.0],
                params: [glow, 2.0, g.d, 0.0],
            });
            inst[RING].push(Inst {
                m: trs([g.x, g.y, z], [0.0, 0.0, -t * 0.9], [GATE_R * 0.78 * scale, GATE_R * 0.78 * scale, GATE_R * 0.5]),
                color: [c[0], c[1], c[2] * 0.8, 1.0],
                params: [glow * 0.6, 2.0, g.d + 1.0, 0.0],
            });
            if g.state == GateState::Ahead && z < 2.0 {
                put_light(&mut lights, &mut light_n, [g.x, g.y, z], 10.0, c, 1.6);
            }
        }
        scene.lights = lights;

        // The glider.
        {
            let blink = if self.invuln > 0.0 { 0.35 * (0.5 + 0.5 * (self.invuln * PI * 3.0).cos()) } else { 0.0 };
            inst[GLIDER].push(Inst {
                m: trs(gp, [self.pitch, 0.0, self.roll], [1.25; 3]),
                color: [0.62, 0.68, 0.78, 1.0],
                params: [2.4, 3.0, 0.0, blink],
            });
            // Engine colour comes from the mote colour.
            inst[GLIDER].last_mut().unwrap().color = [0.6, 0.66, 0.76, 1.0];
        }

        let bursts = {
            let mut b = [[0.0f32; 4]; 16];
            for (k, x) in self.bursts.iter().enumerate() {
                b[k * 2] = [x[0], x[1], x[2], x[3]];
                b[k * 2 + 1] = [x[4], x[5], x[6], x[7]];
            }
            b
        };
        let trail_on = if self.mode == Mode::Over { 0.0 } else { 1.0 };
        let sim = SimU {
            a: [self.sim_dt, t, self.shift, self.frame as f32],
            glider: [gp[0], gp[1], gp[2], trail_on],
            glider_vel: [self.vx, self.vy, 0.0, 0.0],
            mote_col: [pal.mote[0], pal.mote[1], pal.mote[2], 0.0],
            trail_col: [pal.mote[0] * 0.9 + 0.1, pal.mote[1] * 0.9 + 0.1, pal.mote[2], 0.0],
            bursts,
        };
        FrameData { scene, sim, inst, clear: pal.fog }
    }

    pub fn hud(&self, h: &mut Hud) {
        h.clear();
        let w = crate::W as f32;
        let hh = crate::H as f32;
        let white = rgba(0.95, 0.97, 1.0, 1.0);
        let dim = rgba(0.55, 0.65, 0.8, 0.95);
        let gold = rgba(1.0, 0.78, 0.3, 1.0);

        // Crash glow: a red edge that swells and fades, never a full-screen pop.
        if self.hit_glow > 0.01 {
            for k in 0..20 {
                let f = 1.0 - k as f32 / 20.0;
                let a = 0.22 * self.hit_glow * f * f;
                let i = k as f32 * 3.0;
                let c = rgba(0.9, 0.12, 0.06, a);
                h.rect(i, i, w - 2.0 * i, 3.0, c);
                h.rect(i, hh - i - 3.0, w - 2.0 * i, 3.0, c);
                h.rect(i, i + 3.0, 3.0, hh - 2.0 * i - 6.0, c);
                h.rect(w - i - 3.0, i + 3.0, 3.0, hh - 2.0 * i - 6.0, c);
            }
        }

        match self.mode {
            Mode::Title => {
                h.rect(0.0, 120.0, w, 270.0, rgba(0.0, 0.0, 0.02, 0.45));
                h.text_center("STARWELL", w / 2.0, 150.0, 10.0, rgba(0.75, 0.95, 1.0, 1.0));
                h.text_center("A GLIDER, A CANYON OF CRYSTAL, NO WAY BUT DOWN", w / 2.0, 240.0, 2.0, dim);
                let pulse = 0.65 + 0.35 * (self.mode_t * 2.4).sin();
                h.text_center("PRESS A TO START", w / 2.0, 285.0, 4.0, rgba(1.0, 0.85, 0.4, pulse));
                h.text_center("STICK OR D-PAD: STEER     START: PAUSE", w / 2.0, 340.0, 2.0, white);
                h.text_center("FLY THROUGH GOLD RINGS FOR ENERGY. DODGE CRYSTALS.", w / 2.0, 362.0, 2.0, white);
                if self.best > 0 {
                    h.text_center(&format!("BEST {}", self.best), w / 2.0, 420.0, 3.0, gold);
                }
                return;
            }
            _ => {}
        }

        // Score and best.
        h.text("SCORE", 20.0, 16.0, 2.0, dim);
        h.text(&format!("{}", self.score), 20.0, 34.0, 4.0, white);
        h.text_right("BEST", w - 20.0, 16.0, 2.0, dim);
        h.text_right(&format!("{}", self.best), w - 20.0, 34.0, 4.0, gold);

        // Sector and progress to the next one.
        h.text_center(&format!("SECTOR {}", self.sector), w / 2.0, 14.0, 3.0, white);
        let prog = (self.dist % SECTOR_LEN) / SECTOR_LEN;
        let (bx, by, bw) = (w / 2.0 - 130.0, 42.0, 260.0);
        h.rect(bx - 2.0, by - 2.0, bw + 4.0, 12.0, rgba(0.0, 0.0, 0.0, 0.5));
        h.rect(bx, by, bw * prog, 8.0, rgba(0.35, 0.8, 1.0, 0.9));
        let goal_text = if self.sector == 1 { "GOAL: REACH SECTOR 2" } else { "NEXT SECTOR" };
        h.text_center(goal_text, w / 2.0, 56.0, 2.0, dim);

        // Energy.
        let e = (self.energy / ENERGY_MAX).clamp(0.0, 1.0);
        let low = self.energy < 8.0;
        let (ex, ey, ew) = (20.0, hh - 40.0, 300.0);
        h.text("ENERGY", ex, ey - 20.0, 2.0, if low { rgba(1.0, 0.5, 0.3, 1.0) } else { dim });
        h.rect(ex - 3.0, ey - 3.0, ew + 6.0, 20.0, rgba(0.0, 0.0, 0.0, 0.55));
        let ec = if low {
            let p = 0.5 + 0.5 * (self.time * 4.0).sin();
            rgba(1.0, 0.35 + 0.2 * p, 0.15, 1.0)
        } else if e < 0.4 {
            rgba(1.0, 0.75, 0.25, 1.0)
        } else {
            rgba(0.3, 0.95, 0.75, 1.0)
        };
        h.rect(ex, ey, ew * e, 14.0, ec);
        h.text(&format!("{}", self.energy.ceil() as u32), ex + ew + 12.0, ey, 2.0, white);

        if self.combo > 1 {
            h.text_right(&format!("CHAIN x{}", self.combo), w - 20.0, hh - 40.0, 3.0, gold);
        }

        // Popups rise and fade from just above the glider.
        for (k, p) in self.popups.iter().enumerate() {
            let a = (1.0 - p.age / 1.2).clamp(0.0, 1.0);
            let y = 300.0 - p.age * 50.0 - k as f32 * 4.0;
            h.text_center(&p.text, w / 2.0, y, 3.0, [p.color[0], p.color[1], p.color[2], a]);
        }

        if let Some((big, small, t)) = &self.banner {
            let a = (t / 0.4).min(1.0) * ((3.2 - t) / 0.6).clamp(0.0, 1.0);
            h.rect(0.0, 128.0, w, 74.0, rgba(0.0, 0.0, 0.02, 0.35 * a));
            h.text_center(big, w / 2.0, 138.0, 5.0, rgba(1.0, 1.0, 1.0, a));
            h.text_center(small, w / 2.0, 180.0, 2.0, rgba(1.0, 0.85, 0.4, a));
        }

        if self.mode == Mode::Play && self.hint_t < 7.0 && self.hint_t > 0.6 {
            let a = ((self.hint_t - 0.6) / 0.5).min(1.0) * ((7.0 - self.hint_t) / 0.8).clamp(0.0, 1.0);
            h.text_center("STEER THROUGH THE GOLD RINGS FOR ENERGY", w / 2.0, hh - 92.0, 2.0, rgba(1.0, 0.85, 0.4, a));
            h.text_center("AVOID THE CRYSTALS", w / 2.0, hh - 72.0, 2.0, rgba(0.6, 0.9, 1.0, a));
        }

        match self.mode {
            Mode::Paused => {
                h.rect(0.0, 0.0, w, hh, rgba(0.0, 0.0, 0.02, 0.45));
                h.text_center("PAUSED", w / 2.0, 220.0, 7.0, white);
                h.text_center("PRESS START OR A TO RESUME", w / 2.0, 290.0, 2.0, dim);
            }
            Mode::Over => {
                let a = (self.mode_t / 0.8).min(1.0);
                h.rect(0.0, 0.0, w, hh, rgba(0.0, 0.0, 0.02, 0.5 * a));
                h.text_center("OUT OF ENERGY", w / 2.0, 140.0, 6.0, rgba(1.0, 0.6, 0.45, a));
                h.text_center(&format!("SCORE {}", self.score), w / 2.0, 220.0, 4.0, rgba(1.0, 1.0, 1.0, a));
                let best = if self.new_best { "NEW BEST!".to_string() } else { format!("BEST {}", self.best) };
                h.text_center(&best, w / 2.0, 262.0, 3.0, [gold[0], gold[1], gold[2], a]);
                let goal = if self.goal {
                    format!("GOAL CLEARED - REACHED SECTOR {}", self.sector)
                } else {
                    format!("{} TO SECTOR 2", (SECTOR_LEN - self.dist).max(0.0) as u32)
                };
                h.text_center(&goal, w / 2.0, 300.0, 2.0, rgba(0.7, 0.9, 1.0, a));
                h.text_center(&format!("RINGS {}   CRYSTALS HIT {}", self.gates, self.hits), w / 2.0, 324.0, 2.0, rgba(0.7, 0.8, 0.9, a));
                if self.mode_t > 1.2 {
                    let p = 0.65 + 0.35 * (self.mode_t * 2.4).sin();
                    h.text_center("PRESS A TO FLY AGAIN", w / 2.0, 380.0, 3.0, rgba(1.0, 0.85, 0.4, p));
                }
            }
            _ => {}
        }
    }
}
