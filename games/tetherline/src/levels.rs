//! The five islands, built in code: each call places ground, a hazard, a ring
//! or a pickup at tile coordinates (30 px tiles, 18 rows, row 0 at the top).

use macroquad::prelude::*;

pub const TILE: f32 = 30.0;
pub const ROWS: usize = 18;

pub const EMPTY: u8 = 0;
pub const SOLID: u8 = 1;
pub const SPIKES: u8 = 2;
pub const CRUMBLE: u8 = 3;

pub struct Anchor {
    pub base: Vec2,
    /// Horizontal sway in pixels (0 = fixed) and its period in seconds.
    pub sway: f32,
    pub period: f32,
}

impl Anchor {
    pub fn pos(&self, t: f32) -> Vec2 {
        if self.sway == 0.0 {
            self.base
        } else {
            self.base + vec2((t * std::f32::consts::TAU / self.period).sin() * self.sway, 0.0)
        }
    }
}

pub struct Level {
    pub name: &'static str,
    pub cols: usize,
    pub tiles: Vec<u8>,
    pub anchors: Vec<Anchor>,
    pub sparks: Vec<Vec2>,
    pub shards: Vec<Vec2>,
    pub checkpoints: Vec<Vec2>,
    pub signs: Vec<(Vec2, &'static str)>,
    pub start: Vec2,
    pub exit: Vec2,
    /// Sky tint for this island: top and horizon colours.
    pub sky: (Color, Color),
}

/// Tile centre in pixels.
pub fn at(c: usize, r: usize) -> Vec2 {
    vec2(c as f32 * TILE + TILE / 2.0, r as f32 * TILE + TILE / 2.0)
}

struct B {
    l: Level,
}

impl B {
    fn new(name: &'static str, cols: usize, sky: (Color, Color)) -> B {
        B {
            l: Level {
                name,
                cols,
                tiles: vec![EMPTY; cols * ROWS],
                anchors: Vec::new(),
                sparks: Vec::new(),
                shards: Vec::new(),
                checkpoints: Vec::new(),
                signs: Vec::new(),
                start: at(2, 14),
                exit: at(cols - 4, 14),
                sky,
            },
        }
    }
    fn set(&mut self, c: usize, r: usize, v: u8) {
        if c < self.l.cols && r < ROWS {
            self.l.tiles[r * self.l.cols + c] = v;
        }
    }
    /// Solid ground from row `top` down, columns c0..=c1.
    fn ground(&mut self, c0: usize, c1: usize, top: usize) -> &mut Self {
        for c in c0..=c1 {
            for r in top..ROWS {
                self.set(c, r, SOLID);
            }
        }
        self
    }
    /// A solid rectangle.
    fn block(&mut self, c0: usize, c1: usize, r0: usize, r1: usize) -> &mut Self {
        for c in c0..=c1 {
            for r in r0..=r1 {
                self.set(c, r, SOLID);
            }
        }
        self
    }
    fn spikes(&mut self, c0: usize, c1: usize, r: usize) -> &mut Self {
        for c in c0..=c1 {
            self.set(c, r, SPIKES);
        }
        self
    }
    fn crumble(&mut self, c0: usize, c1: usize, r: usize) -> &mut Self {
        for c in c0..=c1 {
            self.set(c, r, CRUMBLE);
        }
        self
    }
    fn ring(&mut self, c: usize, r: usize) -> &mut Self {
        self.l.anchors.push(Anchor { base: at(c, r), sway: 0.0, period: 1.0 });
        self
    }
    fn sway_ring(&mut self, c: usize, r: usize, tiles: f32, period: f32) -> &mut Self {
        self.l.anchors.push(Anchor { base: at(c, r), sway: tiles * TILE, period });
        self
    }
    fn spark(&mut self, c: usize, r: usize) -> &mut Self {
        self.l.sparks.push(at(c, r));
        self
    }
    /// A row of sparks.
    fn sparks(&mut self, c0: usize, c1: usize, r: usize) -> &mut Self {
        for c in c0..=c1 {
            self.spark(c, r);
        }
        self
    }
    /// Sparks along the low arc of a swing under a ring at (c, r).
    fn arc(&mut self, c: usize, r: usize, len: f32) -> &mut Self {
        let centre = at(c, r);
        for k in -3..=3 {
            let a = k as f32 * 0.22;
            let p = centre + vec2(a.sin(), a.cos()) * len * TILE;
            self.l.sparks.push(p);
        }
        self
    }
    fn shard(&mut self, c: usize, r: usize) -> &mut Self {
        self.l.shards.push(at(c, r));
        self
    }
    /// A checkpoint standing on the ground in column c (its base on row `top`).
    fn checkpoint(&mut self, c: usize, top: usize) -> &mut Self {
        self.l.checkpoints.push(at(c, top - 1));
        self
    }
    fn sign(&mut self, c: usize, top: usize, text: &'static str) -> &mut Self {
        self.l.signs.push((at(c, top - 1), text));
        self
    }
    fn start(&mut self, c: usize, top: usize) -> &mut Self {
        self.l.start = at(c, top - 1);
        self
    }
    fn exit(&mut self, c: usize, top: usize) -> &mut Self {
        self.l.exit = at(c, top - 1);
        self
    }
    fn done(self) -> Level {
        self.l
    }
}

pub const COUNT: usize = 5;

pub fn build(i: usize) -> Level {
    match i {
        0 => meadow(),
        1 => cliffs(),
        2 => thorns(),
        3 => drift(),
        _ => summit(),
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba(r, g, b, 255)
}

/// Island 1: everything taught by signs, one idea at a time.
fn meadow() -> Level {
    let mut b = B::new("1  MEADOW", 120, (rgb(92, 160, 230), rgb(236, 214, 170)));
    b.start(2, 15)
        .ground(0, 19, 15)
        .sign(5, 15, "A jumps. Run with the stick or the d-pad.")
        .spikes(12, 13, 14)
        .sparks(11, 14, 11)
        .ground(24, 40, 15)
        .sparks(20, 23, 12)
        .sign(28, 15, "Hold B to fire your tether at the marked ring.")
        .sign(36, 15, "Swing across, then let go of B to fly.")
        .ring(46, 8)
        .arc(46, 8, 5.2)
        .ground(52, 64, 15)
        .checkpoint(54, 15)
        .sign(58, 15, "Push toward where you want to go to swing higher.")
        .ring(71, 7)
        .arc(71, 7, 6.0)
        .shard(71, 12)
        .ground(78, 119, 15)
        .sign(81, 15, "On a tether, up and down reel the line in and out.")
        .block(86, 87, 12, 14)
        .ring(91, 3)
        .block(92, 95, 8, 8)
        .shard(94, 7)
        .sparks(92, 95, 6)
        .sparks(100, 106, 13)
        .shard(20, 9)
        .exit(113, 15);
    b.done()
}

/// Island 2: chains of rings over a long drop.
fn cliffs() -> Level {
    let mut b = B::new("2  CLIFFS", 140, (rgb(70, 120, 200), rgb(240, 190, 150)));
    b.start(2, 13)
        .ground(0, 12, 13)
        .ring(18, 6)
        .arc(18, 6, 5.5)
        .ring(28, 5)
        .arc(28, 5, 6.0)
        .ground(34, 44, 14)
        .checkpoint(36, 14)
        .shard(23, 2)
        .spikes(41, 42, 13)
        .ring(51, 5)
        .ring(60, 6)
        .ring(69, 5)
        .arc(60, 6, 5.5)
        .ground(75, 86, 14)
        .checkpoint(77, 14)
        .shard(65, 13)
        .block(82, 86, 11, 13)
        .ring(92, 4)
        .ring(101, 4)
        .arc(92, 4, 6.0)
        .arc(101, 4, 6.0)
        .ground(107, 112, 13)
        .shard(110, 11)
        .ring(118, 5)
        .arc(118, 5, 5.0)
        .ground(124, 139, 13)
        .exit(134, 13);
    b.done()
}

/// Island 3: spikes below and crumbling ledges.
fn thorns() -> Level {
    let mut b = B::new("3  THORNS", 150, (rgb(120, 90, 170), rgb(240, 170, 160)));
    b.start(2, 14)
        .ground(0, 10, 14)
        .ground(11, 40, 17)
        .spikes(11, 40, 16)
        .crumble(14, 16, 12)
        .crumble(21, 23, 11)
        .sparks(14, 16, 10)
        .sparks(21, 23, 9)
        .ring(30, 5)
        .arc(30, 5, 6.0)
        .shard(19, 6)
        .ground(37, 50, 13)
        .checkpoint(40, 13)
        .spikes(45, 46, 12)
        .ground(51, 90, 17)
        .spikes(51, 90, 16)
        .ring(56, 6)
        .ring(65, 5)
        .crumble(70, 73, 14)
        .ring(79, 5)
        .arc(56, 6, 6.0)
        .arc(79, 5, 6.0)
        .shard(72, 12)
        .ground(86, 100, 13)
        .checkpoint(89, 13)
        .block(96, 97, 5, 10)
        .ring(94, 3)
        .ground(101, 120, 17)
        .spikes(101, 120, 16)
        .ring(106, 6)
        .crumble(111, 113, 14)
        .ring(116, 5)
        .arc(106, 6, 6.0)
        .shard(116, 13)
        .ground(121, 149, 13)
        .exit(144, 13);
    b.done()
}

/// Island 4: rings that sway on the wind.
fn drift() -> Level {
    let mut b = B::new("4  DRIFT", 150, (rgb(40, 70, 130), rgb(220, 140, 120)));
    b.start(2, 13)
        .ground(0, 11, 13)
        .sway_ring(18, 5, 2.0, 3.0)
        .arc(18, 5, 6.0)
        .ground(25, 33, 12)
        .checkpoint(27, 12)
        .sway_ring(40, 5, 3.0, 4.0)
        .sway_ring(51, 4, 2.0, 2.5)
        .shard(45, 13)
        .ground(57, 66, 14)
        .checkpoint(59, 14)
        .spikes(63, 64, 13)
        .ring(72, 4)
        .sway_ring(82, 5, 3.0, 3.5)
        .ring(92, 4)
        .arc(82, 5, 6.0)
        .shard(82, 1)
        .ground(98, 108, 13)
        .checkpoint(100, 13)
        .sway_ring(114, 6, 2.5, 3.0)
        .crumble(119, 121, 14)
        .sway_ring(127, 5, 2.5, 2.2)
        .shard(120, 12)
        .ground(133, 149, 13)
        .exit(144, 13);
    b.done()
}

/// Island 5: the long climb home, everything at once.
fn summit() -> Level {
    let mut b = B::new("5  SUMMIT", 176, (rgb(24, 30, 70), rgb(200, 110, 130)));
    b.start(2, 14)
        .ground(0, 9, 14)
        .ring(15, 6)
        .ring(23, 5)
        .arc(15, 6, 6.0)
        .ground(28, 35, 12)
        .checkpoint(30, 12)
        .spikes(34, 35, 11)
        .ground(36, 70, 17)
        .spikes(36, 70, 16)
        .sway_ring(41, 5, 2.0, 2.8)
        .crumble(46, 48, 14)
        .ring(54, 4)
        .sway_ring(63, 5, 2.5, 3.2)
        .shard(54, 12)
        .ground(70, 80, 12)
        .checkpoint(72, 12)
        .block(78, 80, 6, 11)
        .ring(76, 2)
        .sparks(78, 80, 4)
        .ground(81, 120, 17)
        .spikes(81, 120, 16)
        .ring(86, 6)
        .ring(94, 5)
        .sway_ring(103, 5, 3.0, 3.0)
        .ring(112, 4)
        .shard(98, 1)
        .arc(94, 5, 6.0)
        .ground(118, 128, 13)
        .checkpoint(120, 13)
        .crumble(133, 135, 12)
        .crumble(140, 142, 14)
        .ring(138, 4)
        .sway_ring(148, 5, 2.0, 2.4)
        .shard(141, 12)
        .ground(130, 156, 17)
        .spikes(130, 156, 16)
        .ground(155, 175, 13)
        .exit(170, 13);
    b.done()
}
