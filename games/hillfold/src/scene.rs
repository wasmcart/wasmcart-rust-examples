//! Everything you see: meshes and materials, the field's walls, hedges,
//! brook, trees and fold, the sheep and the dog, the sun and the sky.

use bevy::asset::RenderAssetUsages;
use bevy::light::{CascadeShadowConfigBuilder, NotShadowCaster};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use crate::fields::{FieldDef, GATE_W, PEN_HALF};
use crate::world::{Obstacles, gate_ends, gate_mid, pen_segments};
use crate::{Dog, FieldEnt, Game, Mode, Rng, Sheep};

#[derive(Resource)]
pub struct Kit {
    sheep_body: Handle<Mesh>,
    sheep_puff: Handle<Mesh>,
    sheep_head: Handle<Mesh>,
    ear: Handle<Mesh>,
    leg: Handle<Mesh>,
    wool: [Handle<StandardMaterial>; 3],
    face: Handle<StandardMaterial>,
    hoof: Handle<StandardMaterial>,
    dog_body: Handle<Mesh>,
    dog_head: Handle<Mesh>,
    dog_chest: Handle<Mesh>,
    snout: Handle<Mesh>,
    dog_ear: Handle<Mesh>,
    tail: Handle<Mesh>,
    dog_leg: Handle<Mesh>,
    nose: Handle<Mesh>,
    dog_dark: Handle<StandardMaterial>,
    dog_light: Handle<StandardMaterial>,
    black: Handle<StandardMaterial>,
    unit_cube: Handle<Mesh>,
    wood: Handle<StandardMaterial>,
    wood_dark: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
    mud: Handle<StandardMaterial>,
    unit_plane: Handle<Mesh>,
    trunk: Handle<Mesh>,
    bark: Handle<StandardMaterial>,
    canopy: Handle<Mesh>,
    leaf: [Handle<StandardMaterial>; 3],
    ring: Handle<Mesh>,
    beacon: Handle<Mesh>,
    beacon_mat: Handle<StandardMaterial>,
    chevron_mat: Handle<StandardMaterial>,
    lamp: Handle<Mesh>,
    lamp_mat: Handle<StandardMaterial>,
    batch_mat: Handle<StandardMaterial>,
}

#[derive(Component)]
pub struct Sun;
#[derive(Component)]
pub struct Lantern;
#[derive(Component)]
pub struct Beacon;
#[derive(Component)]
pub struct Chevron;
#[derive(Component)]
pub struct GatePivot {
    open: f32,
    closed: f32,
    angle: f32,
}
#[derive(Component)]
pub struct BarkRing {
    t: f32,
    mat: Handle<StandardMaterial>,
}
#[derive(Component)]
pub struct Puff;

fn mat(mats: &mut Assets<StandardMaterial>, c: Color, rough: f32) -> Handle<StandardMaterial> {
    mats.add(StandardMaterial { base_color: c, perceptual_roughness: rough, reflectance: 0.3, ..default() })
}

pub fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>, mut rng: ResMut<Rng>) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection { fov: 0.72, ..default() }),
        Transform::from_xyz(0.0, 20.0, 18.0).looking_at(Vec3::ZERO, Vec3::Y),
        DistanceFog {
            color: Color::srgb(0.62, 0.76, 0.9),
            directional_light_color: Color::srgba(1.0, 0.85, 0.6, 0.35),
            directional_light_exponent: 20.0,
            falloff: FogFalloff::Linear { start: 55.0, end: 150.0 },
        },
    ));
    commands.spawn((
        Sun,
        DirectionalLight { illuminance: 9000.0, shadow_maps_enabled: true, ..default() },
        Transform::from_xyz(10.0, 20.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { num_cascades: 2, first_cascade_far_bound: 32.0, maximum_distance: 80.0, ..default() }.build(),
    ));

    // the ground: one mesh, its grass shades in the vertex colours
    commands.spawn((
        Mesh3d(meshes.add(ground_mesh(&mut rng))),
        MeshMaterial3d(mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, reflectance: 0.2, ..default() })),
    ));
    // far hills and woods beyond the walls
    let hill = meshes.add(Sphere::new(1.0).mesh().uv(32, 16));
    let hill_mat = [mat(&mut mats, Color::srgb(0.30, 0.48, 0.22), 0.95), mat(&mut mats, Color::srgb(0.36, 0.52, 0.25), 0.95)];
    for i in 0..18 {
        let a = i as f32 / 18.0 * std::f32::consts::TAU + rng.range(-0.1, 0.1);
        let r = rng.range(62.0, 80.0);
        let s = Vec3::new(rng.range(16.0, 28.0), rng.range(5.0, 11.0), rng.range(14.0, 22.0));
        commands.spawn((
            Mesh3d(hill.clone()),
            MeshMaterial3d(hill_mat[i % 2].clone()),
            Transform::from_xyz(a.cos() * r * 1.3, -1.0, a.sin() * r).with_scale(s),
            NotShadowCaster,
        ));
    }

    let wool = [Color::srgb(0.93, 0.92, 0.88), Color::srgb(0.88, 0.85, 0.78), Color::srgb(0.95, 0.94, 0.92)];
    let kit = Kit {
        sheep_body: meshes.add(Sphere::new(0.5).mesh().uv(20, 12)),
        sheep_puff: meshes.add(Sphere::new(0.3).mesh().uv(12, 8)),
        sheep_head: meshes.add(Sphere::new(0.2).mesh().uv(14, 8)),
        ear: meshes.add(Cuboid::new(0.2, 0.06, 0.1)),
        leg: meshes.add(Cuboid::new(0.11, 0.5, 0.11)),
        wool: wool.map(|c| mat(&mut mats, c, 0.97)),
        face: mat(&mut mats, Color::srgb(0.17, 0.15, 0.14), 0.7),
        hoof: mat(&mut mats, Color::srgb(0.12, 0.11, 0.1), 0.7),
        dog_body: meshes.add(Capsule3d::new(0.2, 0.62).mesh().latitudes(8).longitudes(16)),
        dog_head: meshes.add(Sphere::new(0.17).mesh().uv(14, 8)),
        dog_chest: meshes.add(Sphere::new(0.17).mesh().uv(12, 8)),
        snout: meshes.add(Cuboid::new(0.12, 0.1, 0.2)),
        dog_ear: meshes.add(Cone { radius: 0.06, height: 0.15 }),
        tail: meshes.add(Capsule3d::new(0.05, 0.32)),
        dog_leg: meshes.add(Cuboid::new(0.08, 0.42, 0.08)),
        nose: meshes.add(Sphere::new(0.035).mesh().uv(8, 4)),
        dog_dark: mat(&mut mats, Color::srgb(0.13, 0.1, 0.09), 0.65),
        dog_light: mat(&mut mats, Color::srgb(0.95, 0.9, 0.82), 0.7),
        black: mat(&mut mats, Color::srgb(0.02, 0.02, 0.02), 0.3),
        unit_cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        wood: mat(&mut mats, Color::srgb(0.55, 0.38, 0.22), 0.8),
        wood_dark: mat(&mut mats, Color::srgb(0.38, 0.25, 0.14), 0.85),
        water: mats.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.38, 0.52),
            perceptual_roughness: 0.06,
            reflectance: 0.6,
            ..default()
        }),
        mud: mat(&mut mats, Color::srgb(0.33, 0.27, 0.18), 0.95),
        unit_plane: meshes.add(Plane3d::default().mesh().size(1.0, 1.0)),
        trunk: meshes.add(Cylinder::new(0.2, 2.2)),
        bark: mat(&mut mats, Color::srgb(0.33, 0.24, 0.16), 0.9),
        canopy: meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap()),
        leaf: [
            mat(&mut mats, Color::srgb(0.16, 0.36, 0.13), 0.85),
            mat(&mut mats, Color::srgb(0.22, 0.43, 0.16), 0.85),
            mat(&mut mats, Color::srgb(0.28, 0.47, 0.18), 0.85),
        ],
        ring: meshes.add(Torus::new(0.92, 1.0)),
        beacon: meshes.add(Cone { radius: 0.45, height: 0.9 }),
        beacon_mat: mats.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.25),
            emissive: LinearRgba::rgb(2.0, 1.4, 0.3),
            ..default()
        }),
        chevron_mat: mats.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.85, 0.3, 0.75),
            emissive: LinearRgba::rgb(0.6, 0.45, 0.1),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        lamp: meshes.add(Sphere::new(0.16).mesh().uv(10, 6)),
        batch_mat: mat(&mut mats, Color::WHITE, 0.9),
        lamp_mat: mats.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.55),
            emissive: LinearRgba::rgb(4.0, 2.6, 1.0),
            ..default()
        }),
    };
    commands.insert_resource(kit);
}

fn ground_mesh(rng: &mut Rng) -> Mesh {
    let (n, size) = (100usize, 220.0f32);
    let mut pos = Vec::new();
    let mut nor = Vec::new();
    let mut uv = Vec::new();
    let mut col = Vec::new();
    // a few smooth blobs of colour: meadow patches
    let blobs: Vec<(Vec2, f32, f32)> =
        (0..40).map(|_| (Vec2::new(rng.range(-80.0, 80.0), rng.range(-60.0, 60.0)), rng.range(5.0, 16.0), rng.range(-1.0, 1.0))).collect();
    for z in 0..=n {
        for x in 0..=n {
            let p = Vec2::new(x as f32 / n as f32 - 0.5, z as f32 / n as f32 - 0.5) * size;
            pos.push([p.x, 0.0, p.y]);
            nor.push([0.0, 1.0, 0.0]);
            uv.push([x as f32 / n as f32, z as f32 / n as f32]);
            let mut k = 0.0;
            for (c, r, w) in &blobs {
                let d = p.distance(*c) / r;
                if d < 1.0 {
                    k += w * (1.0 - d * d);
                }
            }
            let k = k.clamp(-1.0, 1.0) * 0.5 + rng.range(-0.06, 0.06);
            col.push([0.27 + 0.07 * k, 0.48 + 0.08 * k, 0.17 + 0.03 * k, 1.0]);
        }
    }
    let mut idx = Vec::new();
    for z in 0..n {
        for x in 0..n {
            let i = (z * (n + 1) + x) as u32;
            let j = i + n as u32 + 1;
            idx.extend_from_slice(&[i, j, i + 1, i + 1, j, j + 1]);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nor)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

/// Static scenery merged into one vertex-coloured mesh: hundreds of blocks,
/// posts and grass tufts become one draw instead of one each.
#[derive(Default)]
pub struct Batch {
    mesh: Option<Mesh>,
}

impl Batch {
    pub fn add(&mut self, m: Mesh, t: Transform, c: Color) {
        let n = m.count_vertices();
        let lin = c.to_linear().to_f32_array();
        let mut m = m.transformed_by(t);
        m.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![lin; n]);
        match &mut self.mesh {
            None => self.mesh = Some(m),
            Some(acc) => {
                if let Err(e) = acc.merge(&m) {
                    wasmcart::wc_log(&format!("hillfold: scenery mesh not merged: {e}"));
                }
            }
        }
    }
    fn spawn(self, commands: &mut Commands, meshes: &mut Assets<Mesh>, mat: Handle<StandardMaterial>, shadows: bool) {
        if let Some(m) = self.mesh {
            let mut e = commands.spawn((FieldEnt, Mesh3d(meshes.add(m)), MeshMaterial3d(mat)));
            if !shadows {
                e.insert(NotShadowCaster);
            }
        }
    }
}

const STONE: [Color; 3] = [Color::srgb(0.55, 0.53, 0.5), Color::srgb(0.47, 0.46, 0.44), Color::srgb(0.62, 0.6, 0.55)];
const HEDGE: [Color; 2] = [Color::srgb(0.13, 0.33, 0.12), Color::srgb(0.17, 0.38, 0.14)];
const WOOD: Color = Color::srgb(0.55, 0.38, 0.22);
const WOOD_DARK: Color = Color::srgb(0.38, 0.25, 0.14);
const ROCK: Color = Color::srgb(0.5, 0.5, 0.48);
const GRASS: [Color; 3] = [Color::srgb(0.28, 0.5, 0.18), Color::srgb(0.36, 0.56, 0.2), Color::srgb(0.45, 0.6, 0.24)];
const FLOWER: [Color; 4] = [Color::srgb(1.0, 0.88, 0.25), Color::srgb(0.98, 0.98, 0.95), Color::srgb(0.7, 0.5, 0.95), Color::srgb(0.95, 0.45, 0.4)];

fn spawn_block(batch: &mut Batch, color: Color, c: Vec3, size: Vec3, yaw: f32) {
    batch.add(Cuboid::new(1.0, 1.0, 1.0).into(), Transform::from_translation(c).with_rotation(Quat::from_rotation_y(yaw)).with_scale(size), color);
}

/// A dry-stone wall from a to b: irregular blocks.
fn stone_wall(batch: &mut Batch, rng: &mut Rng, a: Vec2, b: Vec2, height: f32) {
    let len = a.distance(b);
    let dir = (b - a) / len.max(1e-3);
    let yaw = -dir.y.atan2(dir.x);
    let n = (len / 1.4).ceil().max(1.0) as usize;
    for i in 0..n {
        let t0 = i as f32 / n as f32;
        let t1 = (i + 1) as f32 / n as f32;
        let mid = a + dir * len * (t0 + t1) * 0.5;
        let h = height * rng.range(0.85, 1.1);
        let l = len / n as f32 + 0.05;
        let m = STONE[(rng.f() * 3.0) as usize % 3];
        spawn_block(batch, m, Vec3::new(mid.x, h * 0.5, mid.y), Vec3::new(l, h, rng.range(0.7, 0.85)), yaw);
    }
}

fn hedge(batch: &mut Batch, rng: &mut Rng, a: Vec2, b: Vec2) {
    let len = a.distance(b);
    let dir = (b - a) / len.max(1e-3);
    let yaw = -dir.y.atan2(dir.x);
    let n = (len / 2.0).ceil().max(1.0) as usize;
    for i in 0..n {
        let t = (i as f32 + 0.5) / n as f32;
        let mid = a + dir * len * t;
        let h = rng.range(1.3, 1.6);
        let m = HEDGE[i % 2];
        spawn_block(batch, m, Vec3::new(mid.x, h * 0.5, mid.y), Vec3::new(len / n as f32 + 0.2, h, rng.range(1.1, 1.3)), yaw);
    }
}

pub fn spawn_field(commands: &mut Commands, meshes: &mut Assets<Mesh>, kit: &Kit, def: &FieldDef, obs: &Obstacles, rng: &mut Rng) {
    let half = Vec2::new(def.half.0, def.half.1);
    let mut solid = Batch::default();
    let mut grass = Batch::default();
    // the field wall
    let c = [Vec2::new(-half.x, -half.y), Vec2::new(half.x, -half.y), Vec2::new(half.x, half.y), Vec2::new(-half.x, half.y)];
    for i in 0..4 {
        stone_wall(&mut solid, rng, c[i], c[(i + 1) % 4], 0.9);
    }
    for &(x0, z0, x1, z1) in def.walls {
        stone_wall(&mut solid, rng, Vec2::new(x0, z0), Vec2::new(x1, z1), 1.0);
    }
    for &(x0, z0, x1, z1) in def.hedges {
        hedge(&mut solid, rng, Vec2::new(x0, z0), Vec2::new(x1, z1));
    }
    // the brook: banks, water, bridges
    for &(x0, z0, x1, z1) in def.water {
        let (cx, cz, w, d) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5, (x1 - x0).abs(), (z1 - z0).abs());
        commands.spawn((FieldEnt, Mesh3d(kit.unit_plane.clone()), MeshMaterial3d(kit.mud.clone()),
            Transform::from_xyz(cx, 0.012, cz).with_scale(Vec3::new(w + 0.6, 1.0, d + 0.7))));
        commands.spawn((FieldEnt, Mesh3d(kit.unit_plane.clone()), MeshMaterial3d(kit.water.clone()),
            Transform::from_xyz(cx, 0.03, cz).with_scale(Vec3::new(w, 1.0, d))));
    }
    for &(x0, z0, x1, z1) in def.bridges {
        let (cx, cz, w, d) = ((x0 + x1) * 0.5, (z0 + z1) * 0.5, (x1 - x0).abs(), (z1 - z0).abs());
        commands.spawn((FieldEnt, Mesh3d(kit.unit_plane.clone()), MeshMaterial3d(kit.water.clone()),
            Transform::from_xyz(cx, 0.03, cz).with_scale(Vec3::new(w, 1.0, d - 0.6))));
        // planks across the water
        let n = (d / 0.45) as usize;
        for i in 0..n {
            let z = z0.min(z1) + (i as f32 + 0.5) * d / n as f32;
            spawn_block(&mut solid, if i % 2 == 0 { WOOD } else { WOOD_DARK },
                Vec3::new(cx, 0.12, z), Vec3::new(w - 0.2, 0.12, d / n as f32 - 0.04), 0.0);
        }
        for sx in [-1.0, 1.0] {
            spawn_block(&mut solid, WOOD_DARK, Vec3::new(cx + sx * (w * 0.5 - 0.1), 0.6, cz), Vec3::new(0.12, 0.1, d), 0.0);
            for z in [z0.min(z1) + 0.1, z0.max(z1) - 0.1] {
                spawn_block(&mut solid, WOOD_DARK, Vec3::new(cx + sx * (w * 0.5 - 0.1), 0.33, z), Vec3::new(0.14, 0.6, 0.14), 0.0);
            }
        }
    }
    // trees
    for &(x, z) in def.trees {
        commands
            .spawn((FieldEnt, Transform::from_xyz(x, 0.0, z).with_rotation(Quat::from_rotation_y(rng.range(0.0, 6.0))), Visibility::default()))
            .with_children(|p| {
                p.spawn((Mesh3d(kit.trunk.clone()), MeshMaterial3d(kit.bark.clone()), Transform::from_xyz(0.0, 1.1, 0.0)));
                for k in 0..4 {
                    let a = k as f32 * 1.7;
                    let r = if k == 0 { 0.0 } else { 0.9 };
                    let s = if k == 0 { 1.9 } else { rng.range(1.2, 1.5) };
                    p.spawn((
                        Mesh3d(kit.canopy.clone()),
                        MeshMaterial3d(kit.leaf[k % 3].clone()),
                        Transform::from_xyz(a.cos() * r, if k == 0 { 3.6 } else { 3.0 + rng.range(0.0, 0.8) }, a.sin() * r)
                            .with_scale(Vec3::new(s, s * 0.85, s)),
                    ));
                }
            });
    }
    for &(x, z) in def.rocks {
        let t = Transform::from_xyz(x, 0.1, z)
            .with_rotation(Quat::from_rotation_y(rng.range(0.0, 6.0)))
            .with_scale(Vec3::new(0.95, 0.55, 0.75));
        solid.add(Sphere::new(1.0).mesh().ico(1).unwrap(), t, ROCK);
    }

    // the fold: posts and rails, a gate standing open, a lantern, the beacon
    let pen = Vec2::new(def.pen.0, def.pen.1);
    for (a, b) in pen_segments(pen, def.gate) {
        let len = a.distance(b);
        let dir = (b - a) / len.max(1e-3);
        let yaw = -dir.y.atan2(dir.x);
        let n = (len / 1.6).ceil().max(1.0) as usize;
        for i in 0..=n {
            let p = a + dir * len * i as f32 / n as f32;
            solid.add(Cylinder::new(0.09, 1.3).into(), Transform::from_xyz(p.x, 0.65, p.y), WOOD_DARK);
        }
        let mid = (a + b) * 0.5;
        for y in [0.5, 0.98] {
            spawn_block(&mut solid, WOOD, Vec3::new(mid.x, y, mid.y), Vec3::new(len, 0.1, 0.08), yaw);
        }
    }
    let (ga, gb) = gate_ends(pen, def.gate);
    let gdir = (gb - ga).normalize();
    let out = def.gate.out();
    let closed = -gdir.y.atan2(gdir.x);
    // open: swung out of the fold, a wing that funnels sheep in
    let open = -out.y.atan2(out.x);
    commands
        .spawn((
            FieldEnt,
            GatePivot { open, closed, angle: open },
            Transform::from_xyz(ga.x, 0.0, ga.y).with_rotation(Quat::from_rotation_y(open)),
            Visibility::default(),
        ))
        .with_children(|p| {
            for y in [0.4, 0.75, 1.1] {
                p.spawn((Mesh3d(kit.unit_cube.clone()), MeshMaterial3d(kit.wood.clone()),
                    Transform::from_xyz(GATE_W * 0.5, y, 0.0).with_scale(Vec3::new(GATE_W - 0.1, 0.09, 0.07))));
            }
            for x in [0.1, GATE_W * 0.5, GATE_W - 0.1] {
                p.spawn((Mesh3d(kit.unit_cube.clone()), MeshMaterial3d(kit.wood_dark.clone()),
                    Transform::from_xyz(x, 0.75, 0.0).with_scale(Vec3::new(0.09, 0.9, 0.08))));
            }
        });
    let lamp_at = gb + out * 0.35;
    solid.add(Cylinder::new(0.09, 1.3).into(), Transform::from_xyz(lamp_at.x, 1.1, lamp_at.y).with_scale(Vec3::new(0.8, 1.7, 0.8)), WOOD_DARK);
    commands.spawn((FieldEnt, Mesh3d(kit.lamp.clone()), MeshMaterial3d(kit.lamp_mat.clone()), Transform::from_xyz(lamp_at.x, 2.35, lamp_at.y), NotShadowCaster));
    commands.spawn((
        FieldEnt,
        Lantern,
        PointLight { color: Color::srgb(1.0, 0.75, 0.45), intensity: 0.0, range: 14.0, shadow_maps_enabled: false, ..default() },
        Transform::from_xyz(lamp_at.x, 2.5, lamp_at.y),
    ));
    let gm = gate_mid(pen, def.gate) + out * 1.2;
    commands.spawn((
        FieldEnt,
        Beacon,
        Mesh3d(kit.beacon.clone()),
        MeshMaterial3d(kit.beacon_mat.clone()),
        Transform::from_xyz(gm.x, 3.4, gm.y).with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
        NotShadowCaster,
    ));

    // grass and flowers where nothing else stands
    let mut placed = 0;
    let mut tries = 0;
    while placed < 520 && tries < 4000 {
        tries += 1;
        let p = Vec2::new(rng.range(-half.x + 1.0, half.x - 1.0), rng.range(-half.y + 1.0, half.y - 1.0));
        if obs.clearance(p) < 0.4 || obs.in_pen(p, -0.6) {
            continue;
        }
        placed += 1;
        let flower = placed % 4 == 0;
        let s = rng.range(0.7, 1.4);
        let (mesh, c, y): (Mesh, Color, f32) = if flower {
            (Sphere::new(0.07).mesh().uv(8, 4), FLOWER[placed / 4 % 4], 0.12)
        } else {
            (Cone { radius: 0.07, height: 0.42 }.into(), GRASS[placed % 3], 0.2 * s)
        };
        let t = Transform::from_xyz(p.x, y, p.y)
            .with_rotation(Quat::from_rotation_z(rng.range(-0.25, 0.25)))
            .with_scale(Vec3::splat(s));
        grass.add(mesh, t, c);
    }
    // a scatter of grass outside the walls too
    for _ in 0..140 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(1.0, 1.6);
        let p = Vec2::new(a.cos() * half.x * r, a.sin() * half.y * r);
        grass.add(Cone { radius: 0.07, height: 0.42 }.into(), Transform::from_xyz(p.x, 0.25, p.y).with_scale(Vec3::splat(1.3)), GRASS[(r * 10.0) as usize % 3]);
    }
    solid.spawn(commands, meshes, kit.batch_mat.clone(), true);
    grass.spawn(commands, meshes, kit.batch_mat.clone(), false);

    // the flock
    let mut k = 0;
    for &(fx, fz, n) in def.flock {
        for i in 0..n {
            let a = i as f32 * 2.399 + rng.range(0.0, 0.5);
            let r = (i as f32).sqrt() * 0.95;
            let p = obs.push_out(Vec2::new(fx + a.cos() * r, fz + a.sin() * r), 0.45);
            spawn_sheep(commands, kit, rng, p, k);
            k += 1;
        }
    }
    spawn_dog(commands, kit, Vec2::new(def.dog.0, def.dog.1));
    commands.spawn((
        FieldEnt,
        Chevron,
        Mesh3d(kit.beacon.clone()),
        MeshMaterial3d(kit.chevron_mat.clone()),
        Transform::from_xyz(0.0, -5.0, 0.0),
        NotShadowCaster,
    ));
}

fn spawn_sheep(commands: &mut Commands, kit: &Kit, rng: &mut Rng, p: Vec2, k: usize) {
    let wool = kit.wool[k % 3].clone();
    let s = rng.range(0.9, 1.08);
    let heading = rng.range(0.0, std::f32::consts::TAU);
    let mut legs = [Entity::PLACEHOLDER; 4];
    let mut head = Entity::PLACEHOLDER;
    let root = commands
        .spawn((FieldEnt, Transform::from_xyz(p.x, 0.0, p.y).with_scale(Vec3::splat(s)), Visibility::default()))
        .with_children(|c| {
            c.spawn((Mesh3d(kit.sheep_body.clone()), MeshMaterial3d(wool.clone()),
                Transform::from_xyz(0.0, 0.78, 0.0).with_scale(Vec3::new(1.0, 0.86, 1.3))));
            c.spawn((Puff, Mesh3d(kit.sheep_puff.clone()), MeshMaterial3d(wool.clone()), Transform::from_xyz(0.0, 1.06, -0.18)));
            c.spawn((Puff, Mesh3d(kit.sheep_puff.clone()), MeshMaterial3d(wool.clone()), Transform::from_xyz(0.05, 1.02, 0.22).with_scale(Vec3::splat(0.9))));
            head = c
                .spawn((Mesh3d(kit.sheep_head.clone()), MeshMaterial3d(kit.face.clone()),
                    Transform::from_xyz(0.0, 0.98, 0.72).with_scale(Vec3::new(0.9, 1.0, 1.3))))
                .with_children(|h| {
                    for sx in [-1.0, 1.0] {
                        h.spawn((Mesh3d(kit.ear.clone()), MeshMaterial3d(kit.face.clone()),
                            Transform::from_xyz(sx * 0.2, 0.06, -0.05).with_rotation(Quat::from_rotation_z(sx * -0.4))));
                    }
                })
                .id();
            for (i, (lx, lz)) in [(-0.24, 0.38), (0.24, 0.38), (-0.24, -0.38), (0.24, -0.38)].into_iter().enumerate() {
                legs[i] = c.spawn((Mesh3d(kit.leg.clone()), MeshMaterial3d(kit.hoof.clone()), Transform::from_xyz(lx, 0.27, lz))).id();
            }
        })
        .id();
    commands.entity(root).insert(Sheep {
        pos: p,
        vel: Vec2::ZERO,
        alarm: 0.0,
        panic: 0.0,
        penned: false,
        phase: rng.range(0.0, 6.0),
        hop: 0.0,
        graze: rng.range(0.0, 3.0),
        wander: Vec2::ZERO,
        heading,
        legs,
        head,
        baa_cd: rng.range(1.0, 6.0),
    });
}

fn spawn_dog(commands: &mut Commands, kit: &Kit, p: Vec2) {
    let mut legs = [Entity::PLACEHOLDER; 4];
    let mut tail = Entity::PLACEHOLDER;
    let root = commands
        .spawn((FieldEnt, Transform::from_xyz(p.x, 0.0, p.y), Visibility::default()))
        .with_children(|c| {
            c.spawn((Mesh3d(kit.dog_body.clone()), MeshMaterial3d(kit.dog_dark.clone()),
                Transform::from_xyz(0.0, 0.56, 0.0).with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
            c.spawn((Mesh3d(kit.dog_chest.clone()), MeshMaterial3d(kit.dog_light.clone()), Transform::from_xyz(0.0, 0.52, 0.36)));
            c.spawn((Mesh3d(kit.dog_head.clone()), MeshMaterial3d(kit.dog_dark.clone()), Transform::from_xyz(0.0, 0.84, 0.5)));
            c.spawn((Mesh3d(kit.snout.clone()), MeshMaterial3d(kit.dog_light.clone()), Transform::from_xyz(0.0, 0.79, 0.68)));
            c.spawn((Mesh3d(kit.nose.clone()), MeshMaterial3d(kit.black.clone()), Transform::from_xyz(0.0, 0.82, 0.79)));
            for sx in [-1.0f32, 1.0] {
                c.spawn((Mesh3d(kit.dog_ear.clone()), MeshMaterial3d(kit.dog_dark.clone()),
                    Transform::from_xyz(sx * 0.09, 1.0, 0.46).with_rotation(Quat::from_rotation_z(sx * -0.3))));
            }
            tail = c
                .spawn((Mesh3d(kit.tail.clone()), MeshMaterial3d(kit.dog_dark.clone()),
                    Transform::from_xyz(0.0, 0.7, -0.5).with_rotation(Quat::from_rotation_x(-0.7))))
                .id();
            for (i, (lx, lz)) in [(-0.12, 0.26), (0.12, 0.26), (-0.12, -0.26), (0.12, -0.26)].into_iter().enumerate() {
                legs[i] = c
                    .spawn((Mesh3d(kit.dog_leg.clone()), MeshMaterial3d(if i < 2 { kit.dog_light.clone() } else { kit.dog_dark.clone() }),
                        Transform::from_xyz(lx, 0.21, lz)))
                    .id();
            }
        })
        .id();
    commands.entity(root).insert(Dog { pos: p, vel: Vec2::ZERO, heading: 0.0, phase: 0.0, legs, tail, bark_cd: 0.0, sneak: false });
}

pub fn spawn_bark_ring(commands: &mut Commands, kit: &Kit, mats: &mut Assets<StandardMaterial>, at: Vec2) {
    let m = mats.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.9, 0.6, 0.6),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.spawn((
        FieldEnt,
        BarkRing { t: 0.0, mat: m.clone() },
        Mesh3d(kit.ring.clone()),
        MeshMaterial3d(m),
        Transform::from_xyz(at.x, 0.15, at.y),
        NotShadowCaster,
    ));
}

/// Pose every animated thing from the simulation.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn animate(
    mut commands: Commands,
    time: Res<Time>,
    game: Res<Game>,
    field: Option<Res<crate::Field>>,
    sheep: Query<(&Sheep, Entity)>,
    dogs: Query<(&Dog, Entity)>,
    mut tf: Query<&mut Transform>,
    mut rings: Query<(Entity, &mut BarkRing)>,
    beacons: Query<Entity, With<Beacon>>,
    chevrons: Query<Entity, With<Chevron>>,
    mut gates: Query<(Entity, &mut GatePivot)>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    kit: Res<Kit>,
) {
    let dt = time.delta_secs().min(0.05);
    let clock = time.elapsed_secs();
    for (s, e) in &sheep {
        let sp = s.vel.length();
        let walk = (sp / 2.0).min(1.0);
        let bob = (s.phase * 2.0).sin().abs() * 0.06 * walk;
        let hop = (s.hop * std::f32::consts::PI).sin() * 0.45;
        if let Ok(mut t) = tf.get_mut(e) {
            t.translation = Vec3::new(s.pos.x, bob + hop, s.pos.y);
            t.rotation = Quat::from_rotation_y(s.heading);
        }
        for (i, l) in s.legs.iter().enumerate() {
            if let Ok(mut t) = tf.get_mut(*l) {
                let ph = if i == 0 || i == 3 { 0.0 } else { std::f32::consts::PI };
                t.rotation = Quat::from_rotation_x((s.phase * 2.0 + ph).sin() * 0.55 * walk);
            }
        }
        if let Ok(mut t) = tf.get_mut(s.head) {
            // grazing: head down while calm and still
            let graze = if sp < 0.2 && s.alarm < 0.05 && s.wander == Vec2::ZERO { 1.0 } else { 0.0 };
            let y = 0.98 - 0.42 * graze;
            t.translation.y += (y - t.translation.y) * (1.0 - (-dt * 4.0).exp());
            t.translation.z = 0.72 + 0.08 * graze;
            let chew = graze * (clock * 3.0 + s.phase).sin() * 0.08;
            t.rotation = Quat::from_rotation_x(0.5 * graze + chew);
        }
    }
    if let Ok((d, e)) = dogs.single() {
        let sp = d.vel.length();
        let run = (sp / 5.0).min(1.0);
        let crouch = if d.sneak { 0.18 } else { 0.0 };
        if let Ok(mut t) = tf.get_mut(e) {
            let bound = (d.phase * 2.0).sin().abs() * 0.08 * run;
            t.translation = Vec3::new(d.pos.x, bound - crouch, d.pos.y);
            t.rotation = Quat::from_rotation_y(d.heading);
        }
        for (i, l) in d.legs.iter().enumerate() {
            if let Ok(mut t) = tf.get_mut(*l) {
                let ph = if i < 2 { 0.0 } else { 1.3 };
                t.rotation = Quat::from_rotation_x((d.phase * 2.0 + ph).sin() * 0.8 * run);
                t.translation.y = 0.21 + crouch * 0.5;
            }
        }
        if let Ok(mut t) = tf.get_mut(d.tail) {
            let wag = (clock * if sp < 0.3 { 7.0 } else { 3.0 }).sin() * 0.5;
            t.rotation = Quat::from_rotation_y(wag) * Quat::from_rotation_x(if d.sneak { 0.4 } else { -0.7 });
        }
        // the chevron at the dog's feet points to the gate when it is far away
        if let (Ok(ce), Some(field)) = (chevrons.single(), field.as_ref()) {
            let gm = gate_mid(field.obs.pen, field.obs.gate) + field.obs.gate.out() * 1.5;
            let to = gm - d.pos;
            let show = game.mode == Mode::Play && to.length() > 11.0;
            if let Ok(mut t) = tf.get_mut(ce) {
                if show {
                    let dir = to.normalize();
                    let p = d.pos + dir * 1.6;
                    // the cone's tip (+y) laid along dir, flattened
                    let yaw = dir.x.atan2(dir.y);
                    *t = Transform::from_xyz(p.x, 0.08, p.y)
                        .with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                        .with_scale(Vec3::new(0.8, 0.9, 0.08));
                } else {
                    t.translation.y = -5.0;
                }
            }
        }
    }
    for (e, mut r) in &mut rings {
        r.t += dt;
        let k = r.t / 0.6;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        if let Ok(mut t) = tf.get_mut(e) {
            t.scale = Vec3::new(1.0 + 9.0 * k, 1.0, 1.0 + 9.0 * k);
        }
        if let Some(mut m) = mats.get_mut(&r.mat) {
            m.base_color = Color::srgba(1.0, 0.9, 0.6, 0.55 * (1.0 - k) * (1.0 - k));
        }
    }
    let enough = game.penned >= game.need;
    for e in &beacons {
        if let Ok(mut t) = tf.get_mut(e) {
            t.translation.y = 3.4 + (clock * 2.0).sin() * 0.25;
            t.rotate_y(dt * 1.5);
        }
    }
    if let Some(mut m) = mats.get_mut(&kit.beacon_mat) {
        let (c, em) = if enough {
            (Color::srgb(0.45, 1.0, 0.45), LinearRgba::rgb(0.6, 2.2, 0.6))
        } else {
            (Color::srgb(1.0, 0.8, 0.25), LinearRgba::rgb(2.0, 1.4, 0.3))
        };
        if m.base_color != c {
            m.base_color = c;
            m.emissive = em;
        }
    }
    for (e, mut g) in &mut gates {
        let want = if game.gate_closed { g.closed } else { g.open };
        g.angle += (want - g.angle) * (1.0 - (-dt * 5.0).exp());
        if let Ok(mut t) = tf.get_mut(e) {
            t.rotation = Quat::from_rotation_y(g.angle);
        }
    }
}

/// The sun sinks and warms as the day runs out; the lantern comes on.
#[allow(clippy::type_complexity)]
pub fn sky(
    time: Res<Time>,
    mut smooth: Local<Option<f32>>,
    game: Res<Game>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), With<Sun>>,
    mut lantern: Query<&mut PointLight, With<Lantern>>,
    mut fog: Query<&mut DistanceFog>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
) {
    // the light eases between screens instead of jumping
    let target = game.day();
    let cur = smooth.get_or_insert(target);
    *cur += (target - *cur) * (1.0 - (-time.delta_secs() * 1.5).exp());
    let day = *cur;
    let e = day.powf(0.8);
    let elev = 0.12 + 0.75 * e;
    let az = -0.6 - 1.4 * (1.0 - day);
    let dir = Vec3::new(az.sin() * elev.cos(), elev.sin(), az.cos() * elev.cos());
    let warm = Vec3::new(1.0, 0.55, 0.3);
    let noon = Vec3::new(1.0, 0.97, 0.9);
    let c = warm.lerp(noon, day.sqrt());
    if let Ok((mut l, mut t)) = sun.single_mut() {
        l.illuminance = 1800.0 + 8500.0 * e;
        l.color = Color::srgb(c.x, c.y, c.z);
        *t = Transform::from_translation(dir * 30.0).looking_at(Vec3::ZERO, Vec3::Y);
    }
    let dusk = Vec3::new(0.93, 0.56, 0.42);
    let blue = Vec3::new(0.56, 0.74, 0.93);
    let s = dusk.lerp(blue, day.powf(0.6));
    clear.0 = Color::srgb(s.x, s.y, s.z);
    if let Ok(mut f) = fog.single_mut() {
        f.color = Color::srgb(s.x, s.y, s.z);
    }
    let amb = 140.0 + 260.0 * e;
    if (ambient.brightness - amb).abs() > 0.5 {
        ambient.brightness = amb;
        ambient.color = Color::srgb(0.8 + 0.2 * day, 0.8 + 0.15 * day, 0.85 + 0.15 * day);
    }
    let lamp = if game.mode == Mode::Title { 0.3 } else { 1.0 - day };
    for mut l in &mut lantern {
        l.intensity = 40_000.0 + 260_000.0 * lamp;
    }
    let _ = PEN_HALF;
}
