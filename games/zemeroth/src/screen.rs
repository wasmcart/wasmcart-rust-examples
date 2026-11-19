use std::{fmt::Debug, time::Duration};

use log::info;
use mq::{
    color::Color,
    math::{Rect, Vec2},
};

use crate::{assets, input::Pad, utils, ZResult};

mod agent_info;
mod battle;
mod campaign;
mod confirm;
mod general_info;
mod main_menu;

pub use self::{
    agent_info::AgentInfo, battle::Battle, campaign::Campaign, confirm::Confirm,
    general_info::GeneralInfo, main_menu::MainMenu,
};

pub const COLOR_SCREEN_BG: Color = Color::new(0.9, 0.9, 0.8, 1.0);
pub const COLOR_POPUP_BG: Color = Color::new(0.9, 0.9, 0.8, 0.9);

#[derive(Debug)]
pub enum StackCommand {
    None,
    PushScreen(Box<dyn Screen>),
    PushPopup(Box<dyn Screen>),
    Pop,
}

pub trait Screen: Debug {
    fn update(&mut self, dtime: Duration) -> ZResult<StackCommand>;
    fn draw(&self) -> ZResult;
    fn click(&mut self, pos: Vec2) -> ZResult<StackCommand>;
    fn resize(&mut self, aspect_ratio: f32);

    fn move_mouse(&mut self, _pos: Vec2) -> ZResult {
        Ok(())
    }

    /// Gamepad presses (this port's addition).
    fn pad(&mut self, _pad: &Pad) -> ZResult<StackCommand> {
        Ok(StackCommand::None)
    }

    /// The gamepad button prompts shown while a pad is in use.
    fn prompts(&self) -> Vec<Prompt> {
        vec![("A", "choose"), ("B", "back")]
    }

    /// A short name for the debug values (screen kind).
    fn kind(&self) -> i32 {
        0
    }
}

/// A button and what it does, for the prompt bar.
pub type Prompt = (&'static str, &'static str);

/// Gamepad navigation over a screen's GUI: the d-pad moves the focus and
/// returns None; A returns the focused button's centre, to be clicked.
pub fn menu_nav<M: Clone>(gui: &mut ui::Gui<M>, pad: &Pad) -> Option<Vec2> {
    if let Some((dx, dy)) = pad.dir {
        gui.focus_move(Vec2::new(dx as f32, dy as f32));
    }
    if pad.a {
        gui.focus_point()
    } else {
        None
    }
}

const COLOR_PROMPT_BG: Color = Color::new(0.15, 0.13, 0.1, 0.8);
const COLOR_PROMPT_TEXT: Color = Color::new(0.95, 0.93, 0.85, 1.0);
const COLOR_PROMPT_KEY: Color = Color::new(0.85, 0.45, 0.0, 1.0);

/// Draws the prompt bar along the top edge, centred. UI coordinates: the
/// screen is 2 units high, y down.
fn draw_prompts(prompts: &[Prompt]) {
    if prompts.is_empty() {
        return;
    }
    let font = &assets::get().font;
    let h = 0.05;
    let (font_size, font_scale, font_aspect) = mq::text::camera_font_scale(h);
    let params = |color| mq::text::TextParams {
        font: Some(font),
        font_size,
        font_scale,
        font_scale_aspect: font_aspect,
        color,
        ..Default::default()
    };
    let measure = |t: &str| mq::text::measure_text(t, Some(font), font_size, font_scale).width;
    let pad_x = h * 0.35;
    let gap = h * 0.9;
    let widths: Vec<(f32, f32)> = prompts
        .iter()
        .map(|(key, what)| (measure(key) + pad_x * 2.0, measure(what)))
        .collect();
    let total: f32 =
        widths.iter().map(|(k, w)| k + pad_x + w).sum::<f32>() + gap * (prompts.len() as f32 - 1.0);
    let bar_h = h * 1.7;
    let top = -1.0 + 0.02;
    let mut x = -total / 2.0;
    mq::shapes::draw_rectangle(x - gap * 0.5, top, total + gap, bar_h, COLOR_PROMPT_BG);
    let base = top + bar_h * 0.5 + h * 0.35;
    for ((key, what), (kw, ww)) in prompts.iter().zip(widths) {
        let box_h = h * 1.3;
        mq::shapes::draw_rectangle(x, top + (bar_h - box_h) / 2.0, kw, box_h, COLOR_PROMPT_KEY);
        ui::draw_text_unkerned(key, x + pad_x, base, params(COLOR_PROMPT_TEXT));
        x += kw + pad_x;
        ui::draw_text_unkerned(what, x, base, params(COLOR_PROMPT_TEXT));
        x += ww + gap;
    }
}

const ERR_MSG_STACK_EMPTY: &str = "Screen stack is empty";

struct ScreenWithPopups {
    screen: Box<dyn Screen>,
    popups: Vec<Box<dyn Screen>>,
}

impl ScreenWithPopups {
    fn new(screen: Box<dyn Screen>) -> Self {
        Self {
            screen,
            popups: Vec::new(),
        }
    }

    fn top_mut(&mut self) -> &mut dyn Screen {
        match self.popups.last_mut() {
            Some(popup) => popup.as_mut(),
            None => self.screen.as_mut(),
        }
    }
}

pub struct ScreenStack {
    screens: Vec<ScreenWithPopups>,
}

impl ScreenStack {
    pub fn new(start_screen: Box<dyn Screen>) -> ZResult<Self> {
        Ok(Self {
            screens: vec![ScreenWithPopups::new(start_screen)],
        })
    }

    pub fn update(&mut self, dtime: Duration) -> ZResult {
        let command = self.screen_mut().top_mut().update(dtime)?;
        self.handle_command(command)
    }

    pub fn draw(&self) -> ZResult {
        let screen = self.screen();
        screen.screen.draw()?;
        for popup in &screen.popups {
            self.draw_popup_bg();
            popup.draw()?;
        }
        Ok(())
    }

    pub fn click(&mut self, pos: Vec2) -> ZResult {
        let command = self.screen_mut().top_mut().click(pos)?;
        self.handle_command(command)
    }

    pub fn move_mouse(&mut self, pos: Vec2) -> ZResult {
        self.screen_mut().top_mut().move_mouse(pos)
    }

    pub fn pad(&mut self, pad: &Pad) -> ZResult {
        let command = self.screen_mut().top_mut().pad(pad)?;
        self.handle_command(command)
    }

    /// The prompt bar for the top screen or popup, while a pad is in use.
    pub fn draw_prompts(&self) {
        if !ui::is_focus_visible() {
            return;
        }
        let screen = self.screen();
        let top = match screen.popups.last() {
            Some(popup) => popup.as_ref(),
            None => screen.screen.as_ref(),
        };
        utils::make_and_set_camera(utils::aspect_ratio());
        draw_prompts(&top.prompts());
    }

    /// The kind of the top screen or popup (debug values).
    pub fn top_kind(&self) -> i32 {
        let screen = self.screen();
        match screen.popups.last() {
            Some(popup) => popup.kind(),
            None => screen.screen.kind(),
        }
    }

    pub fn resize(&mut self, aspect_ratio: f32) -> ZResult {
        for screen in &mut self.screens {
            screen.screen.resize(aspect_ratio);
            for popup in &mut screen.popups {
                popup.resize(aspect_ratio);
            }
        }
        Ok(())
    }

    pub fn handle_command(&mut self, command: StackCommand) -> ZResult {
        match command {
            StackCommand::None => {}
            StackCommand::PushScreen(screen) => {
                info!("Screens::handle_command: PushScreen");
                self.screens.push(ScreenWithPopups::new(screen));
            }
            StackCommand::Pop => {
                info!("Screens::handle_command: Pop");
                let popups = &mut self.screen_mut().popups;
                if !popups.is_empty() {
                    popups.pop().expect(ERR_MSG_STACK_EMPTY);
                } else if self.screens.len() > 1 {
                    self.screens.pop().expect(ERR_MSG_STACK_EMPTY);
                } else {
                    std::process::exit(0);
                }
            }
            StackCommand::PushPopup(screen) => {
                info!("Screens::handle_command: PushPopup");
                self.screen_mut().popups.push(screen);
            }
        }
        Ok(())
    }

    /// Returns a mutable reference to the top screen.
    fn screen_mut(&mut self) -> &mut ScreenWithPopups {
        self.screens.last_mut().expect(ERR_MSG_STACK_EMPTY)
    }

    /// Returns a reference to the top screen.
    fn screen(&self) -> &ScreenWithPopups {
        self.screens.last().expect(ERR_MSG_STACK_EMPTY)
    }

    fn draw_popup_bg(&self) {
        let aspect_ratio = utils::aspect_ratio();
        let r = Rect::new(-aspect_ratio, -1.0, aspect_ratio * 2.0, 2.0);
        mq::shapes::draw_rectangle(r.x, r.y, r.w, r.h, COLOR_POPUP_BG);
    }
}
