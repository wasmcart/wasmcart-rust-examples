//! Gamepad input for the wasmcart port.
//!
//! wasmcart-macroquad turns every pad into keys (d-pad and left stick:
//! arrows, A: Space, B: X, X: Z, Y: C, Start: Enter, Select: Tab, L: Q,
//! R: E), so reading those keys reads every pad and the keyboard alike.
//! Escape also cancels. Directions repeat while held.

use mq::{
    input::{
        is_key_down, is_key_pressed, is_mouse_button_pressed, mouse_position, KeyCode, MouseButton,
    },
    math::Vec2,
};

/// One frame of gamepad presses.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    /// A direction press (or a held direction's repeat): x and y in -1..=1,
    /// y down.
    pub dir: Option<(i32, i32)>,
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub l: bool,
    pub r: bool,
    pub start: bool,
    pub select: bool,
}

impl Pad {
    pub fn any(&self) -> bool {
        *self != Pad::default()
    }

    /// The buttons as wasmcart WC_BTN_* bits (for the debug values).
    pub fn bits(&self) -> i32 {
        let mut b = 0;
        let mut set = |on: bool, bit: i32| {
            if on {
                b |= 1 << bit;
            }
        };
        set(self.a, 0);
        set(self.b, 1);
        set(self.x, 2);
        set(self.y, 3);
        set(self.l, 4);
        set(self.r, 5);
        set(self.start, 6);
        set(self.select, 7);
        if let Some((dx, dy)) = self.dir {
            set(dy < 0, 8);
            set(dy > 0, 9);
            set(dx < 0, 10);
            set(dx > 0, 11);
        }
        b
    }
}

const REPEAT_DELAY: f32 = 0.35;
const REPEAT_EVERY: f32 = 0.12;

#[derive(Debug)]
pub struct Input {
    held: (i32, i32),
    repeat_in: f32,
    last_mouse: Option<Vec2>,
    mouse_moved: bool,
}

impl Input {
    pub fn new() -> Self {
        Self {
            held: (0, 0),
            repeat_in: 0.0,
            last_mouse: None,
            mouse_moved: false,
        }
    }

    /// Reads this frame's presses. Also switches the focus outlines and
    /// button prompts on (a pad or key was used) or off (the mouse moved or
    /// clicked, or a finger touched).
    pub fn poll(&mut self, dt: f32) -> Pad {
        let axis = |neg: KeyCode, pos: KeyCode| is_key_down(pos) as i32 - is_key_down(neg) as i32;
        let held = (
            axis(KeyCode::Left, KeyCode::Right),
            axis(KeyCode::Up, KeyCode::Down),
        );
        let mut dir = None;
        if held != (0, 0) {
            if held != self.held {
                dir = Some(held);
                self.repeat_in = REPEAT_DELAY;
            } else {
                self.repeat_in -= dt;
                if self.repeat_in <= 0.0 {
                    dir = Some(held);
                    self.repeat_in = REPEAT_EVERY;
                }
            }
        }
        self.held = held;
        let pad = Pad {
            dir,
            a: is_key_pressed(KeyCode::Space),
            b: is_key_pressed(KeyCode::X) || is_key_pressed(KeyCode::Escape),
            x: is_key_pressed(KeyCode::Z),
            y: is_key_pressed(KeyCode::C),
            l: is_key_pressed(KeyCode::Q),
            r: is_key_pressed(KeyCode::E),
            start: is_key_pressed(KeyCode::Enter),
            select: is_key_pressed(KeyCode::Tab),
        };
        if pad.any() {
            ui::set_focus_visible(true);
        }
        self.mouse_moved = self.check_mouse_moved();
        if self.mouse_moved || is_mouse_button_pressed(MouseButton::Left) {
            ui::set_focus_visible(false);
        }
        pad
    }

    /// True when the pointer moved this frame.
    pub fn mouse_moved(&self) -> bool {
        self.mouse_moved
    }

    // The first frame only records where the pointer is.
    fn check_mouse_moved(&mut self) -> bool {
        let p: Vec2 = mouse_position().into();
        let moved = matches!(self.last_mouse, Some(last) if last.distance(p) > 0.5);
        self.last_mouse = Some(p);
        moved
    }
}
