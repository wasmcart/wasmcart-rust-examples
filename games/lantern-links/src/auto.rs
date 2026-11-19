//! The test autopilot, switched on by a debugging host writing the debug
//! field `autopilot` to 1. It plays with the same controls a pad gives: it
//! looks ahead by running the game's own ball physics for a few thousand
//! candidate putts (spread over a few frames), picks the one that sinks or
//! ends nearest the cup along the fairway, then points the stick, holds A
//! for exactly that power and lets go.

use bevy::prelude::*;

use crate::course::{self, Hole};
use wasmcart_bevy::prelude::CartDebug;

use crate::{CHARGE_SECS, Course, Ctl, Game, Mode};

const ANGLES: usize = 120;
const MAX_HOLD: u32 = 66;
const HOLD_STEP: u32 = 3;
const PER_FRAME: usize = 120;
const SEARCH_FRAMES: u32 = 30;
const AIM_FRAMES: u32 = 4;

#[derive(Resource, Default)]
pub struct Auto {
    pub state: u32,
    cands: Vec<(f32, u32)>,
    next: usize,
    best: Option<(f32, f32, u32)>,
    refined: bool,
    t_ref: f32,
    count: u32,
    frame: u32,
    seed: u32,
}

fn score(h: &Hole, o: &course::Outcome) -> f32 {
    if o.sunk {
        -1.0e6 + o.frames as f32
    } else if o.water {
        1.0e5 + h.dist_at(o.rest)
    } else {
        h.dist_at(o.rest)
    }
}

fn release_time(t_ref: f32, hold: u32) -> f32 {
    t_ref + (AIM_FRAMES + hold + 1) as f32 / 60.0
}

pub fn drive(game: Res<Game>, course: Res<Course>, mut ctl: ResMut<Ctl>, mut au: ResMut<Auto>, dbg: Res<CartDebug>) {
    if !game.auto {
        au.state = 0;
        return;
    }
    au.frame += 1;
    let mut c = Ctl::default();
    let tap = au.frame % 30 == 0;
    match game.mode {
        Mode::Title | Mode::Intro | Mode::Sunk | Mode::Card => {
            c.a_just = tap;
            c.a = tap;
            au.state = 0;
        }
        Mode::Paused => c.start_just = tap,
        Mode::Roll => au.state = 0,
        Mode::Aim => {
            let h = &course.0[game.hole];
            match au.state {
                0 => {
                    // start a search
                    au.cands.clear();
                    for k in 0..ANGLES {
                        let a = k as f32 / ANGLES as f32 * std::f32::consts::TAU;
                        let mut n = HOLD_STEP;
                        while n <= MAX_HOLD {
                            au.cands.push((a, n));
                            n += HOLD_STEP;
                        }
                    }
                    au.next = 0;
                    au.best = None;
                    au.refined = false;
                    au.t_ref = game.hole_t + SEARCH_FRAMES as f32 / 60.0;
                    au.state = 1;
                }
                1 => {
                    let end = (au.next + PER_FRAME).min(au.cands.len());
                    for i in au.next..end {
                        let (a, n) = au.cands[i];
                        let p = n as f32 / 60.0 / CHARGE_SECS;
                        let o = course::simulate(h, game.ball.pos, a, p, release_time(au.t_ref, n), 900);
                        let s = score(h, &o);
                        if au.best.is_none_or(|b| s < b.0) {
                            au.best = Some((s, a, n));
                        }
                    }
                    au.next = end;
                    if au.next >= au.cands.len() {
                        if !au.refined {
                            // refine around the best: finer angles, every hold
                            au.refined = true;
                            let (_, a, n) = au.best.unwrap();
                            au.cands.clear();
                            for da in -6..=6 {
                                for dn in -2i32..=2 {
                                    let nn = (n as i32 + dn).clamp(1, MAX_HOLD as i32) as u32;
                                    au.cands.push((a + da as f32 * 0.009, nn));
                                }
                            }
                            au.next = 0;
                        } else {
                            // auto_noise > 0 plays like a person: a little off in
                            // aim and power (to judge how hard the holes are)
                            let noise = dbg.get("auto_noise").unwrap_or(0.0);
                            if noise > 0.0 {
                                au.seed = au.seed.wrapping_mul(1664525).wrapping_add(1013904223);
                                let r1 = (au.seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
                                au.seed = au.seed.wrapping_mul(1664525).wrapping_add(1013904223);
                                let r2 = (au.seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0;
                                if let Some(b) = au.best.as_mut() {
                                    b.1 += r1 * 0.035 * noise;
                                    b.2 = ((b.2 as f32) * (1.0 + r2 * 0.08 * noise)).round().clamp(1.0, MAX_HOLD as f32) as u32;
                                }
                            }
                            au.state = 2;
                        }
                    }
                }
                2 => {
                    // wait for the planned moment (the turnstile's phase)
                    if game.hole_t + 0.5 / 60.0 >= au.t_ref {
                        au.state = 3;
                        au.count = 0;
                    }
                }
                3 => {
                    let (_, a, _) = au.best.unwrap();
                    c.stick = Vec2::new(a.cos(), a.sin());
                    au.count += 1;
                    if au.count >= AIM_FRAMES {
                        au.state = 4;
                        au.count = 0;
                        c.a = true;
                        c.a_just = true;
                    }
                }
                4 => {
                    let (_, a, n) = au.best.unwrap();
                    c.stick = Vec2::new(a.cos(), a.sin());
                    au.count += 1;
                    if au.count < n {
                        c.a = true;
                    } else {
                        // let go: the putt
                        au.state = 5;
                    }
                }
                _ => {}
            }
        }
    }
    *ctl = c;
}
