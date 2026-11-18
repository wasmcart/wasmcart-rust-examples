//! Procedural meshes: position + normal per vertex, u16 indices.

use crate::math::{cross, norm, sub, V3};
use core::f32::consts::TAU;

#[derive(Default)]
pub struct Mesh {
    /// x y z nx ny nz per vertex.
    pub verts: Vec<f32>,
    pub idx: Vec<u16>,
}

impl Mesh {
    fn base(&self) -> u16 {
        (self.verts.len() / 6) as u16
    }
    fn vert(&mut self, p: V3, n: V3) {
        self.verts.extend_from_slice(&[p[0], p[1], p[2], n[0], n[1], n[2]]);
    }
    /// A flat-shaded triangle (its own three vertices), wound counter-clockwise
    /// seen from the side the normal points to.
    fn tri(&mut self, a: V3, b: V3, c: V3) {
        let n = norm(cross(sub(b, a), sub(c, a)));
        let i = self.base();
        self.vert(a, n);
        self.vert(b, n);
        self.vert(c, n);
        self.idx.extend_from_slice(&[i, i + 1, i + 2]);
    }
    fn quad(&mut self, a: V3, b: V3, c: V3, d: V3) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }
}

/// A faceted hexagonal crystal: base at y=0, radius 1, shaft to y=0.82,
/// point at y=1. Flat shaded so each facet catches light on its own.
pub fn crystal() -> Mesh {
    let mut m = Mesh::default();
    let n = 6;
    let ring = |k: usize, y: f32, r: f32| -> V3 {
        let a = k as f32 / n as f32 * TAU + 0.3;
        [a.cos() * r, y, a.sin() * r]
    };
    let tip = [0.0, 1.0, 0.0];
    let bot = [0.0, -0.04, 0.0];
    for k in 0..n {
        let k1 = (k + 1) % n;
        let (b0, b1) = (ring(k, 0.0, 0.92), ring(k1, 0.0, 0.92));
        let (t0, t1) = (ring(k, 0.82, 1.0), ring(k1, 0.82, 1.0));
        // Outward facing: viewed from outside, k1 is to the left of k.
        m.quad(b0, t0, t1, b1);
        m.tri(t0, tip, t1);
        m.tri(b1, bot, b0);
    }
    m
}

/// A ring in the xy plane, radius 1, tube `tube`, facing +-z.
pub fn torus(tube: f32) -> Mesh {
    let mut m = Mesh::default();
    let (seg, side) = (40usize, 10usize);
    for i in 0..=seg {
        let a = i as f32 / seg as f32 * TAU;
        let (sa, ca) = a.sin_cos();
        for j in 0..=side {
            let b = j as f32 / side as f32 * TAU;
            let (sb, cb) = b.sin_cos();
            let n = [ca * cb, sa * cb, sb];
            let p = [ca * (1.0 + tube * cb), sa * (1.0 + tube * cb), tube * sb];
            m.vert(p, n);
        }
    }
    let row = (side + 1) as u16;
    for i in 0..seg as u16 {
        for j in 0..side as u16 {
            let a = i * row + j;
            let b = (i + 1) * row + j;
            m.idx.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
        }
    }
    m
}

/// The glider: a swept delta wing with a raised spine and a cockpit
/// ridge, nose toward -z, about 1.2 wide and 1.6 long.
pub fn glider() -> Mesh {
    let mut m = Mesh::default();
    let nose = [0.0, 0.0, -0.95];
    let tail = [0.0, 0.05, 0.6];
    let spine = [0.0, 0.26, 0.15];
    let lw = [-0.75, -0.04, 0.62];
    let rw = [0.75, -0.04, 0.62];
    let lt = [-0.95, 0.06, 0.78];
    let rt = [0.95, 0.06, 0.78];
    let belly = [0.0, -0.14, 0.2];
    // Top surfaces.
    m.tri(nose, spine, lw);
    m.tri(nose, rw, spine);
    m.tri(spine, tail, lw);
    m.tri(spine, rw, tail);
    // Wing tips turned up.
    m.tri(lw, tail, lt);
    m.tri(rw, rt, tail);
    m.tri(lw, lt, belly);
    m.tri(rw, belly, rt);
    // Underside.
    m.tri(nose, lw, belly);
    m.tri(nose, belly, rw);
    m.tri(belly, lt, tail);
    m.tri(belly, tail, rt);
    m
}

/// One canyon segment, `len` long along -z from z=0: left wall, floor,
/// right wall, as a cross-section polyline extruded. Faces point inward.
pub fn canyon(len: f32, half_w: f32, floor_y: f32, top_y: f32) -> Mesh {
    let mut m = Mesh::default();
    // Cross-section from the top of the left wall around to the top of the
    // right one, with a few kinks so the walls read as rock, not a box.
    let pts: [[f32; 2]; 9] = [
        [-half_w - 2.5, top_y],
        [-half_w - 0.6, top_y * 0.45],
        [-half_w, floor_y + 3.0],
        [-half_w + 1.6, floor_y + 0.4],
        [-half_w + 4.5, floor_y],
        [half_w - 4.5, floor_y],
        [half_w - 1.6, floor_y + 0.4],
        [half_w, floor_y + 3.0],
        [half_w + 0.6, top_y * 0.45],
    ];
    let mut all: Vec<[f32; 2]> = pts.to_vec();
    all.push([half_w + 2.5, top_y]);
    for w in all.windows(2) {
        let (p, q) = (w[0], w[1]);
        let a = [p[0], p[1], 0.0];
        let b = [q[0], q[1], 0.0];
        let c = [q[0], q[1], -len];
        let d = [p[0], p[1], -len];
        // Seen from inside the canyon (from above the floor), a -> b runs
        // left to right, so this winding faces inward.
        m.quad(a, b, c, d);
    }
    m
}
