//! Tetherline: a swinging platformer.
//!
//! Run and jump with the left stick or d-pad and A. Hold B to fire a tether
//! at the ring marked ahead of you, swing, and let go of B to fly (or press A
//! on the tether for a jump off it). Up and down reel the line. Five islands,
//! each with three sun shards to find and a flag at its end. Every connected
//! pad steers the one climber, so an idle second pad changes nothing.
//! Keyboard fallback: arrows, Z jump, X tether, Enter.

mod levels;

use levels::{Level, CRUMBLE, EMPTY, ROWS, SOLID, SPIKES, TILE};
use macroquad::audio::{load_sound, play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;
use wasmcart_macroquad as wc;

const W: f32 = 960.0;
const H: f32 = 540.0;
const GRAVITY: f32 = 1500.0;
const RUN: f32 = 270.0;
const JUMP: f32 = 570.0;
const HALF_W: f32 = 10.0;
const HALF_H: f32 = 13.0;
const REACH: f32 = 290.0;
const ROPE_MIN: f32 = 40.0;
const ROPE_MAX: f32 = 330.0;
/// A fresh tether pulls in to at most this length, so a catch made from far
/// below still swings clear of the ground.
const ROPE_CATCH: f32 = 200.0;
const LIVES: i32 = 5;

fn window_conf() -> Conf {
    Conf { window_title: "Tetherline".to_owned(), window_width: W as i32, window_height: H as i32, ..Default::default() }
}

// ─── input ──────────────────────────────────────────────────────────────

#[derive(Default, Clone, Copy)]
struct Input {
    x: f32,
    y: f32,
    jump: bool,
    jump_held: bool,
    tether_held: bool,
    start: bool,
    confirm: bool,
}

struct Pads {
    prev: u32,
}

impl Pads {
    fn read(&mut self) -> Input {
        let mut held = 0u32;
        let mut stick = Vec2::ZERO;
        for i in 0..4 {
            let p = wc::pad(i);
            if p.connected == 0 {
                continue;
            }
            held |= p.buttons;
            let s = vec2(p.left_x as f32, p.left_y as f32) / 32767.0;
            if s.length() > 0.3 {
                stick += s;
            }
        }
        let k = |c: KeyCode| is_key_down(c);
        let mut keys = 0u32;
        for (c, m) in [
            (KeyCode::Left, wc::WC_BTN_LEFT),
            (KeyCode::Right, wc::WC_BTN_RIGHT),
            (KeyCode::Up, wc::WC_BTN_UP),
            (KeyCode::Down, wc::WC_BTN_DOWN),
            (KeyCode::Z, wc::WC_BTN_A),
            (KeyCode::Space, wc::WC_BTN_A),
            (KeyCode::X, wc::WC_BTN_B),
            (KeyCode::Enter, wc::WC_BTN_START),
        ] {
            if k(c) {
                keys |= m;
            }
        }
        held |= keys;
        self.apply(held, stick)
    }

    /// Turn a held-button mask and stick into this frame's input.
    fn apply(&mut self, held: u32, stick: Vec2) -> Input {
        let pressed = held & !self.prev;
        self.prev = held;
        let d = |m: u32| if held & m != 0 { 1.0 } else { 0.0 };
        let x = (stick.x + d(wc::WC_BTN_RIGHT) - d(wc::WC_BTN_LEFT)).clamp(-1.0, 1.0);
        let y = (stick.y + d(wc::WC_BTN_DOWN) - d(wc::WC_BTN_UP)).clamp(-1.0, 1.0);
        let tether = wc::WC_BTN_B | wc::WC_BTN_X | wc::WC_BTN_R;
        Input {
            x,
            y,
            jump: pressed & wc::WC_BTN_A != 0,
            jump_held: held & wc::WC_BTN_A != 0,
            tether_held: held & tether != 0,
            start: pressed & wc::WC_BTN_START != 0,
            confirm: pressed & (wc::WC_BTN_A | wc::WC_BTN_START) != 0,
        }
    }
}

// ─── autopilot ──────────────────────────────────────────────────────────

/// A test autopilot, switched on by a debug host writing `autopilot = 1`
/// (romdev `wasm({op:'write'})`, `CartHost.writeDebugValue`); a cart built
/// without the `debug` feature never has it on. It presses buttons the way
/// the signs teach: run right, jump hazards, hold B for the marked ring over
/// a drop and let go past the top of the swing, reel up a ring over a wall.
#[derive(Default)]
struct Autopilot {
    air: u32,
    tethered_for: u32,
    leapt: bool,
    climb: bool,
}

impl Autopilot {
    fn buttons(&mut self, g: &Game, frame: u32) -> u32 {
        use wc::{WC_BTN_A as A, WC_BTN_B as B, WC_BTN_RIGHT as RIGHT, WC_BTN_UP as UP};
        if g.mode != Mode::Play {
            let wants = matches!(g.mode, Mode::Title | Mode::Cleared | Mode::Over | Mode::Won);
            return if wants && frame % 40 < 3 { A } else { 0 };
        }
        let mut b = RIGHT;
        if let Some((i, _)) = g.tether {
            let d = g.level.anchors[i].pos(g.clock) - g.pos;
            self.leapt = true;
            self.tethered_for += 1;
            // Swinging into a wall: reel up over it instead.
            if !self.climb && g.wall_ahead() < 70.0 && d.y < -100.0 {
                self.climb = true;
            }
            if self.climb {
                if d.y < -45.0 {
                    return B | UP;
                }
                if d.x < -20.0 {
                    self.climb = false;
                    return RIGHT;
                }
                return B | RIGHT;
            }
            b |= B;
            if (d.x < -60.0 && g.vel.y < 0.0) || d.x < -150.0 || self.tethered_for > 240 {
                b &= !B;
            }
            return b;
        }
        self.tethered_for = 0;
        let ring_ahead = g
            .level
            .anchors
            .iter()
            .map(|a| a.pos(g.clock) - g.pos)
            .filter(|d| d.x > 10.0)
            .min_by(|a, c| a.x.total_cmp(&c.x));
        let target = g.target.map(|i| g.level.anchors[i].pos(g.clock) - g.pos);
        if g.grounded {
            self.air = 0;
            let ring_near = ring_ahead.is_some_and(|d| d.x < 260.0 && d.y < 0.0);
            let gap = g.gap_ahead();
            let over_drop = gap < if ring_near { 30.0 } else { 14.0 };
            if g.spikes_ahead() < 45.0 || g.wall_ahead() < 30.0 || over_drop {
                b |= A;
            }
            self.leapt = gap < 160.0;
            self.climb = g.wall_ahead() < 40.0 && target.is_some_and(|d| d.y < -120.0);
            if self.climb {
                b |= B;
            }
        } else {
            self.air += 1;
            if g.vel.y < 0.0 {
                b |= A;
            }
            let reach = target.is_some_and(|d| d.x > 20.0);
            if reach && (g.vel.y > -150.0 || self.air > 10) && (self.leapt || g.pos.y > 470.0) {
                b |= B;
            }
        }
        b
    }
}

// ─── game ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Title,
    Play,
    Paused,
    Dying,
    Cleared,
    Over,
    Won,
}

struct Sounds {
    jump: Sound,
    tether: Sound,
    clip: Sound,
    whoosh: Sound,
    shard: Sound,
    spark: Sound,
    checkpoint: Sound,
    fall: Sound,
    flag: Sound,
}

struct Puff {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max: f32,
    color: Color,
    size: f32,
}

struct Game {
    mode: Mode,
    mode_t: f32,
    level_i: usize,
    level: Level,
    /// Live tile state (crumbling ledges fall and grow back).
    tiles: Vec<u8>,
    crumble: Vec<(usize, f32)>,
    pos: Vec2,
    vel: Vec2,
    facing: f32,
    grounded: bool,
    coyote: f32,
    jump_buf: f32,
    /// Tether: the anchor index and the rope length.
    tether: Option<(usize, f32)>,
    /// Length the line is pulling in to after a catch.
    rope_goal: f32,
    tether_anim: f32,
    target: Option<usize>,
    checkpoint: Vec2,
    sparks_got: Vec<bool>,
    shards_got: Vec<bool>,
    checks_on: Vec<bool>,
    lives: i32,
    deaths: i32,
    score: i32,
    level_score: i32,
    best: i32,
    time: f32,
    total_shards: i32,
    puffs: Vec<Puff>,
    scarf: [Vec2; 6],
    cam: f32,
    clock: f32,
    squash: f32,
    tally: (i32, i32, i32),
}

fn tile(tiles: &[u8], cols: usize, c: i32, r: i32) -> u8 {
    if c < 0 || c >= cols as i32 {
        return SOLID; // the island's ends are walls
    }
    if r < 0 || r >= ROWS as i32 {
        return EMPTY;
    }
    tiles[r as usize * cols + c as usize]
}

fn solid(t: u8) -> bool {
    t == SOLID || t == CRUMBLE
}

impl Game {
    fn new() -> Game {
        let level = levels::build(0);
        let tiles = level.tiles.clone();
        let start = level.start;
        let n = (level.sparks.len(), level.shards.len(), level.checkpoints.len());
        Game {
            mode: Mode::Title,
            mode_t: 0.0,
            level_i: 0,
            level,
            tiles,
            crumble: Vec::new(),
            pos: start,
            vel: Vec2::ZERO,
            facing: 1.0,
            grounded: false,
            coyote: 0.0,
            jump_buf: 0.0,
            tether: None,
            rope_goal: 0.0,
            tether_anim: 0.0,
            target: None,
            checkpoint: start,
            sparks_got: vec![false; n.0],
            shards_got: vec![false; n.1],
            checks_on: vec![false; n.2],
            lives: LIVES,
            deaths: 0,
            score: 0,
            level_score: 0,
            best: 0,
            time: 0.0,
            total_shards: 0,
            puffs: Vec::new(),
            scarf: [start; 6],
            cam: 0.0,
            clock: 0.0,
            squash: 0.0,
            tally: (0, 0, 0),
        }
    }

    fn set_mode(&mut self, m: Mode) {
        self.mode = m;
        self.mode_t = 0.0;
    }

    fn load(&mut self, i: usize) {
        self.level_i = i;
        self.level = levels::build(i);
        self.tiles = self.level.tiles.clone();
        self.crumble.clear();
        self.sparks_got = vec![false; self.level.sparks.len()];
        self.shards_got = vec![false; self.level.shards.len()];
        self.checks_on = vec![false; self.level.checkpoints.len()];
        self.checkpoint = self.level.start;
        self.level_score = self.score;
        self.time = 0.0;
        self.respawn();
        self.cam = (self.pos.x - W * 0.35).clamp(0.0, self.level.cols as f32 * TILE - W);
    }

    fn respawn(&mut self) {
        self.pos = self.checkpoint;
        self.vel = Vec2::ZERO;
        self.tether = None;
        self.scarf = [self.pos; 6];
        self.facing = 1.0;
        // Ledges grow back when you come back.
        self.tiles = self.level.tiles.clone();
        self.crumble.clear();
    }

    fn puff(&mut self, at: Vec2, color: Color, n: usize, speed: f32) {
        for _ in 0..n {
            let a = rand::gen_range(0.0, std::f32::consts::TAU);
            let life = rand::gen_range(0.3, 0.7);
            self.puffs.push(Puff {
                pos: at,
                vel: vec2(a.cos(), a.sin()) * rand::gen_range(0.2, 1.0) * speed,
                life,
                max: life,
                color,
                size: rand::gen_range(2.0, 5.0),
            });
        }
    }

    fn cols(&self) -> usize {
        self.level.cols
    }

    /// Does the player's box at `p` overlap a solid tile?
    fn hits(&self, p: Vec2) -> bool {
        let c0 = ((p.x - HALF_W) / TILE).floor() as i32;
        let c1 = ((p.x + HALF_W - 0.01) / TILE).floor() as i32;
        let r0 = ((p.y - HALF_H) / TILE).floor() as i32;
        let r1 = ((p.y + HALF_H - 0.01) / TILE).floor() as i32;
        for r in r0..=r1 {
            for c in c0..=c1 {
                if solid(tile(&self.tiles, self.cols(), c, r)) {
                    return true;
                }
            }
        }
        false
    }

    fn touching_spikes(&self) -> bool {
        // A smaller box: spikes only count when you really land on them.
        let (hw, hh) = (HALF_W - 3.0, HALF_H - 2.0);
        let c0 = ((self.pos.x - hw) / TILE).floor() as i32;
        let c1 = ((self.pos.x + hw) / TILE).floor() as i32;
        let r0 = ((self.pos.y - hh) / TILE).floor() as i32;
        let r1 = ((self.pos.y + hh) / TILE).floor() as i32;
        for r in r0..=r1 {
            for c in c0..=c1 {
                if tile(&self.tiles, self.cols(), c, r) == SPIKES {
                    // The spike's lower two thirds.
                    let top = r as f32 * TILE + TILE * 0.35;
                    if self.pos.y + hh > top {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Move by `d`, stopping at walls; returns which axes were blocked.
    fn move_and_collide(&mut self, d: Vec2) -> (bool, bool) {
        let mut bx = false;
        let mut by = false;
        // Sub-steps so a fast swing never tunnels through a ledge.
        let steps = (d.length() / 8.0).ceil().max(1.0) as i32;
        let sd = d / steps as f32;
        for _ in 0..steps {
            let nx = self.pos + vec2(sd.x, 0.0);
            if self.hits(nx) {
                bx = true;
            } else {
                self.pos = nx;
            }
            let ny = self.pos + vec2(0.0, sd.y);
            if self.hits(ny) {
                by = true;
            } else {
                self.pos = ny;
            }
        }
        (bx, by)
    }

    /// True when nothing solid lies on the straight line from a to b.
    fn clear_line(&self, a: Vec2, b: Vec2) -> bool {
        let d = b - a;
        let n = (d.length() / 8.0).ceil() as i32;
        for k in 1..n {
            let p = a + d * (k as f32 / n as f32);
            let t = tile(&self.tiles, self.cols(), (p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32);
            if solid(t) {
                return false;
            }
        }
        true
    }

    fn pick_target(&self, inp: &Input) -> Option<usize> {
        // Aim where the stick points; with no stick, ahead and up.
        let aim = if vec2(inp.x, inp.y).length() > 0.3 {
            let a = vec2(inp.x, inp.y.min(0.2) - 0.6);
            a.normalize()
        } else {
            vec2(self.facing * 0.6, -0.8).normalize()
        };
        let mut best = None;
        let mut best_s = -1.0;
        for (i, a) in self.level.anchors.iter().enumerate() {
            let d = a.pos(self.clock) - self.pos;
            let l = d.length();
            if l > REACH || l < 20.0 || !self.clear_line(self.pos, a.pos(self.clock)) {
                continue;
            }
            let dir = d / l;
            let s = dir.dot(aim) + 0.35 - l / REACH * 0.35;
            if dir.dot(aim) > -0.1 && s > best_s {
                best_s = s;
                best = Some(i);
            }
        }
        best
    }

    fn update(&mut self, inp: &Input, dt: f32, snd: &Sounds) {
        self.mode_t += dt;
        self.clock += dt;
        match self.mode {
            Mode::Title => {
                if inp.confirm && self.mode_t > 0.3 {
                    self.score = 0;
                    self.lives = LIVES;
                    self.deaths = 0;
                    self.total_shards = 0;
                    self.load(0);
                    self.set_mode(Mode::Play);
                }
            }
            Mode::Paused => {
                if inp.start || inp.confirm {
                    self.set_mode(Mode::Play);
                }
            }
            Mode::Play => {
                if inp.start {
                    self.set_mode(Mode::Paused);
                } else {
                    self.play(inp, dt, snd);
                }
            }
            Mode::Dying => {
                self.vel.y += GRAVITY * 0.5 * dt;
                self.pos += self.vel * dt;
                if self.mode_t > 1.0 {
                    if self.lives <= 0 {
                        self.best = self.best.max(self.score);
                        self.set_mode(Mode::Over);
                    } else {
                        self.respawn();
                        self.set_mode(Mode::Play);
                    }
                }
            }
            Mode::Cleared => {
                if inp.confirm && self.mode_t > 1.0 {
                    if self.level_i + 1 >= levels::COUNT {
                        self.best = self.best.max(self.score);
                        self.set_mode(Mode::Won);
                    } else {
                        let next = self.level_i + 1;
                        self.load(next);
                        self.set_mode(Mode::Play);
                    }
                }
            }
            Mode::Over => {
                // Try the island again from its start, with the score it began with.
                if inp.confirm && self.mode_t > 1.2 {
                    self.score = self.level_score;
                    self.lives = LIVES;
                    let i = self.level_i;
                    self.load(i);
                    self.set_mode(Mode::Play);
                }
            }
            Mode::Won => {
                if inp.confirm && self.mode_t > 1.5 {
                    let best = self.best;
                    *self = Game::new();
                    self.best = best;
                }
            }
        }
        for p in self.puffs.iter_mut() {
            p.pos += p.vel * dt;
            p.vel *= 1.0 - 2.0 * dt;
            p.vel.y += 200.0 * dt;
            p.life -= dt;
        }
        self.puffs.retain(|p| p.life > 0.0);
        // The scarf trails behind, each knot easing toward the one ahead.
        let anchor = self.pos + vec2(-self.facing * 6.0, -6.0);
        self.scarf[0] = anchor;
        for i in 1..self.scarf.len() {
            let prev = self.scarf[i - 1];
            let want = prev + vec2(-self.facing * 5.0, 1.5 + (self.clock * 9.0 + i as f32).sin() * 1.2);
            self.scarf[i] += (want - self.scarf[i]) * (18.0 * dt).min(1.0);
            let d = self.scarf[i] - prev;
            if d.length() > 7.0 {
                self.scarf[i] = prev + d.normalize() * 7.0;
            }
        }
    }

    fn play(&mut self, inp: &Input, dt: f32, snd: &Sounds) {
        self.time += dt;
        self.squash = (self.squash - dt * 4.0).max(0.0);
        if inp.x.abs() > 0.2 {
            self.facing = inp.x.signum();
        }
        self.target = if self.tether.is_none() { self.pick_target(inp) } else { None };

        // Tether on: B (held, so a newcomer can keep it down and it catches
        // the moment a ring comes into reach).
        if self.tether.is_none() && inp.tether_held {
            if let Some(i) = self.target {
                let len = (self.level.anchors[i].pos(self.clock) - self.pos).length().clamp(ROPE_MIN, ROPE_MAX);
                self.tether = Some((i, len));
                self.rope_goal = len.min(ROPE_CATCH);
                self.tether_anim = 0.0;
                play_sound(&snd.tether, PlaySoundParams { looped: false, volume: 0.5 });
                play_sound(&snd.clip, PlaySoundParams { looped: false, volume: 0.5 });
                wc::rumble(0, 0.15, 0.1, 60);
            }
        }
        if let Some((_, _)) = self.tether {
            if !inp.tether_held || inp.jump {
                let fast = self.vel.length() > 380.0;
                self.tether = None;
                if inp.jump {
                    self.vel.y = self.vel.y.min(-JUMP * 0.75);
                    play_sound(&snd.jump, PlaySoundParams { looped: false, volume: 0.5 });
                } else if self.vel.y < 0.0 {
                    // Letting go on the way up gives a little lift, so a
                    // well-timed release always clears the gap it was aimed at.
                    self.vel.y -= 90.0;
                }
                if fast {
                    play_sound(&snd.whoosh, PlaySoundParams { looped: false, volume: 0.6 });
                }
            }
        }
        self.tether_anim = (self.tether_anim + dt * 10.0).min(1.0);

        if let Some((i, len)) = self.tether {
            // Swinging: gravity, a push along the swing, reel in and out.
            let a = self.level.anchors[i].pos(self.clock);
            let mut len = len;
            if inp.y < -0.4 {
                self.rope_goal = (self.rope_goal.min(len) - 220.0 * dt).max(ROPE_MIN);
            } else if inp.y > 0.4 {
                self.rope_goal = (self.rope_goal.max(len) + 200.0 * dt).min(ROPE_MAX);
            }
            if len > self.rope_goal {
                let before = len;
                len = (len - 520.0 * dt).max(self.rope_goal);
                // Pulling in keeps angular momentum, so the swing speeds up.
                let r = (self.pos - a).normalize_or_zero();
                let radial = r * self.vel.dot(r);
                let tangential = (self.vel - radial) * (before / len);
                self.vel = radial + tangential;
            } else {
                len = (len + 200.0 * dt).min(self.rope_goal);
            }
            self.tether = Some((i, len));
            self.vel.y += GRAVITY * dt;
            let r = self.pos - a;
            let tangent = vec2(-r.y, r.x).normalize_or_zero();
            let push = if tangent.x * inp.x > 0.0 { 1.0 } else { -1.0 };
            if inp.x.abs() > 0.2 {
                self.vel += tangent * push * 480.0 * inp.x.abs() * dt;
            }
            self.vel = self.vel.clamp_length_max(640.0);
            let (bx, by) = self.move_and_collide(self.vel * dt);
            if bx {
                self.vel.x *= -0.3;
            }
            if by {
                self.vel.y = 0.0;
            }
            // The rope: never longer than len; outward speed is removed.
            let r = self.pos - a;
            let d = r.length();
            if d > len {
                let n = r / d;
                let target = a + n * len;
                let back = target - self.pos;
                self.move_and_collide(back);
                let out = self.vel.dot(n);
                if out > 0.0 {
                    // The line snaps taut: most of the fall turns into swing
                    // instead of being lost, which is what makes a catch feel good.
                    let speed = self.vel.length();
                    let tangential = self.vel - n * out;
                    let dir = if tangential.length() > 1.0 { tangential.normalize() } else { vec2(-n.y, n.x) * self.facing };
                    self.vel = dir * (tangential.length() + (speed - tangential.length()) * 0.85);
                }
            }
            self.grounded = false;
        } else {
            // Running and jumping.
            let accel = if self.grounded { 2400.0 } else { 1300.0 };
            let want = inp.x * RUN;
            let dv = want - self.vel.x;
            // In the air, keep momentum from a swing unless the stick pushes against it.
            if self.grounded || inp.x.abs() > 0.2 && (self.vel.x.abs() < RUN || self.vel.x.signum() != inp.x.signum()) {
                self.vel.x += dv.clamp(-accel * dt, accel * dt);
            } else if !self.grounded {
                self.vel.x *= 1.0 - 0.3 * dt;
            }
            if self.grounded && inp.x.abs() < 0.2 {
                self.vel.x *= 1.0 - (14.0 * dt).min(1.0);
            }
            self.vel.y += GRAVITY * dt;
            // A short tap jumps lower than a held one.
            if self.vel.y < 0.0 && !inp.jump_held {
                self.vel.y += GRAVITY * 1.3 * dt;
            }
            self.vel.y = self.vel.y.min(900.0);
            if inp.jump {
                self.jump_buf = 0.12;
            }
            self.jump_buf -= dt;
            self.coyote -= dt;
            if self.jump_buf > 0.0 && (self.grounded || self.coyote > 0.0) {
                self.vel.y = -JUMP;
                self.jump_buf = 0.0;
                self.coyote = 0.0;
                self.grounded = false;
                play_sound(&snd.jump, PlaySoundParams { looped: false, volume: 0.45 });
                let p = self.pos + vec2(0.0, HALF_H);
                self.puff(p, Color::from_rgba(240, 240, 220, 200), 6, 90.0);
            }
            let was = self.grounded;
            let (_, by) = self.move_and_collide(self.vel * dt);
            if by {
                if self.vel.y > 0.0 {
                    if !was && self.vel.y > 500.0 {
                        self.squash = 1.0;
                        let p = self.pos + vec2(0.0, HALF_H);
                        self.puff(p, Color::from_rgba(240, 240, 220, 200), 8, 110.0);
                    }
                    self.grounded = true;
                }
                self.vel.y = 0.0;
            } else {
                // Still on the ground if there is ground just below.
                self.grounded = self.hits(self.pos + vec2(0.0, 1.0)) && self.vel.y >= 0.0;
            }
            if self.grounded {
                self.coyote = 0.1;
            }
        }

        // Crumbling ledges: standing on one starts it shaking; it drops, then grows back.
        if self.grounded {
            let r = ((self.pos.y + HALF_H + 1.0) / TILE).floor() as i32;
            for c in [((self.pos.x - HALF_W + 1.0) / TILE).floor() as i32, ((self.pos.x + HALF_W - 1.0) / TILE).floor() as i32] {
                if tile(&self.tiles, self.cols(), c, r) == CRUMBLE {
                    let idx = r as usize * self.cols() + c as usize;
                    if !self.crumble.iter().any(|(i, _)| *i == idx) {
                        self.crumble.push((idx, 0.5));
                    }
                }
            }
        }
        let cols = self.cols();
        let mut fell = Vec::new();
        let mut back = Vec::new();
        for (idx, t) in self.crumble.iter_mut() {
            let before = *t;
            *t -= dt;
            if before > 0.0 && *t <= 0.0 {
                fell.push(*idx);
            }
            if *t < -3.0 {
                back.push(*idx);
            }
        }
        for idx in fell {
            self.tiles[idx] = EMPTY;
            let p = vec2((idx % cols) as f32 * TILE + TILE / 2.0, (idx / cols) as f32 * TILE + TILE / 2.0);
            self.puff(p, Color::from_rgba(170, 130, 90, 255), 10, 80.0);
        }
        for idx in back {
            // A ledge grows back only where the climber is not standing in it.
            self.tiles[idx] = self.level.tiles[idx];
            if self.hits(self.pos) {
                self.tiles[idx] = EMPTY;
            } else {
                self.crumble.retain(|(i, _)| *i != idx);
            }
        }

        // Pickups.
        for i in 0..self.level.sparks.len() {
            if !self.sparks_got[i] && self.level.sparks[i].distance(self.pos) < 22.0 {
                self.sparks_got[i] = true;
                self.score += 10;
                play_sound(&snd.spark, PlaySoundParams { looped: false, volume: 0.35 });
                let p = self.level.sparks[i];
                self.puff(p, Color::from_rgba(255, 240, 150, 255), 5, 80.0);
            }
        }
        for i in 0..self.level.shards.len() {
            if !self.shards_got[i] && self.level.shards[i].distance(self.pos) < 26.0 {
                self.shards_got[i] = true;
                self.score += 500;
                self.total_shards += 1;
                play_sound(&snd.shard, PlaySoundParams { looped: false, volume: 0.6 });
                wc::rumble(0, 0.2, 0.3, 120);
                let p = self.level.shards[i];
                self.puff(p, Color::from_rgba(255, 200, 80, 255), 20, 160.0);
            }
        }
        for i in 0..self.level.checkpoints.len() {
            let c = self.level.checkpoints[i];
            if !self.checks_on[i] && (c.x - self.pos.x).abs() < 20.0 && (c.y - self.pos.y).abs() < 40.0 {
                self.checks_on[i] = true;
                self.checkpoint = c;
                play_sound(&snd.checkpoint, PlaySoundParams { looped: false, volume: 0.5 });
                self.puff(c - vec2(0.0, 30.0), Color::from_rgba(130, 255, 180, 255), 14, 110.0);
            }
        }

        // Falling off the island or onto spikes.
        if self.pos.y > ROWS as f32 * TILE + 40.0 || self.touching_spikes() {
            self.lives -= 1;
            self.deaths += 1;
            self.tether = None;
            play_sound(&snd.fall, PlaySoundParams { looped: false, volume: 0.6 });
            wc::rumble(0, 0.7, 0.5, 300);
            let p = self.pos;
            self.puff(p, Color::from_rgba(255, 230, 230, 255), 16, 160.0);
            self.vel = vec2(0.0, -300.0);
            self.set_mode(Mode::Dying);
            return;
        }

        // The flag.
        if self.pos.distance(self.level.exit) < 34.0 {
            let shards = self.shards_got.iter().filter(|b| **b).count() as i32;
            let time_bonus = ((90.0 - self.time).max(0.0) * 20.0) as i32;
            self.score += 1000 + time_bonus;
            self.tally = (1000, time_bonus, shards);
            play_sound(&snd.flag, PlaySoundParams { looped: false, volume: 0.7 });
            let p = self.level.exit;
            self.puff(p, Color::from_rgba(255, 220, 120, 255), 30, 200.0);
            self.set_mode(Mode::Cleared);
        }

        // Camera: follow with a little lead in the direction of travel.
        let lead = (self.vel.x * 0.35).clamp(-160.0, 160.0);
        let want = (self.pos.x - W * 0.4 + lead).clamp(0.0, self.level.cols as f32 * TILE - W);
        self.cam += (want - self.cam) * (5.0 * dt).min(1.0);
    }

    // ── read-outs for the debug table (a bot plays by these) ──

    fn foot_row(&self) -> i32 {
        ((self.pos.y + HALF_H + 1.0) / TILE).floor() as i32
    }

    /// Pixels to the next column ahead with no safe ground under the feet, or 999.
    fn gap_ahead(&self) -> f32 {
        let c0 = (self.pos.x / TILE).floor() as i32;
        let fr = self.foot_row();
        for k in 1..8 {
            let c = c0 + k * self.facing as i32;
            // Safe footing: solid ground within three rows below the feet,
            // with no spikes before it.
            let mut safe = false;
            for r in fr..(fr + 4).min(ROWS as i32) {
                let t = tile(&self.tiles, self.cols(), c, r);
                if t == SPIKES {
                    break;
                }
                if solid(t) {
                    safe = true;
                    break;
                }
            }
            if !safe {
                return (c as f32 * TILE + TILE / 2.0 - self.pos.x).abs() - TILE / 2.0;
            }
        }
        999.0
    }

    /// Pixels to spikes at foot level ahead, or 999.
    fn spikes_ahead(&self) -> f32 {
        let c0 = (self.pos.x / TILE).floor() as i32;
        let fr = self.foot_row();
        for k in 0..6 {
            let c = c0 + k * self.facing as i32;
            if (fr - 1..=fr).any(|r| tile(&self.tiles, self.cols(), c, r) == SPIKES) {
                return ((c as f32 * TILE + TILE / 2.0 - self.pos.x).abs() - TILE / 2.0).max(0.0);
            }
        }
        999.0
    }

    /// Pixels to a wall at body height ahead, or 999.
    fn wall_ahead(&self) -> f32 {
        let c0 = (self.pos.x / TILE).floor() as i32;
        let r = ((self.pos.y) / TILE).floor() as i32;
        for k in 1..5 {
            let c = c0 + k * self.facing as i32;
            if solid(tile(&self.tiles, self.cols(), c, r)) {
                return (c as f32 * TILE + TILE / 2.0 - self.pos.x).abs() - TILE / 2.0;
            }
        }
        999.0
    }
}

// ─── drawing ────────────────────────────────────────────────────────────

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}

fn with_a(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

fn draw_sky(g: &Game) {
    let (top, horizon) = g.level.sky;
    let bands = 24;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        draw_rectangle(0.0, i as f32 * H / bands as f32, W, H / bands as f32 + 1.0, mix(top, horizon, t.powf(1.4)));
    }
    // Sun, low and soft.
    let sun = vec2(W * 0.72 - g.cam * 0.02, 150.0);
    for (r, a) in [(120.0, 0.06), (80.0, 0.1), (46.0, 0.25)] {
        draw_circle(sun.x, sun.y, r, Color::new(1.0, 0.95, 0.8, a));
    }
    draw_circle(sun.x, sun.y, 30.0, Color::new(1.0, 0.97, 0.88, 0.9));
    // Three layers of hills, each scrolling slower than the ground.
    for (k, (par, base, amp, col)) in [
        (0.15, 330.0, 50.0, mix(horizon, top, 0.45)),
        (0.3, 380.0, 40.0, mix(horizon, Color::from_rgba(60, 90, 110, 255), 0.55)),
        (0.5, 430.0, 30.0, mix(horizon, Color::from_rgba(40, 70, 70, 255), 0.75)),
    ]
    .iter()
    .enumerate()
    {
        let off = g.cam * par;
        let step = 24.0;
        let mut x = -((off % step) + step);
        while x < W + step {
            let wx = x + off;
            let h = |wx: f32| base - amp * (0.6 * (wx * 0.006 + k as f32).sin() + 0.4 * (wx * 0.017 + k as f32 * 2.0).sin());
            let (y0, y1) = (h(wx), h(wx + step));
            draw_triangle(vec2(x, y0), vec2(x + step, y1), vec2(x, H), *col);
            draw_triangle(vec2(x + step, y1), vec2(x + step, H), vec2(x, H), *col);
            x += step;
        }
    }
    // Drifting clouds.
    for i in 0..6 {
        let x = ((i as f32 * 211.0 + g.clock * (8.0 + i as f32 * 2.0) - g.cam * 0.1) % (W + 300.0)) - 150.0;
        let y = 60.0 + (i * 37 % 120) as f32;
        let c = Color::new(1.0, 1.0, 1.0, 0.35);
        draw_circle(x, y, 26.0, c);
        draw_circle(x + 26.0, y + 6.0, 20.0, c);
        draw_circle(x - 24.0, y + 8.0, 18.0, c);
    }
}

fn draw_world(g: &Game) {
    let cols = g.cols();
    let c0 = (g.cam / TILE).floor().max(0.0) as usize;
    let c1 = ((g.cam + W) / TILE).ceil().min(cols as f32) as usize;
    let dirt = Color::from_rgba(120, 84, 60, 255);
    let dirt_dark = Color::from_rgba(92, 62, 46, 255);
    let grass = Color::from_rgba(110, 196, 90, 255);
    let grass_hi = Color::from_rgba(160, 226, 120, 255);
    for r in 0..ROWS {
        for c in c0..c1 {
            let t = g.tiles[r * cols + c];
            let x = c as f32 * TILE - g.cam;
            let y = r as f32 * TILE;
            match t {
                SOLID => {
                    let top_open = r == 0 || !solid(g.tiles[(r - 1) * cols + c]);
                    draw_rectangle(x, y, TILE + 0.5, TILE + 0.5, if (c + r) % 2 == 0 { dirt } else { dirt_dark });
                    if (c * 7 + r * 3) % 5 == 0 {
                        draw_circle(x + 9.0, y + 18.0, 3.0, Color::from_rgba(150, 110, 80, 255));
                    }
                    if top_open {
                        draw_rectangle(x, y, TILE + 0.5, 9.0, grass);
                        draw_rectangle(x, y, TILE + 0.5, 3.0, grass_hi);
                        for k in 0..3 {
                            let gx = x + 5.0 + k as f32 * 10.0;
                            draw_triangle(vec2(gx - 2.0, y), vec2(gx + 2.0, y), vec2(gx, y - 5.0), grass);
                        }
                    }
                }
                CRUMBLE => {
                    let shake = g
                        .crumble
                        .iter()
                        .find(|(i, _)| *i == r * cols + c)
                        .map_or(0.0, |(_, t)| if *t > 0.0 { (g.clock * 50.0).sin() * 1.5 } else { 0.0 });
                    let col = Color::from_rgba(196, 160, 110, 255);
                    draw_rectangle(x + 1.0 + shake, y + 1.0, TILE - 2.0, TILE * 0.6, col);
                    draw_line(x + 8.0 + shake, y + 2.0, x + 14.0 + shake, y + 14.0, 2.0, Color::from_rgba(120, 90, 60, 255));
                    draw_line(x + 20.0 + shake, y + 4.0, x + 16.0 + shake, y + 15.0, 2.0, Color::from_rgba(120, 90, 60, 255));
                }
                SPIKES => {
                    for k in 0..3 {
                        let sx = x + k as f32 * 10.0;
                        draw_triangle(vec2(sx, y + TILE), vec2(sx + 10.0, y + TILE), vec2(sx + 5.0, y + 8.0), Color::from_rgba(220, 225, 235, 255));
                        draw_triangle(vec2(sx + 5.0, y + TILE), vec2(sx + 10.0, y + TILE), vec2(sx + 5.0, y + 8.0), Color::from_rgba(150, 155, 175, 255));
                    }
                }
                _ => {}
            }
        }
    }
    // Signs.
    for (p, text) in &g.level.signs {
        let x = p.x - g.cam;
        if x < -300.0 || x > W + 300.0 {
            continue;
        }
        draw_rectangle(x - 2.0, p.y - 8.0, 4.0, 23.0, Color::from_rgba(110, 76, 50, 255));
        draw_rectangle(x - 13.0, p.y - 18.0, 26.0, 14.0, Color::from_rgba(200, 160, 110, 255));
        draw_text("!", x - 3.0, p.y - 6.0, 18.0, Color::from_rgba(90, 60, 40, 255));
        // The message fades in as you walk up to it.
        let d = (p.x - g.pos.x).abs();
        let a = (1.0 - (d - 120.0) / 120.0).clamp(0.0, 1.0);
        if a > 0.0 && g.mode != Mode::Title {
            let size = 24.0;
            let m = measure_text(text, None, size as u16, 1.0);
            let bx = (x - m.width / 2.0).clamp(10.0, W - m.width - 10.0);
            let by = 92.0;
            draw_rectangle(bx - 12.0, by - 26.0, m.width + 24.0, 38.0, Color::new(0.05, 0.06, 0.12, 0.75 * a));
            draw_text(text, bx, by, size, Color::new(1.0, 0.98, 0.9, a));
        }
    }
    // Checkpoints: a little lantern that lights up.
    for (i, p) in g.level.checkpoints.iter().enumerate() {
        let x = p.x - g.cam;
        draw_rectangle(x - 2.0, p.y - 26.0, 4.0, 41.0, Color::from_rgba(80, 70, 70, 255));
        let on = g.checks_on[i];
        let c = if on { Color::from_rgba(140, 255, 180, 255) } else { Color::from_rgba(120, 120, 130, 255) };
        if on {
            draw_circle(x, p.y - 30.0, 18.0, with_a(c, 0.18));
        }
        draw_circle(x, p.y - 30.0, 7.0, c);
    }
    // Sparks and shards.
    for (i, p) in g.level.sparks.iter().enumerate() {
        if g.sparks_got[i] {
            continue;
        }
        let x = p.x - g.cam;
        let bob = (g.clock * 3.0 + p.x * 0.05).sin() * 2.0;
        draw_circle(x, p.y + bob, 7.0, Color::new(1.0, 0.9, 0.5, 0.25));
        draw_poly(x, p.y + bob, 4, 5.0, 45.0, Color::from_rgba(255, 235, 140, 255));
    }
    for (i, p) in g.level.shards.iter().enumerate() {
        if g.shards_got[i] {
            continue;
        }
        let x = p.x - g.cam;
        let bob = (g.clock * 2.0 + i as f32).sin() * 4.0;
        for (r, a) in [(26.0, 0.08), (18.0, 0.14)] {
            draw_circle(x, p.y + bob, r, Color::new(1.0, 0.75, 0.3, a));
        }
        draw_poly(x, p.y + bob, 6, 12.0, g.clock * 60.0, Color::from_rgba(255, 190, 70, 255));
        draw_poly(x, p.y + bob, 6, 6.0, -g.clock * 60.0, Color::from_rgba(255, 245, 200, 255));
    }
    // The flag.
    {
        let p = g.level.exit;
        let x = p.x - g.cam;
        draw_rectangle(x - 2.0, p.y - 60.0, 4.0, 75.0, Color::from_rgba(230, 230, 240, 255));
        let wave = (g.clock * 5.0).sin() * 4.0;
        draw_triangle(vec2(x + 2.0, p.y - 60.0), vec2(x + 2.0, p.y - 34.0), vec2(x + 40.0, p.y - 47.0 + wave), Color::from_rgba(255, 110, 90, 255));
        draw_circle(x, p.y - 62.0, 5.0, Color::from_rgba(255, 220, 120, 255));
    }
    // Rings.
    for (i, a) in g.level.anchors.iter().enumerate() {
        let p = a.pos(g.clock);
        let x = p.x - g.cam;
        if a.sway > 0.0 {
            // A faint track shows where a swaying ring travels.
            draw_line(a.base.x - a.sway - g.cam, a.base.y, a.base.x + a.sway - g.cam, a.base.y, 2.0, Color::new(1.0, 1.0, 1.0, 0.2));
        }
        let pulse = 0.5 + 0.5 * (g.clock * 3.0 + i as f32).sin();
        draw_circle(x, p.y, 16.0, Color::new(0.5, 0.9, 1.0, 0.12 + 0.08 * pulse));
        draw_circle_lines(x, p.y, 9.0, 4.0, Color::from_rgba(120, 220, 255, 255));
        draw_circle_lines(x, p.y, 9.0, 1.5, Color::from_rgba(230, 250, 255, 255));
    }
    // The marked ring: a turning reticle, the cue for B.
    if let Some(i) = g.target {
        let p = g.level.anchors[i].pos(g.clock);
        let x = p.x - g.cam;
        let r = 20.0 + (g.clock * 6.0).sin() * 2.0;
        for k in 0..4 {
            let a = g.clock * 2.0 + k as f32 * std::f32::consts::FRAC_PI_2;
            let d = vec2(a.cos(), a.sin());
            let q = vec2(x, p.y) + d * r;
            draw_line(q.x, q.y, q.x + d.x * 8.0, q.y + d.y * 8.0, 3.0, Color::from_rgba(255, 240, 140, 255));
        }
        if g.mode == Mode::Play && g.tether.is_none() {
            draw_text("B", x + 18.0, p.y - 16.0, 22.0, Color::from_rgba(255, 240, 140, 255));
        }
    }
}

fn draw_player(g: &Game) {
    let p = g.pos - vec2(g.cam, 0.0);
    // The tether.
    if let Some((i, _)) = g.tether {
        let a = g.level.anchors[i].pos(g.clock) - vec2(g.cam, 0.0);
        let end = p + (a - p) * g.tether_anim;
        draw_line(p.x, p.y - 4.0, end.x, end.y, 3.0, Color::from_rgba(60, 40, 40, 255));
        draw_line(p.x, p.y - 4.0, end.x, end.y, 1.5, Color::from_rgba(250, 230, 190, 255));
    }
    let alpha = if g.mode == Mode::Dying { (1.0 - g.mode_t).max(0.0) } else { 1.0 };
    // Scarf.
    for i in 1..g.scarf.len() {
        let a = g.scarf[i - 1] - vec2(g.cam, 0.0);
        let b = g.scarf[i] - vec2(g.cam, 0.0);
        draw_line(a.x, a.y, b.x, b.y, 5.0 - i as f32 * 0.5, Color::new(0.95, 0.35, 0.35, alpha));
    }
    let sq = g.squash;
    let (sx, sy) = (1.0 + 0.25 * sq, 1.0 - 0.25 * sq);
    let body = Color::new(0.98, 0.84, 0.42, alpha);
    draw_ellipse(p.x, p.y + 2.0 * sq, HALF_W * 1.15 * sx, HALF_H * sy, 0.0, body);
    draw_ellipse_lines(p.x, p.y + 2.0 * sq, HALF_W * 1.15 * sx, HALF_H * sy, 0.0, 2.0, Color::new(0.45, 0.3, 0.15, alpha));
    // Goggles look the way you face.
    let ex = p.x + g.facing * 4.0;
    draw_circle(ex, p.y - 4.0, 4.5, Color::new(0.25, 0.3, 0.4, alpha));
    draw_circle(ex + g.facing * 1.0, p.y - 5.0, 2.0, Color::new(1.0, 1.0, 1.0, alpha));
    // Feet.
    let step = if g.grounded && g.vel.x.abs() > 30.0 { (g.clock * 18.0).sin() * 3.0 } else { 0.0 };
    draw_rectangle(p.x - 8.0 + step, p.y + HALF_H - 3.0, 6.0, 4.0, Color::new(0.35, 0.22, 0.12, alpha));
    draw_rectangle(p.x + 2.0 - step, p.y + HALF_H - 3.0, 6.0, 4.0, Color::new(0.35, 0.22, 0.12, alpha));
}

fn centered(s: &str, y: f32, size: f32, c: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, (W - d.width) / 2.0, y, size, c);
}

fn shadowed(s: &str, y: f32, size: f32, c: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, (W - d.width) / 2.0 + 3.0, y + 3.0, size, Color::new(0.0, 0.0, 0.0, 0.35 * c.a));
    draw_text(s, (W - d.width) / 2.0, y, size, c);
}

fn draw_hud(g: &Game) {
    draw_rectangle(0.0, 0.0, W, 40.0, Color::new(0.03, 0.04, 0.1, 0.55));
    draw_text(g.level.name, 16.0, 28.0, 28.0, WHITE);
    draw_text(&format!("{:06}", g.score), 240.0, 28.0, 28.0, Color::from_rgba(255, 235, 160, 255));
    for (i, got) in g.shards_got.iter().enumerate() {
        let x = 420.0 + i as f32 * 28.0;
        if *got {
            draw_poly(x, 20.0, 6, 10.0, 0.0, Color::from_rgba(255, 190, 70, 255));
        } else {
            draw_poly_lines(x, 20.0, 6, 10.0, 0.0, 2.0, Color::new(1.0, 0.8, 0.4, 0.6));
        }
    }
    let t = g.time as i32;
    draw_text(&format!("{}:{:02}", t / 60, t % 60), 540.0, 28.0, 28.0, WHITE);
    for i in 0..g.lives.max(0) {
        let x = W - 24.0 - i as f32 * 26.0;
        draw_circle(x, 20.0, 9.0, Color::from_rgba(250, 214, 107, 255));
        draw_circle(x + 3.0, 18.0, 3.0, Color::from_rgba(64, 77, 102, 255));
    }
}

fn draw(g: &Game) {
    draw_sky(g);
    draw_world(g);
    if g.mode != Mode::Title {
        draw_player(g);
    }
    for p in &g.puffs {
        let a = (p.life / p.max).clamp(0.0, 1.0);
        draw_circle(p.pos.x - g.cam, p.pos.y, p.size, with_a(p.color, a * p.color.a));
    }
    let soft = 0.6 + 0.4 * (g.clock * 2.4).sin();
    match g.mode {
        Mode::Title => {
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.05, 0.05, 0.12, 0.35));
            shadowed("TETHERLINE", 170.0, 104.0, Color::from_rgba(255, 245, 220, 255));
            centered("swing across five islands", 214.0, 30.0, Color::from_rgba(255, 240, 210, 255));
            let lines = [
                ("STICK / D-PAD", "run, and aim the tether"),
                ("A", "jump"),
                ("HOLD B", "tether to the marked ring"),
                ("LET GO OF B", "fly off the swing"),
                ("UP / DOWN", "reel the tether in and out"),
                ("START", "pause"),
            ];
            draw_rectangle(150.0, 248.0, W - 300.0, 198.0, Color::new(0.03, 0.04, 0.1, 0.55));
            for (i, (k, v)) in lines.iter().enumerate() {
                let y = 280.0 + i as f32 * 30.0;
                let d = measure_text(k, None, 26, 1.0);
                draw_text(k, W / 2.0 - 110.0 - d.width, y, 26.0, Color::from_rgba(255, 220, 120, 255));
                draw_text(v, W / 2.0 - 86.0, y, 26.0, WHITE);
            }
            shadowed("press A to start", 486.0, 36.0, Color::new(1.0, 1.0, 1.0, soft));
            if g.best > 0 {
                centered(&format!("best {}", g.best), 520.0, 24.0, WHITE);
            }
        }
        _ => {
            draw_hud(g);
        }
    }
    match g.mode {
        Mode::Paused => {
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.0, 0.0, 0.05, 0.55));
            shadowed("PAUSED", 250.0, 72.0, WHITE);
            centered("press Start to carry on", 300.0, 30.0, Color::new(1.0, 1.0, 1.0, soft));
        }
        Mode::Dying => {
            // The view dims gently and comes back: no flash.
            let k = g.mode_t;
            let a = if k < 0.6 { k / 0.6 } else { 1.0 - (k - 0.6) / 0.4 };
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.02, 0.02, 0.06, 0.45 * a.clamp(0.0, 1.0)));
        }
        Mode::Cleared => {
            let a = (g.mode_t * 2.0).min(1.0);
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.03, 0.04, 0.1, 0.55 * a));
            shadowed("ISLAND CLEARED", 190.0, 72.0, Color::new(1.0, 0.9, 0.6, a));
            let (base, time_bonus, shards) = g.tally;
            centered(&format!("flag {}    time bonus {}", base, time_bonus), 250.0, 30.0, Color::new(1.0, 1.0, 1.0, a));
            centered(&format!("sun shards {} / {}", shards, g.level.shards.len()), 290.0, 30.0, Color::new(1.0, 0.85, 0.5, a));
            centered(&format!("score {}", g.score), 330.0, 34.0, Color::new(1.0, 1.0, 1.0, a));
            if g.mode_t > 1.0 {
                let next = if g.level_i + 1 >= levels::COUNT { "press A to finish" } else { "press A for the next island" };
                centered(next, 410.0, 32.0, Color::new(1.0, 1.0, 1.0, soft));
            }
        }
        Mode::Over => {
            let a = (g.mode_t * 1.5).min(1.0);
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.03, 0.03, 0.08, 0.6 * a));
            shadowed("OUT OF TRIES", 220.0, 72.0, Color::new(1.0, 0.6, 0.55, a));
            centered(&format!("score {}    best {}", g.score, g.best), 280.0, 32.0, Color::new(1.0, 1.0, 1.0, a));
            if g.mode_t > 1.2 {
                centered("press A to try this island again", 380.0, 30.0, Color::new(1.0, 1.0, 1.0, soft));
            }
        }
        Mode::Won => {
            let a = (g.mode_t * 1.0).min(1.0);
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.03, 0.04, 0.1, 0.6 * a));
            shadowed("ALL FIVE ISLANDS", 200.0, 72.0, Color::new(1.0, 0.9, 0.6, a));
            centered(&format!("final score {}", g.score), 260.0, 36.0, Color::new(1.0, 1.0, 1.0, a));
            centered(&format!("sun shards {} / 15    falls {}", g.total_shards, g.deaths), 302.0, 30.0, Color::new(1.0, 0.85, 0.5, a));
            if g.mode_t > 1.5 {
                centered("press A to return to the title", 400.0, 30.0, Color::new(1.0, 1.0, 1.0, soft));
            }
        }
        _ => {}
    }
}

// ─── main ───────────────────────────────────────────────────────────────

#[macroquad::main(window_conf)]
async fn main() {
    wc::set_pad_keymap(&[]);
    let snd = Sounds {
        jump: load_sound("assets/jump.wav").await.unwrap(),
        tether: load_sound("assets/tether.wav").await.unwrap(),
        clip: load_sound("assets/clip.wav").await.unwrap(),
        whoosh: load_sound("assets/whoosh.wav").await.unwrap(),
        shard: load_sound("assets/shard.wav").await.unwrap(),
        spark: load_sound("assets/spark.wav").await.unwrap(),
        checkpoint: load_sound("assets/checkpoint.wav").await.unwrap(),
        fall: load_sound("assets/fall.wav").await.unwrap(),
        flag: load_sound("assets/flag.wav").await.unwrap(),
    };
    let music = load_sound("assets/music.wav").await.unwrap();
    play_sound(&music, PlaySoundParams { looped: true, volume: 0.45 });

    let mut pads = Pads { prev: 0 };
    let mut auto = Autopilot::default();
    let mut frame = 0u32;
    let mut g = Game::new();
    loop {
        let dt = get_frame_time().min(1.0 / 30.0);
        frame += 1;
        let inp = if wc::debug::input_i32("autopilot", 0) != 0 {
            let held = auto.buttons(&g, frame);
            pads.apply(held, Vec2::ZERO)
        } else {
            pads.read()
        };
        g.update(&inp, dt, &snd);

        wc::debug::set_i32("mode", g.mode as i32);
        wc::debug::set_i32("level", g.level_i as i32);
        wc::debug::set_i32("score", g.score);
        wc::debug::set_i32("lives", g.lives);
        wc::debug::set_i32("deaths", g.deaths);
        wc::debug::set_i32("shards", g.total_shards);
        wc::debug::set_f32("px", g.pos.x);
        wc::debug::set_f32("py", g.pos.y);
        wc::debug::set_f32("vx", g.vel.x);
        wc::debug::set_f32("vy", g.vel.y);
        wc::debug::set_i32("grounded", g.grounded as i32);
        wc::debug::set_i32("tethered", g.tether.is_some() as i32);
        let (tx, ty) = g.target.map_or((0.0, 0.0), |i| {
            let p = g.level.anchors[i].pos(g.clock) - g.pos;
            (p.x, p.y)
        });
        wc::debug::set_i32("has_target", g.target.is_some() as i32);
        wc::debug::set_f32("target_dx", tx);
        wc::debug::set_f32("target_dy", ty);
        let (ax, ay) = g.tether.map_or((0.0, 0.0), |(i, _)| {
            let p = g.level.anchors[i].pos(g.clock) - g.pos;
            (p.x, p.y)
        });
        wc::debug::set_f32("anchor_dx", ax);
        wc::debug::set_f32("anchor_dy", ay);
        wc::debug::set_f32("gap", g.gap_ahead());
        wc::debug::set_f32("spikes", g.spikes_ahead());
        wc::debug::set_f32("wall", g.wall_ahead());
        wc::debug::set_f32("exit_dx", g.level.exit.x - g.pos.x);
        // The next ring ahead (by x), wherever it is, for a bot planning a swing.
        let next = g
            .level
            .anchors
            .iter()
            .map(|a| a.pos(g.clock) - g.pos)
            .filter(|d| d.x > 10.0)
            .min_by(|a, b| a.x.total_cmp(&b.x));
        wc::debug::set_f32("ring_dx", next.map_or(9999.0, |d| d.x));
        wc::debug::set_f32("ring_dy", next.map_or(9999.0, |d| d.y));

        draw(&g);
        next_frame().await
    }
}
