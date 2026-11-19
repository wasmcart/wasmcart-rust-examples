//! Lantern Links: nine holes of minigolf in a lantern-lit night garden.
//!
//! A 2D Bevy game on wasmcart: the course is one vertex-coloured mesh per
//! hole with the lanterns' light baked in, the ball, bumpers, turnstiles and
//! fireflies are entities, and the HUD is bevy_ui.
//!
//! Pad: stick or d-pad aims (hold B to aim finely), hold A to charge and
//! release to putt, B while charging cancels, Start pauses.

mod auto;
mod course;
mod draw;

use bevy::prelude::*;
use bevy::sprite_render::AlphaMode2d;
use bevy::text::FontSize;
use wasmcart_bevy::prelude::*;

use course::{Ball, Hole, TILE};
use draw::world;

const W: u32 = 960;
const H: u32 = 540;
const CART: CartConfig = CartConfig::new(W, H).webgpu().debug();
/// Seconds for the power meter to fill.
pub const CHARGE_SECS: f32 = 1.1;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(WasmcartPlugins)
        .insert_resource(ClearColor(Color::srgb(0.03, 0.06, 0.06)))
        .insert_resource(Rng(0x51A7_3C4D))
        .init_resource::<Ctl>()
        .init_resource::<Game>()
        .init_resource::<auto::Auto>()
        .init_resource::<Sounds>()
        .add_systems(Startup, (setup, setup_hud, load_sounds, start_title).chain())
        .add_systems(
            Update,
            (read_input, auto::drive, flow, aim_and_putt, roll, animate, fireflies, effects, hud, audio_out, publish).chain(),
        );
    app
}

wasmcart_bevy::cart!(CART, app, debug);

// ---------------------------------------------------------------- state

#[derive(Resource)]
pub struct Rng(pub u32);
impl Rng {
    pub fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
}

/// The merged controls of every pad (or the autopilot's).
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct Ctl {
    /// Stick, screen directions (+y down).
    pub stick: Vec2,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub a_just: bool,
    pub b: bool,
    pub b_just: bool,
    pub start_just: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Title,
    Intro,
    Aim,
    Roll,
    Sunk,
    Paused,
    Card,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Putt,
    Wall,
    Bumper,
    Bar,
    Cup,
    Splash,
    Sink,
    Ace,
    Round,
    Start,
    Pause,
}

#[derive(Resource)]
pub struct Course(pub Vec<Hole>);

#[derive(Resource)]
pub struct Game {
    pub mode: Mode,
    pub t: f32,
    /// The hole's clock (turnstiles turn by it).
    pub hole_t: f32,
    pub hole: usize,
    pub strokes: u32,
    pub cards: Vec<u32>,
    pub ball: Ball,
    pub last_rest: Vec2,
    pub aim: f32,
    pub power: f32,
    pub charging: bool,
    charge_dir: f32,
    pub picked_up: bool,
    pub splash_t: f32,
    pub sfx: Vec<(Sfx, f32)>,
    pub rumble: Option<(f32, f32, u32)>,
    pub bumped: Vec<(usize, f32)>,
    pub sunk_total: u32,
    pub shots: u32,
    pub auto: bool,
    pub ace_count: u32,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            mode: Mode::Title,
            t: 0.0,
            hole_t: 0.0,
            hole: 0,
            strokes: 0,
            cards: Vec::new(),
            ball: Ball { pos: Vec2::ZERO, vel: Vec2::ZERO },
            last_rest: Vec2::ZERO,
            aim: 0.0,
            power: 0.0,
            charging: false,
            charge_dir: 1.0,
            picked_up: false,
            splash_t: 0.0,
            sfx: Vec::new(),
            rumble: None,
            bumped: Vec::new(),
            sunk_total: 0,
            shots: 0,
            auto: false,
            ace_count: 0,
        }
    }
}

impl Game {
    fn set(&mut self, m: Mode) {
        self.mode = m;
        self.t = 0.0;
    }
    /// Strokes over (or under) par so far.
    pub fn to_par(&self, course: &[Hole]) -> i32 {
        self.cards.iter().enumerate().map(|(i, s)| *s as i32 - course[i].par as i32).sum()
    }
    pub fn max_strokes(&self, h: &Hole) -> u32 {
        h.par + 4
    }
}

#[derive(Component)]
pub struct HoleEnt;
#[derive(Component)]
pub struct BallTag;
#[derive(Component)]
pub struct BallShadow;
#[derive(Component)]
pub struct AimDot(pub usize);
#[derive(Component)]
pub struct Bumper {
    i: usize,
    pulse: f32,
}
#[derive(Component)]
pub struct Bar(pub Vec2);
#[derive(Component)]
pub struct Flag;
#[derive(Component)]
pub struct LanternGlow(f32);
#[derive(Component)]
pub struct Firefly {
    p: Vec2,
    v: Vec2,
    phase: f32,
}
#[derive(Component)]
pub struct Fx {
    t: f32,
    life: f32,
    vel: Vec2,
    grow: f32,
    alpha: f32,
}

#[derive(Resource)]
pub struct Kit {
    glow: Handle<Image>,
    circle: Handle<Mesh>,
    rect: Handle<Mesh>,
    flag: Handle<Mesh>,
    course_mat: Handle<ColorMaterial>,
    white: Handle<ColorMaterial>,
    cup: Handle<ColorMaterial>,
    cup_rim: Handle<ColorMaterial>,
    shadow: Handle<ColorMaterial>,
    bumper: Handle<ColorMaterial>,
    bumper_top: Handle<ColorMaterial>,
    wood: Handle<ColorMaterial>,
    wood_dark: Handle<ColorMaterial>,
    flag_mat: Handle<ColorMaterial>,
    lamp: Handle<ColorMaterial>,
    dot: Handle<ColorMaterial>,
}

// ---------------------------------------------------------------- setup

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<ColorMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut rng: ResMut<Rng>,
) {
    commands.spawn(Camera2d);
    let src = wasmcart_bevy::assets::load_bytes("holes.txt").map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let holes = course::parse(&src);
    commands.insert_resource(Course(holes));
    let cm = |c: Color| ColorMaterial { color: c, alpha_mode: AlphaMode2d::Blend, ..default() };
    let kit = Kit {
        glow: images.add(draw::glow_image()),
        circle: meshes.add(Circle::new(1.0)),
        rect: meshes.add(Rectangle::new(1.0, 1.0)),
        flag: meshes.add(Triangle2d::new(Vec2::new(0.0, 0.0), Vec2::new(0.0, -14.0), Vec2::new(20.0, -7.0))),
        course_mat: mats.add(cm(Color::WHITE)),
        white: mats.add(cm(Color::srgb(0.97, 0.97, 0.94))),
        cup: mats.add(cm(Color::srgb(0.02, 0.03, 0.02))),
        cup_rim: mats.add(cm(Color::srgb(0.55, 0.6, 0.5))),
        shadow: mats.add(cm(Color::srgba(0.0, 0.0, 0.0, 0.38))),
        bumper: mats.add(cm(Color::srgb(0.75, 0.3, 0.5))),
        bumper_top: mats.add(cm(Color::srgb(1.0, 0.6, 0.78))),
        wood: mats.add(cm(Color::srgb(0.62, 0.42, 0.24))),
        wood_dark: mats.add(cm(Color::srgb(0.36, 0.24, 0.14))),
        flag_mat: mats.add(cm(Color::srgb(1.0, 0.72, 0.3))),
        lamp: mats.add(cm(Color::srgb(1.0, 0.88, 0.6))),
        dot: mats.add(cm(Color::srgba(1.0, 0.95, 0.75, 0.9))),
    };
    // ball and its shadow
    commands.spawn((BallShadow, Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.shadow.clone()), Transform::from_xyz(0.0, 0.0, 5.0).with_scale(Vec3::new(7.5, 6.0, 1.0))));
    commands.spawn((BallTag, Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.white.clone()), Transform::from_xyz(0.0, 0.0, 6.0).with_scale(Vec3::splat(course::BALL_R))));
    for i in 0..22 {
        commands.spawn((AimDot(i), Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.dot.clone()), Transform::from_xyz(0.0, 0.0, 4.0).with_scale(Vec3::splat(2.6)), Visibility::Hidden));
    }
    for i in 0..28 {
        let p = Vec2::new(rng.range(0.0, 960.0), rng.range(0.0, 510.0));
        commands.spawn((
            Firefly { p, v: Vec2::ZERO, phase: i as f32 * 1.37 },
            Sprite { image: kit.glow.clone(), custom_size: Some(Vec2::splat(16.0)), color: Color::srgba(0.85, 1.0, 0.5, 0.0), ..default() },
            Transform::from_xyz(0.0, 0.0, 8.0),
        ));
    }
    commands.insert_resource(kit);
}

fn spawn_hole(commands: &mut Commands, kit: &Kit, meshes: &mut Assets<Mesh>, h: &Hole) {
    commands.spawn((HoleEnt, Mesh2d(meshes.add(draw::course_mesh(h))), MeshMaterial2d(kit.course_mat.clone()), Transform::from_xyz(0.0, 0.0, 0.0)));
    let at = |p: Vec2, z: f32| {
        let w = world(p);
        Transform::from_xyz(w.x, w.y, z)
    };
    // the cup, a little lamp beside it and a flag
    commands.spawn((HoleEnt, Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.cup_rim.clone()), at(h.cup, 1.0).with_scale(Vec3::splat(course::CUP_R + 2.0))));
    commands.spawn((HoleEnt, Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.cup.clone()), at(h.cup, 1.1).with_scale(Vec3::splat(course::CUP_R))));
    commands.spawn((HoleEnt, Mesh2d(kit.rect.clone()), MeshMaterial2d(kit.wood_dark.clone()),
        at(h.cup + Vec2::new(0.0, -22.0), 7.0).with_scale(Vec3::new(2.0, 44.0, 1.0))));
    commands.spawn((HoleEnt, Flag, Mesh2d(kit.flag.clone()), MeshMaterial2d(kit.flag_mat.clone()), at(h.cup + Vec2::new(1.0, -44.0), 7.1)));
    commands.spawn((HoleEnt, LanternGlow(0.3), Sprite { image: kit.glow.clone(), custom_size: Some(Vec2::splat(120.0)), color: Color::srgba(1.0, 0.75, 0.4, 0.22), ..default() },
        at(h.cup + Vec2::new(0.0, -40.0), 7.2)));
    for (i, l) in h.lanterns.iter().enumerate() {
        commands.spawn((HoleEnt, Mesh2d(kit.rect.clone()), MeshMaterial2d(kit.wood_dark.clone()), at(*l + Vec2::new(0.0, -14.0), 3.0).with_scale(Vec3::new(5.0, 18.0, 1.0))));
        commands.spawn((HoleEnt, Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.lamp.clone()), at(*l + Vec2::new(0.0, -26.0), 3.1).with_scale(Vec3::splat(6.0))));
        commands.spawn((HoleEnt, LanternGlow(i as f32 * 0.9), Sprite { image: kit.glow.clone(), custom_size: Some(Vec2::splat(200.0)), color: Color::srgba(1.0, 0.72, 0.38, 0.3), ..default() },
            at(*l + Vec2::new(0.0, -24.0), 9.0)));
    }
    for (i, b) in h.bumpers.iter().enumerate() {
        commands.spawn((HoleEnt, Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.shadow.clone()), at(*b + Vec2::new(4.0, 4.0), 1.5).with_scale(Vec3::splat(course::BUMPER_R))));
        commands
            .spawn((HoleEnt, Bumper { i, pulse: 0.0 }, at(*b, 2.0), Visibility::default()))
            .with_children(|p| {
                p.spawn((Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.bumper.clone()), Transform::from_scale(Vec3::splat(course::BUMPER_R))));
                p.spawn((Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.bumper_top.clone()), Transform::from_xyz(-2.0, 2.0, 0.1).with_scale(Vec3::splat(course::BUMPER_R * 0.55))));
                p.spawn((Sprite { image: kit.glow.clone(), custom_size: Some(Vec2::splat(64.0)), color: Color::srgba(1.0, 0.45, 0.7, 0.35), ..default() }, Transform::from_xyz(0.0, 0.0, -0.1)));
            });
    }
    for c in &h.bars {
        commands
            .spawn((HoleEnt, Bar(*c), at(*c, 3.5), Visibility::default()))
            .with_children(|p| {
                p.spawn((Mesh2d(kit.rect.clone()), MeshMaterial2d(kit.shadow.clone()), Transform::from_xyz(4.0, -5.0, -0.1).with_scale(Vec3::new(course::BAR_HALF * 2.0, course::BAR_W * 2.0, 1.0))));
                p.spawn((Mesh2d(kit.rect.clone()), MeshMaterial2d(kit.wood.clone()), Transform::from_scale(Vec3::new(course::BAR_HALF * 2.0, course::BAR_W * 2.0, 1.0))));
            });
        commands.spawn((HoleEnt, Mesh2d(kit.circle.clone()), MeshMaterial2d(kit.wood_dark.clone()), at(*c, 3.7).with_scale(Vec3::splat(9.0))));
    }
}

fn start_title(mut commands: Commands, mut game: ResMut<Game>, kit: Res<Kit>, course: Res<Course>, mut meshes: ResMut<Assets<Mesh>>) {
    game.hole = 0;
    let h = &course.0[0];
    spawn_hole(&mut commands, &kit, &mut meshes, h);
    game.ball = Ball { pos: h.tee, vel: Vec2::ZERO };
    game.aim = (h.cup - h.tee).to_angle();
    game.set(Mode::Title);
}

fn load_hole(commands: &mut Commands, kit: &Kit, meshes: &mut Assets<Mesh>, course: &Course, game: &mut Game, old: &Query<Entity, With<HoleEnt>>) {
    for e in old {
        commands.entity(e).despawn();
    }
    let h = &course.0[game.hole];
    spawn_hole(commands, kit, meshes, h);
    game.ball = Ball { pos: h.tee, vel: Vec2::ZERO };
    game.last_rest = h.tee;
    game.strokes = 0;
    game.hole_t = 0.0;
    game.picked_up = false;
    game.charging = false;
    game.power = 0.0;
    // aim at the cup if it is in sight, else along the open fairway
    let to_cup = h.cup - h.tee;
    game.aim = to_cup.to_angle();
    if !clear_line(h, h.tee, h.cup) {
        let mut best = (f32::MAX, 0.0);
        for k in 0..36 {
            let a = k as f32 * std::f32::consts::TAU / 36.0;
            let p = h.tee + Vec2::new(a.cos(), a.sin()) * 90.0;
            if clear_line(h, h.tee, p) {
                let d = h.dist_at(p);
                if d < best.0 {
                    best = (d, a);
                }
            }
        }
        game.aim = best.1;
    }
}

fn clear_line(h: &Hole, a: Vec2, b: Vec2) -> bool {
    let n = (a.distance(b) / 6.0) as usize + 1;
    (0..=n).all(|i| !h.tile_at(a.lerp(b, i as f32 / n as f32)).solid())
}

// ---------------------------------------------------------------- input

fn read_input(pads: Query<&Gamepad>, mut ctl: ResMut<Ctl>) {
    let mut c = Ctl::default();
    for p in &pads {
        let s = p.left_stick();
        if s.length() > c.stick.length() {
            c.stick = Vec2::new(s.x, -s.y);
        }
        c.left |= p.pressed(GamepadButton::DPadLeft) || p.pressed(GamepadButton::DPadDown);
        c.right |= p.pressed(GamepadButton::DPadRight) || p.pressed(GamepadButton::DPadUp);
        c.a |= p.pressed(GamepadButton::South);
        c.a_just |= p.just_pressed(GamepadButton::South);
        c.b |= p.pressed(GamepadButton::East);
        c.b_just |= p.just_pressed(GamepadButton::East);
        c.start_just |= p.just_pressed(GamepadButton::Start);
    }
    *ctl = c;
}

// ---------------------------------------------------------------- flow

#[allow(clippy::too_many_arguments)]
fn flow(
    mut commands: Commands,
    time: Res<Time>,
    ctl: Res<Ctl>,
    mut game: ResMut<Game>,
    kit: Res<Kit>,
    course: Res<Course>,
    mut meshes: ResMut<Assets<Mesh>>,
    old: Query<Entity, With<HoleEnt>>,
    dbg: Res<CartDebug>,
) {
    game.auto = dbg.get("autopilot").unwrap_or(0.0) > 0.5;
    let dt = time.delta_secs().min(0.05);
    game.t += dt;
    if game.mode != Mode::Paused {
        game.hole_t += dt;
    }
    let n = course.0.len();
    match game.mode {
        Mode::Title => {
            if (ctl.a_just || ctl.start_just) && game.t > 0.4 {
                game.cards.clear();
                game.sunk_total = 0;
                game.shots = 0;
                game.ace_count = 0;
                // a test host may start at a later hole (debug field start_hole)
                game.hole = (dbg.get("start_hole").unwrap_or(0.0).max(0.0) as usize).min(n - 1);
                for k in 0..game.hole {
                    game.cards.push(course.0[k].par);
                }
                load_hole(&mut commands, &kit, &mut meshes, &course, &mut game, &old);
                game.set(Mode::Intro);
                game.sfx.push((Sfx::Start, 0.8));
            }
        }
        Mode::Intro => {
            if game.t > 2.6 || (ctl.a_just && game.t > 0.4) {
                game.set(Mode::Aim);
            }
        }
        Mode::Aim | Mode::Roll => {
            if ctl.start_just {
                game.charging = false;
                game.mode = Mode::Paused;
                game.t = 0.0;
                game.sfx.push((Sfx::Pause, 0.7));
            }
        }
        Mode::Paused => {
            if ctl.start_just {
                let resume = if game.ball.vel.length() > 0.0 { Mode::Roll } else { Mode::Aim };
                game.mode = resume;
                game.sfx.push((Sfx::Pause, 0.5));
            }
        }
        Mode::Sunk => {
            if (ctl.a_just && game.t > 1.0) || game.t > 5.0 {
                if game.hole + 1 >= n {
                    game.set(Mode::Card);
                    game.sfx.push((Sfx::Round, 0.9));
                } else {
                    game.hole += 1;
                    load_hole(&mut commands, &kit, &mut meshes, &course, &mut game, &old);
                    game.set(Mode::Intro);
                }
            }
        }
        Mode::Card => {
            if ctl.a_just && game.t > 2.0 {
                game.hole = 0;
                load_hole(&mut commands, &kit, &mut meshes, &course, &mut game, &old);
                game.set(Mode::Title);
            }
        }
    }
}

fn wrap(a: f32) -> f32 {
    let mut a = a;
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

fn aim_and_putt(time: Res<Time>, ctl: Res<Ctl>, mut game: ResMut<Game>) {
    if game.mode != Mode::Aim {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    // aiming: the stick points the putt; the d-pad turns it; B makes it fine
    let fine = ctl.b && !game.charging;
    if ctl.stick.length() > 0.5 {
        let target = ctl.stick.to_angle();
        if ctl.stick.length() > 0.9 {
            game.aim = target;
        } else {
            let d = wrap(target - game.aim);
            game.aim += d * (dt * 14.0).min(1.0);
        }
    }
    let rate = if fine { 0.35 } else { 1.7 };
    if ctl.left {
        game.aim -= rate * dt;
    }
    if ctl.right {
        game.aim += rate * dt;
    }
    game.aim = wrap(game.aim);
    // power: hold A, release to putt; B cancels
    if ctl.a_just && game.t > 0.15 {
        game.charging = true;
        game.power = 0.0;
        game.charge_dir = 1.0;
    }
    if game.charging {
        if ctl.b_just {
            game.charging = false;
            game.power = 0.0;
            return;
        }
        if ctl.a {
            game.power += game.charge_dir * dt / CHARGE_SECS;
            if game.power >= 1.0 {
                game.power = 1.0;
                game.charge_dir = -1.0;
            } else if game.power <= 0.0 {
                game.power = 0.0;
                game.charge_dir = 1.0;
            }
        } else {
            game.charging = false;
            if game.power > 0.02 {
                let dir = Vec2::new(game.aim.cos(), game.aim.sin());
                game.ball.vel = dir * course::shot_speed(game.power);
                game.strokes += 1;
                game.shots += 1;
                let p = game.power;
                game.sfx.push((Sfx::Putt, 0.4 + 0.6 * p));
                game.rumble = Some((0.05 + 0.2 * p, 0.1 + 0.3 * p, 60));
                game.set(Mode::Roll);
            }
        }
    }
}

fn roll(time: Res<Time>, mut game: ResMut<Game>, course: Res<Course>, mut commands: Commands, kit: Res<Kit>) {
    if game.mode != Mode::Roll {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let h = &course.0[game.hole];
    let mut ball = game.ball;
    let t = game.hole_t;
    let hits = course::step(h, &mut ball, t, dt);
    game.ball = ball;
    if hits.wall > 60.0 {
        game.sfx.push((Sfx::Wall, (hits.wall / 900.0).clamp(0.15, 0.9)));
    }
    if hits.bar > 60.0 {
        game.sfx.push((Sfx::Bar, (hits.bar / 800.0).clamp(0.3, 0.9)));
        game.rumble = Some((0.25, 0.2, 80));
    }
    if let Some(i) = hits.bumper {
        game.sfx.push((Sfx::Bumper, 0.8));
        game.bumped.push((i, 1.0));
        game.rumble = Some((0.3, 0.3, 90));
    }
    if hits.lip {
        game.sfx.push((Sfx::Cup, 0.35));
    }
    if hits.sunk {
        let ace = game.strokes == 1;
        let s = game.strokes;
        game.cards.push(s);
        game.sunk_total += 1;
        game.sfx.push((Sfx::Sink, 0.9));
        if ace {
            game.ace_count += 1;
            game.sfx.push((Sfx::Ace, 0.9));
        }
        game.rumble = Some((0.3, 0.5, 220));
        sparkle(&mut commands, &kit, h.cup, if ace { 24 } else { 12 });
        game.set(Mode::Sunk);
        return;
    }
    if hits.water {
        game.sfx.push((Sfx::Splash, 0.9));
        game.rumble = Some((0.5, 0.35, 250));
        splash(&mut commands, &kit, game.ball.pos);
        game.strokes += 1;
        game.splash_t = 2.5;
        game.ball = Ball { pos: game.last_rest, vel: Vec2::ZERO };
        end_shot(&mut game, h);
        return;
    }
    if course::at_rest(h, &game.ball) {
        game.ball.vel = Vec2::ZERO;
        game.last_rest = game.ball.pos;
        end_shot(&mut game, h);
    }
}

fn end_shot(game: &mut Game, h: &Hole) {
    if game.strokes >= game.max_strokes(h) {
        game.picked_up = true;
        let s = game.max_strokes(h);
        game.cards.push(s);
        game.set(Mode::Sunk);
    } else {
        game.set(Mode::Aim);
    }
}

fn sparkle(commands: &mut Commands, kit: &Kit, at: Vec2, n: usize) {
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        let w = world(at);
        commands.spawn((
            HoleEnt,
            Fx { t: 0.0, life: 1.4, vel: Vec2::new(a.cos(), a.sin()) * 60.0 + Vec2::new(0.0, 30.0), grow: 0.0, alpha: 0.8 },
            Sprite { image: kit.glow.clone(), custom_size: Some(Vec2::splat(14.0)), color: Color::srgba(1.0, 0.9, 0.5, 0.0), ..default() },
            Transform::from_xyz(w.x, w.y, 9.5),
        ));
    }
}

fn splash(commands: &mut Commands, kit: &Kit, at: Vec2) {
    let w = world(at);
    commands.spawn((
        HoleEnt,
        Fx { t: 0.0, life: 0.9, vel: Vec2::ZERO, grow: 70.0, alpha: 0.6 },
        Sprite { image: kit.glow.clone(), custom_size: Some(Vec2::splat(10.0)), color: Color::srgba(0.6, 0.85, 1.0, 0.0), ..default() },
        Transform::from_xyz(w.x, w.y, 9.5),
    ));
}

// ---------------------------------------------------------------- animation

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn animate(
    time: Res<Time>,
    mut game: ResMut<Game>,
    course: Res<Course>,
    mut ball: Query<&mut Transform, (With<BallTag>, Without<BallShadow>)>,
    mut shadow: Query<&mut Transform, (With<BallShadow>, Without<BallTag>)>,
    mut dots: Query<(&AimDot, &mut Transform, &mut Visibility), (Without<BallTag>, Without<BallShadow>)>,
    mut bumpers: Query<(&mut Bumper, &mut Transform), (Without<AimDot>, Without<BallTag>, Without<BallShadow>, Without<Bar>)>,
    mut bars: Query<(&Bar, &mut Transform), (Without<AimDot>, Without<BallTag>, Without<BallShadow>, Without<Bumper>)>,
    mut glows: Query<(&LanternGlow, &mut Sprite)>,
) {
    let dt = time.delta_secs().min(0.05);
    let h = &course.0[game.hole];
    // the ball drops into the cup after it is sunk
    let sink = if game.mode == Mode::Sunk && !game.picked_up { (1.0 - game.t * 3.0).max(0.0) } else { 1.0 };
    let w = world(game.ball.pos);
    for mut t in &mut ball {
        t.translation = Vec3::new(w.x, w.y, 6.0);
        t.scale = Vec3::splat(course::BALL_R * (0.4 + 0.6 * sink));
    }
    for mut t in &mut shadow {
        t.translation = Vec3::new(w.x + 2.5, w.y - 3.0, 5.0);
        t.scale = Vec3::new(7.5, 6.0, 1.0) * sink;
    }
    // aim dots: along the putt, bouncing off the first wall
    let show = game.mode == Mode::Aim && game.t > 0.1;
    let len = if game.charging { 60.0 + 360.0 * game.power } else { 150.0 };
    let path = aim_path(h, game.ball.pos, game.aim, len);
    for (d, mut t, mut v) in &mut dots {
        let want = if show && d.0 < path.len() { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
        if let Some(p) = path.get(d.0) {
            let w = world(*p);
            t.translation = Vec3::new(w.x, w.y, 4.0);
            let k = 1.0 - d.0 as f32 / path.len().max(1) as f32;
            t.scale = Vec3::splat(1.6 + 1.6 * k);
        }
    }
    let bumped = std::mem::take(&mut game.bumped);
    for (mut b, mut t) in &mut bumpers {
        if bumped.iter().any(|x| x.0 == b.i) {
            b.pulse = 1.0;
        }
        b.pulse = (b.pulse - dt * 3.0).max(0.0);
        t.scale = Vec3::splat(1.0 + 0.25 * b.pulse * b.pulse);
    }
    for (b, mut t) in &mut bars {
        let w = world(b.0);
        t.translation = Vec3::new(w.x, w.y, 3.5);
        t.rotation = Quat::from_rotation_z(-course::bar_angle(game.hole_t));
    }
    // lanterns breathe very slowly (well under 1 Hz), never flicker
    let clock = time.elapsed_secs();
    for (g, mut s) in &mut glows {
        let a = 0.27 + 0.04 * (clock * 0.9 + g.0).sin();
        s.color.set_alpha(a);
    }
}

/// Points every 18 px along the aim, reflecting off the first wall.
fn aim_path(h: &Hole, from: Vec2, a: f32, len: f32) -> Vec<Vec2> {
    let mut out = Vec::new();
    let mut p = from;
    let mut d = Vec2::new(a.cos(), a.sin());
    let mut bounced = false;
    let mut walked = 0.0;
    let mut next = 18.0;
    while walked < len && out.len() < 22 {
        let q = p + d * 3.0;
        let hit_x = h.tile_at(Vec2::new(q.x + d.x.signum() * course::BALL_R, p.y)).solid();
        let hit_y = h.tile_at(Vec2::new(p.x, q.y + d.y.signum() * course::BALL_R)).solid();
        if hit_x || hit_y {
            if bounced {
                break;
            }
            bounced = true;
            if hit_x {
                d.x = -d.x;
            }
            if hit_y {
                d.y = -d.y;
            }
            continue;
        }
        p = q;
        walked += 3.0;
        if walked >= next {
            out.push(p);
            next += 18.0;
        }
    }
    out
}

fn fireflies(time: Res<Time>, mut q: Query<(&mut Firefly, &mut Transform, &mut Sprite)>, mut rng: ResMut<Rng>) {
    let dt = time.delta_secs().min(0.05);
    for (mut f, mut t, mut s) in &mut q {
        f.phase += dt;
        let wander = Vec2::new((f.phase * 0.7).sin() + rng.range(-0.3, 0.3), (f.phase * 0.53 + 1.0).cos() + rng.range(-0.3, 0.3));
        f.v = (f.v + wander * 30.0 * dt) * (1.0 - dt * 0.5);
        f.v = f.v.clamp_length_max(26.0);
        let v = f.v;
        f.p += v * dt;
        if f.p.x < -20.0 { f.p.x = 980.0; }
        if f.p.x > 980.0 { f.p.x = -20.0; }
        if f.p.y < -10.0 { f.p.y = 520.0; }
        if f.p.y > 520.0 { f.p.y = -10.0; }
        let w = world(f.p);
        t.translation = Vec3::new(w.x, w.y, 8.0);
        // a slow glow in and out over several seconds
        let a = 0.5 + 0.5 * (f.phase * 0.8).sin();
        s.color.set_alpha(0.15 + 0.5 * a * a);
    }
}

fn effects(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Fx, &mut Transform, &mut Sprite)>) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut f, mut t, mut s) in &mut q {
        f.t += dt;
        let k = f.t / f.life;
        if k >= 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        t.translation += (f.vel * dt).extend(0.0);
        if f.grow > 0.0 {
            let size = 10.0 + f.grow * k;
            s.custom_size = Some(Vec2::splat(size));
        }
        // swell in, fade out
        let a = if k < 0.2 { k / 0.2 } else { 1.0 - (k - 0.2) / 0.8 };
        s.color.set_alpha(f.alpha * a);
    }
}

// ---------------------------------------------------------------- HUD

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Hud {
    Top,
    HoleName,
    Strokes,
    Total,
    PowerPanel,
    PowerFill,
    HintPanel,
    Hint,
    Overlay,
    OvTitle,
    OvSub,
    OvBody,
    OvPrompt,
}

const INK: Color = Color::srgb(0.98, 0.95, 0.86);
const PANEL: Color = Color::srgba(0.02, 0.05, 0.05, 0.7);

fn setup_hud(mut commands: Commands) {
    let text = |s: &str, size: f32, color: Color| {
        (Text::new(s), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color))
    };
    commands
        .spawn((
            Hud::Top,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                height: px(30),
                padding: UiRect::axes(px(14), px(4)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.03, 0.03, 0.9)),
        ))
        .with_children(|p| {
            p.spawn((Hud::HoleName, text("", 17.0, Color::srgb(1.0, 0.85, 0.55))));
            p.spawn((Hud::Strokes, text("", 17.0, INK)));
            p.spawn((Hud::Total, text("", 17.0, INK)));
        });
    commands
        .spawn((
            Hud::PowerPanel,
            Node {
                position_type: PositionType::Absolute,
                left: px(16),
                bottom: px(14),
                padding: UiRect::axes(px(12), px(8)),
                column_gap: px(10),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|p| {
            p.spawn(text("POWER", 14.0, Color::srgb(1.0, 0.85, 0.55)));
            p.spawn((
                Node { width: px(170), height: px(12), border_radius: BorderRadius::all(px(6)), ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ))
            .with_children(|p| {
                p.spawn((
                    Hud::PowerFill,
                    Node { width: percent(0), height: percent(100), border_radius: BorderRadius::all(px(6)), ..default() },
                    BackgroundColor(Color::srgb(1.0, 0.75, 0.3)),
                ));
            });
        });
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(240),
            right: px(16),
            bottom: px(12),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Hud::HintPanel,
                Node { padding: UiRect::axes(px(14), px(7)), border_radius: BorderRadius::all(px(10)), ..default() },
                BackgroundColor(PANEL),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((Hud::Hint, text("", 17.0, INK), TextLayout::justify(Justify::Center)));
            });
        });
    commands
        .spawn((
            Hud::Overlay,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(30),
                bottom: px(0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(10),
                    padding: UiRect::axes(px(36), px(22)),
                    border_radius: BorderRadius::all(px(18)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.02, 0.05, 0.05, 0.72)),
            ))
            .with_children(|p| {
                p.spawn((Hud::OvTitle, text("", 64.0, Color::srgb(1.0, 0.86, 0.55)), TextLayout::justify(Justify::Center), TextShadow::default()));
                p.spawn((Hud::OvSub, text("", 24.0, INK), TextLayout::justify(Justify::Center)));
                p.spawn((Hud::OvBody, text("", 19.0, INK), TextLayout::justify(Justify::Center)));
                p.spawn((Hud::OvPrompt, text("", 22.0, INK), TextLayout::justify(Justify::Center)));
            });
        });
}

fn score_name(strokes: u32, par: u32) -> &'static str {
    if strokes == 1 {
        return "Hole in one!";
    }
    match strokes as i32 - par as i32 {
        i32::MIN..=-3 => "Albatross!",
        -2 => "Eagle!",
        -1 => "Birdie!",
        0 => "Par",
        1 => "Bogey",
        2 => "Double bogey",
        _ => "Holed out",
    }
}

fn fmt_par(v: i32) -> String {
    match v {
        0 => "even".into(),
        v if v > 0 => format!("+{v}"),
        v => format!("{v}"),
    }
}

#[allow(clippy::type_complexity)]
fn hud(
    game: Res<Game>,
    course: Res<Course>,
    mut texts: Query<(&Hud, &mut Text, Option<&mut TextColor>)>,
    mut nodes: Query<(&Hud, &mut Node), Without<Text>>,
    mut vis: Query<(&Hud, &mut Visibility)>,
) {
    let holes = &course.0;
    let h = &holes[game.hole];
    let m = game.mode;
    let mut want: Vec<(Hud, String)> = Vec::new();
    want.push((Hud::HoleName, format!("HOLE {}  {}   PAR {}", game.hole + 1, h.name.to_uppercase(), h.par)));
    want.push((Hud::Strokes, format!("STROKES {}", game.strokes)));
    want.push((Hud::Total, format!("ROUND {}", fmt_par(game.to_par(holes)))));
    let (title, sub, body, prompt) = match m {
        Mode::Title => (
            "LANTERN LINKS".to_string(),
            "Nine holes of night-garden minigolf".to_string(),
            "Stick or d-pad: aim (hold B to aim finely)\nhold A: power, release to putt     Start: pause".to_string(),
            "Press A to start".to_string(),
        ),
        Mode::Intro => (format!("Hole {}", game.hole + 1), h.name.clone(), format!("Par {}", h.par), String::new()),
        Mode::Paused => ("PAUSED".into(), String::new(), String::new(), "Start: resume".into()),
        Mode::Sunk => {
            let s = *game.cards.last().unwrap_or(&0);
            (
                if game.picked_up { "Picked up".into() } else { score_name(s, h.par).into() },
                format!("{} stroke{} on a par {}", s, if s == 1 { "" } else { "s" }, h.par),
                format!("round {}", fmt_par(game.to_par(holes))),
                if game.t > 1.0 { "A: next hole".into() } else { String::new() },
            )
        }
        Mode::Card => {
            let mut card = String::from("hole  ");
            for i in 0..holes.len() {
                card += &format!("{:>3}", i + 1);
            }
            card += "\npar   ";
            for hh in holes.iter() {
                card += &format!("{:>3}", hh.par);
            }
            card += "\nyou   ";
            for s in &game.cards {
                card += &format!("{:>3}", s);
            }
            let total: u32 = game.cards.iter().sum();
            let par: u32 = holes.iter().map(|h| h.par).sum();
            (
                "ROUND COMPLETE".into(),
                format!("{} strokes, par {}  ({})", total, par, fmt_par(total as i32 - par as i32)),
                card,
                if game.t > 2.0 { "A: play again".into() } else { String::new() },
            )
        }
        _ => (String::new(), String::new(), String::new(), String::new()),
    };
    want.push((Hud::OvTitle, title));
    want.push((Hud::OvSub, sub));
    want.push((Hud::OvBody, body));
    want.push((Hud::OvPrompt, prompt));
    let hint = match m {
        Mode::Aim if game.splash_t > 0.0 => Some("Splash! One stroke added; the ball is back where it was."),
        Mode::Aim if game.charging => Some("Release A to putt.   B cancels."),
        Mode::Aim if game.hole == 0 && game.strokes == 0 => Some("Aim with the stick or d-pad, then hold A and release to putt."),
        Mode::Aim if game.hole < 2 && game.strokes > 0 && game.strokes < 3 => Some("Hold B while aiming to aim finely."),
        _ => None,
    };
    want.push((Hud::Hint, hint.unwrap_or("").into()));
    for (tag, mut t, c) in &mut texts {
        if let Some((_, s)) = want.iter().find(|w| w.0 == *tag) {
            if t.0 != *s {
                t.0 = s.clone();
            }
        }
        if *tag == Hud::OvPrompt {
            if let Some(mut c) = c {
                c.0 = Color::srgba(1.0, 0.95, 0.75, 0.7 + 0.3 * (game.t * 2.4).sin());
            }
        }
    }
    for (tag, mut n) in &mut nodes {
        if *tag == Hud::PowerFill {
            let w = Val::Percent(game.power * 100.0);
            if n.width != w {
                n.width = w;
            }
        }
    }
    for (tag, mut v) in &mut vis {
        let show = match tag {
            Hud::Top => m != Mode::Title,
            Hud::PowerPanel => m == Mode::Aim,
            Hud::HintPanel => hint.is_some(),
            Hud::Overlay => !matches!(m, Mode::Aim | Mode::Roll),
            _ => continue,
        };
        let w = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != w {
            *v = w;
        }
    }
}

// ---------------------------------------------------------------- audio

#[derive(Resource, Default)]
pub struct Sounds {
    clips: Vec<(Sfx, AudioClip)>,
    title: Option<AudioClip>,
    music: Option<AudioClip>,
    night: Option<AudioClip>,
    voices: [Option<VoiceId>; 3],
    vols: [f32; 3],
}

fn load_sounds(mut s: ResMut<Sounds>) {
    let load = |name: &str| wasmcart_bevy::assets::load_bytes(name).and_then(|b| AudioClip::from_wav(&b).ok());
    for (k, n) in [
        (Sfx::Putt, "putt.wav"),
        (Sfx::Wall, "wall.wav"),
        (Sfx::Bumper, "bumper.wav"),
        (Sfx::Bar, "bar.wav"),
        (Sfx::Cup, "lip.wav"),
        (Sfx::Splash, "splash.wav"),
        (Sfx::Sink, "sink.wav"),
        (Sfx::Ace, "ace.wav"),
        (Sfx::Round, "round.wav"),
        (Sfx::Start, "start.wav"),
        (Sfx::Pause, "pause.wav"),
    ] {
        if let Some(c) = load(n) {
            s.clips.push((k, c));
        }
    }
    s.title = load("title.wav");
    s.music = load("music.wav");
    s.night = load("night.wav");
}

fn audio_out(time: Res<Time>, audio: Res<Audio>, mut s: ResMut<Sounds>, mut game: ResMut<Game>) {
    let dt = time.delta_secs().min(0.05);
    game.splash_t = (game.splash_t - dt).max(0.0);
    let fx = std::mem::take(&mut game.sfx);
    for (k, vol) in fx {
        if let Some((_, c)) = s.clips.iter().find(|c| c.0 == k) {
            audio.play_with(c, PlaySettings { volume: vol, ..default() });
        }
    }
    let clips = [s.title.clone(), s.music.clone(), s.night.clone()];
    for (i, c) in clips.iter().enumerate() {
        if s.voices[i].is_none() {
            if let Some(c) = c {
                s.voices[i] = Some(audio.play_with(c, PlaySettings { volume: 0.0, looped: true, ..default() }));
            }
        }
    }
    let target = match game.mode {
        Mode::Title | Mode::Card => [0.5, 0.0, 0.25],
        Mode::Paused => [0.0, 0.1, 0.1],
        _ => [0.0, 0.38, 0.22],
    };
    let k = 1.0 - (-dt * 1.5).exp();
    for i in 0..3 {
        s.vols[i] += (target[i] - s.vols[i]) * k;
        if let Some(v) = s.voices[i] {
            audio.set_volume(v, s.vols[i]);
        }
    }
    if let Some((lo, hi, ms)) = game.rumble.take() {
        for (i, p) in wasmcart_bevy::abi::pads().iter().enumerate() {
            if p.connected != 0 {
                wasmcart::wc_pad_rumble(i as u32, lo, hi, ms);
            }
        }
    }
}

// ---------------------------------------------------------------- debug

fn publish(mut dbg: ResMut<CartDebug>, game: Res<Game>, course: Res<Course>, au: Res<auto::Auto>, mut once: Local<bool>) {
    if !*once {
        *once = true;
        dbg.set("autopilot", 0.0);
        dbg.set("start_hole", 0.0);
        dbg.set("auto_noise", 0.0);
    }
    dbg.set("mode", game.mode as u32 as f32);
    dbg.set("hole", game.hole as f32);
    dbg.set("strokes", game.strokes as f32);
    dbg.set("holes_done", game.cards.len() as f32);
    dbg.set("sunk", game.sunk_total as f32);
    dbg.set("shots", game.shots as f32);
    dbg.set("to_par", game.to_par(&course.0) as f32);
    dbg.set("total", game.cards.iter().sum::<u32>() as f32);
    dbg.set("ball_x", game.ball.pos.x);
    dbg.set("ball_y", game.ball.pos.y);
    dbg.set("ball_speed", game.ball.vel.length());
    dbg.set("aim", game.aim);
    dbg.set("power", game.power);
    dbg.set("charging", game.charging as u32 as f32);
    dbg.set("aces", game.ace_count as f32);
    dbg.set("auto_state", au.state as f32);
    let _ = TILE;
}
