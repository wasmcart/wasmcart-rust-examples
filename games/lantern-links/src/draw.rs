//! Building the course picture: one vertex-coloured mesh per hole (floor,
//! raised walls with their shadows, slope arrows, lantern light baked into
//! the colours), plus small shape meshes and a soft glow texture.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::course::{COLS, Hole, ROWS, TILE, Tile};

/// Course pixels (y down, origin at the course's top left) to world.
pub fn world(p: Vec2) -> Vec2 {
    Vec2::new(p.x - 480.0, 240.0 - p.y)
}

#[derive(Default)]
pub struct MeshBuf {
    pos: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

fn lin(c: [f32; 4]) -> [f32; 4] {
    Color::srgba(c[0], c[1], c[2], c[3]).to_linear().to_f32_array()
}

impl MeshBuf {
    /// A quad in course pixels with a colour per corner (tl, tr, br, bl).
    pub fn quad4(&mut self, lo: Vec2, hi: Vec2, c: [[f32; 4]; 4]) {
        let b = self.pos.len() as u32;
        for (p, c) in [(Vec2::new(lo.x, lo.y), c[0]), (Vec2::new(hi.x, lo.y), c[1]), (Vec2::new(hi.x, hi.y), c[2]), (Vec2::new(lo.x, hi.y), c[3])] {
            let w = world(p);
            self.pos.push([w.x, w.y, 0.0]);
            self.col.push(lin(c));
        }
        self.idx.extend_from_slice(&[b, b + 2, b + 1, b, b + 3, b + 2]);
    }
    pub fn quad(&mut self, lo: Vec2, hi: Vec2, c: [f32; 4]) {
        self.quad4(lo, hi, [c; 4]);
    }
    pub fn tri(&mut self, a: Vec2, b2: Vec2, c2: Vec2, c: [f32; 4]) {
        let b = self.pos.len() as u32;
        for p in [a, b2, c2] {
            let w = world(p);
            self.pos.push([w.x, w.y, 0.0]);
            self.col.push(lin(c));
        }
        self.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 1]);
    }
    pub fn build(self) -> Mesh {
        let n = self.pos.len();
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 0.0, 1.0]; n])
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_indices(Indices::U32(self.idx))
    }
}

/// Warm lantern light at a course point (0..~1.5).
fn light(h: &Hole, p: Vec2) -> f32 {
    let mut l = 0.0;
    for q in &h.lanterns {
        let d = p.distance(*q) / 260.0;
        if d < 1.0 {
            l += (1.0 - d) * (1.0 - d);
        }
    }
    // the cup has its own little lamp
    let d = p.distance(h.cup) / 120.0;
    if d < 1.0 {
        l += 0.5 * (1.0 - d) * (1.0 - d);
    }
    l.min(1.5)
}

fn lit(base: [f32; 3], l: f32) -> [f32; 4] {
    let warm = [1.0, 0.78, 0.45];
    let k = 0.5 + 0.55 * l;
    [
        (base[0] * k + warm[0] * 0.1 * l).min(1.0),
        (base[1] * k + warm[1] * 0.1 * l).min(1.0),
        (base[2] * k + warm[2] * 0.1 * l).min(1.0),
        1.0,
    ]
}

fn hash(x: usize, y: usize, k: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663) ^ k.wrapping_mul(83492791);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    (h & 0xffff) as f32 / 65535.0
}

const WALL_H: f32 = 10.0;

pub fn course_mesh(h: &Hole) -> Mesh {
    let mut m = MeshBuf::default();
    // the garden around the course
    for y in 0..ROWS {
        for x in 0..COLS {
            let lo = Vec2::new(x as f32 * TILE, y as f32 * TILE);
            let t = h.tile(x as i32, y as i32);
            if t != Tile::Out {
                continue;
            }
            let l = light(h, lo + Vec2::splat(TILE * 0.5)) * 0.6;
            let v = 0.85 + 0.3 * hash(x, y, 1);
            m.quad(lo, lo + Vec2::splat(TILE), lit([0.05 * v, 0.11 * v, 0.08 * v], l));
            // a few leaves
            for k in 0..3 {
                let c = lo + Vec2::new(hash(x, y, 10 + k) * TILE, hash(x, y, 20 + k) * TILE);
                let a = hash(x, y, 30 + k) * 6.28;
                let d = Vec2::new(a.cos(), a.sin());
                let s = 4.0 + 5.0 * hash(x, y, 40 + k);
                let g = 0.1 + 0.08 * hash(x, y, 50 + k);
                m.tri(c - d * s, c + Vec2::new(-d.y, d.x) * s * 0.45, c + d * s, lit([g * 0.6, g * 1.6, g * 0.9], l));
            }
        }
    }
    // the floor, with the light at each corner for smooth pools of light
    for y in 0..ROWS {
        for x in 0..COLS {
            let t = h.tile(x as i32, y as i32);
            if t.solid() {
                continue;
            }
            let lo = Vec2::new(x as f32 * TILE, y as f32 * TILE);
            let base = match t {
                Tile::Sand => {
                    let v = 0.95 + 0.08 * hash(x, y, 2);
                    [0.80 * v, 0.70 * v, 0.47 * v]
                }
                Tile::Water => [0.08, 0.22, 0.32],
                Tile::Slope(_) => {
                    if x % 2 == 0 { [0.25, 0.47, 0.25] } else { [0.27, 0.5, 0.27] }
                }
                _ => {
                    if x % 2 == 0 { [0.22, 0.45, 0.23] } else { [0.24, 0.49, 0.25] }
                }
            };
            let corners = [lo, lo + Vec2::new(TILE, 0.0), lo + Vec2::splat(TILE), lo + Vec2::new(0.0, TILE)];
            m.quad4(lo, lo + Vec2::splat(TILE), corners.map(|c| lit(base, light(h, c))));
            if let Tile::Slope(d) = t {
                // a chevron pointing downhill
                let c = lo + Vec2::splat(TILE * 0.5);
                let side = Vec2::new(-d.y, d.x);
                let col = lit([0.38, 0.62, 0.36], light(h, c));
                for off in [-5.0, 5.0] {
                    let tip = c + d * (off + 4.0);
                    m.tri(tip, tip - d * 6.0 + side * 7.0, tip - d * 3.0, col);
                    m.tri(tip, tip - d * 3.0, tip - d * 6.0 - side * 7.0, col);
                }
            }
            if t == Tile::Sand {
                for k in 0..4 {
                    let c = lo + Vec2::new(hash(x, y, 60 + k) * TILE, hash(x, y, 70 + k) * TILE);
                    m.quad(c, c + Vec2::splat(2.0), lit([0.62, 0.53, 0.35], light(h, c)));
                }
            }
        }
    }
    // wall shadows, then each row's walls (front face, then top) so lower
    // rows overlap the ones above
    for y in 0..ROWS {
        for x in 0..COLS {
            if h.tile(x as i32, y as i32) != Tile::Wall {
                continue;
            }
            let lo = Vec2::new(x as f32 * TILE, y as f32 * TILE);
            m.quad(lo + Vec2::new(6.0, 4.0), lo + Vec2::new(TILE + 6.0, TILE + 4.0), [0.0, 0.0, 0.0, 0.35]);
        }
    }
    for y in 0..ROWS {
        for x in 0..COLS {
            if h.tile(x as i32, y as i32) != Tile::Wall {
                continue;
            }
            let lo = Vec2::new(x as f32 * TILE, y as f32 * TILE);
            let l = light(h, lo + Vec2::splat(TILE * 0.5));
            let v = 0.9 + 0.15 * hash(x, y, 3);
            if !h.tile(x as i32, y as i32 + 1).solid() {
                m.quad(lo + Vec2::new(0.0, TILE - WALL_H), lo + Vec2::splat(TILE), lit([0.26 * v, 0.2 * v, 0.17 * v], l));
            }
            let top_lo = lo - Vec2::new(0.0, WALL_H);
            let top_c = lit([0.5 * v, 0.43 * v, 0.36 * v], l);
            m.quad(top_lo, top_lo + Vec2::splat(TILE), top_c);
            // stone joints
            let j = lit([0.38 * v, 0.32 * v, 0.27 * v], l);
            m.quad(top_lo + Vec2::new(0.0, TILE * 0.5 - 1.0), top_lo + Vec2::new(TILE, TILE * 0.5 + 1.0), j);
            let jx = if y % 2 == 0 { TILE * 0.3 } else { TILE * 0.7 };
            m.quad(top_lo + Vec2::new(jx - 1.0, 0.0), top_lo + Vec2::new(jx + 1.0, TILE * 0.5), j);
            m.quad(top_lo + Vec2::new(TILE - jx - 1.0, TILE * 0.5), top_lo + Vec2::new(TILE - jx + 1.0, TILE), j);
        }
    }
    m.build()
}

/// A soft round glow, white with alpha falling off from the centre.
pub fn glow_image() -> Image {
    let n = 64u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let d = Vec2::new(x as f32 + 0.5 - n as f32 / 2.0, y as f32 + 0.5 - n as f32 / 2.0).length() / (n as f32 / 2.0);
            let a = (1.0 - d).clamp(0.0, 1.0);
            let a = a * a * (3.0 - 2.0 * a);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    Image::new(
        Extent3d { width: n, height: n, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}
