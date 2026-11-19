#![windows_subsystem = "windows"]

use std::time::Duration;

use mq::{input as mq_input, window};
use wasmcart_macroquad::debug;

mod assets;
mod core;
mod error;
mod geom;
mod input;
mod screen;
mod utils;

type ZResult<T = ()> = Result<T, error::ZError>;

// See getrandom in Cargo.toml: nothing asks it for numbers.
#[cfg(target_arch = "wasm32")]
fn no_os_random(_: &mut [u8]) -> Result<(), getrandom::Error> {
    Err(getrandom::Error::UNSUPPORTED)
}
#[cfg(target_arch = "wasm32")]
getrandom::register_custom_getrandom!(no_os_random);

struct MainState {
    screens: screen::ScreenStack,
    input: input::Input,
}

impl MainState {
    fn new() -> ZResult<Self> {
        let start_screen = Box::new(screen::MainMenu::new()?);
        let screens = screen::ScreenStack::new(start_screen)?;
        Ok(Self {
            screens,
            input: input::Input::new(),
        })
    }

    fn tick(&mut self) -> ZResult {
        // Handle possible window resize and create a camera.
        let aspect_ratio = utils::aspect_ratio();
        let camera = utils::make_and_set_camera(aspect_ratio);
        self.screens.resize(aspect_ratio)?;
        // Handle user input events.
        let dtime = Duration::from_secs_f32(mq::time::get_frame_time());
        let pad = self.input.poll(dtime.as_secs_f32());
        let pos = utils::get_world_mouse_pos(&camera);
        // Hover follows the pointer only when it moves, so a still mouse
        // leaves the gamepad cursor where the pad put it.
        if self.input.mouse_moved() {
            self.screens.move_mouse(pos)?;
        }
        if mq_input::is_mouse_button_pressed(mq_input::MouseButton::Left) {
            self.screens.move_mouse(pos)?;
            self.screens.click(pos)?;
        }
        if pad.any() {
            self.screens.pad(&pad)?;
        }
        debug::set_i32("pad_bits", pad.bits());
        debug::set_i32("screen", self.screens.top_kind());
        // Update the game state.
        self.screens.update(dtime)?;
        // Draw everything.
        mq::window::clear_background(screen::COLOR_SCREEN_BG);
        self.screens.draw()?;
        self.screens.draw_prompts();
        Ok(())
    }
}

fn window_conf() -> window::Conf {
    window::Conf {
        window_title: "Zemeroth".to_owned(),
        // The cart's size: 16:9, the shape of most screens it will play on.
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[mq::main(window_conf)]
#[macroquad(crate_rename = "mq")]
async fn main() -> ZResult {
    // std::env isn't supported on WASM.
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var("RUST_BACKTRACE").is_err() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    env_logger::init();
    quad_rand::srand(mq::miniquad::date::now() as _);
    mq::file::set_pc_assets_folder("assets");
    assets::load().await.expect("Can't load assets");
    // Register the test switches (see screen/battle/autopilot.rs and
    // Battle::new) so a debug host can set them before any battle.
    debug::input_i32("autopilot", 0);
    debug::input_i32("seed", 0);
    let mut state = MainState::new().expect("Can't create the main state");
    loop {
        state.tick().expect("Tick failed");
        window::next_frame().await;
    }
}
