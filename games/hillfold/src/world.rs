//! Field geometry for the simulation: what blocks sheep and the dog, and a
//! coarse grid with path distances (the flock's way home, the autopilot's
//! routes). Plain data, no Bevy systems.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use bevy::math::Vec2;

use crate::fields::{FieldDef, GATE_W, PEN_HALF, Side};

/// A wall: a capsule around the segment a..b.
#[derive(Clone, Copy)]
pub struct Seg {
    pub a: Vec2,
    pub b: Vec2,
    pub r: f32,
}

impl Seg {
    fn closest(&self, p: Vec2) -> Vec2 {
        let ab = self.b - self.a;
        let t = ((p - self.a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        self.a + ab * t
    }
}

pub struct Obstacles {
    pub half: Vec2,
    pub segs: Vec<Seg>,
    pub circles: Vec<(Vec2, f32)>,
    /// Axis-aligned blocked boxes (min, max): the brook.
    pub rects: Vec<(Vec2, Vec2)>,
    pub pen: Vec2,
    pub gate: Side,
}

/// The fold's fence as segments, leaving the gate open.
pub fn pen_segments(pen: Vec2, gate: Side) -> Vec<(Vec2, Vec2)> {
    let h = PEN_HALF;
    let (x0, x1, z0, z1) = (pen.x - h.x, pen.x + h.x, pen.y - h.y, pen.y + h.y);
    let sides = [
        (Side::North, Vec2::new(x0, z0), Vec2::new(x1, z0)),
        (Side::South, Vec2::new(x0, z1), Vec2::new(x1, z1)),
        (Side::West, Vec2::new(x0, z0), Vec2::new(x0, z1)),
        (Side::East, Vec2::new(x1, z0), Vec2::new(x1, z1)),
    ];
    let mut out = Vec::new();
    for (s, a, b) in sides {
        if s == gate {
            let mid = (a + b) * 0.5;
            let dir = (b - a).normalize();
            out.push((a, mid - dir * GATE_W * 0.5));
            out.push((mid + dir * GATE_W * 0.5, b));
        } else {
            out.push((a, b));
        }
    }
    out
}

/// The two ends of the gate opening.
pub fn gate_ends(pen: Vec2, gate: Side) -> (Vec2, Vec2) {
    let mid = pen + gate.out() * match gate {
        Side::North | Side::South => PEN_HALF.y,
        _ => PEN_HALF.x,
    };
    let along = Vec2::new(-gate.out().y, gate.out().x);
    (mid - along * GATE_W * 0.5, mid + along * GATE_W * 0.5)
}

/// Centre of the gate opening.
pub fn gate_mid(pen: Vec2, gate: Side) -> Vec2 {
    let (a, b) = gate_ends(pen, gate);
    (a + b) * 0.5
}

impl Obstacles {
    pub fn new(f: &FieldDef) -> Self {
        let mut segs = Vec::new();
        for &(x0, z0, x1, z1) in f.walls {
            segs.push(Seg { a: Vec2::new(x0, z0), b: Vec2::new(x1, z1), r: 0.45 });
        }
        for &(x0, z0, x1, z1) in f.hedges {
            segs.push(Seg { a: Vec2::new(x0, z0), b: Vec2::new(x1, z1), r: 0.6 });
        }
        let pen = Vec2::new(f.pen.0, f.pen.1);
        for (a, b) in pen_segments(pen, f.gate) {
            segs.push(Seg { a, b, r: 0.15 });
        }
        // the open gate stands out from the fence like a funnel wing
        let (ga, _) = gate_ends(pen, f.gate);
        segs.push(Seg { a: ga, b: ga + f.gate.out() * (GATE_W - 0.1), r: 0.12 });
        let mut circles: Vec<(Vec2, f32)> = f.trees.iter().map(|&(x, z)| (Vec2::new(x, z), 0.45)).collect();
        circles.extend(f.rocks.iter().map(|&(x, z)| (Vec2::new(x, z), 0.75)));
        let rects = f
            .water
            .iter()
            .map(|&(x0, z0, x1, z1)| (Vec2::new(x0.min(x1), z0.min(z1)), Vec2::new(x0.max(x1), z0.max(z1))))
            .collect();
        Self { half: Vec2::new(f.half.0, f.half.1), segs, circles, rects, pen, gate: f.gate }
    }

    /// Move a body of radius `r` at `p` out of everything it overlaps.
    pub fn push_out(&self, mut p: Vec2, r: f32) -> Vec2 {
        for s in &self.segs {
            let c = s.closest(p);
            let d = p - c;
            let min = s.r + r;
            let l = d.length();
            if l < min {
                let n = if l > 1e-4 { d / l } else { Vec2::Y };
                p = c + n * min;
            }
        }
        for &(c, cr) in &self.circles {
            let d = p - c;
            let min = cr + r;
            let l = d.length();
            if l < min {
                let n = if l > 1e-4 { d / l } else { Vec2::X };
                p = c + n * min;
            }
        }
        for &(lo, hi) in &self.rects {
            let lo = lo - Vec2::splat(r);
            let hi = hi + Vec2::splat(r);
            if p.x > lo.x && p.x < hi.x && p.y > lo.y && p.y < hi.y {
                // out through the nearest edge
                let dl = p.x - lo.x;
                let dr = hi.x - p.x;
                let dt = p.y - lo.y;
                let db = hi.y - p.y;
                let m = dl.min(dr).min(dt).min(db);
                if m == dl {
                    p.x = lo.x;
                } else if m == dr {
                    p.x = hi.x;
                } else if m == dt {
                    p.y = lo.y;
                } else {
                    p.y = hi.y;
                }
            }
        }
        let lim = self.half - Vec2::splat(0.6 + r);
        p.clamp(-lim, lim)
    }

    /// Distance from `p` to the nearest obstacle surface (the field wall included).
    pub fn clearance(&self, p: Vec2) -> f32 {
        let mut d = (self.half.x - p.x.abs()).min(self.half.y - p.y.abs()) - 0.6;
        for s in &self.segs {
            d = d.min(p.distance(s.closest(p)) - s.r);
        }
        for &(c, cr) in &self.circles {
            d = d.min(p.distance(c) - cr);
        }
        for &(lo, hi) in &self.rects {
            let q = Vec2::new(p.x.clamp(lo.x, hi.x), p.y.clamp(lo.y, hi.y));
            d = d.min(p.distance(q));
        }
        d
    }

    /// Steering away from obstacles a body moving along `vel` is about to hit.
    pub fn avoid(&self, p: Vec2, vel: Vec2) -> Vec2 {
        let mut f = Vec2::ZERO;
        let mut add = |c: Vec2, surf: f32| {
            let d = p - c;
            let l = d.length();
            let gap = l - surf;
            if gap < 1.3 && l > 1e-4 {
                let n = d / l;
                // only what lies ahead: a sheep may walk along a fence or
                // through a gate without being pushed off it
                let ahead = (-n).dot(vel.normalize_or_zero()).max(0.0);
                f += n * (1.3 - gap) * 2.2 * ahead;
            }
        };
        for s in &self.segs {
            add(s.closest(p), s.r);
        }
        for &(c, cr) in &self.circles {
            add(c, cr);
        }
        for &(lo, hi) in &self.rects {
            add(Vec2::new(p.x.clamp(lo.x, hi.x), p.y.clamp(lo.y, hi.y)), 0.0);
        }
        let lim = self.half - Vec2::splat(0.6);
        if p.x > lim.x - 1.6 { f.x -= 1.6 - (lim.x - p.x); }
        if p.x < -lim.x + 1.6 { f.x += 1.6 - (p.x + lim.x); }
        if p.y > lim.y - 1.6 { f.y -= 1.6 - (lim.y - p.y); }
        if p.y < -lim.y + 1.6 { f.y += 1.6 - (p.y + lim.y); }
        f
    }

    /// True if `p` is inside the fold (with a margin from the fence).
    pub fn in_pen(&self, p: Vec2, margin: f32) -> bool {
        let d = (p - self.pen).abs();
        d.x < PEN_HALF.x - margin && d.y < PEN_HALF.y - margin
    }

    /// Keep a penned sheep inside the fold.
    pub fn clamp_pen(&self, p: Vec2, r: f32) -> Vec2 {
        let lim = PEN_HALF - Vec2::splat(0.25 + r);
        self.pen + (p - self.pen).clamp(-lim, lim)
    }
}

/// A 1 m grid over the field with path distances.
pub struct Grid {
    pub nx: usize,
    pub nz: usize,
    pub origin: Vec2,
    /// Clearance per cell (blocked below a threshold).
    pub clear: Vec<f32>,
}

#[derive(PartialEq)]
struct Node(f32, usize);
impl Eq for Node {}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
    }
}

impl Grid {
    pub fn new(obs: &Obstacles) -> Self {
        let nx = (obs.half.x * 2.0) as usize;
        let nz = (obs.half.y * 2.0) as usize;
        let origin = -obs.half;
        let mut clear = vec![0.0; nx * nz];
        for z in 0..nz {
            for x in 0..nx {
                let p = origin + Vec2::new(x as f32 + 0.5, z as f32 + 0.5);
                clear[z * nx + x] = obs.clearance(p);
            }
        }
        Self { nx, nz, origin, clear }
    }

    pub fn cell(&self, p: Vec2) -> usize {
        let c = p - self.origin;
        let x = (c.x as i32).clamp(0, self.nx as i32 - 1) as usize;
        let z = (c.y as i32).clamp(0, self.nz as i32 - 1) as usize;
        z * self.nx + x
    }

    pub fn center(&self, i: usize) -> Vec2 {
        self.origin + Vec2::new((i % self.nx) as f32 + 0.5, (i / self.nx) as f32 + 0.5)
    }

    fn open(&self, i: usize, min_clear: f32) -> bool {
        self.clear[i] >= min_clear
    }

    /// The open cell nearest `p` (p's own cell when it is open).
    pub fn nearest_open(&self, p: Vec2, min_clear: f32) -> usize {
        let c = self.cell(p);
        if self.open(c, min_clear) {
            return c;
        }
        let mut best = c;
        let mut bd = f32::MAX;
        for i in 0..self.clear.len() {
            if self.open(i, min_clear) {
                let d = self.center(i).distance_squared(p);
                if d < bd {
                    bd = d;
                    best = i;
                }
            }
        }
        best
    }

    /// Path distance from every cell to `goal`, through cells with at least
    /// `min_clear` clearance; `cost` adds a per-cell penalty.
    pub fn distances(&self, goal: Vec2, min_clear: f32, cost: impl Fn(Vec2) -> f32) -> Vec<f32> {
        let mut dist = vec![f32::MAX; self.clear.len()];
        let g = self.nearest_open(goal, min_clear);
        dist[g] = 0.0;
        let mut heap = BinaryHeap::new();
        heap.push(Node(0.0, g));
        const N: [(i32, i32, f32); 8] = [
            (1, 0, 1.0), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0),
            (1, 1, 1.414), (1, -1, 1.414), (-1, 1, 1.414), (-1, -1, 1.414),
        ];
        while let Some(Node(d, i)) = heap.pop() {
            if d > dist[i] {
                continue;
            }
            let (x, z) = ((i % self.nx) as i32, (i / self.nx) as i32);
            for (dx, dz, w) in N {
                let (nx, nz) = (x + dx, z + dz);
                if nx < 0 || nz < 0 || nx >= self.nx as i32 || nz >= self.nz as i32 {
                    continue;
                }
                let j = nz as usize * self.nx + nx as usize;
                if !self.open(j, min_clear) {
                    continue;
                }
                // no corner cutting past a blocked cell
                if dx != 0 && dz != 0 {
                    let a = z as usize * self.nx + nx as usize;
                    let b = nz as usize * self.nx + x as usize;
                    if !self.open(a, min_clear) || !self.open(b, min_clear) {
                        continue;
                    }
                }
                let nd = d + w * (1.0 + cost(self.center(j)));
                if nd < dist[j] {
                    dist[j] = nd;
                    heap.push(Node(nd, j));
                }
            }
        }
        dist
    }

    /// The direction downhill on `dist` from `p` (toward the goal), sampled
    /// over the cells around p.
    pub fn downhill(&self, dist: &[f32], p: Vec2) -> Vec2 {
        let c = self.cell(p);
        let (x, z) = ((c % self.nx) as i32, (c / self.nx) as i32);
        let here = dist[c];
        let mut best = Vec2::ZERO;
        let mut bd = here;
        for dz in -2..=2 {
            for dx in -2..=2 {
                if dx == 0 && dz == 0 {
                    continue;
                }
                let (nx, nz) = (x + dx, z + dz);
                if nx < 0 || nz < 0 || nx >= self.nx as i32 || nz >= self.nz as i32 {
                    continue;
                }
                let j = nz as usize * self.nx + nx as usize;
                let len = ((dx * dx + dz * dz) as f32).sqrt();
                // per metre of travel, so a far cell does not win by distance alone
                let d = here - (here - dist[j]) / len;
                if dist[j] < f32::MAX && d < bd {
                    bd = d;
                    best = self.center(j) - p;
                }
            }
        }
        if here == f32::MAX {
            // off the open grid: head for the nearest open cell
            let j = self
                .clear
                .iter()
                .enumerate()
                .filter(|(i, _)| dist[*i] < f32::MAX)
                .min_by(|a, b| self.center(a.0).distance(p).total_cmp(&self.center(b.0).distance(p)))
                .map(|(i, _)| i);
            if let Some(j) = j {
                return (self.center(j) - p).normalize_or_zero();
            }
        }
        best.normalize_or_zero()
    }

    pub fn at(&self, dist: &[f32], p: Vec2) -> f32 {
        dist[self.cell(p)]
    }
}
