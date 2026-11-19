//! The five fields: where the flock starts, where the fold is, and what is in
//! the way. Ground coordinates are (x, z) with +z toward the camera.

use bevy::math::Vec2;

/// Which side of the fold its gate is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    North,
    South,
    East,
    West,
}

impl Side {
    /// Unit vector pointing out of the fold through the gate.
    pub fn out(self) -> Vec2 {
        match self {
            Side::North => Vec2::new(0.0, -1.0),
            Side::South => Vec2::new(0.0, 1.0),
            Side::East => Vec2::new(1.0, 0.0),
            Side::West => Vec2::new(-1.0, 0.0),
        }
    }
}

pub struct FieldDef {
    pub name: &'static str,
    /// Half the size of the walled field.
    pub half: (f32, f32),
    /// Flock clusters: (x, z, sheep).
    pub flock: &'static [(f32, f32, usize)],
    pub need: usize,
    /// Seconds of daylight.
    pub secs: f32,
    pub dog: (f32, f32),
    pub pen: (f32, f32),
    pub gate: Side,
    /// Dry-stone walls (x0, z0, x1, z1).
    pub walls: &'static [(f32, f32, f32, f32)],
    /// Hedgerows (x0, z0, x1, z1).
    pub hedges: &'static [(f32, f32, f32, f32)],
    /// Brook water, impassable (x0, z0, x1, z1 rectangles).
    pub water: &'static [(f32, f32, f32, f32)],
    /// Bridges over the brook (x0, z0, x1, z1), drawn only.
    pub bridges: &'static [(f32, f32, f32, f32)],
    pub trees: &'static [(f32, f32)],
    pub rocks: &'static [(f32, f32)],
    /// What the field-start card says.
    pub hint: &'static str,
}

impl FieldDef {
    pub fn sheep(&self) -> usize {
        self.flock.iter().map(|c| c.2).sum()
    }
}

/// Half the size of every fold.
pub const PEN_HALF: Vec2 = Vec2::new(4.0, 3.2);
/// Width of the gate opening.
pub const GATE_W: f32 = 4.2;

pub const FIELDS: [FieldDef; 5] = [
    FieldDef {
        name: "Home Meadow",
        half: (24.0, 16.0),
        flock: &[(-13.0, 3.0, 12)],
        need: 10,
        secs: 180.0,
        dog: (-3.0, 9.0),
        pen: (13.0, -5.0),
        gate: Side::West,
        walls: &[],
        hedges: &[],
        water: &[],
        bridges: &[],
        trees: &[(-3.0, -10.0), (5.0, 10.0), (-19.0, -11.0), (20.0, 11.0), (-20.0, 12.0)],
        rocks: &[(2.0, -3.0), (-8.0, 11.0), (10.0, 6.0)],
        hint: "Run behind the flock to drive it into the fold.",
    },
    FieldDef {
        name: "Stone Rows",
        half: (30.0, 18.0),
        flock: &[(-21.0, -9.0, 18)],
        need: 14,
        secs: 240.0,
        dog: (-24.0, 10.0),
        pen: (19.0, 8.0),
        gate: Side::West,
        walls: &[(-9.0, -18.0, -9.0, 5.0), (7.0, -5.0, 7.0, 18.0)],
        hedges: &[],
        water: &[],
        bridges: &[],
        trees: &[(-16.0, 12.0), (-2.0, -12.0), (0.0, 12.0), (16.0, -11.0), (26.0, -14.0)],
        rocks: &[(-20.0, 4.0), (14.0, 0.0), (-3.0, 0.0)],
        hint: "Walk them through the gaps in the stone walls.",
    },
    FieldDef {
        name: "Brookside",
        half: (30.0, 20.0),
        flock: &[(-12.0, -12.0, 24)],
        need: 19,
        secs: 240.0,
        dog: (-22.0, -16.0),
        pen: (0.0, 13.5),
        gate: Side::North,
        walls: &[],
        hedges: &[],
        water: &[(-30.0, -1.3, -18.0, 1.3), (-14.0, -1.3, 12.0, 1.3), (16.0, -1.3, 30.0, 1.3)],
        bridges: &[(-18.0, -1.6, -14.0, 1.6), (12.0, -1.6, 16.0, 1.6)],
        trees: &[(-24.0, -5.0), (-8.0, -4.0), (6.0, 4.0), (22.0, -5.0), (-26.0, 14.0), (24.0, 15.0), (10.0, -15.0)],
        rocks: &[(-4.0, -9.0), (18.0, -10.0), (-14.0, 8.0)],
        hint: "Sheep will not swim: cross the brook on a bridge.",
    },
    FieldDef {
        name: "Hedgerows",
        half: (30.0, 20.0),
        flock: &[(-19.0, -14.0, 28)],
        need: 21,
        secs: 300.0,
        dog: (-27.0, -17.0),
        pen: (19.0, 16.2),
        gate: Side::West,
        walls: &[],
        hedges: &[(-30.0, -7.0, 15.0, -7.0), (-15.0, 6.0, 30.0, 6.0)],
        water: &[],
        bridges: &[],
        trees: &[(0.0, -15.0), (-24.0, 0.0), (24.0, -1.0), (-6.0, 15.0), (6.0, 1.0)],
        rocks: &[(10.0, -13.0), (-12.0, 0.0), (0.0, 12.0)],
        hint: "Follow the lanes between the hedges.",
    },
    FieldDef {
        name: "High Pasture",
        half: (32.0, 21.0),
        flock: &[(-23.0, -13.0, 18), (23.0, -13.0, 18)],
        need: 28,
        secs: 320.0,
        dog: (0.0, -15.0),
        pen: (0.0, 17.0),
        gate: Side::North,
        walls: &[(-16.0, 1.0, -7.0, 1.0), (7.0, 1.0, 16.0, 1.0)],
        hedges: &[],
        water: &[],
        bridges: &[],
        trees: &[(-28.0, 5.0), (28.0, 5.0), (-12.0, -9.0), (12.0, -9.0), (0.0, -3.0), (-20.0, 16.0), (20.0, 16.0)],
        rocks: &[(-4.0, -12.0), (5.0, -16.0), (-24.0, 2.0), (24.0, 2.0)],
        hint: "Two flocks: bring them together, then home.",
    },
];
