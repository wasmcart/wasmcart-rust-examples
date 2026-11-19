//! Starwell: fly a glider down an endless crystal canyon on raw wgpu.
//!
//! A wasmcart WebGPU cart: wgpu through wasmcart-wgpu for rendering (lit
//! instanced meshes, a compute-driven particle system, a bitmap-font HUD),
//! a small software mixer feeding the audio ring, and named debug values a
//! bot or test can read.

mod dbg;
mod font;
mod gfx;
mod math;
mod meshes;
mod shaders;
mod sound;
mod world;

use wasmcart::*;
use wasmcart_wgpu::Gpu;

use world::{Game, Input, Mode, Sfx};

pub const W: u32 = 960;
pub const H: u32 = 540;

wc_cart!(
    W,
    H,
    gpu_api: WC_GPU_API_WEBGPU,
    flags: WC_FLAG_DEBUG | WC_FLAG_KEYBOARD,
    audio_cap: 8192,
    save_size: 16
);

const SAVE_MAGIC: &[u8; 4] = b"SWL1";

struct Sounds {
    title: usize,
    music: usize,
    engine: usize,
    gate: usize,
    hit: usize,
    start: usize,
    sector: usize,
    over: usize,
    pause: usize,
}

struct State {
    gfx: gfx::Renderer,
    game: Game,
    hud: font::Hud,
    mixer: sound::Mixer,
    snd: Sounds,
    saved_best: u32,
    music_vol: f32,
    title_vol: f32,
}

static mut STATE: Option<State> = None;

fn load_best() -> u32 {
    let s = save();
    if s.len() >= 8 && &s[0..4] == SAVE_MAGIC {
        u32::from_le_bytes([s[4], s[5], s[6], s[7]])
    } else {
        0
    }
}

fn store_best(best: u32) {
    let s = save();
    if s.len() >= 8 {
        s[0..4].copy_from_slice(SAVE_MAGIC);
        s[4..8].copy_from_slice(&best.to_le_bytes());
    }
}

#[no_mangle]
pub extern "C" fn wc_init() {
    std::panic::set_hook(Box::new(|info| wc_log(&info.to_string())));
    let gpu = Gpu::new(W, H).expect("this host has no WebGPU device");
    wc_log(&format!("starwell: device core={} srgb={}", wasmcart_wgpu::is_core(&gpu.device), gpu.srgb));
    let renderer = gfx::Renderer::new(gpu);

    let rate = match host_info().audio_sample_rate {
        0 => 48000,
        r => r,
    };
    info().audio_sample_rate = rate;
    let mut mixer = sound::Mixer::new(rate);
    let mut add = |name: &str| mixer.add(sound::load(name));
    let snd = Sounds {
        title: add("title.wav"),
        music: add("music.wav"),
        engine: add("engine.wav"),
        gate: add("gate.wav"),
        hit: add("hit.wav"),
        start: add("start.wav"),
        sector: add("sector.wav"),
        over: add("over.wav"),
        pause: add("pause.wav"),
    };

    let best = load_best();
    let seed = 0x5eed_1234;
    let game = Game::new(best, seed);
    unsafe {
        STATE = Some(State {
            gfx: renderer,
            game,
            hud: font::Hud::default(),
            mixer,
            snd,
            saved_best: best,
            music_vol: 0.0,
            title_vol: 0.0,
        })
    };
    wc_log("starwell: ready");
}

/// Every connected pad's input OR'd together (the strongest stick wins), plus
/// the keyboard as a fallback.
fn read_input() -> Input {
    let mut inp = Input::default();
    let (mut best_x, mut best_y) = (0.0f32, 0.0f32);
    let dead = |v: i16| {
        let f = v as f32 / 32767.0;
        if f.abs() < 0.2 {
            0.0
        } else {
            (f.signum() * (f.abs() - 0.2) / 0.8).clamp(-1.0, 1.0)
        }
    };
    let mut buttons = 0u32;
    for p in pads().iter() {
        if !p.is_connected() {
            continue;
        }
        buttons |= p.buttons;
        let (sx, sy) = (dead(p.left_x), dead(p.left_y));
        if sx.abs() > best_x.abs() {
            best_x = sx;
        }
        if sy.abs() > best_y.abs() {
            best_y = sy;
        }
    }
    let k = keys();
    let key = |c: u8| key_is_down(k, c);
    let mut dx = 0.0;
    let mut dy = 0.0;
    if buttons & WC_BTN_LEFT != 0 || key(key::LEFT) || key(key::A) {
        dx -= 1.0;
    }
    if buttons & WC_BTN_RIGHT != 0 || key(key::RIGHT) || key(key::D) {
        dx += 1.0;
    }
    if buttons & WC_BTN_UP != 0 || key(key::UP) || key(key::W) {
        dy += 1.0;
    }
    if buttons & WC_BTN_DOWN != 0 || key(key::DOWN) || key(key::S) {
        dy -= 1.0;
    }
    // Stick y is down-positive (SDL convention); the game's y is up.
    inp.x = if dx != 0.0 { dx } else { best_x };
    inp.y = if dy != 0.0 { dy } else { -best_y };
    inp.a = buttons & WC_BTN_A != 0 || key(key::SPACE) || key(key::ENTER) || key(key::Z);
    inp.start = buttons & WC_BTN_START != 0 || key(key::ESCAPE) || key(key::P);
    inp
}

#[no_mangle]
pub extern "C" fn wc_render() {
    #[allow(static_mut_refs)]
    let Some(s) = (unsafe { STATE.as_mut() }) else { return };
    let tm = time();
    let mut dt = (tm.delta_ms / 1000.0) as f32;
    if !(dt > 0.0) {
        dt = 1.0 / 60.0;
    }
    let dt = dt.min(0.05);

    let inp = read_input();
    s.game.autoplay = dbg::get_u32("autoplay") != 0;
    s.game.update(dt, inp);
    let fd = s.game.frame_data(dt);
    s.game.hud(&mut s.hud);
    s.gfx.draw(&fd, &s.hud.quads);

    // Sound: effects, crossfaded music, an engine hum that follows speed.
    for &(fx, vol) in s.game.sfx.iter() {
        let id = match fx {
            Sfx::Gate => s.snd.gate,
            Sfx::Hit => s.snd.hit,
            Sfx::Start => s.snd.start,
            Sfx::Sector => s.snd.sector,
            Sfx::Over => s.snd.over,
            Sfx::Pause => s.snd.pause,
        };
        let pitch = if fx == Sfx::Gate { 1.0 + 0.06 * s.game.combo.saturating_sub(1) as f32 } else { 1.0 };
        s.mixer.play(id, vol, (s.game.px / 12.0).clamp(-0.5, 0.5), pitch);
    }
    let mode = s.game.mode;
    let (mt, tt) = match mode {
        Mode::Title => (0.0, 0.55),
        Mode::Play => (0.5, 0.0),
        Mode::Paused => (0.15, 0.0),
        Mode::Over => (0.12, 0.25),
    };
    let fade = 1.0 - (-dt * 2.0).exp();
    s.music_vol += (mt - s.music_vol) * fade;
    s.title_vol += (tt - s.title_vol) * fade;
    s.mixer.loop_vol(1, s.snd.music, s.music_vol, 1.0);
    s.mixer.loop_vol(2, s.snd.title, s.title_vol, 1.0);
    let engine = if mode == Mode::Paused { 0.0 } else { (s.game.speed / 40.0).min(1.0) * 0.22 };
    s.mixer.loop_vol(3, s.snd.engine, engine, 0.8 + s.game.speed / 60.0);
    s.mixer.mix(tm.time_ms);

    if let Some((lo, hi, ms)) = s.game.rumble {
        for (i, p) in pads().iter().enumerate() {
            if p.is_connected() {
                wc_pad_rumble(i as u32, lo, hi, ms);
            }
        }
    }

    if s.game.best > s.saved_best && mode != Mode::Play {
        store_best(s.game.best);
        s.saved_best = s.game.best;
    }

    publish(s);
}

fn publish(s: &State) {
    let g = &s.game;
    dbg::u32("mode", g.mode as u32);
    dbg::u32("score", g.score);
    dbg::u32("best", g.best);
    dbg::u32("sector", g.sector);
    dbg::u32("goal", g.goal as u32);
    dbg::f32("dist", g.dist);
    dbg::f32("speed", g.speed);
    dbg::f32("energy", g.energy);
    dbg::f32("player.x", g.px);
    dbg::f32("player.y", g.py);
    let (gx, gy, gz) = g.next_gate().map(|(x, y, dz)| (x - g.px, y - g.py, dz)).unwrap_or((0.0, 0.0, 999.0));
    dbg::f32("gate.dx", gx);
    dbg::f32("gate.dy", gy);
    dbg::f32("gate.dz", gz);
    dbg::u32("gates", g.gates);
    dbg::u32("hits", g.hits);
    dbg::u32("combo", g.combo);
    dbg::f32s("pillars", &g.pillars_ahead());
    let (by, bz) = g.beam_ahead();
    dbg::f32("beam.y", by);
    dbg::f32("beam.dz", bz);
    dbg::u32("frame", g.frame);
    dbg::f32("audio.level", s.mixer.level);
}
