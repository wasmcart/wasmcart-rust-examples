//! A test autopilot behind the `autopilot` debug value (this port's
//! addition; a player never sees it).
//!
//! It plays your side with the game's own AI, but only through gamepad
//! presses: each decision is one press of the d-pad, A, X or Y, fed to the
//! same handler a real pad feeds. `autopilot` 1 presses them itself; 2 only
//! publishes the press it would make as `ap_bits` (WC_BTN_* bits), so a test
//! can press it on a real host pad instead.

use crate::{
    core::{
        battle::{
            ability::Ability,
            ai::Ai,
            check,
            command::{self, Command},
            movement::{self, Pathfinder},
            state, Id, PlayerId, State,
        },
        map::{self, Distance, PosHex},
    },
    geom,
    input::Pad,
    screen::battle::{view::SelectionMode, Battle},
};

/// Frames between two presses.
const PACE: u32 = 4;
/// Presses one planned command may take before the autopilot gives up on it
/// and ends the turn.
const MAX_PRESSES: u32 = 80;

#[derive(Debug)]
pub struct Autopilot {
    ai: Ai,
    pathfinder: Pathfinder,
    plan: Option<Command>,
    presses: u32,
    wait: u32,
    /// The last plan and how many times in a row the AI has planned it: a
    /// command whose click changed nothing comes back.
    last_plan: String,
    repeats: u32,
    /// Turns ended in a row without striking anyone.
    idle_turns: u32,
}

impl Autopilot {
    pub fn new(map_radius: Distance) -> Self {
        Self {
            ai: Ai::new(PlayerId(0), map_radius),
            pathfinder: Pathfinder::new(map_radius),
            plan: None,
            presses: 0,
            wait: 0,
            last_plan: String::new(),
            repeats: 0,
            idle_turns: 0,
        }
    }
}

pub fn tick(b: &mut Battle) -> Option<Pad> {
    use wasmcart_macroquad::debug;
    let mode = debug::input_i32("autopilot", 0);
    if mode == 0 {
        return None;
    }
    if b.autopilot.wait > 0 {
        b.autopilot.wait -= 1;
        debug::set_i32("ap_bits", 0);
        return None;
    }
    let pad = decide(b);
    b.autopilot.wait = PACE;
    debug::set_i32("ap_bits", pad.bits());
    if mode == 1 && pad.any() {
        Some(pad)
    } else {
        None
    }
}

/// The next command: strike the weakest enemy any fighter can reach; else
/// hold the line and let the enemy come (moving next to an enemy invites its
/// reactive attacks); after two quiet turns, advance as the game's AI would.
fn plan(ap: &mut Autopilot, state: &State) -> Option<Command> {
    let mut enemies = state::enemy_agent_ids(state, PlayerId(0));
    enemies.sort_by_key(|&id| state.parts().strength.get(id).strength.0);
    let own = state::players_agent_ids(state, PlayerId(0));
    for &attacker_id in &own {
        for &target_id in &enemies {
            let command = command::Attack {
                attacker_id,
                target_id,
            }
            .into();
            if check(state, &command).is_ok() {
                ap.idle_turns = 0;
                return Some(command);
            }
        }
    }
    if let Some(command) = close_ranks(ap, state, &own, &enemies) {
        return Some(command);
    }
    if ap.idle_turns < 2 {
        ap.idle_turns += 1;
        return Some(command::EndTurn.into());
    }
    let command = ap.ai.command(state);
    if let Some(Command::EndTurn(_)) = command {
        ap.idle_turns = 0;
    }
    command
}

/// Keeps the fighters shoulder to shoulder around the one with the longest
/// reach, on the side facing the enemy, so whoever comes close walks into
/// its reactive strikes.
fn close_ranks(ap: &mut Autopilot, state: &State, own: &[Id], enemies: &[Id]) -> Option<Command> {
    let parts = state.parts();
    let pos = |id: Id| parts.pos.get(id).0;
    let anchor = *own
        .iter()
        .max_by_key(|&&id| parts.agent.get(id).attack_distance.0)?;
    let enemy_dist = |p: PosHex| {
        enemies
            .iter()
            .map(|&e| map::distance_hex(p, pos(e)).0)
            .min()
            .unwrap_or(0)
    };
    for &id in own {
        if id == anchor || map::distance_hex(pos(id), pos(anchor)).0 <= 1 {
            continue;
        }
        ap.pathfinder.fill_map(state, id);
        let mut best: Option<(i32, i32, movement::Path)> = None;
        for dir in map::dirs() {
            let to = map::Dir::get_neighbor_pos(pos(anchor), dir);
            if !state.map().is_inboard(to) {
                continue;
            }
            let path = match ap.pathfinder.path(to) {
                Some(path) => path,
                None => continue,
            };
            let cost = path.cost_for(state, id).0;
            let key = (cost, enemy_dist(to));
            if best.as_ref().map_or(true, |(c, d, _)| key < (*c, *d)) {
                best = Some((key.0, key.1, path));
            }
        }
        let path = best?.2.truncate(state, id)?;
        let command = command::MoveTo { id, path }.into();
        if check(state, &command).is_ok() {
            return Some(command);
        }
    }
    None
}

fn decide(b: &mut Battle) -> Pad {
    let idle = Pad::default();
    if b.block_timer.is_some()
        || b.view.any_unfinished_actions()
        || b.state.battle_result().is_some()
        || (b.panel_end_turn.is_none() && b.mode == SelectionMode::Normal)
    {
        return idle;
    }
    if b.autopilot.plan.is_none() {
        b.autopilot.plan = plan(&mut b.autopilot, &b.state);
        b.autopilot.presses = 0;
        let text = format!("{:?}", b.autopilot.plan);
        if text == b.autopilot.last_plan {
            b.autopilot.repeats += 1;
        } else {
            b.autopilot.repeats = 0;
            b.autopilot.last_plan = text;
        }
        if b.autopilot.repeats >= 2 {
            b.autopilot.presses = MAX_PRESSES;
        }
    }
    b.autopilot.presses += 1;
    if b.autopilot.presses > MAX_PRESSES {
        b.autopilot.plan = None;
        return if b.mode == SelectionMode::Normal {
            Pad { y: true, ..idle }
        } else {
            Pad { b: true, ..idle }
        };
    }
    let (id, target, ability) = match b.autopilot.plan.clone() {
        Some(Command::MoveTo(c)) => (c.id, c.path.to(), None),
        Some(Command::Attack(c)) => match b.state.parts().pos.get_opt(c.target_id) {
            Some(target) => (c.attacker_id, target.0, None),
            None => {
                b.autopilot.plan = None;
                return idle;
            }
        },
        Some(Command::UseAbility(c)) => (c.id, c.pos, Some(c.ability)),
        _ => {
            b.autopilot.plan = None;
            return if b.mode == SelectionMode::Normal {
                Pad { y: true, ..idle }
            } else {
                Pad { b: true, ..idle }
            };
        }
    };
    if b.state.parts().agent.get_opt(id).is_none() {
        b.autopilot.plan = None;
        return idle;
    }
    let pad = act(b, id, target, ability);
    if pad.a && b.selected_agent_id == Some(id) {
        // The command's own click: done, plan the next one.
        b.autopilot.plan = None;
    }
    pad
}

/// The next press toward: select `id`, pick `ability` if any, click `target`.
fn act(b: &Battle, id: Id, target: PosHex, ability: Option<Ability>) -> Pad {
    let idle = Pad::default();
    if b.selected_agent_id != Some(id) {
        if let SelectionMode::Ability(_) = b.mode {
            return Pad { b: true, ..idle };
        }
        let pos = b.state.parts().pos.get(id).0;
        return step_or_click(b, pos);
    }
    if let Some(ability) = ability {
        if b.mode != SelectionMode::Ability(ability) {
            return Pad { x: true, ..idle };
        }
    }
    step_or_click(b, target)
}

fn step_or_click(b: &Battle, to: PosHex) -> Pad {
    let idle = Pad::default();
    if b.cursor == to {
        return Pad { a: true, ..idle };
    }
    let size = b.view.tile_size();
    let from_point = geom::hex_to_point(size, b.cursor);
    let to_point = geom::hex_to_point(size, to);
    let mut best: Option<(i32, f32, PosHex)> = None;
    for dir in map::dirs() {
        let pos = map::Dir::get_neighbor_pos(b.cursor, dir);
        if !b.state.map().is_inboard(pos) {
            continue;
        }
        let d = map::distance_hex(pos, to).0;
        let px = geom::hex_to_point(size, pos).distance(to_point);
        if best.map_or(true, |(bd, bp, _)| (d, px) < (bd, bp)) {
            best = Some((d, px, pos));
        }
    }
    let (_, _, next) = match best {
        Some(best) => best,
        None => return idle,
    };
    let delta = geom::hex_to_point(size, next) - from_point;
    let dx = if delta.x > 0.01 {
        1
    } else if delta.x < -0.01 {
        -1
    } else {
        0
    };
    let dy = (next.r - b.cursor.r).signum();
    Pad {
        dir: Some((dx, dy)),
        ..idle
    }
}
