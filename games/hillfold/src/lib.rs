//! Hillfold: herd a flock of sheep into the fold before sundown.
//!
//! A 3D Bevy game on wasmcart: Bevy's PBR renderer with a shadow-casting sun
//! that sinks as the day runs out, a flock of up to 36 sheep that each steer
//! by themselves (flocking, fleeing the dog, avoiding walls), hundreds of
//! grass tufts and flowers, and a UI drawn with bevy_ui.
//!
//! Pad: left stick or d-pad runs, A barks, hold B to sneak, Start pauses.

mod auto;
mod fields;
mod scene;
mod world;

use bevy::prelude::*;
use bevy::text::FontSize;
use wasmcart_bevy::prelude::*;

use fields::{FIELDS, FieldDef};
use world::{Grid, Obstacles};

const W: u32 = 960;
const H: u32 = 540;
const CART: CartConfig = CartConfig::new(W, H).webgpu().debug();

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(WasmcartPlugins)
        .insert_resource(ClearColor(Color::srgb(0.55, 0.72, 0.9)))
        .insert_resource(GlobalAmbientLight { brightness: 300.0, ..default() })
        .insert_resource(bevy::light::DirectionalLightShadowMap { size: 2048 })
        .insert_resource(Rng(0x2545_F491))
        .init_resource::<Ctl>()
        .init_resource::<Game>()
        .init_resource::<auto::Auto>()
        .init_resource::<Sounds>()
        .add_systems(Startup, (scene::setup, setup_hud, load_sounds, start_title).chain())
        .add_systems(
            Update,
            (
                read_input,
                auto::drive,
                flow,
                dog_move,
                flock,
                fold_check,
                scene::animate,
                scene::sky,
                camera,
                hud,
                audio_out,
                publish,
            )
                .chain(),
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
    pub mv: Vec2,
    pub a: bool,
    pub a_just: bool,
    pub b: bool,
    pub start_just: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Title,
    Intro,
    Play,
    Paused,
    Cleared,
    Over,
    Victory,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    Bark,
    Baa,
    Fold,
    Gate,
    Clear,
    Fail,
    Start,
    Pause,
    Dusk,
    Win,
}

#[derive(Resource)]
pub struct Game {
    pub mode: Mode,
    /// Seconds in the current mode.
    pub t: f32,
    pub field: usize,
    pub score: u32,
    pub field_start_score: u32,
    pub time_left: f32,
    pub penned: usize,
    pub sheep: usize,
    pub need: usize,
    pub gate_closed: bool,
    pub barks: u32,
    pub fields_cleared: u32,
    /// Clear results: (sheep, sunlight bonus, extra bonus).
    pub result: (usize, u32, u32),
    pub dusk_warned: bool,
    pub sfx: Vec<(Sfx, f32)>,
    pub rumble: Option<(f32, f32, u32)>,
    /// Seconds the dog has stood between the flock and the fold.
    pub wrong_side: f32,
    pub flock_c: Vec2,
    pub flock_spread: f32,
    pub auto: bool,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            mode: Mode::Title,
            t: 0.0,
            field: 0,
            score: 0,
            field_start_score: 0,
            time_left: 0.0,
            penned: 0,
            sheep: 0,
            need: 0,
            gate_closed: false,
            barks: 0,
            fields_cleared: 0,
            result: (0, 0, 0),
            dusk_warned: false,
            sfx: Vec::new(),
            rumble: None,
            wrong_side: 0.0,
            flock_c: Vec2::ZERO,
            flock_spread: 0.0,
            auto: false,
        }
    }
}

impl Game {
    pub fn def(&self) -> &'static FieldDef {
        &FIELDS[self.field]
    }
    fn set(&mut self, m: Mode) {
        self.mode = m;
        self.t = 0.0;
    }
    pub fn day(&self) -> f32 {
        match self.mode {
            Mode::Title => 0.8,
            Mode::Intro => 1.0,
            _ => (self.time_left / self.def().secs).clamp(0.0, 1.0),
        }
    }
    /// The dog and sheep move (everything but the pause).
    pub fn running(&self) -> bool {
        self.mode != Mode::Paused
    }
    pub fn dog_free(&self) -> bool {
        matches!(self.mode, Mode::Intro | Mode::Play)
    }
}

/// The current field's geometry.
#[derive(Resource)]
pub struct Field {
    pub obs: Obstacles,
    pub grid: Grid,
    /// Path distance from each cell to the fold, for sheep-sized bodies.
    pub home: Vec<f32>,
}

#[derive(Component)]
pub struct FieldEnt;

#[derive(Component)]
pub struct Sheep {
    pub pos: Vec2,
    pub vel: Vec2,
    pub alarm: f32,
    pub panic: f32,
    pub penned: bool,
    pub phase: f32,
    pub hop: f32,
    pub graze: f32,
    pub wander: Vec2,
    pub heading: f32,
    pub legs: [Entity; 4],
    pub head: Entity,
    pub baa_cd: f32,
}

#[derive(Component)]
pub struct Dog {
    pub pos: Vec2,
    pub vel: Vec2,
    pub heading: f32,
    pub phase: f32,
    pub legs: [Entity; 4],
    pub tail: Entity,
    pub bark_cd: f32,
    pub sneak: bool,
}

#[derive(Resource, Default)]
pub struct Sounds {
    clips: Vec<(Sfx, Vec<AudioClip>)>,
    title: Option<AudioClip>,
    music: Option<AudioClip>,
    title_voice: Option<VoiceId>,
    music_voice: Option<VoiceId>,
    title_vol: f32,
    music_vol: f32,
    baa_n: usize,
}

// ---------------------------------------------------------------- input

fn read_input(pads: Query<&Gamepad>, mut ctl: ResMut<Ctl>) {
    let mut c = Ctl::default();
    for p in &pads {
        let mut s = p.left_stick();
        if s.length() < 0.2 {
            s = Vec2::ZERO;
        }
        if p.pressed(GamepadButton::DPadLeft) { s.x -= 1.0; }
        if p.pressed(GamepadButton::DPadRight) { s.x += 1.0; }
        if p.pressed(GamepadButton::DPadUp) { s.y += 1.0; }
        if p.pressed(GamepadButton::DPadDown) { s.y -= 1.0; }
        // stick up = away from the camera = -z
        c.mv += Vec2::new(s.x, -s.y);
        c.a |= p.pressed(GamepadButton::South);
        c.a_just |= p.just_pressed(GamepadButton::South);
        c.b |= p.pressed(GamepadButton::East);
        c.start_just |= p.just_pressed(GamepadButton::Start);
    }
    c.mv = c.mv.clamp_length_max(1.0);
    *ctl = c;
}

// ---------------------------------------------------------------- flow

fn start_title(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut game: ResMut<Game>, kit: Res<scene::Kit>, mut rng: ResMut<Rng>, old: Query<Entity, With<FieldEnt>>) {
    game.field = 0;
    load_field(&mut commands, &mut meshes, &mut game, &kit, &mut rng, &old);
    game.set(Mode::Title);
}

fn load_field(commands: &mut Commands, meshes: &mut Assets<Mesh>, game: &mut Game, kit: &scene::Kit, rng: &mut Rng, old: &Query<Entity, With<FieldEnt>>) {
    for e in old {
        commands.entity(e).despawn();
    }
    let def = game.def();
    let obs = Obstacles::new(def);
    let grid = Grid::new(&obs);
    let home = grid.distances(obs.pen, 0.55, |_| 0.0);
    scene::spawn_field(commands, meshes, kit, def, &obs, rng);
    commands.insert_resource(Field { obs, grid, home });
    game.time_left = def.secs;
    game.penned = 0;
    game.sheep = def.sheep();
    game.need = def.need;
    game.gate_closed = false;
    game.dusk_warned = false;
    game.wrong_side = 0.0;
}

#[allow(clippy::too_many_arguments)]
fn flow(
    mut commands: Commands,
    time: Res<Time>,
    ctl: Res<Ctl>,
    mut game: ResMut<Game>,
    kit: Res<scene::Kit>,
    mut rng: ResMut<Rng>,
    old: Query<Entity, With<FieldEnt>>,
    dbg: Res<CartDebug>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    game.auto = dbg.get("autopilot").unwrap_or(0.0) > 0.5;
    let dt = time.delta_secs().min(0.05);
    game.t += dt;
    match game.mode {
        Mode::Title => {
            if (ctl.a_just || ctl.start_just) && game.t > 0.4 {
                game.score = 0;
                // a test host may start at a later field (debug field start_field)
                game.field = (dbg.get("start_field").unwrap_or(0.0).max(0.0) as usize).min(FIELDS.len() - 1);
                game.fields_cleared = 0;
                game.field_start_score = 0;
                load_field(&mut commands, &mut meshes, &mut game, &kit, &mut rng, &old);
                game.set(Mode::Intro);
                game.sfx.push((Sfx::Start, 0.8));
            }
        }
        Mode::Intro => {
            if game.t > 4.0 || (ctl.a_just && game.t > 0.6) {
                game.set(Mode::Play);
            }
        }
        Mode::Play => {
            if ctl.start_just {
                game.set(Mode::Paused);
                game.sfx.push((Sfx::Pause, 0.7));
                return;
            }
            game.time_left -= dt;
            if game.time_left < 25.0 && !game.dusk_warned && game.penned < game.need {
                game.dusk_warned = true;
                game.sfx.push((Sfx::Dusk, 0.7));
            }
            if game.gate_closed || game.penned == game.sheep {
                clear_field(&mut game);
            } else if game.time_left <= 0.0 {
                game.time_left = 0.0;
                if game.penned >= game.need {
                    clear_field(&mut game);
                } else {
                    game.set(Mode::Over);
                    game.sfx.push((Sfx::Fail, 0.9));
                    game.rumble = Some((0.7, 0.5, 450));
                }
            }
        }
        Mode::Paused => {
            if ctl.start_just || (ctl.a_just && game.t > 0.2) {
                game.mode = Mode::Play;
                game.sfx.push((Sfx::Pause, 0.5));
            }
        }
        Mode::Cleared => {
            if ctl.a_just && game.t > 1.5 {
                if game.field + 1 >= FIELDS.len() {
                    game.set(Mode::Victory);
                    game.sfx.push((Sfx::Win, 0.9));
                } else {
                    game.field += 1;
                    game.field_start_score = game.score;
                    load_field(&mut commands, &mut meshes, &mut game, &kit, &mut rng, &old);
                    game.set(Mode::Intro);
                    game.sfx.push((Sfx::Start, 0.8));
                }
            }
        }
        Mode::Over => {
            if game.t > 1.5 && ctl.a_just {
                game.score = game.field_start_score;
                load_field(&mut commands, &mut meshes, &mut game, &kit, &mut rng, &old);
                game.set(Mode::Intro);
                game.sfx.push((Sfx::Start, 0.8));
            } else if game.t > 1.5 && ctl.start_just {
                game.field = 0;
                load_field(&mut commands, &mut meshes, &mut game, &kit, &mut rng, &old);
                game.set(Mode::Title);
            }
        }
        Mode::Victory => {
            if game.t > 2.5 && (ctl.a_just || ctl.start_just) {
                game.field = 0;
                load_field(&mut commands, &mut meshes, &mut game, &kit, &mut rng, &old);
                game.set(Mode::Title);
            }
        }
    }
}

fn clear_field(game: &mut Game) {
    let sun = (game.time_left * 5.0) as u32;
    let extra = (game.penned.saturating_sub(game.need) * 50) as u32 + if game.penned == game.sheep { 500 } else { 0 };
    game.result = (game.penned, sun, extra);
    game.score += sun + extra;
    game.fields_cleared += 1;
    game.gate_closed = true;
    game.set(Mode::Cleared);
    game.sfx.push((Sfx::Gate, 0.9));
    game.sfx.push((Sfx::Clear, 0.9));
    game.rumble = Some((0.4, 0.4, 220));
}

// ---------------------------------------------------------------- the dog

fn dog_move(
    time: Res<Time>,
    ctl: Res<Ctl>,
    mut game: ResMut<Game>,
    field: Option<Res<Field>>,
    mut dogs: Query<&mut Dog>,
    mut commands: Commands,
    kit: Res<scene::Kit>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let Some(field) = field else { return };
    if !game.running() {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let Ok(mut dog) = dogs.single_mut() else { return };
    let free = game.dog_free();
    dog.sneak = free && ctl.b;
    let speed = if dog.sneak { 3.4 } else { 8.5 };
    let want = if free { ctl.mv * speed } else { Vec2::ZERO };
    let k = 1.0 - (-dt * 10.0).exp();
    dog.vel = dog.vel.lerp(want, k);
    let p = dog.pos + dog.vel * dt;
    dog.pos = field.obs.push_out(p, 0.35);
    let sp = dog.vel.length();
    if sp > 0.3 {
        let target = dog.vel.x.atan2(dog.vel.y);
        let mut d = target - dog.heading;
        while d > std::f32::consts::PI { d -= std::f32::consts::TAU; }
        while d < -std::f32::consts::PI { d += std::f32::consts::TAU; }
        dog.heading += d * (1.0 - (-dt * 14.0).exp());
    }
    dog.phase += dt * (2.0 + sp * 2.2);
    dog.bark_cd = (dog.bark_cd - dt).max(0.0);
    if game.mode == Mode::Play && ctl.a_just && dog.bark_cd <= 0.0 {
        dog.bark_cd = 0.7;
        game.barks += 1;
        game.sfx.push((Sfx::Bark, 0.9));
        game.rumble = Some((0.15, 0.3, 90));
        scene::spawn_bark_ring(&mut commands, &kit, &mut mats, dog.pos);
    }
}

// ---------------------------------------------------------------- the flock

#[allow(clippy::too_many_arguments)]
fn flock(
    time: Res<Time>,
    ctl: Res<Ctl>,
    mut game: ResMut<Game>,
    field: Option<Res<Field>>,
    dogs: Query<&Dog>,
    mut sheep: Query<&mut Sheep>,
    mut rng: ResMut<Rng>,
) {
    let Some(field) = field else { return };
    if !game.running() {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let (dog_pos, dog_sneak, dog_active) = match dogs.single() {
        Ok(d) => (d.pos, d.sneak, game.dog_free()),
        Err(_) => (Vec2::splat(999.0), false, false),
    };
    let barked = game.mode == Mode::Play && ctl.a_just && dogs.single().map(|d| d.bark_cd > 0.65).unwrap_or(false);
    let snap: Vec<(Vec2, Vec2, f32, bool)> = sheep.iter().map(|s| (s.pos, s.vel, s.alarm, s.penned)).collect();
    let obs = &field.obs;
    let mut baa = false;
    for mut s in &mut sheep {
        let p = s.pos;
        let mut sep = Vec2::ZERO;
        let mut coh = Vec2::ZERO;
        let mut ali = Vec2::ZERO;
        let mut n = 0.0;
        let mut alarm_n = 0.0;
        for &(q, v, a, pen) in &snap {
            // the fence splits the flock: sheep home and sheep out ignore
            // each other
            if s.penned != pen {
                continue;
            }
            let d = q - p;
            let l2 = d.length_squared();
            if l2 < 1e-6 || l2 > 9.0 {
                continue;
            }
            coh += q;
            ali += v;
            alarm_n += a;
            n += 1.0;
            if l2 < 1.2 * 1.2 {
                sep -= d / (l2 + 0.05);
            }
        }
        // fear of the dog
        let to = p - dog_pos;
        let dd = to.length().max(0.01);
        let flee_r = if dog_sneak { 3.0 } else { 7.5 };
        let mut threat = if dog_active && !s.penned { ((flee_r - dd) / flee_r).clamp(0.0, 1.0) } else { 0.0 };
        if barked && dd < 10.0 && !s.penned {
            s.panic = 1.0;
            if s.baa_cd <= 0.0 {
                baa = true;
            }
        }
        s.panic = (s.panic - dt * 0.8).max(0.0);
        threat = threat.max(s.panic * 0.9);
        // alarm spreads through the flock
        let spread = if n > 0.0 { alarm_n / n * 0.7 } else { 0.0 };
        s.alarm = (s.alarm - dt * 0.6).max(threat).max(spread * 0.85);
        let alarm = s.alarm;
        let mut acc = Vec2::ZERO;
        if threat > 0.0 {
            acc += to / dd * (threat * 18.0 + s.panic * 8.0);
        }
        if n > 0.0 {
            acc += (coh / n - p).clamp_length_max(2.5) * (0.35 + 2.4 * alarm);
            acc += (ali / n - s.vel) * (0.2 + 1.4 * alarm);
        }
        acc += sep * 3.2;
        // sheep know their fold: a driven sheep drifts along the way home
        if !s.penned && alarm > 0.05 {
            acc += field.grid.downhill(&field.home, p) * 5.0 * alarm;
        }
        // calm: graze and amble
        s.graze -= dt;
        if s.graze <= 0.0 {
            s.graze = rng.range(2.0, 6.0);
            let a = rng.range(0.0, std::f32::consts::TAU);
            s.wander = if rng.f() < 0.45 { Vec2::ZERO } else { Vec2::new(a.cos(), a.sin()) * rng.range(0.3, 0.7) };
        }
        let calm = (1.0 - alarm * 3.0).clamp(0.0, 1.0);
        acc += (s.wander - s.vel) * 1.2 * calm;
        acc += obs.avoid(p, s.vel) * (4.0 + 6.0 * alarm);
        let mut v = s.vel + acc * dt;
        let max = 0.8 + 5.2 * alarm + s.panic * 1.2;
        v = v.clamp_length_max(max);
        v *= 1.0 - dt * (0.6 + 1.2 * calm);
        let mut np = obs.push_out(p + v * dt, 0.42);
        if s.penned {
            np = obs.clamp_pen(np, 0.42);
        } else if obs.in_pen(np, 0.5) {
            s.penned = true;
            s.hop = 1.0;
            s.alarm = 0.0;
            s.panic = 0.0;
            game.penned += 1;
            game.score += 100;
            game.sfx.push((Sfx::Fold, 0.55));
        }
        // velocity follows what really happened (walls stop sheep)
        s.vel = if dt > 0.0 { (np - p) / dt } else { v };
        s.pos = np;
        let sp = s.vel.length();
        if sp > 0.15 {
            let target = s.vel.x.atan2(s.vel.y);
            let mut d = target - s.heading;
            while d > std::f32::consts::PI { d -= std::f32::consts::TAU; }
            while d < -std::f32::consts::PI { d += std::f32::consts::TAU; }
            s.heading += d * (1.0 - (-dt * 6.0).exp());
        }
        s.phase += dt * sp * 4.5;
        s.hop = (s.hop - dt * 1.6).max(0.0);
        s.baa_cd = (s.baa_cd - dt).max(0.0);
        if s.alarm > 0.7 && s.baa_cd <= 0.0 && rng.f() < dt * 0.25 {
            baa = true;
        }
        if baa && s.baa_cd <= 0.0 {
            s.baa_cd = rng.range(4.0, 9.0);
        }
    }
    if baa {
        game.sfx.push((Sfx::Baa, 0.5));
    }
    // flock summary for the camera, the hints and the autopilot
    let free: Vec<Vec2> = sheep.iter().filter(|s| !s.penned).map(|s| s.pos).collect();
    if !free.is_empty() {
        let c = free.iter().copied().sum::<Vec2>() / free.len() as f32;
        game.flock_c = c;
        game.flock_spread = free.iter().map(|p| p.distance(c)).fold(0.0, f32::max);
    }
}

/// The gate closes when the dog walks into the fold with enough sheep home,
/// and the "wrong side" hint watches where the dog stands.
fn fold_check(time: Res<Time>, mut game: ResMut<Game>, field: Option<Res<Field>>, dogs: Query<&Dog>, sheep: Query<&Sheep>) {
    let Some(field) = field else { return };
    if game.mode != Mode::Play {
        return;
    }
    let Ok(dog) = dogs.single() else { return };
    if game.penned >= game.need && field.obs.in_pen(dog.pos, 0.8) {
        game.gate_closed = true;
    }
    // the sheep near the dog are further from home than the dog is
    let home_dog = field.grid.at(&field.home, dog.pos);
    let (mut sum, mut n) = (0.0, 0.0);
    for s in &sheep {
        if !s.penned && s.pos.distance(dog.pos) < 7.0 {
            sum += field.grid.at(&field.home, s.pos).min(500.0);
            n += 1.0;
        }
    }
    if n > 0.0 && home_dog + 2.5 < sum / n {
        game.wrong_side += time.delta_secs();
    } else {
        game.wrong_side = (game.wrong_side - time.delta_secs() * 2.0).max(0.0);
    }
}

// ---------------------------------------------------------------- camera

fn camera(
    time: Res<Time>,
    game: Res<Game>,
    field: Option<Res<Field>>,
    dogs: Query<&Dog>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
    mut focus: Local<Option<Vec3>>,
) {
    let Ok(mut t) = cam.single_mut() else { return };
    let Some(field) = field else { return };
    let dt = time.delta_secs().min(0.05);
    let half = field.obs.half;
    if game.mode == Mode::Title {
        let a = game.t * 0.06;
        let c = Vec3::new(-2.0, 0.0, 0.0);
        let eye = c + Vec3::new(a.sin() * 30.0, 17.0, a.cos() * 30.0);
        *t = Transform::from_translation(eye).looking_at(c + Vec3::Y * 1.0, Vec3::Y);
        *focus = None;
        return;
    }
    let Ok(dog) = dogs.single() else { return };
    let c = game.flock_c;
    let toward = (c - dog.pos).clamp_length_max(9.0);
    let mut f2 = dog.pos + toward * 0.45;
    let lim = (half - Vec2::new(10.0, 6.0)).max(Vec2::ZERO);
    f2 = f2.clamp(-lim, lim);
    let want = Vec3::new(f2.x, 0.0, f2.y);
    let cur = focus.get_or_insert(want);
    *cur = cur.lerp(want, 1.0 - (-dt * 3.0).exp());
    let eye = *cur + Vec3::new(0.0, 21.0, 17.5);
    *t = Transform::from_translation(eye).looking_at(*cur + Vec3::new(0.0, 0.0, -0.5), Vec3::Y);
}

// ---------------------------------------------------------------- HUD

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Hud {
    TopLeft,
    FieldName,
    Count,
    Need,
    TopRight,
    SunFill,
    Score,
    HintPanel,
    Hint,
    Overlay,
    OvTitle,
    OvSub,
    OvBody,
    OvPrompt,
}

const INK: Color = Color::srgb(0.98, 0.96, 0.88);
const PANEL: Color = Color::srgba(0.07, 0.09, 0.07, 0.62);

fn setup_hud(mut commands: Commands) {
    let text = |s: &str, size: f32, color: Color| {
        (Text::new(s), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color))
    };
    // top left: field, sheep in the fold
    commands
        .spawn((
            Hud::TopLeft,
            Node {
                position_type: PositionType::Absolute,
                left: px(14),
                top: px(12),
                padding: UiRect::axes(px(14), px(8)),
                flex_direction: FlexDirection::Column,
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|p| {
            p.spawn((Hud::FieldName, text("", 15.0, Color::srgb(0.85, 0.92, 0.75))));
            p.spawn((Hud::Count, text("", 26.0, INK)));
            p.spawn((Hud::Need, text("", 15.0, Color::srgb(0.85, 0.85, 0.8))));
        });
    // top right: the sun and the score
    commands
        .spawn((
            Hud::TopRight,
            Node {
                position_type: PositionType::Absolute,
                right: px(14),
                top: px(12),
                padding: UiRect::axes(px(14), px(8)),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: px(4),
                border_radius: BorderRadius::all(px(10)),
                ..default()
            },
            BackgroundColor(PANEL),
        ))
        .with_children(|p| {
            p.spawn(text("SUNLIGHT", 13.0, Color::srgb(1.0, 0.86, 0.55)));
            p.spawn((
                Node {
                    width: px(180),
                    height: px(12),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            ))
            .with_children(|p| {
                p.spawn((
                    Hud::SunFill,
                    Node { width: percent(100), height: percent(100), border_radius: BorderRadius::all(px(6)), ..default() },
                    BackgroundColor(Color::srgb(1.0, 0.8, 0.3)),
                ));
            });
            p.spawn((Hud::Score, text("", 22.0, INK)));
        });
    // bottom: hints
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            bottom: px(16),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn((
                Hud::HintPanel,
                Node {
                    padding: UiRect::axes(px(16), px(8)),
                    max_width: px(760),
                    border_radius: BorderRadius::all(px(10)),
                    ..default()
                },
                BackgroundColor(PANEL),
                Visibility::Hidden,
            ))
            .with_children(|p| {
                p.spawn((Hud::Hint, text("", 19.0, INK), TextLayout::justify(Justify::Center)));
            });
        });
    // centre: title, field cards, pause, results
    commands
        .spawn((
            Hud::Overlay,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.35)),
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
            BackgroundColor(Color::srgba(0.05, 0.07, 0.06, 0.5)),
          ))
          .with_children(|p| {
            p.spawn((Hud::OvTitle, text("", 72.0, INK), TextLayout::justify(Justify::Center), TextShadow::default()));
            p.spawn((Hud::OvSub, text("", 26.0, Color::srgb(1.0, 0.88, 0.6)), TextLayout::justify(Justify::Center), TextShadow::default()));
            p.spawn((Hud::OvBody, text("", 20.0, INK), TextLayout::justify(Justify::Center), TextShadow::default()));
            p.spawn((Hud::OvPrompt, text("", 24.0, INK), TextLayout::justify(Justify::Center), TextShadow::default()));
          });
        });
}

#[allow(clippy::type_complexity)]
fn hud(
    game: Res<Game>,
    mut texts: Query<(&Hud, &mut Text, Option<&mut TextColor>)>,
    mut nodes: Query<(&Hud, &mut Node, Option<&mut BackgroundColor>), Without<Text>>,
    mut vis: Query<(&Hud, &mut Visibility)>,
) {
    let def = game.def();
    let m = game.mode;
    let playing = matches!(m, Mode::Play | Mode::Intro | Mode::Paused | Mode::Cleared | Mode::Over);
    let mut want: Vec<(Hud, String)> = Vec::new();
    let mut set = |tag: Hud, s: String| want.push((tag, s));
    set(Hud::FieldName, format!("FIELD {}  {}", game.field + 1, def.name.to_uppercase()));
    set(Hud::Count, format!("{} / {} in the fold", game.penned, game.sheep));
    set(
        Hud::Need,
        if game.penned >= game.need { "enough! run into the fold".to_string() } else { format!("need {}", game.need) },
    );
    set(Hud::Score, format!("{}", game.score));

    let (title, sub, body, prompt) = match m {
        Mode::Title => (
            "HILLFOLD".to_string(),
            "Herd the flock home before sundown".to_string(),
            "Left stick: run     A: bark     hold B: sneak     Start: pause".to_string(),
            "Press A to start".to_string(),
        ),
        Mode::Intro => (
            format!("Field {}", game.field + 1),
            def.name.to_string(),
            format!("Bring {} of {} sheep into the fold before the sun sets.\n{}", def.need, game.sheep, def.hint),
            "A: go".to_string(),
        ),
        Mode::Paused => ("PAUSED".into(), String::new(), String::new(), "Start: resume".into()),
        Mode::Cleared => {
            let (n, sun, extra) = game.result;
            (
                "FIELD CLEAR".into(),
                format!("{n} sheep home"),
                format!("sunlight bonus {sun}     extra sheep bonus {extra}\nscore {}", game.score),
                if game.t > 1.5 {
                    if game.field + 1 >= FIELDS.len() { "A: continue".into() } else { "A: next field".into() }
                } else {
                    String::new()
                },
            )
        }
        Mode::Over => (
            "NIGHT FALLS".into(),
            format!("{} of the {} needed sheep made it home", game.penned, game.need),
            format!("score {}", game.score),
            if game.t > 1.5 { "A: try this field again     Start: title".into() } else { String::new() },
        ),
        Mode::Victory => (
            "EVERY FOLD IS FULL".into(),
            "Five fields herded home".into(),
            format!("final score {}", game.score),
            if game.t > 2.5 { "A: title".into() } else { String::new() },
        ),
        Mode::Play => (String::new(), String::new(), String::new(), String::new()),
    };
    set(Hud::OvTitle, title);
    set(Hud::OvSub, sub);
    set(Hud::OvBody, body);
    set(Hud::OvPrompt, prompt);
    // the prompt breathes slowly (well under 1 Hz, never a flash)
    let breathe = 0.7 + 0.3 * (game.t * 2.4).sin();
    for (h, _, c) in &mut texts {
        if *h == Hud::OvPrompt {
            if let Some(mut c) = c {
                c.0 = Color::srgba(1.0, 0.95, 0.7, breathe);
            }
        }
    }

    // hint
    let hint = if m == Mode::Play {
        if game.penned >= game.need && game.penned < game.sheep {
            Some("Enough are home! Run into the fold to close the gate, or bring the rest for a bonus.")
        } else if game.wrong_side > 2.0 {
            Some("Get behind the flock: sheep walk away from you.")
        } else if game.time_left < 25.0 && game.penned < game.need {
            Some("The sun is setting! Bark (A) to hurry them along.")
        } else if game.t < 14.0 && game.field == 0 {
            Some("Run behind the sheep to drive them. The gold arrow marks the fold's gate.  A: bark   hold B: sneak")
        } else if game.t < 7.0 {
            Some(def.hint)
        } else {
            None
        }
    } else {
        None
    };
    set(Hud::Hint, hint.unwrap_or("").to_string());
    for (h, mut t, _) in &mut texts {
        if let Some((_, s)) = want.iter().find(|w| w.0 == *h) {
            if t.0 != *s {
                t.0 = s.clone();
            }
        }
    }

    let day = game.day();
    for (h, mut n, bg) in &mut nodes {
        let Some(mut bg) = bg else { continue };
        if *h == Hud::SunFill {
            let w = Val::Percent(day * 100.0);
            if n.width != w {
                n.width = w;
            }
            bg.0 = Color::srgb(1.0, 0.35 + 0.5 * day, 0.15 + 0.2 * day);
        }
        if *h == Hud::Overlay {
            {
                let a = match m {
                    Mode::Title | Mode::Play => 0.0,
                    Mode::Intro => 0.1,
                    _ => 0.3,
                };
                bg.0 = Color::srgba(0.02, 0.03, 0.05, a);
            }
        }
    }
    for (h, mut v) in &mut vis {
        let show = match h {
            Hud::TopLeft | Hud::TopRight => playing,
            Hud::HintPanel => hint.is_some(),
            Hud::Overlay => m != Mode::Play,
            _ => continue,
        };
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *v != want {
            *v = want;
        }
    }
}

// ---------------------------------------------------------------- audio

fn load_sounds(mut s: ResMut<Sounds>) {
    let load = |name: &str| {
        wasmcart_bevy::assets::load_bytes(name).and_then(|b| AudioClip::from_wav(&b).ok())
    };
    let set = [
        (Sfx::Bark, &["bark.wav", "bark2.wav"][..]),
        (Sfx::Baa, &["baa1.wav", "baa2.wav", "baa3.wav"][..]),
        (Sfx::Fold, &["fold.wav"][..]),
        (Sfx::Gate, &["gate.wav"][..]),
        (Sfx::Clear, &["clear.wav"][..]),
        (Sfx::Fail, &["fail.wav"][..]),
        (Sfx::Start, &["start.wav"][..]),
        (Sfx::Pause, &["pause.wav"][..]),
        (Sfx::Dusk, &["dusk.wav"][..]),
        (Sfx::Win, &["win.wav"][..]),
    ];
    for (k, names) in set {
        let clips: Vec<AudioClip> = names.iter().filter_map(|n| load(n)).collect();
        s.clips.push((k, clips));
    }
    s.title = load("title.wav");
    s.music = load("music.wav");
}

fn audio_out(time: Res<Time>, audio: Res<Audio>, mut s: ResMut<Sounds>, mut game: ResMut<Game>) {
    let dt = time.delta_secs().min(0.05);
    let fx = std::mem::take(&mut game.sfx);
    for (k, vol) in fx {
        let n = s.baa_n;
        s.baa_n += 1;
        if let Some((_, clips)) = s.clips.iter().find(|c| c.0 == k) {
            if !clips.is_empty() {
                let clip = &clips[n % clips.len()];
                let speed = if k == Sfx::Baa { 0.92 + 0.06 * (n % 4) as f32 } else { 1.0 };
                audio.play_with(clip, PlaySettings { volume: vol, speed, ..default() });
            }
        }
    }
    if s.title_voice.is_none() {
        if let Some(c) = s.title.clone() {
            s.title_voice = Some(audio.play_with(&c, PlaySettings { volume: 0.0, looped: true, ..default() }));
        }
    }
    if s.music_voice.is_none() {
        if let Some(c) = s.music.clone() {
            s.music_voice = Some(audio.play_with(&c, PlaySettings { volume: 0.0, looped: true, ..default() }));
        }
    }
    let (mt, tt) = match game.mode {
        Mode::Title | Mode::Victory => (0.0, 0.55),
        Mode::Intro | Mode::Play | Mode::Cleared => (0.42, 0.0),
        Mode::Paused => (0.12, 0.0),
        Mode::Over => (0.0, 0.2),
    };
    let k = 1.0 - (-dt * 1.5).exp();
    s.music_vol += (mt - s.music_vol) * k;
    s.title_vol += (tt - s.title_vol) * k;
    if let Some(v) = s.music_voice {
        audio.set_volume(v, s.music_vol);
    }
    if let Some(v) = s.title_voice {
        audio.set_volume(v, s.title_vol);
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

fn publish(mut dbg: ResMut<CartDebug>, game: Res<Game>, dogs: Query<&Dog>, au: Res<auto::Auto>, mut once: Local<bool>) {
    dbg.set("auto_state", au.state as f32);
    dbg.set("auto_tx", au.target.x);
    dbg.set("auto_tz", au.target.y);
    dbg.set("auto_wx", au.way.x);
    dbg.set("auto_wz", au.way.y);
    if !*once {
        *once = true;
        dbg.set("autopilot", 0.0);
        dbg.set("start_field", 0.0);
    }
    dbg.set("mode", game.mode as u32 as f32);
    dbg.set("field", game.field as f32);
    dbg.set("penned", game.penned as f32);
    dbg.set("sheep", game.sheep as f32);
    dbg.set("need", game.need as f32);
    dbg.set("time_left", game.time_left);
    dbg.set("score", game.score as f32);
    dbg.set("cleared", game.fields_cleared as f32);
    dbg.set("barks", game.barks as f32);
    dbg.set("flock_x", game.flock_c.x);
    dbg.set("flock_z", game.flock_c.y);
    dbg.set("spread", game.flock_spread);
    dbg.set("wrong_side", game.wrong_side);
    if let Ok(d) = dogs.single() {
        dbg.set("dog_x", d.pos.x);
        dbg.set("dog_z", d.pos.y);
        dbg.set("sneak", d.sneak as u32 as f32);
    }
}
