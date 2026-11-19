//! The course: holes parsed from assets/holes.txt, and the ball's physics.
//! Plain data and functions, shared by the game and the autopilot's
//! look-ahead (which runs the very same `step`).

use bevy::math::Vec2;

pub const TILE: f32 = 30.0;
pub const COLS: usize = 32;
pub const ROWS: usize = 17;
pub const BALL_R: f32 = 7.0;
pub const CUP_R: f32 = 10.5;
pub const BUMPER_R: f32 = 13.0;
pub const BAR_HALF: f32 = 68.0;
pub const BAR_W: f32 = 5.0;
pub const BAR_SPIN: f32 = 1.3;
/// Substeps per frame: the ball moves under 3 px a substep at full speed.
pub const SUB: usize = 8;
pub const MAX_SPEED: f32 = 1250.0;

/// Launch speed for a shot of `power` (0..1).
pub fn shot_speed(power: f32) -> f32 {
    70.0 + 1030.0 * power
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tile {
    Out,
    Wall,
    Green,
    Sand,
    Water,
    Slope(Vec2),
}

impl Tile {
    pub fn solid(self) -> bool {
        matches!(self, Tile::Out | Tile::Wall)
    }
}

pub struct Hole {
    pub name: String,
    pub par: u32,
    pub tiles: Vec<Tile>,
    pub tee: Vec2,
    pub cup: Vec2,
    pub bumpers: Vec<Vec2>,
    pub bars: Vec<Vec2>,
    pub lanterns: Vec<Vec2>,
    /// Walking distance (px) from each tile to the cup, for the autopilot.
    pub dist: Vec<f32>,
}

impl Hole {
    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= COLS as i32 || y >= ROWS as i32 {
            return Tile::Out;
        }
        self.tiles[y as usize * COLS + x as usize]
    }
    pub fn tile_at(&self, p: Vec2) -> Tile {
        self.tile((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32)
    }
    /// Path distance to the cup from `p`, through open tiles, plus the
    /// straight bit inside the tile.
    pub fn dist_at(&self, p: Vec2) -> f32 {
        let (x, y) = ((p.x / TILE).floor() as i32, (p.y / TILE).floor() as i32);
        if x < 0 || y < 0 || x >= COLS as i32 || y >= ROWS as i32 {
            return 1e6;
        }
        let d = self.dist[y as usize * COLS + x as usize];
        let c = Vec2::new((x as f32 + 0.5) * TILE, (y as f32 + 0.5) * TILE);
        if d == 0.0 { p.distance(self.cup) } else { d + p.distance(c) * 0.5 }
    }
}

/// Parse the course file.
pub fn parse(src: &str) -> Vec<Hole> {
    let mut holes = Vec::new();
    let mut cur: Option<(String, u32, Vec<String>)> = None;
    let finish = |c: (String, u32, Vec<String>), holes: &mut Vec<Hole>| {
        let (name, par, rows) = c;
        let mut tiles = vec![Tile::Out; COLS * ROWS];
        let (mut tee, mut cup) = (Vec2::ZERO, Vec2::ZERO);
        let (mut bumpers, mut bars, mut lanterns) = (Vec::new(), Vec::new(), Vec::new());
        for (y, row) in rows.iter().enumerate().take(ROWS) {
            for (x, ch) in row.chars().enumerate().take(COLS) {
                let c = Vec2::new((x as f32 + 0.5) * TILE, (y as f32 + 0.5) * TILE);
                let t = match ch {
                    '#' => Tile::Wall,
                    'L' => {
                        lanterns.push(c);
                        Tile::Wall
                    }
                    '.' => Tile::Green,
                    ':' => Tile::Sand,
                    '~' => Tile::Water,
                    '<' => Tile::Slope(Vec2::new(-1.0, 0.0)),
                    '>' => Tile::Slope(Vec2::new(1.0, 0.0)),
                    '^' => Tile::Slope(Vec2::new(0.0, -1.0)),
                    'v' => Tile::Slope(Vec2::new(0.0, 1.0)),
                    'T' => {
                        tee = c;
                        Tile::Green
                    }
                    'O' => {
                        cup = c;
                        Tile::Green
                    }
                    'o' => {
                        bumpers.push(c);
                        Tile::Green
                    }
                    'R' => {
                        bars.push(c);
                        Tile::Green
                    }
                    _ => Tile::Out,
                };
                tiles[y * COLS + x] = t;
            }
        }
        let mut h = Hole { name, par, tiles, tee, cup, bumpers, bars, lanterns, dist: Vec::new() };
        h.dist = distances(&h);
        holes.push(h);
    };
    for line in src.lines() {
        if line.starts_with(';') {
            continue;
        }
        if let Some(head) = line.strip_prefix('=') {
            if let Some(c) = cur.take() {
                finish(c, &mut holes);
            }
            let mut it = head.split('|');
            let name = it.next().unwrap_or("").trim().to_string();
            let par = it.next().and_then(|p| p.trim().parse().ok()).unwrap_or(3);
            cur = Some((name, par, Vec::new()));
        } else if let Some(c) = cur.as_mut() {
            if c.2.len() < ROWS {
                c.2.push(line.to_string());
            }
        }
    }
    if let Some(c) = cur.take() {
        finish(c, &mut holes);
    }
    holes
}

/// Dijkstra over open tiles from the cup (water costs a lot: the ball can
/// cross it only by going round).
fn distances(h: &Hole) -> Vec<f32> {
    let mut d = vec![f32::MAX; COLS * ROWS];
    let start = (h.cup.y / TILE) as usize * COLS + (h.cup.x / TILE) as usize;
    d[start] = 0.0;
    let mut open = vec![start];
    while !open.is_empty() {
        // small grid: a linear scan for the minimum is plenty
        let (k, &i) = open.iter().enumerate().min_by(|a, b| d[*a.1].total_cmp(&d[*b.1])).unwrap();
        open.swap_remove(k);
        let (x, y) = ((i % COLS) as i32, (i / COLS) as i32);
        for (dx, dy, w) in [(1, 0, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0), (1, 1, 1.414), (1, -1, 1.414), (-1, 1, 1.414), (-1, -1, 1.414)] {
            let (nx, ny) = (x + dx, y + dy);
            let t = h.tile(nx, ny);
            if t.solid() || t == Tile::Water {
                continue;
            }
            if dx != 0 && dy != 0 && (h.tile(x + dx, y).solid() || h.tile(x, y + dy).solid()) {
                continue;
            }
            let j = ny as usize * COLS + nx as usize;
            let cost = match t {
                Tile::Sand => 1.6,
                _ => 1.0,
            };
            let nd = d[i] + w * cost * TILE;
            if nd < d[j] {
                if d[j] == f32::MAX {
                    open.push(j);
                }
                d[j] = nd;
            }
        }
    }
    d
}

#[derive(Clone, Copy, Debug)]
pub struct Ball {
    pub pos: Vec2,
    pub vel: Vec2,
}

/// What happened during a step, for sounds and rules.
#[derive(Default, Clone, Copy, Debug)]
pub struct Hits {
    pub wall: f32,
    pub bumper: Option<usize>,
    pub bar: f32,
    pub sunk: bool,
    pub water: bool,
    pub lip: bool,
}

pub fn bar_angle(t: f32) -> f32 {
    t * BAR_SPIN
}

fn closest_on_box(p: Vec2, lo: Vec2, hi: Vec2) -> Vec2 {
    Vec2::new(p.x.clamp(lo.x, hi.x), p.y.clamp(lo.y, hi.y))
}

/// Advance the ball one frame (`dt` seconds) at hole time `t`.
pub fn step(h: &Hole, b: &mut Ball, t: f32, dt: f32) -> Hits {
    let mut hits = Hits::default();
    let s = dt / SUB as f32;
    for k in 0..SUB {
        let tk = t + s * k as f32;
        let tile = h.tile_at(b.pos);
        let sp = b.vel.length();
        // rolling resistance
        let (fric, lin) = match tile {
            Tile::Sand => (900.0, 1.6),
            _ => (190.0, 0.22),
        };
        if sp > 0.0 {
            let dec = (fric + lin * sp) * s;
            b.vel *= ((sp - dec).max(0.0)) / sp;
        }
        if let Tile::Slope(dir) = tile {
            b.vel += dir * 420.0 * s;
        }
        // the cup's lip pulls a slow ball in
        let to_cup = h.cup - b.pos;
        let dc = to_cup.length();
        if dc < CUP_R + 6.0 && dc > 0.01 {
            b.vel += to_cup / dc * 520.0 * (1.0 - dc / (CUP_R + 6.0)) * s;
            if dc < CUP_R - 3.0 {
                if b.vel.length() < 340.0 {
                    hits.sunk = true;
                    b.pos = h.cup;
                    b.vel = Vec2::ZERO;
                    return hits;
                }
                hits.lip = true;
            }
        }
        b.pos += b.vel * s;
        // walls: the solid tiles around the ball
        let (tx, ty) = ((b.pos.x / TILE).floor() as i32, (b.pos.y / TILE).floor() as i32);
        for yy in ty - 1..=ty + 1 {
            for xx in tx - 1..=tx + 1 {
                if !h.tile(xx, yy).solid() {
                    continue;
                }
                let lo = Vec2::new(xx as f32 * TILE, yy as f32 * TILE);
                let c = closest_on_box(b.pos, lo, lo + Vec2::splat(TILE));
                let d = b.pos - c;
                let l = d.length();
                if l < BALL_R {
                    let n = if l > 1e-4 { d / l } else { Vec2::Y };
                    b.pos = c + n * BALL_R;
                    let vn = b.vel.dot(n);
                    if vn < 0.0 {
                        b.vel -= n * vn * 1.76;
                        hits.wall = hits.wall.max(-vn);
                    }
                }
            }
        }
        for (i, bp) in h.bumpers.iter().enumerate() {
            let d = b.pos - *bp;
            let l = d.length();
            if l < BUMPER_R + BALL_R && l > 1e-4 {
                let n = d / l;
                b.pos = *bp + n * (BUMPER_R + BALL_R);
                let vn = b.vel.dot(n);
                if vn < 0.0 {
                    b.vel -= n * vn * 2.1;
                    // a bumper always kicks back with some pace
                    let out = b.vel.dot(n);
                    if out < 260.0 {
                        b.vel += n * (260.0 - out);
                    }
                    hits.bumper = Some(i);
                }
            }
        }
        for c in &h.bars {
            let a = bar_angle(tk);
            let dir = Vec2::new(a.cos(), a.sin());
            let rel = b.pos - *c;
            let along = rel.dot(dir).clamp(-BAR_HALF, BAR_HALF);
            let q = *c + dir * along;
            let d = b.pos - q;
            let l = d.length();
            if l < BAR_W + BALL_R {
                let n = if l > 1e-4 { d / l } else { Vec2::new(-dir.y, dir.x) };
                b.pos = q + n * (BAR_W + BALL_R);
                // the bar's own speed at that point
                let r = q - *c;
                let vbar = Vec2::new(-r.y, r.x) * BAR_SPIN;
                let relv = b.vel - vbar;
                let vn = relv.dot(n);
                if vn < 0.0 {
                    b.vel -= n * vn * 1.7;
                    hits.bar = hits.bar.max(-vn);
                }
            }
        }
        let v = b.vel.length();
        if v > MAX_SPEED {
            b.vel *= MAX_SPEED / v;
        }
        if h.tile_at(b.pos) == Tile::Water {
            hits.water = true;
            b.vel = Vec2::ZERO;
            return hits;
        }
    }
    hits
}

/// True once the ball has stopped (slopes keep it rolling).
pub fn at_rest(h: &Hole, b: &Ball) -> bool {
    let slope = matches!(h.tile_at(b.pos), Tile::Slope(_));
    b.vel.length() < 7.0 && !slope && b.pos.distance(h.cup) > CUP_R + 6.0
        || b.vel.length() < 2.0 && !slope
}

/// Result of a whole shot, for the autopilot's look-ahead.
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    pub rest: Vec2,
    pub sunk: bool,
    pub water: bool,
    pub frames: u32,
}

/// Play a shot from `from` at angle `a`, power `p`, starting at hole time
/// `t0`, until the ball stops (or `max_frames`).
pub fn simulate(h: &Hole, from: Vec2, a: f32, p: f32, t0: f32, max_frames: u32) -> Outcome {
    let dt = 1.0 / 60.0;
    let mut b = Ball { pos: from, vel: Vec2::new(a.cos(), a.sin()) * shot_speed(p) };
    for f in 0..max_frames {
        let hits = step(h, &mut b, t0 + f as f32 * dt, dt);
        if hits.sunk {
            return Outcome { rest: h.cup, sunk: true, water: false, frames: f };
        }
        if hits.water {
            return Outcome { rest: b.pos, sunk: false, water: true, frames: f };
        }
        if f > 2 && at_rest(h, &b) {
            return Outcome { rest: b.pos, sunk: false, water: false, frames: f };
        }
    }
    Outcome { rest: b.pos, sunk: false, water: false, frames: max_frames }
}
