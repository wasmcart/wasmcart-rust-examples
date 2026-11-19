//! Hexbloom: a twin-stick arena shooter in a glowing hex garden.
//!
//! Left stick (or d-pad) moves, the right stick aims and fires. Holding A
//! fires at the nearest enemy, so the game plays with a d-pad alone too.
//! B sets off a bloom bomb. Clear eight waves; waves 4 and 8 are guarded by
//! a great thorn flower. Every connected pad steers the one gardener, so an
//! idle second pad changes nothing. Keyboard fallback: WASD move, arrows aim
//! and fire, Space bomb, Enter start.

use macroquad::audio::{load_sound, play_sound, play_sound_once, stop_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;
use std::f32::consts::TAU;
use wasmcart_macroquad as wc;

const W: f32 = 960.0;
const H: f32 = 540.0;
/// The arena: a rounded field inside the screen, HUD above it.
const AX0: f32 = 24.0;
const AY0: f32 = 56.0;
const AX1: f32 = W - 24.0;
const AY1: f32 = H - 16.0;
const PLAYER_R: f32 = 11.0;
const PLAYER_SPEED: f32 = 290.0;
const FIRE_GAP: f32 = 0.105;
const BULLET_SPEED: f32 = 760.0;
const WAVES: usize = 8;
const START_LIVES: i32 = 3;

fn window_conf() -> Conf {
    Conf { window_title: "Hexbloom".to_owned(), window_width: W as i32, window_height: H as i32, ..Default::default() }
}

// ─── input ──────────────────────────────────────────────────────────────

/// One frame of input, every connected pad and the keyboard merged.
#[derive(Default)]
struct Input {
    mv: Vec2,
    aim: Vec2,
    auto_fire: bool,
    bomb: bool,
    start: bool,
    confirm: bool,
}

struct Pads {
    prev: u32,
}

impl Pads {
    fn read(&mut self) -> Input {
        let mut inp = Input::default();
        let mut held = 0u32;
        for i in 0..4 {
            let p = wc::pad(i);
            if p.connected == 0 {
                continue;
            }
            held |= p.buttons;
            let l = vec2(p.left_x as f32, p.left_y as f32) / 32767.0;
            if l.length() > 0.25 {
                inp.mv += l;
            }
            let r = vec2(p.right_x as f32, p.right_y as f32) / 32767.0;
            if r.length() > 0.35 {
                inp.aim += r;
            }
        }
        let pressed = held & !self.prev;
        self.prev = held;
        let d = |m: u32| if held & m != 0 { 1.0 } else { 0.0 };
        inp.mv += vec2(d(wc::WC_BTN_RIGHT) - d(wc::WC_BTN_LEFT), d(wc::WC_BTN_DOWN) - d(wc::WC_BTN_UP));
        let k = |c: KeyCode| if is_key_down(c) { 1.0 } else { 0.0 };
        inp.mv += vec2(k(KeyCode::D) - k(KeyCode::A), k(KeyCode::S) - k(KeyCode::W));
        inp.aim += vec2(k(KeyCode::Right) - k(KeyCode::Left), k(KeyCode::Down) - k(KeyCode::Up));
        if inp.mv.length() > 1.0 {
            inp.mv = inp.mv.normalize();
        }
        inp.auto_fire = held & wc::WC_BTN_A != 0;
        inp.bomb = pressed & (wc::WC_BTN_B | wc::WC_BTN_R) != 0 || is_key_pressed(KeyCode::Space);
        inp.start = pressed & wc::WC_BTN_START != 0 || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape);
        inp.confirm = pressed & (wc::WC_BTN_A | wc::WC_BTN_START) != 0 || is_key_pressed(KeyCode::Enter);
        inp
    }
}

// ─── autopilot ──────────────────────────────────────────────────────────

/// A test autopilot, switched on by a debug host writing `autopilot = 1`
/// (romdev `wasm({op:'write'})`, `CartHost.writeDebugValue`); a cart built
/// without the `debug` feature never has it on. It plays as the title screen
/// teaches: left stick away from the closest threat, right stick at the
/// nearest enemy, B when something is about to touch it.
fn autopilot(g: &Game, frame: u32) -> Input {
    let mut inp = Input::default();
    if g.mode != Mode::Play {
        inp.confirm = matches!(g.mode, Mode::Title | Mode::Over | Mode::Won) && frame % 30 == 0;
        return inp;
    }
    let threat = g
        .thorns
        .iter()
        .map(|t| t.pos)
        .chain(g.enemies.iter().map(|e| e.pos))
        .min_by(|a, b| a.distance_squared(g.pos).total_cmp(&b.distance_squared(g.pos)));
    let centre = vec2(W / 2.0, (AY0 + AY1) / 2.0);
    let mut mv = vec2((centre.x - g.pos.x) / 300.0, (centre.y - g.pos.y) / 200.0);
    if let Some(t) = threat {
        let d = t - g.pos;
        let l = d.length();
        if l < 150.0 {
            let away = -d / l.max(1.0);
            mv = away + vec2(-away.y, away.x) * 0.6;
        }
        inp.bomb = l < 40.0 && g.bombs > 0 && frame % 20 == 0;
    }
    inp.mv = mv.clamp_length_max(1.0);
    if let Some(e) = g.nearest_enemy() {
        inp.aim = (e - g.pos).normalize_or_zero();
    }
    inp
}

// ─── world ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Seed,
    Darter,
    Splitter,
    Sprout,
    Orbiter,
    Boss,
}

struct Enemy {
    kind: Kind,
    pos: Vec2,
    vel: Vec2,
    hp: f32,
    max_hp: f32,
    r: f32,
    t: f32,
    /// Per-kind timer: darter dash phase, orbiter and boss shot clocks.
    clock: f32,
    phase: f32,
    hit: f32,
}

impl Enemy {
    fn new(kind: Kind, pos: Vec2, wave: usize) -> Enemy {
        let (hp, r) = match kind {
            Kind::Seed => (if wave >= 5 { 2.0 } else { 1.0 }, 11.0),
            Kind::Darter => (1.0, 12.0),
            Kind::Splitter => (4.0, 19.0),
            Kind::Sprout => (1.0, 8.0),
            Kind::Orbiter => (3.0, 15.0),
            Kind::Boss => (if wave >= 8 { 190.0 } else { 120.0 }, 46.0),
        };
        Enemy {
            kind,
            pos,
            vel: Vec2::ZERO,
            hp,
            max_hp: hp,
            r,
            t: 0.0,
            clock: rand::gen_range(0.5, 1.5),
            phase: rand::gen_range(0.0, TAU),
            hit: 0.0,
        }
    }
    fn points(&self) -> i32 {
        match self.kind {
            Kind::Seed => 10,
            Kind::Sprout => 5,
            Kind::Darter => 25,
            Kind::Splitter => 40,
            Kind::Orbiter => 50,
            Kind::Boss => 2000,
        }
    }
    fn color(&self) -> Color {
        match self.kind {
            Kind::Seed | Kind::Sprout => Color::from_rgba(120, 235, 120, 255),
            Kind::Darter => Color::from_rgba(255, 90, 200, 255),
            Kind::Splitter => Color::from_rgba(255, 160, 60, 255),
            Kind::Orbiter => Color::from_rgba(90, 200, 255, 255),
            Kind::Boss => Color::from_rgba(255, 80, 110, 255),
        }
    }
}

struct Shot {
    pos: Vec2,
    vel: Vec2,
    life: f32,
}

struct Thorn {
    pos: Vec2,
    vel: Vec2,
    r: f32,
}

struct Pollen {
    pos: Vec2,
    vel: Vec2,
    life: f32,
}

struct Spark {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max: f32,
    color: Color,
    size: f32,
}

/// A spawn about to happen: a ring closes on the spot first, so nothing
/// appears on top of the player without warning.
struct Warn {
    kind: Kind,
    pos: Vec2,
    t: f32,
}

struct Ring {
    pos: Vec2,
    r: f32,
    max: f32,
    life: f32,
    color: Color,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Title,
    Play,
    Paused,
    Over,
    Won,
}

struct Sounds {
    shoot: Sound,
    pop: Sound,
    pickup: Sound,
    hurt: Sound,
    bomb: Sound,
    wave: Sound,
    bosshit: Sound,
    music: Sound,
    boss: Sound,
}

struct Game {
    mode: Mode,
    mode_t: f32,
    pos: Vec2,
    vel: Vec2,
    facing: Vec2,
    lives: i32,
    bombs: i32,
    invuln: f32,
    hurt_glow: f32,
    fire_cd: f32,
    score: i32,
    best: i32,
    mult: i32,
    pollen_count: i32,
    wave: usize,
    /// Enemies still to spawn this wave.
    queue: Vec<Kind>,
    spawn_cd: f32,
    banner: f32,
    banner_text: String,
    between: f32,
    enemies: Vec<Enemy>,
    shots: Vec<Shot>,
    thorns: Vec<Thorn>,
    pollen: Vec<Pollen>,
    sparks: Vec<Spark>,
    warns: Vec<Warn>,
    rings: Vec<Ring>,
    kills: i32,
    shot_sound_cd: f32,
    music_boss: bool,
    first_bloom: bool,
    /// The garden floor: each hex cell blooms where something was cleared.
    bloom: Vec<Cell>,
}

/// A floor cell: how open its bloom is, how far it is opening, its colour.
#[derive(Clone, Copy)]
struct Cell {
    v: f32,
    target: f32,
    c: Color,
}

const HEX: f32 = 30.0;
const HEX_ROWS: usize = ((AY1 - AY0) / (HEX * 1.5)) as usize + 2;
const HEX_COLS: usize = ((AX1 - AX0) / (HEX * 1.7320508)) as usize + 2;

fn hex_centre(row: usize, col: usize) -> Vec2 {
    let hw = HEX * 3f32.sqrt();
    vec2(AX0 + col as f32 * hw + if row % 2 == 1 { hw / 2.0 } else { 0.0 }, AY0 + row as f32 * HEX * 1.5)
}



fn wave_plan(wave: usize) -> Vec<Kind> {
    use Kind::*;
    let mut v = Vec::new();
    let mut add = |k: Kind, n: usize| (0..n).for_each(|_| v.push(k));
    match wave {
        1 => add(Seed, 16),
        2 => {
            add(Seed, 16);
            add(Darter, 6);
        }
        3 => {
            add(Seed, 14);
            add(Splitter, 5);
            add(Orbiter, 3);
        }
        4 => {
            add(Boss, 1);
        }
        5 => {
            add(Seed, 22);
            add(Darter, 10);
            add(Orbiter, 4);
        }
        6 => {
            add(Seed, 10);
            add(Splitter, 9);
            add(Darter, 10);
            add(Orbiter, 6);
        }
        7 => {
            add(Seed, 26);
            add(Darter, 12);
            add(Splitter, 6);
            add(Orbiter, 6);
        }
        _ => add(Boss, 1),
    }
    // Shuffle all but a boss, which comes first.
    let n = v.len();
    for i in (1..n).rev() {
        let j = rand::gen_range(0, i + 1);
        v.swap(i, j);
    }
    v.sort_by_key(|k| if *k == Boss { 0 } else { 1 });
    v.reverse(); // queue pops from the back
    v
}

fn in_arena(p: Vec2, r: f32) -> Vec2 {
    vec2(p.x.clamp(AX0 + r, AX1 - r), p.y.clamp(AY0 + r, AY1 - r))
}

impl Game {
    fn new() -> Game {
        Game {
            mode: Mode::Title,
            mode_t: 0.0,
            pos: vec2(W / 2.0, (AY0 + AY1) / 2.0),
            vel: Vec2::ZERO,
            facing: vec2(1.0, 0.0),
            lives: START_LIVES,
            bombs: 2,
            invuln: 0.0,
            hurt_glow: 0.0,
            fire_cd: 0.0,
            score: 0,
            best: 0,
            mult: 1,
            pollen_count: 0,
            wave: 0,
            queue: Vec::new(),
            spawn_cd: 0.0,
            banner: 0.0,
            banner_text: String::new(),
            between: 0.0,
            enemies: Vec::new(),
            shots: Vec::new(),
            thorns: Vec::new(),
            pollen: Vec::new(),
            sparks: Vec::new(),
            warns: Vec::new(),
            rings: Vec::new(),
            kills: 0,
            shot_sound_cd: 0.0,
            music_boss: false,
            first_bloom: false,
            bloom: vec![Cell { v: 0.0, target: 0.0, c: WHITE }; HEX_ROWS * HEX_COLS],
        }
    }

    fn set_mode(&mut self, m: Mode) {
        self.mode = m;
        self.mode_t = 0.0;
    }

    fn start(&mut self) {
        let best = self.best;
        *self = Game::new();
        self.best = best;
        self.set_mode(Mode::Play);
        self.next_wave();
    }

    fn next_wave(&mut self) {
        self.wave += 1;
        self.queue = wave_plan(self.wave);
        self.spawn_cd = 1.2;
        self.bombs = (self.bombs + 1).min(3).max(2);
        self.banner = 2.2;
        self.banner_text = if self.wave == 4 || self.wave >= 8 {
            format!("WAVE {}   THE THORN FLOWER", self.wave)
        } else {
            format!("WAVE {}", self.wave)
        };
    }

    fn burst(&mut self, at: Vec2, color: Color, n: usize, speed: f32) {
        for _ in 0..n {
            let a = rand::gen_range(0.0, TAU);
            let s = rand::gen_range(0.2, 1.0) * speed;
            let life = rand::gen_range(0.3, 0.8);
            self.sparks.push(Spark {
                pos: at,
                vel: vec2(a.cos(), a.sin()) * s,
                life,
                max: life,
                color,
                size: rand::gen_range(2.0, 4.5),
            });
        }
    }

    fn spawn_point(&self) -> Vec2 {
        // Away from the player: try a few spots and keep the farthest.
        let mut best = vec2(AX0 + 40.0, AY0 + 40.0);
        let mut best_d = 0.0;
        for _ in 0..6 {
            let p = vec2(rand::gen_range(AX0 + 40.0, AX1 - 40.0), rand::gen_range(AY0 + 40.0, AY1 - 40.0));
            let d = p.distance(self.pos);
            if d > best_d {
                best_d = d;
                best = p;
            }
        }
        best
    }

    fn nearest_enemy(&self) -> Option<Vec2> {
        self.enemies
            .iter()
            .min_by(|a, b| a.pos.distance_squared(self.pos).total_cmp(&b.pos.distance_squared(self.pos)))
            .map(|e| e.pos)
    }

    fn update(&mut self, inp: &Input, dt: f32, snd: &Sounds) {
        self.mode_t += dt;
        match self.mode {
            Mode::Title => {
                // The title garden blooms by itself, a cell at a time.
                if (self.mode_t * 1.6).floor() != ((self.mode_t - dt) * 1.6).floor() {
                    let p = vec2(rand::gen_range(AX0, AX1), rand::gen_range(AY0, AY1));
                    let c = [Color::from_rgba(120, 235, 120, 255), Color::from_rgba(255, 90, 200, 255), Color::from_rgba(255, 160, 60, 255), Color::from_rgba(90, 200, 255, 255)][rand::gen_range(0, 4)];
                    self.blossom(p, c, 70.0);
                }
                if inp.confirm && self.mode_t > 0.3 {
                    rand::srand((get_time() * 1000.0) as u64 ^ 0x5eed);
                    self.start();
                }
            }
            Mode::Over | Mode::Won => {
                if inp.confirm && self.mode_t > 1.2 {
                    self.start();
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
                    return;
                }
                self.play(inp, dt, snd);
            }
        }
        // Effects run in every mode so a game over still settles.
        if self.mode != Mode::Paused {
            for s in self.sparks.iter_mut() {
                s.pos += s.vel * dt;
                s.vel *= 1.0 - 2.5 * dt;
                s.life -= dt;
            }
            self.sparks.retain(|s| s.life > 0.0);
            for r in self.rings.iter_mut() {
                r.life -= dt;
                r.r += (r.max - r.r) * (8.0 * dt).min(1.0);
            }
            self.rings.retain(|r| r.life > 0.0);
            self.hurt_glow = (self.hurt_glow - dt * 1.4).max(0.0);
            // Cells open over about half a second (never a pop), then fade slowly.
            for cell in self.bloom.iter_mut() {
                if cell.target > cell.v {
                    cell.v = (cell.v + dt * 2.0).min(cell.target);
                } else {
                    cell.target = 0.0;
                    cell.v = (cell.v - dt * 0.025).max(0.0);
                }
            }
        }
    }

    fn play(&mut self, inp: &Input, dt: f32, snd: &Sounds) {
        // Player: a little inertia makes the stick feel weighty, not floaty.
        let target = inp.mv * PLAYER_SPEED;
        self.vel += (target - self.vel) * (14.0 * dt).min(1.0);
        self.pos = in_arena(self.pos + self.vel * dt, PLAYER_R);
        self.invuln = (self.invuln - dt).max(0.0);
        self.banner = (self.banner - dt).max(0.0);
        self.shot_sound_cd -= dt;

        // Aim: the right stick (or arrows), else A aims at the nearest enemy.
        let mut aim = Vec2::ZERO;
        if inp.aim.length() > 0.1 {
            aim = inp.aim.normalize();
        } else if inp.auto_fire {
            if let Some(e) = self.nearest_enemy() {
                aim = (e - self.pos).normalize_or_zero();
            } else if inp.mv.length() > 0.1 {
                aim = inp.mv.normalize();
            } else {
                aim = self.facing;
            }
        }
        self.fire_cd -= dt;
        if aim != Vec2::ZERO {
            self.facing = aim;
            if self.fire_cd <= 0.0 {
                self.fire_cd = FIRE_GAP;
                let side = vec2(-aim.y, aim.x) * 5.0;
                for s in [-1.0, 1.0] {
                    self.shots.push(Shot { pos: self.pos + aim * 14.0 + side * s, vel: aim * BULLET_SPEED, life: 0.9 });
                }
                if self.shot_sound_cd <= 0.0 {
                    play_sound(&snd.shoot, PlaySoundParams { looped: false, volume: 0.25 });
                    self.shot_sound_cd = 0.07;
                }
            }
        } else if inp.mv.length() > 0.1 {
            self.facing = inp.mv.normalize();
        }

        if inp.bomb && self.bombs > 0 {
            self.bombs -= 1;
            self.bomb(snd);
        }

        // Spawning.
        let alive_cap = 10 + self.wave * 3;
        self.spawn_cd -= dt;
        if self.spawn_cd <= 0.0 && !self.queue.is_empty() && self.enemies.len() + self.warns.len() < alive_cap {
            // Enemies arrive in small clusters around one spot.
            let centre = self.spawn_point();
            let group = rand::gen_range(2, 5);
            for n in 0..group {
                let Some(k) = self.queue.pop() else { break };
                if k == Kind::Boss {
                    self.warns.push(Warn { kind: k, pos: vec2(W / 2.0, AY0 + 120.0), t: 1.6 });
                    break;
                }
                let a = n as f32 * 2.1;
                let pos = in_arena(centre + vec2(a.cos(), a.sin()) * if n == 0 { 0.0 } else { 36.0 }, 20.0);
                self.warns.push(Warn { kind: k, pos, t: 0.9 + n as f32 * 0.08 });
            }
            self.spawn_cd = (1.5 - self.wave as f32 * 0.1).max(0.7);
        }
        let mut born = Vec::new();
        for w in self.warns.iter_mut() {
            w.t -= dt;
            if w.t <= 0.0 {
                born.push((w.kind, w.pos));
            }
        }
        self.warns.retain(|w| w.t > 0.0);
        for (k, p) in born {
            self.enemies.push(Enemy::new(k, p, self.wave));
            if k == Kind::Boss && !self.music_boss {
                stop_sound(&snd.music);
                play_sound(&snd.boss, PlaySoundParams { looped: true, volume: 0.5 });
                self.music_boss = true;
            }
        }

        self.update_enemies(dt);
        self.update_shots(dt, snd);
        self.update_thorns(dt);
        self.update_pollen(dt, snd);

        // Touching an enemy or a thorn hurts.
        if self.invuln <= 0.0 {
            let hit_enemy = self.enemies.iter().any(|e| e.pos.distance(self.pos) < e.r + PLAYER_R - 3.0);
            let hit_thorn = self.thorns.iter().any(|t| t.pos.distance(self.pos) < t.r + PLAYER_R - 4.0);
            if hit_enemy || hit_thorn {
                self.hurt(snd);
            }
        }

        // Wave cleared?
        if self.queue.is_empty() && self.warns.is_empty() && self.enemies.is_empty() {
            if self.between <= 0.0 {
                self.between = 2.0;
                play_sound_once(&snd.wave);
                self.thorns.clear();
                if self.music_boss {
                    stop_sound(&snd.boss);
                    play_sound(&snd.music, PlaySoundParams { looped: true, volume: 0.5 });
                    self.music_boss = false;
                }
                if self.wave == 4 {
                    self.first_bloom = true;
                }
                self.banner = 2.0;
                self.banner_text = if self.wave >= WAVES {
                    "THE GARDEN IS CLEAR".to_owned()
                } else if self.wave == 4 {
                    "THE FIRST THORN FLOWER FALLS".to_owned()
                } else {
                    format!("WAVE {} CLEAR", self.wave)
                };
            } else {
                self.between -= dt;
                if self.between <= 0.0 {
                    if self.wave >= WAVES {
                        self.score += 500 * self.lives;
                        self.best = self.best.max(self.score);
                        self.set_mode(Mode::Won);
                    } else {
                        self.next_wave();
                    }
                }
            }
        }
    }

    fn hurt(&mut self, snd: &Sounds) {
        self.lives -= 1;
        self.invuln = 2.2;
        self.hurt_glow = 1.0;
        self.mult = 1;
        self.pollen_count = 0;
        play_sound_once(&snd.hurt);
        wc::rumble(0, 0.9, 0.6, 380);
        let p = self.pos;
        self.burst(p, Color::from_rgba(255, 240, 200, 255), 30, 320.0);
        // Clear a breathing space: push enemies out and dissolve nearby thorns.
        for e in self.enemies.iter_mut() {
            let d = e.pos - p;
            if d.length() < 150.0 && e.kind != Kind::Boss {
                e.pos = in_arena(p + d.normalize_or(vec2(1.0, 0.0)) * 150.0, e.r);
            }
        }
        self.thorns.retain(|t| t.pos.distance(p) > 170.0);
        if self.lives <= 0 {
            self.best = self.best.max(self.score);
            self.set_mode(Mode::Over);
        }
    }

    fn bomb(&mut self, snd: &Sounds) {
        play_sound_once(&snd.bomb);
        wc::rumble(0, 0.5, 0.3, 250);
        let p = self.pos;
        self.rings.push(Ring { pos: p, r: 10.0, max: 270.0, life: 0.9, color: Color::from_rgba(255, 220, 120, 255) });
        self.thorns.retain(|t| t.pos.distance(p) > 280.0);
        let mut dead = Vec::new();
        for (i, e) in self.enemies.iter_mut().enumerate() {
            if e.pos.distance(p) < 270.0 + e.r {
                e.hp -= if e.kind == Kind::Boss { 12.0 } else { 99.0 };
                e.hit = 0.25;
                if e.hp <= 0.0 {
                    dead.push(i);
                }
            }
        }
        for i in dead.into_iter().rev() {
            self.kill(i, snd);
        }
    }

    fn blossom(&mut self, p: Vec2, c: Color, radius: f32) {
        for row in 0..HEX_ROWS {
            for col in 0..HEX_COLS {
                let d = hex_centre(row, col).distance(p);
                if d < radius {
                    let v = 1.0 - d / radius * 0.6;
                    let cell = &mut self.bloom[row * HEX_COLS + col];
                    if v > cell.target.max(cell.v) {
                        cell.target = v;
                        if cell.v < 0.05 {
                            cell.c = c;
                        }
                    }
                }
            }
        }
    }

    fn kill(&mut self, i: usize, snd: &Sounds) {
        let e = self.enemies.swap_remove(i);
        self.blossom(e.pos, e.color(), if e.kind == Kind::Boss { 400.0 } else { 34.0 + e.r * 2.0 });
        self.kills += 1;
        self.score += e.points() * self.mult;
        let c = e.color();
        let n = if e.kind == Kind::Boss { 140 } else { 16 };
        self.burst(e.pos, c, n, if e.kind == Kind::Boss { 520.0 } else { 260.0 });
        self.rings.push(Ring { pos: e.pos, r: e.r * 0.5, max: e.r * 2.6, life: 0.4, color: c });
        play_sound(&snd.pop, PlaySoundParams { looped: false, volume: if e.kind == Kind::Boss { 1.0 } else { 0.5 } });
        let drops = match e.kind {
            Kind::Boss => 30,
            Kind::Splitter | Kind::Orbiter => 3,
            Kind::Sprout => 0,
            _ => 1,
        };
        for _ in 0..drops {
            let a = rand::gen_range(0.0, TAU);
            self.pollen.push(Pollen { pos: e.pos, vel: vec2(a.cos(), a.sin()) * rand::gen_range(40.0, 160.0), life: 7.0 });
        }
        if e.kind == Kind::Splitter {
            for k in 0..3 {
                let a = k as f32 * TAU / 3.0 + rand::gen_range(0.0, 1.0);
                let mut s = Enemy::new(Kind::Sprout, e.pos + vec2(a.cos(), a.sin()) * 14.0, self.wave);
                s.vel = vec2(a.cos(), a.sin()) * 220.0;
                self.enemies.push(s);
            }
        }
        if e.kind == Kind::Boss {
            wc::rumble(0, 1.0, 0.8, 700);
            self.thorns.clear();
            // The boss's garden withers with it.
            let rest: Vec<Vec2> = self.enemies.iter().map(|e| e.pos).collect();
            for p in rest {
                self.burst(p, Color::from_rgba(120, 235, 120, 255), 8, 200.0);
            }
            self.score += self.enemies.len() as i32 * 10;
            self.enemies.clear();
            self.queue.clear();
        }
    }

    fn update_enemies(&mut self, dt: f32) {
        let player = self.pos;
        let wave = self.wave as f32;
        let mut new_thorns = Vec::new();
        let mut new_seeds = Vec::new();
        let n = self.enemies.len();
        for i in 0..n {
            let e = &mut self.enemies[i];
            e.t += dt;
            e.hit = (e.hit - dt).max(0.0);
            let to = player - e.pos;
            let dir = to.normalize_or_zero();
            match e.kind {
                Kind::Seed | Kind::Sprout => {
                    let speed = if e.kind == Kind::Seed { 100.0 + wave * 9.0 } else { 130.0 + wave * 7.0 };
                    let wob = vec2(-dir.y, dir.x) * (e.t * 3.0 + e.phase).sin() * 0.6;
                    let want = (dir + wob).normalize_or_zero() * speed;
                    e.vel += (want - e.vel) * (3.0 * dt).min(1.0);
                }
                Kind::Darter => {
                    // Drift, wind up (a visible crouch), then dash along a locked line.
                    e.clock -= dt;
                    if e.clock > 0.0 {
                        let want = dir * 55.0;
                        e.vel += (want - e.vel) * (3.0 * dt).min(1.0);
                    } else if e.clock > -0.45 {
                        e.vel *= 1.0 - (8.0 * dt).min(1.0);
                        if e.phase >= 0.0 {
                            e.phase = -1.0;
                        }
                    } else if e.clock > -0.95 {
                        if e.phase < 0.0 && e.phase > -2.0 {
                            e.vel = dir * (380.0 + wave * 12.0);
                            e.phase = -3.0;
                        }
                    } else {
                        e.clock = rand::gen_range(1.4, 2.4);
                        e.phase = 0.0;
                    }
                }
                Kind::Splitter => {
                    let want = dir * (50.0 + wave * 4.0);
                    e.vel += (want - e.vel) * (2.0 * dt).min(1.0);
                }
                Kind::Orbiter => {
                    // Circle the player at a distance and lob slow thorns.
                    let ang = (-to).y.atan2((-to).x) + dt * 0.7;
                    let want_pos = player + vec2(ang.cos(), ang.sin()) * 230.0;
                    let want = (want_pos - e.pos).clamp_length_max(1.0) * 140.0;
                    e.vel += ((want_pos - e.pos).normalize_or_zero() * 140.0 - e.vel) * (2.5 * dt).min(1.0);
                    let _ = want;
                    e.clock -= dt;
                    if e.clock <= 0.0 {
                        e.clock = (2.3 - wave * 0.12).max(1.3);
                        new_thorns.push(Thorn { pos: e.pos, vel: dir * 175.0, r: 6.0 });
                    }
                }
                Kind::Boss => {
                    // Drift slowly, spin, spray rings of thorns with a gap to slip through.
                    let home = vec2(W / 2.0 + (e.t * 0.35).sin() * 260.0, (AY0 + AY1) / 2.0 + (e.t * 0.5).sin() * 110.0);
                    e.vel += ((home - e.pos) * 1.2 - e.vel) * (1.5 * dt).min(1.0);
                    let rage = 1.0 - e.hp / e.max_hp;
                    let hard = if wave >= 8.0 { 1.0 } else { 0.0 };
                    e.clock -= dt;
                    if e.clock <= 0.0 {
                        e.clock = 1.5 - rage * 0.5 - hard * 0.3;
                        let count = 14 + (rage * 8.0) as usize + hard as usize * 4;
                        let gap = rand::gen_range(0, count);
                        let spin = e.t * 0.8;
                        for k in 0..count {
                            if k == gap || (k + 1) % count == gap {
                                continue;
                            }
                            let a = spin + k as f32 * TAU / count as f32;
                            let v = vec2(a.cos(), a.sin());
                            new_thorns.push(Thorn { pos: e.pos + v * 40.0, vel: v * (135.0 + rage * 60.0 + hard * 25.0), r: 7.0 });
                        }
                    }
                    e.phase += dt;
                    if e.phase > 3.5 - rage * 1.0 {
                        e.phase = 0.0;
                        new_seeds.push(e.pos);
                        // An aimed volley too, once it is angry.
                        if rage > 0.4 || hard > 0.0 {
                            for s in [-0.2f32, 0.0, 0.2] {
                                let a = dir.y.atan2(dir.x) + s;
                                new_thorns.push(Thorn { pos: e.pos, vel: vec2(a.cos(), a.sin()) * 210.0, r: 6.0 });
                            }
                        }
                    }
                }
            }
            e.pos += e.vel * dt;
            let r = e.r;
            let clamped = in_arena(e.pos, r);
            if clamped.x != e.pos.x {
                e.vel.x = -e.vel.x * 0.5;
            }
            if clamped.y != e.pos.y {
                e.vel.y = -e.vel.y * 0.5;
            }
            e.pos = clamped;
        }
        // Light separation so swarms spread instead of stacking.
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = self.enemies.split_at_mut(j);
                let (ea, eb) = (&mut a[i], &mut b[0]);
                let d = eb.pos - ea.pos;
                let min = ea.r + eb.r;
                let l = d.length();
                if l < min && l > 0.01 {
                    let push = d / l * (min - l) * 0.5;
                    if ea.kind != Kind::Boss {
                        ea.pos -= push;
                    }
                    if eb.kind != Kind::Boss {
                        eb.pos += push;
                    }
                }
            }
        }
        self.thorns.extend(new_thorns);
        for p in new_seeds {
            if self.enemies.len() < 14 {
                for k in 0..2 {
                    let a = rand::gen_range(0.0, TAU) + k as f32;
                    let mut s = Enemy::new(Kind::Sprout, p + vec2(a.cos(), a.sin()) * 50.0, self.wave);
                    s.vel = vec2(a.cos(), a.sin()) * 160.0;
                    self.enemies.push(s);
                }
            }
        }
    }

    fn update_shots(&mut self, dt: f32, snd: &Sounds) {
        let mut kills = Vec::new();
        let mut boss_hit = false;
        let mut sparks = Vec::new();
        for s in self.shots.iter_mut() {
            s.pos += s.vel * dt;
            s.life -= dt;
            if s.pos.x < AX0 || s.pos.x > AX1 || s.pos.y < AY0 || s.pos.y > AY1 {
                s.life = 0.0;
                continue;
            }
            for (i, e) in self.enemies.iter_mut().enumerate() {
                if e.hp > 0.0 && s.pos.distance(e.pos) < e.r + 4.0 {
                    e.hp -= 1.0;
                    e.hit = 0.12;
                    e.vel += s.vel.normalize_or_zero() * if e.kind == Kind::Boss { 4.0 } else { 40.0 };
                    s.life = 0.0;
                    sparks.push((s.pos, e.color()));
                    if e.kind == Kind::Boss {
                        boss_hit = true;
                    }
                    if e.hp <= 0.0 {
                        kills.push(i);
                    }
                    break;
                }
            }
        }
        self.shots.retain(|s| s.life > 0.0);
        for (p, c) in sparks {
            self.burst(p, c, 3, 140.0);
        }
        if boss_hit && self.shot_sound_cd <= 0.0 {
            play_sound(&snd.bosshit, PlaySoundParams { looped: false, volume: 0.3 });
        }
        kills.sort_unstable();
        kills.dedup();
        for i in kills.into_iter().rev() {
            self.kill(i, snd);
        }
    }

    fn update_thorns(&mut self, dt: f32) {
        for t in self.thorns.iter_mut() {
            t.pos += t.vel * dt;
        }
        self.thorns.retain(|t| t.pos.x > AX0 - 10.0 && t.pos.x < AX1 + 10.0 && t.pos.y > AY0 - 10.0 && t.pos.y < AY1 + 10.0);
    }

    fn update_pollen(&mut self, dt: f32, snd: &Sounds) {
        let p = self.pos;
        let mut got = 0;
        for o in self.pollen.iter_mut() {
            o.life -= dt;
            let d = p - o.pos;
            let l = d.length();
            // Pollen drifts to a stop, then the gardener pulls it in.
            o.vel *= 1.0 - (3.0 * dt).min(1.0);
            if l < 140.0 {
                o.vel += d / l.max(1.0) * 900.0 * dt;
            }
            o.pos += o.vel * dt;
            if l < PLAYER_R + 8.0 {
                o.life = 0.0;
                got += 1;
            }
        }
        self.pollen.retain(|o| o.life > 0.0);
        if got > 0 {
            self.pollen_count += got;
            self.score += 5 * got * self.mult;
            self.mult = (1 + self.pollen_count / 12).min(8);
            play_sound(&snd.pickup, PlaySoundParams { looped: false, volume: 0.3 });
        }
    }
}

// ─── drawing ────────────────────────────────────────────────────────────

fn with_a(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

/// A soft glow: stacked translucent discs.
fn glow(p: Vec2, r: f32, c: Color, strength: f32) {
    for (k, a) in [(2.4, 0.05), (1.7, 0.08), (1.25, 0.12)] {
        draw_circle(p.x, p.y, r * k, with_a(c, a * strength));
    }
}

fn draw_floor(t: f32, beat: f32, bloom: &[Cell]) {
    clear_background(Color::from_rgba(7, 9, 20, 255));
    // The hex garden floor.
    let size = HEX;
    for row in 0..HEX_ROWS {
        for col in 0..HEX_COLS {
            let Vec2 { x, y } = hex_centre(row, col);
            if x > AX1 + 10.0 || y > AY1 + 10.0 {
                continue;
            }
            let Cell { v: b, c: bc, .. } = bloom[row * HEX_COLS + col];
            let inside = x > AX0 + 12.0 && x < AX1 - 12.0 && y > AY0 + 12.0 && y < AY1 - 12.0;
            if b > 0.0 && inside {
                // A cleared cell glows in the colour of what fell there, with a
                // small flower at its heart, fading over about forty seconds.
                draw_poly(x, y, 6, size - 3.0, 30.0, with_a(bc, 0.16 * b));
                let fr = 6.0 * b;
                for k in 0..5 {
                    let a = k as f32 * TAU / 5.0 + t * 0.3;
                    draw_circle(x + a.cos() * fr, y + a.sin() * fr, 3.5 * b, with_a(bc, 0.5 * b));
                }
                draw_circle(x, y, 2.5 * b, Color::new(1.0, 0.95, 0.7, 0.7 * b));
            }
            let wave = ((x * 0.012 + y * 0.009 - t * 1.2).sin() * 0.5 + 0.5) * 0.5 + beat * 0.25;
            let c = Color::new(0.10 + 0.10 * wave, 0.25 + 0.25 * wave, 0.30 + 0.25 * wave, 0.25 + 0.2 * wave);
            draw_poly_lines(x, y, 6, size - 2.0, 30.0, 1.5, c);
        }
    }
    // Arena wall.
    draw_rectangle_lines(AX0 - 4.0, AY0 - 4.0, AX1 - AX0 + 8.0, AY1 - AY0 + 8.0, 3.0, Color::from_rgba(70, 150, 170, 200));
    draw_rectangle_lines(AX0 - 9.0, AY0 - 9.0, AX1 - AX0 + 18.0, AY1 - AY0 + 18.0, 2.0, Color::from_rgba(40, 80, 110, 120));
}

fn draw_enemy(e: &Enemy, t: f32) {
    let c = e.color();
    let lit = if e.hit > 0.0 { 0.35 } else { 0.0 };
    let body = Color::new((c.r + lit).min(1.0), (c.g + lit).min(1.0), (c.b + lit).min(1.0), 1.0);
    glow(e.pos, e.r, c, 1.0);
    match e.kind {
        Kind::Seed | Kind::Sprout => {
            draw_poly(e.pos.x, e.pos.y, 6, e.r, e.t * 90.0, with_a(body, 0.85));
            draw_poly_lines(e.pos.x, e.pos.y, 6, e.r, e.t * 90.0, 2.0, WHITE);
            draw_circle(e.pos.x, e.pos.y, e.r * 0.3, Color::from_rgba(20, 60, 20, 255));
        }
        Kind::Darter => {
            let a = e.vel.y.atan2(e.vel.x);
            let crouch = if e.phase == -1.0 { 0.7 } else { 1.0 };
            let f = vec2(a.cos(), a.sin());
            let s = vec2(-f.y, f.x);
            let r = e.r;
            draw_triangle(e.pos + f * r * 1.3 * crouch, e.pos - f * r * 0.8 + s * r, e.pos - f * r * 0.8 - s * r, body);
            if e.phase == -1.0 {
                // Wind-up tell: a pale ring tightening around it.
                draw_circle_lines(e.pos.x, e.pos.y, r * 2.2 + e.clock * 20.0, 2.0, with_a(WHITE, 0.6));
            }
        }
        Kind::Splitter => {
            draw_poly(e.pos.x, e.pos.y, 6, e.r, -e.t * 40.0, with_a(body, 0.8));
            for k in 0..3 {
                let a = e.t * 1.5 + k as f32 * TAU / 3.0;
                draw_circle(e.pos.x + a.cos() * e.r * 0.45, e.pos.y + a.sin() * e.r * 0.45, e.r * 0.28, Color::from_rgba(255, 230, 160, 255));
            }
            draw_poly_lines(e.pos.x, e.pos.y, 6, e.r, -e.t * 40.0, 2.0, WHITE);
        }
        Kind::Orbiter => {
            draw_circle_lines(e.pos.x, e.pos.y, e.r, 3.0, body);
            draw_circle(e.pos.x, e.pos.y, e.r * 0.45, body);
            let a = e.t * 4.0;
            draw_circle(e.pos.x + a.cos() * e.r, e.pos.y + a.sin() * e.r, 3.5, WHITE);
        }
        Kind::Boss => {
            // A great thorn flower: rotating petals around a pulsing heart.
            let petals = 8;
            for k in 0..petals {
                let a = e.t * 0.6 + k as f32 * TAU / petals as f32;
                let f = vec2(a.cos(), a.sin());
                let s = vec2(-f.y, f.x);
                let tip = e.pos + f * e.r * 1.6;
                let pc = if k % 2 == 0 { body } else { Color::new(body.r * 0.8, body.g * 0.6, body.b * 0.9, 1.0) };
                draw_triangle(e.pos + s * e.r * 0.35, e.pos - s * e.r * 0.35, tip, pc);
            }
            let pulse = 1.0 + 0.08 * (t * 6.0).sin();
            draw_circle(e.pos.x, e.pos.y, e.r * 0.7 * pulse, Color::from_rgba(255, 210, 120, 255));
            draw_circle(e.pos.x, e.pos.y, e.r * 0.4 * pulse, Color::from_rgba(255, 120, 80, 255));
        }
    }
}

fn centered(s: &str, y: f32, size: f32, c: Color) {
    let d = measure_text(s, None, size as u16, 1.0);
    draw_text(s, (W - d.width) / 2.0, y, size, c);
}

fn draw(g: &Game, t: f32) {
    let beat = {
        // The floor pulses on the music's quarter notes (128 bpm), softly.
        let ph = (t * 128.0 / 60.0).fract();
        (1.0 - ph).powi(3) * 0.6
    };
    draw_floor(t, if g.mode == Mode::Play { beat } else { 0.0 }, &g.bloom);

    for w in &g.warns {
        // Spawn tell: a ring closing in on the spot.
        let r = 10.0 + w.t * 40.0;
        let c = Enemy::new(w.kind, w.pos, g.wave).color();
        draw_circle_lines(w.pos.x, w.pos.y, r, 2.0, with_a(c, 0.7));
        draw_circle(w.pos.x, w.pos.y, 4.0, with_a(c, 0.5));
    }

    for o in &g.pollen {
        let a = (o.life * 2.0).min(1.0);
        glow(o.pos, 4.0, GOLD, a);
        draw_circle(o.pos.x, o.pos.y, 3.0, with_a(Color::from_rgba(255, 230, 120, 255), a));
    }
    for e in &g.enemies {
        draw_enemy(e, t);
    }
    for th in &g.thorns {
        draw_circle(th.pos.x, th.pos.y, th.r + 3.0, Color::from_rgba(255, 80, 110, 70));
        draw_poly(th.pos.x, th.pos.y, 4, th.r, t * 300.0, Color::from_rgba(255, 150, 170, 255));
    }
    for s in &g.shots {
        let tail = s.pos - s.vel.normalize_or_zero() * 14.0;
        draw_line(tail.x, tail.y, s.pos.x, s.pos.y, 4.0, Color::from_rgba(150, 255, 240, 120));
        draw_line(tail.x, tail.y, s.pos.x, s.pos.y, 2.0, Color::from_rgba(230, 255, 250, 255));
    }
    for r in &g.rings {
        let a = (r.life * 1.5).min(1.0);
        draw_circle_lines(r.pos.x, r.pos.y, r.r, 6.0 * a + 1.0, with_a(r.color, a * 0.8));
        draw_circle(r.pos.x, r.pos.y, r.r, with_a(r.color, a * 0.08));
    }
    for s in &g.sparks {
        let a = (s.life / s.max).clamp(0.0, 1.0);
        draw_rectangle(s.pos.x - s.size / 2.0, s.pos.y - s.size / 2.0, s.size, s.size, with_a(s.color, a));
    }

    if matches!(g.mode, Mode::Play | Mode::Paused) || (g.mode == Mode::Over && g.mode_t < 0.01) || g.mode == Mode::Won {
        draw_player(g, t);
    }

    // A hit tints the arena edges red, swelling in and fading out (never a flash).
    if g.hurt_glow > 0.0 {
        let k = g.hurt_glow;
        let a = (if k > 0.8 { (1.0 - k) * 5.0 } else { k / 0.8 }) * 0.22;
        for i in 0..6 {
            let inset = i as f32 * 10.0;
            draw_rectangle_lines(inset, inset, W - inset * 2.0, H - inset * 2.0, 10.0, Color::new(1.0, 0.2, 0.25, a * (1.0 - i as f32 / 6.0)));
        }
    }

    draw_hud(g, t);
    draw_overlays(g, t);
}

fn draw_player(g: &Game, t: f32) {
    let p = g.pos;
    let blink = if g.invuln > 0.0 { 0.45 + 0.35 * (t * 10.0).sin().abs() } else { 1.0 };
    let body = Color::new(0.55, 1.0, 0.95, blink);
    glow(p, PLAYER_R, Color::from_rgba(120, 255, 230, 255), blink);
    let f = g.facing;
    let s = vec2(-f.y, f.x);
    draw_triangle(p + f * 17.0, p - f * 9.0 + s * 11.0, p - f * 9.0 - s * 11.0, body);
    draw_circle(p.x, p.y, 5.0, Color::new(1.0, 1.0, 1.0, blink));
    if g.invuln > 0.0 {
        draw_circle_lines(p.x, p.y, 22.0, 1.5, Color::new(0.6, 1.0, 0.95, 0.4 * blink));
    }
}

fn draw_hud(g: &Game, _t: f32) {
    if g.mode == Mode::Title {
        return;
    }
    draw_rectangle(0.0, 0.0, W, 44.0, Color::from_rgba(5, 8, 18, 230));
    draw_text(&format!("SCORE {}", g.score), 24.0, 31.0, 32.0, WHITE);
    let m = format!("x{}", g.mult);
    draw_text(&m, 250.0, 31.0, 32.0, Color::from_rgba(255, 220, 110, 255));
    if g.wave > 0 {
        centered(&format!("WAVE {}/{}", g.wave.min(WAVES), WAVES), 31.0, 30.0, Color::from_rgba(150, 230, 255, 255));
    }
    // Lives as small ships, bombs as rings.
    for i in 0..g.lives.max(0) {
        let x = W - 40.0 - i as f32 * 28.0;
        draw_triangle(vec2(x, 12.0), vec2(x - 9.0, 32.0), vec2(x + 9.0, 32.0), Color::from_rgba(140, 255, 230, 255));
    }
    for i in 0..g.bombs.max(0) {
        let x = W - 150.0 - i as f32 * 26.0;
        draw_circle_lines(x, 22.0, 9.0, 3.0, Color::from_rgba(255, 220, 120, 255));
    }
    if g.mode == Mode::Play {
        if let Some(b) = g.enemies.iter().find(|e| e.kind == Kind::Boss) {
            let w = 420.0;
            let x = (W - w) / 2.0;
            draw_rectangle(x, AY1 - 18.0, w, 8.0, Color::from_rgba(60, 20, 30, 200));
            draw_rectangle(x, AY1 - 18.0, w * (b.hp / b.max_hp).max(0.0), 8.0, Color::from_rgba(255, 90, 120, 255));
        }
    }
}

fn draw_overlays(g: &Game, t: f32) {
    let soft = 0.6 + 0.4 * (t * 2.4).sin();
    match g.mode {
        Mode::Title => {
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.02, 0.03, 0.08, 0.55));
            let title_c = Color::from_rgba(150, 255, 220, 255);
            for (k, a) in [(6.0, 0.05), (3.0, 0.1)] {
                let d = measure_text("HEXBLOOM", None, 110, 1.0);
                draw_text("HEXBLOOM", (W - d.width) / 2.0 - k, 175.0 + k, 110.0, with_a(title_c, a));
            }
            centered("HEXBLOOM", 175.0, 110.0, title_c);
            centered("the garden is overgrown. clear eight waves.", 220.0, 28.0, Color::from_rgba(190, 210, 230, 255));
            let lines = [
                ("LEFT STICK / D-PAD", "move"),
                ("RIGHT STICK", "aim and fire"),
                ("HOLD A", "fire at the nearest enemy"),
                ("B", "bloom bomb (clears around you)"),
                ("START", "pause"),
            ];
            for (i, (k, v)) in lines.iter().enumerate() {
                let y = 285.0 + i as f32 * 30.0;
                let d = measure_text(k, None, 26, 1.0);
                draw_text(k, W / 2.0 - 12.0 - d.width, y, 26.0, Color::from_rgba(255, 220, 120, 255));
                draw_text(v, W / 2.0 + 12.0, y, 26.0, Color::from_rgba(210, 225, 240, 255));
            }
            centered("press A to start", 470.0, 36.0, Color::new(1.0, 1.0, 1.0, soft));
            if g.best > 0 {
                centered(&format!("best {}", g.best), 505.0, 24.0, LIGHTGRAY);
            }
        }
        Mode::Paused => {
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.0, 0.0, 0.05, 0.6));
            centered("PAUSED", 250.0, 72.0, WHITE);
            centered("press Start to carry on", 300.0, 30.0, Color::new(1.0, 1.0, 1.0, soft));
        }
        Mode::Over | Mode::Won => {
            let a = (g.mode_t * 1.5).min(1.0);
            draw_rectangle(0.0, 0.0, W, H, Color::new(0.02, 0.02, 0.06, 0.65 * a));
            let (head, c) = if g.mode == Mode::Won {
                ("THE GARDEN IS CLEAR", Color::from_rgba(150, 255, 220, 255))
            } else {
                ("OVERGROWN", Color::from_rgba(255, 110, 140, 255))
            };
            centered(head, 220.0, 72.0, with_a(c, a));
            let reached = if g.mode == Mode::Won { "all eight waves cleared".to_owned() } else { format!("reached wave {}", g.wave) };
            centered(&reached, 268.0, 30.0, with_a(WHITE, a));
            centered(&format!("score {}    best {}", g.score, g.best), 310.0, 34.0, with_a(WHITE, a));
            if g.first_bloom && g.mode == Mode::Over {
                centered("you felled the first thorn flower", 350.0, 26.0, with_a(GOLD, a));
            }
            if g.mode_t > 1.2 {
                centered("press A to play again", 420.0, 32.0, Color::new(1.0, 1.0, 1.0, soft));
            }
        }
        Mode::Play => {
            if g.banner > 0.0 {
                // Fades in and out over two seconds.
                let k = g.banner;
                let a = (k.min(2.2 - k.min(2.2)) * 3.0).clamp(0.0, 1.0);
                centered(&g.banner_text, 250.0, 54.0, Color::new(1.0, 0.95, 0.8, a));
                if g.wave == 1 && g.banner_text.starts_with("WAVE") {
                    centered("right stick fires, or hold A", 292.0, 28.0, Color::new(0.8, 0.9, 1.0, a));
                }
            }
        }
    }
}

// ─── main ───────────────────────────────────────────────────────────────

#[macroquad::main(window_conf)]
async fn main() {
    // Pads are read directly; keys stay the keyboard's.
    wc::set_pad_keymap(&[]);
    let snd = Sounds {
        shoot: load_sound("assets/shoot.wav").await.unwrap(),
        pop: load_sound("assets/pop.wav").await.unwrap(),
        pickup: load_sound("assets/pickup.wav").await.unwrap(),
        hurt: load_sound("assets/hurt.wav").await.unwrap(),
        bomb: load_sound("assets/bomb.wav").await.unwrap(),
        wave: load_sound("assets/wave.wav").await.unwrap(),
        bosshit: load_sound("assets/bosshit.wav").await.unwrap(),
        music: load_sound("assets/music.wav").await.unwrap(),
        boss: load_sound("assets/boss.wav").await.unwrap(),
    };
    play_sound(&snd.music, PlaySoundParams { looped: true, volume: 0.5 });

    let mut pads = Pads { prev: 0 };
    let mut g = Game::new();
    let mut frame = 0u32;
    loop {
        let dt = get_frame_time().min(1.0 / 20.0);
        let t = get_time() as f32;
        frame += 1;
        let inp = pads.read();
        let inp = if wc::debug::input_i32("autopilot", 0) != 0 { autopilot(&g, frame) } else { inp };
        g.update(&inp, dt, &snd);

        wc::debug::set_i32("mode", g.mode as i32);
        wc::debug::set_i32("wave", g.wave as i32);
        wc::debug::set_i32("score", g.score);
        wc::debug::set_i32("lives", g.lives);
        wc::debug::set_i32("bombs", g.bombs);
        wc::debug::set_i32("enemies", g.enemies.len() as i32);
        wc::debug::set_i32("kills", g.kills);
        wc::debug::set_i32("thorns", g.thorns.len() as i32);
        wc::debug::set_f32("px", g.pos.x);
        wc::debug::set_f32("py", g.pos.y);
        let (nx, ny) = g.nearest_enemy().map_or((0.0, 0.0), |e| (e.x - g.pos.x, e.y - g.pos.y));
        wc::debug::set_f32("near_dx", nx);
        wc::debug::set_f32("near_dy", ny);
        // The closest threat (enemy or thorn) for a dodging bot.
        let threat = g
            .thorns
            .iter()
            .map(|t| t.pos)
            .chain(g.enemies.iter().map(|e| e.pos))
            .min_by(|a, b| a.distance_squared(g.pos).total_cmp(&b.distance_squared(g.pos)))
            .map_or((0.0, 0.0, 9999.0), |p| (p.x - g.pos.x, p.y - g.pos.y, p.distance(g.pos)));
        wc::debug::set_f32("threat_dx", threat.0);
        wc::debug::set_f32("threat_dy", threat.1);
        wc::debug::set_f32("threat_d", threat.2);
        wc::debug::set_i32("first_bloom", g.first_bloom as i32);

        draw(&g, t);
        next_frame().await
    }
}
