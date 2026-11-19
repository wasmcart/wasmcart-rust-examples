//! The test autopilot: plays Hillfold the way its screens teach, switched on
//! by a debugging host writing the debug field `autopilot` to 1. It reads the
//! same state a player sees and drives the same controls a pad does.
//!
//! Herding follows the usual shepherding rule: if a sheep has strayed far
//! from the flock, get behind it and push it back in (collect); otherwise get
//! behind the whole flock, on the far side from the fold, and walk it home
//! (drive). Routes go round walls and the brook on a coarse grid, and keep
//! clear of the flock so the dog does not split it.

use bevy::prelude::*;

use crate::world::{Grid, gate_mid};
use crate::{Ctl, Dog, Field, Game, Mode, Sheep};

#[derive(Resource, Default)]
pub struct Auto {
    nav: Vec<f32>,
    nav_goal: Vec2,
    to_flock: Vec<f32>,
    replan: f32,
    frame: u32,
    stall: f32,
    last_penned: usize,
    since_pen: f32,
    bark_cd: f32,
    pub state: u32,
    pub target: Vec2,
    stuck: f32,
    side: f32,
    pub way: Vec2,
}

fn penalty(grid: &Grid, sheep: &[Vec2], r: f32) -> Vec<f32> {
    let mut pen = vec![0.0; grid.clear.len()];
    for s in sheep {
        let c = grid.cell(*s);
        let (cx, cz) = ((c % grid.nx) as i32, (c / grid.nx) as i32);
        let k = r.ceil() as i32;
        for dz in -k..=k {
            for dx in -k..=k {
                let (x, z) = (cx + dx, cz + dz);
                if x < 0 || z < 0 || x >= grid.nx as i32 || z >= grid.nz as i32 {
                    continue;
                }
                let i = z as usize * grid.nx + x as usize;
                let d = grid.center(i).distance(*s);
                if d < r {
                    pen[i] += 6.0 * (1.0 - d / r);
                }
            }
        }
    }
    pen
}

pub fn drive(
    time: Res<Time>,
    game: Res<Game>,
    field: Option<Res<Field>>,
    dogs: Query<&Dog>,
    sheep: Query<&Sheep>,
    mut ctl: ResMut<Ctl>,
    mut au: ResMut<Auto>,
) {
    if !game.auto {
        return;
    }
    let dt = time.delta_secs().min(0.05);
    au.frame += 1;
    let mut c = Ctl::default();
    let tap = au.frame % 30 == 0;
    match game.mode {
        Mode::Title | Mode::Intro | Mode::Cleared | Mode::Over | Mode::Victory => c.a_just = tap,
        Mode::Paused => c.start_just = tap,
        Mode::Play => {}
    }
    let (Some(field), Ok(dog)) = (field, dogs.single()) else {
        *ctl = c;
        return;
    };
    if game.mode != Mode::Play {
        au.replan = 0.0;
        au.since_pen = 0.0;
        au.last_penned = game.penned;
        *ctl = c;
        return;
    }
    let grid = &field.grid;
    let obs = &field.obs;
    let all_free: Vec<Vec2> = sheep.iter().filter(|s| !s.penned).map(|s| s.pos).collect();
    // work on one group at a time: the group nearest home along the path
    let free = nearest_group(&all_free, grid, &field.home);
    if game.penned != au.last_penned {
        au.last_penned = game.penned;
        au.since_pen = 0.0;
    }
    au.since_pen += dt;
    au.bark_cd = (au.bark_cd - dt).max(0.0);

    // what to do
    let enough = game.penned >= game.need;
    let close_gate = free.is_empty()
        || (enough && (game.time_left < 35.0 || au.since_pen > 20.0 || all_free.len() <= 1));
    let n = free.len().max(1) as f32;
    let mut cen = if free.is_empty() { obs.pen } else { free.iter().copied().sum::<Vec2>() / n };
    if obs.clearance(cen) < 0.6 {
        // the middle of the group is inside a wall: use the sheep nearest it
        if let Some(p) = free.iter().min_by(|a, b| a.distance(cen).total_cmp(&b.distance(cen))) {
            cen = *p;
        }
    }
    let collect_r = 1.5 + 1.1 * n.sqrt();
    let (far_i, far_d) = free
        .iter()
        .enumerate()
        .map(|(i, p)| (i, p.distance(cen)))
        .fold((0, 0.0), |a, b| if b.1 > a.1 { b } else { a });
    let gm = gate_mid(obs.pen, obs.gate);

    au.replan -= dt;
    let target;
    let mut careful = false;
    if close_gate {
        target = obs.pen;
        au.state = 3;
    } else if far_d > collect_r {
        // collect the straggler: stand behind it, on the side away from the flock
        if au.replan <= 0.0 {
            au.to_flock = grid.distances(cen, 0.55, |_| 0.0);
        }
        let s = free[far_i];
        let way = if au.to_flock.is_empty() { (cen - s).normalize_or_zero() } else { grid.downhill(&au.to_flock, s) };
        target = s - way * 2.6;
        careful = true;
        au.state = 1;
    } else {
        // drive the flock home along the path the sheep must take
        let way = grid.downhill(&field.home, cen);
        au.way = way;
        let near_gate = cen.distance(gm) < 6.0;
        let back = far_d.min(collect_r) + if near_gate { 1.6 } else { 2.6 };
        target = cen - way * back;
        au.state = 2;
    }
    let target = obs.push_out(target, 0.4);
    au.target = target;

    if au.replan <= 0.0 || au.nav_goal.distance(target) > 2.0 {
        au.replan = 0.3;
        au.nav_goal = target;
        let pen = penalty(grid, &all_free, if close_gate { 0.0 } else { 2.8 });
        au.nav = grid.distances(target, 0.8, |p| pen[grid.cell(p)]);
    }
    let to = target - dog.pos;
    let mut mv = if to.length() < 2.5 || au.nav.is_empty() { to / 2.5 } else { grid.downhill(&au.nav, dog.pos) };
    // close to the flock, walk; far away, run
    let near_flock = all_free.iter().any(|p| p.distance(dog.pos) < 4.0);
    if near_flock && to.length() < 3.5 {
        mv *= 0.55;
    }
    // walk round anything the coarse grid missed, and if the dog has not
    // moved for a moment while trying to, step sideways for a bit
    mv += obs.avoid(dog.pos, mv) * 0.6;
    if mv.length() > 0.5 && dog.vel.length() < 0.8 {
        au.stuck += dt;
    } else {
        au.stuck = (au.stuck - dt).max(0.0);
    }
    if au.stuck > 0.4 {
        au.side = 0.8;
        au.stuck = 0.0;
    }
    if au.side > 0.0 {
        au.side -= dt;
        mv = Vec2::new(-mv.y, mv.x) + mv * 0.3;
    }
    c.mv = mv.clamp_length_max(1.0);
    // sneak while circling round the flock
    c.b = near_flock && to.length() > 3.0 && !close_gate;
    let _ = careful;
    // a stalled flock gets a bark from behind
    let flock_moving = game.flock_c.distance(cen) > 0.0;
    let _ = flock_moving;
    if au.state == 2 && to.length() < 2.0 {
        au.stall += dt;
    } else {
        au.stall = 0.0;
    }
    if au.stall > 4.0 && au.bark_cd <= 0.0 {
        c.a_just = true;
        au.bark_cd = 3.0;
        au.stall = 0.0;
    }
    *ctl = c;
}

/// Sheep linked by gaps under 3.5 m form a group; returns the group whose
/// middle is the shortest walk from the fold.
fn nearest_group(free: &[Vec2], grid: &Grid, home: &[f32]) -> Vec<Vec2> {
    let n = free.len();
    let mut group = vec![usize::MAX; n];
    let mut g = 0;
    for i in 0..n {
        if group[i] != usize::MAX {
            continue;
        }
        group[i] = g;
        let mut stack = vec![i];
        while let Some(a) = stack.pop() {
            for b in 0..n {
                if group[b] == usize::MAX && free[a].distance(free[b]) < 3.5 {
                    group[b] = g;
                    stack.push(b);
                }
            }
        }
        g += 1;
    }
    let mut best = (f32::MAX, 0);
    for k in 0..g {
        let m: Vec<Vec2> = (0..n).filter(|i| group[*i] == k).map(|i| free[i]).collect();
        // the group's distance home: its members' mean path distance
        let d = m.iter().map(|p| grid.at(home, *p).min(500.0)).sum::<f32>() / m.len() as f32;
        // a lone sheep waits for the bigger groups unless it is much closer
        let d = d + if m.len() < 3 { 8.0 } else { 0.0 };
        if d < best.0 {
            best = (d, k);
        }
    }
    (0..n).filter(|i| group[*i] == best.1).map(|i| free[i]).collect()
}
